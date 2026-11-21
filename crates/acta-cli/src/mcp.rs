//! `acta mcp`: the stdio MCP server (ADR 0005, section 2). Tools only, plain
//! text results, no `structuredContent`. Everything a tool does is in
//! `acta_core::agent`; this file only maps MCP calls onto it.

use crate::open::Opener;
use acta_core::agent::args::*;
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
}

#[derive(Clone)]
pub struct ActaServer {
    state: Arc<Mutex<State>>,
    tool_router: ToolRouter<ActaServer>,
}

/// Drops what costs context and tells the model nothing: `$schema`, integer
/// formats, `null` in the types of optional fields (they are already absent
/// from `required`) and `null` defaults. `tools/list` stays under 7,000
/// characters (ADR 0005).
pub(crate) fn compact_schema(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::Object(o) => {
            o.remove("$schema");
            o.remove("format");
            if o.get("default") == Some(&Value::Null) {
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
            })),
            tool_router: Self::compact_router(),
        }
    }

    fn compact_router() -> ToolRouter<ActaServer> {
        let mut router = Self::tool_router();
        for route in router.map.values_mut() {
            let mut schema = serde_json::Value::Object((*route.attr.input_schema).clone());
            compact_schema(&mut schema);
            if let serde_json::Value::Object(o) = schema {
                route.attr.input_schema = Arc::new(o);
            }
        }
        router
    }

    fn run(&self, f: impl FnOnce(&mut Agent<'_>) -> Reply) -> CallToolResult {
        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let state = &mut *guard;
        if state.open.is_none() {
            match state.opener.open() {
                Ok(store) => {
                    let session = Session::resolve(&store, &state.launch, Author::Claude);
                    state.open = Some((store, session));
                }
                Err(msg) => return result(Err(msg)),
            }
        }
        let (store, session) = state.open.as_mut().expect("opened above");
        let mut agent = Agent { store, session };
        result(f(&mut agent))
    }
}

#[tool_router]
impl ActaServer {
    #[tool(
        name = "brief",
        description = "Call when unsure where things stand: the focus topic's state, last entries and counts; an epic's topics; or the project overview. With focus=true, id becomes this session's focus.",
        annotations(read_only_hint = true),
        meta = always_load()
    )]
    fn brief(&self, Parameters(a): Parameters<BriefArgs>) -> CallToolResult {
        self.run(|ag| ag.brief(a))
    }

    #[tool(
        name = "entries",
        description = "Call to answer a question about a topic's history: its timeline, newest first, a page at a time. id returns one entry in full.",
        annotations(read_only_hint = true)
    )]
    fn entries(&self, Parameters(a): Parameters<EntriesArgs>) -> CallToolResult {
        self.run(|ag| ag.entries(a))
    }

    #[tool(
        name = "log",
        description = "Call after a decision, a finished step or a dead end worth not repeating (never per file edit): appends to the topic's timeline and can refresh its state and status in the same call.",
        annotations(idempotent_hint = false),
        meta = always_load()
    )]
    fn log(&self, Parameters(a): Parameters<LogArgs>) -> CallToolResult {
        self.run(|ag| ag.log(a))
    }

    #[tool(
        name = "update",
        description = "Call to change a topic, epic or note: title, description, state, status, or move a topic to another epic. A note's status open puts it back in the inbox.",
        annotations(idempotent_hint = false)
    )]
    fn update(&self, Parameters(a): Parameters<UpdateArgs>) -> CallToolResult {
        self.run(|ag| ag.update(a))
    }

    #[tool(
        name = "find",
        description = "Call to locate topics, epics, notes or timeline entries in this project, by query or filters. kind=note without query lists the inbox.",
        annotations(read_only_hint = true)
    )]
    fn find(&self, Parameters(a): Parameters<FindArgs>) -> CallToolResult {
        self.run(|ag| ag.find(a))
    }

    #[tool(
        name = "create",
        description = "Call to create topics, epics or notes (up to 10), or to apply an agreed inbox triage in one all-or-nothing call: from_notes files notes into each item, kind=entry adds them to an existing topic, dismiss closes the rest.",
        annotations(idempotent_hint = false)
    )]
    fn create(&self, Parameters(a): Parameters<CreateArgs>) -> CallToolResult {
        self.run(|ag| ag.create(a))
    }

    #[tool(
        name = "kb",
        description = "Call before working on an unfamiliar area: lists the documents and memories in scope (topic, then epic, then project) with one-line summaries, or searches them with query.",
        annotations(read_only_hint = true)
    )]
    fn kb(&self, Parameters(a): Parameters<KbArgs>) -> CallToolResult {
        self.run(|ag| ag.kb(a))
    }

    #[tool(
        name = "kb_read",
        description = "Call to read a knowledge entry whose summary matches your task: whole when small, else its outline; then one section or a line range.",
        annotations(read_only_hint = true)
    )]
    fn kb_read(&self, Parameters(a): Parameters<KbReadArgs>) -> CallToolResult {
        self.run(|ag| ag.kb_read(a))
    }

    #[tool(
        name = "kb_write",
        description = "Call to save a durable fact (memory) or a long reference (document), or to edit one by section. Use it instead of your own memory files; search with kb first.",
        annotations(idempotent_hint = false)
    )]
    fn kb_write(&self, Parameters(a): Parameters<KbWriteArgs>) -> CallToolResult {
        self.run(|ag| ag.kb_write(a))
    }

    #[tool(
        name = "hook",
        description = "Internal: called by Acta's hooks. Never call it yourself.",
        annotations(read_only_hint = false, idempotent_hint = false)
    )]
    fn hook(&self, Parameters(a): Parameters<HookArgs>) -> CallToolResult {
        self.run(|ag| ag.hook(a))
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
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| e.to_string())?;
        service.waiting().await.map_err(|e| e.to_string())?;
        Ok(())
    })
}
