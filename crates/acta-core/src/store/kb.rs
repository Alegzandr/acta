//! Knowledge entries (ADR 0007): documents and memories on a project, an
//! epic or a topic, with full revisions, the scope an agent looks up and the
//! roll-up a person looks down.

use super::{STAMP_COLS, Store, new_uuid, owner_at, owner_condition, owner_ids, required, stamp};
use crate::error::{Error, Result};
use crate::markdown;
use crate::model::{
    Author, ChangeKind, KbEntry, KbId, KbKind, KbRevision, KbSummary, Level, Owner, Page, PageReq,
    ProjectId,
};
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};

#[derive(Clone, Debug)]
pub struct NewKb {
    pub owner: Owner,
    pub kind: KbKind,
    pub title: String,
    /// One line; required: it is how an agent decides what to open.
    pub summary: String,
    pub body: String,
    /// Created from a `.md` or `.txt` file (shown in the history).
    pub from_file: bool,
}

/// How a save changes the body.
#[derive(Clone, Debug)]
pub enum KbEdit {
    /// Title or summary only.
    Keep,
    /// The whole body (the editor's save).
    Body(String),
    /// One section by outline number, optionally renamed (ADR 0005).
    ReplaceSection {
        number: String,
        heading: Option<String>,
        text: String,
    },
    AppendSection {
        heading: String,
        text: String,
    },
}

#[derive(Clone, Debug)]
pub struct KbUpdate {
    /// The revision the writer read; a different current one is refused.
    pub expected_rev: Option<i64>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub edit: KbEdit,
}

#[derive(Clone, Debug, Default)]
pub struct KbFilter {
    pub kind: Option<KbKind>,
    pub created_by: Option<Author>,
    pub level: Option<Level>,
}

/// One row of a history list: no body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KbRevisionSummary {
    pub rev: i64,
    pub author: Author,
    pub change: ChangeKind,
    pub section: Option<String>,
    pub size: i64,
    pub created_at: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KbCounts {
    pub documents: i64,
    pub memories: i64,
}

const SUMMARY_COLS: &str =
    "k.id, k.kind, k.project_id, k.epic_id, k.topic_id, k.title, k.summary, k.rev,
    length(CAST(k.body AS BLOB)), k.created_by, k.updated_at, k.updated_by";

fn summary_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<KbSummary> {
    Ok(KbSummary {
        id: KbId(r.get(0)?),
        kind: r.get(1)?,
        owner: owner_at(r, 2)?,
        title: r.get(5)?,
        summary: r.get(6)?,
        rev: r.get(7)?,
        size: r.get(8)?,
        created_by: r.get(9)?,
        updated_at: r.get(10)?,
        updated_by: r.get(11)?,
    })
}

const LEVEL_RANK: &str =
    "CASE WHEN k.topic_id IS NOT NULL THEN 0 WHEN k.epic_id IS NOT NULL THEN 1 ELSE 2 END";

impl Store {
    pub fn kb_create(&mut self, new: NewKb, author: Author) -> Result<KbId> {
        let title = required("title", &new.title)?;
        let summary = required("summary", &new.summary)?;
        let change = if new.from_file {
            ChangeKind::CreatedFromFile
        } else {
            ChangeKind::Created
        };
        let tx = self.write()?;
        let (project, epic, topic) = owner_ids(&tx, new.owner)?;
        let now = now_ms();
        tx.execute(
            "INSERT INTO kb_entries (uuid, project_id, epic_id, topic_id, kind, title, summary, body, rev,
                                     created_by, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?10, ?9)",
            params![new_uuid(), project, epic, topic, new.kind, title, summary, new.body, author, now],
        )?;
        let id = KbId(tx.last_insert_rowid());
        insert_revision(
            &tx, id, 1, &title, &summary, &new.body, author, change, None, now,
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn kb_entry(&self, id: KbId) -> Result<KbEntry> {
        read_kb(&self.conn, id)
    }

    /// Saves a new revision. The previous one stays in the history.
    /// Returns the new revision number.
    pub fn kb_update(&mut self, id: KbId, update: KbUpdate, author: Author) -> Result<i64> {
        // Read inside the write transaction: the revision check and the
        // write are atomic against other processes.
        let tx = self.write()?;
        let cur = read_kb(&tx, id)?;
        if let Some(expected) = update.expected_rev
            && expected != cur.rev
        {
            return Err(Error::StaleRevision { current: cur.rev });
        }
        let title = match &update.title {
            Some(t) => required("title", t)?,
            None => cur.title.clone(),
        };
        let summary = match &update.summary {
            Some(s) => required("summary", s)?,
            None => cur.summary.clone(),
        };
        let (body, change, section) = match update.edit {
            KbEdit::Keep => (cur.body.clone(), ChangeKind::Edited, None),
            KbEdit::Body(body) => (body, ChangeKind::Edited, None),
            KbEdit::ReplaceSection {
                number,
                heading,
                text,
            } => {
                let (body, title) =
                    markdown::replace_section(&cur.body, &number, heading.as_deref(), &text)
                        .ok_or_else(|| Error::NotFound(format!("section {number} of {id}")))?;
                (body, ChangeKind::ReplacedSection, Some(title))
            }
            KbEdit::AppendSection { heading, text } => {
                let heading = required("heading", &heading)?;
                let body = markdown::append_section(&cur.body, &heading, &text);
                (body, ChangeKind::AppendedSection, Some(heading))
            }
        };
        if title == cur.title && summary == cur.summary && body == cur.body {
            return Ok(cur.rev);
        }
        let rev = next_rev(&tx, id)?;
        let now = now_ms();
        tx.execute(
            "UPDATE kb_entries SET title = ?2, summary = ?3, body = ?4, rev = ?5, updated_at = ?6, updated_by = ?7
             WHERE id = ?1",
            params![id.0, title, summary, body, rev, now, author],
        )?;
        insert_revision(
            &tx,
            id,
            rev,
            &title,
            &summary,
            &body,
            author,
            change,
            section.as_deref(),
            now,
        )?;
        tx.commit()?;
        Ok(rev)
    }

    /// Files an entry at another level (ADR 0007: the human moves what the
    /// agent filed). Recorded as a revision.
    pub fn kb_move(&mut self, id: KbId, owner: Owner, author: Author) -> Result<i64> {
        let tx = self.write()?;
        let (project, epic, topic) = owner_ids(&tx, owner)?;
        let cur = read_kb(&tx, id)?;
        if cur.owner == owner {
            return Ok(cur.rev);
        }
        let rev = next_rev(&tx, id)?;
        let now = now_ms();
        tx.execute(
            "UPDATE kb_entries SET project_id = ?2, epic_id = ?3, topic_id = ?4, rev = ?5,
                    updated_at = ?6, updated_by = ?7 WHERE id = ?1",
            params![id.0, project, epic, topic, rev, now, author],
        )?;
        insert_revision(
            &tx,
            id,
            rev,
            &cur.title,
            &cur.summary,
            &cur.body,
            author,
            ChangeKind::Moved,
            None,
            now,
        )?;
        tx.commit()?;
        Ok(rev)
    }

    /// A page of an entry's revisions, newest first, without bodies.
    pub fn kb_history(&self, id: KbId, page: PageReq) -> Result<Page<KbRevisionSummary>> {
        self.kb_entry(id)?;
        let (offset, limit) = page.clamped();
        let total: i64 = self.conn.query_row(
            "SELECT count(*) FROM kb_revisions WHERE entry_id = ?1",
            [id.0],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT rev, author, change, section, length(CAST(body AS BLOB)), created_at
             FROM kb_revisions WHERE entry_id = ?1 ORDER BY rev DESC LIMIT {limit} OFFSET {offset}"
        ))?;
        let rows = stmt.query_map([id.0], |r| {
            Ok(KbRevisionSummary {
                rev: r.get(0)?,
                author: r.get(1)?,
                change: r.get(2)?,
                section: r.get(3)?,
                size: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(Page {
            items: rows.collect::<rusqlite::Result<_>>()?,
            total,
            offset,
            capped: false,
        })
    }

    pub fn kb_revision(&self, id: KbId, rev: i64) -> Result<KbRevision> {
        self.conn
            .query_row(
                "SELECT uuid, rev, title, summary, body, author, change, section, created_at
                 FROM kb_revisions WHERE entry_id = ?1 AND rev = ?2",
                params![id.0, rev],
                revision_row,
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("{id} rev {rev}")))
    }

    /// Makes an old revision current again, as a new revision.
    pub fn kb_restore_revision(&mut self, id: KbId, rev: i64, author: Author) -> Result<i64> {
        let old = self.kb_revision(id, rev)?;
        let tx = self.write()?;
        read_kb(&tx, id)?;
        let new_rev = next_rev(&tx, id)?;
        let now = now_ms();
        tx.execute(
            "UPDATE kb_entries SET title = ?2, summary = ?3, body = ?4, rev = ?5, updated_at = ?6, updated_by = ?7
             WHERE id = ?1",
            params![id.0, old.title, old.summary, old.body, new_rev, now, author],
        )?;
        insert_revision(
            &tx,
            id,
            new_rev,
            &old.title,
            &old.summary,
            &old.body,
            author,
            ChangeKind::Restored,
            Some(&format!("rev {rev}")),
            now,
        )?;
        tx.commit()?;
        Ok(new_rev)
    }

    /// What an agent sees from `owner`, looking up: the owner's entries, then
    /// its epic's, then its project's; newest first inside a level.
    pub fn kb_scope(
        &self,
        owner: Owner,
        filter: &KbFilter,
        page: PageReq,
    ) -> Result<Page<KbSummary>> {
        let cond = self.scope_condition(owner)?;
        self.kb_page(&cond, filter, page)
    }

    /// What a person sees from `owner`, looking down: a project's entries at
    /// every level, an epic's and its topics', a topic's own.
    pub fn kb_rollup(
        &self,
        owner: Owner,
        filter: &KbFilter,
        page: PageReq,
    ) -> Result<Page<KbSummary>> {
        owner_ids(&self.conn, owner)?;
        let cond = match owner {
            Owner::Project(id) => format!("k.project_id = {}", id.0),
            Owner::Epic(id) => format!(
                "(k.epic_id = {0} OR k.topic_id IN (SELECT id FROM topics WHERE epic_id = {0} AND deleted_at IS NULL))",
                id.0
            ),
            Owner::Topic(id) => format!("k.topic_id = {}", id.0),
        };
        self.kb_page(&cond, filter, page)
    }

    /// Documents and memories in an agent's scope (for the session briefing).
    pub fn kb_counts(&self, owner: Owner) -> Result<KbCounts> {
        let cond = self.scope_condition(owner)?;
        let (documents, memories) = self.conn.query_row(
            &format!(
                "SELECT count(*) FILTER (WHERE kind = 'document'), count(*) FILTER (WHERE kind = 'memory')
                 FROM kb_entries k WHERE k.deleted_at IS NULL AND {cond}"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(KbCounts {
            documents,
            memories,
        })
    }

    /// SQL condition (alias `k`) selecting the knowledge an agent at `owner`
    /// sees, looking up.
    pub(crate) fn scope_condition(&self, owner: Owner) -> Result<String> {
        Ok(match owner {
            Owner::Project(id) => {
                self.project(id)?;
                format!("({})", prefixed(owner_condition(owner)))
            }
            Owner::Epic(id) => {
                let e = self.epic(id)?;
                format!(
                    "({} OR {})",
                    prefixed(owner_condition(owner)),
                    prefixed(owner_condition(Owner::Project(e.project)))
                )
            }
            Owner::Topic(id) => {
                let t = self.topic(id)?;
                let epic = t.epic.map_or(String::new(), |e| {
                    format!(" OR (k.epic_id = {} AND k.topic_id IS NULL)", e.0)
                });
                format!(
                    "({}{epic} OR {})",
                    prefixed(owner_condition(owner)),
                    prefixed(owner_condition(Owner::Project(t.project)))
                )
            }
        })
    }

    fn kb_page(&self, cond: &str, filter: &KbFilter, page: PageReq) -> Result<Page<KbSummary>> {
        let (offset, limit) = page.clamped();
        let mut conds = vec!["k.deleted_at IS NULL".to_owned(), cond.to_owned()];
        if let Some(kind) = filter.kind {
            conds.push(format!("k.kind = '{}'", kind.as_str()));
        }
        if let Some(author) = filter.created_by {
            conds.push(format!("k.created_by = '{}'", author.as_str()));
        }
        if let Some(level) = filter.level {
            conds.push(
                match level {
                    Level::Topic => "k.topic_id IS NOT NULL",
                    Level::Epic => "k.epic_id IS NOT NULL",
                    Level::Project => "k.epic_id IS NULL AND k.topic_id IS NULL",
                }
                .into(),
            );
        }
        let where_ = conds.join(" AND ");
        let total: i64 = self.conn.query_row(
            &format!("SELECT count(*) FROM kb_entries k WHERE {where_}"),
            [],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SUMMARY_COLS} FROM kb_entries k WHERE {where_}
             ORDER BY {LEVEL_RANK}, k.updated_at DESC, k.id DESC LIMIT {limit} OFFSET {offset}"
        ))?;
        let items = stmt
            .query_map([], summary_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Page {
            items,
            total,
            offset,
            capped: false,
        })
    }
}

/// `owner_condition` with the `k.` alias.
fn prefixed(cond: String) -> String {
    cond.replace("project_id", "k.project_id")
        .replace("epic_id", "k.epic_id")
        .replace("topic_id", "k.topic_id")
}

pub(crate) fn revision_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<KbRevision> {
    Ok(KbRevision {
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
}

/// The number of the next revision: one above every revision kept, so a
/// merge import that brought in higher numbers can never collide.
pub(crate) fn next_rev(conn: &rusqlite::Connection, id: KbId) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT coalesce(max(rev), 0) + 1 FROM kb_revisions WHERE entry_id = ?1",
        [id.0],
        |r| r.get(0),
    )?)
}

#[allow(clippy::too_many_arguments)]
fn insert_revision(
    tx: &rusqlite::Connection,
    id: KbId,
    rev: i64,
    title: &str,
    summary: &str,
    body: &str,
    author: Author,
    change: ChangeKind,
    section: Option<&str>,
    now: i64,
) -> Result<()> {
    tx.execute(
        "INSERT INTO kb_revisions (uuid, entry_id, rev, title, summary, body, author, change, section, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![new_uuid(), id.0, rev, title, summary, body, author, change, section, now],
    )?;
    Ok(())
}

pub(crate) fn read_kb(conn: &rusqlite::Connection, id: KbId) -> Result<KbEntry> {
    conn
        .query_row(
            &format!(
                "SELECT id, kind, project_id, epic_id, topic_id, title, summary, body, rev, created_by, {STAMP_COLS}
                 FROM kb_entries WHERE id = ?1 AND deleted_at IS NULL"
            ),
            [id.0],
            |r| {
                Ok(KbEntry {
                    id: KbId(r.get(0)?),
                    kind: r.get(1)?,
                    project: ProjectId(r.get(2)?),
                    owner: owner_at(r, 2)?,
                    title: r.get(5)?,
                    summary: r.get(6)?,
                    body: r.get(7)?,
                    rev: r.get(8)?,
                    created_by: r.get(9)?,
                    stamp: stamp(r, 10)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| Error::NotFound(id.to_string()))
}
