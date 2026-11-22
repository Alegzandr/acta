//! People and Report to (docs/design/data.md request 19). No people table:
//! a person is the text written on a question, a note, a topic or an epic,
//! with its key beside it (`crate::person`). Who must be told about a
//! note, a topic or an epic, and whether they were.

use super::Store;
use super::records::{read_epic, read_topic};
use super::timeline::system_event;
use crate::error::{Error, Result};
use crate::model::{
    Author, EpicSummary, EventKind, Note, NoteId, Person, ProjectId, Record, TopicSummary,
};
use crate::person::{clean, person_key};
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};

/// A person value from a caller: cleaned, or refused over 80 chars.
pub(crate) fn person_arg(text: Option<&str>) -> Result<Option<String>> {
    match text {
        None => Ok(None),
        Some(t) => clean(t).map_err(Error::Invalid),
    }
}

/// The table and local id of a record that can carry a Report to.
fn report_table(record: Record) -> Result<(&'static str, i64)> {
    match record {
        Record::Topic(id) => Ok(("topics", id.0)),
        Record::Epic(id) => Ok(("epics", id.0)),
        Record::Note(id) => Ok(("notes", id.0)),
        Record::Project(_) => Err(Error::Invalid("projects have no report_to".into())),
        other => Err(Error::Invalid(format!("{other} has no report_to"))),
    }
}

/// The live record's current person.
fn current_person(tx: &rusqlite::Connection, record: Record) -> Result<Option<String>> {
    let (table, id) = report_table(record)?;
    tx.query_row(
        &format!("SELECT report_to FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        |r| r.get::<_, Option<String>>(0),
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(record.to_string()))
}

/// Inside the caller's transaction: sets (`Some`) or clears the person of
/// a note, a topic or an epic. A different key resets "reported"; the same
/// key in another spelling keeps it. On a topic the change is a system
/// event of kind `report`. Returns whether anything changed.
pub(crate) fn write_report_to(
    tx: &rusqlite::Connection,
    record: Record,
    person: Option<String>,
    author: Author,
    now: i64,
) -> Result<bool> {
    let (table, id) = report_table(record)?;
    let current = current_person(tx, record)?;
    if current == person {
        return Ok(false);
    }
    let key = person.as_deref().map(person_key);
    let same_key = current.as_deref().map(person_key) == key;
    let reset = if table == "notes" || same_key {
        ""
    } else {
        ", reported_at = NULL, reported_by = NULL"
    };
    tx.execute(
        &format!(
            "UPDATE {table} SET report_to = ?2, report_key = ?3, updated_at = ?4, updated_by = ?5{reset}
             WHERE id = ?1"
        ),
        params![id, person, key, now, author],
    )?;
    if let Record::Topic(t) = record {
        system_event(tx, t, EventKind::Report, current, person, author, now)?;
    }
    Ok(true)
}

/// Inside the caller's transaction: a note's person goes to the topic or
/// epic it is filed into when that record has none (request 19): the
/// first note naming someone, in the order given. A record that has its
/// own keeps it.
pub(crate) fn carry_person(
    tx: &rusqlite::Connection,
    notes: &[NoteId],
    target: Record,
    author: Author,
    now: i64,
) -> Result<()> {
    if current_person(tx, target)?.is_some() {
        return Ok(());
    }
    for n in notes {
        let person: Option<String> =
            tx.query_row("SELECT report_to FROM notes WHERE id = ?1", [n.0], |r| {
                r.get(0)
            })?;
        if person.is_some() {
            write_report_to(tx, target, person, author, now)?;
            break;
        }
    }
    Ok(())
}

/// What is still to be reported to one person (the person page's "To
/// report", `find(person=...)`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReportTargets {
    /// Not reported and not Dropped, in tree order.
    pub topics: Vec<TopicSummary>,
    /// Not reported, in stored order.
    pub epics: Vec<EpicSummary>,
    /// Open notes naming them, newest first.
    pub notes: Vec<Note>,
}

impl Store {
    /// The topics, epics and open notes of a project whose Report to has
    /// `key` ([`person_key`]) and that are not marked reported.
    pub fn report_targets(&self, project: ProjectId, key: &str) -> Result<ReportTargets> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {} FROM topics t LEFT JOIN epics e ON e.id = t.epic_id
             WHERE t.project_id = ?1 AND t.deleted_at IS NULL AND t.report_key = ?2
               AND t.reported_at IS NULL AND t.status <> 'dropped'
             ORDER BY t.epic_id IS NOT NULL, e.position, t.epic_id, t.position, t.id",
            super::records::TOPIC_SUMMARY_COLS
        ))?;
        let topics = stmt
            .query_map(params![project.0, key], super::records::topic_summary)?
            .collect::<rusqlite::Result<_>>()?;
        let epics = self
            .epics(project)?
            .into_iter()
            .filter(|e| {
                !e.reported && e.report_to.as_deref().map(person_key).as_deref() == Some(key)
            })
            .collect();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {}, {} FROM notes
             WHERE project_id = ?1 AND deleted_at IS NULL AND filed_at IS NULL AND report_key = ?2
             ORDER BY created_at DESC, id DESC",
            super::timeline::NOTE_COLS,
            super::STAMP_COLS
        ))?;
        let notes = stmt
            .query_map(params![project.0, key], super::timeline::note_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ReportTargets {
            topics,
            epics,
            notes,
        })
    }

    /// Sets (`Some`) or clears (`None`, or blank text) who a note, a topic
    /// or an epic must be reported to (request 19). Projects are refused.
    pub fn set_report_to(
        &mut self,
        record: Record,
        person: Option<&str>,
        author: Author,
    ) -> Result<()> {
        let person = person_arg(person)?;
        let tx = self.write()?;
        write_report_to(&tx, record, person, author, now_ms())?;
        tx.commit()?;
        Ok(())
    }

    /// Marks a topic or an epic reported to its person, or not reported.
    /// Refused without a person. Acta never infers it: a human (or Claude
    /// Code, when asked) records it. On a topic, a system event of kind
    /// `reported` (new: the person when marked; old: when unmarked).
    pub fn set_reported(&mut self, record: Record, reported: bool, author: Author) -> Result<()> {
        let (table, id) = match record {
            Record::Topic(id) => ("topics", id.0),
            Record::Epic(id) => ("epics", id.0),
            Record::Project(_) => {
                return Err(Error::Invalid("projects have no report_to".into()));
            }
            other => return Err(Error::Invalid(format!("{other} has no reported"))),
        };
        let tx = self.write()?;
        let (person, at) = match record {
            Record::Topic(t) => {
                let r = read_topic(&tx, t)?.report;
                (r.to, r.at)
            }
            Record::Epic(e) => {
                let r = read_epic(&tx, e)?.report;
                (r.to, r.at)
            }
            _ => unreachable!("refused above"),
        };
        let Some(person) = person else {
            return Err(Error::Invalid(format!(
                "{record} has no report_to; set one first"
            )));
        };
        if at.is_some() == reported {
            return Ok(());
        }
        let now = now_ms();
        let (when, who) = if reported {
            (Some(now), Some(author))
        } else {
            (None, None)
        };
        tx.execute(
            &format!(
                "UPDATE {table} SET reported_at = ?2, reported_by = ?3, updated_at = ?4, updated_by = ?5
                 WHERE id = ?1"
            ),
            params![id, when, who, now, author],
        )?;
        if let Record::Topic(t) = record {
            let (old, new) = if reported {
                (None, Some(person))
            } else {
                (Some(person), None)
            };
            system_event(&tx, t, EventKind::Reported, old, new, author, now)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The people of a project (request 19), from one grouped query over
    /// the four person columns: most open items first, then most recently
    /// used. Open: questions still open, notes still in the inbox, topics
    /// and epics not reported (topics also not Dropped).
    pub fn people(&self, project: ProjectId) -> Result<Vec<Person>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, name, max(at) AS last, sum(open) AS n FROM (
                 SELECT asked_key AS key, asked_to AS name, updated_at AS at, status = 'open' AS open
                   FROM questions WHERE project_id = ?1 AND deleted_at IS NULL AND asked_key IS NOT NULL
                 UNION ALL
                 SELECT report_key, report_to, updated_at, filed_at IS NULL
                   FROM notes WHERE project_id = ?1 AND deleted_at IS NULL AND report_key IS NOT NULL
                 UNION ALL
                 SELECT report_key, report_to, updated_at, reported_at IS NULL AND status <> 'dropped'
                   FROM topics WHERE project_id = ?1 AND deleted_at IS NULL AND report_key IS NOT NULL
                 UNION ALL
                 SELECT report_key, report_to, updated_at, reported_at IS NULL
                   FROM epics WHERE project_id = ?1 AND deleted_at IS NULL AND report_key IS NOT NULL
             ) GROUP BY key ORDER BY n DESC, last DESC, key",
        )?;
        let rows = stmt.query_map([project.0], |r| {
            Ok(Person {
                key: r.get(0)?,
                display: r.get(1)?,
                last_used: r.get(2)?,
                open: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
