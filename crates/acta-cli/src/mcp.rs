//! `acta mcp`: the stdio MCP server (ADR 0005, section 2). Tools only, plain
//! text results, no `structuredContent`. Everything a tool does is in
//! `acta_core::agent`; this file only maps MCP calls onto it.

use crate::open::Opener;
use acta_core::agent::args::*;
use acta_core::agent::presence::{Event, HEARTBEAT_MS, Tracker};
use acta_core::agent::{Agent, INSTRUCTIONS, Launch, Reply, Session};
use acta_core::model::Author;
use acta_core::store::Store;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, MetaObject, ServerCapabilities, ServerConfig,
};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use std::sync::{Arc, Mutex};

/// Opened lazily on the first call, so a server whose keychain is slow or
/// locked still starts and answers `tools/list`.
struct State {
    opener: Opener,
    launch: Launch,
    open: Option<(Store, Session)>,
    /// Session presence; `None` until registered, and for sessions in
    /// folders Acta does not track.
    tracker: Option<Tracker>,
    /// Whether this session may open the database for presence at all
    /// (`Launch::worth_opening`), decided once.
    tracked: Option<bool>,
}

impl State {
    fn ensure_open(&mut self) -> Result<(), String> {
        if self.open.is_none() {
            let store = self.opener.open()?;
            let session = Session::resolve(&store, &self.launch, Author::Claude);
            self.open = Some((store, session));
        }
        Ok(())
    }

    /// The presence tracker, registering the session on first use. Never
    /// opens the database for a session in an unrelated folder.
    fn tracker(&mut self) -> Option<&mut Tracker> {
        if self.tracker.is_none() {
            let launch = &self.launch;
            let opener = &self.opener;
            let tracked = *self
                .tracked
                .get_or_insert_with(|| opener.data_dir().is_some_and(|d| launch.worth_opening(&d)));
            if !tracked {
                return None;
            }
            self.ensure_open().ok()?;
            let (store, session) = self.open.as_ref()?;
            let token = self.launch.token.clone();
            self.tracker = Tracker::start(store, session, token, now()).ok().flatten();
        }
        self.tracker.as_mut()
    }

    /// Presence is definitely not kept for this session: its folder is not
    /// linked, or the database opened and the session is bound to no
    /// project or has no id. A failure to open is not this: it is retried.
    fn untracked(&self) -> bool {
        self.tracked == Some(false)
            || self
                .open
                .as_ref()
                .is_some_and(|(_, s)| s.project.is_none() || s.session_id.is_none())
    }
}

fn now() -> i64 {
    acta_core::time::now_ms()
}

#[derive(Clone)]
pub struct ActaServer {
    state: Arc<Mutex<State>>,
    tool_router: ToolRouter<ActaServer>,
}

/// Drops what costs context and tells the model nothing: `$schema`, integer
/// formats, `null` in the types of optional fields (they are already absent
/// from `required`) and empty defaults (`null`, `""`, `false`). `tools/list` stays under 7,000
/// characters (ADR 0005).
pub(crate) fn compact_schema(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::Object(o) => {
            o.remove("$schema");
            o.remove("format");
            // A default that only says "absent" (null, "", false) tells
            // the model nothing the optional field does not.
            if matches!(
                o.get("default"),
                Some(Value::Null) | Some(Value::Bool(false))
            ) || o.get("default").and_then(Value::as_str) == Some("")
            {
                o.remove("default");
            }
            if let Some(Value::Array(types)) = o.get("type") {
                let kept: Vec<Value> = types.iter().filter(|t| *t != "null").cloned().collect();
                if kept.len() == 1 {
                    o.insert("type".into(), kept[0].clone());
                }
            }
            if let Some(Value::Array(any)) = o.get("anyOf")
                && any.len() == 2
                && let Some(i) = any
                    .iter()
                    .position(|x| x.get("type") == Some(&Value::from("null")))
            {
                let other = any[1 - i].clone();
                o.remove("anyOf");
                if let Value::Object(other) = other {
                    o.extend(other);
                }
            }
            for x in o.values_mut() {
                compact_schema(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(compact_schema),
        _ => {}
    }
}

/// Replaces each `{"$ref": "#/$defs/X"}` with the definition and drops
/// `$defs`: a type used once costs its name and the indirection for
/// nothing. The argument types are not recursive; a schema that still has
/// a reference after a few rounds keeps its `$defs`.
pub(crate) fn inline_defs(schema: &mut serde_json::Value) {
    use serde_json::Value;
    let Some(defs) = schema.get("$defs").cloned() else {
        return;
    };
    fn walk(v: &mut Value, defs: &Value) -> bool {
        match v {
            Value::Object(o) => {
                let target = o
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|r| r.strip_prefix("#/$defs/"))
                    .and_then(|n| defs.get(n))
                    .cloned();
                let mut found = false;
                if let Some(Value::Object(d)) = target {
                    o.remove("$ref");
                    // Fields beside the reference (its description) win.
                    for (k, x) in d {
                        o.entry(k).or_insert(x);
                    }
                    found = true;
                }
                o.iter_mut()
                    .filter(|(k, _)| *k != "$defs")
                    .fold(found, |acc, (_, x)| walk(x, defs) | acc)
            }
            Value::Array(a) => a.iter_mut().fold(false, |acc, x| walk(x, defs) | acc),
            _ => false,
        }
    }
    for _ in 0..4 {
        if !walk(schema, &defs) {
            if let Some(o) = schema.as_object_mut() {
                o.remove("$defs");
            }
            return;
        }
    }
}

/// The tool only hooks call (ADR 0005, `hook`).
const HOOK_TOOL: &str = "hook";

fn always_load() -> MetaObject {
    let mut m = MetaObject::new();
    m.0.insert("anthropic/alwaysLoad".into(), serde_json::Value::Bool(true));
    m
}

fn result(reply: Reply) -> CallToolResult {
    match reply {
        Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(text) => CallToolResult::error(vec![ContentBlock::text(text)]),
    }
}

impl ActaServer {
    pub fn new(opener: Opener, launch: Launch) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                opener,
                launch,
                open: None,
                tracker: None,
                tracked: None,
            })),
            tool_router: Self::compact_router(),
        }
    }

    fn compact_router() -> ToolRouter<ActaServer> {
        let mut router = Self::tool_router();
        for route in router.map.values_mut() {
            let mut schema = serde_json::Value::Object((*route.attr.input_schema).clone());
            inline_defs(&mut schema);
            compact_schema(&mut schema);
            if let serde_json::Value::Object(o) = schema {
                route.attr.input_schema = Arc::new(o);
            }
        }
        router
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn run(&self, f: impl FnOnce(&mut Agent<'_>) -> Reply) -> CallToolResult {
        self.run_then(f, |_, _| {})
    }

    /// Runs a tool, then lets `after` report it to presence (only after a
    /// success; the session may have moved).
    fn run_then(
        &self,
        f: impl FnOnce(&mut Agent<'_>) -> Reply,
        after: impl FnOnce(&mut Tracker, &Session),
    ) -> CallToolResult {
        let mut guard = self.lock();
        let state = &mut *guard;
        if let Err(msg) = state.ensure_open() {
            return result(Err(msg));
        }
        let (store, session) = state.open.as_mut().expect("opened above");
        let mut agent = Agent { store, session };
        let reply = f(&mut agent);
        if reply.is_ok() {
            let session = state.open.as_ref().map(|(_, s)| s.clone());
            if let (Some(session), Some(tracker)) = (session, state.tracker()) {
                after(tracker, &session);
            }
        }
        result(reply)
    }

    /// Registers the session (if its folder is linked) and beats every
    /// [`HEARTBEAT_MS`] on the blocking pool; returns only once presence is
    /// definitely not kept for this session (a failed open is retried). The database is opened here, off the transport, so a slow
    /// keychain never delays `initialize`.
    async fn heartbeat(&self) {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(HEARTBEAT_MS as u64));
        loop {
            interval.tick().await;
            let state = Arc::clone(&self.state);
            let tracked = tokio::task::spawn_blocking(move || {
                let mut guard = match state.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                if let Some(t) = guard.tracker() {
                    let _ = t.heartbeat(now());
                    return true;
                }
                // Not yet (keychain locked, database busy): try again at
                // the next beat, unless the session will never be tracked.
                !guard.untracked()
            })
            .await
            .unwrap_or(true);
            if !tracked {
                return;
            }
        }
    }

    /// The client closed the connection: the session ended.
    fn ended(&self) {
        if let Some(t) = self.lock().tracker.as_mut() {
            let _ = t.end(now());
        }
    }

    /// A presence hook: updates the row. Presence-only events answer an
    /// empty text, so nothing reaches the model's context; `subagent` goes
    /// on to its briefing.
    fn presence_hook(&self, a: &HookArgs) -> Option<CallToolResult> {
        let event = a.event.trim().to_ascii_lowercase();
        let parsed = match event.as_str() {
            "subagent" | "subagentstart" => Some(Event::Subagent),
            e => Event::parse(e, a.detail.as_deref()),
        };
        let ev = parsed?;
        {
            let mut guard = self.lock();
            if let Some(t) = guard.tracker() {
                if let Some(id) = a.session.as_deref().filter(|s| !s.starts_with("${")) {
                    let _ = t.adopt(id, now());
                }
                let _ = t.on_event(&ev, now());
            }
        }
        (ev != Event::Subagent)
            .then(|| CallToolResult::success(vec![ContentBlock::text(String::new())]))
    }
}

#[tool_router]
impl ActaServer {
    #[tool(
        name = "brief",
        description = "When unsure where things stand: the focus topic's state, last entries and counts; an epic's topics; or the project overview. focus=true makes id the session's focus.",
        annotations(read_only_hint = true),
        meta = always_load()
    )]
    fn brief(&self, Parameters(a): Parameters<BriefArgs>) -> CallToolResult {
        self.run_then(
            |ag| ag.brief(a),
            |t, s| {
                let _ = t.follow(s);
            },
        )
    }

    #[tool(
        name = "entries",
        description = "To answer a question about a topic's history: its timeline, newest first, by page; id: one entry in full.",
        annotations(read_only_hint = true)
    )]
    fn entries(&self, Parameters(a): Parameters<EntriesArgs>) -> CallToolResult {
        self.run(|ag| ag.entries(a))
    }

    #[tool(
        name = "log",
        description = "After a decision, a finished step or a dead end worth not repeating (never per file edit): appends to the topic's timeline; can refresh its state and status too.",
        meta = always_load()
    )]
    fn log(&self, Parameters(a): Parameters<LogArgs>) -> CallToolResult {
        let text = a.text.clone();
        self.run_then(
            |ag| ag.log(a),
            |t, _| {
                let _ = t.activity(&text, now());
            },
        )
    }

    #[tool(
        name = "update",
        description = "Changes a topic, epic, note, question (its answer) or contact (its level). A replaced state stays in history; a note's status open puts it back in the inbox."
    )]
    fn update(&self, Parameters(a): Parameters<UpdateArgs>) -> CallToolResult {
        self.run(|ag| ag.update(a))
    }

    #[tool(
        name = "find",
        description = "Locates topics, epics, notes, entries, questions or contacts by query or filters. kind=note without query: the inbox; due: deadlines; person: what is open with someone.",
        annotations(read_only_hint = true)
    )]
    fn find(&self, Parameters(a): Parameters<FindArgs>) -> CallToolResult {
        self.run(|ag| ag.find(a))
    }

    #[tool(
        name = "create",
        description = "Creates topics, epics, notes, questions or contacts (up to 10), or applies an agreed inbox triage all-or-nothing: from_notes files notes into an item, kind=entry into a topic, dismiss closes the rest."
    )]
    fn create(&self, Parameters(a): Parameters<CreateArgs>) -> CallToolResult {
        self.run(|ag| ag.create(a))
    }

    #[tool(
        name = "kb",
        description = "Before working on an unfamiliar area: knowledge in scope (topic, epic, project) with one-line summaries; query searches it.",
        annotations(read_only_hint = true)
    )]
    fn kb(&self, Parameters(a): Parameters<KbArgs>) -> CallToolResult {
        self.run(|ag| ag.kb(a))
    }

    #[tool(
        name = "kb_read",
        description = "Reads a knowledge entry whose summary matches the task: whole when small, else its outline, then a section or lines.",
        annotations(read_only_hint = true)
    )]
    fn kb_read(&self, Parameters(a): Parameters<KbReadArgs>) -> CallToolResult {
        self.run(|ag| ag.kb_read(a))
    }

    #[tool(
        name = "kb_write",
        description = "Saves a durable fact (memory) or long reference (document), or edits one by section, instead of your own memory files; search with kb first."
    )]
    fn kb_write(&self, Parameters(a): Parameters<KbWriteArgs>) -> CallToolResult {
        self.run(|ag| ag.kb_write(a))
    }

    #[tool(
        name = "attachment",
        description = "Reads a file (A-n) from a files: segment when the task needs its content: text, PDF text or the image.",
        annotations(read_only_hint = true)
    )]
    fn attachment(&self, Parameters(a): Parameters<AttachmentArgs>) -> CallToolResult {
        let mut image = None;
        let call = self.run(|ag| {
            ag.attachment(a).map(|answer| {
                image = answer.image;
                answer.text
            })
        });
        // The picture after its text line (ADR 0012, 10): base64 in an
        // image block, never in text.
        match (call.is_error, image) {
            (Some(true), _) | (_, None) => call,
            (_, Some((bytes, media_type))) => {
                use base64::Engine as _;
                let mut content = call.content;
                content.push(ContentBlock::image(
                    base64::engine::general_purpose::STANDARD.encode(bytes),
                    media_type,
                ));
                CallToolResult::success(content)
            }
        }
    }

    #[tool(
        name = "hook",
        description = "Internal: called by Acta's hooks. Never call it yourself.",
        annotations(read_only_hint = false, idempotent_hint = false)
    )]
    fn hook(&self, Parameters(a): Parameters<HookArgs>) -> CallToolResult {
        if let Some(done) = self.presence_hook(&a) {
            return done;
        }
        let new_id = a.session.clone();
        self.run_then(
            |ag| ag.hook(a),
            |t, _| {
                // `clear`: the process goes on under the new session id.
                if let Some(id) = new_id.as_deref().filter(|s| !s.starts_with("${")) {
                    let _ = t.adopt(id, now());
                }
            },
        )
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ActaServer {
    /// Every tool except `hook`: the model is never offered it, so it cannot
    /// call it; `mcp_tool` hooks call it by name.
    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        let tools = self
            .tool_router
            .list_all()
            .into_iter()
            .filter(|t| t.name != HOOK_TOOL)
            .collect();
        // As the generated handler does: cache hints for 2026-07-28 peers.
        let hints = context
            .protocol_version()
            .is_some_and(|v| v >= rmcp::model::ProtocolVersion::V_2026_07_28);
        let mut result = rmcp::model::ListToolsResult::with_all_items(tools);
        result.ttl_ms = hints.then_some(0);
        result.cache_scope = hints.then_some(rmcp::model::CacheScope::Public);
        Ok(result)
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("acta", acta_core::VERSION))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Serves on stdin/stdout until the client closes the connection.
pub fn serve(opener: Opener) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let server = ActaServer::new(opener, Launch::from_env());
        let service = server
            .clone()
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| e.to_string())?;
        let beats = server.clone();
        let heartbeat = tokio::spawn(async move { beats.heartbeat().await });
        let done = service.waiting().await.map_err(|e| e.to_string());
        heartbeat.abort();
        let _ = tokio::task::spawn_blocking(move || server.ended()).await;
        done?;
        Ok(())
    })
}
