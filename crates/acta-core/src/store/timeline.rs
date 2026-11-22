//! Timeline entries and the notes inbox.

use super::records::{TopicPatch, apply_topic_patch};
use super::{STAMP_COLS, Store, new_uuid, required, stamp};
use crate::error::{Error, Result};
use crate::model::{
    Author, Entry, EntryId, EntryKind, EpicId, EventKind, Note, NoteId, Page, PageReq, ProjectId,
    Record, Status, SystemEvent, TopicId,
};
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};

/// A timeline entry written by a person or an agent (ADR 0005 `log`), with
/// an optional state and status refresh in the same transaction.
#[derive(Clone, Debug)]
pub struct NewEntry {
    pub kind: EntryKind,
    pub text: String,
    pub state: Option<String>,
    pub status: Option<Status>,
}

#[derive(Clone, Debug, Default)]
pub struct EntryFilter {
    /// Empty means every kind except `System`, unless `include_system`.
    pub kinds: Vec<EntryKind>,
    pub include_system: bool,
    /// Only entries with a sequence number below this one (older).
    pub before_seq: Option<i64>,
}

const ENTRY_COLS: &str = "id, topic_id, seq, kind, author, text, event, old_value, new_value";

fn entry_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let event: Option<EventKind> = r.get(6)?;
    Ok(Entry {
        id: EntryId(r.get(0)?),
        topic: TopicId(r.get(1)?),
        seq: r.get(2)?,
        kind: r.get(3)?,
        author: r.get(4)?,
        text: r.get(5)?,
        event: match event {
            Some(kind) => Some(SystemEvent {
                kind,
                old: r.get(7)?,
                new: r.get(8)?,
            }),
            None => None,
        },
        stamp: stamp(r, 9)?,
    })
}

pub(super) const NOTE_COLS: &str =
    "id, project_id, text, author, filed_topic_id, filed_at, filed_epic_id, filed_by, report_contact_id,
    (SELECT c.name FROM contacts c WHERE c.id = report_contact_id),
    (SELECT c.deleted_at IS NOT NULL FROM contacts c WHERE c.id = report_contact_id)";

pub(super) fn note_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: NoteId(r.get(0)?),
        project: ProjectId(r.get(1)?),
        text: r.get(2)?,
        author: r.get(3)?,
        filed_topic: r.get::<_, Option<i64>>(4)?.map(TopicId),
        filed_at: r.get(5)?,
        filed_epic: r.get::<_, Option<i64>>(6)?.map(EpicId),
        filed_by: r.get(7)?,
        report_to: super::people::contact_ref(r, 8)?,
        stamp: stamp(r, 11)?,
    })
}

/// Writes a system entry: Acta's own record of a change (request 10).
pub(crate) fn system_event(
    tx: &rusqlite::Connection,
    topic: TopicId,
    kind: EventKind,
    old: Option<String>,
    new: Option<String>,
    author: Author,
    now: i64,
) -> Result<()> {
    tx.execute(
        "INSERT INTO entries (uuid, topic_id, kind, author, text, event, old_value, new_value,
                              created_at, updated_at, updated_by)
         VALUES (?1, ?2, 'system', ?3, '', ?4, ?5, ?6, ?7, ?7, ?3)",
        params![new_uuid(), topic.0, author, kind, old, new, now],
    )?;
    Ok(())
}

/// A write to a topic's timeline is a write to the topic: its Updated
/// value and author follow (components-records.md 5, Properties block).
pub(crate) fn touch_topic(
    tx: &rusqlite::Connection,
    topic: TopicId,
    author: Author,
    now: i64,
) -> Result<()> {
    tx.execute(
        "UPDATE topics SET updated_at = ?2, updated_by = ?3 WHERE id = ?1",
        params![topic.0, now, author],
    )?;
    Ok(())
}

impl Store {
    /// Appends an entry and returns it with its number in the topic.
    pub fn log(&mut self, topic: TopicId, new: NewEntry, author: Author) -> Result<Entry> {
        if new.kind == EntryKind::System {
            return Err(Error::Invalid("system entries are written by Acta".into()));
        }
        let text = required("text", &new.text)?;
        let tx = self.write()?;
        let cur = super::records::read_topic(&tx, topic)?;
        let now = now_ms();
        let seq: i64 = tx.query_row(
            "SELECT coalesce(max(seq), 0) + 1 FROM entries WHERE topic_id = ?1",
            [topic.0],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO entries (uuid, topic_id, seq, kind, author, text, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?5)",
            params![new_uuid(), topic.0, seq, new.kind, author, text, now],
        )?;
        let id = EntryId(tx.last_insert_rowid());
        if new.state.is_some() || new.status.is_some() {
            let patch = TopicPatch {
                state: new.state,
                status: new.status,
                ..TopicPatch::default()
            };
            // A log changes no epic: nothing is lifted.
            apply_topic_patch(&tx, &cur, patch, author, now)?;
        } else {
            touch_topic(&tx, topic, author, now)?;
        }
        tx.commit()?;
        self.entry(id)
    }

    /// Edits an entry's text and type in place (the entry menu's Edit and
    /// Change type). System events are written by Acta and refused. The
    /// previous type and text are kept in an `edited` system event of the
    /// same transaction, so the timeline stays append-only in substance.
    pub fn update_entry(
        &mut self,
        id: EntryId,
        text: &str,
        kind: EntryKind,
        author: Author,
    ) -> Result<()> {
        if kind == EntryKind::System {
            return Err(Error::Invalid("system entries are written by Acta".into()));
        }
        let text = required("text", text)?;
        let tx = self.write()?;
        let current = tx
            .query_row(
                &format!(
                    "SELECT {ENTRY_COLS}, {STAMP_COLS} FROM entries WHERE id = ?1 AND deleted_at IS NULL"
                ),
                [id.0],
                entry_row,
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.to_string()))?;
        if current.kind == EntryKind::System {
            return Err(Error::Invalid("system entries are written by Acta".into()));
        }
        if current.text == text && current.kind == kind {
            return Ok(());
        }
        let now = now_ms();
        let changed = tx.execute(
            "UPDATE entries SET text = ?2, kind = ?3, updated_at = ?4, updated_by = ?5
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.0, text, kind, now, author],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(id.to_string()));
        }
        let before = serde_json::json!({"kind": current.kind.as_str(), "text": current.text});
        system_event(
            &tx,
            current.topic,
            EventKind::Edited,
            Some(before.to_string()),
            current.seq.map(|n| n.to_string()),
            author,
            now,
        )?;
        touch_topic(&tx, current.topic, author, now)?;
        tx.commit()?;
        Ok(())
    }

    pub fn entry(&self, id: EntryId) -> Result<Entry> {
        self.conn
            .query_row(
                &format!("SELECT {ENTRY_COLS}, {STAMP_COLS} FROM entries WHERE id = ?1 AND deleted_at IS NULL"),
                [id.0],
                entry_row,
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(id.to_string()))
    }

    /// The entry `T-42#14`.
    pub fn entry_by_seq(&self, topic: TopicId, seq: i64) -> Result<Entry> {
        self.conn
            .query_row(
                &format!(
                    "SELECT {ENTRY_COLS}, {STAMP_COLS} FROM entries
                     WHERE topic_id = ?1 AND seq = ?2 AND deleted_at IS NULL ORDER BY id LIMIT 1"
                ),
                params![topic.0, seq],
                entry_row,
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("{topic}#{seq}")))
    }

    /// A page of a topic's timeline, newest first.
    pub fn entries(
        &self,
        topic: TopicId,
        filter: &EntryFilter,
        page: PageReq,
    ) -> Result<Page<Entry>> {
        let (offset, limit) = page.clamped();
        let mut cond = vec!["topic_id = ?1".to_owned(), "deleted_at IS NULL".to_owned()];
        if filter.kinds.is_empty() {
            if !filter.include_system {
                cond.push("kind <> 'system'".into());
            }
        } else {
            let list: Vec<String> = filter
                .kinds
                .iter()
                .map(|k| format!("'{}'", k.as_str()))
                .collect();
            cond.push(format!("kind IN ({})", list.join(", ")));
        }
        if let Some(before) = filter.before_seq {
            cond.push(format!("seq < {before}"));
        }
        let where_ = cond.join(" AND ");
        let total: i64 = self.conn.query_row(
            &format!("SELECT count(*) FROM entries WHERE {where_}"),
            [topic.0],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLS}, {STAMP_COLS} FROM entries WHERE {where_}
             ORDER BY created_at DESC, id DESC LIMIT {limit} OFFSET {offset}"
        ))?;
        let items = stmt
            .query_map([topic.0], entry_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Page {
            items,
            total,
            offset,
            capped: false,
        })
    }

    // ---- notes ----

    pub fn add_note(&mut self, project: ProjectId, text: &str, author: Author) -> Result<NoteId> {
        let text = required("text", text)?;
        let tx = self.write()?;
        super::records::read_project(&tx, project)?;
        let now = now_ms();
        tx.execute(
            "INSERT INTO notes (uuid, project_id, text, author, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?4)",
            params![new_uuid(), project.0, text, author, now],
        )?;
        let id = NoteId(tx.last_insert_rowid());
        tx.commit()?;
        Ok(id)
    }

    pub fn note(&self, id: NoteId) -> Result<Note> {
        read_note(&self.conn, id)
    }

    pub fn update_note(&mut self, id: NoteId, text: &str, author: Author) -> Result<()> {
        let text = required("text", text)?;
        let tx = self.write()?;
        read_note(&tx, id)?;
        tx.execute(
            "UPDATE notes SET text = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
            params![id.0, text, now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The project's notes not yet filed, newest first.
    pub fn inbox(&self, project: ProjectId, page: PageReq) -> Result<Page<Note>> {
        self.notes_where(
            project,
            "filed_at IS NULL",
            "created_at DESC, id DESC",
            page,
        )
    }

    /// The project's notes that left the inbox (filed or dismissed), most
    /// recently filed first.
    pub fn filed_notes(&self, project: ProjectId, page: PageReq) -> Result<Page<Note>> {
        self.notes_where(
            project,
            "filed_at IS NOT NULL",
            "filed_at DESC, id DESC",
            page,
        )
    }

    fn notes_where(
        &self,
        project: ProjectId,
        cond: &str,
        order: &str,
        page: PageReq,
    ) -> Result<Page<Note>> {
        let (offset, limit) = page.clamped();
        let where_ = format!("project_id = ?1 AND deleted_at IS NULL AND {cond}");
        let total: i64 = self.conn.query_row(
            &format!("SELECT count(*) FROM notes WHERE {where_}"),
            [project.0],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {NOTE_COLS}, {STAMP_COLS} FROM notes WHERE {where_}
             ORDER BY {order} LIMIT {limit} OFFSET {offset}"
        ))?;
        let items = stmt
            .query_map([project.0], note_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Page {
            items,
            total,
            offset,
            capped: false,
        })
    }

    /// Files a note as a new topic in one transaction: the topic is created
    /// (title and description from the caller) and the note filed under it.
    pub fn note_to_topic(
        &mut self,
        note: NoteId,
        new: super::NewTopic,
        author: Author,
    ) -> Result<TopicId> {
        self.atomically(|store| {
            let project = read_note(&store.conn, note)?.project;
            if project != new.project {
                return Err(Error::Invalid(format!(
                    "{note} belongs to another project than {}",
                    new.project
                )));
            }
            let topic = store.create_topic(new, author)?;
            store.file_notes(&[note], topic, author)?;
            Ok(topic)
        })
    }

    /// Puts a filed or dismissed note back in the inbox (the undo of
    /// filing it). What filing created stays: the topic, epic or entries.
    pub fn unfile_note(&mut self, id: NoteId, author: Author) -> Result<()> {
        let tx = self.write()?;
        read_note(&tx, id)?;
        tx.execute(
            "UPDATE notes SET filed_topic_id = NULL, filed_epic_id = NULL, filed_at = NULL,
                    filed_by = NULL, updated_at = ?2, updated_by = ?3 WHERE id = ?1",
            params![id.0, now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Marks open notes as turned into, or filed under, a topic of their
    /// project. The topic takes the first note's person when it has none
    /// (request 19).
    pub fn file_notes(&mut self, notes: &[NoteId], topic: TopicId, author: Author) -> Result<()> {
        let tx = self.write()?;
        let project = super::records::read_topic(&tx, topic)?.project;
        set_fate(
            &tx,
            notes,
            project,
            &topic.to_string(),
            Some(topic.0),
            None,
            author,
        )?;
        super::people::carry_person(&tx, notes, Record::Topic(topic), author, now_ms())?;
        tx.commit()?;
        Ok(())
    }

    /// Marks open notes as turned into an epic of their project.
    pub fn file_notes_in_epic(
        &mut self,
        notes: &[NoteId],
        epic: EpicId,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        let project = super::records::read_epic(&tx, epic)?.project;
        set_fate(
            &tx,
            notes,
            project,
            &epic.to_string(),
            None,
            Some(epic.0),
            author,
        )?;
        super::people::carry_person(&tx, notes, Record::Epic(epic), author, now_ms())?;
        tx.commit()?;
        Ok(())
    }

    /// Takes open notes out of the inbox without filing them anywhere
    /// (duplicates, done, noise). `unfile_note` brings one back.
    pub fn dismiss_notes(&mut self, notes: &[NoteId], author: Author) -> Result<()> {
        let Some(first) = notes.first() else {
            return Ok(());
        };
        let tx = self.write()?;
        let project = read_note(&tx, *first)?.project;
        set_fate(&tx, notes, project, &first.to_string(), None, None, author)?;
        tx.commit()?;
        Ok(())
    }

    /// Sets a note's fate exactly as it was (the undo of putting it back in
    /// the inbox): target, time and who filed it.
    pub fn restore_note_fate(&mut self, id: NoteId, before: &Note, author: Author) -> Result<()> {
        let tx = self.write()?;
        read_note(&tx, id)?;
        tx.execute(
            "UPDATE notes SET filed_topic_id = ?2, filed_epic_id = ?3, filed_at = ?4, filed_by = ?5,
                    updated_at = ?6, updated_by = ?7 WHERE id = ?1",
            params![
                id.0,
                before.filed_topic.map(|t| t.0),
                before.filed_epic.map(|e| e.0),
                before.filed_at,
                before.filed_by,
                now_ms(),
                author
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// Inside the caller's transaction: every note must be live, in `project`
/// and still open (read here, so a note filed by another process since the
/// caller looked is refused), then all are stamped.
fn set_fate(
    tx: &rusqlite::Connection,
    notes: &[NoteId],
    project: ProjectId,
    target: &str,
    topic: Option<i64>,
    epic: Option<i64>,
    author: Author,
) -> Result<()> {
    for id in notes {
        let note = read_note(tx, *id)?;
        if note.project != project {
            return Err(Error::Invalid(format!(
                "{id} belongs to another project than {target}"
            )));
        }
        if !note.is_open() {
            return Err(Error::Invalid(format!(
                "{id} is no longer in the inbox (filed or dismissed since)"
            )));
        }
    }
    let now = now_ms();
    for id in notes {
        tx.execute(
            "UPDATE notes SET filed_topic_id = ?2, filed_epic_id = ?3, filed_at = ?4, filed_by = ?5,
                    updated_at = ?4, updated_by = ?5 WHERE id = ?1",
            params![id.0, topic, epic, now, author],
        )?;
    }
    Ok(())
}

pub(crate) fn read_note(conn: &rusqlite::Connection, id: NoteId) -> Result<Note> {
    conn.query_row(
        &format!(
            "SELECT {NOTE_COLS}, {STAMP_COLS} FROM notes WHERE id = ?1 AND deleted_at IS NULL"
        ),
        [id.0],
        note_row,
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(id.to_string()))
}
