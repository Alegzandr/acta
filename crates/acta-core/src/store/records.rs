//! Projects, epics and topics: create, read, update, reorder, and the tree.

use super::{STAMP_COLS, Store, new_uuid, required, stamp};
use crate::date::Date;
use crate::error::{Error, Result};
use crate::model::{
    Author, Epic, EpicId, EpicSummary, EventKind, FolderId, Owner, Page, PageReq, Progress,
    Project, ProjectId, ProjectSummary, ProjectTree, Record, Status, Topic, TopicId, TopicSummary,
};
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};

#[derive(Clone, Debug, Default)]
pub struct ProjectPatch {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct EpicPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewTopic {
    pub project: ProjectId,
    pub epic: Option<EpicId>,
    pub title: String,
    pub description: String,
    pub status: Status,
}

#[derive(Clone, Debug, Default)]
pub struct TopicPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    /// Replaces the current state; the previous one is kept as a system entry.
    pub state: Option<String>,
    pub status: Option<Status>,
    /// `Some(None)` moves the topic out of any epic.
    pub epic: Option<Option<EpicId>>,
}

#[derive(Clone, Debug, Default)]
pub struct TopicFilter {
    pub project: Option<ProjectId>,
    /// `Some(None)` selects topics outside any epic.
    pub epic: Option<Option<EpicId>>,
    /// Empty means every status.
    pub statuses: Vec<Status>,
}

const TOPIC_SUMMARY_COLS: &str = "t.id, t.epic_id, t.title, t.status, t.position,
    (SELECT count(*) FROM entries e WHERE e.topic_id = t.id AND e.deleted_at IS NULL AND e.kind <> 'system'),
    t.updated_at, t.updated_by, t.due_on";

fn topic_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<TopicSummary> {
    Ok(TopicSummary {
        id: TopicId(row.get(0)?),
        epic: row.get::<_, Option<i64>>(1)?.map(EpicId),
        title: row.get(2)?,
        status: row.get(3)?,
        position: row.get(4)?,
        entries: row.get(5)?,
        updated_at: row.get(6)?,
        updated_by: row.get(7)?,
        due_on: row.get(8)?,
    })
}

impl Store {
    // ---- projects ----

    pub fn create_project(
        &mut self,
        title: &str,
        description: &str,
        author: Author,
    ) -> Result<ProjectId> {
        let title = required("title", title)?;
        let tx = self.write()?;
        let now = now_ms();
        let position: i64 = tx.query_row(
            "SELECT coalesce(max(position) + 1, 0) FROM projects WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO projects (uuid, title, description, position, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            params![new_uuid(), title, description.trim(), position, now, author],
        )?;
        let id = ProjectId(tx.last_insert_rowid());
        tx.commit()?;
        Ok(id)
    }

    pub fn project(&self, id: ProjectId) -> Result<Project> {
        read_project(&self.conn, id)
    }

    pub fn update_project(
        &mut self,
        id: ProjectId,
        patch: ProjectPatch,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        let cur = read_project(&tx, id)?;
        let title = match &patch.title {
            Some(t) => required("title", t)?,
            None => cur.title,
        };
        let description = patch
            .description
            .map_or(cur.description, |d| d.trim().to_owned());
        tx.execute(
            "UPDATE projects SET title = ?2, description = ?3, updated_at = ?4, updated_by = ?5 WHERE id = ?1",
            params![id.0, title, description, now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Every live project, in order, with its counts.
    pub fn projects(&self) -> Result<Vec<ProjectSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.title, p.position, p.updated_at,
               (SELECT count(*) FROM topics t WHERE t.project_id = p.id AND t.deleted_at IS NULL AND t.status = 'done'),
               (SELECT count(*) FROM topics t WHERE t.project_id = p.id AND t.deleted_at IS NULL AND t.status <> 'dropped'),
               (SELECT count(*) FROM notes n WHERE n.project_id = p.id AND n.deleted_at IS NULL AND n.filed_at IS NULL),
               (SELECT count(*) FROM kb_entries k WHERE k.project_id = p.id AND k.deleted_at IS NULL),
               p.due_on
             FROM projects p WHERE p.deleted_at IS NULL ORDER BY p.position, p.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ProjectSummary {
                id: ProjectId(r.get(0)?),
                title: r.get(1)?,
                position: r.get(2)?,
                updated_at: r.get(3)?,
                progress: Progress {
                    done: r.get(4)?,
                    total: r.get(5)?,
                },
                inbox: r.get(6)?,
                knowledge: r.get(7)?,
                due_on: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- epics ----

    pub fn create_epic(
        &mut self,
        project: ProjectId,
        title: &str,
        description: &str,
        author: Author,
    ) -> Result<EpicId> {
        let title = required("title", title)?;
        let tx = self.write()?;
        read_project(&tx, project)?;
        let now = now_ms();
        let position: i64 = tx.query_row(
            "SELECT coalesce(max(position) + 1, 0) FROM epics WHERE project_id = ?1 AND deleted_at IS NULL",
            [project.0],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO epics (uuid, project_id, title, description, position, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
            params![new_uuid(), project.0, title, description.trim(), position, now, author],
        )?;
        let id = EpicId(tx.last_insert_rowid());
        tx.commit()?;
        Ok(id)
    }

    pub fn epic(&self, id: EpicId) -> Result<Epic> {
        read_epic(&self.conn, id)
    }

    pub fn update_epic(&mut self, id: EpicId, patch: EpicPatch, author: Author) -> Result<()> {
        let tx = self.write()?;
        let cur = read_epic(&tx, id)?;
        let title = match &patch.title {
            Some(t) => required("title", t)?,
            None => cur.title,
        };
        let description = patch
            .description
            .map_or(cur.description, |d| d.trim().to_owned());
        let state = patch.state.map_or(cur.state, |s| s.trim().to_owned());
        tx.execute(
            "UPDATE epics SET title = ?2, description = ?3, state = ?4, updated_at = ?5, updated_by = ?6 WHERE id = ?1",
            params![id.0, title, description, state, now_ms(), author],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The live epics of a project, in order, with progress.
    pub fn epics(&self, project: ProjectId) -> Result<Vec<EpicSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.title, e.position, e.updated_at,
               (SELECT count(*) FROM topics t WHERE t.epic_id = e.id AND t.deleted_at IS NULL AND t.status = 'done'),
               (SELECT count(*) FROM topics t WHERE t.epic_id = e.id AND t.deleted_at IS NULL AND t.status <> 'dropped'),
               e.due_on
             FROM epics e WHERE e.project_id = ?1 AND e.deleted_at IS NULL ORDER BY e.position, e.id",
        )?;
        let rows = stmt.query_map([project.0], |r| {
            Ok(EpicSummary {
                id: EpicId(r.get(0)?),
                title: r.get(1)?,
                position: r.get(2)?,
                updated_at: r.get(3)?,
                progress: Progress {
                    done: r.get(4)?,
                    total: r.get(5)?,
                },
                due_on: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- due dates (data.md request 16) ----

    /// Sets (`Some`) or clears (`None`) the due day of a project, an epic or
    /// a topic, with its `updated_at` / `updated_by`. On a topic the change
    /// is also a system event of kind `due` (old and new as `YYYY-MM-DD`).
    /// Setting the date it already has writes nothing.
    pub fn set_due(&mut self, record: Owner, due: Option<Date>, author: Author) -> Result<()> {
        if let Some(d) = due
            && !d.in_range()
        {
            return Err(Error::Invalid(format!(
                "a due date must be in 2000 to 2099, got {d}"
            )));
        }
        let tx = self.write()?;
        let (table, id, current) = match record {
            Owner::Project(id) => ("projects", id.0, read_project(&tx, id)?.due_on),
            Owner::Epic(id) => ("epics", id.0, read_epic(&tx, id)?.due_on),
            Owner::Topic(id) => ("topics", id.0, read_topic(&tx, id)?.due_on),
        };
        if current == due {
            return Ok(());
        }
        let now = now_ms();
        tx.execute(
            &format!(
                "UPDATE {table} SET due_on = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1"
            ),
            params![id, due, now, author],
        )?;
        if let Owner::Topic(t) = record {
            super::timeline::system_event(
                &tx,
                t,
                EventKind::Due,
                current.map(|d| d.to_string()),
                due.map(|d| d.to_string()),
                author,
                now,
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Sample data for captures (the app's debug dev seed): move a topic
    /// with its entries, a note, or a knowledge entry `days` back in time,
    /// so the Overview has idle work, an old inbox and stale memories to
    /// show. Debug builds only; nothing else rewrites history.
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn dev_backdate(&mut self, record: Record, days: i64) -> Result<()> {
        let back = days * crate::time::MS_PER_DAY;
        let tx = self.write()?;
        let shift = "SET created_at = created_at - ?2, updated_at = updated_at - ?2";
        match record {
            Record::Topic(id) => {
                tx.execute(
                    &format!("UPDATE topics {shift} WHERE id = ?1"),
                    params![id.0, back],
                )?;
                tx.execute(
                    &format!("UPDATE entries {shift} WHERE topic_id = ?1"),
                    params![id.0, back],
                )?;
            }
            Record::Note(id) => {
                tx.execute(
                    &format!("UPDATE notes {shift} WHERE id = ?1"),
                    params![id.0, back],
                )?;
            }
            Record::Kb(id) => {
                tx.execute(
                    &format!("UPDATE kb_entries {shift} WHERE id = ?1"),
                    params![id.0, back],
                )?;
            }
            other => return Err(Error::Invalid(format!("cannot backdate {other}"))),
        }
        tx.commit()?;
        Ok(())
    }

    // ---- topics ----

    pub fn create_topic(&mut self, new: NewTopic, author: Author) -> Result<TopicId> {
        let title = required("title", &new.title)?;
        let tx = self.write()?;
        read_project(&tx, new.project)?;
        if let Some(epic) = new.epic {
            check_epic_in(&tx, epic, new.project)?;
        }
        let now = now_ms();
        let position = next_topic_position(&tx, new.project, new.epic)?;
        tx.execute(
            "INSERT INTO topics (uuid, project_id, epic_id, title, description, status, position,
                                 created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
            params![
                new_uuid(),
                new.project.0,
                new.epic.map(|e| e.0),
                title,
                new.description.trim(),
                new.status,
                position,
                now,
                author
            ],
        )?;
        let id = TopicId(tx.last_insert_rowid());
        super::timeline::system_event(&tx, id, EventKind::Created, None, None, author, now)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn topic(&self, id: TopicId) -> Result<Topic> {
        read_topic(&self.conn, id)
    }

    /// The topic of `project` that Claude Code wrote to last, with the time
    /// of that write (the status bar's "Last wrote 2 min ago in ...").
    pub fn last_claude_write(&self, project: ProjectId) -> Result<Option<(TopicId, String, i64)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT t.id, t.title, max(w.at) AS last FROM (
                     SELECT topic_id AS tid, created_at AS at FROM entries
                     WHERE author = ?2 AND deleted_at IS NULL
                     UNION ALL
                     SELECT id, updated_at FROM topics WHERE updated_by = ?2
                 ) w JOIN topics t ON t.id = w.tid
                 WHERE t.project_id = ?1 AND t.deleted_at IS NULL
                 GROUP BY t.id ORDER BY last DESC, t.id DESC LIMIT 1",
                params![project.0, Author::Claude],
                |r| Ok((TopicId(r.get(0)?), r.get(1)?, r.get(2)?)),
            )
            .optional()?)
    }

    /// Edits a topic. Status changes, moves and state replacements are
    /// recorded as system entries in its timeline (request 10).
    pub fn update_topic(&mut self, id: TopicId, patch: TopicPatch, author: Author) -> Result<()> {
        let tx = self.write()?;
        let cur = read_topic(&tx, id)?;
        if let Some(Some(epic)) = patch.epic
            && read_epic(&tx, epic)?.project != cur.project
        {
            return Err(Error::Invalid(format!(
                "{epic} belongs to another project than {}",
                cur.project
            )));
        }
        apply_topic_patch(&tx, &cur, patch, author, now_ms())?;
        tx.commit()?;
        Ok(())
    }

    /// Topics matching a filter, in tree order, with entry counts from one
    /// grouped query (request 8).
    pub fn topics(&self, filter: &TopicFilter, page: PageReq) -> Result<Page<TopicSummary>> {
        let (offset, limit) = page.clamped();
        let mut cond = vec!["t.deleted_at IS NULL".to_owned()];
        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(p) = filter.project {
            args.push(p.0.into());
            cond.push(format!("t.project_id = ?{}", args.len()));
        }
        match filter.epic {
            Some(Some(e)) => {
                args.push(e.0.into());
                cond.push(format!("t.epic_id = ?{}", args.len()));
            }
            Some(None) => cond.push("t.epic_id IS NULL".into()),
            None => {}
        }
        if !filter.statuses.is_empty() {
            let list: Vec<String> = filter
                .statuses
                .iter()
                .map(|s| format!("'{}'", s.as_str()))
                .collect();
            cond.push(format!("t.status IN ({})", list.join(", ")));
        }
        let where_ = cond.join(" AND ");
        let total: i64 = self.conn.query_row(
            &format!("SELECT count(*) FROM topics t WHERE {where_}"),
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        let sql = format!(
            "SELECT {TOPIC_SUMMARY_COLS} FROM topics t
             LEFT JOIN epics e ON e.id = t.epic_id
             WHERE {where_}
             ORDER BY t.project_id, t.epic_id IS NOT NULL, e.position, t.epic_id, t.position, t.id
             LIMIT {limit} OFFSET {offset}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let items = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), topic_summary)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Page {
            items,
            total,
            offset,
            capped: false,
        })
    }

    /// The sidebar tree of a project: loose topics, then epics with theirs.
    pub fn tree(&self, project: ProjectId) -> Result<ProjectTree> {
        let summary = self
            .projects()?
            .into_iter()
            .find(|p| p.id == project)
            .ok_or_else(|| Error::NotFound(project.to_string()))?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TOPIC_SUMMARY_COLS} FROM topics t
             WHERE t.project_id = ?1 AND t.deleted_at IS NULL
             ORDER BY t.position, t.id"
        ))?;
        let topics: Vec<TopicSummary> = stmt
            .query_map([project.0], topic_summary)?
            .collect::<rusqlite::Result<_>>()?;
        let loose_topics = topics
            .iter()
            .filter(|t| t.epic.is_none())
            .cloned()
            .collect();
        let epics = self
            .epics(project)?
            .into_iter()
            .map(|e| {
                let mine = topics
                    .iter()
                    .filter(|t| t.epic == Some(e.id))
                    .cloned()
                    .collect();
                (e, mine)
            })
            .collect();
        Ok(ProjectTree {
            project: summary,
            loose_topics,
            epics,
        })
    }

    /// Moves a project, epic, topic or folder to `index` among its siblings
    /// (clamped). Siblings whose position changes get a new `updated_at`, so
    /// an import carries the order.
    /// Where a record stands among its siblings, in the stored order
    /// (what [`Store::reorder`] takes as `index`).
    pub fn order_index(&self, record: Record) -> Result<usize> {
        let (_, ids, raw) = siblings(&self.conn, record)?;
        ids.iter()
            .position(|(id, _)| *id == raw)
            .ok_or_else(|| Error::NotFound(record.to_string()))
    }

    pub fn reorder(&mut self, record: Record, index: usize, author: Author) -> Result<()> {
        let tx = self.write()?;
        let (table, mut ids, raw) = siblings(&tx, record)?;
        let from = ids
            .iter()
            .position(|(id, _)| *id == raw)
            .ok_or_else(|| Error::NotFound(record.to_string()))?;
        let moved = ids.remove(from);
        ids.insert(index.min(ids.len()), moved);
        let now = now_ms();
        for (pos, (id, old)) in ids.iter().enumerate() {
            if *old != pos as i64 {
                tx.execute(
                    &format!("UPDATE {table} SET position = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1"),
                    params![id, pos as i64, now, author],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// A reorderable record's table, its live siblings `(id, position)` in the
/// stored order, and its own local id.
type Siblings = (&'static str, Vec<(i64, i64)>, i64);

/// The table of a reorderable record, its live siblings `(id, position)` in
/// the stored order, and its own local id.
fn siblings(tx: &rusqlite::Connection, record: Record) -> Result<Siblings> {
    {
        let (table, scope) = match record {
            Record::Project(id) => {
                read_project(tx, id)?;
                ("projects", String::from("1 = 1"))
            }
            Record::Epic(id) => {
                let e = read_epic(tx, id)?;
                ("epics", format!("project_id = {}", e.project.0))
            }
            Record::Topic(id) => {
                let t = read_topic(tx, id)?;
                let epic = t
                    .epic
                    .map_or("IS NULL".to_owned(), |e| format!("= {}", e.0));
                (
                    "topics",
                    format!("project_id = {} AND epic_id {epic}", t.project.0),
                )
            }
            Record::Folder(id) => {
                let f = super::folders::read_folder(tx, id)?;
                ("folders", super::owner_condition(f.owner))
            }
            other => return Err(Error::Invalid(format!("{other} cannot be reordered"))),
        };
        let raw = match record {
            Record::Project(id) => id.0,
            Record::Epic(id) => id.0,
            Record::Topic(id) => id.0,
            Record::Folder(id) => id.0,
            _ => unreachable!(),
        };
        let ids: Vec<(i64, i64)> = {
            let mut stmt = tx.prepare(&format!(
                "SELECT id, position FROM {table} WHERE {scope} AND deleted_at IS NULL ORDER BY position, id"
            ))?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        Ok((table, ids, raw))
    }
}

fn check_epic_in(conn: &rusqlite::Connection, epic: EpicId, project: ProjectId) -> Result<()> {
    let e = read_epic(conn, epic)?;
    if e.project != project {
        return Err(Error::Invalid(format!(
            "{epic} belongs to another project than {project}"
        )));
    }
    Ok(())
}

fn next_topic_position(
    tx: &rusqlite::Connection,
    project: ProjectId,
    epic: Option<EpicId>,
) -> Result<i64> {
    Ok(tx.query_row(
        "SELECT coalesce(max(position) + 1, 0) FROM topics
         WHERE project_id = ?1 AND epic_id IS ?2 AND deleted_at IS NULL",
        params![project.0, epic.map(|e| e.0)],
        |r| r.get(0),
    )?)
}

/// Applies a topic patch inside a transaction, writing the system events.
pub(crate) fn apply_topic_patch(
    tx: &rusqlite::Connection,
    cur: &Topic,
    patch: TopicPatch,
    author: Author,
    now: i64,
) -> Result<()> {
    use super::timeline::system_event;
    let title = match &patch.title {
        Some(t) => required("title", t)?,
        None => cur.title.clone(),
    };
    let description = patch
        .description
        .map_or(cur.description.clone(), |d| d.trim().to_owned());
    let status = patch.status.unwrap_or(cur.status);
    if status != cur.status {
        system_event(
            tx,
            cur.id,
            EventKind::Status,
            Some(cur.status.as_str().into()),
            Some(status.as_str().into()),
            author,
            now,
        )?;
    }
    let (mut state, mut state_at, mut state_by) = (
        cur.state.clone(),
        cur.state_updated_at,
        cur.state_updated_by,
    );
    if let Some(new_state) = patch.state {
        let new_state = new_state.trim().to_owned();
        if new_state != cur.state {
            system_event(
                tx,
                cur.id,
                EventKind::State,
                Some(cur.state.clone()).filter(|s| !s.is_empty()),
                None,
                author,
                now,
            )?;
            state = new_state;
            state_at = Some(now);
            state_by = Some(author);
        }
    }
    let mut epic = cur.epic;
    let mut position = cur.position;
    if let Some(new_epic) = patch.epic
        && new_epic != cur.epic
    {
        let uuid_of = |e: Option<EpicId>| -> Result<Option<String>> {
            Ok(match e {
                Some(e) => {
                    Some(tx.query_row("SELECT uuid FROM epics WHERE id = ?1", [e.0], |r| r.get(0))?)
                }
                None => None,
            })
        };
        system_event(
            tx,
            cur.id,
            EventKind::Moved,
            uuid_of(cur.epic)?,
            uuid_of(new_epic)?,
            author,
            now,
        )?;
        epic = new_epic;
        position = next_topic_position(tx, cur.project, new_epic)?;
        // An active folder inherited from the old epic is out of sight now
        // (ADR 0011): the default rule applies again.
        tx.execute(
            "UPDATE topics SET active_folder_id = NULL WHERE id = ?1 AND active_folder_id IN
                (SELECT id FROM folders WHERE epic_id IS NOT NULL AND epic_id IS NOT ?2)",
            params![cur.id.0, new_epic.map(|e| e.0)],
        )?;
    }
    tx.execute(
        "UPDATE topics SET title = ?2, description = ?3, status = ?4, state = ?5, state_updated_at = ?6,
                state_updated_by = ?7, epic_id = ?8, position = ?9, updated_at = ?10, updated_by = ?11
         WHERE id = ?1",
        params![
            cur.id.0,
            title,
            description,
            status,
            state,
            state_at,
            state_by,
            epic.map(|e| e.0),
            position,
            now,
            author
        ],
    )?;
    Ok(())
}

pub(crate) fn read_project(conn: &rusqlite::Connection, id: ProjectId) -> Result<Project> {
    conn.query_row(
        &format!(
            "SELECT id, title, description, position, active_folder_id, due_on, {STAMP_COLS}
                 FROM projects WHERE id = ?1 AND deleted_at IS NULL"
        ),
        [id.0],
        |r| {
            Ok(Project {
                id: ProjectId(r.get(0)?),
                title: r.get(1)?,
                description: r.get(2)?,
                position: r.get(3)?,
                active_folder: r.get::<_, Option<i64>>(4)?.map(FolderId),
                due_on: r.get(5)?,
                stamp: stamp(r, 6)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(id.to_string()))
}

pub(crate) fn read_epic(conn: &rusqlite::Connection, id: EpicId) -> Result<Epic> {
    conn
        .query_row(
            &format!(
                "SELECT id, project_id, title, description, state, position, active_folder_id, due_on, {STAMP_COLS}
                 FROM epics WHERE id = ?1 AND deleted_at IS NULL"
            ),
            [id.0],
            |r| {
                Ok(Epic {
                    id: EpicId(r.get(0)?),
                    project: ProjectId(r.get(1)?),
                    title: r.get(2)?,
                    description: r.get(3)?,
                    state: r.get(4)?,
                    position: r.get(5)?,
                    active_folder: r.get::<_, Option<i64>>(6)?.map(FolderId),
                    due_on: r.get(7)?,
                    stamp: stamp(r, 8)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| Error::NotFound(id.to_string()))
}

pub(crate) fn read_topic(conn: &rusqlite::Connection, id: TopicId) -> Result<Topic> {
    conn
        .query_row(
            &format!(
                "SELECT id, project_id, epic_id, title, description, status, state,
                        state_updated_at, state_updated_by, position, active_folder_id, due_on, {STAMP_COLS}
                 FROM topics WHERE id = ?1 AND deleted_at IS NULL"
            ),
            [id.0],
            |r| {
                Ok(Topic {
                    id: TopicId(r.get(0)?),
                    project: ProjectId(r.get(1)?),
                    epic: r.get::<_, Option<i64>>(2)?.map(EpicId),
                    title: r.get(3)?,
                    description: r.get(4)?,
                    status: r.get(5)?,
                    state: r.get(6)?,
                    state_updated_at: r.get(7)?,
                    state_updated_by: r.get(8)?,
                    position: r.get(9)?,
                    active_folder: r.get::<_, Option<i64>>(10)?.map(FolderId),
                    due_on: r.get(11)?,
                    stamp: stamp(r, 12)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| Error::NotFound(id.to_string()))
}
