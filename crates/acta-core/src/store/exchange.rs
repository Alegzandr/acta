//! Export and import of the open `.acta.json` format (ADR 0006).
//!
//! One JSON document with every record of the database, or of one project,
//! trashed records included and flagged by `deleted_at`. Records refer to
//! each other by UUID; local ids and short ids are never exported. Arrays are
//! sorted by UUID so the same data always gives the same file.
//!
//! Import is a merge keyed on UUID, in one transaction: unknown records are
//! created, known records are updated only when the file's `updated_at` is
//! newer, and nothing is ever deleted: a newer version can restore a trashed
//! record but never trashes a live one. Importing the same file twice changes
//! nothing. A dry run reports the counts and rolls back.
//!
//! Folder paths are machine-specific; they are exported marked `local` and
//! imported as they are, shown as not found when they do not exist here.

use super::Store;
use crate::error::{Error, Result};
use crate::model::{Author, ChangeKind, EntryKind, EventKind, KbKind, ProjectId, Status};
use crate::snapshots::SnapshotKind;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

/// The format name written in every file.
pub const FORMAT_NAME: &str = "acta";
/// The format version this build writes and the newest it reads.
pub const FORMAT_VERSION: i64 = 1;
/// The file extension, without the leading dot.
pub const FILE_EXTENSION: &str = "acta.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportFile {
    pub format: String,
    pub version: i64,
    pub exported_at: i64,
    /// The UUID of the exported project, or `None` for everything.
    pub project: Option<String>,
    pub projects: Vec<XProject>,
    pub epics: Vec<XEpic>,
    pub topics: Vec<XTopic>,
    pub entries: Vec<XEntry>,
    pub notes: Vec<XNote>,
    pub knowledge: Vec<XKb>,
    pub folders: Vec<XFolder>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XStamp {
    pub uuid: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub updated_by: Author,
    pub deleted_at: Option<i64>,
    pub deleted_by: Option<Author>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XProject {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub title: String,
    pub description: String,
    pub position: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XEpic {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: String,
    pub title: String,
    pub description: String,
    pub state: String,
    pub position: i64,
    pub include_inherited_folders: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XTopic {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: String,
    pub epic: Option<String>,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub state: String,
    pub state_updated_at: Option<i64>,
    pub state_updated_by: Option<Author>,
    pub position: i64,
    pub include_inherited_folders: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XEntry {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub topic: String,
    pub seq: Option<i64>,
    pub kind: EntryKind,
    pub author: Author,
    pub text: String,
    pub event: Option<EventKind>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XNote {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: String,
    pub text: String,
    pub author: Author,
    pub filed_topic: Option<String>,
    pub filed_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XKb {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: String,
    pub epic: Option<String>,
    pub topic: Option<String>,
    pub kind: KbKind,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub rev: i64,
    pub created_by: Author,
    pub revisions: Vec<XRevision>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XRevision {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XFolder {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: String,
    pub epic: Option<String>,
    pub topic: Option<String>,
    pub path: String,
    pub position: i64,
    /// Always true: a path only means something on the machine it came from.
    pub local: bool,
}

impl ExportFile {
    pub fn to_json(&self) -> Result<String> {
        let mut s = serde_json::to_string_pretty(self)?;
        s.push('\n');
        Ok(s)
    }

    /// Parses a file, refusing a newer format with a clear error.
    pub fn from_json(text: &str) -> Result<ExportFile> {
        #[derive(Deserialize)]
        struct Header {
            format: String,
            version: i64,
        }
        let header: Header = serde_json::from_str(text)?;
        if header.format != FORMAT_NAME {
            return Err(Error::Invalid("this is not an Acta export".into()));
        }
        if header.version > FORMAT_VERSION {
            return Err(Error::FormatTooNew {
                found: header.version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(serde_json::from_str(text)?)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportCounts {
    pub created: i64,
    pub updated: i64,
    pub skipped: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub projects: ImportCounts,
    pub epics: ImportCounts,
    pub topics: ImportCounts,
    pub entries: ImportCounts,
    pub notes: ImportCounts,
    pub knowledge: ImportCounts,
    pub revisions: ImportCounts,
    pub folders: ImportCounts,
}

impl ImportReport {
    pub fn total(&self) -> ImportCounts {
        let all = [
            self.projects,
            self.epics,
            self.topics,
            self.entries,
            self.notes,
            self.knowledge,
            self.revisions,
            self.folders,
        ];
        all.iter()
            .fold(ImportCounts::default(), |a, c| ImportCounts {
                created: a.created + c.created,
                updated: a.updated + c.updated,
                skipped: a.skipped + c.skipped,
            })
    }
}

const STAMP_SQL: &str =
    "x.uuid, x.created_at, x.updated_at, x.updated_by, x.deleted_at, x.deleted_by";

fn xstamp(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<XStamp> {
    Ok(XStamp {
        uuid: r.get(at)?,
        created_at: r.get(at + 1)?,
        updated_at: r.get(at + 2)?,
        updated_by: r.get(at + 3)?,
        deleted_at: r.get(at + 4)?,
        deleted_by: r.get(at + 5)?,
    })
}

fn collect<T>(
    conn: &rusqlite::Connection,
    sql: &str,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

impl Store {
    /// Everything, or one project (trash included), as an export document.
    pub fn export(&self, project: Option<ProjectId>) -> Result<ExportFile> {
        let project_uuid = match project {
            Some(p) => Some(
                self.conn
                    .query_row("SELECT uuid FROM projects WHERE id = ?1", [p.0], |r| {
                        r.get(0)
                    })
                    .optional()?
                    .ok_or_else(|| Error::NotFound(p.to_string()))?,
            ),
            None => None,
        };
        let only = |col: &str| project.map_or(String::new(), |p| format!(" AND {col} = {}", p.0));
        let conn = &self.conn;
        let projects = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, x.title, x.description, x.position FROM projects x WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.id")
            ),
            |r| {
                Ok(XProject {
                    stamp: xstamp(r, 0)?,
                    title: r.get(6)?,
                    description: r.get(7)?,
                    position: r.get(8)?,
                })
            },
        )?;
        let epics = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, x.title, x.description, x.state, x.position, x.include_inherited_folders
                 FROM epics x JOIN projects p ON p.id = x.project_id WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.project_id")
            ),
            |r| {
                Ok(XEpic {
                    stamp: xstamp(r, 0)?,
                    project: r.get(6)?,
                    title: r.get(7)?,
                    description: r.get(8)?,
                    state: r.get(9)?,
                    position: r.get(10)?,
                    include_inherited_folders: r.get(11)?,
                })
            },
        )?;
        let topics = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, x.title, x.description, x.status, x.state,
                        x.state_updated_at, x.state_updated_by, x.position, x.include_inherited_folders
                 FROM topics x JOIN projects p ON p.id = x.project_id LEFT JOIN epics e ON e.id = x.epic_id
                 WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.project_id")
            ),
            |r| {
                Ok(XTopic {
                    stamp: xstamp(r, 0)?,
                    project: r.get(6)?,
                    epic: r.get(7)?,
                    title: r.get(8)?,
                    description: r.get(9)?,
                    status: r.get(10)?,
                    state: r.get(11)?,
                    state_updated_at: r.get(12)?,
                    state_updated_by: r.get(13)?,
                    position: r.get(14)?,
                    include_inherited_folders: r.get(15)?,
                })
            },
        )?;
        let entries = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, t.uuid, x.seq, x.kind, x.author, x.text, x.event, x.old_value, x.new_value
                 FROM entries x JOIN topics t ON t.id = x.topic_id WHERE 1 = 1{} ORDER BY x.uuid",
                only("t.project_id")
            ),
            |r| {
                Ok(XEntry {
                    stamp: xstamp(r, 0)?,
                    topic: r.get(6)?,
                    seq: r.get(7)?,
                    kind: r.get(8)?,
                    author: r.get(9)?,
                    text: r.get(10)?,
                    event: r.get(11)?,
                    old_value: r.get(12)?,
                    new_value: r.get(13)?,
                })
            },
        )?;
        let notes = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, x.text, x.author, t.uuid, x.filed_at
                 FROM notes x JOIN projects p ON p.id = x.project_id LEFT JOIN topics t ON t.id = x.filed_topic_id
                 WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.project_id")
            ),
            |r| {
                Ok(XNote {
                    stamp: xstamp(r, 0)?,
                    project: r.get(6)?,
                    text: r.get(7)?,
                    author: r.get(8)?,
                    filed_topic: r.get(9)?,
                    filed_at: r.get(10)?,
                })
            },
        )?;
        let mut knowledge = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, t.uuid, x.kind, x.title, x.summary, x.body, x.rev, x.created_by, x.id
                 FROM kb_entries x JOIN projects p ON p.id = x.project_id
                 LEFT JOIN epics e ON e.id = x.epic_id LEFT JOIN topics t ON t.id = x.topic_id
                 WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.project_id")
            ),
            |r| {
                Ok((
                    XKb {
                        stamp: xstamp(r, 0)?,
                        project: r.get(6)?,
                        epic: r.get(7)?,
                        topic: r.get(8)?,
                        kind: r.get(9)?,
                        title: r.get(10)?,
                        summary: r.get(11)?,
                        body: r.get(12)?,
                        rev: r.get(13)?,
                        created_by: r.get(14)?,
                        revisions: Vec::new(),
                    },
                    r.get::<_, i64>(15)?,
                ))
            },
        )?;
        for (kb, id) in &mut knowledge {
            kb.revisions = collect(
                conn,
                &format!(
                    "SELECT uuid, rev, title, summary, body, author, change, section, created_at
                     FROM kb_revisions WHERE entry_id = {id} ORDER BY rev, uuid"
                ),
                |r| {
                    Ok(XRevision {
                        uuid: r.get(0)?,
                        rev: r.get(1)?,
                        title: r.get(2)?,
                        summary: r.get(3)?,
                        body: r.get(4)?,
                        author: r.get(5)?,
                        change: r.get(6)?,
                        section: r.get(7)?,
                        created_at: r.get(8)?,
                    })
                },
            )?;
        }
        let folders = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, t.uuid, x.path, x.position
                 FROM folders x JOIN projects p ON p.id = x.project_id
                 LEFT JOIN epics e ON e.id = x.epic_id LEFT JOIN topics t ON t.id = x.topic_id
                 WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.project_id")
            ),
            |r| {
                Ok(XFolder {
                    stamp: xstamp(r, 0)?,
                    project: r.get(6)?,
                    epic: r.get(7)?,
                    topic: r.get(8)?,
                    path: r.get(9)?,
                    position: r.get(10)?,
                    local: true,
                })
            },
        )?;
        Ok(ExportFile {
            format: FORMAT_NAME.into(),
            version: FORMAT_VERSION,
            exported_at: crate::time::now_ms(),
            project: project_uuid,
            projects,
            epics,
            topics,
            entries,
            notes,
            knowledge: knowledge.into_iter().map(|(k, _)| k).collect(),
            folders,
        })
    }

    /// Merges a file into the database. With `dry_run` nothing is kept and
    /// the report says what would happen; otherwise an `import` snapshot is
    /// taken first.
    pub fn import(&mut self, file: &ExportFile, dry_run: bool) -> Result<ImportReport> {
        if file.version > FORMAT_VERSION {
            return Err(Error::FormatTooNew {
                found: file.version,
                supported: FORMAT_VERSION,
            });
        }
        if !dry_run {
            self.check_writable()?;
            self.snapshot(SnapshotKind::Import)?;
        }
        let tx = self.write()?;
        let report = merge(&tx, file)?;
        if dry_run {
            tx.rollback()?;
        } else {
            tx.commit()?;
            self.refresh_link_index();
        }
        Ok(report)
    }
}

/// What a known record looks like locally.
struct Local {
    id: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
}

enum Action {
    Insert,
    Update(Local),
    Skip,
}

fn decide(tx: &Transaction<'_>, table: &str, stamp: &XStamp) -> Result<Action> {
    let local = tx
        .query_row(
            &format!("SELECT id, updated_at, deleted_at FROM {table} WHERE uuid = ?1"),
            [&stamp.uuid],
            |r| {
                Ok(Local {
                    id: r.get(0)?,
                    updated_at: r.get(1)?,
                    deleted_at: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(match local {
        None => Action::Insert,
        Some(l) if stamp.updated_at > l.updated_at => Action::Update(l),
        Some(_) => Action::Skip,
    })
}

/// The trash state after a merge: an import never trashes a live record.
fn merged_deleted(local: &Local, stamp: &XStamp) -> (Option<i64>, Option<Author>) {
    if local.deleted_at.is_none() {
        (None, None)
    } else {
        (stamp.deleted_at, stamp.deleted_by)
    }
}

fn local_id(tx: &Transaction<'_>, table: &str, uuid: &str) -> Result<i64> {
    tx.query_row(
        &format!("SELECT id FROM {table} WHERE uuid = ?1"),
        [uuid],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| {
        Error::Invalid(format!(
            "the file refers to a missing record {uuid} in {table}"
        ))
    })
}

fn opt_id(tx: &Transaction<'_>, table: &str, uuid: &Option<String>) -> Result<Option<i64>> {
    uuid.as_deref().map(|u| local_id(tx, table, u)).transpose()
}

fn count(c: &mut ImportCounts, action: &Action) {
    match action {
        Action::Insert => c.created += 1,
        Action::Update(_) => c.updated += 1,
        Action::Skip => c.skipped += 1,
    }
}

fn merge(tx: &Transaction<'_>, f: &ExportFile) -> Result<ImportReport> {
    let mut rep = ImportReport::default();

    for x in &f.projects {
        let s = &x.stamp;
        let action = decide(tx, "projects", s)?;
        count(&mut rep.projects, &action);
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO projects (uuid, title, description, position, created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![s.uuid, x.title, x.description, x.position, s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE projects SET title = ?2, description = ?3, position = ?4, updated_at = ?5, updated_by = ?6,
                            deleted_at = ?7, deleted_by = ?8 WHERE id = ?1",
                    params![l.id, x.title, x.description, x.position, s.updated_at, s.updated_by, d, db],
                )?;
            }
            Action::Skip => {}
        }
    }

    for x in &f.epics {
        let s = &x.stamp;
        let action = decide(tx, "epics", s)?;
        count(&mut rep.epics, &action);
        let project = local_id(tx, "projects", &x.project)?;
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO epics (uuid, project_id, title, description, state, position, include_inherited_folders,
                                        created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![s.uuid, project, x.title, x.description, x.state, x.position, x.include_inherited_folders,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE epics SET project_id = ?2, title = ?3, description = ?4, state = ?5, position = ?6,
                            include_inherited_folders = ?7, updated_at = ?8, updated_by = ?9, deleted_at = ?10, deleted_by = ?11
                     WHERE id = ?1",
                    params![l.id, project, x.title, x.description, x.state, x.position, x.include_inherited_folders,
                            s.updated_at, s.updated_by, d, db],
                )?;
            }
            Action::Skip => {}
        }
    }

    for x in &f.topics {
        let s = &x.stamp;
        let action = decide(tx, "topics", s)?;
        count(&mut rep.topics, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let epic = opt_id(tx, "epics", &x.epic)?;
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO topics (uuid, project_id, epic_id, title, description, status, state, state_updated_at,
                                         state_updated_by, position, include_inherited_folders,
                                         created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                    params![s.uuid, project, epic, x.title, x.description, x.status, x.state, x.state_updated_at,
                            x.state_updated_by, x.position, x.include_inherited_folders,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE topics SET project_id = ?2, epic_id = ?3, title = ?4, description = ?5, status = ?6, state = ?7,
                            state_updated_at = ?8, state_updated_by = ?9, position = ?10, include_inherited_folders = ?11,
                            updated_at = ?12, updated_by = ?13, deleted_at = ?14, deleted_by = ?15
                     WHERE id = ?1",
                    params![l.id, project, epic, x.title, x.description, x.status, x.state, x.state_updated_at,
                            x.state_updated_by, x.position, x.include_inherited_folders,
                            s.updated_at, s.updated_by, d, db],
                )?;
            }
            Action::Skip => {}
        }
    }

    for x in &f.entries {
        let s = &x.stamp;
        let action = decide(tx, "entries", s)?;
        count(&mut rep.entries, &action);
        let topic = local_id(tx, "topics", &x.topic)?;
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO entries (uuid, topic_id, seq, kind, author, text, event, old_value, new_value,
                                          created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![s.uuid, topic, x.seq, x.kind, x.author, x.text, x.event, x.old_value, x.new_value,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE entries SET text = ?2, updated_at = ?3, updated_by = ?4, deleted_at = ?5, deleted_by = ?6
                     WHERE id = ?1",
                    params![l.id, x.text, s.updated_at, s.updated_by, d, db],
                )?;
            }
            Action::Skip => {}
        }
    }

    for x in &f.notes {
        let s = &x.stamp;
        let action = decide(tx, "notes", s)?;
        count(&mut rep.notes, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let filed = opt_id(tx, "topics", &x.filed_topic)?;
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO notes (uuid, project_id, text, author, filed_topic_id, filed_at,
                                        created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        s.uuid,
                        project,
                        x.text,
                        x.author,
                        filed,
                        x.filed_at,
                        s.created_at,
                        s.updated_at,
                        s.updated_by,
                        s.deleted_at,
                        s.deleted_by
                    ],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE notes SET text = ?2, filed_topic_id = ?3, filed_at = ?4, updated_at = ?5, updated_by = ?6,
                            deleted_at = ?7, deleted_by = ?8 WHERE id = ?1",
                    params![l.id, x.text, filed, x.filed_at, s.updated_at, s.updated_by, d, db],
                )?;
            }
            Action::Skip => {}
        }
    }

    for x in &f.knowledge {
        let s = &x.stamp;
        let action = decide(tx, "kb_entries", s)?;
        count(&mut rep.knowledge, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let epic = opt_id(tx, "epics", &x.epic)?;
        let topic = opt_id(tx, "topics", &x.topic)?;
        let id = match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO kb_entries (uuid, project_id, epic_id, topic_id, kind, title, summary, body, rev, created_by,
                                             created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![s.uuid, project, epic, topic, x.kind, x.title, x.summary, x.body, x.rev, x.created_by,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
                tx.last_insert_rowid()
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE kb_entries SET project_id = ?2, epic_id = ?3, topic_id = ?4, kind = ?5, title = ?6, summary = ?7,
                            body = ?8, rev = ?9, updated_at = ?10, updated_by = ?11, deleted_at = ?12, deleted_by = ?13
                     WHERE id = ?1",
                    params![l.id, project, epic, topic, x.kind, x.title, x.summary, x.body, x.rev,
                            s.updated_at, s.updated_by, d, db],
                )?;
                l.id
            }
            Action::Skip => local_id(tx, "kb_entries", &s.uuid)?,
        };
        for r in &x.revisions {
            let known: Option<i64> = tx
                .query_row(
                    "SELECT id FROM kb_revisions WHERE uuid = ?1",
                    [&r.uuid],
                    |row| row.get(0),
                )
                .optional()?;
            if known.is_some() {
                rep.revisions.skipped += 1;
                continue;
            }
            // A revision number already taken by another history is kept out
            // rather than overwriting it.
            let added = tx.execute(
                "INSERT OR IGNORE INTO kb_revisions (uuid, entry_id, rev, title, summary, body, author, change, section, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![r.uuid, id, r.rev, r.title, r.summary, r.body, r.author, r.change, r.section, r.created_at],
            )?;
            if added == 1 {
                rep.revisions.created += 1;
            } else {
                rep.revisions.skipped += 1;
            }
        }
    }

    for x in &f.folders {
        let s = &x.stamp;
        let action = decide(tx, "folders", s)?;
        count(&mut rep.folders, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let epic = opt_id(tx, "epics", &x.epic)?;
        let topic = opt_id(tx, "topics", &x.topic)?;
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO folders (uuid, project_id, epic_id, topic_id, path, position,
                                          created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![s.uuid, project, epic, topic, x.path, x.position,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE folders SET project_id = ?2, epic_id = ?3, topic_id = ?4, path = ?5, position = ?6,
                            updated_at = ?7, updated_by = ?8, deleted_at = ?9, deleted_by = ?10 WHERE id = ?1",
                    params![l.id, project, epic, topic, x.path, x.position, s.updated_at, s.updated_by, d, db],
                )?;
            }
            Action::Skip => {}
        }
    }

    Ok(rep)
}
