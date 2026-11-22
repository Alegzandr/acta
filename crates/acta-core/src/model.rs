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
id_type!(QuestionId, "Q");
id_type!(ContactId, "C");
id_type!(AttachmentId, "A");

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
    /// The Report to contact changed: old and new are contact UUIDs,
    /// absent for none (data.md requests 19 and 22).
    Report,
    /// Marked reported (new is the contact's UUID) or not reported (old
    /// is) (data.md request 19).
    Reported,
    /// Written by a newer build: shown as a plain system line.
    #[serde(other)]
    Unknown,
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
    Question(QuestionId),
    Contact(ContactId),
    Attachment(AttachmentId),
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
            Record::Question(id) => id.fmt(f),
            Record::Contact(id) => id.fmt(f),
            Record::Attachment(id) => id.fmt(f),
        }
    }
}

/// What a file can be attached to (ADR 0012, 1): a note, a timeline entry,
/// or the description of a topic or an epic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttachOwner {
    Note(NoteId),
    Entry(EntryId),
    /// The topic's description.
    Topic(TopicId),
    /// The epic's description.
    Epic(EpicId),
}

impl AttachOwner {
    /// The record whose trash state the attachment follows.
    pub fn record(self) -> Record {
        match self {
            AttachOwner::Note(id) => Record::Note(id),
            AttachOwner::Entry(id) => Record::Entry(id),
            AttachOwner::Topic(id) => Record::Topic(id),
            AttachOwner::Epic(id) => Record::Epic(id),
        }
    }
}

impl fmt::Display for AttachOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.record().fmt(f)
    }
}

/// How a file can be shown (ADR 0012, 6 and 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    /// PNG, JPEG, GIF, WebP or BMP: decoded in memory, thumbnail stored.
    Image,
    /// Valid UTF-8 with no NUL byte in the first 8 KB.
    Text,
    Pdf,
    /// Anything else: Save as only.
    Other,
}

/// A file attached to a note, an entry or a description, without its
/// content (read with [`crate::Store::read_attachment`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub id: AttachmentId,
    pub owner: AttachOwner,
    pub project: ProjectId,
    /// The file name as given, at most 255 chars.
    pub name: String,
    /// Bytes.
    pub size: i64,
    pub media_type: String,
    pub kind: FileKind,
    /// Pixel size of an image.
    pub pixels: Option<(u32, u32)>,
    /// Order in its owner's strip (the order files were added).
    pub position: i64,
    /// The note it came from when filing moved it (ADR 0012, 5).
    pub moved_from: Option<NoteId>,
    pub created_by: Author,
    pub stamp: Stamp,
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
    /// Someone vouched the text is still true (ADR 0015, 1); no new text.
    Verified,
    /// Replaced by a successor (ADR 0015, 2); the revision's `section`
    /// is empty, the entry's `superseded_by` names the successor.
    Superseded,
    /// No longer true, no successor; `section` holds the reason.
    Retired,
    /// Superseded or retired, then made live again.
    MadeLive,
    /// Another entry's body was merged into this one (ADR 0015, 3).
    Merged,
    /// Written by a newer build; shown as an edit.
    #[serde(other)]
    Unknown,
}

/// Whether a knowledge entry is current (ADR 0015, 2). Only live entries
/// appear in briefings, `kb` lists and `kb` search; the others stay
/// readable by id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KbState {
    #[default]
    Live,
    Superseded,
    Retired,
}

/// What a pair of entries was found to be (ADR 0015, 2 and 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KbPairKind {
    Duplicate,
    Conflict,
}

/// Where a found pair stands: still to look at, kept apart on purpose
/// ("keep both"), or settled by a supersede or a merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KbPairState {
    Open,
    Distinct,
    Resolved,
}

/// What the local quality log records (ADR 0015, 8): what history cannot
/// tell, warnings and their outcomes, stale-rev refusals and automatic
/// merges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KbLogEvent {
    DuplicateWarning,
    ConflictWarning,
    RelatedNote,
    /// A warning's outcome: the agent replaced the old entry.
    Superseded,
    /// A warning's outcome: the agent edited the old entry.
    Fixed,
    /// A warning's outcome: the identical call repeated ("keep both").
    KeptBoth,
    /// A warning's outcome: no write followed within the session.
    Abandoned,
    StaleRefused,
    AutoMerged,
    /// An agent read an entry that had a flag (metric 5).
    FlaggedRead,
    /// Written by a newer build.
    #[serde(other)]
    Unknown,
}

/// What a knowledge entry names as its source (ADR 0015, 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KbLinkKind {
    Topic,
    Entry,
    Question,
    Note,
    Kb,
    Commit,
    File,
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
    pub report: Report,
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
    pub report: Report,
    pub stamp: Stamp,
}

/// Who a topic or an epic must be reported back to, and whether it was
/// (data.md request 19). Default: no one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The contact (data.md request 22).
    pub to: Option<ContactRef>,
    /// When it was marked reported; reset when the person changes.
    pub at: Option<i64>,
    pub by: Option<Author>,
}

impl Report {
    pub fn is_reported(&self) -> bool {
        self.to.is_some() && self.at.is_some()
    }
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
    /// The Report to contact, if any (requests 19 and 22).
    pub report_to: Option<ContactRef>,
    pub reported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpicSummary {
    pub id: EpicId,
    pub title: String,
    pub position: i64,
    pub progress: Progress,
    pub due_on: Option<Date>,
    pub updated_at: i64,
    pub report_to: Option<ContactRef>,
    pub reported: bool,
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
    /// Who it must be reported back to (request 19); filing carries it.
    pub report_to: Option<ContactRef>,
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

/// A question's state (components-questions.md 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestionStatus {
    Open,
    Answered,
    Dropped,
}

/// A question to someone outside the work, answered later (data.md
/// request 18). Owned by exactly one project, epic or topic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub id: QuestionId,
    pub project: ProjectId,
    pub owner: Owner,
    /// 1 to 300 chars, one paragraph.
    pub text: String,
    /// The contact asked (requests 19 and 22).
    pub asked_to: Option<ContactRef>,
    /// The day the answer is expected.
    pub expected_on: Option<Date>,
    pub status: QuestionStatus,
    /// Markdown, at most 1,500 chars; kept when reopened.
    pub answer: Option<String>,
    /// When and by whom it was answered or dropped.
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<Author>,
    /// The Decision entry logged with the answer.
    pub decision_entry: Option<EntryId>,
    /// Stored order among the owner's questions.
    pub position: i64,
    pub created_by: Author,
    pub stamp: Stamp,
}

impl Question {
    pub fn is_open(&self) -> bool {
        self.status == QuestionStatus::Open
    }
}

/// A reference to a contact as a record shows it: the name drawn now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactRef {
    pub id: ContactId,
    pub name: String,
    /// The contact is in the trash: still named, never offered.
    pub in_trash: bool,
}

/// Where a contact lives (data.md request 22): all projects, a project,
/// or an epic of a project. It is visible there and below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContactLevel {
    All,
    Project(ProjectId),
    Epic(ProjectId, EpicId),
}

impl ContactLevel {
    pub fn project(self) -> Option<ProjectId> {
        match self {
            ContactLevel::All => None,
            ContactLevel::Project(p) | ContactLevel::Epic(p, _) => Some(p),
        }
    }

    pub fn epic(self) -> Option<EpicId> {
        match self {
            ContactLevel::Epic(_, e) => Some(e),
            _ => None,
        }
    }

    /// 0 for an epic, 1 for a project, 2 for all projects: nearest first.
    pub fn width(self) -> u8 {
        match self {
            ContactLevel::Epic(..) => 0,
            ContactLevel::Project(_) => 1,
            ContactLevel::All => 2,
        }
    }

    /// The narrowest level that sees everything both levels see.
    pub fn join(self, other: ContactLevel) -> ContactLevel {
        use ContactLevel::*;
        if self == other {
            return self;
        }
        match (self.project(), other.project()) {
            (Some(p), Some(q)) if p == q => Project(p),
            _ => All,
        }
    }

    /// The narrowest level that sees `place`: its epic, else its project.
    pub fn of_place(place: Place) -> ContactLevel {
        match place.epic {
            Some(e) => ContactLevel::Epic(place.project, e),
            None => ContactLevel::Project(place.project),
        }
    }
}

impl fmt::Display for ContactLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContactLevel::All => f.write_str("all"),
            ContactLevel::Project(p) => p.fmt(f),
            ContactLevel::Epic(_, e) => e.fmt(f),
        }
    }
}

/// Where a record sits, for contact visibility: its project, and the epic
/// it is in or is (a topic's epic, the epic itself; none for a note or a
/// project).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub project: ProjectId,
    pub epic: Option<EpicId>,
}

/// Whether a contact at `level` can be named from `place` (data.md
/// request 22): it is global, or in the place's project with no epic, or
/// in the place's epic. One pure function for the picker, the store and
/// the agent.
pub fn visible(level: ContactLevel, place: Place) -> bool {
    match level {
        ContactLevel::All => true,
        ContactLevel::Project(p) => p == place.project,
        ContactLevel::Epic(p, e) => p == place.project && place.epic == Some(e),
    }
}

/// A person work is asked of or reported to (data.md request 22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contact {
    pub id: ContactId,
    /// 1 to 80 chars, cleaned by [`crate::person::clean`].
    pub name: String,
    pub job_title: Option<String>,
    pub company: Option<String>,
    /// How to address them: plain text.
    pub address_note: Option<String>,
    pub level: ContactLevel,
    pub created_by: Author,
    /// The contact this one was merged into (its trash row).
    pub merged_into: Option<ContactId>,
    pub stamp: Stamp,
}

/// One place a contact is named: a live record and the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactUse {
    pub record: Record,
    pub place: Place,
    /// The record's title, or a question's text.
    pub title: String,
    /// `report_to` or `asked`.
    pub field: &'static str,
}

/// A contact lifted to a wider level so a moved record still sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lift {
    pub contact: ContactId,
    pub from: ContactLevel,
    pub to: ContactLevel,
    /// The moved record that needed it.
    pub record: Record,
}

/// The human's own name and job title (data.md request 23).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub name: Option<String>,
    pub job_title: Option<String>,
}

#[cfg(test)]
mod contact_tests {
    use super::*;

    #[test]
    fn visibility_follows_the_level_and_below() {
        let (p, q) = (ProjectId(1), ProjectId(2));
        let (e, f) = (EpicId(1), EpicId(2));
        let topic_in_e = Place {
            project: p,
            epic: Some(e),
        };
        let loose = Place {
            project: p,
            epic: None,
        };
        let other = Place {
            project: q,
            epic: None,
        };
        let table = [
            (ContactLevel::All, topic_in_e, true),
            (ContactLevel::All, other, true),
            (ContactLevel::Project(p), topic_in_e, true),
            (ContactLevel::Project(p), loose, true),
            (ContactLevel::Project(p), other, false),
            (ContactLevel::Epic(p, e), topic_in_e, true),
            (ContactLevel::Epic(p, e), loose, false),
            (ContactLevel::Epic(p, f), topic_in_e, false),
            (ContactLevel::Epic(p, e), other, false),
        ];
        for (level, place, expected) in table {
            assert_eq!(visible(level, place), expected, "{level} from {place:?}");
        }
        assert_eq!(
            ContactLevel::Epic(p, e).join(ContactLevel::Epic(p, f)),
            ContactLevel::Project(p)
        );
        assert_eq!(
            ContactLevel::Epic(p, e).join(ContactLevel::Project(q)),
            ContactLevel::All
        );
        assert_eq!(
            ContactLevel::Epic(p, e).join(ContactLevel::Epic(p, e)),
            ContactLevel::Epic(p, e)
        );
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
    /// When someone last asserted the text is true (ADR 0015, 1).
    pub verified_at: i64,
    pub state: KbState,
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
    /// ADR 0015, 1: distinct from `stamp.updated_at`; a move or a title
    /// change does not set it.
    pub verified_at: i64,
    pub verified_by: Author,
    pub state: KbState,
    pub superseded_by: Option<KbId>,
    /// ADR 0015, 4: a branch memory is true only on this branch of this
    /// folder.
    pub context_branch: Option<String>,
    pub context_folder: Option<FolderId>,
    /// ADR 0015, 5: the Claude Code session and focus topic that created
    /// it (agent writes only).
    pub created_session: Option<String>,
    pub created_topic: Option<TopicId>,
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
