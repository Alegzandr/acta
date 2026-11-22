//! The nine agent tools of ADR 0005 and the hook briefings, as functions of
//! the domain layer. The MCP server and the CLI call these and print what
//! they return; neither holds any logic of its own.
//!
//! Every answer is plain text under the tool's budget ([`render`]). A refusal
//! is an `Err` with a one-line reason the agent can act on.

use super::args::*;
use super::due;
use super::questions as q;
use super::render::{self, Fence, Text, budget, limit};
use super::session::{Session, parse_owner};
use crate::date::{Date, DueState, due_state};
use crate::error::Error;
use crate::markdown;
use crate::model::{
    AttachOwner, Author, ContactId, ContactLevel, Entry, EntryKind, EpicId, EpicSummary, KbId,
    KbKind, KbSummary, Level, Note, NoteFate, NoteId, Owner, PageReq, Place, ProjectId, QuestionId,
    QuestionStatus, Record, Status, TopicId, TopicSummary,
};
use crate::store::{
    ANSWER_MAX, EntryFilter, EpicPatch, INBOX_DAYS, KbEdit, KbFilter, KbUpdate, NewEntry, NewKb,
    NewQuestion, NewTopic, OverviewFilter, OverviewItem, QUESTION_MAX, SNIPPET_CLOSE, SNIPPET_OPEN,
    Scope, SearchKind, SearchQuery, Store, TopicFilter, TopicPatch,
};

/// The server `instructions` (ADR 0005, section 2). Under 800 characters.
pub const INSTRUCTIONS: &str = "Acta is this project's tracker and your working memory: topics (T-n) in epics (E-n), each with a current state, a timeline and a knowledge base (K-n: documents and memories). The session briefing names your focus topic. Call `brief` when unsure where things stand. After a meaningful step or a decision call `log`, and refresh the state in the same call. Save durable facts with `kb_write` instead of your own memory files; check `kb` before working on an unfamiliar area. Record what only someone else can answer with `create` (kind question), and the answers with `update`. Tools default to the focus topic; pass an id to address another. Subagents always pass ids. Do not copy code or long output into Acta.";

pub const NOT_LINKED: &str = "This folder is not linked to an Acta project, so Acta is off here. The user can link it with `acta link`.";

/// What a tool returns: the text for the agent, or a refusal.
pub type Reply = Result<String, String>;

pub(super) fn err(e: Error) -> String {
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

/// A contact level argument: `all`, `P-n` or `E-n`.
fn level_arg(store: &Store, text: &str) -> Result<ContactLevel, String> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("all") {
        return Ok(ContactLevel::All);
    }
    match parse_owner(t) {
        Some(Owner::Project(p)) => Ok(ContactLevel::Project(p)),
        Some(Owner::Epic(e)) => Ok(ContactLevel::Epic(store.epic(e).map_err(err)?.project, e)),
        _ => Err("level must be all, P-n or E-n".into()),
    }
}

/// A contact named in a `create` item: an existing one, or a contact item
/// of the same call (`@k`).
#[derive(Clone, Copy)]
pub(super) enum PersonRef {
    Existing(ContactId),
    Item(usize),
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

    pub(super) fn project(&self) -> Result<ProjectId, String> {
        self.session.project.ok_or_else(|| NOT_LINKED.to_owned())
    }

    /// An unbound session reaches nothing, not even by explicit id: the
    /// plugin runs in every repository, and a prompt there must not read or
    /// write the user's projects. A bound one never reaches another project.
    pub(super) fn check_project(&self, project: ProjectId, what: &str) -> Result<(), String> {
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
        let body = self.render_brief(target, budget::BRIEF - render::FENCE_COST)?;
        Ok(fenced_reply(None, &body, None, budget::BRIEF))
    }

    fn focus_mark(&self, o: Owner) -> &'static str {
        if self.session.focus == Some(o) {
            " · focus"
        } else {
            ""
        }
    }

    /// The `Attention:` line of a project or an epic (data.md request 17),
    /// built from the Overview's own lists so it always agrees with the UI:
    /// blocked, overdue (topics, and on a project also epics, as
    /// `find(due=overdue)`), idle, and on a project an inbox older than 7
    /// days. `None` when nothing needs attention.
    fn attention(&self, scope: Scope, max: usize) -> Result<Option<String>, String> {
        let today = self.session.today();
        let zone = self.session.zone;
        // One read of the scope for every list (the inbox is the project's).
        let mut filters = vec![
            OverviewFilter::Blocked,
            OverviewFilter::Overdue,
            OverviewFilter::Idle,
        ];
        if matches!(scope, Scope::Project(_)) {
            filters.push(OverviewFilter::Inbox);
        }
        let mut lists = self
            .store
            .overview_lists(scope, &filters, today, zone)
            .map_err(err)?
            .into_iter();
        let mut next = || lists.next().unwrap_or_default();
        let with_days = |items: Vec<OverviewItem>| -> Vec<(String, i64)> {
            items
                .into_iter()
                .map(|i| (i.record.to_string(), i.days.unwrap_or(0)))
                .collect()
        };
        let blocked: Vec<String> = next().into_iter().map(|i| i.record.to_string()).collect();
        let mut overdue = with_days(next());
        let idle = with_days(next());
        let late: Vec<(String, i64)> = self
            .store
            .late_answers(scope, today)
            .map_err(err)?
            .into_iter()
            .map(|(id, d)| (id.to_string(), d))
            .collect();
        let mut inbox = None;
        if let Scope::Project(p) = scope {
            for e in self.store.epics(p).map_err(err)? {
                if let DueState::Overdue(d) = due_state(e.due_on, today, e.progress.is_open()) {
                    overdue.push((e.id.to_string(), d));
                }
            }
            // Oldest date first; stable, so topics keep their order.
            overdue.sort_by_key(|(_, d)| std::cmp::Reverse(*d));
            let notes = next();
            if let Some(d) = notes.first().and_then(|n| n.days)
                && d >= INBOX_DAYS
            {
                inbox = Some(format!("inbox {}, oldest {d}d", notes.len()));
            }
        }
        let days = |v: Vec<(String, i64)>| -> Vec<String> {
            v.into_iter().map(|(id, d)| format!("{id} {d}d")).collect()
        };
        Ok(due::attention_line(
            &[
                ("blocked", blocked),
                ("overdue", days(overdue)),
                ("answers late", days(late)),
                ("idle", days(idle)),
            ],
            inbox,
            max,
        ))
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

    /// The brief, unfenced: every caller puts it inside one data fence.
    /// Stored text is flattened to one line per item.
    pub(crate) fn render_brief(&self, o: Owner, max: usize) -> Reply {
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
                                render::clip(&render::one_line(&s.title), 40),
                                s.progress.done,
                                s.progress.total
                            )
                        })
                        .unwrap_or_default(),
                    None => String::new(),
                };
                let today = self.session.today();
                let due = due::segment(t.due_on, today, t.status.is_open());
                let report = match &t.report.to {
                    Some(p) => {
                        let d = self
                            .store
                            .contact_any(p.id)
                            .map(|c| q::details(&c))
                            .unwrap_or_default();
                        q::header_segment(p, &d, t.report.is_reported())
                    }
                    None => String::new(),
                };
                // An idle Active topic: its state may be out of date.
                let idle = self
                    .store
                    .topic_idle_days(id, today, self.session.zone)
                    .map_err(err)?
                    .map_or(String::new(), |d| format!(" · no activity {d}d"));
                // Files on the description (ADR 0012, 10).
                let desc_files = super::attach::files_segment(
                    &self
                        .store
                        .attachments(AttachOwner::Topic(id))
                        .map_err(err)?,
                    1,
                );
                out.line_clipped(format!(
                    "{id} {} · {}{due}{idle}{report}{epic}{desc_files}{}",
                    render::clip(&render::one_line(&t.title), 80),
                    t.status.as_str(),
                    self.focus_mark(o)
                ));
                let counts = self.counts_line(o, t.project, None);
                let page = self
                    .store
                    .entries(id, &EntryFilter::default(), PageReq::first(3))
                    .map_err(err)?;
                // Reserve room for the counts so they always print.
                let reserve = render::chars(&counts) + 20;
                // Open questions, after the state and before the entries:
                // over budget, entries give way first, then question lines
                // from the last; the count line always stays.
                let open = self.open_questions(o)?;
                let qcount = q::count_line(&open, today);
                let qlines: Vec<String> = open
                    .iter()
                    .take(3)
                    .map(|x| q::brief_line(x, today))
                    .collect();
                let qroom = qcount.as_ref().map_or(0, |c| render::chars(c) + 1)
                    + qlines.iter().map(|l| render::chars(l) + 1).sum::<usize>();
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
                let room = out.left().saturating_sub(reserve + qroom + 2 * 60);
                out.line(render::defuse(&render::clip(&state, room.max(160))));
                if let Some(c) = &qcount {
                    out.line(c);
                    for l in &qlines {
                        if out.left() < reserve + render::chars(l) {
                            break;
                        }
                        out.line(l);
                    }
                }
                let files = self.entry_files(&page.items)?;
                for e in &page.items {
                    let room = out.left().saturating_sub(reserve);
                    if room < 40 {
                        break;
                    }
                    let seg = super::attach::segment_of(&files, AttachOwner::Entry(e.id), 1);
                    let text_room = room.min(130).saturating_sub(render::chars(&seg)).max(40);
                    out.line(format!("{}{seg}", entry_line(e, text_room)));
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
                let progress = s.as_ref().map_or(String::new(), |s| {
                    format!(" · {}/{} done", s.progress.done, s.progress.total)
                });
                let today = self.session.today();
                let open = s.as_ref().is_none_or(|s| s.progress.is_open());
                let report = match (&e.report.to, &s) {
                    (Some(p), Some(s)) => {
                        let d = self
                            .store
                            .contact_any(p.id)
                            .map(|c| q::details(&c))
                            .unwrap_or_default();
                        q::header_segment(p, &d, s.reported)
                    }
                    _ => String::new(),
                };
                let desc_files = super::attach::files_segment(
                    &self.store.attachments(AttachOwner::Epic(id)).map_err(err)?,
                    1,
                );
                out.line_clipped(format!(
                    "{id} {}{progress}{}{report}{desc_files}{}",
                    render::clip(&render::one_line(&e.title), 80),
                    due::segment(e.due_on, today, open),
                    self.focus_mark(o)
                ));
                if let Some(line) = self.attention(Scope::Epic(id), due::ATTENTION_EPIC)? {
                    out.line(line);
                }
                if !e.state.trim().is_empty() {
                    out.line(format!(
                        "State: {}",
                        render::clip(&render::one_line(&e.state), 400)
                    ));
                }
                let counts = self.counts_line(o, e.project, None);
                // The epic's own open questions: the count and two lines.
                let own = self.open_questions(o)?;
                if let Some(c) = q::count_line(&own, today) {
                    out.line(c);
                    for x in own.iter().take(2) {
                        if out.left() < 300 {
                            break;
                        }
                        out.line(q::brief_line(x, today));
                    }
                }
                let per_topic = self
                    .store
                    .open_questions_by_topic(e.project, today)
                    .map_err(err)?;
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
                        let qn = per_topic
                            .get(&t.id)
                            .map_or(String::new(), |(n, _)| format!(" · {n}q"));
                        format!(
                            "{} {}  {}{}{qn}{}",
                            t.id,
                            t.status.as_str(),
                            render::clip(&render::one_line(&t.title), 80),
                            due::short_segment(t.due_on, today, t.status.is_open()),
                            q::topic_segment(t)
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
                let today = self.session.today();
                let own_due = self.store.project(id).map_err(err)?.due_on;
                out.line_clipped(format!(
                    "{id} {} · {}/{} topics done{} · inbox {} · knowledge {}",
                    render::clip(&render::one_line(&p.title), 60),
                    p.progress.done,
                    p.progress.total,
                    due::segment(own_due, today, p.progress.is_open()),
                    p.inbox,
                    p.knowledge
                ));
                // Right after the header: the epic and topic lines below are
                // what gets cut when space is short.
                if let Some(line) = self.attention(Scope::Project(id), due::ATTENTION_PROJECT)? {
                    out.line(line);
                }
                let footer = "No focus topic: brief(id=\"T-n\", focus=true) sets one.";
                let epics = self.store.epics(id).map_err(err)?;
                let open = self
                    .store
                    .topics(
                        &TopicFilter {
                            project: Some(id),
                            epic: Some(None),
                            statuses: Status::ALL.into_iter().filter(|s| s.is_open()).collect(),
                            due_before: None,
                        },
                        PageReq::first(20),
                    )
                    .map_err(err)?;
                let mut lines: Vec<String> = epics
                    .iter()
                    .map(|e| {
                        format!(
                            "{} {} ({}/{} done){}",
                            e.id,
                            render::clip(&render::one_line(&e.title), 60),
                            e.progress.done,
                            e.progress.total,
                            due::short_segment(e.due_on, today, e.progress.is_open())
                        )
                    })
                    .collect();
                lines.extend(open.items.iter().map(|t| {
                    format!(
                        "{} {}  {}{}",
                        t.id,
                        t.status.as_str(),
                        render::clip(&render::one_line(&t.title), 60),
                        due::short_segment(t.due_on, today, t.status.is_open())
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

    /// The files of a page of entries, in one read.
    fn entry_files(&self, entries: &[Entry]) -> Result<FilesByOwner, String> {
        let owners: Vec<AttachOwner> = entries.iter().map(|e| AttachOwner::Entry(e.id)).collect();
        self.store.attachments_of(&owners).map_err(err)
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
            // Its files after the text, inside the fence (names are stored
            // text).
            let files = super::attach::files_segment(
                &self
                    .store
                    .attachments(AttachOwner::Entry(e.id))
                    .map_err(err)?,
                5,
            );
            let files = files.trim_start_matches(" · ").to_owned();
            let room = budget::ENTRY_FULL
                - head.len()
                - 1
                - render::FENCE_COST
                - 60
                - render::chars(&files);
            let total = render::chars(&e.text);
            let (body, cut) = if total > room {
                (
                    render::clip(&e.text, room),
                    Some(format!("[cut: {} more chars not shown]", total - room)),
                )
            } else {
                (e.text.clone(), None)
            };
            let body = if files.is_empty() {
                body
            } else {
                format!("{body}\n{files}")
            };
            return Ok(fenced_reply(
                Some(&head),
                &body,
                cut.as_deref(),
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
        let files = self.entry_files(&page.items)?;
        let lines: Vec<String> = page
            .items
            .iter()
            .map(|e| {
                format!(
                    "{}{}",
                    entry_line(e, 240),
                    super::attach::segment_of(&files, AttachOwner::Entry(e.id), 2)
                )
            })
            .collect();
        let n = render::fitting(&lines, max - 120 - render::FENCE_COST);
        let shown = &page.items[..n];
        let newest = shown.first().and_then(|e| e.seq).unwrap_or(0);
        let oldest = shown.last().and_then(|e| e.seq).unwrap_or(0);
        let more = page.total > n as i64;
        let head = format!(
            "{topic} entries #{newest}-#{oldest} of {all}{}",
            if more {
                format!(" · older: before={oldest}")
            } else {
                String::new()
            }
        );
        Ok(fenced_reply(Some(&head), &lines[..n].join("\n"), None, max))
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
        // Validated first: a bad date refuses the whole call.
        let due = a.due.as_deref().map(due::parse_arg).transpose()?;
        let id = a.id.trim().to_owned();
        let id = id.as_str();
        let upper = id.to_ascii_uppercase();
        if let Some(qid) = QuestionId::parse(id).filter(|_| upper.starts_with("Q-")) {
            return self.update_question(qid, a, due);
        }
        if let Some(cid) = ContactId::parse(id).filter(|_| upper.starts_with("C-")) {
            return self.update_contact(cid, a);
        }
        if a.level.is_some() {
            return Err("level is for contacts (C-n)".into());
        }
        if a.asked_to.is_some() || a.answer.is_some() || a.log_decision.is_some() {
            return Err("asked_to, answer and log_decision are for questions (Q-n)".into());
        }
        if let Some(p) = ProjectId::parse(id).filter(|_| upper.starts_with("P-")) {
            self.owner_project(Owner::Project(p))?;
            if a.report_to.is_some() || a.reported.is_some() {
                return Err("projects have no report_to".into());
            }
            let others = a.title.is_some()
                || a.description.is_some()
                || a.state.is_some()
                || a.status.is_some()
                || a.epic.is_some();
            if others {
                return Err("a project takes only due here; edit the rest in Acta".into());
            }
            let Some(due) = due else {
                return Err("nothing to change: pass due (YYYY-MM-DD, or \"\" to clear)".into());
            };
            self.store
                .set_due(Owner::Project(p), due, self.author())
                .map_err(err)?;
            return Ok(render::finish(
                format!("ok {p} · {}", due::ack(due, self.session.today())),
                budget::ACK,
            ));
        }
        // Who to report to: a contact the record sees, by id or by a name
        // that resolves to exactly one.
        let report_to: Option<Option<ContactId>> = match a.report_to.as_deref() {
            None => None,
            Some(p) => {
                let record = if upper.starts_with("T-") {
                    TopicId::parse(id).map(Record::Topic)
                } else if upper.starts_with("E-") {
                    EpicId::parse(id).map(Record::Epic)
                } else if upper.starts_with("N-") {
                    NoteId::parse(id).map(Record::Note)
                } else {
                    None
                }
                .ok_or("id must be T-n, E-n, N-n, Q-n, C-n or P-n")?;
                let place = self.store.place(record).map_err(err)?;
                self.check_project(place.project, id)?;
                Some(self.person_ref(p, place, id)?)
            }
        };
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
        let author = self.author();
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
            let changed = !fields.is_empty();
            let reported = a.reported;
            let person = report_to;
            self.store
                .atomically(|store| {
                    if changed {
                        store.update_topic(t, patch, author)?;
                    }
                    if let Some(d) = due {
                        store.set_due(Owner::Topic(t), d, author)?;
                    }
                    if let Some(p) = person {
                        store.set_report_to(Record::Topic(t), p, author)?;
                    }
                    if let Some(r) = reported {
                        store.set_reported(Record::Topic(t), r, author)?;
                    }
                    Ok(())
                })
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
            let changed = !fields.is_empty();
            let reported = a.reported;
            let person = report_to;
            self.store
                .atomically(|store| {
                    if changed {
                        store.update_epic(e, patch, author)?;
                    }
                    if let Some(d) = due {
                        store.set_due(Owner::Epic(e), d, author)?;
                    }
                    if let Some(p) = person {
                        store.set_report_to(Record::Epic(e), p, author)?;
                    }
                    if let Some(r) = reported {
                        store.set_reported(Record::Epic(e), r, author)?;
                    }
                    Ok(())
                })
                .map_err(err)?;
            e.to_string()
        } else if let Some(n) =
            NoteId::parse(id).filter(|_| id.to_ascii_uppercase().starts_with("N-"))
        {
            let note = self.store.note(n).map_err(err)?;
            self.check_project(note.project, id)?;
            if a.state.is_some() || a.epic.is_some() {
                return Err("a note has no state or epic; file it with create(from_notes)".into());
            }
            if due.is_some() {
                return Err("a note has no due date; set it on a topic, epic or project".into());
            }
            if a.reported.is_some() {
                return Err("a note has no reported; set it on the topic or epic it became".into());
            }
            let text = a.description.or(a.title);
            let fate = match a.status.as_deref().map(|s| s.trim().to_ascii_lowercase()) {
                None => None,
                Some(s) if s == "open" => Some(true),
                Some(s) if s == "dismissed" => Some(false),
                Some(_) => return Err("a note's status is open or dismissed".into()),
            };
            if text.is_none() && fate.is_none() && report_to.is_none() {
                return Err(
                    "nothing to change: pass description (new text), status (open, dismissed) or report_to"
                        .into(),
                );
            }
            if let Some(t) = &text {
                check_len("note", t, limit::NOTE)?;
            }
            if fate == Some(false) {
                match note.fate() {
                    NoteFate::Open => {}
                    NoteFate::Dismissed => return Err(format!("{n} is already dismissed")),
                    NoteFate::Topic(t) => {
                        return Err(format!(
                            "{n} is filed in {t}; put it back in the inbox first (status=\"open\")"
                        ));
                    }
                    NoteFate::Epic(e) => {
                        return Err(format!(
                            "{n} is filed as {e}; put it back in the inbox first (status=\"open\")"
                        ));
                    }
                }
            }
            let already_open = fate == Some(true) && note.is_open();
            let person = report_to;
            self.store
                .atomically(|store| {
                    if let Some(t) = &text {
                        store.update_note(n, t, author)?;
                    }
                    if let Some(p) = person {
                        store.set_report_to(Record::Note(n), p, author)?;
                    }
                    match fate {
                        Some(true) if !already_open => store.unfile_note(n, author),
                        Some(false) => store.dismiss_notes(&[n], author),
                        _ => Ok(()),
                    }
                })
                .map_err(err)?;
            fields = Vec::new();
            if text.is_some() {
                fields.push("text");
            }
            match fate {
                Some(true) if already_open => {
                    if text.is_none() && report_to.is_none() {
                        return Ok(format!("{n} is already in the inbox; nothing changed"));
                    }
                    fields.push("already in the inbox");
                }
                Some(true) => fields.push("back in the inbox"),
                Some(false) => fields.push("dismissed"),
                None => {}
            }
            n.to_string()
        } else {
            return Err("id must be T-n, E-n, N-n, Q-n, C-n or P-n".into());
        };
        let mut tail: Vec<String> = Vec::new();
        if let Some(d) = due {
            tail.push(due::ack(d, self.session.today()));
        }
        // Contacts by id only: names are stored text, read fenced.
        match &report_to {
            Some(Some(c)) => tail.push(format!("report to {c} (existing contact)")),
            Some(None) => tail.push("report_to cleared".into()),
            None => {}
        }
        match a.reported {
            Some(true) => {
                let who = match parse_owner(&label) {
                    Some(Owner::Topic(t)) => self.store.topic(t).ok().and_then(|t| t.report.to),
                    Some(Owner::Epic(e)) => self.store.epic(e).ok().and_then(|e| e.report.to),
                    _ => None,
                };
                tail.push(match who {
                    Some(c) => format!("reported to {}", c.id),
                    None => "reported".into(),
                });
            }
            Some(false) => tail.push("not reported".into()),
            None => {}
        }
        let text = match (fields.is_empty(), tail.is_empty()) {
            (true, true) => {
                return Err(
                    "nothing to change: pass title, description, state, status, epic, due, report_to or reported"
                        .into(),
                );
            }
            (true, false) => format!("ok {label} · {}", tail.join(" · ")),
            (false, false) => format!(
                "ok {label} updated: {} · {}",
                fields.join(", "),
                tail.join(" · ")
            ),
            (false, true) => format!("ok {label} updated: {}", fields.join(", ")),
        };
        Ok(render::finish(text, budget::ACK))
    }

    /// `update` on a contact: its level only (data.md request 22). A move
    /// down is refused while a record outside the new level names it.
    /// The name and details, merging and deleting are the user's, in Acta.
    fn update_contact(&mut self, id: ContactId, a: UpdateArgs) -> Reply {
        let c = self.store.contact(id).map_err(err)?;
        if let Some(p) = c.level.project() {
            self.check_project(p, &id.to_string())?;
        }
        let others = a.title.is_some()
            || a.description.is_some()
            || a.state.is_some()
            || a.status.is_some()
            || a.epic.is_some()
            || a.due.is_some()
            || a.asked_to.is_some()
            || a.answer.is_some()
            || a.log_decision.is_some()
            || a.report_to.is_some()
            || a.reported.is_some();
        if others {
            return Err(
                "a contact takes only level here; its name and details are the user's, in Acta"
                    .into(),
            );
        }
        let Some(level) = a.level.as_deref() else {
            return Err("nothing to change: pass level (all, P-n or E-n)".into());
        };
        let level = level_arg(self.store, level)?;
        if let Some(p) = level.project() {
            self.check_project(p, &level.to_string())?;
        }
        self.store
            .move_contact(id, level, self.author())
            .map_err(err)?;
        Ok(render::finish(
            format!("ok {id} · level {}", q::level_short(level)),
            budget::ACK,
        ))
    }

    /// `update` on a question (data.md request 18): text, person, answer
    /// date, state, answer, and the decision logged with the answer.
    fn update_question(
        &mut self,
        id: QuestionId,
        a: UpdateArgs,
        due: Option<Option<Date>>,
    ) -> Reply {
        let cur = self.store.question(id).map_err(err)?;
        self.check_project(cur.project, &id.to_string())?;
        if a.description.is_some()
            || a.state.is_some()
            || a.epic.is_some()
            || a.report_to.is_some()
            || a.reported.is_some()
        {
            return Err(
                "a question takes title, asked_to, due, status, answer and log_decision".into(),
            );
        }
        if let Some(t) = &a.title {
            check_len("title", t, QUESTION_MAX)?;
        }
        if a.level.is_some() {
            return Err("level is for contacts (C-n)".into());
        }
        let asked: Option<Option<ContactId>> = match a.asked_to.as_deref() {
            None => None,
            Some(p) => {
                let place = self.store.place(Record::Question(id)).map_err(err)?;
                Some(self.person_ref(p, place, &id.to_string())?)
            }
        };
        if let Some(ans) = &a.answer {
            check_len("answer", ans, ANSWER_MAX)?;
        }
        let status = match a.status.as_deref().map(|s| s.trim().to_ascii_lowercase()) {
            None => None,
            Some(s) => Some(
                QuestionStatus::parse(&s)
                    .ok_or("a question's status is open, answered or dropped")?,
            ),
        };
        let log = a.log_decision.unwrap_or(false);
        if log && status != Some(QuestionStatus::Answered) {
            return Err("log_decision goes with status=\"answered\"".into());
        }
        if log && !matches!(cur.owner, Owner::Topic(_)) {
            return Err(format!(
                "log_decision needs a question on a topic; {id} is on {}",
                cur.owner
            ));
        }
        if a.answer.is_some() && status.is_none() && cur.status != QuestionStatus::Answered {
            return Err(format!(
                "{id} is {}; pass status=\"answered\" with the answer",
                cur.status.as_str()
            ));
        }
        if a.answer.is_some()
            && matches!(status, Some(QuestionStatus::Open | QuestionStatus::Dropped))
        {
            return Err("answer goes with status=\"answered\"".into());
        }
        let mut fields = Vec::new();
        if a.title.is_some() {
            fields.push("title");
        }
        if asked.is_some() {
            fields.push("asked_to");
        }
        // Answering without an answer keeps the one already written (a
        // reopened question keeps its text).
        let answer = a.answer.clone().or(cur.answer.clone());
        let answering = status == Some(QuestionStatus::Answered)
            || (a.answer.is_some() && cur.status == QuestionStatus::Answered);
        let author = self.author();
        let title = a.title.clone();
        let entry = self
            .store
            .atomically(|store| {
                if title.is_some() || asked.is_some() {
                    store.edit_question(id, title.as_deref(), asked, author)?;
                }
                if let Some(d) = due {
                    store.set_expected(id, d, author)?;
                }
                let mut entry = None;
                match status {
                    _ if answering => {
                        entry = store.answer(id, answer.as_deref(), log, author)?;
                    }
                    Some(QuestionStatus::Dropped) => store.drop_question(id, author)?,
                    Some(QuestionStatus::Open) => store.reopen_question(id, author)?,
                    _ => {}
                }
                Ok(entry)
            })
            .map_err(err)?;
        let mut parts = vec![format!("ok {id}")];
        match status {
            Some(QuestionStatus::Answered) if cur.status != QuestionStatus::Answered => {
                parts[0].push_str(" answered")
            }
            Some(QuestionStatus::Dropped) if cur.status != QuestionStatus::Dropped => {
                parts[0].push_str(" dropped")
            }
            Some(QuestionStatus::Open) if cur.status != QuestionStatus::Open => {
                parts[0].push_str(" reopened")
            }
            Some(s) if s == cur.status && a.answer.is_none() && !log => {
                parts[0].push_str(&format!(" already {}", s.as_str()))
            }
            _ => {}
        }
        if a.answer.is_some() && cur.status == QuestionStatus::Answered {
            fields.push("answer");
        }
        if !fields.is_empty() {
            parts.push(format!("updated: {}", fields.join(", ")));
        }
        if let Some(e) = &entry {
            parts.push(format!(
                "logged {}#{} decision",
                e.topic,
                e.seq.unwrap_or(0)
            ));
        }
        if let Some(d) = due {
            parts.push(q::by_ack(d, self.session.today()));
        }
        if parts.len() == 1 && status.is_none() {
            return Err("nothing to change: pass title, asked_to, due, status or answer".into());
        }
        Ok(render::finish(parts.join(" · "), budget::ACK))
    }

    /// `acta update --json`: what a script needs after an update, the id,
    /// the due date (`YYYY-MM-DD` or null) and whether it is overdue.
    /// Notes have no date: only the id.
    pub fn record_json(&self, id: &str) -> Reply {
        let today = self.session.today();
        let id = id.trim();
        // Stored text (a question, an answer, a person) only for the human:
        // in Claude Code, stored text is read fenced, from the text forms.
        let text_ok = self.author() == Author::Human;
        let upper = id.to_ascii_uppercase();
        if let Some(qid) = QuestionId::parse(id).filter(|_| upper.starts_with("Q-")) {
            let x = self.store.question(qid).map_err(err)?;
            let mut o = serde_json::json!({
                "id": qid.to_string(),
                "owner": x.owner.to_string(),
                "status": x.status.as_str(),
                "due": x.expected_on.map(|d| d.to_string()),
                "overdue": due::is_overdue(x.expected_on, today, x.is_open()),
                "resolved_at": x.resolved_at,
                "decision": x.decision_entry.and_then(|e| self.store.entry(e).ok())
                    .map(|e| format!("{}#{}", e.topic, e.seq.unwrap_or(0))),
            });
            if text_ok {
                o["text"] = x.text.into();
                o["asked_to"] = x.asked_to.as_ref().map(|c| c.name.clone()).into();
                o["asked_contact"] = x.asked_to.map(|c| c.id.to_string()).into();
                o["answer"] = x.answer.into();
            }
            return Ok(o.to_string());
        }
        if let Some(n) = NoteId::parse(id).filter(|_| upper.starts_with("N-")) {
            let mut o = serde_json::json!({ "id": n.to_string() });
            if text_ok {
                o["report_to"] = self
                    .store
                    .note(n)
                    .map_err(err)?
                    .report_to
                    .map(|c| c.name)
                    .into();
            }
            return Ok(o.to_string());
        }
        let mut report = None;
        let (due, open) = match parse_owner(id) {
            Some(Owner::Topic(t)) => {
                let t = self.store.topic(t).map_err(err)?;
                report = Some(t.report.clone());
                (t.due_on, t.status.is_open())
            }
            Some(Owner::Epic(e)) => {
                let epic = self.store.epic(e).map_err(err)?;
                let open = self
                    .store
                    .epics(epic.project)
                    .map_err(err)?
                    .iter()
                    .find(|s| s.id == e)
                    .is_none_or(|s| s.progress.is_open());
                report = Some(epic.report.clone());
                (epic.due_on, open)
            }
            Some(Owner::Project(p)) => {
                let due = self.store.project(p).map_err(err)?.due_on;
                let open = self
                    .store
                    .projects()
                    .map_err(err)?
                    .iter()
                    .find(|s| s.id == p)
                    .is_none_or(|s| s.progress.is_open());
                (due, open)
            }
            None => return Ok(serde_json::json!({ "id": id.to_ascii_uppercase() }).to_string()),
        };
        let mut o = serde_json::json!({
            "id": id.to_ascii_uppercase(),
            "due": due.map(|d| d.to_string()),
            "overdue": due::is_overdue(due, today, open),
        });
        if let Some(r) = report {
            o["reported_at"] = r.at.into();
            if text_ok {
                o["report_to"] = r.to.as_ref().map(|c| c.name.clone()).into();
                o["report_contact"] = r.to.map(|c| c.id.to_string()).into();
            }
        }
        Ok(o.to_string())
    }

    // ---- find ----

    pub fn find(&mut self, a: FindArgs) -> Reply {
        let page = self.find_page(a)?;
        if let Some(note) = &page.note {
            // Names are stored text: inside the fence.
            return Ok(fenced_reply(None, note, None, budget::FIND));
        }
        if page.hits.is_empty() && page.head.is_empty() {
            return Ok("0 hits".into());
        }
        // The person header (details, address) first, then the hits.
        let lines: Vec<String> = page
            .head
            .iter()
            .cloned()
            .chain(page.hits.iter().map(|h| h.line.clone()))
            .collect();
        // Titles, notes and excerpts are stored text: all inside the fence.
        let n = render::fitting(&lines, budget::FIND - 60 - render::FENCE_COST);
        let shown_end = page.offset + n.saturating_sub(page.head.len()) as i64;
        let foot = if shown_end < page.total {
            format!(
                "{}-{shown_end} of {} · more: cursor={shown_end}",
                page.offset + 1,
                page.total
            )
        } else {
            plural(page.total, "hit", "hits")
        };
        Ok(fenced_reply(
            None,
            &lines[..n].join("\n"),
            Some(&foot),
            budget::FIND,
        ))
    }

    /// `acta find --json`: the same page as `find`, for scripts. Topics and
    /// epics carry `due` (`YYYY-MM-DD` or null) and `overdue`.
    pub fn find_json(&mut self, a: FindArgs) -> Reply {
        let page = self.find_page(a)?;
        let end = page.offset + page.hits.len() as i64;
        let hits: Vec<serde_json::Value> = page.hits.iter().map(FindHit::json).collect();
        let mut o = serde_json::json!({
            "hits": hits,
            "total": page.total,
            "next_cursor": (end < page.total).then_some(end),
        });
        if let Some(n) = page.note {
            o["note"] = n.into();
        }
        if !page.head.is_empty() {
            o["person"] = page.head.join("\n").into();
        }
        Ok(o.to_string())
    }

    /// One page of `find`: the hits and their lines, before any budget.
    fn find_page(&mut self, a: FindArgs) -> Result<FindPage, String> {
        let project = self.project()?;
        let limit = a.limit.unwrap_or(8).clamp(1, 20);
        let offset = a.cursor.unwrap_or(0).max(0);
        let today = self.session.today();
        let kind = a.kind.as_deref().map(|k| k.trim().to_ascii_lowercase());
        if kind.as_deref().is_some_and(|k| {
            !matches!(
                k,
                "topic" | "epic" | "note" | "entry" | "question" | "contact"
            )
        }) {
            return Err("kind must be topic, epic, note, entry, question or contact".into());
        }
        let status_word = a.status.as_deref().map(|s| s.trim().to_ascii_lowercase());
        if let Some(p) = a.person.as_deref() {
            check_len("person", p, crate::person::MAX_CHARS)?;
            if a.query.is_some() || a.due.is_some() || a.epic.is_some() || a.status.is_some() {
                return Err("person lists everything open naming them; it takes only kind".into());
            }
            if matches!(kind.as_deref(), Some("entry" | "contact")) {
                return Err("person applies to questions, topics, epics and notes".into());
            }
            return self.find_person(project, p, kind.as_deref(), PageReq { offset, limit });
        }
        if kind.as_deref() == Some("contact") {
            if a.due.is_some() || a.epic.is_some() || a.status.is_some() {
                return Err("kind=contact takes only query".into());
            }
            let query = a.query.as_deref().map(str::trim).filter(|q| !q.is_empty());
            return self.find_contacts(project, query, PageReq { offset, limit });
        }
        if kind.as_deref() == Some("question") {
            let query = a.query.as_deref().map(str::trim).filter(|q| !q.is_empty());
            if let Some(q) = query {
                check_len("query", q, limit::QUERY)?;
            }
            let epic = a.epic.as_deref().map(|e| self.epic_arg(e)).transpose()?;
            let cutoff = a
                .due
                .as_deref()
                .map(|d| due::find_cutoff(d, today))
                .transpose()?;
            return self.find_questions(
                project,
                status_word.as_deref(),
                query,
                epic,
                cutoff,
                None,
                PageReq { offset, limit },
            );
        }
        let statuses: Vec<Status> = match status_word.as_deref() {
            None | Some("open") => Status::ALL.into_iter().filter(|s| s.is_open()).collect(),
            Some("closed") => vec![Status::Done, Status::Dropped],
            Some("all") => Vec::new(),
            Some(s) => vec![status_of(s)?],
        };
        let epic = a.epic.as_deref().map(|e| self.epic_arg(e)).transpose()?;
        let query = a.query.as_deref().map(str::trim).filter(|q| !q.is_empty());
        if let Some(q) = query {
            check_len("query", q, limit::QUERY)?;
        }
        if let Some(d) = a.due.as_deref() {
            let cutoff = due::find_cutoff(d, today)?;
            if query.is_some() {
                return Err("due lists by date and does not take a query".into());
            }
            if matches!(kind.as_deref(), Some("note" | "entry")) {
                return Err("due applies to topics and epics".into());
            }
            // Only open records can be overdue or due soon; a specific
            // status narrows the topics and leaves epics out.
            let specific = !matches!(status_word.as_deref(), None | Some("open" | "all"));
            return self.find_due(
                project,
                cutoff,
                kind.as_deref(),
                statuses,
                specific,
                epic,
                PageReq { offset, limit },
            );
        }
        let mut hits: Vec<FindHit> = Vec::new();
        let page_total: i64;
        match (query, kind.as_deref()) {
            (None, Some("note")) => {
                let page = self
                    .store
                    .inbox(project, PageReq { offset, limit })
                    .map_err(err)?;
                let owners: Vec<AttachOwner> =
                    page.items.iter().map(|n| AttachOwner::Note(n.id)).collect();
                let files = self.store.attachments_of(&owners).map_err(err)?;
                hits.extend(page.items.iter().map(|n| {
                    let mut hit = q::note_hit(n);
                    with_files(&mut hit, &files, AttachOwner::Note(n.id));
                    hit
                }));
                page_total = page.total;
            }
            (None, Some("epic")) => {
                let all = self.store.epics(project).map_err(err)?;
                page_total = all.len() as i64;
                hits.extend(
                    all.iter()
                        .skip(offset as usize)
                        .take(limit as usize)
                        .map(|e| epic_hit(e, today, false)),
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
                            due_before: None,
                        },
                        PageReq { offset, limit },
                    )
                    .map_err(err)?;
                hits.extend(page.items.iter().map(|t| topic_hit(t, today, false)));
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
                            epic_hits.push(epic_hit(&e, today, false));
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
                hits.extend(
                    epic_hits
                        .into_iter()
                        .skip(offset as usize)
                        .take(limit as usize),
                );
                let search_offset = (offset - epic_count).max(0);
                let search_limit = limit - hits.len() as i64;
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
                    let owners: Vec<AttachOwner> = items
                        .iter()
                        .filter_map(|h| match h.record {
                            Record::Note(n) => Some(AttachOwner::Note(n)),
                            Record::Entry(e) => Some(AttachOwner::Entry(e)),
                            _ => None,
                        })
                        .collect();
                    let files = self.store.attachments_of(&owners).map_err(err)?;
                    for h in items {
                        let snip = render::clip(&render::one_line(&strip_marks(&h.snippet)), 150);
                        let hit = match h.record {
                            Record::Topic(t) => {
                                let Ok(topic) = self.store.topic(t) else {
                                    continue;
                                };
                                let open = topic.status.is_open();
                                FindHit {
                                    id: t.to_string(),
                                    kind: "topic",
                                    status: Some(topic.status.as_str()),
                                    title: Some(topic.title.clone()),
                                    text: Some(snip.clone()),
                                    due: Some(topic.due_on),
                                    overdue: Some(due::is_overdue(topic.due_on, today, open)),
                                    line: format!(
                                        "{t} {}  {} - {snip}",
                                        topic.status.as_str(),
                                        render::clip(&render::one_line(&topic.title), 60)
                                    ),
                                    extra: report_extra(&topic.report),
                                }
                            }
                            Record::Entry(id) => {
                                let Ok(e) = self.store.entry(id) else {
                                    continue;
                                };
                                let eid = format!("{}#{}", e.topic, e.seq.unwrap_or(0));
                                let mut hit = FindHit {
                                    line: format!("{eid} {}  {snip}", e.kind.as_str()),
                                    id: eid,
                                    kind: "entry",
                                    text: Some(snip),
                                    ..FindHit::default()
                                };
                                with_files(&mut hit, &files, AttachOwner::Entry(id));
                                hit
                            }
                            Record::Note(n) => {
                                let mut hit = FindHit {
                                    id: n.to_string(),
                                    kind: "note",
                                    line: format!("{n} note  {snip}"),
                                    text: Some(snip),
                                    ..FindHit::default()
                                };
                                with_files(&mut hit, &files, AttachOwner::Note(n));
                                hit
                            }
                            _ => continue,
                        };
                        hits.push(hit);
                    }
                }
                page_total = total;
            }
        }
        Ok(FindPage {
            hits,
            offset,
            total: page_total,
            note: None,
            head: Vec::new(),
        })
    }

    /// `find(due=...)`: open topics and epics due before `cutoff`, by date
    /// then stored order, an epic before the topics due the same day.
    #[allow(clippy::too_many_arguments)]
    fn find_due(
        &self,
        project: ProjectId,
        cutoff: Date,
        kind: Option<&str>,
        statuses: Vec<Status>,
        specific_status: bool,
        epic: Option<EpicId>,
        page: PageReq,
    ) -> Result<FindPage, String> {
        let today = self.session.today();
        let mut epics: Vec<EpicSummary> =
            if matches!(kind, None | Some("epic")) && epic.is_none() && !specific_status {
                self.store
                    .epics(project)
                    .map_err(err)?
                    .into_iter()
                    .filter(|e| e.progress.is_open() && e.due_on.is_some_and(|d| d < cutoff))
                    .collect()
            } else {
                Vec::new()
            };
        // Stable: the same date keeps the stored order.
        epics.sort_by_key(|e| e.due_on);
        let open: Vec<Status> = if statuses.is_empty() {
            Status::ALL.into_iter().filter(|s| s.is_open()).collect()
        } else {
            statuses.into_iter().filter(|s| s.is_open()).collect()
        };
        // A topic at index i of the date order sits at merged position i
        // plus the epics before it, at most i + epics: every topic before
        // `skip` is before the page, so the query can start there.
        let skip = (page.offset - epics.len() as i64).max(0);
        let (topics, topic_total) = if matches!(kind, None | Some("topic")) && !open.is_empty() {
            let p = self
                .store
                .topics(
                    &TopicFilter {
                        project: Some(project),
                        epic: epic.map(Some),
                        statuses: open,
                        due_before: Some(cutoff),
                    },
                    PageReq {
                        offset: skip,
                        limit: page.limit + epics.len() as i64,
                    },
                )
                .map_err(err)?;
            (p.items, p.total)
        } else {
            (Vec::new(), 0)
        };
        let mut merged: Vec<FindHit> = Vec::new();
        let (mut ti, mut ei) = (0, 0);
        while ti < topics.len() || ei < epics.len() {
            let epic_first = match (topics.get(ti), epics.get(ei)) {
                (Some(t), Some(e)) => e.due_on <= t.due_on,
                (None, Some(_)) => true,
                _ => false,
            };
            if epic_first {
                merged.push(epic_hit(&epics[ei], today, true));
                ei += 1;
            } else {
                merged.push(topic_hit(&topics[ti], today, true));
                ti += 1;
            }
        }
        let hits = merged
            .into_iter()
            .skip((page.offset - skip) as usize)
            .take(page.limit as usize)
            .collect();
        Ok(FindPage {
            hits,
            offset: page.offset,
            total: topic_total + epics.len() as i64,
            note: None,
            head: Vec::new(),
        })
    }

    // ---- create ----

    /// One open note of this project, not already used in this call.
    fn open_note(
        &self,
        id: &str,
        what: &str,
        seen: &mut Vec<NoteId>,
    ) -> Result<(NoteId, Note), String> {
        let nid = NoteId::parse(id)
            .filter(|_| id.trim().to_ascii_uppercase().starts_with("N-"))
            .ok_or(format!("{what}: {} is not a note id", render::clip(id, 20)))?;
        let note = self.store.note(nid).map_err(err)?;
        self.check_project(note.project, &nid.to_string())?;
        if seen.contains(&nid) {
            return Err(format!("{what}: {nid} is used twice in this call"));
        }
        let gone = match note.fate() {
            NoteFate::Open => None,
            NoteFate::Topic(t) => Some(format!("filed in {t}")),
            NoteFate::Epic(e) => Some(format!("filed as {e}")),
            NoteFate::Dismissed => Some("dismissed".to_owned()),
        };
        if let Some(gone) = gone {
            return Err(format!(
                "{what}: {nid} is already {gone}; update(id=\"{nid}\", status=\"open\") puts it back in the inbox"
            ));
        }
        seen.push(nid);
        Ok((nid, note))
    }

    /// Creates topics, epics and notes, files inbox notes into them or into
    /// existing topics, and dismisses notes: validated first, then written
    /// in one transaction, so a bad item writes nothing.
    pub fn create(&mut self, a: CreateArgs) -> Reply {
        let project = self.project()?;
        let dismiss = a.dismiss.unwrap_or_default();
        if a.items.is_empty() && dismiss.is_empty() {
            return Err("nothing to do: pass items, dismiss or both".into());
        }
        if a.items.len() > limit::CREATE_ITEMS {
            return Err(format!(
                "{} items; the limit is {} per call",
                a.items.len(),
                limit::CREATE_ITEMS
            ));
        }
        let note_count = a
            .items
            .iter()
            .map(|i| i.from_notes.as_ref().map_or(0, Vec::len))
            .sum::<usize>()
            + dismiss.len();
        if note_count > limit::CREATE_NOTES {
            return Err(format!(
                "{note_count} notes; the limit is {} per call",
                limit::CREATE_NOTES
            ));
        }
        let kinds: Vec<String> = a
            .items
            .iter()
            .map(|i| i.kind.trim().to_ascii_lowercase())
            .collect();
        let mut seen = Vec::new();
        let mut plans = Vec::new();
        for (i, it) in a.items.iter().enumerate() {
            let what = format!("item {}", i + 1);
            let allowed: &[&str] = match kinds[i].as_str() {
                "topic" => &[
                    "title",
                    "description",
                    "epic",
                    "from_notes",
                    "due",
                    "report_to",
                ],
                "epic" => &["title", "description", "from_notes", "due", "report_to"],
                "note" => &["title", "description", "report_to"],
                "entry" => &["topic", "from_notes"],
                "question" => &["title", "asked_to", "due", "owner"],
                "contact" => &["title", "level"],
                _ => {
                    return Err(format!(
                        "{what}: kind must be topic, epic, note, entry, question or contact"
                    ));
                }
            };
            let given = [
                ("title", !it.title.trim().is_empty()),
                ("description", it.description.is_some()),
                ("epic", it.epic.is_some()),
                ("topic", it.topic.is_some()),
                (
                    "from_notes",
                    it.from_notes.as_ref().is_some_and(|n| !n.is_empty()),
                ),
                ("due", it.due.is_some()),
                ("asked_to", it.asked_to.is_some()),
                ("owner", it.owner.is_some()),
                ("report_to", it.report_to.is_some()),
                ("level", it.level.is_some()),
            ];
            if let Some((field, _)) = given.iter().find(|(f, g)| *g && !allowed.contains(f)) {
                let article = if matches!(kinds[i].as_str(), "epic" | "entry") {
                    "an"
                } else {
                    "a"
                };
                return Err(format!(
                    "{what}: {article} {} takes only {}, not {field}",
                    kinds[i],
                    allowed.join(", ")
                ));
            }
            let mut notes = Vec::new();
            for id in it.from_notes.iter().flatten() {
                notes.push(self.open_note(id, &what, &mut seen)?.1);
            }
            let ids: Vec<NoteId> = notes.iter().map(|n| n.id).collect();
            let title = it.title.trim().to_owned();
            let kind = kinds[i].as_str();
            // A person: `@k` names a contact item of this call; otherwise
            // a contact the new record sees (its project; a topic's
            // existing epic too), by id or by a name naming exactly one.
            let person = |text: &str, place: Place| -> Result<Option<PersonRef>, String> {
                let t = text.trim();
                if let Some(k) = t.strip_prefix('@') {
                    let k = k
                        .parse::<usize>()
                        .ok()
                        .filter(|k| *k >= 1 && kinds.get(k - 1).is_some_and(|x| x == "contact"))
                        .ok_or(format!(
                            "{what}: {} must name a contact item of this call (@1 is item 1)",
                            render::clip(t, 20)
                        ))?;
                    return Ok(Some(PersonRef::Item(k - 1)));
                }
                self.person_ref(t, place, &what)
                    .map(|c| c.map(PersonRef::Existing))
                    .map_err(|e| format!("{what}: {e}"))
            };
            let base = Place {
                project,
                epic: None,
            };
            let report_to = match it.report_to.as_deref() {
                Some(p) => {
                    let place = match (kind, it.epic.as_deref().map(str::trim)) {
                        ("topic", Some(e)) if !e.starts_with('@') => Place {
                            project,
                            epic: EpicId::parse(e),
                        },
                        _ => base,
                    };
                    person(p, place)?
                }
                None => None,
            };
            let due = match it.due.as_deref() {
                None => None,
                Some(d) => due::parse_arg(d).map_err(|e| format!("{what}: {e}"))?,
            };
            if matches!(kind, "topic" | "epic") {
                if title.is_empty() {
                    return Err(format!("{what}: a {kind} needs a title"));
                }
                check_len(&format!("{what} title"), &title, limit::TITLE)?;
            }
            // Without a description, the human's words from the notes.
            let description = match &it.description {
                Some(d) => {
                    check_len(&format!("{what} description"), d, limit::DESCRIPTION)?;
                    d.clone()
                }
                None => {
                    let joined: Vec<&str> = notes.iter().map(|n| n.text.trim()).collect();
                    render::clip(&joined.join("\n\n"), limit::DESCRIPTION)
                }
            };
            match kind {
                "topic" => {
                    let epic = match it.epic.as_deref().map(str::trim) {
                        None => None,
                        Some(e) if e.starts_with('@') => {
                            let k = e[1..]
                                .parse::<usize>()
                                .ok()
                                .filter(|k| {
                                    *k >= 1 && kinds.get(k - 1).is_some_and(|x| x == "epic")
                                })
                                .ok_or(format!(
                                    "{what}: epic {} must name an epic item of this call (@1 is item 1)",
                                    render::clip(e, 20)
                                ))?;
                            Some(EpicRef::Item(k - 1))
                        }
                        Some(e) => Some(EpicRef::Existing(self.epic_arg(e)?)),
                    };
                    // Topics from notes are proposals the human refines.
                    let status = if ids.is_empty() {
                        Status::Planned
                    } else {
                        Status::Draft
                    };
                    plans.push(Plan::Topic {
                        epic,
                        title,
                        description,
                        status,
                        notes: ids,
                        due,
                        report_to,
                    });
                }
                "epic" => plans.push(Plan::Epic {
                    title,
                    description,
                    notes: ids,
                    due,
                    report_to,
                }),
                "question" => {
                    if title.is_empty() {
                        return Err(format!("{what}: a question needs its text in title"));
                    }
                    check_len(&format!("{what} title"), &title, QUESTION_MAX)?;
                    let owner = match it.owner.as_deref() {
                        Some(o) => {
                            let o = parse_owner(o)
                                .ok_or(format!("{what}: owner must be T-n, E-n or P-n"))?;
                            self.owner_project(o)?;
                            o
                        }
                        None => self
                            .session
                            .focus
                            .ok_or(format!("{what}: pass owner (no focus)"))?,
                    };
                    let place = self
                        .store
                        .place(match owner {
                            Owner::Project(p) => Record::Project(p),
                            Owner::Epic(e) => Record::Epic(e),
                            Owner::Topic(t) => Record::Topic(t),
                        })
                        .map_err(err)?;
                    let asked = match it.asked_to.as_deref() {
                        Some(p) => person(p, place)?,
                        None => None,
                    };
                    plans.push(Plan::Question(
                        NewQuestion {
                            owner,
                            text: title,
                            asked_to: None,
                            expected_on: due,
                        },
                        asked,
                    ));
                }
                "contact" => {
                    // Name and level only: the details are the user's.
                    let name = crate::person::clean(&title)
                        .map_err(|e| format!("{what}: {e}"))?
                        .ok_or(format!("{what}: a contact needs its name in title"))?;
                    let level = match it.level.as_deref() {
                        Some(l) => {
                            let l = level_arg(self.store, l).map_err(|e| format!("{what}: {e}"))?;
                            if let Some(p) = l.project() {
                                self.check_project(p, &l.to_string())?;
                            }
                            l
                        }
                        None => ContactLevel::Project(project),
                    };
                    plans.push(Plan::Contact(name, level));
                }
                "note" => {
                    if !ids.is_empty() {
                        return Err(format!("{what}: a note cannot come from notes"));
                    }
                    let d = it.description.clone().unwrap_or_default();
                    let text = if d.trim().is_empty() {
                        title
                    } else {
                        format!("{title}\n\n{}", d.trim())
                    };
                    if text.is_empty() {
                        return Err(format!("{what}: a note needs its text in title"));
                    }
                    check_len(&format!("{what} note"), &text, limit::NOTE)?;
                    plans.push(Plan::Note(text, report_to));
                }
                "entry" => {
                    let topic = it
                        .topic
                        .as_deref()
                        .ok_or(format!("{what}: an entry needs topic=\"T-n\""))?;
                    let topic = self.topic_arg(Some(topic))?;
                    if notes.is_empty() {
                        return Err(format!(
                            "{what}: an entry comes from notes; for your own text call log"
                        ));
                    }
                    plans.push(Plan::Entries { topic, notes });
                }
                _ => {
                    return Err(format!(
                        "{what}: kind must be topic, epic, note, entry or question"
                    ));
                }
            }
        }
        let mut dismissed = Vec::new();
        for id in &dismiss {
            dismissed.push(self.open_note(id, "dismiss", &mut seen)?.0);
        }

        let author = self.author();
        let today = self.session.today();
        let (acks, carried) = self
            .store
            .atomically(|store| apply_plans(store, project, plans, &dismissed, author, today))
            .map_err(err)?;
        let render_acks = |notes_shown: bool| {
            let mut parts: Vec<String> = acks
                .iter()
                .map(|(ack, notes)| {
                    if notes_shown && !notes.is_empty() {
                        format!("{ack} ({})", id_list(notes))
                    } else {
                        ack.clone()
                    }
                })
                .collect();
            if !dismissed.is_empty() {
                parts.push(if notes_shown {
                    format!("dismissed {}", id_list(&dismissed))
                } else {
                    format!(
                        "dismissed {}",
                        plural(dismissed.len() as i64, "note", "notes")
                    )
                });
            }
            format!("ok {}", parts.join(" · "))
        };
        let full = render_acks(true);
        let text = if render::chars(&full) <= budget::CREATE {
            full
        } else {
            render_acks(false)
        };
        if carried.is_empty() {
            return Ok(render::finish(text, budget::CREATE));
        }
        // Who filing carried and who it did not: names read from the
        // notes are stored text, inside the fence.
        Ok(fenced_reply(
            Some(&text),
            &carried.join("\n"),
            None,
            budget::CREATE + render::FENCE_COST + 160,
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
            let mut lines = Vec::new();
            for h in &page.items {
                let Record::Kb(id) = h.record else { continue };
                let section = self
                    .store
                    .kb_entry(id)
                    .ok()
                    .and_then(|e| section_of_match(&e.body, &h.snippet))
                    .map_or(String::new(), |s| format!(" §{s}"));
                let excerpt = render::clip(&render::one_line(&strip_marks(&h.snippet)), 160);
                lines.push(format!(
                    "{id}{section} {}: {excerpt}",
                    render::clip(&render::one_line(&h.title), 60)
                ));
            }
            let end = offset + page.items.len() as i64;
            let foot =
                (end < page.total).then(|| format!("{} more: cursor={end}", page.total - end));
            return Ok(fenced_reply(
                None,
                &lines.join("\n"),
                foot.as_deref(),
                budget::KB,
            ));
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
        let n = render::fitting(&lines, budget::KB - 130 - render::FENCE_COST);
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
        if page.total == 0 {
            return Ok(head);
        }
        Ok(fenced_reply(
            Some(&head),
            &lines[..n].join("\n"),
            None,
            budget::KB,
        ))
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
        // Ids and sizes outside the fence; the title, outline and body are
        // stored text and go inside it.
        let head = format!(
            "{id} {} · rev {} · {}",
            kind_word(e.kind),
            e.rev,
            render::token_label(size)
        );
        let title = format!("Title: {}", render::clip(&render::one_line(&e.title), 100));
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
                        outline_fenced(&e.body, 1_200)
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
                &title,
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
                &title,
                &all[from - 1..to],
                from,
                all.len(),
                max,
            ));
        }
        // Whole when the fenced body fits, line by line (a line never has
        // more than `WRAP` characters).
        if size + all.len() + render::chars(&head) + render::chars(&title) + render::FENCE_COST + 4
            < max
        {
            let body = format!("{title}\n{}", all.join("\n"));
            return Ok(fenced_reply(Some(&head), &body, None, max));
        }
        let outline = markdown::outline(&e.body);
        if outline.is_empty() {
            return Ok(fenced_reply(
                Some(&format!(
                    "{head}, over the {} budget; no headings ({} lines). Read a part with lines=\"1-80\".",
                    render::token_label(max),
                    all.len()
                )),
                &title,
                None,
                max,
            ));
        }
        Ok(fenced_reply(
            Some(&format!(
                "{head}, over the {} budget; outline below. Read one with section=<n>.",
                render::token_label(max)
            )),
            &format!("{title}\n{}", outline_text(&e.body, max - 400)),
            None,
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
                        quoted(&dup.title, 60),
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
                        outline_fenced(&cur.body, 1_200)
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
                format!("section \"{}\" appended", quoted(heading, 40))
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
                        outline_fenced(&fresh.body, 1_200)
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
        // ADR 0011: the folders line (names: stored text, in the fence) and
        // the commit line (Acta's instruction, outside it), both before the
        // memory titles, so the titles give way first.
        let lines = super::folders::briefing(
            self.store,
            scope,
            self.session.dir.as_deref(),
            self.session.in_acta,
        );
        let line_len = |l: &Option<String>| l.as_deref().map_or(0, |l| render::chars(l) + 1);
        // The profile (request 23): who Claude Code works for, fenced (a
        // name is stored text), never dropped; memory titles give way.
        let user = self.store.profile().ok().and_then(|p| {
            let name = p.name?;
            let mut s = format!("User: {}", render::one_line(&name));
            if let Some(j) = p.job_title.filter(|j| !j.trim().is_empty()) {
                s.push_str(&format!(", {}", render::one_line(&j)));
            }
            Some(render::clip(&s, 170))
        });
        let folders_len = line_len(&lines.folders) + line_len(&lines.commit) + line_len(&user);
        let brief = self
            .render_brief(
                scope,
                max - tail_len - head.len() - render::FENCE_COST - folders_len - 120,
            )
            .ok()?;
        // The tail is Acta's own text, after the fence: hold its room back.
        let mut out = Text::new(max - tail_len);
        out.line(head);
        if let Some(commit) = &lines.commit {
            out.line(commit);
        }
        // The whole body is stored text: one fence around it, every line
        // (memory titles included) defused by the fence.
        out.open_fence(&Fence::new());
        out.lines_of(&brief);
        if let Some(f) = &lines.folders {
            out.line(f);
        }
        if let Some(u) = &user {
            out.line(u);
        }
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
        let room = out.left();
        let mut titles = String::new();
        for m in &memories {
            let item = format!("{} {}", m.id, quoted(&m.title, 60));
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
        let mut text = out.finish();
        for t in tail {
            text.push('\n');
            text.push_str(&t);
        }
        Some(render::finish(text, max))
    }

    /// The `SubagentStart` note: at most 300 characters.
    pub fn subagent_note(&self) -> Option<String> {
        let project = self.session.project?;
        let p = self.store.project(project).ok()?;
        let mut parts = vec![format!("Acta: project {project} {}", quoted(&p.title, 40))];
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
            // Presence events are the server's (agent::presence); they
            // never put anything in the model's context.
            e if super::presence::Event::parse(e, a.detail.as_deref()).is_some() => {
                return Ok(String::new());
            }
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

/// One response: `head` (Acta's own words and ids, no stored text), then
/// `body` inside one data fence with a fresh nonce, then `foot` (paging).
/// Body lines that do not fit the budget are dropped at a line boundary.
fn fenced_reply(head: Option<&str>, body: &str, foot: Option<&str>, max: usize) -> String {
    let foot_len = foot.map_or(0, |f| render::chars(f) + 1);
    let mut out = Text::new(max.saturating_sub(foot_len));
    if let Some(h) = head {
        out.line_clipped(h);
    }
    out.open_fence(&Fence::new());
    out.lines_of(body);
    let mut s = out.finish();
    if let Some(f) = foot {
        s.push('\n');
        s.push_str(f);
    }
    s
}

/// A title, summary or heading quoted outside a fence: one line, markers
/// defused.
fn quoted(text: &str, max: usize) -> String {
    render::defuse(&render::clip(&render::one_line(text), max))
}

/// An epic a planned topic goes into: an existing one, or an epic item of
/// the same `create` call (by index).
pub(super) enum EpicRef {
    Existing(EpicId),
    Item(usize),
}

/// One validated `create` item.
pub(super) enum Plan {
    Topic {
        epic: Option<EpicRef>,
        title: String,
        description: String,
        status: Status,
        notes: Vec<NoteId>,
        due: Option<Date>,
        report_to: Option<PersonRef>,
    },
    Epic {
        title: String,
        description: String,
        notes: Vec<NoteId>,
        due: Option<Date>,
        report_to: Option<PersonRef>,
    },
    /// A note's text and its contact.
    Note(String, Option<PersonRef>),
    /// A question and the contact asked.
    Question(NewQuestion, Option<PersonRef>),
    /// A contact's name and level (data.md request 22).
    Contact(String, ContactLevel),
    /// Notes filed into an existing topic, each as a discussion entry.
    Entries { topic: TopicId, notes: Vec<Note> },
}

/// What filing did with the notes' contacts (request 19), for the ack, by
/// id only: `T-51 report to C-3; N-5 named C-7, not carried`, or for an
/// existing topic with its own, `T-12 keeps C-3; N-4 named C-7, not
/// carried`. `None` when no note named anyone.
fn carry_line(
    store: &Store,
    target: Record,
    before: Option<ContactId>,
    explicit: bool,
    notes: &[NoteId],
) -> crate::Result<Option<String>> {
    let named: Vec<(NoteId, ContactId)> = notes
        .iter()
        .filter_map(|n| {
            store
                .note(*n)
                .ok()
                .and_then(|x| x.report_to)
                .map(|c| (*n, c.id))
        })
        .collect();
    if named.is_empty() {
        return Ok(None);
    }
    let now = match target {
        Record::Topic(t) => store.topic(t)?.report.to.map(|c| c.id),
        Record::Epic(e) => store.epic(e)?.report.to.map(|c| c.id),
        _ => None,
    };
    let mut line = match (now, before.is_some() || explicit) {
        (Some(c), true) => format!("{target} keeps {c}"),
        (Some(c), false) => format!("{target} report to {c}"),
        (None, _) => target.to_string(),
    };
    let left: Vec<String> = named
        .iter()
        .filter(|(_, c)| Some(*c) != now)
        .map(|(n, c)| format!("{n} named {c}"))
        .collect();
    if !left.is_empty() {
        line.push_str(&format!("; {}, not carried", left.join(", ")));
    }
    Ok(Some(line))
}

/// Writes validated plans (inside one transaction, see `Agent::create`);
/// returns one ack and the notes it filed per item, and what filing did
/// with the notes' people.
type Applied = (Vec<(String, Vec<NoteId>)>, Vec<String>);

pub(super) fn apply_plans(
    store: &mut Store,
    project: ProjectId,
    plans: Vec<Plan>,
    dismissed: &[NoteId],
    author: Author,
    today: Date,
) -> crate::Result<Applied> {
    // Contacts first, so any item can name them by item number.
    let mut contacts: Vec<Option<ContactId>> = vec![None; plans.len()];
    for (i, plan) in plans.iter().enumerate() {
        if let Plan::Contact(name, level) = plan {
            contacts[i] = Some(store.create_contact(name, *level, author)?);
        }
    }
    let who = |r: Option<PersonRef>| -> Option<ContactId> {
        match r? {
            PersonRef::Existing(c) => Some(c),
            PersonRef::Item(k) => contacts[k],
        }
    };
    // Epics next, so topics can name them by item number.
    let mut epics: Vec<Option<EpicId>> = vec![None; plans.len()];
    for (i, plan) in plans.iter().enumerate() {
        if let Plan::Epic {
            title, description, ..
        } = plan
        {
            epics[i] = Some(store.create_epic(project, title, description, author)?);
        }
    }
    let mut acks = Vec::new();
    let mut carried = Vec::new();
    for (i, plan) in plans.into_iter().enumerate() {
        let (ack, notes) = match plan {
            Plan::Epic {
                notes,
                due,
                report_to,
                ..
            } => {
                let id = epics[i].expect("created above");
                if due.is_some() {
                    store.set_due(Owner::Epic(id), due, author)?;
                }
                let explicit = report_to.is_some();
                if explicit {
                    store.set_report_to(Record::Epic(id), who(report_to), author)?;
                }
                let moved = files_on(store, &notes)?;
                store.file_notes_in_epic(&notes, id, author)?;
                carried.extend(carry_line(store, Record::Epic(id), None, explicit, &notes)?);
                (format!("{id}{}", moved_ack(moved)), notes)
            }
            Plan::Topic {
                epic,
                title,
                description,
                status,
                notes,
                due,
                report_to,
            } => {
                let epic = match epic {
                    None => None,
                    Some(EpicRef::Existing(e)) => Some(e),
                    Some(EpicRef::Item(k)) => epics[k],
                };
                let id = store.create_topic(
                    NewTopic {
                        project,
                        epic,
                        title,
                        description,
                        status,
                    },
                    author,
                )?;
                if due.is_some() {
                    store.set_due(Owner::Topic(id), due, author)?;
                }
                let explicit = report_to.is_some();
                if explicit {
                    store.set_report_to(Record::Topic(id), who(report_to), author)?;
                }
                let moved = files_on(store, &notes)?;
                store.file_notes(&notes, id, author)?;
                carried.extend(carry_line(
                    store,
                    Record::Topic(id),
                    None,
                    explicit,
                    &notes,
                )?);
                let mut ack = id.to_string();
                if status == Status::Draft {
                    ack.push_str(" draft");
                }
                if let Some(e) = epic {
                    ack.push_str(&format!(" in {e}"));
                }
                ack.push_str(&moved_ack(moved));
                (ack, notes)
            }
            Plan::Note(text, report_to) => {
                let id = store.add_note(project, &text, author)?;
                if report_to.is_some() {
                    store.set_report_to(Record::Note(id), who(report_to), author)?;
                }
                (id.to_string(), Vec::new())
            }
            Plan::Contact(name, level) => {
                let id = contacts[i].expect("created above");
                (
                    // "created": the one word that says a contact is new.
                    format!(
                        "created {id} {} ({})",
                        render::clip(&render::one_line(&name), 40),
                        q::level_short(level)
                    ),
                    Vec::new(),
                )
            }
            Plan::Question(mut new, asked_ref) => {
                new.asked_to = who(asked_ref);
                let owner = new.owner;
                let asked = new.asked_to;
                let by = new.expected_on;
                let id = store.ask(new, author)?;
                let mut ack = format!("{id} ({owner})");
                // Whether the contact existed or this call created it, so
                // the agent never says it created someone Acta knew.
                if let Some(c) = asked {
                    let how = match asked_ref {
                        Some(PersonRef::Item(_)) => "created in this call",
                        _ => "existing contact",
                    };
                    ack.push_str(&format!(" · asked {c} ({how})"));
                }
                if by.is_some() {
                    ack.push_str(&format!(" · {}", q::by_ack(by, today)));
                }
                (ack, Vec::new())
            }
            Plan::Entries { topic, notes } => {
                let before = store.topic(topic)?.report.to.map(|c| c.id);
                let ids: Vec<NoteId> = notes.iter().map(|n| n.id).collect();
                let moved = files_on(store, &ids)?;
                // Each entry keeps its note's words, author and files.
                for n in &ids {
                    store.note_to_entry(*n, topic, author)?;
                }
                carried.extend(carry_line(
                    store,
                    Record::Topic(topic),
                    before,
                    false,
                    &ids,
                )?);
                (
                    format!(
                        "{topic} +{}{}",
                        plural(ids.len() as i64, "entry", "entries"),
                        moved_ack(moved)
                    ),
                    ids,
                )
            }
        };
        acks.push((ack, notes));
    }
    store.dismiss_notes(dismissed, author)?;
    Ok((acks, carried))
}

/// How many live files the notes hold (what filing them moves).
fn files_on(store: &Store, notes: &[NoteId]) -> crate::Result<usize> {
    let mut n = 0;
    for id in notes {
        n += store.attachments(AttachOwner::Note(*id))?.len();
    }
    Ok(n)
}

/// ` (2 files moved)`, or nothing.
fn moved_ack(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => " (1 file moved)".into(),
        n => format!(" ({n} files moved)"),
    }
}

/// One `find` hit: its line for the agent and its fields for `--json`.
#[derive(Default)]
pub(super) struct FindHit {
    pub(super) id: String,
    pub(super) kind: &'static str,
    pub(super) status: Option<&'static str>,
    pub(super) title: Option<String>,
    /// A note's text or a search excerpt.
    pub(super) text: Option<String>,
    /// Topics, epics and questions: `Some(None)` is "no date".
    pub(super) due: Option<Option<Date>>,
    pub(super) overdue: Option<bool>,
    pub(super) line: String,
    /// More fields for `--json` (a question's answer, a person).
    pub(super) extra: Vec<(&'static str, serde_json::Value)>,
}

/// `report_to` and `reported_at` for `--json`.
fn report_extra(r: &crate::model::Report) -> Vec<(&'static str, serde_json::Value)> {
    vec![
        ("report_to", r.to.as_ref().map(|c| c.name.clone()).into()),
        ("reported_at", r.at.into()),
    ]
}

impl FindHit {
    fn json(&self) -> serde_json::Value {
        let mut o = serde_json::Map::new();
        o.insert("id".into(), self.id.clone().into());
        o.insert("kind".into(), self.kind.into());
        if let Some(s) = self.status {
            o.insert("status".into(), s.into());
        }
        if let Some(t) = &self.title {
            o.insert("title".into(), t.clone().into());
        }
        if let Some(t) = &self.text {
            o.insert("text".into(), t.clone().into());
        }
        if let Some(d) = self.due {
            o.insert("due".into(), d.map(|d| d.to_string()).into());
            o.insert("overdue".into(), self.overdue.unwrap_or(false).into());
        }
        for (k, v) in &self.extra {
            o.insert((*k).into(), v.clone());
        }
        o.into()
    }
}

pub(super) struct FindPage {
    pub(super) hits: Vec<FindHit>,
    pub(super) offset: i64,
    pub(super) total: i64,
    /// One line said instead of hits (`no contact ...`): stored names.
    pub(super) note: Option<String>,
    /// Lines before the hits (`find(person=...)`: the contact's details
    /// and address note): stored text.
    pub(super) head: Vec<String>,
}

/// A topic line of `find`: the shortened due segment in lists, the full one
/// when the list is by date.
pub(super) fn topic_hit(t: &TopicSummary, today: Date, full_due: bool) -> FindHit {
    let open = t.status.is_open();
    let epic = t.epic.map_or(String::new(), |e| format!(" · {e}"));
    let seg = if full_due {
        due::segment(t.due_on, today, open)
    } else {
        due::short_segment(t.due_on, today, open)
    };
    FindHit {
        id: t.id.to_string(),
        kind: "topic",
        status: Some(t.status.as_str()),
        title: Some(t.title.clone()),
        text: None,
        due: Some(t.due_on),
        overdue: Some(due::is_overdue(t.due_on, today, open)),
        line: format!(
            "{} {}  {}{epic}{seg}{}",
            t.id,
            t.status.as_str(),
            render::clip(&render::one_line(&t.title), 80),
            q::topic_segment(t)
        ),
        extra: vec![
            (
                "report_to",
                t.report_to.as_ref().map(|c| c.name.clone()).into(),
            ),
            ("reported", t.reported.into()),
        ],
    }
}

pub(super) fn epic_hit(e: &EpicSummary, today: Date, full_due: bool) -> FindHit {
    let open = e.progress.is_open();
    let seg = if full_due {
        due::segment(e.due_on, today, open)
    } else {
        due::short_segment(e.due_on, today, open)
    };
    FindHit {
        id: e.id.to_string(),
        kind: "epic",
        status: None,
        title: Some(e.title.clone()),
        text: None,
        due: Some(e.due_on),
        overdue: Some(due::is_overdue(e.due_on, today, open)),
        line: format!(
            "{} epic  {} ({}/{} done){seg}{}",
            e.id,
            render::clip(&render::one_line(&e.title), 80),
            e.progress.done,
            e.progress.total,
            q::report_segment(e.report_to.as_ref(), q::epic_report_state(e))
        ),
        extra: vec![
            (
                "report_to",
                e.report_to.as_ref().map(|c| c.name.clone()).into(),
            ),
            ("reported", e.reported.into()),
        ],
    }
}

type FilesByOwner = std::collections::HashMap<AttachOwner, Vec<crate::model::Attachment>>;

/// Adds a record's `· files:` segment to its find line, and the files to
/// its `--json` fields.
fn with_files(hit: &mut FindHit, files: &FilesByOwner, owner: AttachOwner) {
    let Some(list) = files.get(&owner) else {
        return;
    };
    hit.line.push_str(&super::attach::files_segment(list, 2));
    let json: Vec<serde_json::Value> = list
        .iter()
        .map(|a| serde_json::json!({"id": a.id.to_string(), "name": a.name, "size": a.size}))
        .collect();
    hit.extra.push(("files", json.into()));
}

/// `N-3 N-5`.
fn id_list(notes: &[NoteId]) -> String {
    notes
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

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
                render::clip(&render::one_line(&s.title), 50),
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
pub(super) fn cut_body(
    head: &str,
    what: &str,
    title: &str,
    part: &[String],
    first: usize,
    total: usize,
    max: usize,
) -> String {
    const FOOT_ROOM: usize = 90;
    let mut out = Text::new(max - FOOT_ROOM);
    out.line_clipped(format!("{head} · {what}"));
    out.open_fence(&Fence::new());
    out.line_clipped(title);
    let mut shown = 0;
    for l in part {
        if !out.line(l) {
            break;
        }
        shown += 1;
    }
    let mut text = out.finish();
    if shown < part.len() {
        let next = first + shown;
        let last = first + part.len() - 1;
        text.push_str(&format!(
            "\n[cut at line {} of {total}: read on with lines=\"{next}-{last}\"]",
            next - 1
        ));
    }
    render::finish(text, max)
}

/// The outline of a body inside its own data fence (headings are stored
/// text), for refusals that show it.
fn outline_fenced(body: &str, max: usize) -> String {
    Fence::new().wrap(&outline_text(body, max))
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
