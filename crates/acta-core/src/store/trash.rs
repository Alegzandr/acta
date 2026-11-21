//! Soft delete, the trash and restore (ADR 0006).
//!
//! Deleting marks the record and everything under it with the same
//! `deleted_at`, `deleted_by` and batch id; nothing is erased. Records keep their
//! parent and position, so a restore puts them back where they were, with
//! everything that was deleted together with them. The trash lists what was
//! deleted on its own, not the descendants that went with it. Records stay
//! restorable for 30 days; there is no purge and no "empty trash" in v1.

use super::Store;
use crate::error::{Error, Result};
use crate::model::{
    Author, EntryId, EpicId, EventKind, FolderId, KbId, NoteId, Page, PageReq, ProjectId, Record,
    TopicId,
};
use crate::time::{MS_PER_DAY, now_ms};
use rusqlite::{OptionalExtension, Transaction, params};

/// What identifies one delete: its batch id, or for records trashed
/// before batches existed (or imported trashed) their deletion time.
const BATCH: &str = "coalesce(deleted_batch, CAST(deleted_at AS TEXT))";

/// How long a record stays in the trash before a future purge may erase it.
pub const TRASH_RETENTION_MS: i64 = 30 * MS_PER_DAY;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashItem {
    pub record: Record,
    pub project: ProjectId,
    /// Title, or the start of the text for entries and notes.
    pub label: String,
    pub deleted_at: i64,
    pub deleted_by: Author,
    /// When the 30 days are over.
    pub expires_at: i64,
}

/// The tables under a record, as `(table, condition)` pairs on its local id.
fn cascade(record: Record) -> Vec<(&'static str, String)> {
    match record {
        Record::Project(ProjectId(id)) => [
            "projects",
            "epics",
            "topics",
            "notes",
            "kb_entries",
            "folders",
        ]
        .iter()
        .map(|t| {
            let col = if *t == "projects" { "id" } else { "project_id" };
            (*t, format!("{col} = {id}"))
        })
        .chain([(
            "entries",
            format!("topic_id IN (SELECT id FROM topics WHERE project_id = {id})"),
        )])
        .collect(),
        Record::Epic(EpicId(id)) => {
            let topics = format!("SELECT id FROM topics WHERE epic_id = {id}");
            vec![
                ("epics", format!("id = {id}")),
                ("topics", format!("epic_id = {id}")),
                ("entries", format!("topic_id IN ({topics})")),
                (
                    "kb_entries",
                    format!("epic_id = {id} OR topic_id IN ({topics})"),
                ),
                (
                    "folders",
                    format!("epic_id = {id} OR topic_id IN ({topics})"),
                ),
            ]
        }
        Record::Topic(TopicId(id)) => vec![
            ("topics", format!("id = {id}")),
            ("entries", format!("topic_id = {id}")),
            ("kb_entries", format!("topic_id = {id}")),
            ("folders", format!("topic_id = {id}")),
        ],
        Record::Entry(EntryId(id)) => vec![("entries", format!("id = {id}"))],
        Record::Note(NoteId(id)) => vec![("notes", format!("id = {id}"))],
        Record::Kb(KbId(id)) => vec![("kb_entries", format!("id = {id}"))],
        Record::Folder(FolderId(id)) => vec![("folders", format!("id = {id}"))],
    }
}

fn table_of(record: Record) -> (&'static str, i64) {
    match record {
        Record::Project(id) => ("projects", id.0),
        Record::Epic(id) => ("epics", id.0),
        Record::Topic(id) => ("topics", id.0),
        Record::Entry(id) => ("entries", id.0),
        Record::Note(id) => ("notes", id.0),
        Record::Kb(id) => ("kb_entries", id.0),
        Record::Folder(id) => ("folders", id.0),
    }
}

/// The parent that must be live for `record` to be restored, as
/// `(table, id)`, read from the record's row.
fn parent_of(tx: &Transaction<'_>, record: Record) -> Result<Option<(&'static str, i64, String)>> {
    let one = |sql: &str, id: i64| -> rusqlite::Result<(Option<i64>, Option<i64>, Option<i64>)> {
        tx.query_row(sql, [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    };
    let (p, e, t) = match record {
        Record::Project(_) => return Ok(None),
        Record::Epic(id) => one(
            "SELECT project_id, NULL, NULL FROM epics WHERE id = ?1",
            id.0,
        )?,
        Record::Topic(id) => one(
            "SELECT project_id, epic_id, NULL FROM topics WHERE id = ?1",
            id.0,
        )?,
        Record::Entry(id) => one(
            "SELECT NULL, NULL, topic_id FROM entries WHERE id = ?1",
            id.0,
        )?,
        Record::Note(id) => one(
            "SELECT project_id, NULL, NULL FROM notes WHERE id = ?1",
            id.0,
        )?,
        Record::Kb(id) => one(
            "SELECT project_id, epic_id, topic_id FROM kb_entries WHERE id = ?1",
            id.0,
        )?,
        Record::Folder(id) => one(
            "SELECT project_id, epic_id, topic_id FROM folders WHERE id = ?1",
            id.0,
        )?,
    };
    Ok(match (p, e, t) {
        (_, _, Some(t)) => Some(("topics", t, TopicId(t).to_string())),
        (_, Some(e), None) => Some(("epics", e, EpicId(e).to_string())),
        (Some(p), None, None) => Some(("projects", p, ProjectId(p).to_string())),
        _ => None,
    })
}

impl Store {
    /// Moves a record and everything under it to the trash.
    pub fn delete(&mut self, record: Record, author: Author) -> Result<()> {
        let (table, id) = table_of(record);
        let tx = self.write()?;
        let live: Option<i64> = tx
            .query_row(
                &format!("SELECT id FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if live.is_none() {
            return Err(Error::NotFound(record.to_string()));
        }
        let now = now_ms();
        // Everything this delete trashes shares one batch id, so a restore
        // brings back exactly that set, even within the same millisecond.
        let batch = super::new_uuid();
        for (t, cond) in cascade(record) {
            tx.execute(
                &format!(
                    "UPDATE {t} SET deleted_at = ?1, deleted_by = ?2, deleted_batch = ?3
                     WHERE ({cond}) AND deleted_at IS NULL"
                ),
                params![now, author, batch],
            )?;
        }
        tx.commit()?;
        self.refresh_link_index();
        Ok(())
    }

    /// Restores a trashed record where it was, with what was deleted with it.
    pub fn restore(&mut self, record: Record, author: Author) -> Result<()> {
        let (table, id) = table_of(record);
        let tx = self.write()?;
        let batch: Option<String> = tx
            .query_row(
                &format!("SELECT {BATCH} FROM {table} WHERE id = ?1"),
                [id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(record.to_string()))?;
        let Some(batch) = batch else {
            return Ok(());
        };
        if let Some((ptable, pid, name)) = parent_of(&tx, record)? {
            let parent_deleted: Option<i64> = tx.query_row(
                &format!("SELECT deleted_at FROM {ptable} WHERE id = ?1"),
                [pid],
                |r| r.get(0),
            )?;
            if parent_deleted.is_some() {
                return Err(Error::ParentDeleted(name));
            }
        }
        let now = now_ms();
        for (t, cond) in cascade(record) {
            tx.execute(
                &format!(
                    "UPDATE {t} SET deleted_at = NULL, deleted_by = NULL, deleted_batch = NULL,
                            updated_at = ?2, updated_by = ?3
                     WHERE ({cond}) AND deleted_at IS NOT NULL AND {BATCH} = ?1"
                ),
                params![batch, now, author],
            )?;
        }
        if let Record::Topic(t) = record {
            super::timeline::system_event(&tx, t, EventKind::Restored, None, None, author, now)?;
        }
        tx.commit()?;
        self.refresh_link_index();
        Ok(())
    }

    /// What was deleted on its own, newest first.
    pub fn trash(&self, page: PageReq) -> Result<Page<TrashItem>> {
        let (offset, limit) = page.clamped();
        let sql = "
            SELECT 'project', x.id, x.id, x.title, x.deleted_at, x.deleted_by FROM projects x
             WHERE x.deleted_at IS NOT NULL
            UNION ALL
            SELECT 'epic', x.id, x.project_id, x.title, x.deleted_at, x.deleted_by FROM epics x
             JOIN projects p ON p.id = x.project_id
             WHERE x.deleted_at IS NOT NULL AND coalesce(p.deleted_batch, CAST(p.deleted_at AS TEXT)) IS NOT coalesce(x.deleted_batch, CAST(x.deleted_at AS TEXT))
            UNION ALL
            SELECT 'topic', x.id, x.project_id, x.title, x.deleted_at, x.deleted_by FROM topics x
             JOIN projects p ON p.id = x.project_id LEFT JOIN epics e ON e.id = x.epic_id
             WHERE x.deleted_at IS NOT NULL AND coalesce(p.deleted_batch, CAST(p.deleted_at AS TEXT)) IS NOT coalesce(x.deleted_batch, CAST(x.deleted_at AS TEXT))
               AND coalesce(e.deleted_batch, CAST(e.deleted_at AS TEXT)) IS NOT coalesce(x.deleted_batch, CAST(x.deleted_at AS TEXT))
            UNION ALL
            SELECT 'entry', x.id, t.project_id, substr(x.text, 1, 80), x.deleted_at, x.deleted_by FROM entries x
             JOIN topics t ON t.id = x.topic_id
             WHERE x.deleted_at IS NOT NULL AND coalesce(t.deleted_batch, CAST(t.deleted_at AS TEXT)) IS NOT coalesce(x.deleted_batch, CAST(x.deleted_at AS TEXT))
            UNION ALL
            SELECT 'note', x.id, x.project_id, substr(x.text, 1, 80), x.deleted_at, x.deleted_by FROM notes x
             JOIN projects p ON p.id = x.project_id
             WHERE x.deleted_at IS NOT NULL AND coalesce(p.deleted_batch, CAST(p.deleted_at AS TEXT)) IS NOT coalesce(x.deleted_batch, CAST(x.deleted_at AS TEXT))
            UNION ALL
            SELECT 'kb', x.id, x.project_id, x.title, x.deleted_at, x.deleted_by FROM kb_entries x
             JOIN projects p ON p.id = x.project_id LEFT JOIN epics e ON e.id = x.epic_id
             LEFT JOIN topics t ON t.id = x.topic_id
             WHERE x.deleted_at IS NOT NULL AND (CASE WHEN x.topic_id IS NOT NULL THEN coalesce(t.deleted_batch, CAST(t.deleted_at AS TEXT))
                   WHEN x.epic_id IS NOT NULL THEN coalesce(e.deleted_batch, CAST(e.deleted_at AS TEXT)) ELSE coalesce(p.deleted_batch, CAST(p.deleted_at AS TEXT)) END) IS NOT coalesce(x.deleted_batch, CAST(x.deleted_at AS TEXT))
            UNION ALL
            SELECT 'folder', x.id, x.project_id, x.path, x.deleted_at, x.deleted_by FROM folders x
             JOIN projects p ON p.id = x.project_id LEFT JOIN epics e ON e.id = x.epic_id
             LEFT JOIN topics t ON t.id = x.topic_id
             WHERE x.deleted_at IS NOT NULL AND (CASE WHEN x.topic_id IS NOT NULL THEN coalesce(t.deleted_batch, CAST(t.deleted_at AS TEXT))
                   WHEN x.epic_id IS NOT NULL THEN coalesce(e.deleted_batch, CAST(e.deleted_at AS TEXT)) ELSE coalesce(p.deleted_batch, CAST(p.deleted_at AS TEXT)) END) IS NOT coalesce(x.deleted_batch, CAST(x.deleted_at AS TEXT))
            ORDER BY 5 DESC, 1, 2 DESC";
        let mut stmt = self.conn.prepare(sql)?;
        let all: Vec<TrashItem> = stmt
            .query_map([], |r| {
                let kind: String = r.get(0)?;
                let id: i64 = r.get(1)?;
                let record = match kind.as_str() {
                    "project" => Record::Project(ProjectId(id)),
                    "epic" => Record::Epic(EpicId(id)),
                    "topic" => Record::Topic(TopicId(id)),
                    "entry" => Record::Entry(EntryId(id)),
                    "note" => Record::Note(NoteId(id)),
                    "kb" => Record::Kb(KbId(id)),
                    _ => Record::Folder(FolderId(id)),
                };
                let deleted_at: i64 = r.get(4)?;
                Ok(TrashItem {
                    record,
                    project: ProjectId(r.get(2)?),
                    label: r.get(3)?,
                    deleted_at,
                    deleted_by: r.get::<_, Option<Author>>(5)?.unwrap_or(Author::Human),
                    expires_at: deleted_at + TRASH_RETENTION_MS,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let total = all.len() as i64;
        let items = all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(Page {
            items,
            total,
            offset,
            capped: false,
        })
    }
}
