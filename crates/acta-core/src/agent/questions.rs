//! Questions and people in agent output (docs/design/data.md requests 18
//! and 19, ADR 0005 changelog): the line forms of `find` and `brief`, and
//! the two `find` paths that are not topic searches, `kind="question"` and
//! `person=...`. Every question, answer and person is stored text: the
//! callers put these lines inside the data fence.

use super::render;
use super::tools::{Agent, FindHit, FindPage, err};
use crate::date::{Date, DueState, due_state};
use crate::model::{
    EpicSummary, Note, Owner, PageReq, ProjectId, Question, QuestionStatus, Status, TopicSummary,
};
use crate::person::person_key;
use crate::store::QuestionFilter;

/// A person in a list line: one line, at most this many characters.
pub const PERSON_IN_LINE: usize = 20;
/// A question in a `find` line, cut at a word.
pub const TEXT_IN_FIND: usize = 140;
/// An answer in a `find` line.
pub const ANSWER_IN_FIND: usize = 100;
/// A question line of a `brief`.
pub const BRIEF_LINE: usize = 120;

/// A person value for a line: flattened and clipped.
pub fn person_short(name: &str) -> String {
    render::clip(&render::one_line(name), PERSON_IN_LINE)
}

/// At most `max` characters, cut at a word boundary with `...`.
pub fn cut_words(text: &str, max: usize) -> String {
    let flat = render::one_line(text);
    if render::chars(&flat) <= max {
        return flat;
    }
    let keep: String = flat.chars().take(max.saturating_sub(3)).collect();
    let cut = match keep.rfind(' ') {
        Some(i) if i > keep.len() / 2 => &keep[..i],
        _ => &keep[..],
    };
    format!("{}...", cut.trim_end())
}

/// The answer date of a question in a line: `overdue 3d`, `by today`,
/// `by tomorrow`, `by 12-04`; empty for a closed question or no date.
pub fn by_segment(day: Option<Date>, today: Date, open: bool) -> String {
    let Some(d) = day else {
        return String::new();
    };
    match due_state(day, today, open) {
        DueState::None | DueState::Closed => String::new(),
        DueState::Overdue(n) => format!("overdue {n}d"),
        DueState::Today => "by today".into(),
        DueState::Soon(1) => "by tomorrow".into(),
        DueState::Soon(_) | DueState::Later(_) => format!("by {}", super::due::day(d, today)),
    }
}

/// What an ack says about an answer date: `by 2026-12-04 (in 13d)`.
pub fn by_ack(day: Option<Date>, today: Date) -> String {
    match day {
        None => "answer date cleared".into(),
        Some(_) => super::due::ack(day, today).replacen("due ", "by ", 1),
    }
}

/// Where a record stands with its person (request 19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportState {
    /// Open work, not reported.
    Open,
    /// A Done topic or a complete epic, not reported yet.
    NotYet,
    Reported,
    /// A Dropped topic: nothing to report unless marked.
    Dropped,
}

/// `· report Marie`, `· report Marie (not yet)`, `· reported Marie`;
/// empty without a person or for a Dropped topic.
pub fn report_segment(to: Option<&str>, state: ReportState) -> String {
    let Some(to) = to else {
        return String::new();
    };
    let name = person_short(to);
    match state {
        ReportState::Open => format!(" · report {name}"),
        ReportState::NotYet => format!(" · report {name} (not yet)"),
        ReportState::Reported => format!(" · reported {name}"),
        ReportState::Dropped => String::new(),
    }
}

pub fn topic_report_state(status: Status, reported: bool) -> ReportState {
    match (reported, status) {
        (true, _) => ReportState::Reported,
        (false, Status::Dropped) => ReportState::Dropped,
        (false, Status::Done) => ReportState::NotYet,
        (false, _) => ReportState::Open,
    }
}

pub fn epic_report_state(e: &EpicSummary) -> ReportState {
    match (e.reported, e.progress.is_open()) {
        (true, _) => ReportState::Reported,
        (false, false) => ReportState::NotYet,
        (false, true) => ReportState::Open,
    }
}

fn status_word(s: QuestionStatus) -> &'static str {
    s.as_str()
}

/// The `find` line of a question:
/// `Q-12 open T-42 · Marie · overdue 3d · Can we keep...`;
/// `Q-9 answered 11-19 T-42 · Legal team · Can the export leave? -> Yes.`
pub fn find_line(q: &Question, today: Date) -> String {
    let mut head = format!("{} {}", q.id, status_word(q.status));
    if let Some(at) = q.resolved_at.filter(|_| !q.is_open()) {
        head.push(' ');
        head.push_str(&render::short_date(at));
    }
    head.push(' ');
    head.push_str(&q.owner.to_string());
    let mut parts = vec![head];
    if let Some(p) = &q.asked_to {
        parts.push(person_short(p));
    }
    let by = by_segment(q.expected_on, today, q.is_open());
    if !by.is_empty() {
        parts.push(by);
    }
    let mut text = cut_words(&q.text, TEXT_IN_FIND);
    if q.status == QuestionStatus::Answered
        && let Some(a) = &q.answer
    {
        text.push_str(" -> ");
        text.push_str(&cut_words(a, ANSWER_IN_FIND));
    }
    parts.push(text);
    parts.join(" · ")
}

/// The `brief` line of an open question, without its state and owner:
/// `Q-12 Marie · overdue 3d · Can we keep...`, at most 120 chars.
pub fn brief_line(q: &Question, today: Date) -> String {
    let mut head = q.id.to_string();
    if let Some(p) = &q.asked_to {
        head.push(' ');
        head.push_str(&person_short(p));
    }
    let mut parts = vec![head];
    let by = by_segment(q.expected_on, today, q.is_open());
    if !by.is_empty() {
        parts.push(by);
    }
    let used: usize = parts.iter().map(|p| render::chars(p) + 3).sum();
    parts.push(cut_words(&q.text, BRIEF_LINE.saturating_sub(used)));
    render::clip(&parts.join(" · "), BRIEF_LINE)
}

/// `Questions 3 open, 1 overdue:` (no overdue part at 0); `None` without
/// open questions.
pub fn count_line(open: &[Question], today: Date) -> Option<String> {
    if open.is_empty() {
        return None;
    }
    let late = open
        .iter()
        .filter(|q| matches!(due_state(q.expected_on, today, true), DueState::Overdue(_)))
        .count();
    Some(if late > 0 {
        format!("Questions {} open, {late} overdue:", open.len())
    } else {
        format!("Questions {} open:", open.len())
    })
}

/// The open questions of an owner a brief shows first: overdue first,
/// then by answer date, then stored order.
pub fn brief_order(mut open: Vec<Question>) -> Vec<Question> {
    // Stable: the stored order breaks ties.
    open.sort_by_key(|q| (q.expected_on.is_none(), q.expected_on));
    open
}

/// One hit of `find(kind="question")`.
pub(super) fn question_hit(q: &Question, today: Date) -> FindHit {
    let open = q.is_open();
    FindHit {
        id: q.id.to_string(),
        kind: "question",
        status: Some(status_word(q.status)),
        title: None,
        text: Some(q.text.clone()),
        due: Some(q.expected_on),
        overdue: Some(super::due::is_overdue(q.expected_on, today, open)),
        line: find_line(q, today),
        extra: vec![
            ("owner", q.owner.to_string().into()),
            ("asked_to", q.asked_to.clone().into()),
            ("answer", q.answer.clone().into()),
            ("resolved_at", q.resolved_at.into()),
        ],
    }
}

impl Agent<'_> {
    /// `find(kind="question")`: the project's questions, open by default.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn find_questions(
        &self,
        project: ProjectId,
        status: Option<&str>,
        query: Option<&str>,
        epic: Option<crate::model::EpicId>,
        due_cutoff: Option<Date>,
        person: Option<String>,
        page: PageReq,
    ) -> Result<FindPage, String> {
        let today = self.session.today();
        let statuses = match status {
            None | Some("open") => vec![QuestionStatus::Open],
            Some("answered") => vec![QuestionStatus::Answered],
            Some("dropped") => vec![QuestionStatus::Dropped],
            Some("closed") => vec![QuestionStatus::Answered, QuestionStatus::Dropped],
            Some("all") => Vec::new(),
            Some(_) => {
                return Err("a question's status is open, answered, dropped, closed or all".into());
            }
        };
        let filter = QuestionFilter {
            statuses,
            epic,
            expected_before: due_cutoff,
            person_key: person,
            ..QuestionFilter::default()
        };
        let mut all = self.store.find_questions(project, &filter).map_err(err)?;
        if let Some(q) = query {
            // Every word, case and accents ignored, in the question or
            // its answer.
            let words: Vec<String> = person_key(q).split(' ').map(str::to_owned).collect();
            all.retain(|x| {
                let hay = person_key(&format!("{} {}", x.text, x.answer.as_deref().unwrap_or("")));
                words.iter().all(|w| hay.contains(w.as_str()))
            });
        }
        let total = all.len() as i64;
        let hits = all
            .iter()
            .skip(page.offset as usize)
            .take(page.limit as usize)
            .map(|q| question_hit(q, today))
            .collect();
        Ok(FindPage {
            hits,
            offset: page.offset,
            total,
            note: None,
        })
    }

    /// `find(person=...)`: everything open that names the person, by key:
    /// open questions asked to them, then topics and epics to report to
    /// them (done and not reported first, then open work), then open
    /// notes. `kind` narrows. With no exact key, one line naming up to 5
    /// people whose key contains the text (`FindPage::note`).
    pub(super) fn find_person(
        &self,
        project: ProjectId,
        name: &str,
        kind: Option<&str>,
        page: PageReq,
    ) -> Result<FindPage, String> {
        let key = person_key(name);
        if key.is_empty() {
            return Err("person must not be empty".into());
        }
        let today = self.session.today();
        let people = self.store.people(project).map_err(err)?;
        if !people.iter().any(|p| p.key == key) {
            let near: Vec<String> = people
                .iter()
                .filter(|p| p.key.contains(&key))
                .take(5)
                .map(|p| person_short(&p.display))
                .collect();
            let asked = render::clip(&render::one_line(name), 40);
            let line = if near.is_empty() {
                format!("no one named \"{asked}\"")
            } else {
                format!("no one named \"{asked}\" · people: {}", near.join(", "))
            };
            return Ok(FindPage {
                hits: Vec::new(),
                offset: 0,
                total: 0,
                note: Some(line),
            });
        }
        let wants = |k: &str| kind.is_none_or(|x| x == k);
        let mut hits: Vec<FindHit> = Vec::new();
        if wants("question") {
            let filter = QuestionFilter {
                statuses: vec![QuestionStatus::Open],
                person_key: Some(key.clone()),
                ..QuestionFilter::default()
            };
            for q in self.store.find_questions(project, &filter).map_err(err)? {
                hits.push(question_hit(&q, today));
            }
        }
        let targets = self.store.report_targets(project, &key).map_err(err)?;
        // Work done and not reported first: telling them is what is left.
        let mut done: Vec<FindHit> = Vec::new();
        let mut open: Vec<FindHit> = Vec::new();
        if wants("topic") {
            for t in &targets.topics {
                let hit = super::tools::topic_hit(t, today, false);
                if t.status == Status::Done {
                    done.push(hit);
                } else {
                    open.push(hit);
                }
            }
        }
        if wants("epic") {
            for e in &targets.epics {
                let hit = super::tools::epic_hit(e, today, false);
                if e.progress.is_open() {
                    open.push(hit);
                } else {
                    done.push(hit);
                }
            }
        }
        hits.extend(done);
        hits.extend(open);
        if wants("note") {
            hits.extend(targets.notes.iter().map(note_hit));
        }
        let total = hits.len() as i64;
        let hits = hits
            .into_iter()
            .skip(page.offset as usize)
            .take(page.limit as usize)
            .collect();
        Ok(FindPage {
            hits,
            offset: page.offset,
            total,
            note: None,
        })
    }

    /// The open questions of an owner, in the order a brief shows them.
    pub(super) fn open_questions(&self, owner: Owner) -> Result<Vec<Question>, String> {
        let all = self.store.questions_of(owner).map_err(err)?;
        Ok(brief_order(
            all.into_iter().filter(|q| q.is_open()).collect(),
        ))
    }
}

/// A note line of `find`: `N-3 note  text · report Marie`.
pub(super) fn note_hit(n: &Note) -> FindHit {
    let seg = report_segment(n.report_to.as_deref(), ReportState::Open);
    FindHit {
        id: n.id.to_string(),
        kind: "note",
        text: Some(n.text.clone()),
        line: format!(
            "{} note  {}{seg}",
            n.id,
            render::clip(&render::one_line(&n.text), 200)
        ),
        extra: vec![("report_to", n.report_to.clone().into())],
        ..FindHit::default()
    }
}

/// The person segment of a topic line.
pub fn topic_segment(t: &TopicSummary) -> String {
    report_segment(
        t.report_to.as_deref(),
        topic_report_state(t.status, t.reported),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_cut_at_a_boundary() {
        assert_eq!(cut_words("short", 10), "short");
        let cut = cut_words("Can we keep the v1 export format until March", 20);
        assert!(render::chars(&cut) <= 20, "{cut}");
        assert!(cut.ends_with("..."), "{cut}");
        assert!(!cut.contains("expo..."), "{cut}");
        // One long word is cut inside it.
        let cut = cut_words(&"x".repeat(50), 20);
        assert_eq!(render::chars(&cut), 20);
    }

    #[test]
    fn report_segments() {
        assert_eq!(report_segment(None, ReportState::Open), "");
        assert_eq!(
            report_segment(Some("Marie"), ReportState::Open),
            " · report Marie"
        );
        assert_eq!(
            report_segment(Some("Marie"), ReportState::NotYet),
            " · report Marie (not yet)"
        );
        assert_eq!(
            report_segment(Some("Marie"), ReportState::Reported),
            " · reported Marie"
        );
        assert_eq!(report_segment(Some("Marie"), ReportState::Dropped), "");
        let long = "Alexandre ".repeat(8);
        assert!(render::chars(&report_segment(Some(&long), ReportState::NotYet)) <= 40);
    }

    #[test]
    fn answer_dates_in_lines() {
        let d = |s: &str| Date::parse_iso(s).unwrap();
        let today = d("2026-11-21");
        assert_eq!(by_segment(Some(d("2026-11-18")), today, true), "overdue 3d");
        assert_eq!(by_segment(Some(d("2026-11-21")), today, true), "by today");
        assert_eq!(
            by_segment(Some(d("2026-11-22")), today, true),
            "by tomorrow"
        );
        assert_eq!(by_segment(Some(d("2026-12-04")), today, true), "by 12-04");
        assert_eq!(by_segment(Some(d("2026-11-18")), today, false), "");
        assert_eq!(
            by_ack(Some(d("2026-12-04")), today),
            "by 2026-12-04 (in 13d)"
        );
        assert_eq!(by_ack(None, today), "answer date cleared");
    }
}
