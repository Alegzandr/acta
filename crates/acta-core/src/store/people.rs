//! Contacts, Report to and the profile (docs/design/data.md requests 19,
//! 22 and 23; components-contacts.md 5). A person is always a contact: a
//! name with optional details, living at one level (all projects, a
//! project, an epic) and visible there and below ([`visible`]). Every
//! write that sets a reference checks the contact is visible from the
//! record, in its transaction. Moving a record lifts a contact it names
//! that the new place cannot see; a contact moves down only while every
//! use stays in scope.

use super::records::{read_epic, read_project, read_topic};
use super::timeline::system_event;
use super::{STAMP_COLS, Store, new_uuid, stamp};
use crate::error::{Error, Result};
use crate::model::{
    Author, Contact, ContactId, ContactLevel, ContactRef, ContactUse, EpicId, EpicSummary,
    EventKind, Lift, Note, NoteId, Owner, Place, Profile, ProjectId, QuestionId, Record, TopicId,
    TopicSummary, visible,
};
use crate::person::{clean, person_key};
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};

/// Longest job title and company, in characters.
pub const DETAIL_MAX: usize = 80;
/// Longest address note when typed.
pub const ADDRESS_MAX: usize = 500;

/// Reads a contact reference at column `at`: id, name, in trash.
pub(crate) fn contact_ref(
    row: &rusqlite::Row<'_>,
    at: usize,
) -> rusqlite::Result<Option<ContactRef>> {
    let id: Option<i64> = row.get(at)?;
    Ok(match id {
        Some(id) => Some(ContactRef {
            id: ContactId(id),
            name: row.get::<_, Option<String>>(at + 1)?.unwrap_or_default(),
            in_trash: row.get::<_, Option<bool>>(at + 2)?.unwrap_or(false),
        }),
        None => None,
    })
}

const CONTACT_COLS: &str =
    "id, project_id, epic_id, name, job_title, company, address_note, created_by, merged_into";

fn contact_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Contact> {
    let project: Option<i64> = r.get(1)?;
    let epic: Option<i64> = r.get(2)?;
    let level = match (project, epic) {
        (Some(p), Some(e)) => ContactLevel::Epic(ProjectId(p), EpicId(e)),
        (Some(p), None) => ContactLevel::Project(ProjectId(p)),
        _ => ContactLevel::All,
    };
    Ok(Contact {
        id: ContactId(r.get(0)?),
        level,
        name: r.get(3)?,
        job_title: r.get(4)?,
        company: r.get(5)?,
        address_note: r.get(6)?,
        created_by: r.get(7)?,
        merged_into: r.get::<_, Option<i64>>(8)?.map(ContactId),
        stamp: stamp(r, 9)?,
    })
}

/// A contact, live or (with `trashed`) in the trash too.
pub(crate) fn read_contact(
    conn: &rusqlite::Connection,
    id: ContactId,
    trashed: bool,
) -> Result<Contact> {
    let live = if trashed {
        ""
    } else {
        " AND deleted_at IS NULL"
    };
    conn.query_row(
        &format!("SELECT {CONTACT_COLS}, {STAMP_COLS} FROM contacts WHERE id = ?1{live}"),
        [id.0],
        contact_row,
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(id.to_string()))
}

/// Where a live record sits, for visibility.
pub(crate) fn place_of(conn: &rusqlite::Connection, record: Record) -> Result<Place> {
    Ok(match record {
        Record::Project(p) => Place {
            project: read_project(conn, p)?.id,
            epic: None,
        },
        Record::Epic(e) => Place {
            project: read_epic(conn, e)?.project,
            epic: Some(e),
        },
        Record::Topic(t) => {
            let t = read_topic(conn, t)?;
            Place {
                project: t.project,
                epic: t.epic,
            }
        }
        Record::Note(n) => Place {
            project: super::timeline::read_note(conn, n)?.project,
            epic: None,
        },
        Record::Question(q) => {
            let q = super::questions::read_question(conn, q)?;
            owner_place(conn, q.owner)?
        }
        other => return Err(Error::Invalid(format!("{other} names no contact"))),
    })
}

pub(crate) fn owner_place(conn: &rusqlite::Connection, owner: Owner) -> Result<Place> {
    place_of(
        conn,
        match owner {
            Owner::Project(p) => Record::Project(p),
            Owner::Epic(e) => Record::Epic(e),
            Owner::Topic(t) => Record::Topic(t),
        },
    )
}

/// Inside the caller's transaction: the contact is live and visible from
/// `place`, else [`Error::OutOfScope`].
pub(crate) fn check_ref(
    tx: &rusqlite::Connection,
    contact: ContactId,
    place: Place,
    record: &str,
) -> Result<()> {
    let c = read_contact(tx, contact, false)?;
    if visible(c.level, place) {
        Ok(())
    } else {
        Err(Error::OutOfScope {
            contact: contact.to_string(),
            record: record.to_owned(),
            level: level_name(c.level),
        })
    }
}

/// `all projects`, `P-2`, `E-3`.
pub fn level_name(level: ContactLevel) -> String {
    match level {
        ContactLevel::All => "all projects".into(),
        other => other.to_string(),
    }
}

/// Inside the caller's transaction: if `place` cannot see the contact,
/// widen it to the narrowest level that sees both its level and the
/// place (an epic's contact to its project, else to all projects).
pub(crate) fn lift(
    tx: &rusqlite::Connection,
    contact: ContactId,
    place: Place,
    record: Record,
    author: Author,
    now: i64,
) -> Result<Option<Lift>> {
    let c = read_contact(tx, contact, true)?;
    if visible(c.level, place) {
        return Ok(None);
    }
    let to = c.level.join(ContactLevel::of_place(place));
    write_level(tx, contact, to, author, now)?;
    Ok(Some(Lift {
        contact,
        from: c.level,
        to,
        record,
    }))
}

fn write_level(
    tx: &rusqlite::Connection,
    contact: ContactId,
    level: ContactLevel,
    author: Author,
    now: i64,
) -> Result<()> {
    tx.execute(
        "UPDATE contacts SET project_id = ?2, epic_id = ?3, updated_at = ?4, updated_by = ?5 WHERE id = ?1",
        params![
            contact.0,
            level.project().map(|p| p.0),
            level.epic().map(|e| e.0),
            now,
            author
        ],
    )?;
    Ok(())
}

/// Inside the caller's transaction, after a move: the lifts the moved
/// record needs for the contacts it names (a topic: its Report to and its
/// questions' asked; a question: its asked).
pub(crate) fn lift_for(
    tx: &rusqlite::Connection,
    record: Record,
    author: Author,
    now: i64,
) -> Result<Vec<Lift>> {
    let sql = match record {
        Record::Topic(_) => {
            "SELECT report_contact_id, 'topic', id FROM topics WHERE id = ?1 AND report_contact_id IS NOT NULL
             UNION ALL
             SELECT asked_contact_id, 'question', id FROM questions
              WHERE topic_id = ?1 AND deleted_at IS NULL AND asked_contact_id IS NOT NULL"
        }
        Record::Question(_) => {
            "SELECT asked_contact_id, 'question', id FROM questions WHERE id = ?1 AND asked_contact_id IS NOT NULL"
        }
        _ => return Ok(Vec::new()),
    };
    let raw = match record {
        Record::Topic(t) => t.0,
        Record::Question(q) => q.0,
        _ => unreachable!("returned above"),
    };
    let named: Vec<(i64, String, i64)> = {
        let mut stmt = tx.prepare(sql)?;
        stmt.query_map([raw], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut lifts = Vec::new();
    for (c, kind, id) in named {
        let rec = if kind == "topic" {
            Record::Topic(TopicId(id))
        } else {
            Record::Question(QuestionId(id))
        };
        let place = place_of(tx, rec)?;
        lifts.extend(lift(tx, ContactId(c), place, rec, author, now)?);
    }
    Ok(lifts)
}

/// The contact that holds a contact's uses now: itself, or the end of its
/// `merged_into` chain (bounded, so a cycle in a file cannot loop).
pub(crate) fn follow_merges(tx: &rusqlite::Connection, id: i64) -> Result<i64> {
    let mut cur = id;
    for _ in 0..32 {
        let next: Option<i64> = tx
            .query_row(
                "SELECT merged_into FROM contacts WHERE id = ?1",
                [cur],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        match next {
            Some(n) if n != cur => cur = n,
            _ => return Ok(cur),
        }
    }
    Ok(cur)
}

/// Points every reference to a merged contact at the end of its chain
/// (after an import brought a merge made elsewhere).
pub(crate) fn repoint_merged(tx: &rusqlite::Connection) -> Result<()> {
    let merged: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id FROM contacts WHERE merged_into IS NOT NULL")?;
        stmt.query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for id in merged {
        let to = follow_merges(tx, id)?;
        if to == id {
            continue;
        }
        tx.execute(
            "UPDATE questions SET asked_contact_id = ?2 WHERE asked_contact_id = ?1",
            params![id, to],
        )?;
        for table in ["topics", "epics", "notes"] {
            tx.execute(
                &format!("UPDATE {table} SET report_contact_id = ?2 WHERE report_contact_id = ?1"),
                params![id, to],
            )?;
        }
    }
    Ok(())
}

/// Lifts every contact named from outside its scope by a live record
/// (after a restore or an import put records back where a contact cannot
/// be seen). Returns the lifts.
pub(crate) fn lift_out_of_scope(
    tx: &rusqlite::Connection,
    author: Author,
    now: i64,
) -> Result<Vec<Lift>> {
    let refs: Vec<(String, i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT 'question', id, asked_contact_id FROM questions
               WHERE deleted_at IS NULL AND asked_contact_id IS NOT NULL
             UNION ALL SELECT 'topic', id, report_contact_id FROM topics
               WHERE deleted_at IS NULL AND report_contact_id IS NOT NULL
             UNION ALL SELECT 'epic', id, report_contact_id FROM epics
               WHERE deleted_at IS NULL AND report_contact_id IS NOT NULL
             UNION ALL SELECT 'note', id, report_contact_id FROM notes
               WHERE deleted_at IS NULL AND report_contact_id IS NOT NULL",
        )?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut lifts = Vec::new();
    for (kind, id, c) in refs {
        let record = match kind.as_str() {
            "question" => Record::Question(QuestionId(id)),
            "topic" => Record::Topic(TopicId(id)),
            "epic" => Record::Epic(EpicId(id)),
            _ => Record::Note(NoteId(id)),
        };
        // A record whose parent is in the trash is not live here.
        let Ok(place) = place_of(tx, record) else {
            continue;
        };
        lifts.extend(lift(tx, ContactId(c), place, record, author, now)?);
    }
    Ok(lifts)
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

fn current_contact(tx: &rusqlite::Connection, record: Record) -> Result<Option<ContactId>> {
    let (table, id) = report_table(record)?;
    Ok(tx
        .query_row(
            &format!("SELECT report_contact_id FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
            |r| r.get::<_, Option<i64>>(0),
        )
        .optional()?
        .ok_or_else(|| Error::NotFound(record.to_string()))?
        .map(ContactId))
}

fn uuid_of(tx: &rusqlite::Connection, c: Option<ContactId>) -> Result<Option<String>> {
    Ok(match c {
        Some(c) => Some(
            tx.query_row("SELECT uuid FROM contacts WHERE id = ?1", [c.0], |r| {
                r.get(0)
            })?,
        ),
        None => None,
    })
}

/// Inside the caller's transaction: sets or clears the contact of a note,
/// a topic or an epic (checked visible). Another contact resets
/// "reported". On a topic the change is a system event of kind `report`
/// with the contacts' UUIDs. Returns whether anything changed.
pub(crate) fn write_report_to(
    tx: &rusqlite::Connection,
    record: Record,
    contact: Option<ContactId>,
    author: Author,
    now: i64,
) -> Result<bool> {
    let (table, id) = report_table(record)?;
    let current = current_contact(tx, record)?;
    if current == contact {
        return Ok(false);
    }
    if let Some(c) = contact {
        check_ref(tx, c, place_of(tx, record)?, &record.to_string())?;
    }
    let reset = if table == "notes" {
        ""
    } else {
        ", reported_at = NULL, reported_by = NULL"
    };
    tx.execute(
        &format!(
            "UPDATE {table} SET report_contact_id = ?2, updated_at = ?3, updated_by = ?4{reset} WHERE id = ?1"
        ),
        params![id, contact.map(|c| c.0), now, author],
    )?;
    if let Record::Topic(t) = record {
        let (old, new) = (uuid_of(tx, current)?, uuid_of(tx, contact)?);
        system_event(tx, t, EventKind::Report, old, new, author, now)?;
    }
    Ok(true)
}

/// Inside the caller's transaction: a note's contact goes to the topic or
/// epic it is filed into when that record has none (request 19): the
/// first note naming someone, in the order given. A record that has its
/// own keeps it. Filing stays inside one project, so a contact the note
/// sees, the record sees too.
pub(crate) fn carry_person(
    tx: &rusqlite::Connection,
    notes: &[NoteId],
    target: Record,
    author: Author,
    now: i64,
) -> Result<()> {
    if current_contact(tx, target)?.is_some() {
        return Ok(());
    }
    for n in notes {
        let c: Option<i64> = tx.query_row(
            "SELECT report_contact_id FROM notes WHERE id = ?1",
            [n.0],
            |r| r.get(0),
        )?;
        if let Some(c) = c {
            write_report_to(tx, target, Some(ContactId(c)), author, now)?;
            break;
        }
    }
    Ok(())
}

/// A contact's name, cleaned, or refused.
fn name_arg(name: &str) -> Result<String> {
    clean(name)
        .map_err(Error::Invalid)?
        .ok_or_else(|| Error::Invalid("a contact needs a name".into()))
}

/// An optional detail: trimmed, empty is none, at most `max`.
fn detail(field: &str, text: Option<&str>, max: usize) -> Result<Option<String>> {
    let Some(t) = text.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    let n = t.chars().count();
    if n > max {
        return Err(Error::Invalid(format!(
            "{field} is {n} chars; the limit is {max}. Shorten it."
        )));
    }
    Ok(Some(t.to_owned()))
}

/// What to change on a contact; `Some(None)` clears a detail.
#[derive(Clone, Debug, Default)]
pub struct ContactPatch {
    pub name: Option<String>,
    pub job_title: Option<Option<String>>,
    pub company: Option<Option<String>>,
    pub address_note: Option<Option<String>>,
}

/// A contact as the picker and `find(kind="contact")` list it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactSummary {
    pub contact: Contact,
    /// Open items naming them in the place's project.
    pub open: i64,
    /// The newest write of a record naming them (0 when none).
    pub last_named: i64,
}

/// What is still to be reported to one contact (`find(person=...)`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReportTargets {
    /// Not reported and not Dropped, in tree order.
    pub topics: Vec<TopicSummary>,
    /// Not reported, in stored order.
    pub epics: Vec<EpicSummary>,
    /// Open notes naming them, newest first.
    pub notes: Vec<Note>,
}

/// The references to a contact from live records: kind, id, field, title.
const USES_SQL: &str = "
    SELECT 'question', id, 'asked', text FROM questions WHERE asked_contact_id = ?1 AND deleted_at IS NULL
    UNION ALL SELECT 'topic', id, 'report_to', title FROM topics WHERE report_contact_id = ?1 AND deleted_at IS NULL
    UNION ALL SELECT 'epic', id, 'report_to', title FROM epics WHERE report_contact_id = ?1 AND deleted_at IS NULL
    UNION ALL SELECT 'note', id, 'report_to', text FROM notes WHERE report_contact_id = ?1 AND deleted_at IS NULL";

fn uses_in(tx: &rusqlite::Connection, contact: ContactId) -> Result<Vec<ContactUse>> {
    let rows: Vec<(String, i64, String, String)> = {
        let mut stmt = tx.prepare(USES_SQL)?;
        stmt.query_map([contact.0], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    let mut out = Vec::new();
    for (kind, id, field, title) in rows {
        let record = match kind.as_str() {
            "question" => Record::Question(QuestionId(id)),
            "topic" => Record::Topic(TopicId(id)),
            "epic" => Record::Epic(EpicId(id)),
            _ => Record::Note(NoteId(id)),
        };
        // A record whose parent went to the trash is not a live use.
        let Ok(place) = place_of(tx, record) else {
            continue;
        };
        out.push(ContactUse {
            record,
            place,
            title,
            field: if field == "asked" {
                "asked"
            } else {
                "report_to"
            },
        });
    }
    Ok(out)
}

fn check_level(tx: &rusqlite::Connection, level: ContactLevel) -> Result<()> {
    match level {
        ContactLevel::All => {}
        ContactLevel::Project(p) => {
            read_project(tx, p)?;
        }
        ContactLevel::Epic(p, e) => {
            if read_epic(tx, e)?.project != p {
                return Err(Error::Invalid(format!(
                    "{e} belongs to another project than {p}"
                )));
            }
        }
    }
    Ok(())
}

impl Store {
    // ---- contacts (request 22) ----

    /// A new contact at `level`, with its name only (the details are the
    /// human's, written in the app).
    pub fn create_contact(
        &mut self,
        name: &str,
        level: ContactLevel,
        author: Author,
    ) -> Result<ContactId> {
        let name = name_arg(name)?;
        let tx = self.write()?;
        check_level(&tx, level)?;
        let now = now_ms();
        tx.execute(
            "INSERT INTO contacts (uuid, project_id, epic_id, name, name_key, created_by, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?6)",
            params![
                new_uuid(),
                level.project().map(|p| p.0),
                level.epic().map(|e| e.0),
                name,
                person_key(&name),
                author,
                now
            ],
        )?;
        let id = ContactId(tx.last_insert_rowid());
        tx.commit()?;
        Ok(id)
    }

    /// Where a live record sits, for contact visibility.
    pub fn place(&self, record: Record) -> Result<Place> {
        place_of(&self.conn, record)
    }

    /// A live contact.
    pub fn contact(&self, id: ContactId) -> Result<Contact> {
        read_contact(&self.conn, id, false)
    }

    /// A contact, also when it is in the trash (records still name it).
    pub fn contact_any(&self, id: ContactId) -> Result<Contact> {
        read_contact(&self.conn, id, true)
    }

    /// Renames a contact or changes its details (the human's).
    pub fn update_contact(
        &mut self,
        id: ContactId,
        patch: ContactPatch,
        author: Author,
    ) -> Result<()> {
        let name = patch.name.as_deref().map(name_arg).transpose()?;
        let job = match &patch.job_title {
            Some(v) => Some(detail("a job title", v.as_deref(), DETAIL_MAX)?),
            None => None,
        };
        let company = match &patch.company {
            Some(v) => Some(detail("a company", v.as_deref(), DETAIL_MAX)?),
            None => None,
        };
        let note = match &patch.address_note {
            Some(v) => Some(detail("an address note", v.as_deref(), ADDRESS_MAX)?),
            None => None,
        };
        let tx = self.write()?;
        let cur = read_contact(&tx, id, false)?;
        let name = name.unwrap_or(cur.name);
        tx.execute(
            "UPDATE contacts SET name = ?2, name_key = ?3, job_title = ?4, company = ?5, address_note = ?6,
                    updated_at = ?7, updated_by = ?8 WHERE id = ?1",
            params![
                id.0,
                name,
                person_key(&name),
                job.unwrap_or(cur.job_title),
                company.unwrap_or(cur.company),
                note.unwrap_or(cur.address_note),
                now_ms(),
                author
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The live contacts visible from `place`, nearest level first, then
    /// by open items in the place's project, then most recently named.
    pub fn contacts_visible(&self, place: Place) -> Result<Vec<ContactSummary>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {CONTACT_COLS}, {STAMP_COLS},
               (SELECT count(*) FROM questions q WHERE q.asked_contact_id = contacts.id AND q.project_id = ?1
                   AND q.status = 'open' AND q.deleted_at IS NULL)
             + (SELECT count(*) FROM topics t WHERE t.report_contact_id = contacts.id AND t.project_id = ?1
                   AND t.reported_at IS NULL AND t.status <> 'dropped' AND t.deleted_at IS NULL)
             + (SELECT count(*) FROM epics e WHERE e.report_contact_id = contacts.id AND e.project_id = ?1
                   AND e.reported_at IS NULL AND e.deleted_at IS NULL)
             + (SELECT count(*) FROM notes n WHERE n.report_contact_id = contacts.id AND n.project_id = ?1
                   AND n.filed_at IS NULL AND n.deleted_at IS NULL),
               max(coalesce((SELECT max(updated_at) FROM questions q WHERE q.asked_contact_id = contacts.id), 0),
                   coalesce((SELECT max(updated_at) FROM topics t WHERE t.report_contact_id = contacts.id), 0),
                   coalesce((SELECT max(updated_at) FROM epics e WHERE e.report_contact_id = contacts.id), 0),
                   coalesce((SELECT max(updated_at) FROM notes n WHERE n.report_contact_id = contacts.id), 0))
             FROM contacts
             WHERE deleted_at IS NULL
               AND (project_id IS NULL OR (project_id = ?1 AND (epic_id IS NULL OR epic_id IS ?2)))"
        ))?;
        let mut rows: Vec<ContactSummary> = stmt
            .query_map(params![place.project.0, place.epic.map(|e| e.0)], |r| {
                Ok(ContactSummary {
                    contact: contact_row(r)?,
                    open: r.get(15)?,
                    last_named: r.get(16)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        rows.retain(|c| visible(c.contact.level, place));
        rows.sort_by(|a, b| {
            a.contact
                .level
                .width()
                .cmp(&b.contact.level.width())
                .then(b.open.cmp(&a.open))
                .then(b.last_named.cmp(&a.last_named))
                .then(a.contact.name.cmp(&b.contact.name))
        });
        Ok(rows)
    }

    /// Where a contact is named, live records only.
    pub fn contact_uses(&self, id: ContactId) -> Result<Vec<ContactUse>> {
        uses_in(&self.conn, id)
    }

    /// Moves a contact to another level. Up is never refused; down is
    /// refused while a record that names it would not see it
    /// ([`Error::ContactInUse`], with the places).
    pub fn move_contact(
        &mut self,
        id: ContactId,
        level: ContactLevel,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        let cur = read_contact(&tx, id, false)?;
        if cur.level == level {
            return Ok(());
        }
        check_level(&tx, level)?;
        let outside: Vec<(String, &'static str)> = uses_in(&tx, id)?
            .into_iter()
            .filter(|u| !visible(level, u.place))
            .map(|u| (u.record.to_string(), u.field))
            .collect();
        if !outside.is_empty() {
            return Err(Error::ContactInUse {
                contact: id.to_string(),
                level: level_name(level),
                places: outside,
            });
        }
        write_level(&tx, id, level, author, now_ms())?;
        tx.commit()?;
        Ok(())
    }

    /// Merges `from` into `into` in one transaction: references and
    /// events move to `into`, its empty details take `from`'s, the address
    /// notes are joined (`into`'s first), it lands at the wider of the two
    /// levels, and `from` goes to the trash with `merged_into`.
    pub fn merge_contacts(
        &mut self,
        from: ContactId,
        into: ContactId,
        author: Author,
    ) -> Result<()> {
        if from == into {
            return Err(Error::Invalid(
                "a contact cannot be merged into itself".into(),
            ));
        }
        let tx = self.write()?;
        let a = read_contact(&tx, from, false)?;
        let b = read_contact(&tx, into, false)?;
        let now = now_ms();
        let level = a.level.join(b.level);
        let note = match (&b.address_note, &a.address_note) {
            (Some(x), Some(y)) => Some(format!("{x}\n\n{y}")),
            (x, y) => x.clone().or(y.clone()),
        };
        tx.execute(
            "UPDATE contacts SET project_id = ?2, epic_id = ?3, job_title = ?4, company = ?5, address_note = ?6,
                    updated_at = ?7, updated_by = ?8 WHERE id = ?1",
            params![
                into.0,
                level.project().map(|p| p.0),
                level.epic().map(|e| e.0),
                b.job_title.or(a.job_title),
                b.company.or(a.company),
                note,
                now,
                author
            ],
        )?;
        // Every rewritten record is stamped, so an export carries the
        // merge to another machine and its import takes the new reference.
        tx.execute(
            "UPDATE questions SET asked_contact_id = ?2, updated_at = ?3, updated_by = ?4
             WHERE asked_contact_id = ?1",
            params![from.0, into.0, now, author],
        )?;
        for table in ["topics", "epics", "notes"] {
            tx.execute(
                &format!(
                    "UPDATE {table} SET report_contact_id = ?2, updated_at = ?3, updated_by = ?4
                     WHERE report_contact_id = ?1"
                ),
                params![from.0, into.0, now, author],
            )?;
        }
        for col in ["old_value", "new_value"] {
            tx.execute(
                &format!(
                    "UPDATE entries SET {col} = (SELECT uuid FROM contacts WHERE id = ?2),
                            updated_at = ?3, updated_by = ?4
                     WHERE kind = 'system' AND event IN ('report', 'reported')
                       AND {col} = (SELECT uuid FROM contacts WHERE id = ?1)"
                ),
                params![from.0, into.0, now, author],
            )?;
        }
        tx.execute(
            "DELETE FROM contact_distinct WHERE a = ?1 OR b = ?1",
            [from.0],
        )?;
        tx.execute(
            "UPDATE contacts SET merged_into = ?2, deleted_at = ?3, deleted_by = ?4, deleted_batch = ?5
             WHERE id = ?1",
            params![from.0, into.0, now, author, new_uuid()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Takes back a merge (the app's undo): `from` comes back from the
    /// trash, the records in `uses` (what named `from` before the merge)
    /// name it again, and `into` gets back its details and level as
    /// `before` held them.
    pub fn unmerge_contacts(
        &mut self,
        from: ContactId,
        before: &Contact,
        uses: &[Record],
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        let now = now_ms();
        tx.execute(
            "UPDATE contacts SET merged_into = NULL, deleted_at = NULL, deleted_by = NULL, deleted_batch = NULL,
                    updated_at = ?2, updated_by = ?3 WHERE id = ?1",
            params![from.0, now, author],
        )?;
        write_level(&tx, before.id, before.level, author, now)?;
        tx.execute(
            "UPDATE contacts SET job_title = ?2, company = ?3, address_note = ?4 WHERE id = ?1",
            params![
                before.id.0,
                before.job_title,
                before.company,
                before.address_note
            ],
        )?;
        for record in uses {
            let (table, col, id) = match *record {
                Record::Question(q) => ("questions", "asked_contact_id", q.0),
                Record::Topic(t) => ("topics", "report_contact_id", t.0),
                Record::Epic(e) => ("epics", "report_contact_id", e.0),
                Record::Note(n) => ("notes", "report_contact_id", n.0),
                _ => continue,
            };
            tx.execute(
                &format!(
                    "UPDATE {table} SET {col} = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1 AND {col} = ?5"
                ),
                params![id, from.0, now, author, before.id.0],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The names of the contacts with these UUIDs, in the trash too (the
    /// timeline's Report to events store UUIDs and draw the name now).
    pub fn contact_names(
        &self,
        uuids: &[String],
    ) -> Result<std::collections::HashMap<String, String>> {
        let mut out = std::collections::HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM contacts WHERE uuid = ?1")?;
        for u in uuids {
            if let Some(name) = stmt.query_row([u], |r| r.get::<_, String>(0)).optional()? {
                out.insert(u.clone(), name);
            }
        }
        Ok(out)
    }

    /// How many live contacts exist (the sidebar's Contacts row).
    pub fn contact_count(&self) -> Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM contacts WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Every live contact, each with its open items across its whole scope
    /// (the Contacts page's "3 open"): open questions asked to them, topics
    /// and epics not reported, open notes naming them.
    pub fn contacts_all(&self) -> Result<Vec<ContactSummary>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {CONTACT_COLS}, {STAMP_COLS},
               (SELECT count(*) FROM questions q WHERE q.asked_contact_id = contacts.id
                   AND q.status = 'open' AND q.deleted_at IS NULL)
             + (SELECT count(*) FROM topics t WHERE t.report_contact_id = contacts.id
                   AND t.reported_at IS NULL AND t.status <> 'dropped' AND t.deleted_at IS NULL)
             + (SELECT count(*) FROM epics e WHERE e.report_contact_id = contacts.id
                   AND e.reported_at IS NULL AND e.deleted_at IS NULL)
             + (SELECT count(*) FROM notes n WHERE n.report_contact_id = contacts.id
                   AND n.filed_at IS NULL AND n.deleted_at IS NULL),
               0
             FROM contacts WHERE deleted_at IS NULL
             ORDER BY id"
        ))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(ContactSummary {
                    contact: contact_row(r)?,
                    open: r.get(15)?,
                    last_named: r.get(16)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// "Keep both": the pair is no longer flagged as a possible duplicate.
    pub fn mark_distinct(&mut self, a: ContactId, b: ContactId) -> Result<()> {
        let (a, b) = if a.0 < b.0 { (a, b) } else { (b, a) };
        if a == b {
            return Ok(());
        }
        let tx = self.write()?;
        tx.execute(
            "INSERT OR IGNORE INTO contact_distinct (a, b) VALUES (?1, ?2)",
            params![a.0, b.0],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Whether the human kept these two apart.
    pub fn is_distinct(&self, a: ContactId, b: ContactId) -> Result<bool> {
        let (a, b) = if a.0 < b.0 { (a, b) } else { (b, a) };
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM contact_distinct WHERE a = ?1 AND b = ?2)",
            params![a.0, b.0],
            |r| r.get(0),
        )?)
    }

    /// Lifts made by the last moves (a topic to another epic, a question
    /// to another level), oldest first, for the UI's toast; cleared here.
    pub fn take_lifts(&self) -> Vec<Lift> {
        self.lifts.take()
    }

    // ---- Report to (request 19) ----

    /// Sets (`Some`) or clears who a note, a topic or an epic must be
    /// reported to. The contact must be visible from the record.
    pub fn set_report_to(
        &mut self,
        record: Record,
        contact: Option<ContactId>,
        author: Author,
    ) -> Result<()> {
        let tx = self.write()?;
        write_report_to(&tx, record, contact, author, now_ms())?;
        tx.commit()?;
        Ok(())
    }

    /// Marks a topic or an epic reported to its contact, or not reported.
    /// Refused without one. On a topic, a system event of kind `reported`
    /// (new: the contact's UUID when marked; old: when unmarked).
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
        let (contact, at) = match record {
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
        let Some(contact) = contact else {
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
                "UPDATE {table} SET reported_at = ?2, reported_by = ?3, updated_at = ?4, updated_by = ?5 WHERE id = ?1"
            ),
            params![id, when, who, now, author],
        )?;
        if let Record::Topic(t) = record {
            let uuid = uuid_of(&tx, Some(contact.id))?;
            let (old, new) = if reported { (None, uuid) } else { (uuid, None) };
            system_event(&tx, t, EventKind::Reported, old, new, author, now)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The topics, epics and open notes of a project that name `contact`
    /// as Report to and are not marked reported.
    pub fn report_targets(&self, project: ProjectId, contact: ContactId) -> Result<ReportTargets> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {} FROM topics t LEFT JOIN epics e ON e.id = t.epic_id
             WHERE t.project_id = ?1 AND t.deleted_at IS NULL AND t.report_contact_id = ?2
               AND t.reported_at IS NULL AND t.status <> 'dropped'
             ORDER BY t.epic_id IS NOT NULL, e.position, t.epic_id, t.position, t.id",
            super::records::TOPIC_SUMMARY_COLS
        ))?;
        let topics = stmt
            .query_map(params![project.0, contact.0], super::records::topic_summary)?
            .collect::<rusqlite::Result<_>>()?;
        let epics = self
            .epics(project)?
            .into_iter()
            .filter(|e| !e.reported && e.report_to.as_ref().is_some_and(|c| c.id == contact))
            .collect();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {}, {} FROM notes
             WHERE project_id = ?1 AND deleted_at IS NULL AND filed_at IS NULL AND report_contact_id = ?2
             ORDER BY created_at DESC, id DESC",
            super::timeline::NOTE_COLS,
            STAMP_COLS
        ))?;
        let notes = stmt
            .query_map(params![project.0, contact.0], super::timeline::note_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ReportTargets {
            topics,
            epics,
            notes,
        })
    }

    // ---- profile (request 23) ----

    /// The human's own name and job title.
    pub fn profile(&self) -> Result<Profile> {
        Ok(self
            .conn
            .query_row(
                "SELECT name, job_title FROM profile WHERE id = 1",
                [],
                |r| {
                    Ok(Profile {
                        name: r.get(0)?,
                        job_title: r.get(1)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }

    /// Sets the profile; empty text clears a field.
    pub fn set_profile(&mut self, name: Option<&str>, job_title: Option<&str>) -> Result<()> {
        let name = match name {
            Some(n) => clean(n).map_err(Error::Invalid)?,
            None => None,
        };
        let job = detail("a job title", job_title, DETAIL_MAX)?;
        let tx = self.write()?;
        tx.execute(
            "INSERT INTO profile (id, name, job_title, updated_at) VALUES (1, ?1, ?2, ?3)
             ON CONFLICT (id) DO UPDATE SET name = excluded.name, job_title = excluded.job_title,
                 updated_at = excluded.updated_at",
            params![name, job, now_ms()],
        )?;
        tx.commit()?;
        Ok(())
    }
}
