//! The nine agent tools of ADR 0005 and the hook briefings, as functions of
//! the domain layer. The MCP server and the CLI call these and print what
//! they return; neither holds any logic of its own.
//!
//! Every answer is plain text under the tool's budget ([`render`]). A refusal
//! is an `Err` with a one-line reason the agent can act on.

use super::args::*;
use super::render::{self, Text, budget, limit};
use super::session::{Session, parse_owner};
use crate::error::Error;
use crate::markdown;
use crate::model::{
    Author, Entry, EntryKind, EpicId, KbId, KbKind, KbSummary, Level, NoteId, Owner, PageReq,
    ProjectId, Record, Status, TopicId,
};
use crate::store::{
    EntryFilter, EpicPatch, KbEdit, KbFilter, KbUpdate, NewEntry, NewKb, NewTopic, SNIPPET_CLOSE,
    SNIPPET_OPEN, SearchKind, SearchQuery, Store, TopicFilter, TopicPatch,
};

/// The server `instructions` (ADR 0005, section 2). Under 800 characters.
pub const INSTRUCTIONS: &str = "Acta is this project's tracker and your working memory: topics (T-n) in epics (E-n), each with a current state, a timeline and a knowledge base (K-n: documents and memories). The session briefing names your focus topic. Call `brief` when unsure where things stand. After a meaningful step or a decision call `log`, and refresh the state in the same call. Save durable facts with `kb_write` instead of your own memory files; check `kb` before working on an unfamiliar area. Tools default to the focus topic; pass an id to address another. Subagents always pass ids. Do not copy code or long output into Acta.";

pub const NOT_LINKED: &str = "This folder is not linked to an Acta project, so Acta is off here. The user can link it with `acta link`.";

/// What a tool returns: the text for the agent, or a refusal.
pub type Reply = Result<String, String>;

fn err(e: Error) -> String {
    match e {
        Error::NotFound(what) => format!("{what} not found (or in the trash)"),
        other => other.to_string(),
    }
}

/// The tools, bound to one open store and one session.
pub struct Agent<'a> {
    pub store: &'a mut Store,
    pub session: &'a mut Session,
}

fn status_of(text: &str) -> Result<Status, String> {
    Status::parse(&text.trim().to_ascii_lowercase()).ok_or_else(|| {
        format!(
            "unknown status \"{}\": use draft, planned, active, blocked, done or dropped",
            render::clip(text, 20)
        )
    })
}

fn entry_kind(text: &str) -> Result<EntryKind, String> {
    match EntryKind::parse(&text.trim().to_ascii_lowercase()) {
        Some(k) if k != EntryKind::System => Ok(k),
        _ => Err("type must be progress, decision or discussion".into()),
    }
}

fn kb_kind(text: &str) -> Result<KbKind, String> {
    match text.trim().to_ascii_lowercase().as_str() {
        "document" | "doc" => Ok(KbKind::Document),
        "memory" | "mem" => Ok(KbKind::Memory),
        _ => Err("kind must be document or memory".into()),
    }
}

fn check_len(field: &str, text: &str, max: usize) -> Result<(), String> {
    let n = render::chars(text);
    if n > max {
        Err(render::over_limit(field, max, n))
    } else {
        Ok(())
    }
}

fn kind_word(k: KbKind) -> &'static str {
    match k {
        KbKind::Document => "doc",
        KbKind::Memory => "mem",
    }
}

fn level_word(o: Owner) -> &'static str {
    match o {
        Owner::Project(_) => "project",
        Owner::Epic(_) => "epic",
        Owner::Topic(_) => "topic",
    }
}

fn strip_marks(s: &str) -> String {
    s.replace([SNIPPET_OPEN, SNIPPET_CLOSE], "")
}

/// `T-42#14` into its topic and number.
fn parse_entry_ref(text: &str) -> Option<(TopicId, i64)> {
    let (t, n) = text.trim().split_once('#')?;
    Some((TopicId::parse(t)?, n.trim().parse().ok()?))
}

/// Lower-case words of three letters or more: what near-duplicate detection
/// compares.
fn words(text: &str) -> Vec<String> {
    let mut w: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_lowercase)
        .collect();
    w.sort();
    w.dedup();
    w
}

fn jaccard(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.iter().filter(|w| b.contains(w)).count() as f64;
    let union = (a.len() + b.len()) as f64 - inter;
    inter / union
}

/// Whether a new memory closely matches an existing one (ADR 0005 `kb_write`).
pub fn near_duplicate(title: &str, summary: &str, other: &KbSummary) -> bool {
    let (ta, tb) = (words(title), words(&other.title));
    if !ta.is_empty() && ta == tb {
        return true;
    }
    jaccard(&ta, &tb) >= 0.6 || jaccard(&words(summary), &words(&other.summary)) >= 0.6
}

const REFUSED_KEY: &str = "kb.refused";

impl Agent<'_> {
    fn author(&self) -> Author {
        self.session.author
    }

    fn project(&self) -> Result<ProjectId, String> {
        self.session.project.ok_or_else(|| NOT_LINKED.to_owned())
    }

    /// An unbound session reaches nothing, not even by explicit id: the
    /// plugin runs in every repository, and a prompt there must not read or
    /// write the user's projects. A bound one never reaches another project.
    fn check_project(&self, project: ProjectId, what: &str) -> Result<(), String> {
        match self.session.project {
            None => Err(NOT_LINKED.to_owned()),
            Some(p) if p != project => Err(format!(
                "{what} is in another project; Acta tools stay in this session's project"
            )),
            _ => Ok(()),
        }
    }

    fn owner_project(&self, owner: Owner) -> Result<ProjectId, String> {
        let p = match owner {
            Owner::Project(p) => self.store.project(p).map(|p| p.id),
            Owner::Epic(e) => self.store.epic(e).map(|e| e.project),
            Owner::Topic(t) => self.store.topic(t).map(|t| t.project),
        }
        .map_err(err)?;
        self.check_project(p, &owner.to_string())?;
        Ok(p)
    }

    fn topic_arg(&self, given: Option<&str>) -> Result<TopicId, String> {
        match given {
            Some(t) => {
                let id = TopicId::parse(t).ok_or("topic must be a topic id such as T-42")?;
                self.owner_project(Owner::Topic(id))?;
                Ok(id)
            }
            None => {
                match self.session.focus {
                    Some(Owner::Topic(t)) => Ok(t),
                    _ => {
                        self.project()?;
                        Err("no focus topic: pass topic=\"T-n\", or set one with brief(id, focus=true)".into())
                    }
                }
            }
        }
    }

    fn epic_arg(&self, given: &str) -> Result<EpicId, String> {
        let id = EpicId::parse(given).ok_or("epic must be an epic id such as E-3")?;
        self.owner_project(Owner::Epic(id))?;
        Ok(id)
    }

    /// The scope an agent works in: the given owner, else the focus, else
    /// the project.
    fn scope(&self, given: Option<&str>) -> Result<Owner, String> {
        match given {
            Some(id) => {
                let o = parse_owner(id).ok_or("id must be T-n, E-n or P-n")?;
                self.owner_project(o)?;
                Ok(o)
            }
            None => Ok(self
                .session
                .focus
                .unwrap_or(Owner::Project(self.project()?))),
        }
    }

    // ---- brief ----

    pub fn brief(&mut self, a: BriefArgs) -> Reply {
        self.project()?;
        let target = match a.id.as_deref() {
            Some(id) => {
                let o = parse_owner(id).ok_or("id must be T-n, E-n or P-n")?;
                self.owner_project(o)?;
                if a.focus {
                    self.session.set_focus(self.store, o).map_err(err)?;
                }
                o
            }
            None => self.scope(None)?,
        };
        self.render_brief(target, budget::BRIEF, true)
    }

    fn focus_mark(&self, o: Owner) -> &'static str {
        if self.session.focus == Some(o) {
            " · focus"
        } else {
            ""
        }
    }

    fn counts_line(&self, scope: Owner, project: ProjectId, entries: Option<i64>) -> String {
        let kb = self.store.kb_counts(scope).unwrap_or_default();
        let inbox = self
            .store
            .inbox(project, PageReq::first(1))
            .map(|p| p.total)
            .unwrap_or(0);
        let mut parts = Vec::new();
        if let Some(n) = entries {
            parts.push(plural(n, "entry", "entries"));
        }
        parts.push(format!(
            "knowledge in scope: {}, {}",
            plural(kb.documents, "document", "documents"),
            plural(kb.memories, "memory", "memories")
        ));
        parts.push(format!("inbox {inbox}"));
        parts.join(" · ")
    }

    /// `fence`: wrap the stored text in data fences; the session briefing
    /// passes `false` because it fences its whole body.
    pub(crate) fn render_brief(&self, o: Owner, max: usize, fence: bool) -> Reply {
        let mut out = Text::new(max);
        match o {
            Owner::Topic(id) => {
                let t = self.store.topic(id).map_err(err)?;
                let epic = match t.epic {
                    Some(e) => self
                        .store
                        .epics(t.project)
                        .map_err(err)?
                        .into_iter()
                        .find(|s| s.id == e)
                        .map(|s| {
                            format!(
                                " · {} {} ({}/{} done)",
                                s.id,
                                render::clip(&s.title, 40),
                                s.progress.done,
                                s.progress.total
                            )
                        })
                        .unwrap_or_default(),
                    None => String::new(),
                };
                out.line_clipped(format!(
                    "{id} {} · {}{epic}{}",
                    render::clip(&render::one_line(&t.title), 80),
                    t.status.as_str(),
                    self.focus_mark(o)
                ));
                let counts = self.counts_line(o, t.project, None);
                let page = self
                    .store
                    .entries(id, &EntryFilter::default(), PageReq::first(3))
                    .map_err(err)?;
                // Reserve room for the counts and the closing fence so they
                // always print.
                let close = if fence {
                    render::DATA_CLOSE.len() + 1
                } else {
                    0
                };
                let reserve = render::chars(&counts) + 20 + close;
                if fence {
                    out.line(render::DATA_OPEN);
                }
                // Stored text is flattened to one line: it cannot forge a
                // header or an entry line.
                let state = if t.state.trim().is_empty() {
                    "State: none yet. Set one with log(..., state=...).".to_owned()
                } else {
                    let when = t
                        .state_updated_at
                        .map(render::short_date)
                        .unwrap_or_default();
                    let who = t.state_updated_by.map(render::author_word).unwrap_or("");
                    format!("State ({when}, {who}): {}", render::one_line(&t.state))
                };
                let room = out.left().saturating_sub(reserve + 3 * 60);
                out.line(render::defuse(&render::clip(&state, room.max(80))));
                for e in &page.items {
                    let room = out.left().saturating_sub(reserve);
                    if room < 40 {
                        break;
                    }
                    out.line(entry_line(e, room.min(130)));
                }
                if fence {
                    out.line(render::DATA_CLOSE);
                }
                out.line_clipped(format!(
                    "{} · {counts}",
                    plural(page.total, "entry", "entries")
                ));
            }
            Owner::Epic(id) => {
                let e = self.store.epic(id).map_err(err)?;
                let s = self
                    .store
                    .epics(e.project)
                    .map_err(err)?
                    .into_iter()
                    .find(|s| s.id == id);
                let progress = s.map_or(String::new(), |s| {
                    format!(" · {}/{} done", s.progress.done, s.progress.total)
                });
                out.line_clipped(format!(
                    "{id} {}{progress}{}",
                    render::clip(&render::one_line(&e.title), 80),
                    self.focus_mark(o)
                ));
                if !e.state.trim().is_empty() {
                    let state =
                        format!("State: {}", render::clip(&render::one_line(&e.state), 400));
                    if fence {
                        out.line(render::fenced(&state));
                    } else {
                        out.line(render::defuse(&state));
                    }
                }
                let counts = self.counts_line(o, e.project, None);
                let topics = self
                    .store
                    .topics(
                        &TopicFilter {
                            epic: Some(Some(id)),
                            ..TopicFilter::default()
                        },
                        PageReq::first(50),
                    )
                    .map_err(err)?;
                let lines: Vec<String> = topics
                    .items
                    .iter()
                    .map(|t| {
                        format!(
                            "{} {}  {}",
                            t.id,
                            t.status.as_str(),
                            render::clip(&render::one_line(&t.title), 80)
                        )
                    })
                    .collect();
                let more_room = 40 + render::chars(&counts);
                let n = render::fitting(&lines, out.left().saturating_sub(more_room));
                for l in &lines[..n] {
                    out.line(l);
                }
                let rest = topics.total - n as i64;
                if rest > 0 {
                    out.line(format!("+{rest} more topics: find(epic=\"{id}\")"));
                }
                out.line_clipped(counts);
            }
            Owner::Project(id) => {
                let p = self
                    .store
                    .projects()
                    .map_err(err)?
                    .into_iter()
                    .find(|p| p.id == id)
                    .ok_or_else(|| err(Error::NotFound(id.to_string())))?;
                out.line_clipped(format!(
                    "{id} {} · {}/{} topics done · inbox {} · knowledge {}",
                    render::clip(&render::one_line(&p.title), 60),
                    p.progress.done,
                    p.progress.total,
                    p.inbox,
                    p.knowledge
                ));
                let footer = "No focus topic: brief(id=\"T-n\", focus=true) sets one.";
                let epics = self.store.epics(id).map_err(err)?;
                let open = self
                    .store
                    .topics(
                        &TopicFilter {
                            project: Some(id),
                            epic: Some(None),
                            statuses: Status::ALL.into_iter().filter(|s| s.is_open()).collect(),
                        },
                        PageReq::first(20),
                    )
                    .map_err(err)?;
                let mut lines: Vec<String> = epics
                    .iter()
                    .map(|e| {
                        format!(
                            "{} {} ({}/{} done)",
                            e.id,
                            render::clip(&render::one_line(&e.title), 60),
                            e.progress.done,
                            e.progress.total
                        )
                    })
                    .collect();
                lines.extend(open.items.iter().map(|t| {
                    format!(
                        "{} {}  {}",
                        t.id,
                        t.status.as_str(),
                        render::clip(&render::one_line(&t.title), 60)
                    )
                }));
                let total = epics.len() as i64 + open.total;
                let n = render::fitting(&lines, out.left().saturating_sub(120));
                for l in &lines[..n] {
                    out.line(l);
                }
                if total > n as i64 {
                    out.line(format!("+{} more: find()", total - n as i64));
                }
                if self.session.focus.is_none() {
                    out.line(footer);
                }
            }
        }
        Ok(out.finish())
    }

    // ---- entries ----

    pub fn entries(&mut self, a: EntriesArgs) -> Reply {
        self.project()?;
        if let Some(id) = a.id.as_deref() {
            let (topic, seq) =
                parse_entry_ref(id).ok_or("id must be an entry id such as T-42#14")?;
            self.owner_project(Owner::Topic(topic))?;
            let e = self.store.entry_by_seq(topic, seq).map_err(err)?;
            let head = format!(
                "{topic}#{seq} {} {} {}",
                render::short_date(e.stamp.created_at),
                e.kind.as_str(),
                render::author_word(e.author)
            );
            let room = budget::ENTRY_FULL - head.len() - 1 - render::FENCE_COST - 60;
            let total = render::chars(&e.text);
            let (body, cut) = if total > room {
                (
                    render::clip(&e.text, room),
                    format!("\n[cut: {} more chars not shown]", total - room),
                )
            } else {
                (e.text.clone(), String::new())
            };
            return Ok(render::finish(
                format!("{head}\n{}{cut}", render::fenced(&body)),
                budget::ENTRY_FULL,
            ));
        }
        let topic = self.topic_arg(a.topic.as_deref())?;
        let limit = a.limit.unwrap_or(5).clamp(1, 20);
        let max = if limit <= 5 {
            budget::ENTRIES
        } else {
            budget::ENTRY_FULL
        };
        let kinds = match a.kind.as_deref() {
            Some(k) => vec![entry_kind(k)?],
            None => Vec::new(),
        };
        let filter = EntryFilter {
            kinds,
            include_system: false,
            before_seq: a.before,
        };
        let page = self
            .store
            .entries(topic, &filter, PageReq::first(limit))
            .map_err(err)?;
        let all = self
            .store
            .entries(topic, &EntryFilter::default(), PageReq::first(1))
            .map_err(err)?
            .total;
        if page.items.is_empty() {
            return Ok(format!(
                "{topic}: no entries{}",
                if a.before.is_some() {
                    " before that one"
                } else {
                    ""
                }
            ));
        }
        let lines: Vec<String> = page.items.iter().map(|e| entry_line(e, 240)).collect();
        let n = render::fitting(&lines, max - 120 - render::FENCE_COST);
        let shown = &page.items[..n];
        let newest = shown.first().and_then(|e| e.seq).unwrap_or(0);
        let oldest = shown.last().and_then(|e| e.seq).unwrap_or(0);
        let more = page.total > n as i64;
        let mut out = Text::new(max);
        out.line(format!(
            "{topic} entries #{newest}-#{oldest} of {all}{}",
            if more {
                format!(" · older: before={oldest}")
            } else {
                String::new()
            }
        ));
        out.line(render::fenced(&lines[..n].join("\n")));
        Ok(out.finish())
    }

    // ---- log ----

    pub fn log(&mut self, a: LogArgs) -> Reply {
        self.project()?;
        let kind = entry_kind(&a.kind)?;
        check_len("text", &a.text, limit::LOG_TEXT)?;
        if let Some(s) = &a.state {
            check_len("state", s, limit::STATE)?;
        }
        let status = a.status.as_deref().map(status_of).transpose()?;
        let topic = self.topic_arg(a.topic.as_deref())?;
        let state_set = a.state.is_some();
        let e = self
            .store
            .log(
                topic,
                NewEntry {
                    kind,
                    text: a.text,
                    state: a.state,
                    status,
                },
                self.author(),
            )
            .map_err(err)?;
        let mut ack = format!("ok {topic}#{} {}", e.seq.unwrap_or(0), kind.as_str());
        if state_set {
            ack.push_str(" · state updated");
        }
        if let Some(s) = status {
            ack.push_str(&format!(" · status {}", s.as_str()));
        }
        Ok(render::finish(ack, budget::ACK))
    }

    // ---- update ----

    pub fn update(&mut self, a: UpdateArgs) -> Reply {
        self.project()?;
        let mut fields = Vec::new();
        if let Some(t) = &a.title {
            check_len("title", t, limit::TITLE)?;
            fields.push("title");
        }
        if let Some(d) = &a.description {
            check_len("description", d, limit::DESCRIPTION)?;
            fields.push("description");
        }
        if let Some(s) = &a.state {
            check_len("state", s, limit::STATE)?;
            fields.push("state");
        }
        let id = a.id.trim();
        let label = if let Some(t) =
            TopicId::parse(id).filter(|_| id.to_ascii_uppercase().starts_with("T-"))
        {
            self.owner_project(Owner::Topic(t))?;
            let status = a.status.as_deref().map(status_of).transpose()?;
            if status.is_some() {
                fields.push("status");
            }
            let epic = match a.epic.as_deref() {
                None => None,
                Some(e) if e.trim().eq_ignore_ascii_case("none") => Some(None),
                Some(e) => Some(Some(self.epic_arg(e)?)),
            };
            if epic.is_some() {
                fields.push("epic");
            }
            let patch = TopicPatch {
                title: a.title,
                description: a.description,
                state: a.state,
                status,
                epic,
            };
            self.store
                .update_topic(t, patch, self.author())
                .map_err(err)?;
            t.to_string()
        } else if let Some(e) =
            EpicId::parse(id).filter(|_| id.to_ascii_uppercase().starts_with("E-"))
        {
            self.owner_project(Owner::Epic(e))?;
            if a.status.is_some() || a.epic.is_some() {
                return Err("an epic has no status or epic; set those on its topics".into());
            }
            let patch = EpicPatch {
                title: a.title,
                description: a.description,
                state: a.state,
            };
            self.store
                .update_epic(e, patch, self.author())
                .map_err(err)?;
            e.to_string()
        } else if let Some(n) =
            NoteId::parse(id).filter(|_| id.to_ascii_uppercase().starts_with("N-"))
        {
            let note = self.store.note(n).map_err(err)?;
            self.check_project(note.project, id)?;
            let text = a
                .description
                .or(a.title)
                .ok_or("a note takes its new text in description")?;
            check_len("note", &text, limit::NOTE)?;
            self.store
                .update_note(n, &text, self.author())
                .map_err(err)?;
            fields = vec!["text"];
            n.to_string()
        } else {
            return Err("id must be T-n, E-n or N-n".into());
        };
        if fields.is_empty() {
            return Err("nothing to change: pass title, description, state, status or epic".into());
        }
        Ok(render::finish(
            format!("ok {label} updated: {}", fields.join(", ")),
            budget::ACK,
        ))
    }

    // ---- find ----

    pub fn find(&mut self, a: FindArgs) -> Reply {
        let project = self.project()?;
        let limit = a.limit.unwrap_or(8).clamp(1, 20);
        let offset = a.cursor.unwrap_or(0).max(0);
        let kind = a.kind.as_deref().map(|k| k.trim().to_ascii_lowercase());
        if kind
            .as_deref()
            .is_some_and(|k| !matches!(k, "topic" | "epic" | "note" | "entry"))
        {
            return Err("kind must be topic, epic, note or entry".into());
        }
        let statuses: Vec<Status> = match a.status.as_deref().map(|s| s.trim().to_ascii_lowercase())
        {
            None => Status::ALL.into_iter().filter(|s| s.is_open()).collect(),
            Some(s) if s == "open" => Status::ALL.into_iter().filter(|s| s.is_open()).collect(),
            Some(s) if s == "closed" => vec![Status::Done, Status::Dropped],
            Some(s) if s == "all" => Vec::new(),
            Some(s) => vec![status_of(&s)?],
        };
        let epic = a.epic.as_deref().map(|e| self.epic_arg(e)).transpose()?;
        let query = a.query.as_deref().map(str::trim).filter(|q| !q.is_empty());
        if let Some(q) = query {
            check_len("query", q, limit::QUERY)?;
        }
        let mut lines: Vec<String> = Vec::new();
        let page_total: i64;
        match (query, kind.as_deref()) {
            (None, Some("note")) => {
                let page = self
                    .store
                    .inbox(project, PageReq { offset, limit })
                    .map_err(err)?;
                lines.extend(page.items.iter().map(|n| {
                    format!(
                        "{} note  {}",
                        n.id,
                        render::clip(&render::one_line(&n.text), 200)
                    )
                }));
                page_total = page.total;
            }
            (None, Some("epic")) => {
                let all = self.store.epics(project).map_err(err)?;
                page_total = all.len() as i64;
                lines.extend(
                    all.iter()
                        .skip(offset as usize)
                        .take(limit as usize)
                        .map(|e| {
                            format!(
                                "{} epic  {} ({}/{} done)",
                                e.id,
                                render::clip(&render::one_line(&e.title), 80),
                                e.progress.done,
                                e.progress.total
                            )
                        }),
                );
            }
            (None, Some("entry")) => {
                return Err(
                    "entries are found by query; for a timeline call entries(topic)".into(),
                );
            }
            (None, None | Some("topic")) => {
                let page = self
                    .store
                    .topics(
                        &TopicFilter {
                            project: Some(project),
                            epic: epic.map(Some),
                            statuses,
                        },
                        PageReq { offset, limit },
                    )
                    .map_err(err)?;
                lines.extend(page.items.iter().map(|t| {
                    let epic = t.epic.map_or(String::new(), |e| format!(" · {e}"));
                    format!(
                        "{} {}  {}{epic}",
                        t.id,
                        t.status.as_str(),
                        render::clip(&render::one_line(&t.title), 80)
                    )
                }));
                page_total = page.total;
            }
            (None, Some(_)) => {
                return Err("kind must be topic, epic, note or entry".into());
            }
            (Some(q), k) => {
                // One list: matching epic titles first, then the full-text
                // hits; the cursor is an offset into that list, so the
                // search offset is the cursor minus the epic count.
                let mut epic_hits = Vec::new();
                if matches!(k, None | Some("epic")) && epic.is_none() {
                    let qw = words(q);
                    for e in self.store.epics(project).map_err(err)? {
                        let ew = words(&e.title);
                        if !qw.is_empty()
                            && qw
                                .iter()
                                .all(|w| ew.iter().any(|x| x.starts_with(w.as_str())))
                        {
                            epic_hits.push(format!(
                                "{} epic  {} ({}/{} done)",
                                e.id,
                                render::clip(&render::one_line(&e.title), 80),
                                e.progress.done,
                                e.progress.total
                            ));
                        }
                    }
                }
                let kinds = match k {
                    None => vec![SearchKind::Topic, SearchKind::Entry, SearchKind::Note],
                    Some("topic") => vec![SearchKind::Topic],
                    Some("entry") => vec![SearchKind::Entry],
                    Some("note") => vec![SearchKind::Note],
                    _ => Vec::new(),
                };
                let epic_count = epic_hits.len() as i64;
                let mut total = epic_count;
                lines.extend(
                    epic_hits
                        .into_iter()
                        .skip(offset as usize)
                        .take(limit as usize),
                );
                let search_offset = (offset - epic_count).max(0);
                let search_limit = limit - lines.len() as i64;
                if !kinds.is_empty() {
                    // With a query, the status filter applies only when given.
                    let statuses = if a.status.is_some() {
                        statuses
                    } else {
                        Vec::new()
                    };
                    let page = self
                        .store
                        .search(
                            &SearchQuery {
                                text: q.to_owned(),
                                project: Some(project),
                                kinds,
                                statuses,
                                epic,
                            },
                            PageReq {
                                offset: search_offset,
                                limit: search_limit.max(1),
                            },
                        )
                        .map_err(err)?;
                    total += page.total;
                    let items = if search_limit > 0 {
                        &page.items[..]
                    } else {
                        &[]
                    };
                    for h in items {
                        let snip = render::clip(&render::one_line(&strip_marks(&h.snippet)), 150);
                        let line = match h.record {
                            Record::Topic(t) => {
                                let Ok(topic) = self.store.topic(t) else {
                                    continue;
                                };
                                format!(
                                    "{t} {}  {} - {snip}",
                                    topic.status.as_str(),
                                    render::clip(&render::one_line(&topic.title), 60)
                                )
                            }
                            Record::Entry(id) => {
                                let Ok(e) = self.store.entry(id) else {
                                    continue;
                                };
                                format!(
                                    "{}#{} {}  {snip}",
                                    e.topic,
                                    e.seq.unwrap_or(0),
                                    e.kind.as_str()
                                )
                            }
                            Record::Note(n) => format!("{n} note  {snip}"),
                            _ => continue,
                        };
                        lines.push(line);
                    }
                }
                page_total = total;
            }
        }
        if lines.is_empty() {
            return Ok("0 hits".into());
        }
        let n = render::fitting(&lines, budget::FIND - 60);
        let mut out = Text::new(budget::FIND);
        for l in &lines[..n] {
            out.line(l);
        }
        let shown_end = offset + n as i64;
        if shown_end < page_total {
            out.line(format!(
                "{}-{shown_end} of {page_total} · more: cursor={shown_end}",
                offset + 1
            ));
        } else {
            out.line(plural(page_total, "hit", "hits"));
        }
        Ok(out.finish())
    }

    // ---- create ----

    pub fn create(&mut self, a: CreateArgs) -> Reply {
        let project = self.project()?;
        if a.items.is_empty() {
            return Err("items is empty".into());
        }
        if a.items.len() > limit::CREATE_ITEMS {
            return Err(format!(
                "{} items; the limit is {} per call",
                a.items.len(),
                limit::CREATE_ITEMS
            ));
        }
        // Validate everything first, so a bad item writes nothing.
        enum Plan {
            Topic(NewTopic, Vec<NoteId>),
            Epic(String, String),
            Note(String),
        }
        let mut plans = Vec::new();
        for (i, it) in a.items.iter().enumerate() {
            let n = i + 1;
            let desc = it.description.clone().unwrap_or_default();
            match it.kind.trim().to_ascii_lowercase().as_str() {
                "topic" => {
                    check_len(&format!("item {n} title"), &it.title, limit::TITLE)?;
                    check_len(&format!("item {n} description"), &desc, limit::DESCRIPTION)?;
                    let epic = it.epic.as_deref().map(|e| self.epic_arg(e)).transpose()?;
                    let mut notes = Vec::new();
                    for id in it.from_notes.iter().flatten() {
                        let nid =
                            NoteId::parse(id).ok_or(format!("item {n}: {id} is not a note id"))?;
                        let note = self.store.note(nid).map_err(err)?;
                        self.check_project(note.project, id)?;
                        notes.push(nid);
                    }
                    plans.push(Plan::Topic(
                        NewTopic {
                            project,
                            epic,
                            title: it.title.clone(),
                            description: desc,
                            status: Status::Planned,
                        },
                        notes,
                    ));
                }
                "epic" => {
                    check_len(&format!("item {n} title"), &it.title, limit::TITLE)?;
                    check_len(&format!("item {n} description"), &desc, limit::DESCRIPTION)?;
                    plans.push(Plan::Epic(it.title.clone(), desc));
                }
                "note" => {
                    let text = if desc.trim().is_empty() {
                        it.title.clone()
                    } else {
                        format!("{}\n\n{}", it.title.trim(), desc.trim())
                    };
                    check_len(&format!("item {n} note"), &text, limit::NOTE)?;
                    plans.push(Plan::Note(text));
                }
                _ => return Err(format!("item {n}: kind must be topic, epic or note")),
            }
        }
        let author = self.author();
        let mut acks = Vec::new();
        for plan in plans {
            match plan {
                Plan::Topic(new, notes) => {
                    let id = self.store.create_topic(new, author).map_err(err)?;
                    if notes.is_empty() {
                        acks.push(id.to_string());
                    } else {
                        self.store.file_notes(&notes, id, author).map_err(err)?;
                        let list: Vec<String> = notes.iter().map(|n| n.to_string()).collect();
                        acks.push(format!("{id} (notes {} filed)", list.join(" ")));
                    }
                }
                Plan::Epic(title, desc) => {
                    let id = self
                        .store
                        .create_epic(project, &title, &desc, author)
                        .map_err(err)?;
                    acks.push(id.to_string());
                }
                Plan::Note(text) => {
                    let id = self.store.add_note(project, &text, author).map_err(err)?;
                    acks.push(id.to_string());
                }
            }
        }
        Ok(render::finish(
            format!("ok {}", acks.join(" · ")),
            budget::CREATE,
        ))
    }

    // ---- kb ----

    pub fn kb(&mut self, a: KbArgs) -> Reply {
        self.project()?;
        let scope = self.scope(a.id.as_deref())?;
        let kind = a.kind.as_deref().map(kb_kind).transpose()?;
        let offset = a.cursor.unwrap_or(0).max(0);
        if let Some(q) = a.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            check_len("query", q, limit::QUERY)?;
            let page = self
                .store
                .kb_search(scope, q, kind, PageReq { offset, limit: 5 })
                .map_err(err)?;
            if page.items.is_empty() {
                return Ok(format!("No knowledge in scope of {scope} matches."));
            }
            let mut out = Text::new(budget::KB);
            for h in &page.items {
                let Record::Kb(id) = h.record else { continue };
                let section = self
                    .store
                    .kb_entry(id)
                    .ok()
                    .and_then(|e| section_of_match(&e.body, &h.snippet))
                    .map_or(String::new(), |s| format!(" §{s}"));
                let excerpt = render::clip(&render::one_line(&strip_marks(&h.snippet)), 160);
                out.line_clipped(format!(
                    "{id}{section} {}: {excerpt}",
                    render::clip(&render::one_line(&h.title), 60)
                ));
            }
            let end = offset + page.items.len() as i64;
            if end < page.total {
                out.line(format!("{} more: cursor={end}", page.total - end));
            }
            return Ok(out.finish());
        }
        let counts = self.store.kb_counts(scope).map_err(err)?;
        let page = self
            .store
            .kb_scope(
                scope,
                &KbFilter {
                    kind,
                    ..KbFilter::default()
                },
                PageReq { offset, limit: 10 },
            )
            .map_err(err)?;
        let lines: Vec<String> = page.items.iter().map(kb_line).collect();
        let n = render::fitting(&lines, budget::KB - 130);
        let end = offset + n as i64;
        let mut head = format!(
            "Knowledge in scope of {scope}: {}, {}",
            plural(counts.documents, "document", "documents"),
            plural(counts.memories, "memory", "memories")
        );
        if page.total == 0 {
            head.push_str(". Add with kb_write.");
        } else {
            head.push_str(&format!(" · {}-{end} of {}", offset + 1, page.total));
            if end < page.total {
                head.push_str(&format!(" · more: cursor={end}"));
            }
        }
        let mut out = Text::new(budget::KB);
        out.line(head);
        for l in &lines[..n] {
            out.line(l);
        }
        Ok(out.finish())
    }

    fn kb_owned(&self, id: &str) -> Result<KbId, String> {
        let k = KbId::parse(id)
            .filter(|_| id.trim().to_ascii_uppercase().starts_with("K-"))
            .ok_or("id must be a knowledge id such as K-2")?;
        let e = self.store.kb_entry(k).map_err(err)?;
        self.check_project(e.project, id)?;
        Ok(k)
    }

    // ---- kb_read ----

    pub fn kb_read(&mut self, a: KbReadArgs) -> Reply {
        self.project()?;
        let id = self.kb_owned(&a.id)?;
        let e = self.store.kb_entry(id).map_err(err)?;
        let size = render::chars(&e.body);
        let head = format!(
            "{id} {} · {} · rev {} · {}",
            render::clip(&render::one_line(&e.title), 80),
            kind_word(e.kind),
            e.rev,
            render::token_label(size)
        );
        let max = budget::KB_READ;
        // Lines are counted after wrapping long ones, so a body made of one
        // huge line is still readable part by part.
        let all = display_lines(&e.body);
        if let Some(n) = a.section.as_deref() {
            let n = n.trim();
            let s = markdown::outline(&e.body)
                .into_iter()
                .find(|s| s.number == n)
                .ok_or_else(|| {
                    format!(
                        "{id} rev {} has no section {n}; outline:\n{}",
                        e.rev,
                        outline_text(&e.body, 1_200)
                    )
                })?;
            let physical: Vec<&str> = e.body.lines().collect();
            let width = |l: &&str| l.chars().count().div_ceil(WRAP).max(1);
            let first = physical[..s.start].iter().map(width).sum::<usize>() + 1;
            let count: usize = physical[s.start..s.end].iter().map(width).sum();
            let part = &all[first - 1..(first - 1 + count).min(all.len())];
            return Ok(cut_body(
                &head,
                &format!("section {n}"),
                part,
                first,
                all.len(),
                max,
            ));
        }
        if let Some(range) = a.lines.as_deref() {
            let (from, to) = range
                .split_once('-')
                .and_then(|(a, b)| {
                    Some((
                        a.trim().parse::<usize>().ok()?,
                        b.trim().parse::<usize>().ok()?,
                    ))
                })
                .filter(|(a, b)| *a >= 1 && a <= b)
                .ok_or("lines must look like 40-80")?;
            if from > all.len() {
                return Err(format!("{id} has {} lines", all.len()));
            }
            let to = to.min(all.len());
            return Ok(cut_body(
                &head,
                &format!("lines {from}-{to} of {}", all.len()),
                &all[from - 1..to],
                from,
                all.len(),
                max,
            ));
        }
        if size + render::chars(&head) + render::FENCE_COST < max {
            return Ok(render::finish(
                format!("{head}\n{}", render::fenced(e.body.trim_end())),
                max,
            ));
        }
        let outline = markdown::outline(&e.body);
        if outline.is_empty() {
            return Ok(format!(
                "{head}, over the {} budget; no headings ({} lines).\nRead a part with lines=\"1-80\".",
                render::token_label(max),
                all.len()
            ));
        }
        Ok(render::finish(
            format!(
                "{head}, over the {} budget; outline:\n{}\nRead one with section=<n>.",
                render::token_label(max),
                outline_text(&e.body, max - 300)
            ),
            max,
        ))
    }

    // ---- kb_write ----

    pub fn kb_write(&mut self, a: KbWriteArgs) -> Reply {
        self.project()?;
        if let Some(s) = &a.summary {
            check_len("summary", s, limit::SUMMARY)?;
        }
        if let Some(t) = &a.title {
            check_len("title", t, limit::TITLE)?;
        }
        match a.id.clone() {
            None => self.kb_create(a),
            Some(id) => self.kb_edit(&id, a),
        }
    }

    /// The owner a write targets: `owner`, else `level` relative to the focus.
    fn write_owner(&self, owner: Option<&str>, level: Option<&str>) -> Result<Owner, String> {
        if let Some(o) = owner {
            return self.scope(Some(o));
        }
        let level = match level.map(|l| l.trim().to_ascii_lowercase()) {
            None => Level::Topic,
            Some(l) => Level::parse(&l).ok_or("level must be topic, epic or project")?,
        };
        let project = self.project()?;
        Ok(match (level, self.session.focus) {
            (Level::Project, _) => Owner::Project(project),
            (Level::Topic, Some(Owner::Topic(t))) => Owner::Topic(t),
            (Level::Topic, _) => {
                return Err(
                    "no focus topic: pass owner=\"T-n\", or level=\"epic\" or \"project\"".into(),
                );
            }
            (Level::Epic, Some(Owner::Epic(e))) => Owner::Epic(e),
            (Level::Epic, Some(Owner::Topic(t))) => match self.store.topic(t).map_err(err)?.epic {
                Some(e) => Owner::Epic(e),
                None => {
                    return Err(format!(
                        "{t} has no epic: pass owner=\"E-n\" or level=\"project\""
                    ));
                }
            },
            (Level::Epic, _) => return Err("no focus epic: pass owner=\"E-n\"".into()),
        })
    }

    fn kb_create(&mut self, a: KbWriteArgs) -> Reply {
        let kind = kb_kind(
            a.kind
                .as_deref()
                .ok_or("a new entry needs kind (document or memory)")?,
        )?;
        let title = a
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .ok_or("a new entry needs a title")?;
        let summary = a
            .summary
            .clone()
            .filter(|s| !s.trim().is_empty())
            .ok_or("a new entry needs a summary: one sentence saying what it answers")?;
        let max = match kind {
            KbKind::Memory => limit::MEMORY_BODY,
            KbKind::Document => limit::DOCUMENT_WRITE,
        };
        check_len(
            if kind == KbKind::Memory {
                "memory body"
            } else {
                "text"
            },
            &a.text,
            max,
        )?;
        let owner = self.write_owner(a.owner.as_deref(), a.level.as_deref())?;
        if kind == KbKind::Memory {
            let refusal_key = format!("{owner}|{}", words(&title).join(" "));
            let page = self
                .store
                .kb_scope(
                    owner,
                    &KbFilter {
                        kind: Some(KbKind::Memory),
                        ..KbFilter::default()
                    },
                    PageReq::first(PageReq::MAX_LIMIT),
                )
                .map_err(err)?;
            let previous = self.store.local_value(REFUSED_KEY).map_err(err)?;
            if previous.as_deref() == Some(refusal_key.as_str()) {
                self.store.set_local_value(REFUSED_KEY, None).map_err(err)?;
            } else if let Some(dup) = page
                .items
                .iter()
                .find(|m| near_duplicate(&title, &summary, m))
            {
                self.store
                    .set_local_value(REFUSED_KEY, Some(&refusal_key))
                    .map_err(err)?;
                return Err(render::finish(
                    format!(
                        "not saved: {} \"{}\" (rev {}) looks like the same memory. Update it with kb_write(id=\"{}\", rev={}, op=\"replace_body\", ...), or repeat this call to save a new one anyway.",
                        dup.id,
                        render::clip(&dup.title, 60),
                        dup.rev,
                        dup.id,
                        dup.rev
                    ),
                    600,
                ));
            }
        }
        let id = self
            .store
            .kb_create(
                NewKb {
                    owner,
                    kind,
                    title,
                    summary,
                    body: a.text,
                    from_file: false,
                },
                self.author(),
            )
            .map_err(err)?;
        Ok(render::finish(
            format!(
                "ok {id} {} · {} {owner} · rev 1",
                kind.as_str(),
                level_word(owner)
            ),
            budget::ACK,
        ))
    }

    fn kb_edit(&mut self, id_text: &str, a: KbWriteArgs) -> Reply {
        let id = self.kb_owned(id_text)?;
        let cur = self.store.kb_entry(id).map_err(err)?;
        let op = a.op.as_deref().map(|o| o.trim().to_ascii_lowercase());
        let edit = match op.as_deref() {
            None => {
                if !a.text.is_empty() {
                    return Err(
                        "text needs an op: replace_body, replace_section or append_section".into(),
                    );
                }
                KbEdit::Keep
            }
            Some("replace_body") => KbEdit::Body(a.text.clone()),
            Some("replace_section") => KbEdit::ReplaceSection {
                number: a
                    .section
                    .clone()
                    .ok_or("replace_section needs section=<n>")?
                    .trim()
                    .to_owned(),
                heading: a.heading.clone(),
                text: a.text.clone(),
            },
            Some("append_section") => KbEdit::AppendSection {
                heading: a.heading.clone().ok_or("append_section needs a heading")?,
                text: a.text.clone(),
            },
            Some(_) => {
                return Err("op must be replace_body, replace_section or append_section".into());
            }
        };
        if !matches!(edit, KbEdit::Keep) && a.rev.is_none() {
            return Err(format!(
                "pass rev: the revision you read (current is {}). Read it first if you have not.",
                cur.rev
            ));
        }
        // Size of the body after the edit, for the limits.
        let new_body = match &edit {
            KbEdit::Keep => cur.body.clone(),
            KbEdit::Body(b) => b.clone(),
            KbEdit::ReplaceSection {
                number,
                heading,
                text,
            } => markdown::replace_section(&cur.body, number, heading.as_deref(), text)
                .map(|(b, _)| b)
                .ok_or_else(|| {
                    format!(
                        "{id} rev {} has no section {number}; outline:\n{}",
                        cur.rev,
                        outline_text(&cur.body, 1_200)
                    )
                })?,
            KbEdit::AppendSection { heading, text } => {
                markdown::append_section(&cur.body, heading, text)
            }
        };
        if cur.kind == KbKind::Memory {
            check_len("memory body", &new_body, limit::MEMORY_BODY)?;
        } else {
            check_len("text", &a.text, limit::DOCUMENT_WRITE)?;
        }
        let what = match &edit {
            KbEdit::Keep => "title and summary".to_owned(),
            KbEdit::Body(_) => "body replaced".to_owned(),
            KbEdit::ReplaceSection { number, .. } => format!(
                "section {number} replaced ({} -> {})",
                render::token_label(
                    markdown::section_text(&cur.body, number).map_or(0, |s| s.len())
                ),
                render::token_label(
                    markdown::section_text(&new_body, number).map_or(0, |s| s.len())
                )
            ),
            KbEdit::AppendSection { heading, .. } => {
                format!("section \"{}\" appended", render::clip(heading, 40))
            }
        };
        // A move target is validated before anything is written, so a bad
        // level or owner refuses the whole call.
        let target = if a.level.is_some() || a.owner.is_some() {
            Some(self.write_owner(a.owner.as_deref(), a.level.as_deref())?)
        } else {
            None
        };
        let author = self.author();
        let mut rev = match self.store.kb_update(
            id,
            KbUpdate {
                expected_rev: a.rev,
                title: a.title.clone(),
                summary: a.summary.clone(),
                edit,
            },
            author,
        ) {
            Ok(r) => r,
            Err(Error::StaleRevision { current }) => {
                let fresh = self.store.kb_entry(id).map_err(err)?;
                return Err(render::finish(
                    format!(
                        "not saved: {id} changed since rev {}; it is rev {current} now. Outline:\n{}\nRe-read what you change, then retry with rev={current}.",
                        a.rev.unwrap_or(0),
                        outline_text(&fresh.body, 1_200)
                    ),
                    1_600,
                ));
            }
            Err(e) => return Err(err(e)),
        };
        let mut moved = String::new();
        if let Some(owner) = target
            && owner != cur.owner
        {
            rev = self.store.kb_move(id, owner, author).map_err(err)?;
            moved = format!(" · moved to {owner}");
        }
        Ok(render::finish(
            format!("ok {id} rev {rev} · {what}{moved}"),
            budget::ACK,
        ))
    }

    // ---- hooks ----

    /// The `SessionStart` briefing (ADR 0005, hooks): the brief, the counts,
    /// and up to 5 memory titles, within 1,200 characters. `None` when the
    /// session is bound to nothing.
    pub fn session_briefing(&self, after_compaction: bool) -> Option<String> {
        let project = self.session.project?;
        let scope = self.session.focus.unwrap_or(Owner::Project(project));
        let max = budget::SESSION_BRIEFING;
        let mut tail = Vec::new();
        if !self.session.candidates.is_empty() {
            let list: Vec<String> = self
                .session
                .candidates
                .iter()
                .take(6)
                .map(|t| t.to_string())
                .collect();
            tail.push(format!(
                "This folder is attached to several topics ({}); set the focus with brief(id, focus=true).",
                list.join(", ")
            ));
        }
        if after_compaction {
            tail.push(
                "Context was compacted: log decisions and progress not yet logged, then continue."
                    .to_owned(),
            );
        }
        let tail_len: usize = tail.iter().map(|t| render::chars(t) + 1).sum();
        let head = "Acta briefing (project tracker and memory; see the acta:using-acta skill).";
        let brief = self
            .render_brief(
                scope,
                max - tail_len - head.len() - render::FENCE_COST - 120,
                false,
            )
            .ok()?;
        let mut out = Text::new(max);
        out.line(head);
        // The whole body is stored text: one fence around it.
        out.line(render::DATA_OPEN);
        for l in brief.lines() {
            out.line(render::defuse(l));
        }
        let tail_len = tail_len + render::DATA_CLOSE.len() + 1;
        // Memory titles: topic, then epic, then project; newest first.
        let memories = self
            .store
            .kb_scope(
                scope,
                &KbFilter {
                    kind: Some(KbKind::Memory),
                    ..KbFilter::default()
                },
                PageReq::first(5),
            )
            .map(|p| p.items)
            .unwrap_or_default();
        let room = out.left().saturating_sub(tail_len);
        let mut titles = String::new();
        for m in &memories {
            let item = format!("{} {}", m.id, render::clip(&render::one_line(&m.title), 60));
            let sep = if titles.is_empty() {
                "Recent memories: "
            } else {
                " · "
            };
            if render::chars(&titles) + sep.len() + render::chars(&item) > room {
                break;
            }
            titles.push_str(sep);
            titles.push_str(&item);
        }
        if !titles.is_empty() {
            out.line(titles);
        }
        out.line(render::DATA_CLOSE);
        for t in tail {
            out.line_clipped(t);
        }
        Some(out.finish())
    }

    /// The `SubagentStart` note: at most 300 characters.
    pub fn subagent_note(&self) -> Option<String> {
        let project = self.session.project?;
        let p = self.store.project(project).ok()?;
        let mut parts = vec![format!(
            "Acta: project {project} {}",
            render::clip(&render::one_line(&p.title), 40)
        )];
        match self.session.focus {
            Some(Owner::Topic(t)) => {
                if let Some(e) = self.store.topic(t).ok().and_then(|t| t.epic) {
                    parts.push(format!("epic {e}"));
                }
                parts.push(format!("parent focus {t}"));
            }
            Some(Owner::Epic(e)) => parts.push(format!("parent focus {e}")),
            _ => {}
        }
        let s = format!(
            "{}. Pass ids explicitly to Acta tools (topic=\"T-n\"); log findings with log; check kb before unfamiliar areas.",
            parts.join(", ")
        );
        Some(render::finish(s, budget::SUBAGENT))
    }

    /// The `hook` tool (MCP only, for `mcp_tool` hooks): hook JSON.
    pub fn hook(&mut self, a: HookArgs) -> Reply {
        if let Some(s) = a.session.as_deref() {
            let _ = self.session.adopt_session_id(self.store, s);
        }
        let event = a.event.trim().to_ascii_lowercase();
        let (name, text) = match event.as_str() {
            "clear" | "compact" | "startup" | "resume" | "sessionstart" => {
                ("SessionStart", self.session_briefing(event == "compact"))
            }
            "subagent" | "subagentstart" => ("SubagentStart", self.subagent_note()),
            _ => return Err("event must be clear, compact or subagent".into()),
        };
        Ok(match text {
            Some(t) => hook_json(name, &t),
            None => "{}".to_owned(),
        })
    }
}

/// `{"hookSpecificOutput":{"hookEventName":..,"additionalContext":..}}`.
pub fn hook_json(event: &str, context: &str) -> String {
    serde_json::json!({
        "hookSpecificOutput": {"hookEventName": event, "additionalContext": context}
    })
    .to_string()
}

/// What `PreCompact` prints (ADR 0005): fixed text, no database.
pub const PRE_COMPACT: &str = "Acta: in the summary, keep the Acta focus id (T-n or E-n) and list the decisions and progress not yet logged to Acta, so they can be logged with `log` after compaction.";

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn entry_line(e: &Entry, max: usize) -> String {
    let seq = e.seq.unwrap_or(0);
    let head = format!(
        "#{seq} {} {} {}  ",
        render::short_date(e.stamp.created_at),
        e.kind.as_str(),
        render::author_word(e.author)
    );
    let room = max.saturating_sub(head.len());
    format!(
        "{head}{}",
        render::defuse(&render::clip_with_pointer(
            &e.text,
            room,
            &format!("{}#{seq}", e.topic)
        ))
    )
}

fn kb_line(k: &KbSummary) -> String {
    let size = match k.kind {
        KbKind::Document => format!(" · {}", render::token_label(k.size as usize)),
        KbKind::Memory => String::new(),
    };
    format!(
        "{:<5} {} {:<7} {}{size} · {} - {}",
        k.id.to_string(),
        kind_word(k.kind),
        level_word(k.owner),
        render::clip(&render::one_line(&k.title), 60),
        render::short_date(k.updated_at),
        render::clip(&render::one_line(&k.summary), 120)
    )
}

/// `1 Overview 210 · 2 Crates 640 · 2.1 acta-core 300` (sizes in tokens).
fn outline_text(body: &str, max: usize) -> String {
    let items: Vec<String> = markdown::outline(body)
        .iter()
        .map(|s| {
            format!(
                "{} {} {}",
                s.number,
                render::clip(&s.title, 50),
                render::tokens(s.bytes)
            )
        })
        .collect();
    if items.is_empty() {
        return format!(
            "(no headings; {} lines: read with lines=\"a-b\")",
            body.lines().count()
        );
    }
    let mut out = String::new();
    for (i, it) in items.iter().enumerate() {
        let sep = if out.is_empty() { "" } else { " · " };
        if render::chars(&out) + sep.len() + render::chars(it) + 12 > max {
            out.push_str(&format!(" · +{} more", items.len() - i));
            break;
        }
        out.push_str(sep);
        out.push_str(it);
    }
    out
}

/// Longest line handed out whole; longer ones are split, so `lines=` can
/// reach every part of any body.
const WRAP: usize = 400;

/// The lines `kb_read` counts: physical lines, each longer than [`WRAP`]
/// characters split into chunks of that length.
fn display_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            out.push(String::new());
        }
        for chunk in chars.chunks(WRAP) {
            out.push(chunk.iter().collect());
        }
    }
    out
}

/// A run of display lines (numbered from `first` in a body of `total`)
/// under the budget, fenced, cut at a line with a pointer to the rest.
fn cut_body(
    head: &str,
    what: &str,
    part: &[String],
    first: usize,
    total: usize,
    max: usize,
) -> String {
    let head = format!("{head} · {what}");
    let room = max.saturating_sub(render::chars(&head) + render::FENCE_COST + 90);
    let mut shown = Vec::new();
    let mut used = 0;
    for l in part {
        let add = render::chars(l) + 1;
        if used + add > room {
            break;
        }
        used += add;
        shown.push(l.as_str());
    }
    let body = render::fenced(shown.join("\n").trim_end());
    if shown.len() == part.len() {
        return render::finish(format!("{head}\n{body}"), max);
    }
    let next = first + shown.len();
    let last = first + part.len() - 1;
    render::finish(
        format!(
            "{head}\n{body}\n[cut at line {} of {total}: read on with lines=\"{next}-{last}\"]",
            next - 1
        ),
        max,
    )
}

/// Outline number of the section holding the first highlighted term of a
/// search excerpt.
fn section_of_match(body: &str, snippet: &str) -> Option<String> {
    let start = snippet.find(SNIPPET_OPEN)? + SNIPPET_OPEN.len_utf8();
    let end = snippet[start..].find(SNIPPET_CLOSE)? + start;
    let term = snippet[start..end].to_lowercase();
    if term.is_empty() {
        return None;
    }
    let line = body
        .lines()
        .position(|l| l.to_lowercase().contains(&term))?;
    markdown::outline(body)
        .into_iter()
        .rfind(|s| s.start <= line && line < s.end)
        .map(|s| s.number)
}
