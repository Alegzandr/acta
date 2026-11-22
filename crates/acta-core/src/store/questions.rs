//! Questions (docs/design/data.md request 18; components-questions.md 5):
//! what someone outside the work must answer, on a project, an epic or a
//! topic. One function per change, each writing `updated_*` on the
//! question and on its owner (a question written by Claude Code makes its
//! owner unread, request 1). Questions write no system events: the
//! question records who created, answered or dropped it, and the Decision
//! entry is the deliberate way to put an answer in the timeline.

use super::people::{check_ref, contact_ref, lift_for, owner_place};
use super::{Scope, Store, new_uuid, owner_at, stamp};
use crate::date::{Date, DueState, due_state};
use crate::error::{Error, Result};
use crate::model::{
    Author, ContactId, Entry, EntryId, EntryKind, EpicId, Owner, ProjectId, Question, QuestionId,
    QuestionStatus, Record, TopicId,
};
use crate::time::now_ms;
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;

/// Longest question, in characters: five lines in a 480 column.
pub const QUESTION_MAX: usize = 300;
/// Longest answer, in characters.
pub const ANSWER_MAX: usize = 1_500;

/// A question to ask (the composer, `create` kind question).
#[derive(Clone, Debug)]
pub struct NewQuestion {
    pub owner: Owner,
    pub text: String,
    pub asked_to: Option<ContactId>,
    pub expected_on: Option<Date>,
}

/// Which questions of a project to list (the Questions page, the person
/// page, `find(kind="question")`).
#[derive(Clone, Debug, Default)]
pub struct QuestionFilter {
    /// Empty means every state.
    pub statuses: Vec<QuestionStatus>,
    /// The epic's own questions and those of its topics.
    pub epic: Option<EpicId>,
    /// Only one owner's own questions.
    pub owner: Option<Owner>,
    /// Only questions whose answer date is before this day.
    pub expected_before: Option<Date>,
    /// Only questions asked to this contact.
    pub contact: Option<ContactId>,
}

/// The columns [`question_row`] reads, on the alias `q`.
const QUESTION_COLS: &str = "q.id, q.project_id, q.epic_id, q.topic_id, q.text, q.asked_contact_id,
    (SELECT c.name FROM contacts c WHERE c.id = q.asked_contact_id),
    (SELECT c.deleted_at IS NOT NULL FROM contacts c WHERE c.id = q.asked_contact_id),
    q.expected_on, q.status, q.answer, q.resolved_at, q.resolved_by, q.decision_entry_id, q.position,
    q.created_by, q.uuid, q.created_at, q.updated_at, q.updated_by, q.deleted_at, q.deleted_by";

fn question_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Question> {
    Ok(Question {
        id: QuestionId(r.get(0)?),
        project: ProjectId(r.get(1)?),
        owner: owner_at(r, 1)?,
        text: r.get(4)?,
        asked_to: contact_ref(r, 5)?,
        expected_on: r.get(8)?,
        status: r.get(9)?,
        answer: r.get(10)?,
        resolved_at: r.get(11)?,
        resolved_by: r.get(12)?,
        decision_entry: r.get::<_, Option<i64>>(13)?.map(EntryId),
        position: r.get(14)?,
        created_by: r.get(15)?,
        stamp: stamp(r, 16)?,
    })
}

pub(crate) fn read_question(conn: &rusqlite::Connection, id: QuestionId) -> Result<Question> {
    conn.query_row(
        &format!(
            "SELECT {QUESTION_COLS} FROM questions q WHERE q.id = ?1 AND q.deleted_at IS NULL"
        ),
        [id.0],
        question_row,
    )
    .optional()?
    .ok_or_else(|| Error::NotFound(id.to_string()))
}

/// The question's text: one paragraph, 1 to 300 characters.
fn question_text(text: &str) -> Result<String> {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = t.chars().count();
    if n == 0 {
        return Err(Error::Invalid("a question must not be empty".into()));
    }
    if n > QUESTION_MAX {
        return Err(Error::Invalid(format!(
            "a question is {n} chars; the limit is {QUESTION_MAX}. Shorten it."
        )));
    }
    Ok(t)
}

/// An answer: trimmed, empty is none (answered elsewhere), at most 1,500.
fn answer_text(text: Option<&str>) -> Result<Option<String>> {
    let Some(t) = text.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    let n = t.chars().count();
    if n > ANSWER_MAX {
        return Err(Error::Invalid(format!(
            "an answer is {n} chars; the limit is {ANSWER_MAX}. Shorten it."
        )));
    }
    Ok(Some(t.to_owned()))
}

fn check_day(day: Option<Date>) -> Result<()> {
    match day {
        Some(d) if !d.in_range() => Err(Error::Invalid(format!(
            "an answer date must be in 2000 to 2099, got {d}"
        ))),
        _ => Ok(()),
    }
}

/// The live owner's project and columns: project, epic, topic.
fn owner_cols(tx: &rusqlite::Connection, owner: Owner) -> Result<(i64, Option<i64>, Option<i64>)> {
    super::owner_ids(tx, owner)
}

/// A write to a question is a write to its owner: its Updated value and
/// author follow, and the unread dot of a topic (request 1).
fn touch_owner(tx: &rusqlite::Connection, owner: Owner, author: Author, now: i64) -> Result<()> {
    let (table, id) = match owner {
        Owner::Project(p) => ("projects", p.0),
        Owner::Epic(e) => ("epics", e.0),
        Owner::Topic(t) => ("topics", t.0),
    };
    tx.execute(
        &format!("UPDATE {table} SET updated_at = ?2, updated_by = ?3 WHERE id = ?1"),
        params![id, now, author],
    )?;
    Ok(())
}

fn next_position(tx: &rusqlite::Connection, owner: Owner) -> Result<i64> {
    Ok(tx.query_row(
        &format!(
            "SELECT coalesce(max(position) + 1, 0) FROM questions WHERE {} AND deleted_at IS NULL",
            super::owner_condition(owner)
        ),
        [],
        |r| r.get(0),
    )?)
}

/// The Decision entry's body: the question as a Markdown quote line, a
/// blank line, the answer. No words added by Acta.
pub fn decision_body(question: &str, answer: Option<&str>) -> String {
    match answer {
        Some(a) => format!("> {question}\n\n{a}"),
        None => format!("> {question}"),
    }
}

impl Store {
    /// Adds a question at the end of its owner's list.
    pub fn ask(&mut self, new: NewQuestion, author: Author) -> Result<QuestionId> {
        let text = question_text(&new.text)?;
        let asked = new.asked_to;
        check_day(new.expected_on)?;
        let tx = self.write()?;
        let (project, epic, topic) = owner_cols(&tx, new.owner)?;
        if let Some(c) = asked {
            check_ref(&tx, c, owner_place(&tx, new.owner)?, &new.owner.to_string())?;
        }
        let position = next_position(&tx, new.owner)?;
        let now = now_ms();
        tx.execute(
            "INSERT INTO questions (uuid, project_id, epic_id, topic_id, text, asked_contact_id, expected_on,
                                    status, position, created_by, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', ?8, ?9, ?10, ?10, ?9)",
            params![
                new_uuid(),
                project,
                epic,
                topic,
                text,
                asked.map(|c| c.0),
                new.expected_on,
                position,
                author,
                now
            ],
        )?;
        let id = QuestionId(tx.last_insert_rowid());
        touch_owner(&tx, new.owner, author, now)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn question(&self, id: QuestionId) -> Result<Question> {
        read_question(&self.conn, id)
    }

    /// An owner's own live questions: open ones in stored order, then the
    /// closed ones, newest closed first.
    pub fn questions_of(&self, owner: Owner) -> Result<Vec<Question>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {QUESTION_COLS} FROM questions q
             WHERE {} AND q.deleted_at IS NULL
             ORDER BY q.status <> 'open', CASE WHEN q.status = 'open' THEN q.position ELSE -q.resolved_at END, q.id",
            super::owner_condition(owner)
                .replace("project_id", "q.project_id")
                .replace("epic_id", "q.epic_id")
                .replace("topic_id", "q.topic_id")
        ))?;
        let rows = stmt.query_map([], question_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The live questions of a project matching `filter`, overdue first,
    /// then by answer date (undated last), then by owner (project, epics,
    /// topics) and stored order.
    pub fn find_questions(
        &self,
        project: ProjectId,
        filter: &QuestionFilter,
    ) -> Result<Vec<Question>> {
        let mut cond = vec![
            "q.project_id = ?1".to_owned(),
            "q.deleted_at IS NULL".to_owned(),
        ];
        let mut args: Vec<rusqlite::types::Value> = vec![project.0.into()];
        if !filter.statuses.is_empty() {
            let list: Vec<String> = filter
                .statuses
                .iter()
                .map(|s| format!("'{}'", s.as_str()))
                .collect();
            cond.push(format!("q.status IN ({})", list.join(", ")));
        }
        if let Some(e) = filter.epic {
            args.push(e.0.into());
            let n = args.len();
            cond.push(format!(
                "(q.epic_id = ?{n} OR q.topic_id IN (SELECT id FROM topics WHERE epic_id = ?{n} AND deleted_at IS NULL))"
            ));
        }
        match filter.owner {
            Some(Owner::Project(_)) => cond.push("q.epic_id IS NULL AND q.topic_id IS NULL".into()),
            Some(Owner::Epic(e)) => cond.push(format!("q.epic_id = {}", e.0)),
            Some(Owner::Topic(t)) => cond.push(format!("q.topic_id = {}", t.0)),
            None => {}
        }
        if let Some(d) = filter.expected_before {
            args.push(d.to_string().into());
            cond.push(format!(
                "q.expected_on IS NOT NULL AND q.expected_on < ?{}",
                args.len()
            ));
        }
        if let Some(c) = filter.contact {
            args.push(c.0.into());
            cond.push(format!("q.asked_contact_id = ?{}", args.len()));
        }
        let sql = format!(
            "SELECT {QUESTION_COLS} FROM questions q
             WHERE {}
             ORDER BY q.expected_on IS NULL, q.expected_on, q.topic_id IS NOT NULL, q.epic_id IS NOT NULL,
                      coalesce(q.topic_id, q.epic_id, 0), q.position, q.id",
            cond.join(" AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), question_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Open questions per topic of a project, as `(open, overdue)`, from
    /// one grouped query (the epic brief's `· 2q`, the topic table's
    /// Questions column).
    pub fn open_questions_by_topic(
        &self,
        project: ProjectId,
        today: Date,
    ) -> Result<HashMap<TopicId, (i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT topic_id, count(*), sum(expected_on IS NOT NULL AND expected_on < ?2)
             FROM questions
             WHERE project_id = ?1 AND topic_id IS NOT NULL AND status = 'open' AND deleted_at IS NULL
             GROUP BY topic_id",
        )?;
        let rows = stmt.query_map(params![project.0, today.to_string()], |r| {
            Ok((TopicId(r.get(0)?), (r.get(1)?, r.get(2)?)))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Open questions at every level of a scope whose answer date has
    /// passed, with the days late, earliest date first: the Overview's
    /// fifth attention line, the `answers late` segment of the agent's
    /// `Attention:` line and the Questions page's Overdue filter
    /// (components-questions.md 5). Overdue is [`due_state`] on the answer
    /// date, so they always agree.
    pub fn late_answers(&self, scope: Scope, today: Date) -> Result<Vec<(QuestionId, i64)>> {
        let (project, epic) = match scope {
            Scope::Project(p) => (self.project(p)?.id, None),
            Scope::Epic(e) => (self.epic(e)?.project, Some(e)),
        };
        let filter = QuestionFilter {
            statuses: vec![QuestionStatus::Open],
            epic,
            expected_before: Some(today),
            ..QuestionFilter::default()
        };
        Ok(self
            .find_questions(project, &filter)?
            .into_iter()
            .filter_map(|q| match due_state(q.expected_on, today, q.is_open()) {
                DueState::Overdue(d) => Some((q.id, d)),
                _ => None,
            })
            .collect())
    }

    /// Changes the text and, with `Some`, the person asked (`Some(None)`
    /// clears). Nothing changed writes nothing.
    pub fn edit_question(
        &mut self,
        id: QuestionId,
        text: Option<&str>,
        asked_to: Option<Option<ContactId>>,
        author: Author,
    ) -> Result<()> {
        let text = text.map(question_text).transpose()?;
        let tx = self.write()?;
        let cur = read_question(&tx, id)?;
        let text = text.unwrap_or(cur.text.clone());
        let was = cur.asked_to.as_ref().map(|c| c.id);
        let asked = asked_to.unwrap_or(was);
        if text == cur.text && asked == was {
            return Ok(());
        }
        if let Some(c) = asked.filter(|c| Some(*c) != was) {
            check_ref(&tx, c, owner_place(&tx, cur.owner)?, &id.to_string())?;
        }
        let now = now_ms();
        tx.execute(
            "UPDATE questions SET text = ?2, asked_contact_id = ?3, updated_at = ?4, updated_by = ?5
             WHERE id = ?1",
            params![id.0, text, asked.map(|c| c.0), now, author],
        )?;
        touch_owner(&tx, cur.owner, author, now)?;
        tx.commit()?;
        Ok(())
    }

    /// Sets or clears the day the answer is expected.
    pub fn set_expected(
        &mut self,
        id: QuestionId,
        day: Option<Date>,
        author: Author,
    ) -> Result<()> {
        check_day(day)?;
        let tx = self.write()?;
        let cur = read_question(&tx, id)?;
        if cur.expected_on == day {
            return Ok(());
        }
        let now = now_ms();
        tx.execute(
            "UPDATE questions SET expected_on = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
            params![id.0, day, now, author],
        )?;
        touch_owner(&tx, cur.owner, author, now)?;
        tx.commit()?;
        Ok(())
    }

    /// Marks a question answered with `answer` (none: answered elsewhere),
    /// or corrects the answer of one already answered (its time and author
    /// stay). With `log_decision`, also adds a Decision entry to the
    /// owner's timeline by the same author, in the same transaction, and
    /// links it; refused when the owner is not a topic (only topics have a
    /// timeline) or when an entry was already logged. Returns the entry.
    pub fn answer(
        &mut self,
        id: QuestionId,
        answer: Option<&str>,
        log_decision: bool,
        author: Author,
    ) -> Result<Option<Entry>> {
        let answer = answer_text(answer)?;
        self.atomically(|store| {
            let cur = read_question(&store.conn, id)?;
            let topic = match (log_decision, cur.owner) {
                (false, _) => None,
                (true, Owner::Topic(t)) => Some(t),
                (true, other) => {
                    return Err(Error::Invalid(format!(
                        "log_decision needs a question on a topic; {id} is on {other}"
                    )));
                }
            };
            if let (Some(_), Some(e)) = (topic, cur.decision_entry)
                && let Ok(entry) = store.entry(e)
            {
                return Err(Error::Invalid(format!(
                    "{id} is already logged as {}#{}",
                    entry.topic,
                    entry.seq.unwrap_or(0)
                )));
            }
            let entry = match topic {
                Some(t) => Some(store.log(
                    t,
                    super::NewEntry {
                        kind: EntryKind::Decision,
                        text: decision_body(&cur.text, answer.as_deref()),
                        state: None,
                        status: None,
                    },
                    author,
                )?),
                None => None,
            };
            let now = now_ms();
            let (resolved_at, resolved_by) = match cur.status {
                QuestionStatus::Answered => (cur.resolved_at, cur.resolved_by),
                _ => (Some(now), Some(author)),
            };
            let tx = store.write()?;
            tx.execute(
                "UPDATE questions SET status = 'answered', answer = ?2, resolved_at = ?3, resolved_by = ?4,
                        decision_entry_id = coalesce(?5, decision_entry_id), updated_at = ?6, updated_by = ?7
                 WHERE id = ?1",
                params![
                    id.0,
                    answer,
                    resolved_at,
                    resolved_by,
                    entry.as_ref().map(|e| e.id.0),
                    now,
                    author
                ],
            )?;
            touch_owner(&tx, cur.owner, author, now)?;
            tx.commit()?;
            Ok(entry)
        })
    }

    /// Closes a question without an answer. Its answer text, if any, stays.
    pub fn drop_question(&mut self, id: QuestionId, author: Author) -> Result<()> {
        self.resolve(id, QuestionStatus::Dropped, author)
    }

    /// Puts an answered or dropped question back at the end of the open
    /// list; `resolved_*` are cleared, the answer text is kept.
    pub fn reopen_question(&mut self, id: QuestionId, author: Author) -> Result<()> {
        self.resolve(id, QuestionStatus::Open, author)
    }

    fn resolve(&mut self, id: QuestionId, to: QuestionStatus, author: Author) -> Result<()> {
        let tx = self.write()?;
        let cur = read_question(&tx, id)?;
        if cur.status == to {
            return Ok(());
        }
        let now = now_ms();
        let (at, by, position) = match to {
            QuestionStatus::Open => (None, None, next_position(&tx, cur.owner)?),
            _ => (Some(now), Some(author), cur.position),
        };
        tx.execute(
            "UPDATE questions SET status = ?2, resolved_at = ?3, resolved_by = ?4, position = ?5,
                    updated_at = ?6, updated_by = ?7 WHERE id = ?1",
            params![id.0, to, at, by, position, now, author],
        )?;
        touch_owner(&tx, cur.owner, author, now)?;
        tx.commit()?;
        Ok(())
    }

    /// Moves a question to another level of the same project, at the end
    /// of that owner's list.
    pub fn move_question(&mut self, id: QuestionId, to: Owner, author: Author) -> Result<()> {
        let tx = self.write()?;
        let cur = read_question(&tx, id)?;
        if cur.owner == to {
            return Ok(());
        }
        let (project, epic, topic) = owner_cols(&tx, to)?;
        if project != cur.project.0 {
            return Err(Error::Invalid(format!(
                "{to} belongs to another project than {id}"
            )));
        }
        let position = next_position(&tx, to)?;
        let now = now_ms();
        tx.execute(
            "UPDATE questions SET epic_id = ?2, topic_id = ?3, position = ?4, updated_at = ?5, updated_by = ?6
             WHERE id = ?1",
            params![id.0, epic, topic, position, now, author],
        )?;
        touch_owner(&tx, cur.owner, author, now)?;
        touch_owner(&tx, to, author, now)?;
        // The contact asked follows: lifted when the new owner cannot see it.
        let lifts = lift_for(&tx, Record::Question(id), author, now)?;
        tx.commit()?;
        self.lifts.borrow_mut().extend(lifts);
        Ok(())
    }

    /// Moves an open question to `index` among its owner's open questions
    /// (clamped). Questions whose position changes get a new `updated_at`,
    /// so an import carries the order.
    pub fn reorder_question(&mut self, id: QuestionId, index: usize, author: Author) -> Result<()> {
        let tx = self.write()?;
        let cur = read_question(&tx, id)?;
        if !cur.is_open() {
            return Err(Error::Invalid(format!(
                "{id} is closed; only open questions are reordered"
            )));
        }
        let mut ids: Vec<(i64, i64)> = {
            let mut stmt = tx.prepare(&format!(
                "SELECT id, position FROM questions WHERE {} AND status = 'open' AND deleted_at IS NULL
                 ORDER BY position, id",
                super::owner_condition(cur.owner)
            ))?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let from = ids
            .iter()
            .position(|(q, _)| *q == id.0)
            .ok_or_else(|| Error::NotFound(id.to_string()))?;
        let moved = ids.remove(from);
        ids.insert(index.min(ids.len()), moved);
        let now = now_ms();
        let base = ids.iter().map(|(_, p)| *p).min().unwrap_or(0);
        for (i, (q, old)) in ids.iter().enumerate() {
            let pos = base + i as i64;
            if *old != pos {
                tx.execute(
                    "UPDATE questions SET position = ?2, updated_at = ?3, updated_by = ?4 WHERE id = ?1",
                    params![q, pos, now, author],
                )?;
            }
        }
        touch_owner(&tx, cur.owner, author, now)?;
        tx.commit()?;
        Ok(())
    }
}
