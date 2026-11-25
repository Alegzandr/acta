//! The payload of a backup (ADR 0006, amended 2026-11-22): one JSON
//! document with every record of the database, or of one project, trashed
//! records included and flagged by `deleted_at`. It only ever leaves Acta
//! encrypted, inside an `.acta-backup` file (`backup.rs`); there is no
//! plaintext export or import. Records refer to each other by UUID; local
//! ids and short ids are never exported. Arrays are sorted by UUID so the
//! same data always gives the same document.
//!
//! Import is a merge keyed on UUID, in one transaction: unknown records are
//! created, known records are updated only when the file's `updated_at` is
//! newer, and nothing is ever deleted: a newer version can restore a trashed
//! record but never trashes a live one. Importing the same file twice changes
//! nothing. A dry run reports the counts and rolls back.
//!
//! Folder paths are machine-specific; they are exported marked `local` and
//! imported as they are, shown as not found when they do not exist here.
//! A path that could name another computer (`\\server\share`, `//server`,
//! a device path) is left out and counted in `folders_refused`: checking
//! that it exists would send the user's credentials to that computer.
//! Commit settings are exported but never imported: a folder new here, or
//! given a new path, is Manual; a known folder keeps its local settings.
//!
//! Versions: the payload carries its own version, [`FORMAT_VERSION`]. 5
//! is the first one a backup holds (versions 1 to 4 were the plaintext
//! `.acta.json` files of 0.3 to 0.6, which no Acta reads any more: an
//! older payload is refused with [`Error::FormatTooOld`], a newer one with
//! [`Error::FormatTooNew`]). It holds the records, contacts and the
//! profile (in an "Everything" export only), questions, attachments (a
//! `blobs` array, one per distinct content with its SHA-256, size, media
//! type and base64 content unless the export left files out, and an
//! `attachments` array) and memory quality (ADR 0015, 9: verified date,
//! state, successor, context, creating topic, sources and pairs). Claude
//! Code session ids are local and never exported, nor is the quality log.
//! Import checks each blob's hash, links content this database already
//! holds, and leaves out attachment rows whose content is neither here nor
//! in the file (named as "not in the export").

use super::Store;
use crate::date::Date;
use crate::error::{Error, Result};
use crate::model::{
    Author, ChangeKind, CommitMode, EntryKind, EventKind, KbId, KbKind, KbLinkKind, KbPairKind,
    KbPairState, KbState, ProjectId, QuestionStatus, Status,
};
use crate::snapshots::SnapshotKind;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// The format name written in every file.
pub const FORMAT_NAME: &str = "acta";
/// The payload version this build writes and the newest it reads.
pub const FORMAT_VERSION: i64 = 5;
/// The oldest payload version this build reads: the first one a backup
/// held. Older versions only existed as plaintext files.
pub const OLDEST_FORMAT_VERSION: i64 = 5;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Version 3; absent from older files.
    #[serde(default)]
    pub questions: Vec<XQuestion>,
    /// Version 3: contacts (request 22).
    #[serde(default)]
    pub contacts: Vec<XContact>,
    /// Version 3: the profile, in an "Everything" export only (request 23).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<XProfile>,
    /// Version 4: the content of attached files, one per hash.
    #[serde(default)]
    pub blobs: Vec<XBlob>,
    /// Version 4: attached files (ADR 0012).
    #[serde(default)]
    pub attachments: Vec<XAttachment>,
    /// Version 5: duplicate and conflict pairs (ADR 0015).
    #[serde(default)]
    pub kb_pairs: Vec<XKbPair>,
}

/// Version 5: a source a knowledge entry names: a record (`target_uuid`),
/// or a commit or a path (`target_text`, a path with its folder's UUID).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XKbLink {
    pub uuid: String,
    pub kind: KbLinkKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    pub created_at: i64,
    pub created_by: Author,
}

/// Version 5: a pair the checker found, by the entries' UUIDs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XKbPair {
    pub a: String,
    pub b: String,
    pub kind: KbPairKind,
    pub state: KbPairState,
    pub score: f64,
    pub created_at: i64,
    pub created_by: Author,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<Author>,
}

/// Version 4: one distinct content. `content` is base64, absent when the
/// export left files out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XBlob {
    /// SHA-256 of the content, lowercase hex.
    pub hash: String,
    pub size: i64,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Version 4: an attached file. Its owner is exactly one of `note`,
/// `entry`, `topic` (the description) or `epic` (the description).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XAttachment {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    /// The blob's hash.
    pub blob: String,
    pub name: String,
    pub size: i64,
    pub position: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_from_note: Option<String>,
    pub created_by: Author,
}

/// Version 3: a contact. Its level is the epic, else the project, else
/// all projects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XContact {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: Option<String>,
    pub epic: Option<String>,
    pub name: String,
    pub job_title: Option<String>,
    pub company: Option<String>,
    pub address_note: Option<String>,
    pub created_by: Author,
    /// The UUID of the contact this one was merged into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_into: Option<String>,
}

/// Version 3: the human's own name and job title.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XProfile {
    pub name: Option<String>,
    pub job_title: Option<String>,
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
    /// Version 2: the UUID of the active folder, absent for the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_folder: Option<String>,
    /// Version 2: `YYYY-MM-DD`, absent for no date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_on: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_on: Option<String>,
    #[serde(flatten)]
    pub report: XReport,
}

/// Version 3: who a topic or an epic is reported to (a contact UUID), and
/// when it was.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_by: Option<Author>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_on: Option<String>,
    #[serde(flatten)]
    pub report: XReport,
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
    /// Added in schema 2; absent from older files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filed_epic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filed_by: Option<Author>,
    /// Version 3: a contact UUID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_to: Option<String>,
    /// Version 4: the UUID of the entry the note was filed as.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filed_entry: Option<String>,
}

/// Version 3: a question (data.md request 18). Its owner is the topic,
/// else the epic, else the project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XQuestion {
    #[serde(flatten)]
    pub stamp: XStamp,
    pub project: String,
    pub epic: Option<String>,
    pub topic: Option<String>,
    pub text: String,
    /// A contact UUID.
    pub asked_to: Option<String>,
    pub expected_on: Option<String>,
    pub status: QuestionStatus,
    pub answer: Option<String>,
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<Author>,
    /// The UUID of the Decision entry logged with the answer.
    pub decision_entry: Option<String>,
    pub position: i64,
    pub created_by: Author,
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
    /// Version 5 (ADR 0015); defaults for older files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_by: Option<Author>,
    #[serde(default)]
    pub state: KbState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_topic: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<XKbLink>,
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
    /// Version 5: the branch of the session that wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
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
    /// Version 2 (ADR 0011); defaults for older files.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub commit_mode: CommitMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_as_user: Option<bool>,
    #[serde(default)]
    pub commit_on_default: bool,
}

impl ExportFile {
    /// The payload as pretty JSON ending with a newline.
    pub fn to_json(&self) -> Result<String> {
        let mut s = serde_json::to_string_pretty(self)?;
        s.push('\n');
        Ok(s)
    }

    /// [`ExportFile::to_json`], written as it is produced: a backup
    /// encrypts it chunk by chunk and never holds the whole text.
    pub fn write_json(&self, mut out: impl std::io::Write) -> Result<()> {
        serde_json::to_writer_pretty(&mut out, self)?;
        out.write_all(b"\n")?;
        Ok(())
    }

    /// Parses a payload, refusing a newer or older format with a clear
    /// error. The text is parsed whole once; only a payload that does not
    /// parse is read again for its header, to tell a foreign, newer or
    /// older document from a broken one.
    pub fn from_json(text: &str) -> Result<ExportFile> {
        match serde_json::from_str::<ExportFile>(text) {
            Ok(file) => {
                check_header(&file.format, file.version)?;
                Ok(file)
            }
            Err(err) => {
                if let Ok(header) = serde_json::from_str::<Header>(text) {
                    check_header(&header.format, header.version)?;
                }
                Err(err.into())
            }
        }
    }
}

/// The two fields that say what a payload is; the rest is skipped
/// without being kept.
#[derive(Deserialize)]
pub(super) struct Header {
    pub format: String,
    pub version: i64,
}

/// Refuses a payload that is not Acta's, or of a version this build does
/// not read.
pub(super) fn check_header(format: &str, version: i64) -> Result<()> {
    if format != FORMAT_NAME {
        return Err(Error::Invalid("this is not an Acta backup".into()));
    }
    if version > FORMAT_VERSION {
        return Err(Error::FormatTooNew {
            found: version,
            supported: FORMAT_VERSION,
        });
    }
    if version < OLDEST_FORMAT_VERSION {
        return Err(Error::FormatTooOld {
            found: version,
            oldest: OLDEST_FORMAT_VERSION,
        });
    }
    Ok(())
}

/// The largest payload an import reads: 512 MiB.
pub const MAX_IMPORT_BYTES: u64 = 512 * 1024 * 1024;

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
    /// New folders left out because their path could name another computer
    /// (a network share, a device): see [`crate::folder_path::is_probe_safe`].
    /// A known folder the file moves to such a path keeps its local row and
    /// counts as skipped.
    pub folders_refused: i64,
    pub questions: ImportCounts,
    pub contacts: ImportCounts,
    /// Contacts widened so every imported reference stays in scope.
    pub lifted: i64,
    pub attachments: ImportCounts,
    /// Names of attachments whose content was in neither the file nor
    /// this database (an export without files): left out.
    pub files_not_in_export: Vec<String>,
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
            self.questions,
            self.contacts,
            self.attachments,
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

fn xreport(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<XReport> {
    Ok(XReport {
        report_to: r.get(at)?,
        reported_at: r.get(at + 1)?,
        reported_by: r.get(at + 2)?,
    })
}

impl Store {
    /// Bytes of the distinct contents an export would carry.
    fn export_files_bytes(&self, project: Option<ProjectId>) -> Result<u64> {
        let only = project.map_or(String::new(), |p| format!(" WHERE project_id = {}", p.0));
        let n: i64 = self.conn.query_row(
            &format!(
                "SELECT coalesce(sum(size), 0) FROM attachment_blobs
                 WHERE complete = 1 AND id IN (SELECT blob_id FROM attachments{only})"
            ),
            [],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u64)
    }
}

/// The blobs the exported attachments name, each once, sorted by hash;
/// the content read chunk by chunk.
fn export_blobs(
    conn: &rusqlite::Connection,
    project: Option<ProjectId>,
    include_files: bool,
) -> Result<Vec<XBlob>> {
    use base64::Engine as _;
    let only = project.map_or(String::new(), |p| format!(" WHERE project_id = {}", p.0));
    let heads = collect(
        conn,
        &format!(
            "SELECT id, hash, size, media_type FROM attachment_blobs
             WHERE complete = 1 AND id IN (SELECT blob_id FROM attachments{only}) ORDER BY hash"
        ),
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                XBlob {
                    hash: r.get(1)?,
                    size: r.get(2)?,
                    media_type: r.get(3)?,
                    content: None,
                },
            ))
        },
    )?;
    let mut out = Vec::with_capacity(heads.len());
    for (id, mut blob) in heads {
        if include_files {
            let mut stmt =
                conn.prepare("SELECT data FROM attachment_chunks WHERE blob_id = ?1 ORDER BY seq")?;
            let mut rows = stmt.query([id])?;
            let mut text = String::with_capacity((blob.size as usize).div_ceil(3) * 4);
            // 1 MiB is not a multiple of 3: bytes left over from a chunk
            // are encoded with the next one.
            let mut carry: Vec<u8> = Vec::new();
            while let Some(row) = rows.next()? {
                let data: Vec<u8> = row.get(0)?;
                carry.extend_from_slice(&data);
                let whole = carry.len() / 3 * 3;
                base64::engine::general_purpose::STANDARD.encode_string(&carry[..whole], &mut text);
                carry.drain(..whole);
            }
            base64::engine::general_purpose::STANDARD.encode_string(&carry, &mut text);
            blob.content = Some(text);
        }
        out.push(blob);
    }
    Ok(out)
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
    /// Everything, or one project (trash included), as an export document,
    /// attached files' content included.
    pub fn export(&self, project: Option<ProjectId>) -> Result<ExportFile> {
        self.export_with(project, true)
    }

    /// [`Store::export`]; with `include_files` false, attachments keep
    /// their names and sizes and the blobs carry no content (the export
    /// dialog's checkbox, ADR 0012, 9).
    ///
    /// With files, an export an import could not read back (over
    /// [`MAX_IMPORT_BYTES`]) is refused with [`Error::ExportTooLarge`]:
    /// export without files then (ADR 0012, 9).
    pub fn export_with(
        &self,
        project: Option<ProjectId>,
        include_files: bool,
    ) -> Result<ExportFile> {
        self.export_within(project, include_files, MAX_IMPORT_BYTES)
    }

    /// [`Store::export_with`] against a given import limit (tests).
    pub(crate) fn export_within(
        &self,
        project: Option<ProjectId>,
        include_files: bool,
        limit: u64,
    ) -> Result<ExportFile> {
        if include_files {
            let bytes = self.export_files_bytes(project)?;
            // Base64 is 4 characters per 3 bytes; the records around it
            // get a 64th of the limit.
            let size = bytes.div_ceil(3) * 4;
            if size + limit / 64 > limit {
                return Err(Error::ExportTooLarge { size, limit });
            }
        }
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
        // One read transaction: every table is read from the same state even
        // while other processes write.
        let read = self.conn.unchecked_transaction()?;
        let conn: &rusqlite::Connection = &read;
        let projects = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, x.title, x.description, x.position, f.uuid, x.due_on
                 FROM projects x LEFT JOIN folders f ON f.id = x.active_folder_id
                 WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.id")
            ),
            |r| {
                Ok(XProject {
                    stamp: xstamp(r, 0)?,
                    title: r.get(6)?,
                    description: r.get(7)?,
                    position: r.get(8)?,
                    active_folder: r.get(9)?,
                    due_on: r.get(10)?,
                })
            },
        )?;
        let epics = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, x.title, x.description, x.state, x.position, f.uuid, x.due_on,
                        c.uuid, x.reported_at, x.reported_by
                 FROM epics x LEFT JOIN contacts c ON c.id = x.report_contact_id JOIN projects p ON p.id = x.project_id
                 LEFT JOIN folders f ON f.id = x.active_folder_id
                 WHERE 1 = 1{} ORDER BY x.uuid",
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
                    active_folder: r.get(11)?,
                    due_on: r.get(12)?,
                    report: xreport(r, 13)?,
                })
            },
        )?;
        let topics = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, x.title, x.description, x.status, x.state,
                        x.state_updated_at, x.state_updated_by, x.position, f.uuid, x.due_on,
                        c.uuid, x.reported_at, x.reported_by
                 FROM topics x LEFT JOIN contacts c ON c.id = x.report_contact_id JOIN projects p ON p.id = x.project_id LEFT JOIN epics e ON e.id = x.epic_id
                 LEFT JOIN folders f ON f.id = x.active_folder_id
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
                    active_folder: r.get(15)?,
                    due_on: r.get(16)?,
                    report: xreport(r, 17)?,
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
                "SELECT {STAMP_SQL}, p.uuid, x.text, x.author, t.uuid, x.filed_at, e.uuid, x.filed_by, c.uuid, fe.uuid
                 FROM notes x LEFT JOIN contacts c ON c.id = x.report_contact_id JOIN projects p ON p.id = x.project_id LEFT JOIN topics t ON t.id = x.filed_topic_id
                 LEFT JOIN epics e ON e.id = x.filed_epic_id LEFT JOIN entries fe ON fe.id = x.filed_entry_id
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
                    filed_epic: r.get(11)?,
                    filed_by: r.get(12)?,
                    report_to: r.get(13)?,
                    filed_entry: r.get(14)?,
                })
            },
        )?;
        let mut knowledge = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, t.uuid, x.kind, x.title, x.summary, x.body, x.rev, x.created_by, x.id,
                        coalesce(x.verified_at, x.updated_at), coalesce(x.verified_by, x.updated_by), x.state,
                        s.uuid, x.context_branch, f.uuid, ct.uuid
                 FROM kb_entries x JOIN projects p ON p.id = x.project_id
                 LEFT JOIN epics e ON e.id = x.epic_id LEFT JOIN topics t ON t.id = x.topic_id
                 LEFT JOIN kb_entries s ON s.id = x.superseded_by
                 LEFT JOIN folders f ON f.id = x.context_folder_id
                 LEFT JOIN topics ct ON ct.id = x.created_topic_id
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
                        verified_at: r.get(16)?,
                        verified_by: r.get(17)?,
                        state: r.get(18)?,
                        superseded_by: r.get(19)?,
                        context_branch: r.get(20)?,
                        context_folder: r.get(21)?,
                        created_topic: r.get(22)?,
                        links: Vec::new(),
                    },
                    r.get::<_, i64>(15)?,
                ))
            },
        )?;
        for (kb, id) in &mut knowledge {
            kb.revisions = collect(
                conn,
                &format!(
                    "SELECT uuid, rev, title, summary, body, author, change, section, created_at, branch
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
                        branch: r.get(9)?,
                    })
                },
            )?;
            kb.links = collect(
                conn,
                &format!(
                    "SELECT l.uuid, l.kind, l.target_uuid, l.target_text, f.uuid, l.blob, l.created_at, l.created_by
                     FROM kb_links l LEFT JOIN folders f ON f.id = l.folder_id
                     WHERE l.entry_id = {id} ORDER BY l.uuid"
                ),
                |r| {
                    Ok(XKbLink {
                        uuid: r.get(0)?,
                        kind: r.get(1)?,
                        target_uuid: r.get(2)?,
                        target_text: r.get(3)?,
                        folder: r.get(4)?,
                        blob: r.get(5)?,
                        created_at: r.get(6)?,
                        created_by: r.get(7)?,
                    })
                },
            )?;
        }
        let kb_pairs = collect(
            conn,
            &format!(
                "SELECT ka.uuid, kb.uuid, x.kind, x.state, x.score, x.created_at, x.created_by,
                        x.resolved_at, x.resolved_by
                 FROM kb_pairs x JOIN kb_entries ka ON ka.id = x.a JOIN kb_entries kb ON kb.id = x.b
                 WHERE 1 = 1{}{} ORDER BY ka.uuid, kb.uuid, x.kind",
                only("ka.project_id"),
                only("kb.project_id")
            ),
            |r| {
                Ok(XKbPair {
                    a: r.get(0)?,
                    b: r.get(1)?,
                    kind: r.get(2)?,
                    state: r.get(3)?,
                    score: r.get(4)?,
                    created_at: r.get(5)?,
                    created_by: r.get(6)?,
                    resolved_at: r.get(7)?,
                    resolved_by: r.get(8)?,
                })
            },
        )?;
        let folders = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, t.uuid, x.path, x.position,
                        x.name, x.icon, x.commit_mode, x.commit_as_user, x.commit_on_default
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
                    name: r.get(11)?,
                    icon: r.get(12)?,
                    commit_mode: r.get(13)?,
                    commit_as_user: r.get(14)?,
                    commit_on_default: r.get(15)?,
                })
            },
        )?;
        let questions = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, t.uuid, x.text, c.uuid, x.expected_on, x.status,
                        x.answer, x.resolved_at, x.resolved_by, d.uuid, x.position, x.created_by
                 FROM questions x JOIN projects p ON p.id = x.project_id
                 LEFT JOIN contacts c ON c.id = x.asked_contact_id
                 LEFT JOIN epics e ON e.id = x.epic_id LEFT JOIN topics t ON t.id = x.topic_id
                 LEFT JOIN entries d ON d.id = x.decision_entry_id
                 WHERE 1 = 1{} ORDER BY x.uuid",
                only("x.project_id")
            ),
            |r| {
                Ok(XQuestion {
                    stamp: xstamp(r, 0)?,
                    project: r.get(6)?,
                    epic: r.get(7)?,
                    topic: r.get(8)?,
                    text: r.get(9)?,
                    asked_to: r.get(10)?,
                    expected_on: r.get(11)?,
                    status: r.get(12)?,
                    answer: r.get(13)?,
                    resolved_at: r.get(14)?,
                    resolved_by: r.get(15)?,
                    decision_entry: r.get(16)?,
                    position: r.get(17)?,
                    created_by: r.get(18)?,
                })
            },
        )?;
        // A project's export carries its own and its epics' contacts and
        // the global contacts it names (they import as global).
        let contact_scope = match project {
            None => String::new(),
            Some(p) => {
                let p = p.0;
                format!(
                    " AND (x.project_id = {p} OR (x.project_id IS NULL AND x.id IN (
                        SELECT asked_contact_id FROM questions WHERE project_id = {p}
                        UNION SELECT report_contact_id FROM topics WHERE project_id = {p}
                        UNION SELECT report_contact_id FROM epics WHERE project_id = {p}
                        UNION SELECT report_contact_id FROM notes WHERE project_id = {p})))"
                )
            }
        };
        let contacts = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, e.uuid, x.name, x.job_title, x.company, x.address_note,
                        x.created_by, m.uuid
                 FROM contacts x LEFT JOIN projects p ON p.id = x.project_id
                 LEFT JOIN epics e ON e.id = x.epic_id LEFT JOIN contacts m ON m.id = x.merged_into
                 WHERE 1 = 1{contact_scope} ORDER BY x.uuid"
            ),
            |r| {
                Ok(XContact {
                    stamp: xstamp(r, 0)?,
                    project: r.get(6)?,
                    epic: r.get(7)?,
                    name: r.get(8)?,
                    job_title: r.get(9)?,
                    company: r.get(10)?,
                    address_note: r.get(11)?,
                    created_by: r.get(12)?,
                    merged_into: r.get(13)?,
                })
            },
        )?;
        let profile = match project {
            Some(_) => None,
            None => {
                let p = self.profile()?;
                (p.name.is_some() || p.job_title.is_some()).then_some(XProfile {
                    name: p.name,
                    job_title: p.job_title,
                })
            }
        };
        let attachments = collect(
            conn,
            &format!(
                "SELECT {STAMP_SQL}, p.uuid, n.uuid, en.uuid, t.uuid, e.uuid, b.hash, x.name, b.size,
                        x.position, m.uuid, x.created_by
                 FROM attachments x JOIN projects p ON p.id = x.project_id
                 JOIN attachment_blobs b ON b.id = x.blob_id
                 LEFT JOIN notes n ON n.id = x.note_id LEFT JOIN entries en ON en.id = x.entry_id
                 LEFT JOIN topics t ON t.id = x.topic_id LEFT JOIN epics e ON e.id = x.epic_id
                 LEFT JOIN notes m ON m.id = x.moved_from_note_id
                 WHERE b.complete = 1{} ORDER BY x.uuid",
                only("x.project_id")
            ),
            |r| {
                Ok(XAttachment {
                    stamp: xstamp(r, 0)?,
                    project: r.get(6)?,
                    note: r.get(7)?,
                    entry: r.get(8)?,
                    topic: r.get(9)?,
                    epic: r.get(10)?,
                    blob: r.get(11)?,
                    name: r.get(12)?,
                    size: r.get(13)?,
                    position: r.get(14)?,
                    moved_from_note: r.get(15)?,
                    created_by: r.get(16)?,
                })
            },
        )?;
        let blobs = export_blobs(conn, project, include_files)?;
        drop(read);
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
            questions,
            contacts,
            profile,
            blobs,
            attachments,
            kb_pairs,
        })
    }

    /// Merges a file into the database, as the human. With `dry_run`
    /// nothing is kept and the report says what would happen; otherwise an
    /// `import` snapshot is taken first.
    pub fn import(&mut self, file: &ExportFile, dry_run: bool) -> Result<ImportReport> {
        self.import_by(file, dry_run, Author::Human)
    }

    /// [`Store::import`] with its author. An import by Claude Code never
    /// sets what only the user sets (ADR 0011): folder commit settings keep
    /// their local values (Manual for a new folder) and no active folder is
    /// set. The CLI refuses `acta import` under Claude Code; this is the
    /// second line.
    pub fn import_by(
        &mut self,
        file: &ExportFile,
        dry_run: bool,
        author: Author,
    ) -> Result<ImportReport> {
        check_header(&file.format, file.version)?;
        if !dry_run {
            self.check_writable()?;
            self.snapshot(SnapshotKind::Import)?;
        }
        // A dry run writes nothing that survives, so it works on a
        // read-only database too.
        if dry_run {
            let tx = self
                .conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let report = merge(&tx, file, author)?;
            tx.rollback()?;
            return Ok(report);
        }
        let tx = self.write()?;
        let report = merge(&tx, file, author)?;
        tx.commit()?;
        let _ = self.refresh_link_index();
        // Commit settings and paths may have changed: Claude Code's
        // settings follow the columns (ADR 0011, Export). An import by
        // Claude Code changes neither, and never writes them.
        if author != Author::Claude {
            let _ = crate::claude_settings::sync_all(self);
        }
        Ok(report)
    }
}

/// An imported topic or knowledge entry counts as seen (data.md request 1):
/// an import is the human's own act, and its rows must not light up as
/// if Claude Code had just written them. Inside the import transaction.
fn seen_on_import(tx: &rusqlite::Connection, kind: &str, id: i64, at: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO read_markers (kind, record_id, last_seen_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (kind, record_id)
         DO UPDATE SET last_seen_at = MAX(last_seen_at, excluded.last_seen_at)",
        params![kind, id, at],
    )?;
    Ok(())
}

/// An imported question or contact counts as seen, as topics do.
fn seen_marker(tx: &rusqlite::Connection, kind: &str, id: i64, at: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO record_markers (kind, record_id, last_seen_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (kind, record_id)
         DO UPDATE SET last_seen_at = MAX(last_seen_at, excluded.last_seen_at)",
        params![kind, id, at],
    )?;
    Ok(())
}

/// A contact reference from a file: a contact UUID, followed through
/// merges to the contact that holds its uses now.
fn contact_id(tx: &rusqlite::Connection, text: &Option<String>) -> Result<Option<i64>> {
    let Some(t) = text.as_deref() else {
        return Ok(None);
    };
    let known: Option<i64> = tx
        .query_row("SELECT id FROM contacts WHERE uuid = ?1", [t], |r| r.get(0))
        .optional()?;
    match known {
        Some(id) => Ok(Some(super::people::follow_merges(tx, id)?)),
        None => Err(Error::Invalid(format!(
            "the file refers to a missing record {t} in contacts"
        ))),
    }
}

/// The Report to of an imported topic or epic.
fn write_report(tx: &rusqlite::Connection, table: &str, id: i64, r: &XReport) -> Result<()> {
    let to = contact_id(tx, &r.report_to)?;
    let (at, by) = if to.is_some() {
        (r.reported_at, r.reported_by)
    } else {
        (None, None)
    };
    tx.execute(
        &format!(
            "UPDATE {table} SET report_contact_id = ?2, reported_at = ?3, reported_by = ?4 WHERE id = ?1"
        ),
        params![id, to, at, by],
    )?;
    Ok(())
}

/// Points an entry's `rev` at the newest revision holding its current
/// title, summary and body. If none does (the file's version of the entry
/// was newer than any revision it carried), that version is saved as a new
/// revision, so history always contains what the entry shows.
fn reconcile_rev(tx: &rusqlite::Connection, id: i64) -> Result<()> {
    let matching: Option<i64> = tx.query_row(
        "SELECT max(r.rev) FROM kb_revisions r JOIN kb_entries k ON k.id = r.entry_id
         WHERE k.id = ?1 AND r.title = k.title AND r.summary = k.summary AND r.body = k.body",
        [id],
        |r| r.get(0),
    )?;
    let rev = match matching {
        Some(rev) => rev,
        None => {
            let rev = super::kb::next_rev(tx, KbId(id))?;
            tx.execute(
                "INSERT INTO kb_revisions (uuid, entry_id, rev, title, summary, body, author, change, section, created_at)
                 SELECT ?2, id, ?3, title, summary, body, updated_by, 'edited', NULL, updated_at
                 FROM kb_entries WHERE id = ?1",
                params![id, super::new_uuid(), rev],
            )?;
            rev
        }
    };
    tx.execute(
        "UPDATE kb_entries SET rev = ?2 WHERE id = ?1 AND rev <> ?2",
        params![id, rev],
    )?;
    Ok(())
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

fn decide(tx: &rusqlite::Connection, table: &str, stamp: &XStamp) -> Result<Action> {
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

fn local_id(tx: &rusqlite::Connection, table: &str, uuid: &str) -> Result<i64> {
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

fn opt_id(tx: &rusqlite::Connection, table: &str, uuid: &Option<String>) -> Result<Option<i64>> {
    uuid.as_deref().map(|u| local_id(tx, table, u)).transpose()
}

/// A due day from a file: the ISO form or absent.
fn due(uuid: &str, text: &Option<String>) -> Result<Option<Date>> {
    match text {
        None => Ok(None),
        Some(t) => Date::parse_iso(t)
            .map(Some)
            .ok_or_else(|| Error::Invalid(format!("{uuid} has an invalid due date"))),
    }
}

/// A folder icon from a file: one of the known names, else automatic.
fn icon(name: &str) -> &str {
    if crate::model::FOLDER_ICONS.contains(&name) {
        name
    } else {
        ""
    }
}

fn count(c: &mut ImportCounts, action: &Action) {
    match action {
        Action::Insert => c.created += 1,
        Action::Update(_) => c.updated += 1,
        Action::Skip => c.skipped += 1,
    }
}

fn merge(tx: &rusqlite::Connection, f: &ExportFile, author: Author) -> Result<ImportReport> {
    let mut rep = ImportReport::default();
    // Epics name contacts, and contacts live in epics: an epic's Report to
    // is written once the contacts exist.
    let mut epic_reports: Vec<(i64, &XReport)> = Vec::new();
    let by_claude = author == Author::Claude;
    // Active folders are set once the folders exist: (table, local id,
    // folder UUID) of every project, epic and topic the merge wrote.
    let mut pointers: Vec<(&str, i64, Option<&str>)> = Vec::new();

    for x in &f.projects {
        let s = &x.stamp;
        let action = decide(tx, "projects", s)?;
        count(&mut rep.projects, &action);
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO projects (uuid, title, description, position, due_on, created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![s.uuid, x.title, x.description, x.position, due(&s.uuid, &x.due_on)?,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
                pointers.push((
                    "projects",
                    tx.last_insert_rowid(),
                    x.active_folder.as_deref(),
                ));
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE projects SET title = ?2, description = ?3, position = ?4, updated_at = ?5, updated_by = ?6,
                            deleted_at = ?7, deleted_by = ?8,
                            deleted_batch = CASE WHEN deleted_at IS ?7 THEN deleted_batch ELSE NULL END,
                            due_on = ?9 WHERE id = ?1",
                    params![l.id, x.title, x.description, x.position, s.updated_at, s.updated_by, d, db,
                            due(&s.uuid, &x.due_on)?],
                )?;
                pointers.push(("projects", l.id, x.active_folder.as_deref()));
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
                    "INSERT INTO epics (uuid, project_id, title, description, state, position, due_on,
                                        created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![s.uuid, project, x.title, x.description, x.state, x.position, due(&s.uuid, &x.due_on)?,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
                let id = tx.last_insert_rowid();
                epic_reports.push((id, &x.report));
                pointers.push(("epics", id, x.active_folder.as_deref()));
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE epics SET project_id = ?2, title = ?3, description = ?4, state = ?5, position = ?6,
                            due_on = ?7,
                            updated_at = ?8, updated_by = ?9, deleted_at = ?10, deleted_by = ?11,
                            deleted_batch = CASE WHEN deleted_at IS ?10 THEN deleted_batch ELSE NULL END
                     WHERE id = ?1",
                    params![l.id, project, x.title, x.description, x.state, x.position, due(&s.uuid, &x.due_on)?,
                            s.updated_at, s.updated_by, d, db],
                )?;
                epic_reports.push((l.id, &x.report));
                pointers.push(("epics", l.id, x.active_folder.as_deref()));
            }
            Action::Skip => {}
        }
    }

    for x in &f.contacts {
        let s = &x.stamp;
        let action = decide(tx, "contacts", s)?;
        count(&mut rep.contacts, &action);
        let project = opt_id(tx, "projects", &x.project)?;
        let epic = opt_id(tx, "epics", &x.epic)?.filter(|_| project.is_some());
        let name = crate::person::clean(&x.name)
            .ok()
            .flatten()
            .ok_or_else(|| Error::Invalid(format!("{} has an invalid name", s.uuid)))?;
        let key = crate::person::person_key(&name);
        // A contact Claude Code brings in is stamped as its own: a file
        // cannot make it pass for the human's.
        let created_by = if by_claude {
            Author::Claude
        } else {
            x.created_by
        };
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO contacts (uuid, project_id, epic_id, name, name_key, job_title, company, address_note,
                                           created_by, created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![s.uuid, project, epic, name, key, x.job_title, x.company, x.address_note,
                            created_by, s.created_at, s.updated_at,
                            if by_claude { Author::Claude } else { s.updated_by },
                            s.deleted_at, s.deleted_by],
                )?;
                seen_marker(tx, "contact", tx.last_insert_rowid(), s.updated_at)?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                // The name and details are the human's: an import by Claude
                // Code keeps the local ones (as it keeps commit modes).
                tx.execute(
                    "UPDATE contacts SET project_id = ?2, epic_id = ?3,
                            name = CASE WHEN ?13 THEN name ELSE ?4 END,
                            name_key = CASE WHEN ?13 THEN name_key ELSE ?5 END,
                            job_title = CASE WHEN ?13 THEN job_title ELSE ?6 END,
                            company = CASE WHEN ?13 THEN company ELSE ?7 END,
                            address_note = CASE WHEN ?13 THEN address_note ELSE ?8 END,
                            updated_at = ?9, updated_by = ?10,
                            deleted_at = ?11, deleted_by = ?12,
                            deleted_batch = CASE WHEN deleted_at IS ?11 THEN deleted_batch ELSE NULL END
                     WHERE id = ?1",
                    params![l.id, project, epic, name, key, x.job_title, x.company, x.address_note,
                            s.updated_at, if by_claude { Author::Claude } else { s.updated_by }, d, db,
                            by_claude],
                )?;
                seen_marker(tx, "contact", l.id, s.updated_at)?;
            }
            Action::Skip => {}
        }
    }
    for x in &f.contacts {
        if let Some(m) = &x.merged_into {
            tx.execute(
                "UPDATE contacts SET merged_into = (SELECT id FROM contacts WHERE uuid = ?2) WHERE uuid = ?1",
                params![x.stamp.uuid, m],
            )?;
        }
    }
    for (id, r) in epic_reports {
        write_report(tx, "epics", id, r)?;
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
                                         state_updated_by, position, due_on,
                                         created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                    params![s.uuid, project, epic, x.title, x.description, x.status, x.state, x.state_updated_at,
                            x.state_updated_by, x.position, due(&s.uuid, &x.due_on)?,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
                let id = tx.last_insert_rowid();
                write_report(tx, "topics", id, &x.report)?;
                pointers.push(("topics", id, x.active_folder.as_deref()));
                seen_on_import(tx, "topic", id, s.updated_at)?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE topics SET project_id = ?2, epic_id = ?3, title = ?4, description = ?5, status = ?6, state = ?7,
                            state_updated_at = ?8, state_updated_by = ?9, position = ?10,
                            due_on = ?11,
                            updated_at = ?12, updated_by = ?13, deleted_at = ?14, deleted_by = ?15,
                            deleted_batch = CASE WHEN deleted_at IS ?14 THEN deleted_batch ELSE NULL END
                     WHERE id = ?1",
                    params![l.id, project, epic, x.title, x.description, x.status, x.state, x.state_updated_at,
                            x.state_updated_by, x.position, due(&s.uuid, &x.due_on)?,
                            s.updated_at, s.updated_by, d, db],
                )?;
                write_report(tx, "topics", l.id, &x.report)?;
                pointers.push(("topics", l.id, x.active_folder.as_deref()));
                seen_on_import(tx, "topic", l.id, s.updated_at)?;
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
                // Two installations may both have written `T-42#15`: the
                // imported one keeps its UUID and takes the next number.
                let seq = match x.seq {
                    Some(seq) => {
                        let taken: bool = tx.query_row(
                            "SELECT EXISTS (SELECT 1 FROM entries WHERE topic_id = ?1 AND seq = ?2)",
                            params![topic, seq],
                            |r| r.get(0),
                        )?;
                        if taken {
                            Some(tx.query_row(
                                "SELECT coalesce(max(seq), 0) + 1 FROM entries WHERE topic_id = ?1",
                                [topic],
                                |r| r.get::<_, i64>(0),
                            )?)
                        } else {
                            Some(seq)
                        }
                    }
                    None => None,
                };
                tx.execute(
                    "INSERT INTO entries (uuid, topic_id, seq, kind, author, text, event, old_value, new_value,
                                          created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![s.uuid, topic, seq, x.kind, x.author, x.text, x.event, x.old_value, x.new_value,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE entries SET text = ?2, updated_at = ?3, updated_by = ?4, deleted_at = ?5, deleted_by = ?6,
                            deleted_batch = CASE WHEN deleted_at IS ?5 THEN deleted_batch ELSE NULL END
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
        let filed_epic = opt_id(tx, "epics", &x.filed_epic)?;
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO notes (uuid, project_id, text, author, filed_topic_id, filed_at,
                                        filed_epic_id, filed_by,
                                        created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        s.uuid,
                        project,
                        x.text,
                        x.author,
                        filed,
                        x.filed_at,
                        filed_epic,
                        x.filed_by,
                        s.created_at,
                        s.updated_at,
                        s.updated_by,
                        s.deleted_at,
                        s.deleted_by
                    ],
                )?;
                let id = tx.last_insert_rowid();
                let to = contact_id(tx, &x.report_to)?;
                tx.execute(
                    "UPDATE notes SET report_contact_id = ?2 WHERE id = ?1",
                    params![id, to],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE notes SET text = ?2, filed_topic_id = ?3, filed_at = ?4, updated_at = ?5, updated_by = ?6,
                            deleted_at = ?7, deleted_by = ?8,
                            deleted_batch = CASE WHEN deleted_at IS ?7 THEN deleted_batch ELSE NULL END,
                            filed_epic_id = ?9, filed_by = ?10 WHERE id = ?1",
                    params![l.id, x.text, filed, x.filed_at, s.updated_at, s.updated_by, d, db, filed_epic, x.filed_by],
                )?;
                let to = contact_id(tx, &x.report_to)?;
                tx.execute(
                    "UPDATE notes SET report_contact_id = ?2 WHERE id = ?1",
                    params![l.id, to],
                )?;
            }
            Action::Skip => {}
        }
    }

    // Version 5: memory quality (ADR 0015). Entries the file wrote get
    // their successor, context folder and creating topic once every
    // entry and folder is in (`merge_kb_quality`).
    let mut applied: Vec<(&XKb, i64)> = Vec::new();
    for x in &f.knowledge {
        let s = &x.stamp;
        let action = decide(tx, "kb_entries", s)?;
        count(&mut rep.knowledge, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let epic = opt_id(tx, "epics", &x.epic)?;
        let topic = opt_id(tx, "topics", &x.topic)?;
        let (verified_at, verified_by) = (
            x.verified_at.unwrap_or(s.updated_at),
            x.verified_by.unwrap_or(s.updated_by),
        );
        let id = match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO kb_entries (uuid, project_id, epic_id, topic_id, kind, title, summary, body, rev, created_by,
                                             created_at, updated_at, updated_by, deleted_at, deleted_by,
                                             verified_at, verified_by, state, context_branch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                    params![s.uuid, project, epic, topic, x.kind, x.title, x.summary, x.body, x.rev, x.created_by,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by,
                            verified_at, verified_by, x.state, x.context_branch],
                )?;
                let id = tx.last_insert_rowid();
                seen_on_import(tx, "kb", id, s.updated_at)?;
                applied.push((x, id));
                id
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE kb_entries SET project_id = ?2, epic_id = ?3, topic_id = ?4, kind = ?5, title = ?6, summary = ?7,
                            body = ?8, rev = ?9, updated_at = ?10, updated_by = ?11, deleted_at = ?12, deleted_by = ?13,
                            deleted_batch = CASE WHEN deleted_at IS ?12 THEN deleted_batch ELSE NULL END
                     WHERE id = ?1",
                    params![l.id, project, epic, topic, x.kind, x.title, x.summary, x.body, x.rev,
                            s.updated_at, s.updated_by, d, db],
                )?;
                tx.execute(
                    "UPDATE kb_entries SET verified_at = ?2, verified_by = ?3 WHERE id = ?1",
                    params![l.id, verified_at, verified_by],
                )?;
                tx.execute(
                    "UPDATE kb_entries SET state = ?2, context_branch = ?3 WHERE id = ?1",
                    params![l.id, x.state, x.context_branch],
                )?;
                applied.push((x, l.id));
                seen_on_import(tx, "kb", l.id, s.updated_at)?;
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
            // A number another history already uses: the imported revision
            // is kept, under the next free number.
            let taken: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM kb_revisions WHERE entry_id = ?1 AND rev = ?2)",
                params![id, r.rev],
                |row| row.get(0),
            )?;
            let rev = if taken {
                super::kb::next_rev(tx, KbId(id))?
            } else {
                r.rev
            };
            tx.execute(
                "INSERT INTO kb_revisions (uuid, entry_id, rev, title, summary, body, author, change, section, created_at, branch)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![r.uuid, id, rev, r.title, r.summary, r.body, r.author, r.change, r.section, r.created_at,
                        r.branch],
            )?;
            rep.revisions.created += 1;
        }
        reconcile_rev(tx, id)?;
    }

    for x in &f.folders {
        let s = &x.stamp;
        let action = decide(tx, "folders", s)?;
        // Whether the file gives this row a path this installation has not
        // known it by: always for a new row; for a known one only when the
        // path differs (Claude Code's imports keep the local path).
        let new_path = match &action {
            Action::Insert => true,
            Action::Update(l) => {
                let here: String =
                    tx.query_row("SELECT path FROM folders WHERE id = ?1", [l.id], |r| {
                        r.get(0)
                    })?;
                !by_claude && here != x.path
            }
            Action::Skip => false,
        };
        // A path that could name another computer is never taken from a
        // file: Acta checks every folder's existence, and on Windows that
        // alone sends the user's credentials to the computer named.
        // A new row is left out and counted (it is missing here); a known
        // row keeps its local version, so nothing is missing.
        if new_path && !crate::folder_path::is_probe_safe(&x.path) {
            match action {
                Action::Insert => rep.folders_refused += 1,
                _ => rep.folders.skipped += 1,
            }
            continue;
        }
        count(&mut rep.folders, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let epic = opt_id(tx, "epics", &x.epic)?;
        let topic = opt_id(tx, "topics", &x.topic)?;
        // Commit settings grant Claude Code `git commit` and `git push` in
        // the folder, and the file may come from someone else: they are
        // never taken from it. A folder new here, or given a new path by
        // the file, is Manual; a known folder at the same path keeps its
        // local settings.
        let keep_commit = !new_path;
        let (mode, as_user, on_default) = (CommitMode::Manual, None::<bool>, false);
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO folders (uuid, project_id, epic_id, topic_id, path, position,
                                          name, icon, commit_mode, commit_as_user, commit_on_default,
                                          created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                    params![s.uuid, project, epic, topic, x.path, x.position,
                            x.name, icon(&x.icon), mode, as_user, on_default,
                            s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
                super::folders::forget_identity(
                    tx,
                    crate::model::FolderId(tx.last_insert_rowid()),
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE folders SET project_id = ?2, epic_id = ?3, topic_id = ?4,
                            path = CASE WHEN ?17 THEN path ELSE ?5 END, position = ?6,
                            updated_at = ?7, updated_by = ?8, deleted_at = ?9, deleted_by = ?10,
                            deleted_batch = CASE WHEN deleted_at IS ?9 THEN deleted_batch ELSE NULL END,
                            name = ?11,
                            icon = ?12,
                            commit_mode = CASE WHEN ?16 THEN commit_mode ELSE ?13 END,
                            commit_as_user = CASE WHEN ?16 THEN commit_as_user ELSE ?14 END,
                            commit_on_default = CASE WHEN ?16 THEN commit_on_default ELSE ?15 END
                     WHERE id = ?1",
                    params![l.id, project, epic, topic, x.path, x.position, s.updated_at, s.updated_by, d, db,
                            x.name, icon(&x.icon), mode, as_user, on_default,
                            keep_commit, by_claude],
                )?;
            }
            Action::Skip => {}
        }
    }

    for x in &f.questions {
        let s = &x.stamp;
        let action = decide(tx, "questions", s)?;
        count(&mut rep.questions, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let epic = opt_id(tx, "epics", &x.epic)?;
        let topic = opt_id(tx, "topics", &x.topic)?;
        // An entry this database does not have (a partial file) leaves the
        // link out rather than refusing the whole import.
        let entry: Option<i64> = match &x.decision_entry {
            Some(u) => tx
                .query_row("SELECT id FROM entries WHERE uuid = ?1", [u], |r| r.get(0))
                .optional()?,
            None => None,
        };
        let asked = contact_id(tx, &x.asked_to)?;
        let expected = due(&s.uuid, &x.expected_on)?;
        let (epic, topic) = if topic.is_some() {
            (None, topic)
        } else {
            (epic, None)
        };
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO questions (uuid, project_id, epic_id, topic_id, text, asked_contact_id, expected_on,
                                            status, answer, resolved_at, resolved_by, decision_entry_id, position,
                                            created_by, created_at, updated_at, updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                    params![s.uuid, project, epic, topic, x.text, asked, expected,
                            x.status, x.answer, x.resolved_at, x.resolved_by, entry, x.position,
                            x.created_by, s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
                seen_marker(tx, "question", tx.last_insert_rowid(), s.updated_at)?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE questions SET project_id = ?2, epic_id = ?3, topic_id = ?4, text = ?5,
                            asked_contact_id = ?6, expected_on = ?7, status = ?8, answer = ?9, resolved_at = ?10,
                            resolved_by = ?11, decision_entry_id = ?12, position = ?13,
                            updated_at = ?14, updated_by = ?15, deleted_at = ?16, deleted_by = ?17,
                            deleted_batch = CASE WHEN deleted_at IS ?16 THEN deleted_batch ELSE NULL END
                     WHERE id = ?1",
                    params![l.id, project, epic, topic, x.text, asked, expected,
                            x.status, x.answer, x.resolved_at, x.resolved_by, entry, x.position,
                            s.updated_at, s.updated_by, d, db],
                )?;
                seen_marker(tx, "question", l.id, s.updated_at)?;
            }
            Action::Skip => {}
        }
    }

    merge_attachments(tx, f, &mut rep)?;

    // Records of this database that still name a contact the file merged
    // away now name the one that holds its uses; and every reference the
    // merge left out of its contact's scope lifts that contact.
    super::people::repoint_merged(tx)?;
    rep.lifted = super::people::lift_out_of_scope(tx, author, crate::time::now_ms())?.len() as i64;
    // The profile goes only into an empty one: it is this installation's.
    if let Some(p) = &f.profile {
        let empty: bool = tx.query_row(
            "SELECT NOT EXISTS (SELECT 1 FROM profile WHERE name IS NOT NULL OR job_title IS NOT NULL)",
            [],
            |r| r.get(0),
        )?;
        if empty {
            let name = p
                .name
                .as_deref()
                .and_then(|n| crate::person::clean(n).ok().flatten());
            tx.execute(
                "INSERT INTO profile (id, name, job_title, updated_at) VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT (id) DO UPDATE SET name = excluded.name, job_title = excluded.job_title,
                     updated_at = excluded.updated_at",
                params![name, p.job_title, crate::time::now_ms()],
            )?;
        }
    }

    // A pointer to a folder this database does not have is dropped: the
    // default rule applies, as it does to a pointer nobody can see. A
    // version 1 file has no pointers to give, and Claude Code never sets
    // one: the local choice stays.
    if by_claude {
        pointers.clear();
    }
    merge_kb_quality(tx, f, &applied)?;
    for (table, id, folder) in pointers {
        let folder: Option<i64> = match folder {
            Some(uuid) => tx
                .query_row("SELECT id FROM folders WHERE uuid = ?1", [uuid], |r| {
                    r.get(0)
                })
                .optional()?,
            None => None,
        };
        tx.execute(
            &format!("UPDATE {table} SET active_folder_id = ?2 WHERE id = ?1"),
            params![id, folder],
        )?;
    }

    Ok(rep)
}

/// Version 5: for the entries the file wrote, the successor, the context
/// folder and the creating topic (a UUID this database does not have is
/// dropped); then every entry's sources not known here (by UUID; a folder
/// not known here leaves a file source out); then pairs, the newer
/// decision winning (`resolved_at`, else `created_at`).
fn merge_kb_quality(
    tx: &rusqlite::Connection,
    f: &ExportFile,
    applied: &[(&XKb, i64)],
) -> Result<()> {
    let find = |table: &str, uuid: &Option<String>| -> Result<Option<i64>> {
        Ok(match uuid {
            Some(u) => tx
                .query_row(
                    &format!("SELECT id FROM {table} WHERE uuid = ?1"),
                    [u],
                    |r| r.get(0),
                )
                .optional()?,
            None => None,
        })
    };
    for (x, id) in applied {
        let successor = find("kb_entries", &x.superseded_by)?;
        let folder = find("folders", &x.context_folder)?;
        let topic = find("topics", &x.created_topic)?;
        tx.execute(
            "UPDATE kb_entries SET superseded_by = ?2, context_folder_id = ?3, created_topic_id = ?4,
                    context_branch = CASE WHEN ?3 IS NULL THEN NULL ELSE context_branch END
             WHERE id = ?1",
            params![id, successor, folder, topic],
        )?;
    }
    for x in &f.knowledge {
        let Some(id) = find("kb_entries", &Some(x.stamp.uuid.clone()))? else {
            continue;
        };
        for l in &x.links {
            let known: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM kb_links WHERE uuid = ?1)",
                [&l.uuid],
                |r| r.get(0),
            )?;
            if known || l.target_uuid.is_some() == l.target_text.is_some() {
                continue;
            }
            let folder = find("folders", &l.folder)?;
            if l.kind == KbLinkKind::File && folder.is_none() {
                continue;
            }
            tx.execute(
                "INSERT INTO kb_links (uuid, entry_id, kind, target_uuid, target_text, folder_id, blob, created_at, created_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![l.uuid, id, l.kind, l.target_uuid, l.target_text, folder, l.blob, l.created_at,
                        l.created_by],
            )?;
        }
    }
    for p in &f.kb_pairs {
        let (Some(x), Some(y)) = (
            find("kb_entries", &Some(p.a.clone()))?,
            find("kb_entries", &Some(p.b.clone()))?,
        ) else {
            continue;
        };
        if x == y {
            continue;
        }
        let (a, b) = if x < y { (x, y) } else { (y, x) };
        let at = p.resolved_at.unwrap_or(p.created_at);
        let local: Option<i64> = tx
            .query_row(
                "SELECT coalesce(resolved_at, created_at) FROM kb_pairs WHERE a = ?1 AND b = ?2 AND kind = ?3",
                params![a, b, p.kind],
                |r| r.get(0),
            )
            .optional()?;
        if local.is_some_and(|l| l >= at) {
            continue;
        }
        tx.execute(
            "INSERT INTO kb_pairs (a, b, kind, state, score, created_at, created_by, resolved_at, resolved_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (a, b, kind) DO UPDATE SET state = ?4, score = ?5, resolved_at = ?8, resolved_by = ?9",
            params![a, b, p.kind, p.state, p.score, p.created_at, p.created_by, p.resolved_at, p.resolved_by],
        )?;
    }
    Ok(())
}

/// Version 4: notes' `filed_entry`, then blobs (hash checked, content this
/// database holds linked), then attachment rows. Rows whose content is in
/// neither place are left out and named in the report.
fn merge_attachments(
    tx: &rusqlite::Connection,
    f: &ExportFile,
    rep: &mut ImportReport,
) -> Result<()> {
    use base64::Engine as _;
    for x in &f.notes {
        let Some(e) = &x.filed_entry else { continue };
        let entry: Option<i64> = tx
            .query_row("SELECT id FROM entries WHERE uuid = ?1", [e], |r| r.get(0))
            .optional()?;
        if entry.is_some() {
            tx.execute(
                "UPDATE notes SET filed_entry_id = ?2 WHERE uuid = ?1",
                params![x.stamp.uuid, entry],
            )?;
        }
    }
    let mut blobs: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for b in &f.blobs {
        let local: Option<i64> = tx
            .query_row(
                "SELECT id FROM attachment_blobs WHERE hash = ?1 AND complete = 1",
                [&b.hash],
                |r| r.get(0),
            )
            .optional()?;
        let id = match (local, &b.content) {
            (Some(id), _) => id,
            (None, Some(text)) => {
                // Over the attachment limit, or longer than its own size
                // says: refused before anything is decoded.
                let max = super::attachments::MAX_FILE_BYTES;
                if b.size < 0
                    || b.size as u64 > max
                    || text.len() as u64 > (b.size as u64).div_ceil(3) * 4 + 4
                {
                    return Err(Error::Invalid(format!(
                        "blob {} is over the {} limit for a file, or larger than its size says",
                        b.hash,
                        crate::files::size_label(max as i64)
                    )));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(text)
                    .map_err(|_| Error::Invalid(format!("blob {} is not valid base64", b.hash)))?;
                if bytes.len() as i64 != b.size {
                    return Err(Error::Invalid(format!(
                        "blob {} has {} bytes, not the {} it declares",
                        b.hash,
                        bytes.len(),
                        b.size
                    )));
                }
                if super::attachments::hash_of(&bytes) != b.hash {
                    return Err(Error::Invalid(format!(
                        "blob {} does not match its hash",
                        b.hash
                    )));
                }
                super::attachments::put_blob(tx, &bytes)?
            }
            (None, None) => continue,
        };
        blobs.insert(b.hash.as_str(), id);
    }
    for x in &f.attachments {
        let s = &x.stamp;
        let blob = match blobs.get(x.blob.as_str()) {
            Some(id) => Some(*id),
            None => tx
                .query_row(
                    "SELECT id FROM attachment_blobs WHERE hash = ?1 AND complete = 1",
                    [&x.blob],
                    |r| r.get(0),
                )
                .optional()?,
        };
        let Some(blob) = blob else {
            rep.files_not_in_export
                .push(super::attachments::clean_name(&x.name));
            continue;
        };
        let action = decide(tx, "attachments", s)?;
        count(&mut rep.attachments, &action);
        let project = local_id(tx, "projects", &x.project)?;
        let note = opt_id(tx, "notes", &x.note)?;
        let entry = opt_id(tx, "entries", &x.entry)?;
        let topic = opt_id(tx, "topics", &x.topic)?;
        let epic = opt_id(tx, "epics", &x.epic)?;
        let owners = [note, entry, topic, epic]
            .iter()
            .filter(|o| o.is_some())
            .count();
        if owners != 1 {
            return Err(Error::Invalid(format!(
                "attachment {} must have exactly one owner",
                s.uuid
            )));
        }
        let moved = match &x.moved_from_note {
            Some(u) => tx
                .query_row("SELECT id FROM notes WHERE uuid = ?1", [u], |r| {
                    r.get::<_, i64>(0)
                })
                .optional()?,
            None => None,
        };
        let name = super::attachments::clean_name(&x.name);
        match action {
            Action::Insert => {
                tx.execute(
                    "INSERT INTO attachments (uuid, project_id, note_id, entry_id, topic_id, epic_id, blob_id, name,
                                              position, moved_from_note_id, created_by, created_at, updated_at,
                                              updated_by, deleted_at, deleted_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                    params![s.uuid, project, note, entry, topic, epic, blob, name, x.position, moved,
                            x.created_by, s.created_at, s.updated_at, s.updated_by, s.deleted_at, s.deleted_by],
                )?;
            }
            Action::Update(l) => {
                let (d, db) = merged_deleted(&l, s);
                tx.execute(
                    "UPDATE attachments SET project_id = ?2, note_id = ?3, entry_id = ?4, topic_id = ?5, epic_id = ?6,
                            name = ?7, position = ?8, moved_from_note_id = ?9, updated_at = ?10, updated_by = ?11,
                            deleted_at = ?12, deleted_by = ?13,
                            deleted_batch = CASE WHEN deleted_at IS ?12 THEN deleted_batch ELSE NULL END
                     WHERE id = ?1",
                    params![l.id, project, note, entry, topic, epic, name, x.position, moved,
                            s.updated_at, s.updated_by, d, db],
                )?;
            }
            Action::Skip => {}
        }
    }
    Ok(())
}
