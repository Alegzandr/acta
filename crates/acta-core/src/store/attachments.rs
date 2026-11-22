//! Attachments (ADR 0012; components-attachments.md 5): files on notes,
//! timeline entries and the descriptions of topics and epics, stored inside
//! the encrypted database.
//!
//! Content is addressed by the SHA-256 of the plaintext: adding bytes Acta
//! already holds links the existing blob. A new file is read twice: once to
//! hash it (nothing written, so a duplicate costs no write), then again to
//! store it in 1 MiB chunks, each in its own short transaction, so no
//! transaction spans a file and memory stays bounded. The second read is
//! hashed too: a file that changed in between is refused. Images are read
//! whole once for their pixel size and thumbnail (at most 25 MB).
//!
//! Removing an attachment, or deleting its owner, trashes the row (ADR 0006
//! batches). [`Store::purge_attachments`] deletes rows whose 30 days are
//! over, then every blob no row names, and returns the pages to the file
//! system with `PRAGMA incremental_vacuum`.

use super::{STAMP_COLS, Store, new_uuid, stamp};
use crate::error::{Error, Result};
use crate::files;
use crate::model::{
    AttachOwner, Attachment, AttachmentId, Author, EntryId, EpicId, FileKind, NoteId, ProjectId,
    TopicId,
};
use crate::time::now_ms;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

/// One stored chunk.
pub const CHUNK_BYTES: usize = 1024 * 1024;
/// The largest file Acta takes: 25 MB (1 MB = 1,048,576 bytes, as sizes
/// are shown).
pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
/// Over this, the editor shows the snapshot notice once.
pub const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// The most files one owner holds (filing may carry more; adding refuses).
pub const MAX_FILES_PER_OWNER: i64 = 20;
/// Longest stored file name, in characters.
pub const NAME_MAX: usize = 255;
/// A blob still being written, or just looked up, by another process is
/// left alone this long.
const PENDING_GRACE_MS: i64 = 60 * 60 * 1000;

/// Where the bytes of a new attachment come from.
pub enum AttachSource {
    /// A file chosen, dropped or pasted from a file manager. Read twice,
    /// never kept open between calls.
    Path(PathBuf),
    /// Bytes already in memory (a pasted image).
    Bytes(Vec<u8>),
}

impl AttachSource {
    fn open(&self) -> std::io::Result<Box<dyn Read + '_>> {
        Ok(match self {
            AttachSource::Path(p) => Box::new(std::fs::File::open(p)?),
            AttachSource::Bytes(b) => Box::new(&b[..]),
        })
    }

    fn len(&self) -> std::io::Result<u64> {
        Ok(match self {
            AttachSource::Path(p) => std::fs::metadata(p)?.len(),
            AttachSource::Bytes(b) => b.len() as u64,
        })
    }
}

/// Bytes and files of the attachments in a scope; each blob counted once
/// (data.md 11: Settings "Attached files: 312 MB in 48 files", the export
/// checkbox).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileTotals {
    pub bytes: i64,
    pub files: i64,
}

/// What a purge removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Purged {
    pub rows: i64,
    pub blobs: i64,
    pub bytes: i64,
}

/// The columns [`attachment_row`] reads, on the alias `a` joined to the
/// blob `b`.
const COLS: &str = "a.id, a.project_id, a.note_id, a.entry_id, a.topic_id, a.epic_id, a.name,
    b.size, b.media_type, b.width, b.height, a.position, a.moved_from_note_id, a.created_by";

fn attachment_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Attachment> {
    let owner = owner_at(r, 2)?;
    let media_type: String = r.get(8)?;
    let w: Option<i64> = r.get(9)?;
    let h: Option<i64> = r.get(10)?;
    Ok(Attachment {
        id: AttachmentId(r.get(0)?),
        project: ProjectId(r.get(1)?),
        owner,
        name: r.get(6)?,
        size: r.get(7)?,
        kind: files::kind_of(&media_type),
        media_type,
        pixels: w.zip(h).map(|(w, h)| (w as u32, h as u32)),
        position: r.get(11)?,
        moved_from: r.get::<_, Option<i64>>(12)?.map(NoteId),
        created_by: r.get(13)?,
        stamp: stamp(r, 14)?,
    })
}

fn owner_at(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<AttachOwner> {
    let ids: [Option<i64>; 4] = [r.get(at)?, r.get(at + 1)?, r.get(at + 2)?, r.get(at + 3)?];
    Ok(match ids {
        [Some(n), ..] => AttachOwner::Note(NoteId(n)),
        [_, Some(e), ..] => AttachOwner::Entry(EntryId(e)),
        [_, _, Some(t), _] => AttachOwner::Topic(TopicId(t)),
        [_, _, _, Some(e)] => AttachOwner::Epic(EpicId(e)),
        // The CHECK constraint makes this unreachable.
        _ => AttachOwner::Note(NoteId(0)),
    })
}

/// `(column, id)` of an owner.
pub(crate) fn owner_column(owner: AttachOwner) -> (&'static str, i64) {
    match owner {
        AttachOwner::Note(id) => ("note_id", id.0),
        AttachOwner::Entry(id) => ("entry_id", id.0),
        AttachOwner::Topic(id) => ("topic_id", id.0),
        AttachOwner::Epic(id) => ("epic_id", id.0),
    }
}

/// The project of a live owner.
fn owner_project(conn: &Connection, owner: AttachOwner) -> Result<ProjectId> {
    let sql = match owner {
        AttachOwner::Note(_) => "SELECT project_id FROM notes WHERE id = ?1 AND deleted_at IS NULL",
        AttachOwner::Entry(_) => {
            "SELECT t.project_id FROM entries x JOIN topics t ON t.id = x.topic_id
             WHERE x.id = ?1 AND x.deleted_at IS NULL"
        }
        AttachOwner::Topic(_) => {
            "SELECT project_id FROM topics WHERE id = ?1 AND deleted_at IS NULL"
        }
        AttachOwner::Epic(_) => "SELECT project_id FROM epics WHERE id = ?1 AND deleted_at IS NULL",
    };
    conn.query_row(sql, [owner_column(owner).1], |r| r.get(0))
        .optional()?
        .map(ProjectId)
        .ok_or_else(|| Error::NotFound(owner.to_string()))
}

fn live_count(conn: &Connection, owner: AttachOwner) -> Result<i64> {
    let (col, id) = owner_column(owner);
    Ok(conn.query_row(
        &format!("SELECT count(*) FROM attachments WHERE {col} = ?1 AND deleted_at IS NULL"),
        [id],
        |r| r.get(0),
    )?)
}

fn next_position(conn: &Connection, owner: AttachOwner) -> Result<i64> {
    let (col, id) = owner_column(owner);
    Ok(conn.query_row(
        &format!("SELECT coalesce(max(position), 0) + 1 FROM attachments WHERE {col} = ?1"),
        [id],
        |r| r.get(0),
    )?)
}

/// A file name fit to store and show: control characters removed, at most
/// [`NAME_MAX`] characters with the extension kept, never empty.
pub fn clean_name(name: &str) -> String {
    let flat: String = name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = flat.trim();
    if t.is_empty() {
        return "file".into();
    }
    let n = t.chars().count();
    if n <= NAME_MAX {
        return t.to_owned();
    }
    // Keep the extension (up to 16 chars) and cut the stem.
    let ext: String = match t.rsplit_once('.') {
        Some((_, e)) if !e.is_empty() && e.chars().count() <= 16 => format!(".{e}"),
        _ => String::new(),
    };
    let keep = NAME_MAX - ext.chars().count() - 1;
    let stem: String = t.chars().take(keep).collect();
    format!("{stem}…{ext}")
}

/// A file stored ahead of its row: an editor's file before the note or
/// entry it goes with is saved (components-attachments.md 5, While
/// adding). New content is a pending blob, which no purge touches for an
/// hour; content Acta already held is only named. [`Store::attach_staged`]
/// gives it its row; [`Store::discard_staged`] drops what is still pending.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Staged {
    hash: String,
    size: u64,
    kind: FileKind,
    pixels: Option<(u32, u32)>,
    /// The pending blob this staging wrote (`None` when the content was
    /// already stored).
    blob: Option<i64>,
}

impl Staged {
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn kind(&self) -> FileKind {
        self.kind
    }

    pub fn pixels(&self) -> Option<(u32, u32)> {
        self.pixels
    }
}

/// The first pass over a file: its hash, size, first bytes, and for an
/// image its whole content (needed for the thumbnail).
struct Scan {
    hash: String,
    size: u64,
    head: Vec<u8>,
    whole: Option<Vec<u8>>,
}

fn too_large(size: u64) -> Error {
    Error::AttachmentTooLarge {
        size,
        limit: MAX_FILE_BYTES,
    }
}

fn scan(src: &AttachSource) -> Result<Scan> {
    let mut reader = src.open()?;
    let mut hasher = Sha256::new();
    let mut head = vec![0u8; files::SNIFF_BYTES];
    let mut got = 0;
    while got < head.len() {
        let n = reader.read(&mut head[got..])?;
        if n == 0 {
            break;
        }
        got += n;
    }
    head.truncate(got);
    hasher.update(&head);
    let mut size = got as u64;
    let mut whole = is_image(&head).then(|| head.clone());
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        // The file may grow while it is read: never past the limit.
        if size > MAX_FILE_BYTES {
            return Err(too_large(size));
        }
        hasher.update(&buf[..n]);
        if let Some(w) = whole.as_mut() {
            w.extend_from_slice(&buf[..n]);
        }
    }
    Ok(Scan {
        hash: hex(&hasher.finalize()),
        size,
        head,
        whole,
    })
}

fn is_image(head: &[u8]) -> bool {
    files::kind_of(files::sniff(head)) == crate::model::FileKind::Image
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Media type, pixel size and thumbnail of new content. An image that does
/// not decode is kept as binary.
fn describe(scan: &Scan) -> (&'static str, Option<files::ImageInfo>) {
    let media = files::sniff(&scan.head);
    if files::kind_of(media) == crate::model::FileKind::Image {
        match scan.whole.as_deref().and_then(files::image_info) {
            Some(info) => (media, Some(info)),
            None => (files::MEDIA_BINARY, None),
        }
    } else {
        (media, None)
    }
}

fn complete_blob(conn: &Connection, hash: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM attachment_blobs WHERE hash = ?1 AND complete = 1",
            [hash],
            |r| r.get(0),
        )
        .optional()?)
}

/// Deletes a blob and its chunks.
fn drop_blob(conn: &Connection, blob: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM attachment_chunks WHERE blob_id = ?1", [blob])?;
    conn.execute("DELETE FROM attachment_blobs WHERE id = ?1", [blob])?;
    Ok(())
}

/// Stores `bytes` as a complete blob inside the caller's transaction
/// (import), or links the blob that already holds them. Returns its id.
pub(crate) fn put_blob(conn: &Connection, bytes: &[u8]) -> Result<i64> {
    let hash = hash_of(bytes);
    if let Some(id) = complete_blob(conn, &hash)? {
        return Ok(id);
    }
    let head = bytes[..bytes.len().min(files::SNIFF_BYTES)].to_vec();
    let scan = Scan {
        hash: hash.clone(),
        size: bytes.len() as u64,
        whole: is_image(&head).then(|| bytes.to_vec()),
        head,
    };
    let (media, info) = describe(&scan);
    conn.execute(
        "INSERT INTO attachment_blobs (hash, size, media_type, width, height, thumbnail, complete, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
        params![
            hash,
            bytes.len() as i64,
            media,
            info.as_ref().map(|i| i.width),
            info.as_ref().map(|i| i.height),
            info.as_ref().map(|i| i.thumbnail.as_slice()),
            now_ms()
        ],
    )?;
    let blob = conn.last_insert_rowid();
    for (seq, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
        conn.execute(
            "INSERT INTO attachment_chunks (blob_id, seq, data) VALUES (?1, ?2, ?3)",
            params![blob, seq as i64, chunk],
        )?;
    }
    Ok(blob)
}

/// Inserts an attachment row at the end of its owner's strip.
fn insert_row(
    conn: &Connection,
    owner: AttachOwner,
    project: ProjectId,
    blob: i64,
    name: &str,
    author: Author,
) -> Result<AttachmentId> {
    let (col, id) = owner_column(owner);
    let now = now_ms();
    conn.execute(
        &format!(
            "INSERT INTO attachments (uuid, project_id, {col}, blob_id, name, position, created_by,
                                      created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?7)"
        ),
        params![
            new_uuid(),
            project.0,
            id,
            blob,
            name,
            next_position(conn, owner)?,
            author,
            now
        ],
    )?;
    Ok(AttachmentId(conn.last_insert_rowid()))
}

/// Reads a live attachment.
pub(crate) fn read_attachment_row(conn: &Connection, id: AttachmentId) -> Result<Attachment> {
    conn.query_row(
        &format!(
            "SELECT {COLS}, {} FROM attachments a JOIN attachment_blobs b ON b.id = a.blob_id
             WHERE a.id = ?1 AND a.deleted_at IS NULL",
            prefixed_stamp()
        ),
        [id.0],
        attachment_row,
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(id.to_string()))
}

fn prefixed_stamp() -> String {
    STAMP_COLS
        .split(", ")
        .map(|c| format!("a.{c}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Moves live attachments to another owner, at the end of its strip, in
/// the caller's transaction. `from_note` records the note they left when a
/// filing moves them; `None` clears it (the undo puts them back).
pub(crate) fn move_rows(
    conn: &Connection,
    ids: &[AttachmentId],
    to: AttachOwner,
    from_note: Option<NoteId>,
    author: Author,
) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let project = owner_project(conn, to)?;
    let (col, id) = owner_column(to);
    let now = now_ms();
    let mut moved = 0;
    for a in ids {
        let pos = next_position(conn, to)?;
        moved += conn.execute(
            &format!(
                "UPDATE attachments SET note_id = NULL, entry_id = NULL, topic_id = NULL, epic_id = NULL,
                        {col} = ?2, project_id = ?3, position = ?4, moved_from_note_id = ?5,
                        updated_at = ?6, updated_by = ?7
                 WHERE id = ?1 AND deleted_at IS NULL"
            ),
            params![a.0, id, project.0, pos, from_note.map(|n| n.0), now, author],
        )?;
    }
    Ok(moved)
}

/// The live attachments of an owner, in order.
pub(crate) fn owner_ids(conn: &Connection, owner: AttachOwner) -> Result<Vec<AttachmentId>> {
    let (col, id) = owner_column(owner);
    let mut stmt = conn.prepare(&format!(
        "SELECT id FROM attachments WHERE {col} = ?1 AND deleted_at IS NULL ORDER BY position, id"
    ))?;
    let ids = stmt
        .query_map([id], |r| Ok(AttachmentId(r.get(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

/// Filing (ADR 0012, 5): moves each note's live files to `to`, recording
/// the note they came from. Returns how many moved.
pub(crate) fn carry_from_notes(
    conn: &Connection,
    notes: &[NoteId],
    to: AttachOwner,
    author: Author,
) -> Result<usize> {
    let mut moved = 0;
    for n in notes {
        let ids = owner_ids(conn, AttachOwner::Note(*n))?;
        moved += move_rows(conn, &ids, to, Some(*n), author)?;
    }
    Ok(moved)
}

/// The undo of a filing: files moved from `note` that are still live go
/// back to it.
pub(crate) fn return_to_note(conn: &Connection, note: NoteId, author: Author) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id FROM attachments WHERE moved_from_note_id = ?1 AND deleted_at IS NULL
         ORDER BY position, id",
    )?;
    let ids: Vec<AttachmentId> = stmt
        .query_map([note.0], |r| Ok(AttachmentId(r.get(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    move_rows(conn, &ids, AttachOwner::Note(note), None, author)
}

/// Reads an attachment's content chunk by chunk: one row (1 MiB) in
/// memory at a time. Each chunk is its own query; a purge in between ends
/// the read early with an error.
pub struct AttachmentReader<'a> {
    conn: &'a Connection,
    blob: i64,
    next: i64,
    buf: Vec<u8>,
    pos: usize,
    left: u64,
}

impl Read for AttachmentReader<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos == self.buf.len() {
            if self.left == 0 {
                return Ok(0);
            }
            let data: Option<Vec<u8>> = self
                .conn
                .query_row(
                    "SELECT data FROM attachment_chunks WHERE blob_id = ?1 AND seq = ?2",
                    params![self.blob, self.next],
                    |r| r.get(0),
                )
                .optional()
                .map_err(std::io::Error::other)?;
            let data = data.ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "the file's content is no longer in the database",
                )
            })?;
            self.next += 1;
            self.buf = data;
            self.pos = 0;
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        self.left = self.left.saturating_sub(n as u64);
        Ok(n)
    }
}

impl Store {
    /// Attaches a file to a note, an entry, or a topic's or epic's
    /// description, as `name` (cleaned: [`clean_name`]). Runs off the UI
    /// thread: the file is read twice and stored in 1 MiB chunks, each its
    /// own transaction; `progress(done, total)` is called after each chunk.
    /// Same bytes as a stored file: linked, nothing written but the row.
    ///
    /// Refusals: [`Error::AttachmentTooLarge`] over 25 MB,
    /// [`Error::TooManyFiles`] when the owner already holds 20,
    /// [`Error::Io`] when the file cannot be read (moved, locked).
    pub fn attach(
        &mut self,
        owner: AttachOwner,
        name: &str,
        source: AttachSource,
        author: Author,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Attachment> {
        self.check_writable()?;
        let name = clean_name(name);
        let project = owner_project(&self.conn, owner)?;
        if live_count(&self.conn, owner)? >= MAX_FILES_PER_OWNER {
            return Err(Error::TooManyFiles {
                limit: MAX_FILES_PER_OWNER,
            });
        }
        let size = source.len()?;
        if size > MAX_FILE_BYTES {
            return Err(too_large(size));
        }
        let scan = scan(&source)?;
        // Content already stored is linked; new content is written as a
        // pending blob first. Either way the row goes in with one final
        // transaction that re-reads the blob, so a purge (which deletes
        // only blobs no row names) can never take it in between: if the
        // linked blob went meanwhile, the content is stored again.
        for _ in 0..2 {
            let pending = match complete_blob(&self.conn, &scan.hash)? {
                Some(_) => None,
                None => Some(self.store_pending(&source, &scan, progress)?),
            };
            #[cfg(test)]
            BEFORE_ROW.with(|f| {
                if let Some(f) = f.borrow_mut().as_mut() {
                    f(&self.conn)
                }
            });
            let done = self.finish_attach(owner, project, &name, &scan, pending, author);
            match done {
                Ok(Some(a)) => {
                    progress(scan.size, scan.size);
                    return Ok(a);
                }
                Ok(None) => continue,
                Err(e) => {
                    if let Some(blob) = pending
                        && let Ok(tx) = self.write()
                    {
                        // Best effort; the purge removes what this leaves.
                        let _ = drop_blob(&tx, blob).map(|_| tx.commit());
                    }
                    return Err(e);
                }
            }
        }
        Err(changed())
    }

    /// Reads and stores a file without giving it an owner yet (an editor's
    /// file while the note or entry is still being written). Same checks
    /// as [`Store::attach`] but the count, which [`Store::attach_staged`]
    /// makes; `progress(done, total)` after each stored chunk.
    pub fn stage(
        &mut self,
        source: &AttachSource,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Staged> {
        let cell = std::cell::RefCell::new(self);
        stage_with(
            || std::cell::RefMut::map(cell.borrow_mut(), |s| &mut **s),
            source,
            progress,
        )
    }

    /// Keeps the pending blobs of files open editors still hold: their
    /// age restarts, so no purge in another process takes them (the app
    /// calls it every 30 minutes and when the record is saved).
    pub fn touch_staged(&mut self, staged: &[Staged]) -> Result<()> {
        let ids: Vec<String> = staged
            .iter()
            .filter_map(|s| s.blob)
            .map(|b| b.to_string())
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let tx = self.write()?;
        tx.execute(
            &format!(
                "UPDATE attachment_blobs SET created_at = ?1 WHERE complete = 0 AND id IN ({})",
                ids.join(",")
            ),
            [now_ms()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Gives a staged file its row on `owner`, as `name`, in one
    /// transaction with the blob it names: the stored content with its
    /// hash if there is one (a pending blob of this staging is then
    /// dropped), else the pending blob, completed here. Refused with
    /// [`Error::TooManyFiles`] when the owner holds 20, and as a changed
    /// file when the pending blob is gone (purged after an hour unsaved).
    pub fn attach_staged(
        &mut self,
        owner: AttachOwner,
        name: &str,
        staged: &Staged,
        author: Author,
    ) -> Result<Attachment> {
        self.check_writable()?;
        let name = clean_name(name);
        let tx = self.write()?;
        let project = owner_project(&tx, owner)?;
        if live_count(&tx, owner)? >= MAX_FILES_PER_OWNER {
            return Err(Error::TooManyFiles {
                limit: MAX_FILES_PER_OWNER,
            });
        }
        let ours = match staged.blob {
            Some(b) => tx
                .query_row(
                    "SELECT id FROM attachment_blobs WHERE id = ?1 AND complete = 0",
                    [b],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?,
            None => None,
        };
        let blob = match (complete_blob(&tx, &staged.hash)?, ours) {
            (Some(theirs), Some(ours)) => {
                drop_blob(&tx, ours)?;
                theirs
            }
            (Some(theirs), None) => theirs,
            (None, Some(ours)) => {
                tx.execute(
                    "UPDATE attachment_blobs SET hash = ?2, complete = 1 WHERE id = ?1",
                    params![ours, staged.hash],
                )?;
                ours
            }
            (None, None) => return Err(Error::StagedGone),
        };
        let id = insert_row(&tx, owner, project, blob, &name, author)?;
        let a = read_attachment_row(&tx, id)?;
        tx.commit()?;
        if let Some(b) = staged.blob {
            self.held.remove(&b);
        }
        Ok(a)
    }

    /// Drops the pending blob of a staged file that will not be saved (the
    /// editor was cancelled). A blob that was completed meanwhile, or that
    /// the file only named, is left alone.
    pub fn discard_staged(&mut self, staged: &Staged) -> Result<()> {
        let Some(blob) = staged.blob else {
            return Ok(());
        };
        self.held.remove(&blob);
        let tx = self.write()?;
        let pending: Option<i64> = tx
            .query_row(
                "SELECT id FROM attachment_blobs WHERE id = ?1 AND complete = 0",
                [blob],
                |r| r.get(0),
            )
            .optional()?;
        if pending.is_some() {
            drop_blob(&tx, blob)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The thumbnail stored with a staged image (the editor's tile before
    /// the note is saved).
    pub fn staged_thumbnail(&self, staged: &Staged) -> Result<Option<Vec<u8>>> {
        let by_blob = match staged.blob {
            Some(b) => self
                .conn
                .query_row(
                    "SELECT thumbnail FROM attachment_blobs WHERE id = ?1",
                    [b],
                    |r| r.get::<_, Option<Vec<u8>>>(0),
                )
                .optional()?
                .flatten(),
            None => None,
        };
        if by_blob.is_some() {
            return Ok(by_blob);
        }
        Ok(self
            .conn
            .query_row(
                "SELECT thumbnail FROM attachment_blobs WHERE hash = ?1 AND complete = 1",
                [&staged.hash],
                |r| r.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()?
            .flatten())
    }

    /// The owner of an attachment, live or in the trash (the Trash row's
    /// place).
    pub fn attachment_owner(&self, id: AttachmentId) -> Result<AttachOwner> {
        self.conn
            .query_row(
                "SELECT note_id, entry_id, topic_id, epic_id FROM attachments WHERE id = ?1",
                [id.0],
                |r| owner_at(r, 0),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.to_string()))
    }

    /// The row, in one transaction with the blob it names: the stored blob
    /// with this hash if there is one (a pending blob of ours is then
    /// dropped), else ours, completed here. `None` when there is neither
    /// (the linked blob was purged since it was looked up).
    fn finish_attach(
        &mut self,
        owner: AttachOwner,
        project: ProjectId,
        name: &str,
        scan: &Scan,
        pending: Option<i64>,
        author: Author,
    ) -> Result<Option<Attachment>> {
        let tx = self.write()?;
        owner_project(&tx, owner)?;
        if live_count(&tx, owner)? >= MAX_FILES_PER_OWNER {
            return Err(Error::TooManyFiles {
                limit: MAX_FILES_PER_OWNER,
            });
        }
        let blob = match (complete_blob(&tx, &scan.hash)?, pending) {
            (Some(theirs), Some(ours)) => {
                drop_blob(&tx, ours)?;
                theirs
            }
            (Some(theirs), None) => theirs,
            (None, Some(ours)) => {
                tx.execute(
                    "UPDATE attachment_blobs SET hash = ?2, complete = 1 WHERE id = ?1",
                    params![ours, scan.hash],
                )?;
                ours
            }
            (None, None) => return Ok(None),
        };
        let id = insert_row(&tx, owner, project, blob, name, author)?;
        let a = read_attachment_row(&tx, id)?;
        tx.commit()?;
        Ok(Some(a))
    }

    /// The second pass: chunks written one transaction each under a
    /// pending hash. The blob stays pending (no purge touches it for an
    /// hour) until [`Store::finish_attach`] completes it with its row.
    fn store_pending(
        &mut self,
        source: &AttachSource,
        scan: &Scan,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<i64> {
        let (media, info) = describe(scan);
        let blob = insert_pending(self, scan, media, info.as_ref())?;
        if let Err(e) = self.write_chunks(blob, source, scan, progress) {
            if let Ok(tx) = self.write() {
                let _ = drop_blob(&tx, blob).map(|_| tx.commit());
            }
            return Err(e);
        }
        Ok(blob)
    }

    fn write_chunks(
        &mut self,
        blob: i64,
        source: &AttachSource,
        scan: &Scan,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<()> {
        let cell = std::cell::RefCell::new(self);
        write_chunks_with(
            &mut || std::cell::RefMut::map(cell.borrow_mut(), |s| &mut **s),
            blob,
            source,
            scan,
            progress,
        )
    }
}

/// A new pending blob for `scan`, in its own transaction.
fn insert_pending(
    store: &mut Store,
    scan: &Scan,
    media: &str,
    info: Option<&files::ImageInfo>,
) -> Result<i64> {
    let pending = format!("pending:{}", new_uuid());
    let tx = store.write()?;
    tx.execute(
        "INSERT INTO attachment_blobs (hash, size, media_type, width, height, thumbnail, complete, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![
            pending,
            scan.size as i64,
            media,
            info.map(|i| i.width),
            info.map(|i| i.height),
            info.map(|i| i.thumbnail.as_slice()),
            now_ms()
        ],
    )?;
    let blob = tx.last_insert_rowid();
    tx.commit()?;
    Ok(blob)
}

/// [`Store::stage`] for a store shared behind a lock: `lock` is taken for
/// each step and released in between, so the file is read and hashed
/// without it and other reads of the store run between its 1 MiB chunks.
/// The pending blob is held by this store until it is attached or
/// discarded: no purge of this process takes it.
pub fn stage_with<G: std::ops::DerefMut<Target = Store>>(
    mut lock: impl FnMut() -> G,
    source: &AttachSource,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Staged> {
    lock().check_writable()?;
    let size = source.len()?;
    if size > MAX_FILE_BYTES {
        return Err(too_large(size));
    }
    let scan = scan(source)?;
    let stored: Option<(String, Option<i64>, Option<i64>)> = lock()
        .conn
        .query_row(
            "SELECT media_type, width, height FROM attachment_blobs
             WHERE hash = ?1 AND complete = 1",
            [&scan.hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((media, w, h)) = stored {
        progress(scan.size, scan.size);
        return Ok(Staged {
            hash: scan.hash.clone(),
            size: scan.size,
            kind: files::kind_of(&media),
            pixels: w.zip(h).map(|(w, h)| (w as u32, h as u32)),
            blob: None,
        });
    }
    // The thumbnail is made without the lock.
    let (media, info) = describe(&scan);
    let blob = {
        let mut store = lock();
        let blob = insert_pending(&mut store, &scan, media, info.as_ref())?;
        store.held.insert(blob);
        blob
    };
    if let Err(e) = write_chunks_with(&mut lock, blob, source, &scan, progress) {
        let mut store = lock();
        store.held.remove(&blob);
        if let Ok(tx) = store.write() {
            let _ = drop_blob(&tx, blob).map(|_| tx.commit());
        }
        return Err(e);
    }
    progress(scan.size, scan.size);
    Ok(Staged {
        hash: scan.hash.clone(),
        size: scan.size,
        kind: files::kind_of(media),
        pixels: info.map(|i| (i.width, i.height)),
        blob: Some(blob),
    })
}

/// The second pass over a file: its chunks, each in its own transaction
/// with the lock taken for that chunk only; re-hashed, so a file that
/// changed since the first pass is refused.
fn write_chunks_with<G: std::ops::DerefMut<Target = Store>>(
    lock: &mut impl FnMut() -> G,
    blob: i64,
    source: &AttachSource,
    scan: &Scan,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<()> {
    {
        let mut reader = source.open()?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; CHUNK_BYTES];
        let mut done = 0u64;
        let mut seq = 0i64;
        loop {
            // Fill a whole chunk (a reader may return less per call).
            let mut filled = 0;
            while filled < CHUNK_BYTES {
                let n = reader.read(&mut buf[filled..])?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
            if filled == 0 {
                break;
            }
            done += filled as u64;
            if done > scan.size {
                return Err(changed());
            }
            hasher.update(&buf[..filled]);
            {
                let mut store = lock();
                let tx = store.write()?;
                tx.execute(
                    "INSERT INTO attachment_chunks (blob_id, seq, data) VALUES (?1, ?2, ?3)",
                    params![blob, seq, &buf[..filled]],
                )?;
                tx.commit()?;
            }
            seq += 1;
            progress(done, scan.size);
            if filled < CHUNK_BYTES {
                break;
            }
        }
        if done != scan.size || hex(&hasher.finalize()) != scan.hash {
            return Err(changed());
        }
        Ok(())
    }
}

impl Store {
    /// A live attachment, without its content.
    pub fn attachment(&self, id: AttachmentId) -> Result<Attachment> {
        read_attachment_row(&self.conn, id)
    }

    /// An owner's live attachments, in the order they were added.
    pub fn attachments(&self, owner: AttachOwner) -> Result<Vec<Attachment>> {
        let (col, id) = owner_column(owner);
        self.attachments_where(&format!("a.{col} = {id}"))
    }

    /// The live attachments a filing moved from `note` (the Filed list's
    /// chips and "Moved to T-51").
    pub fn attachments_moved_from(&self, note: NoteId) -> Result<Vec<Attachment>> {
        self.attachments_where(&format!("a.moved_from_note_id = {}", note.0))
    }

    /// The live attachments of many owners at once (agent list lines).
    pub fn attachments_of(
        &self,
        owners: &[AttachOwner],
    ) -> Result<HashMap<AttachOwner, Vec<Attachment>>> {
        let mut out: HashMap<AttachOwner, Vec<Attachment>> = HashMap::new();
        if owners.is_empty() {
            return Ok(out);
        }
        let mut by_col: HashMap<&str, Vec<String>> = HashMap::new();
        for o in owners {
            let (col, id) = owner_column(*o);
            by_col.entry(col).or_default().push(id.to_string());
        }
        let cond = by_col
            .iter()
            .map(|(col, ids)| format!("a.{col} IN ({})", ids.join(",")))
            .collect::<Vec<_>>()
            .join(" OR ");
        for a in self.attachments_where(&format!("({cond})"))? {
            out.entry(a.owner).or_default().push(a);
        }
        Ok(out)
    }

    fn attachments_where(&self, cond: &str) -> Result<Vec<Attachment>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLS}, {} FROM attachments a JOIN attachment_blobs b ON b.id = a.blob_id
             WHERE {cond} AND a.deleted_at IS NULL ORDER BY a.position, a.id",
            prefixed_stamp()
        ))?;
        let rows = stmt
            .query_map([], attachment_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// The content hash of a live attachment: what caches of its derived
    /// text are keyed by (never shown).
    pub fn attachment_hash(&self, id: AttachmentId) -> Result<String> {
        self.conn
            .query_row(
                "SELECT b.hash FROM attachments a JOIN attachment_blobs b ON b.id = a.blob_id
                 WHERE a.id = ?1 AND a.deleted_at IS NULL AND b.complete = 1",
                [id.0],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.to_string()))
    }

    /// The stored PNG thumbnail of an image attachment.
    pub fn attachment_thumbnail(&self, id: AttachmentId) -> Result<Option<Vec<u8>>> {
        self.conn
            .query_row(
                "SELECT b.thumbnail FROM attachments a JOIN attachment_blobs b ON b.id = a.blob_id
                 WHERE a.id = ?1 AND a.deleted_at IS NULL",
                [id.0],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.to_string()))
    }

    /// A reader over a live attachment's content, decrypted in memory a
    /// chunk at a time (the viewer, Save as, the agent).
    pub fn read_attachment(&self, id: AttachmentId) -> Result<AttachmentReader<'_>> {
        let (blob, size): (i64, i64) = self
            .conn
            .query_row(
                "SELECT b.id, b.size FROM attachments a JOIN attachment_blobs b ON b.id = a.blob_id
                 WHERE a.id = ?1 AND a.deleted_at IS NULL AND b.complete = 1",
                [id.0],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.to_string()))?;
        Ok(AttachmentReader {
            conn: &self.conn,
            blob,
            next: 0,
            buf: Vec::new(),
            pos: 0,
            left: size as u64,
        })
    }

    /// At most the first `max` bytes of an attachment (the text viewer's
    /// first 1 MB; the agent's reads).
    pub fn attachment_prefix(&self, id: AttachmentId, max: usize) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.read_attachment(id)?
            .take(max as u64)
            .read_to_end(&mut out)?;
        Ok(out)
    }

    /// Moves files to another owner, at the end of its strip (the undo of
    /// an undo; filing moves files itself).
    pub fn move_attachments(
        &mut self,
        ids: &[AttachmentId],
        to: AttachOwner,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        move_rows(&tx, ids, to, None, author)?;
        tx.commit()?;
        Ok(())
    }

    /// Bytes (each blob once) and files of the attachments that are not
    /// purged, in the trash included: Settings (`None`) and the export
    /// checkbox (a project, or `None` for everything).
    pub fn attachment_totals(&self, project: Option<ProjectId>) -> Result<FileTotals> {
        let only = project.map_or(String::new(), |p| format!(" WHERE a.project_id = {}", p.0));
        let (bytes, files) = self.conn.query_row(
            &format!(
                "SELECT coalesce((SELECT sum(size) FROM attachment_blobs WHERE id IN
                            (SELECT a.blob_id FROM attachments a{only})), 0),
                        (SELECT count(*) FROM attachments a{only})"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(FileTotals { bytes, files })
    }

    /// Deletes trashed attachment rows older than the trash retention,
    /// then every blob no row names (live or in the trash) and blobs left
    /// half-written for over an hour, in one transaction; then returns the
    /// freed pages with `PRAGMA incremental_vacuum` (ADR 0012, 4).
    pub fn purge_attachments(&mut self, now: i64) -> Result<Purged> {
        let cutoff = now - super::TRASH_RETENTION_MS;
        // Blobs an open editor of this process holds stay too.
        let held = if self.held.is_empty() {
            String::new()
        } else {
            let ids: Vec<String> = self.held.iter().map(|b| b.to_string()).collect();
            format!(" AND b.id NOT IN ({})", ids.join(","))
        };
        let tx = self.write()?;
        let rows = tx.execute(
            "DELETE FROM attachments WHERE deleted_at IS NOT NULL AND deleted_at <= ?1",
            [cutoff],
        )? as i64;
        // Blobs younger than the grace stay, complete or not: one may be
        // between its lookup and its row in another process.
        let orphans = format!(
            "SELECT id FROM attachment_blobs b
             WHERE NOT EXISTS (SELECT 1 FROM attachments a WHERE a.blob_id = b.id)
               AND b.created_at <= ?1{held}"
        );
        let orphans = orphans.as_str();
        let (blobs, bytes): (i64, i64) = tx.query_row(
            &format!(
                "SELECT count(*), coalesce(sum(size), 0) FROM attachment_blobs WHERE id IN ({orphans})"
            ),
            [now - PENDING_GRACE_MS],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        tx.execute(
            &format!("DELETE FROM attachment_chunks WHERE blob_id IN ({orphans})"),
            [now - PENDING_GRACE_MS],
        )?;
        tx.execute(
            &format!("DELETE FROM attachment_blobs WHERE id IN ({orphans})"),
            [now - PENDING_GRACE_MS],
        )?;
        tx.commit()?;
        if self.conn.is_autocommit() && (rows > 0 || blobs > 0) {
            // Returns every free page; nothing to undo if it fails. Each
            // step of the statement frees one page: run it to the end.
            let _ = incremental_vacuum(&self.conn);
        }
        Ok(Purged { rows, blobs, bytes })
    }
}

fn incremental_vacuum(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare("PRAGMA incremental_vacuum")?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

fn changed() -> Error {
    Error::Io(std::io::Error::other(
        "the file changed while it was being added",
    ))
}

/// The SHA-256 of `bytes`, lowercase hex, as blobs are keyed.
pub(crate) fn hash_of(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[cfg(test)]
pub(crate) type RowHook = Option<Box<dyn FnMut(&Connection)>>;

#[cfg(test)]
thread_local! {
    /// Runs between the blob lookup and the row's transaction (tests of
    /// the attach and purge race).
    pub(crate) static BEFORE_ROW: std::cell::RefCell<RowHook> =
        std::cell::RefCell::new(None);
}
