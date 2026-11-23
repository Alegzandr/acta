//! Deleting from the trash for good, on the human's demand (data.md 11,
//! request 25; ADR 0006, amendment of 2026-11-22).
//!
//! A purge removes the chosen trashed records and everything under them in
//! one transaction, clears the references surviving rows hold to them,
//! then removes every file content no row names any more (ADR 0012, 4).
//! It is not undoable and not recorded in history. Only the app calls it:
//! there is no MCP tool and no agent CLI command for it.

use std::collections::{BTreeMap, BTreeSet};

use super::Store;
use super::trash::{cascade, table_of};
use crate::error::{Error, Result};
use crate::model::{PageReq, ProjectId, Record};
use crate::time::now_ms;

/// What a purge removed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PurgeReport {
    /// Rows removed by kind (`project`, `epic`, `topic`, `entry`, `note`,
    /// `kb`, `folder`, `question`, `contact`, `attachment`), cascaded ones
    /// included; kinds with none are left out.
    pub counts: Vec<(&'static str, u64)>,
    /// Chosen ids that were not purged: no longer in the trash (restored
    /// meanwhile, or gone), or a contact still named somewhere.
    pub skipped: Vec<Record>,
    /// File contents removed, and their size.
    pub blobs: i64,
    pub bytes: i64,
}

impl PurgeReport {
    /// Every row removed, all kinds together.
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|(_, n)| n).sum()
    }
}

/// The kind name of a table, in the order of [`PurgeReport::counts`].
const KINDS: [(&str, &str); 10] = [
    ("projects", "project"),
    ("epics", "epic"),
    ("topics", "topic"),
    ("entries", "entry"),
    ("notes", "note"),
    ("kb_entries", "kb"),
    ("folders", "folder"),
    ("questions", "question"),
    ("contacts", "contact"),
    ("attachments", "attachment"),
];

/// `id IN (...)` for a set of row ids (integers only, so inlined).
fn list(ids: &BTreeSet<i64>) -> String {
    let parts: Vec<String> = ids.iter().map(i64::to_string).collect();
    parts.join(",")
}

impl Store {
    /// Deletes the given trashed records and what was trashed with them,
    /// for good (data.md 11, request 25).
    pub fn trash_purge(&mut self, ids: &[Record]) -> Result<PurgeReport> {
        let now = now_ms();
        let held = self.held.clone();
        let tx = self.write()?;
        // Deferred: rows go in any order; the keys are checked at commit.
        tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let mut rows: BTreeMap<&'static str, BTreeSet<i64>> = BTreeMap::new();
        let mut skipped = Vec::new();
        // Contacts chosen on their own, as opposed to ones under a chosen
        // project or epic.
        let mut chosen_contacts = BTreeSet::new();
        for &record in ids {
            let (table, id) = table_of(record);
            let trashed: bool = tx.query_row(
                &format!("SELECT count(*) FROM {table} WHERE id = ?1 AND deleted_at IS NOT NULL"),
                [id],
                |r| r.get::<_, i64>(0).map(|n| n > 0),
            )?;
            if !trashed {
                skipped.push(record);
                continue;
            }
            if let Record::Contact(c) = record {
                chosen_contacts.insert(c.0);
            }
            for (t, cond) in cascade(record) {
                let mut stmt = tx.prepare(&format!(
                    "SELECT id FROM {t} WHERE ({cond}) AND deleted_at IS NOT NULL"
                ))?;
                let found = stmt
                    .query_map([], |r| r.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows.entry(t).or_default().extend(found);
            }
        }
        let set = |rows: &BTreeMap<&str, BTreeSet<i64>>, t: &str| {
            list(rows.get(t).unwrap_or(&BTreeSet::new()))
        };

        // A contact still named by a row that stays is kept: skipped when
        // chosen on its own, lifted to all projects when its project or
        // epic goes.
        let contacts: Vec<i64> = rows
            .get("contacts")
            .map(|c| c.iter().copied().collect())
            .unwrap_or_default();
        let mut lifted = BTreeSet::new();
        for c in contacts {
            let named: i64 = tx.query_row(
                &format!(
                    "SELECT (SELECT count(*) FROM questions WHERE asked_contact_id = ?1 AND id NOT IN ({q}))
                          + (SELECT count(*) FROM topics WHERE report_contact_id = ?1 AND id NOT IN ({t}))
                          + (SELECT count(*) FROM epics WHERE report_contact_id = ?1 AND id NOT IN ({e}))
                          + (SELECT count(*) FROM notes WHERE report_contact_id = ?1 AND id NOT IN ({n}))",
                    q = set(&rows, "questions"),
                    t = set(&rows, "topics"),
                    e = set(&rows, "epics"),
                    n = set(&rows, "notes"),
                ),
                [c],
                |r| r.get(0),
            )?;
            if named == 0 {
                continue;
            }
            if let Some(set) = rows.get_mut("contacts") {
                set.remove(&c);
            }
            let in_purged_level: bool = tx.query_row(
                &format!(
                    "SELECT count(*) FROM contacts WHERE id = ?1
                       AND (project_id IN ({p}) OR epic_id IN ({e}))",
                    p = set(&rows, "projects"),
                    e = set(&rows, "epics"),
                ),
                [c],
                |r| r.get::<_, i64>(0).map(|n| n > 0),
            )?;
            if in_purged_level {
                lifted.insert(c);
            } else if chosen_contacts.contains(&c) {
                skipped.push(Record::Contact(crate::model::ContactId(c)));
            }
        }
        if !lifted.is_empty() {
            tx.execute(
                &format!(
                    "UPDATE contacts SET project_id = NULL, epic_id = NULL WHERE id IN ({})",
                    list(&lifted)
                ),
                [],
            )?;
        }

        // References surviving rows hold to purged ones.
        let nulls: [(&str, &str, &str); 14] = [
            ("notes", "filed_topic_id", "topics"),
            ("notes", "filed_epic_id", "epics"),
            ("notes", "filed_entry_id", "entries"),
            ("projects", "active_folder_id", "folders"),
            ("epics", "active_folder_id", "folders"),
            ("topics", "active_folder_id", "folders"),
            ("kb_entries", "superseded_by", "kb_entries"),
            ("kb_entries", "context_folder_id", "folders"),
            ("kb_entries", "created_topic_id", "topics"),
            ("kb_links", "folder_id", "folders"),
            ("attachments", "moved_from_note_id", "notes"),
            ("contacts", "merged_into", "contacts"),
            ("questions", "decision_entry_id", "entries"),
            ("questions", "asked_contact_id", "contacts"),
        ];
        for (table, col, target) in nulls {
            let ids = set(&rows, target);
            if ids.is_empty() {
                continue;
            }
            tx.execute(
                &format!("UPDATE {table} SET {col} = NULL WHERE {col} IN ({ids})"),
                [],
            )?;
        }
        let reports = set(&rows, "contacts");
        if !reports.is_empty() {
            for table in ["topics", "epics", "notes"] {
                tx.execute(
                    &format!(
                        "UPDATE {table} SET report_contact_id = NULL WHERE report_contact_id IN ({reports})"
                    ),
                    [],
                )?;
            }
        }

        // Rows that only exist for a purged one.
        let kb = set(&rows, "kb_entries");
        let contacts = set(&rows, "contacts");
        let dependents = [
            format!("DELETE FROM kb_revisions WHERE entry_id IN ({kb})"),
            format!("DELETE FROM kb_links WHERE entry_id IN ({kb})"),
            format!("DELETE FROM kb_pairs WHERE a IN ({kb}) OR b IN ({kb})"),
            format!("DELETE FROM contact_distinct WHERE a IN ({contacts}) OR b IN ({contacts})"),
            format!(
                "DELETE FROM read_markers WHERE (kind = 'topic' AND record_id IN ({}))
                    OR (kind = 'kb' AND record_id IN ({kb}))",
                set(&rows, "topics")
            ),
            format!(
                "DELETE FROM record_markers WHERE (kind = 'question' AND record_id IN ({}))
                    OR (kind = 'contact' AND record_id IN ({contacts}))",
                set(&rows, "questions")
            ),
        ];
        for sql in dependents {
            tx.execute(&sql, [])?;
        }

        let mut counts = Vec::new();
        for (table, kind) in KINDS {
            let Some(ids) = rows.get(table).filter(|s| !s.is_empty()) else {
                continue;
            };
            let n = tx.execute(
                &format!(
                    "DELETE FROM {table} WHERE id IN ({}) AND deleted_at IS NOT NULL",
                    list(ids)
                ),
                [],
            )?;
            if n > 0 {
                counts.push((kind, n as u64));
            }
        }
        let (blobs, bytes) = super::attachments::sweep_blobs(&tx, now, &held)?;
        // A reference this purge left behind would fail the commit with no
        // name: find which tables hold it first, for the log (table names
        // only, never values).
        let blocking = blocking_tables(&tx)?;
        if !blocking.is_empty() {
            return Err(Error::Invalid(format!(
                "a purge would leave references in {}",
                blocking.join(", ")
            )));
        }
        tx.commit()?;
        if self.conn.is_autocommit() && (!counts.is_empty() || blobs > 0) {
            let _ = super::attachments::incremental_vacuum(&self.conn);
        }
        let _ = self.refresh_link_index();
        Ok(PurgeReport {
            counts,
            skipped,
            blobs,
            bytes,
        })
    }

    /// A trashed project's content as its trash row and the purge
    /// confirmation count it: (topics, notes) in the trash under it.
    pub fn trash_project_content(&self, id: ProjectId) -> Result<(u64, u64)> {
        Ok(self.conn.query_row(
            "SELECT (SELECT count(*) FROM topics WHERE project_id = ?1 AND deleted_at IS NOT NULL),
                    (SELECT count(*) FROM notes WHERE project_id = ?1 AND deleted_at IS NOT NULL)",
            [id.0],
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64)),
        )?)
    }

    /// Empties the trash: [`Store::trash_purge`] of everything it lists.
    pub fn trash_purge_all(&mut self) -> Result<PurgeReport> {
        let mut ids = Vec::new();
        let mut offset = 0;
        loop {
            let page = self.trash(PageReq {
                offset,
                limit: PageReq::MAX_LIMIT,
            })?;
            let n = page.items.len() as i64;
            ids.extend(page.items.into_iter().map(|i| i.record));
            offset += n;
            if n == 0 || offset >= page.total {
                break;
            }
        }
        self.trash_purge(&ids)
    }
}

/// The tables holding a reference to a missing row, from
/// `PRAGMA foreign_key_check` (deferred keys are not yet checked).
fn blocking_tables(tx: &rusqlite::Connection) -> Result<Vec<String>> {
    let mut stmt = tx.prepare("PRAGMA foreign_key_check")?;
    let mut tables: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    tables.sort();
    tables.dedup();
    Ok(tables)
}
