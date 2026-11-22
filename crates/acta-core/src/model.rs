//! Domain types. Plain data: no storage handle, no UI type.
//!
//! Ids are the per-installation integer keys of the database (ADR 0003); the
//! short ids shown to agents (`T-42`) are these numbers with a prefix. The
//! identity that survives export and import is the UUIDv7 (ADR 0006).

use crate::date::Date;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub i64);

        impl $name {
            /// Prefix of the short id shown to agents and in the UI.
            pub const PREFIX: &'static str = $prefix;

            /// Parses `T-42` (any case) or a bare `42`.
            pub fn parse(text: &str) -> Option<Self> {
                let t = text.trim();
                let n = match t.split_once('-') {
                    Some((p, n)) if p.eq_ignore_ascii_case($prefix) => n,
                    Some(_) => return None,
                    None => t,
                };
                n.parse().ok().filter(|n| *n > 0).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}-{}", $prefix, self.0)
            }
        }
    };
}

id_type!(ProjectId, "P");
id_type!(EpicId, "E");
id_type!(TopicId, "T");
id_type!(EntryId, "TE");
id_type!(NoteId, "N");
id_type!(KbId, "K");
id_type!(FolderId, "F");

/// Who made a write. Over MCP, and when `CLAUDECODE=1`, it is Claude Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Author {
    Human,
    Claude,
}

/// Topic statuses (docs/design/color.md 2.3). Four open, two closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Planned,
    Active,
    Blocked,
    Done,
    Dropped,
}

impl Status {
    pub const ALL: [Status; 6] = [
        Status::Draft,
        Status::Planned,
        Status::Active,
        Status::Blocked,
        Status::Done,
        Status::Dropped,
    ];

    pub fn is_open(self) -> bool {
        !matches!(self, Status::Done | Status::Dropped)
    }
}

/// Kind of a timeline entry. `System` entries are written by Acta itself
/// (creation, status changes, moves, state changes) and carry a [`SystemEvent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Progress,
    Decision,
    Discussion,
    System,
}

/// What a system entry records (data model request 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    /// The topic was created. No values.
    Created,
    /// Status change: old and new are status words.
    Status,
    /// Moved between epics: old and new are epic UUIDs, absent for "no epic".
    Moved,
    /// The current state was replaced: old holds the previous state, so the
    /// history of the state is kept (ADR 0005, `update`).
    State,
    /// Restored from the trash.
    Restored,
    /// An entry was edited in place: old holds its previous type and text
    /// (`{"kind":"progress","text":"..."}`), new the entry's number, so
    /// no wording is lost.
    Edited,
    /// The due date changed: old and new are `YYYY-MM-DD`, absent for no
    /// date (data.md request 16).
    Due,
}

/// Level a knowledge entry or a folder is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Project,
    Epic,
    Topic,
}

/// A project, epic or topic: what owns knowledge and folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    Project(ProjectId),
    Epic(EpicId),
    Topic(TopicId),
}

impl Owner {
    pub fn level(self) -> Level {
        match self {
            Owner::Project(_) => Level::Project,
            Owner::Epic(_) => Level::Epic,
            Owner::Topic(_) => Level::Topic,
        }
    }
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Owner::Project(id) => id.fmt(f),
            Owner::Epic(id) => id.fmt(f),
            Owner::Topic(id) => id.fmt(f),
        }
    }
}

/// Any record that can be trashed and restored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Record {
    Project(ProjectId),
    Epic(EpicId),
    Topic(TopicId),
    Entry(EntryId),
    Note(NoteId),
    Kb(KbId),
    Folder(FolderId),
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Record::Project(id) => id.fmt(f),
            Record::Epic(id) => id.fmt(f),
            Record::Topic(id) => id.fmt(f),
            Record::Entry(id) => id.fmt(f),
            Record::Note(id) => id.fmt(f),
            Record::Kb(id) => id.fmt(f),
            Record::Folder(id) => id.fmt(f),
        }
    }
}

/// Document or memory (ADR 0007): the same entity, different defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KbKind {
    Document,
    Memory,
}

/// What a knowledge revision did (data model request 12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Created,
    CreatedFromFile,
    Edited,
    ReplacedSection,
    AppendedSection,
    Moved,
    Restored,
}

/// Fields every record carries (ADR 0003, 0006; requests 9 and 11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub uuid: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub updated_by: Author,
    pub deleted_at: Option<i64>,
    pub deleted_by: Option<Author>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub description: String,
    pub position: i64,
    /// The folder this page chose as active (ADR 0011); `None` follows the
    /// default rule. See [`crate::store::VisibleFolders`].
    pub active_folder: Option<FolderId>,
    pub due_on: Option<Date>,
    pub stamp: Stamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Epic {
    pub id: EpicId,
    pub project: ProjectId,
    pub title: String,
    pub description: String,
    pub state: String,
    pub position: i64,
    pub active_folder: Option<FolderId>,
    pub due_on: Option<Date>,
    pub stamp: Stamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topic {
    pub id: TopicId,
    pub project: ProjectId,
    pub epic: Option<EpicId>,
    pub title: String,
    pub description: String,
    pub status: Status,
    /// Current state: a few lines, overwritten (ADR 0007).
    pub state: String,
    pub state_updated_at: Option<i64>,
    pub state_updated_by: Option<Author>,
    pub position: i64,
    pub active_folder: Option<FolderId>,
    pub due_on: Option<Date>,
    pub stamp: Stamp,
}

/// One line of a list: no long text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicSummary {
    pub id: TopicId,
    pub epic: Option<EpicId>,
    pub title: String,
    pub status: Status,
    pub position: i64,
    /// Count of timeline entries (system events excluded).
    pub entries: i64,
    pub due_on: Option<Date>,
    pub updated_at: i64,
    pub updated_by: Author,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpicSummary {
    pub id: EpicId,
    pub title: String,
    pub position: i64,
    pub progress: Progress,
    pub due_on: Option<Date>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSummary {
    pub id: ProjectId,
    pub title: String,
    pub position: i64,
    pub progress: Progress,
    /// Notes not yet filed.
    pub inbox: i64,
    /// Knowledge entries at every level of the project.
    pub knowledge: i64,
    pub due_on: Option<Date>,
    pub updated_at: i64,
}

/// Topics done out of topics not dropped (docs/design/data.md 11). Computed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: i64,
    pub total: i64,
}

impl Progress {
    /// An epic or a project is open for due dates while its progress is
    /// below 100% or it has no topic (components-records.md 5).
    pub fn is_open(self) -> bool {
        self.total == 0 || self.done < self.total
    }
}

/// The sidebar tree of one project: loose topics first, then epics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectTree {
    pub project: ProjectSummary,
    pub loose_topics: Vec<TopicSummary>,
    pub epics: Vec<(EpicSummary, Vec<TopicSummary>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemEvent {
    pub kind: EventKind,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: EntryId,
    pub topic: TopicId,
    /// Number of the entry in its topic (`T-42#14`); system events have none.
    pub seq: Option<i64>,
    pub kind: EntryKind,
    pub author: Author,
    pub text: String,
    pub event: Option<SystemEvent>,
    pub stamp: Stamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: NoteId,
    pub project: ProjectId,
    pub text: String,
    pub author: Author,
    /// The topic the note was turned into, or added to as an entry.
    pub filed_topic: Option<TopicId>,
    /// The epic the note was turned into.
    pub filed_epic: Option<EpicId>,
    /// Set once the note has left the inbox: filed, or dismissed when
    /// neither `filed_topic` nor `filed_epic` is set.
    pub filed_at: Option<i64>,
    /// Who filed or dismissed it; `None` for notes filed before it was kept.
    pub filed_by: Option<Author>,
    pub stamp: Stamp,
}

/// Where a note went when it left the inbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteFate {
    Open,
    Topic(TopicId),
    Epic(EpicId),
    Dismissed,
}

impl Note {
    pub fn fate(&self) -> NoteFate {
        // `filed_at` decides: a target without it (an old import) is open.
        match (self.filed_at, self.filed_topic, self.filed_epic) {
            (None, _, _) => NoteFate::Open,
            (Some(_), Some(t), _) => NoteFate::Topic(t),
            (Some(_), None, Some(e)) => NoteFate::Epic(e),
            (Some(_), None, None) => NoteFate::Dismissed,
        }
    }

    pub fn is_open(&self) -> bool {
        self.fate() == NoteFate::Open
    }
}

/// A knowledge entry without its body: what lists show (ADR 0005 `kb`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KbSummary {
    pub id: KbId,
    pub kind: KbKind,
    pub owner: Owner,
    pub title: String,
    pub summary: String,
    pub rev: i64,
    /// Body length in bytes; an agent-facing size is about a quarter of it in tokens.
    pub size: i64,
    pub created_by: Author,
    pub updated_at: i64,
    pub updated_by: Author,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KbEntry {
    pub id: KbId,
    pub kind: KbKind,
    pub owner: Owner,
    pub project: ProjectId,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub rev: i64,
    pub created_by: Author,
    pub stamp: Stamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KbRevision {
    pub uuid: String,
    pub rev: i64,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub author: Author,
    pub change: ChangeKind,
    pub section: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub id: FolderId,
    pub owner: Owner,
    pub project: ProjectId,
    /// Absolute canonical path, shown as stored (ADR 0008).
    pub path: String,
    pub position: i64,
    /// The name the user gave; empty means the last segment of the path,
    /// so it follows a Locate or a move ([`Folder::display_name`]).
    pub name: String,
    /// One of [`FOLDER_ICONS`], or empty for automatic (ADR 0011).
    pub icon: String,
    pub commit_mode: CommitMode,
    /// "Commit in my name, without Co-Authored-By Claude"; `None` follows
    /// the Settings default.
    pub commit_as_user: Option<bool>,
    /// "Also on <default branch>".
    pub commit_on_default: bool,
    pub stamp: Stamp,
}

impl Folder {
    /// The name shown: the user's, else the last segment of the path.
    pub fn display_name(&self) -> &str {
        if !self.name.is_empty() {
            return &self.name;
        }
        let trimmed = self.path.trim_end_matches(['/', '\\']);
        trimmed
            .rsplit(['/', '\\'])
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.path)
    }
}

/// The icons a folder may be given besides automatic (ADR 0011, 1).
pub const FOLDER_ICONS: [&str; 12] = [
    "folder-open",
    "folder-git-2",
    "git-branch",
    "code",
    "book-open",
    "package",
    "server",
    "globe",
    "smartphone",
    "database",
    "flask-conical",
    "palette",
];

/// What Claude Code may do with git in a folder (ADR 0011, 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitMode {
    /// Commits only when the user asks (Claude Code's own rule).
    #[default]
    Manual,
    /// Commit automatically.
    Commit,
    /// Commit and push automatically.
    Push,
}

/// A page of a list: summaries first, then a cursor (ADR 0005).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Count of all matching items, not only this page.
    pub total: i64,
    pub offset: i64,
    /// `true` when `total` stopped at a cap (search ranks at most 500 hits).
    pub capped: bool,
}

impl<T> Page<T> {
    /// Offset of the next page, if there is one.
    pub fn next(&self) -> Option<i64> {
        let end = self.offset + self.items.len() as i64;
        (end < self.total).then_some(end)
    }
}

/// Which slice of a list to return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageReq {
    pub offset: i64,
    pub limit: i64,
}

impl PageReq {
    pub const MAX_LIMIT: i64 = 200;

    pub fn first(limit: i64) -> Self {
        Self { offset: 0, limit }
    }

    pub(crate) fn clamped(self) -> (i64, i64) {
        (self.offset.max(0), self.limit.clamp(1, Self::MAX_LIMIT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_ids_parse_and_print() {
        assert_eq!(TopicId::parse("T-42"), Some(TopicId(42)));
        assert_eq!(TopicId::parse("t-42"), Some(TopicId(42)));
        assert_eq!(TopicId::parse("42"), Some(TopicId(42)));
        assert_eq!(TopicId::parse("E-42"), None);
        assert_eq!(TopicId::parse("T-0"), None);
        assert_eq!(KbId(7).to_string(), "K-7");
        assert_eq!(Owner::Epic(EpicId(3)).to_string(), "E-3");
    }

    #[test]
    fn pages_know_their_next_offset() {
        let page = Page {
            items: vec![1, 2],
            total: 5,
            offset: 2,
            capped: false,
        };
        assert_eq!(page.next(), Some(4));
        let last = Page {
            items: vec![5],
            total: 5,
            offset: 4,
            capped: false,
        };
        assert_eq!(last.next(), None);
    }

    #[test]
    fn closed_statuses() {
        let open: Vec<_> = Status::ALL.into_iter().filter(|s| s.is_open()).collect();
        assert_eq!(open.len(), 4);
    }
}
