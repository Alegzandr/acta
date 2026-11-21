//! The Overview of a project or an epic (docs/design/data.md request 17;
//! components-records.md 5): a few counts of what needs attention, and the
//! records behind each. Nothing is stored; everything is computed from
//! records that exist.
//!
//! Every count is the length of the list [`Store::overview_list`] returns
//! for the same filter, computed by the same code, so a number and the list
//! it opens cannot disagree. The one figure that is not a list length is
//! [`Overview::decisions`], the number of decision entries ("2 decisions"),
//! whose list is their distinct topics ([`Overview::decided`]).
//!
//! Days are local calendar days (copy.md 10.2.1). `today` and the
//! [`Zone`] are passed in, never read inside, so the edges are testable.
//! "Since" is local midnight 6 days before today: the window is today and
//! the six days before it.

use super::Store;
use crate::date::{Date, DueState, Zone, due_state};
use crate::error::Result;
use crate::model::{EpicId, KbId, NoteId, Progress, ProjectId, Record, Status, TopicId};
use rusqlite::{OptionalExtension, params};
use std::collections::{HashMap, HashSet};

/// Days without activity before an Active topic is called idle.
pub const IDLE_DAYS: i64 = 7;
/// Days before an inbox note makes the inbox line appear.
pub const INBOX_DAYS: i64 = 7;
/// The "Last 7 days" window, today included.
pub const WEEK_DAYS: i64 = 7;
/// Days without an update before a memory is called stale.
pub const STALE_MEMORY_DAYS: i64 = 90;

/// What an Overview is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Project(ProjectId),
    Epic(EpicId),
}

/// The records behind one figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverviewFilter {
    /// Topics with status Blocked, in tree order.
    Blocked,
    /// Open topics due before today, oldest date first. Topics only, so
    /// the count equals the topic table's Overdue filter.
    Overdue,
    /// Active topics whose last activity is at least 7 local days old,
    /// the longest idle first.
    Idle,
    /// Open notes of the project, oldest first. Project-wide, also for an
    /// epic scope.
    Inbox,
    /// Topics Done now with a status change to done in the window.
    Done,
    /// Topics with a decision entry in the window.
    Decided,
    /// Topics created in the window.
    Created,
    /// Memories at every level of the project not updated for 90 local
    /// days, oldest first. Project-wide, also for an epic scope.
    StaleMemories,
}

impl OverviewFilter {
    pub const ALL: [OverviewFilter; 8] = [
        OverviewFilter::Blocked,
        OverviewFilter::Overdue,
        OverviewFilter::Idle,
        OverviewFilter::Inbox,
        OverviewFilter::Done,
        OverviewFilter::Decided,
        OverviewFilter::Created,
        OverviewFilter::StaleMemories,
    ];
}

/// One record behind a figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewItem {
    /// A topic, a note (Inbox) or a knowledge entry (StaleMemories).
    pub record: Record,
    /// Days overdue (Overdue), days since the last activity (Idle), the
    /// note's age (Inbox) or days since the update (StaleMemories);
    /// `None` for the other filters.
    pub days: Option<i64>,
}

/// One row of the Epics block behind "More".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverviewEpic {
    /// `None` is the "No epic" row: loose topics.
    pub epic: Option<EpicId>,
    pub title: String,
    pub progress: Progress,
    pub due_on: Option<Date>,
}

/// The figures of request 17.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overview {
    pub blocked: usize,
    pub overdue: usize,
    pub idle: usize,
    /// Open notes; the inbox line shows only when
    /// [`Overview::inbox_needs_attention`].
    pub inbox: usize,
    /// Age in local days of the oldest open note.
    pub inbox_oldest_days: Option<i64>,
    /// Last 7 days: topics done.
    pub done: usize,
    /// Last 7 days: decision entries.
    pub decisions: usize,
    /// Last 7 days: distinct topics with a decision (the list of
    /// `decisions`).
    pub decided: usize,
    /// Last 7 days: topics created.
    pub created: usize,
    /// The newest entry of the scope, computed only when nothing was done,
    /// decided or created in the window ("Last activity {when}").
    pub last_activity: Option<i64>,
    /// Epics not complete that have a topic not Dropped, in stored order,
    /// "No epic" first when loose topics not Dropped exist. Project scope
    /// only.
    pub epics: Vec<OverviewEpic>,
    /// Epics whose every topic not Dropped is Done. Project scope only.
    pub complete_epics: usize,
    pub stale_memories: usize,
}

impl Overview {
    pub fn inbox_needs_attention(&self) -> bool {
        self.inbox_oldest_days.is_some_and(|d| d >= INBOX_DAYS)
    }

    /// How many attention lines apply (blocked, overdue, idle, old inbox).
    pub fn attention_lines(&self) -> usize {
        [
            self.blocked > 0,
            self.overdue > 0,
            self.idle > 0,
            self.inbox_needs_attention(),
        ]
        .iter()
        .filter(|on| **on)
        .count()
    }
}

/// The topics of a scope, in tree order: loose topics first, then by epic.
const TOPICS_SQL: &str = "SELECT t.id, t.epic_id, t.status, t.due_on, t.created_at, t.position
    FROM topics t WHERE t.project_id = ?1 AND t.deleted_at IS NULL";
/// The same for an epic.
const EPIC_TOPICS_SQL: &str = "SELECT t.id, t.epic_id, t.status, t.due_on, t.created_at, t.position
    FROM topics t WHERE t.project_id = ?1 AND t.epic_id = ?2 AND t.deleted_at IS NULL";
/// The live epics of a project, in stored order.
const EPICS_SQL: &str = "SELECT e.id, e.title, e.due_on FROM epics e
    WHERE e.project_id = ?1 AND e.deleted_at IS NULL ORDER BY e.position, e.id";
/// Active topics with their newest live entry (any kind), one probe each
/// on `entries_live`, which answers without reading the entry rows.
const ACTIVE_SQL: &str = "SELECT t.id, t.created_at,
        (SELECT e.created_at FROM entries e
          WHERE e.topic_id = t.id AND e.deleted_at IS NULL
          ORDER BY e.created_at DESC LIMIT 1)
    FROM topics t
    WHERE t.project_id = ?1 AND t.status = 'active' AND t.deleted_at IS NULL";
/// The window's status changes to done and decisions. The condition
/// repeats the index's own, so `entries_recent` is usable.
const RECENT_SQL: &str = "SELECT e.topic_id, e.kind FROM entries e
    WHERE e.deleted_at IS NULL
      AND (kind = 'decision' OR (kind = 'system' AND event = 'status'))
      AND e.created_at >= ?1
      AND (e.kind = 'decision' OR e.new_value = 'done')";
/// The open notes of a project, oldest first.
const INBOX_SQL: &str = "SELECT n.id, n.created_at FROM notes n
    WHERE n.project_id = ?1 AND n.deleted_at IS NULL AND n.filed_at IS NULL
    ORDER BY n.created_at, n.id";
/// Memories of a project updated before a cutoff, oldest first.
const STALE_SQL: &str = "SELECT k.id, k.updated_at FROM kb_entries k
    WHERE k.project_id = ?1 AND k.deleted_at IS NULL AND k.kind = 'memory' AND k.updated_at < ?2
    ORDER BY k.updated_at, k.id";
/// The newest live entry of any topic of a project.
const LAST_ACTIVITY_SQL: &str = "SELECT max((SELECT e.created_at FROM entries e
          WHERE e.topic_id = t.id AND e.deleted_at IS NULL
          ORDER BY e.created_at DESC LIMIT 1))
    FROM topics t WHERE t.project_id = ?1 AND t.deleted_at IS NULL";
/// The same for an epic.
const LAST_ACTIVITY_EPIC_SQL: &str = "SELECT max((SELECT e.created_at FROM entries e
          WHERE e.topic_id = t.id AND e.deleted_at IS NULL
          ORDER BY e.created_at DESC LIMIT 1))
    FROM topics t WHERE t.project_id = ?1 AND t.epic_id = ?2 AND t.deleted_at IS NULL";

struct ScopeTopic {
    id: TopicId,
    epic: Option<EpicId>,
    status: Status,
    due_on: Option<Date>,
    created_at: i64,
    position: i64,
}

/// What every figure is computed from, read once.
struct Facts {
    project: ProjectId,
    epic: Option<EpicId>,
    /// In tree order.
    topics: Vec<ScopeTopic>,
}

impl Store {
    /// The figures of a project or an epic for a reader on `today` in
    /// `zone` (request 17).
    pub fn overview(&self, scope: Scope, today: Date, zone: Zone) -> Result<Overview> {
        let facts = self.facts(scope)?;
        let len = |f| self.list(&facts, f, today, zone).map(|l| l.len());
        let inbox = self.list(&facts, OverviewFilter::Inbox, today, zone)?;
        let decisions = self.recent(&facts, today, zone)?.1;
        let mut o = Overview {
            blocked: len(OverviewFilter::Blocked)?,
            overdue: len(OverviewFilter::Overdue)?,
            idle: len(OverviewFilter::Idle)?,
            inbox: inbox.len(),
            inbox_oldest_days: inbox.first().and_then(|i| i.days),
            done: len(OverviewFilter::Done)?,
            decisions: decisions.values().sum(),
            decided: len(OverviewFilter::Decided)?,
            created: len(OverviewFilter::Created)?,
            stale_memories: len(OverviewFilter::StaleMemories)?,
            ..Overview::default()
        };
        if o.done + o.decisions + o.created == 0 {
            o.last_activity = self.last_activity(&facts)?;
        }
        if facts.epic.is_none() {
            (o.epics, o.complete_epics) = self.epic_rows(&facts)?;
        }
        Ok(o)
    }

    /// The records behind one figure of [`Store::overview`].
    pub fn overview_list(
        &self,
        scope: Scope,
        filter: OverviewFilter,
        today: Date,
        zone: Zone,
    ) -> Result<Vec<OverviewItem>> {
        let facts = self.facts(scope)?;
        self.list(&facts, filter, today, zone)
    }

    /// Days since an Active topic's last activity when it is idle by the
    /// definition of [`OverviewFilter::Idle`]; `None` when it is not Active
    /// or not idle. For the agent's topic header (`· no activity 12d`).
    pub fn topic_idle_days(&self, topic: TopicId, today: Date, zone: Zone) -> Result<Option<i64>> {
        let t = self.topic(topic)?;
        if t.status != Status::Active {
            return Ok(None);
        }
        let last: Option<i64> = self
            .conn
            .query_row(
                "SELECT e.created_at FROM entries e
                  WHERE e.topic_id = ?1 AND e.deleted_at IS NULL
                  ORDER BY e.created_at DESC LIMIT 1",
                [topic.0],
                |r| r.get(0),
            )
            .optional()?;
        let last = last.unwrap_or(t.stamp.created_at);
        let cutoff = zone.midnight(today.add_days(1 - IDLE_DAYS));
        Ok((last < cutoff).then(|| zone.date_of(last).days_until(today)))
    }

    fn facts(&self, scope: Scope) -> Result<Facts> {
        let (project, epic) = match scope {
            Scope::Project(p) => (self.project(p)?.id, None),
            Scope::Epic(e) => (self.epic(e)?.project, Some(e)),
        };
        let map = |r: &rusqlite::Row<'_>| {
            Ok(ScopeTopic {
                id: TopicId(r.get(0)?),
                epic: r.get::<_, Option<i64>>(1)?.map(EpicId),
                status: r.get(2)?,
                due_on: r.get(3)?,
                created_at: r.get(4)?,
                position: r.get(5)?,
            })
        };
        let mut topics: Vec<ScopeTopic> = match epic {
            None => self
                .conn
                .prepare(TOPICS_SQL)?
                .query_map([project.0], map)?
                .collect::<rusqlite::Result<_>>()?,
            Some(e) => self
                .conn
                .prepare(EPIC_TOPICS_SQL)?
                .query_map([project.0, e.0], map)?
                .collect::<rusqlite::Result<_>>()?,
        };
        // Tree order: loose topics, then each epic's in the epics' order.
        let epic_rank: HashMap<EpicId, usize> = if epic.is_none() {
            self.conn
                .prepare(EPICS_SQL)?
                .query_map([project.0], |r| Ok(EpicId(r.get(0)?)))?
                .enumerate()
                .map(|(i, e)| e.map(|e| (e, i)))
                .collect::<rusqlite::Result<_>>()?
        } else {
            HashMap::new()
        };
        topics.sort_by_key(|t| {
            let rank = t
                .epic
                .map_or(0, |e| epic_rank.get(&e).map_or(usize::MAX, |r| r + 1));
            (rank, t.position, t.id.0)
        });
        Ok(Facts {
            project,
            epic,
            topics,
        })
    }

    fn list(
        &self,
        facts: &Facts,
        filter: OverviewFilter,
        today: Date,
        zone: Zone,
    ) -> Result<Vec<OverviewItem>> {
        let topic = |t: &ScopeTopic, days: Option<i64>| OverviewItem {
            record: Record::Topic(t.id),
            days,
        };
        let since = zone.midnight(today.add_days(1 - WEEK_DAYS));
        Ok(match filter {
            OverviewFilter::Blocked => facts
                .topics
                .iter()
                .filter(|t| t.status == Status::Blocked)
                .map(|t| topic(t, None))
                .collect(),
            OverviewFilter::Overdue => {
                let mut late: Vec<(&ScopeTopic, i64)> = facts
                    .topics
                    .iter()
                    .filter_map(|t| match due_state(t.due_on, today, t.status.is_open()) {
                        DueState::Overdue(days) => Some((t, days)),
                        _ => None,
                    })
                    .collect();
                // Stable: the same date keeps the tree order.
                late.sort_by_key(|(t, _)| t.due_on);
                late.into_iter().map(|(t, d)| topic(t, Some(d))).collect()
            }
            OverviewFilter::Idle => {
                let cutoff = zone.midnight(today.add_days(1 - IDLE_DAYS));
                let in_scope: HashSet<TopicId> = facts.topics.iter().map(|t| t.id).collect();
                let mut idle: Vec<(TopicId, i64)> = Vec::new();
                let mut stmt = self.conn.prepare(ACTIVE_SQL)?;
                let rows = stmt.query_map([facts.project.0], |r| {
                    let created: i64 = r.get(1)?;
                    let last: Option<i64> = r.get(2)?;
                    Ok((TopicId(r.get(0)?), last.unwrap_or(created)))
                })?;
                for row in rows {
                    let (id, last) = row?;
                    if last < cutoff && in_scope.contains(&id) {
                        idle.push((id, last));
                    }
                }
                idle.sort_by_key(|(id, last)| (*last, id.0));
                idle.into_iter()
                    .map(|(id, last)| OverviewItem {
                        record: Record::Topic(id),
                        days: Some(zone.date_of(last).days_until(today)),
                    })
                    .collect()
            }
            OverviewFilter::Inbox => {
                let mut stmt = self.conn.prepare(INBOX_SQL)?;
                let rows = stmt.query_map([facts.project.0], |r| {
                    Ok((NoteId(r.get(0)?), r.get::<_, i64>(1)?))
                })?;
                rows.map(|r| {
                    r.map(|(id, at)| OverviewItem {
                        record: Record::Note(id),
                        days: Some(zone.date_of(at).days_until(today)),
                    })
                })
                .collect::<rusqlite::Result<_>>()?
            }
            OverviewFilter::Done => {
                let done = self.recent(facts, today, zone)?.0;
                facts
                    .topics
                    .iter()
                    .filter(|t| t.status == Status::Done && done.contains(&t.id))
                    .map(|t| topic(t, None))
                    .collect()
            }
            OverviewFilter::Decided => {
                let decided = self.recent(facts, today, zone)?.1;
                facts
                    .topics
                    .iter()
                    .filter(|t| decided.contains_key(&t.id))
                    .map(|t| topic(t, None))
                    .collect()
            }
            OverviewFilter::Created => facts
                .topics
                .iter()
                .filter(|t| t.created_at >= since)
                .map(|t| topic(t, None))
                .collect(),
            OverviewFilter::StaleMemories => {
                let cutoff = zone.midnight(today.add_days(1 - STALE_MEMORY_DAYS));
                let mut stmt = self.conn.prepare(STALE_SQL)?;
                let rows = stmt.query_map(params![facts.project.0, cutoff], |r| {
                    Ok((KbId(r.get(0)?), r.get::<_, i64>(1)?))
                })?;
                rows.map(|r| {
                    r.map(|(id, at)| OverviewItem {
                        record: Record::Kb(id),
                        days: Some(zone.date_of(at).days_until(today)),
                    })
                })
                .collect::<rusqlite::Result<_>>()?
            }
        })
    }

    /// Topics with a status change to done in the window, and decision
    /// entries per topic, both limited to the scope.
    fn recent(
        &self,
        facts: &Facts,
        today: Date,
        zone: Zone,
    ) -> Result<(HashSet<TopicId>, HashMap<TopicId, usize>)> {
        let since = zone.midnight(today.add_days(1 - WEEK_DAYS));
        let in_scope: HashSet<TopicId> = facts.topics.iter().map(|t| t.id).collect();
        let mut done = HashSet::new();
        let mut decided: HashMap<TopicId, usize> = HashMap::new();
        let mut stmt = self.conn.prepare(RECENT_SQL)?;
        let rows = stmt.query_map([since], |r| {
            Ok((TopicId(r.get(0)?), r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, kind) = row?;
            if !in_scope.contains(&id) {
                continue;
            }
            if kind == "decision" {
                *decided.entry(id).or_default() += 1;
            } else {
                done.insert(id);
            }
        }
        Ok((done, decided))
    }

    fn last_activity(&self, facts: &Facts) -> Result<Option<i64>> {
        Ok(match facts.epic {
            None => self
                .conn
                .prepare(LAST_ACTIVITY_SQL)?
                .query_row([facts.project.0], |r| r.get(0))?,
            Some(e) => self
                .conn
                .prepare(LAST_ACTIVITY_EPIC_SQL)?
                .query_row([facts.project.0, e.0], |r| r.get(0))?,
        })
    }

    /// The Epics block: open rows and the count of complete epics.
    fn epic_rows(&self, facts: &Facts) -> Result<(Vec<OverviewEpic>, usize)> {
        let progress_of = |epic: Option<EpicId>| {
            let mut p = Progress::default();
            for t in facts.topics.iter().filter(|t| t.epic == epic) {
                if t.status != Status::Dropped {
                    p.total += 1;
                }
                if t.status == Status::Done {
                    p.done += 1;
                }
            }
            p
        };
        let mut rows = Vec::new();
        let loose = progress_of(None);
        if loose.total > 0 {
            rows.push(OverviewEpic {
                epic: None,
                title: String::new(),
                progress: loose,
                due_on: None,
            });
        }
        let mut complete = 0;
        let mut stmt = self.conn.prepare(EPICS_SQL)?;
        let epics = stmt.query_map([facts.project.0], |r| {
            Ok((EpicId(r.get(0)?), r.get::<_, String>(1)?, r.get(2)?))
        })?;
        for epic in epics {
            let (id, title, due_on) = epic?;
            let progress = progress_of(Some(id));
            if progress.total == 0 {
                continue;
            }
            if progress.done == progress.total {
                complete += 1;
            } else {
                rows.push(OverviewEpic {
                    epic: Some(id),
                    title,
                    progress,
                    due_on,
                });
            }
        }
        Ok((rows, complete))
    }
}

/// Every query of the Overview with the index it must use, for the
/// `EXPLAIN QUERY PLAN` test.
#[cfg(test)]
pub(crate) const QUERIES: &[(&str, &str)] = &[
    (TOPICS_SQL, "topics_project"),
    (EPIC_TOPICS_SQL, "topics_project"),
    (EPICS_SQL, "epics_project"),
    (ACTIVE_SQL, "entries_live"),
    (RECENT_SQL, "entries_recent"),
    (INBOX_SQL, "notes_project"),
    (STALE_SQL, "kb_project"),
    (LAST_ACTIVITY_SQL, "entries_live"),
    (LAST_ACTIVITY_EPIC_SQL, "entries_live"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Author, KbKind, Owner};
    use crate::store::test_support::{TestStore, open};
    use crate::store::{NewKb, NewTopic};
    use crate::time::MS_PER_DAY;

    const H: Author = Author::Human;

    /// Central European time in 2026: UTC+1, UTC+2 from 29 March 01:00 UTC.
    fn paris(ms: i64) -> i64 {
        let start = Date::from_ymd(2026, 3, 29).unwrap().days() * MS_PER_DAY + 3_600_000;
        let end = Date::from_ymd(2026, 10, 25).unwrap().days() * MS_PER_DAY + 3_600_000;
        if (start..end).contains(&ms) {
            7200
        } else {
            3600
        }
    }

    const ZONE: Zone = Zone::from_fn(paris);

    /// Tuesday 31 March 2026: the window (25 to 31 March) holds the
    /// spring daylight-saving change.
    fn today() -> Date {
        Date::from_ymd(2026, 3, 31).unwrap()
    }

    /// Noon, local, `days` before today.
    fn noon(days: i64) -> i64 {
        ZONE.midnight(today().add_days(-days)) + 12 * 3_600_000
    }

    fn new_topic(
        s: &mut Store,
        p: ProjectId,
        epic: Option<EpicId>,
        title: &str,
        status: Status,
    ) -> TopicId {
        s.create_topic(
            NewTopic {
                project: p,
                epic,
                title: title.into(),
                description: String::new(),
                status,
            },
            H,
        )
        .unwrap()
    }

    fn set(s: &Store, sql: &str, params: impl rusqlite::Params) {
        s.conn.execute(sql, params).unwrap();
    }

    fn add_entry(
        s: &Store,
        t: TopicId,
        kind: &str,
        event: Option<&str>,
        new: Option<&str>,
        at: i64,
    ) -> i64 {
        set(
            s,
            "INSERT INTO entries (uuid, topic_id, kind, author, text, event, new_value, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, 'human', '', ?4, ?5, ?6, ?6, 'human')",
            params![crate::store::new_uuid(), t.0, kind, event, new, at],
        );
        s.conn.last_insert_rowid()
    }

    struct Seeded {
        store: TestStore,
        p: ProjectId,
        e: EpicId,
        t: HashMap<&'static str, TopicId>,
    }

    /// One project with a case for every definition and its edges.
    fn seeded() -> Seeded {
        let mut ts = open();
        let s = &mut ts.store;
        let p = s.create_project("Acta", "", H).unwrap();
        let e = s.create_epic(p, "Terminal", "", H).unwrap();
        let e_done = s.create_epic(p, "Done epic", "", H).unwrap();
        let _e_empty = s.create_epic(p, "Empty epic", "", H).unwrap();
        let mut t = HashMap::new();
        for (name, epic, status) in [
            ("blocked", None, Status::Blocked),
            ("blocked_in_epic", Some(e), Status::Blocked),
            ("overdue", Some(e), Status::Planned),
            ("overdue_done", None, Status::Done),
            ("due_today", None, Status::Draft),
            ("idle_7", Some(e), Status::Active),
            ("busy_6", None, Status::Active),
            ("idle_no_entry", None, Status::Active),
            ("idle_deleted_newer", None, Status::Active),
            ("done_in_window", None, Status::Done),
            ("done_reopened", None, Status::Active),
            ("done_before", None, Status::Done),
            ("done_at_since", Some(e), Status::Done),
            ("decided", None, Status::Planned),
            ("deleted_decision", None, Status::Planned),
            ("created_in_window", None, Status::Draft),
            ("created_before_since", None, Status::Draft),
            ("dropped", None, Status::Dropped),
            ("complete", Some(e_done), Status::Done),
        ] {
            t.insert(name, new_topic(s, p, epic, name, status));
        }
        let old = noon(60);
        // Everything old and quiet first: the store wrote "now".
        set(s, "UPDATE topics SET created_at = ?1", [old]);
        set(s, "UPDATE entries SET created_at = ?1", [old]);
        let since = ZONE.midnight(today().add_days(-6));

        set(
            s,
            "UPDATE topics SET due_on = ?2 WHERE id = ?1",
            params![t["overdue"].0, today().add_days(-3)],
        );
        set(
            s,
            "UPDATE topics SET due_on = ?2 WHERE id = ?1",
            params![t["overdue_done"].0, today().add_days(-3)],
        );
        set(
            s,
            "UPDATE topics SET due_on = ?2 WHERE id = ?1",
            params![t["due_today"].0, today()],
        );
        set(
            s,
            "UPDATE epics SET due_on = ?2 WHERE id = ?1",
            params![e.0, today().add_days(-1)],
        );

        // Idle: exactly 7 local days is idle, 6 days at 23:59 is not.
        add_entry(
            s,
            t["idle_7"],
            "progress",
            None,
            None,
            ZONE.midnight(today().add_days(-7)) + 60_000,
        );
        add_entry(
            s,
            t["busy_6"],
            "progress",
            None,
            None,
            ZONE.midnight(today().add_days(-5)) - 60_000,
        );
        // No entry at all, not even its creation event: its creation counts.
        set(
            s,
            "DELETE FROM entries WHERE topic_id = ?1",
            [t["idle_no_entry"].0],
        );
        set(
            s,
            "UPDATE topics SET created_at = ?2 WHERE id = ?1",
            params![t["idle_no_entry"].0, noon(10)],
        );
        add_entry(s, t["idle_deleted_newer"], "progress", None, None, noon(20));
        let newer = add_entry(s, t["idle_deleted_newer"], "progress", None, None, noon(1));
        set(
            s,
            "UPDATE entries SET deleted_at = 1 WHERE id = ?1",
            [newer],
        );

        // Done in the window; done then reopened in it; done before it;
        // done at the first instant of the window (local midnight on the
        // far side of the daylight-saving change).
        add_entry(
            s,
            t["done_in_window"],
            "system",
            Some("status"),
            Some("done"),
            noon(3),
        );
        add_entry(
            s,
            t["done_reopened"],
            "system",
            Some("status"),
            Some("done"),
            noon(2),
        );
        add_entry(
            s,
            t["done_reopened"],
            "system",
            Some("status"),
            Some("active"),
            noon(1),
        );
        add_entry(
            s,
            t["done_before"],
            "system",
            Some("status"),
            Some("done"),
            since - 1,
        );
        add_entry(
            s,
            t["done_at_since"],
            "system",
            Some("status"),
            Some("done"),
            since,
        );

        // Two decisions in one topic; a deleted one; one before the window.
        add_entry(s, t["decided"], "decision", None, None, noon(0));
        add_entry(s, t["decided"], "decision", None, None, noon(6));
        add_entry(s, t["decided"], "decision", None, None, since - 3_600_000);
        let gone = add_entry(s, t["deleted_decision"], "decision", None, None, noon(2));
        set(s, "UPDATE entries SET deleted_at = 1 WHERE id = ?1", [gone]);

        set(
            s,
            "UPDATE topics SET created_at = ?2 WHERE id = ?1",
            params![t["created_in_window"].0, since],
        );
        set(
            s,
            "UPDATE topics SET created_at = ?2 WHERE id = ?1",
            params![t["created_before_since"].0, since - 1],
        );

        // Inbox: two open notes (12 and 2 days old), one dismissed, one
        // deleted, one filed.
        for (text, days) in [
            ("old", 12),
            ("new", 2),
            ("dismissed", 30),
            ("deleted", 40),
            ("filed", 50),
        ] {
            let n = s.add_note(p, text, H).unwrap();
            set(
                s,
                "UPDATE notes SET created_at = ?2 WHERE id = ?1",
                params![n.0, noon(days)],
            );
            match text {
                "dismissed" => s.dismiss_notes(&[n], H).unwrap(),
                "deleted" => s.delete(Record::Note(n), H).unwrap(),
                "filed" => s.file_notes(&[n], t["decided"], H).unwrap(),
                _ => {}
            }
        }

        // Memories at 89 and 90 days, an old document, a deleted memory,
        // one on a topic at 120 days.
        for (title, kind, owner, days) in [
            ("m89", KbKind::Memory, Owner::Project(p), 89),
            ("m90", KbKind::Memory, Owner::Project(p), 90),
            ("doc", KbKind::Document, Owner::Project(p), 200),
            ("deleted", KbKind::Memory, Owner::Project(p), 200),
            ("on_topic", KbKind::Memory, Owner::Topic(t["blocked"]), 120),
        ] {
            let k = s
                .kb_create(
                    NewKb {
                        owner,
                        kind,
                        title: title.into(),
                        summary: "Kept by Alexandre".into(),
                        body: "x".into(),
                        from_file: false,
                    },
                    H,
                )
                .unwrap();
            // Midnight, the first instant of that local day.
            set(
                s,
                "UPDATE kb_entries SET updated_at = ?2 WHERE id = ?1",
                params![k.0, ZONE.midnight(today().add_days(-days))],
            );
            if title == "deleted" {
                s.delete(Record::Kb(k), H).unwrap();
            }
        }
        Seeded { store: ts, p, e, t }
    }

    fn topics_of(items: &[OverviewItem]) -> Vec<TopicId> {
        items
            .iter()
            .filter_map(|i| match i.record {
                Record::Topic(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn every_definition_and_its_edges() {
        let Seeded { store, p, e, t } = seeded();
        let s = &store.store;
        let list = |f| {
            s.overview_list(Scope::Project(p), f, today(), ZONE)
                .unwrap()
        };

        assert_eq!(
            topics_of(&list(OverviewFilter::Blocked)),
            [t["blocked"], t["blocked_in_epic"]]
        );
        let overdue = list(OverviewFilter::Overdue);
        assert_eq!(topics_of(&overdue), [t["overdue"]]);
        assert_eq!(overdue[0].days, Some(3));

        let idle = list(OverviewFilter::Idle);
        assert_eq!(
            topics_of(&idle),
            [t["idle_deleted_newer"], t["idle_no_entry"], t["idle_7"]]
        );
        assert_eq!(
            idle.iter().map(|i| i.days.unwrap()).collect::<Vec<_>>(),
            [20, 10, 7]
        );

        let inbox = list(OverviewFilter::Inbox);
        assert_eq!(
            inbox.iter().map(|i| i.days.unwrap()).collect::<Vec<_>>(),
            [12, 2]
        );

        assert_eq!(
            topics_of(&list(OverviewFilter::Done)),
            [t["done_in_window"], t["done_at_since"]]
        );
        assert_eq!(topics_of(&list(OverviewFilter::Decided)), [t["decided"]]);
        assert_eq!(
            topics_of(&list(OverviewFilter::Created)),
            [t["created_in_window"]]
        );
        let stale = list(OverviewFilter::StaleMemories);
        assert_eq!(
            stale.iter().map(|i| i.days.unwrap()).collect::<Vec<_>>(),
            [120, 90]
        );

        let o = s.overview(Scope::Project(p), today(), ZONE).unwrap();
        assert_eq!((o.blocked, o.overdue, o.idle, o.inbox), (2, 1, 3, 2));
        assert_eq!(o.inbox_oldest_days, Some(12));
        assert!(o.inbox_needs_attention());
        assert_eq!(o.attention_lines(), 4);
        assert_eq!((o.done, o.decisions, o.decided, o.created), (2, 2, 1, 1));
        assert_eq!(o.last_activity, None);
        assert_eq!(o.stale_memories, 2);
        // Epics: "No epic" first, then Terminal; the complete epic is
        // counted, the empty one is absent.
        assert_eq!(
            o.epics.iter().map(|r| r.epic).collect::<Vec<_>>(),
            [None, Some(e)]
        );
        assert_eq!(o.epics[1].progress, Progress { done: 1, total: 4 });
        assert_eq!(o.epics[1].due_on, Some(today().add_days(-1)));
        assert_eq!(o.complete_epics, 1);

        // The epic: its topics only; inbox and memories stay project-wide.
        let oe = s.overview(Scope::Epic(e), today(), ZONE).unwrap();
        assert_eq!((oe.blocked, oe.overdue, oe.idle), (1, 1, 1));
        assert_eq!((oe.done, oe.decisions, oe.created), (1, 0, 0));
        assert!(oe.epics.is_empty());
    }

    #[test]
    fn each_count_equals_the_length_of_its_list() {
        let Seeded { store, p, e, .. } = seeded();
        let s = &store.store;
        for scope in [Scope::Project(p), Scope::Epic(e)] {
            let o = s.overview(scope, today(), ZONE).unwrap();
            for f in OverviewFilter::ALL {
                let n = s.overview_list(scope, f, today(), ZONE).unwrap().len();
                let count = match f {
                    OverviewFilter::Blocked => o.blocked,
                    OverviewFilter::Overdue => o.overdue,
                    OverviewFilter::Idle => o.idle,
                    OverviewFilter::Inbox => o.inbox,
                    OverviewFilter::Done => o.done,
                    OverviewFilter::Decided => o.decided,
                    OverviewFilter::Created => o.created,
                    OverviewFilter::StaleMemories => o.stale_memories,
                };
                assert_eq!(count, n, "{scope:?} {f:?}");
            }
        }
    }

    #[test]
    fn a_quiet_project_reports_its_last_activity() {
        let mut ts = open();
        let s = &mut ts.store;
        let p = s.create_project("Acta", "", H).unwrap();
        let t = new_topic(s, p, None, "Old", Status::Planned);
        set(s, "UPDATE topics SET created_at = ?1", [noon(30)]);
        set(s, "UPDATE entries SET created_at = ?1", [noon(30)]);
        add_entry(s, t, "progress", None, None, noon(12));
        let o = s.overview(Scope::Project(p), today(), ZONE).unwrap();
        assert_eq!(o.attention_lines(), 0);
        assert_eq!(o.last_activity, Some(noon(12)));
        assert!(!o.inbox_needs_attention());
    }

    #[test]
    fn every_query_names_its_index() {
        let Seeded { store, .. } = seeded();
        let s = &store.store;
        for (sql, index) in QUERIES {
            let n = sql.matches('?').count();
            let args: Vec<i64> = vec![1; n];
            let mut stmt = s
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap();
            let plan: Vec<String> = stmt
                .query_map(rusqlite::params_from_iter(args), |r| r.get::<_, String>(3))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            assert!(
                plan.iter()
                    .any(|line| line.contains(&format!("INDEX {index}"))),
                "{index} not used by {sql}: {plan:?}"
            );
        }
    }

    /// The budget of request 17: a project of 10,000 topics, 200,000
    /// entries, 2,000 notes and 1,000 memories in at most 5 ms. Timed
    /// locally (`cargo test -p acta-core --release -- --ignored
    /// overview_timing --nocapture`), not asserted: a timing would be
    /// flaky in CI. `ACTA_TIMING_CACHE_KIB` overrides the store's page cache
    /// for the run.
    #[test]
    #[ignore]
    fn overview_timing() {
        let ts = open();
        let s = &ts.store;
        let now = noon(0);
        let base = noon(400);
        s.conn.execute_batch("BEGIN").unwrap();
        set(
            s,
            "INSERT INTO projects (uuid, title, position, created_at, updated_at, updated_by)
                VALUES ('p', 'Acta', 0, 0, 0, 'human')",
            [],
        );
        for e in 0..100i64 {
            set(s, "INSERT INTO epics (uuid, project_id, title, position, created_at, updated_at, updated_by)
                    VALUES (?1, 1, 'Epic', ?2, 0, 0, 'human')", params![format!("e{e}"), e]);
        }
        let statuses = ["draft", "planned", "active", "blocked", "done", "dropped"];
        {
            let mut topic = s.conn.prepare(
                "INSERT INTO topics (uuid, project_id, epic_id, title, status, position, due_on, created_at, updated_at, updated_by)
                 VALUES (?1, 1, ?2, 'Topic', ?3, ?4, ?5, ?6, ?6, 'human')").unwrap();
            let mut entry = s.conn.prepare(
                "INSERT INTO entries (uuid, topic_id, kind, author, text, event, new_value, created_at, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, 'claude', 'Logged by Alexandre''s session', ?4, ?5, ?6, ?6, 'claude')").unwrap();
            let mut n = 0i64;
            for i in 0..10_000i64 {
                let epic = (i % 5 != 0).then_some(1 + i % 100);
                let status = statuses[(i % 6) as usize];
                let due = (i % 3 == 0).then(|| today().add_days(i % 40 - 20));
                topic
                    .execute(params![
                        format!("t{i}"),
                        epic,
                        status,
                        i,
                        due,
                        base + i * 1000
                    ])
                    .unwrap();
                let id = s.conn.last_insert_rowid();
                for j in 0..20i64 {
                    n += 1;
                    let at = base + n * 172_800;
                    let (kind, event, new) = match j % 10 {
                        0 => ("system", Some("status"), Some(status)),
                        1 => ("decision", None, None),
                        _ => ("progress", None, None),
                    };
                    entry
                        .execute(params![format!("x{n}"), id, kind, event, new, at])
                        .unwrap();
                }
            }
            let mut note = s.conn.prepare(
                "INSERT INTO notes (uuid, project_id, text, author, filed_at, created_at, updated_at, updated_by)
                 VALUES (?1, 1, 'Note', 'human', ?2, ?3, ?3, 'human')").unwrap();
            for i in 0..2_000i64 {
                let filed = (i % 10 != 0).then_some(now);
                note.execute(params![format!("n{i}"), filed, base + i * 60_000])
                    .unwrap();
            }
            let mut kb = s.conn.prepare(
                "INSERT INTO kb_entries (uuid, project_id, kind, title, summary, body, rev, created_by, created_at, updated_at, updated_by)
                 VALUES (?1, 1, 'memory', 'Memory', '', 'x', 1, 'claude', ?2, ?2, 'claude')").unwrap();
            for i in 0..1_000i64 {
                kb.execute(params![format!("k{i}"), base + i * 3_600_000])
                    .unwrap();
            }
        }
        s.conn.execute_batch("COMMIT; ANALYZE;").unwrap();
        if let Ok(kib) = std::env::var("ACTA_TIMING_CACHE_KIB") {
            s.conn
                .execute_batch(&format!("PRAGMA cache_size = -{kib};"))
                .unwrap();
        }
        let scope = Scope::Project(ProjectId(1));
        // Warm the statement cache and the page cache, as the store
        // thread is after the first page open.
        let o = s.overview(scope, today(), ZONE).unwrap();
        let runs = 20;
        let start = std::time::Instant::now();
        for _ in 0..runs {
            std::hint::black_box(s.overview(scope, today(), ZONE).unwrap());
        }
        let per = start.elapsed() / runs;
        // A calm window: the last-activity query runs too.
        let calm = Date::from_ymd(2027, 6, 1).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..runs {
            std::hint::black_box(s.overview(scope, calm, ZONE).unwrap());
        }
        let calm_per = start.elapsed() / runs;
        for f in OverviewFilter::ALL {
            let start = std::time::Instant::now();
            for _ in 0..runs {
                std::hint::black_box(s.overview_list(scope, f, today(), ZONE).unwrap());
            }
            println!("{f:?}: {:?}", start.elapsed() / runs);
        }
        println!(
            "overview of 10,000 topics / 200,000 entries / 2,000 notes / 1,000 memories: {per:?} per call ({calm_per:?} with no activity in the window); blocked {} overdue {} idle {} inbox {} done {} decisions {} created {} stale {}",
            o.blocked, o.overdue, o.idle, o.inbox, o.done, o.decisions, o.created, o.stale_memories
        );
    }
}
