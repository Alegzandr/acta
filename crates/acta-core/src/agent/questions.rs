//! Questions and contacts in agent output (docs/design/data.md requests
//! 18, 19 and 22, ADR 0005 changelog): the line forms of `find` and
//! `brief`, the `find` paths that are not topic searches
//! (`kind="question"`, `kind="contact"`, `person=...`), and how a
//! `report_to` or `asked_to` argument resolves to a contact. Every
//! question, answer, name, job title, company and note is stored text: the
//! callers put these lines inside the data fence, and acks and refusals
//! name contacts by id only (near matches go in a fence).

use super::render;
use super::tools::{Agent, FindHit, FindPage, err};
use crate::date::{Date, DueState, due_state};
use crate::model::{
    Contact, ContactId, ContactLevel, ContactRef, EpicSummary, Note, Owner, PageReq, Place,
    ProjectId, Question, QuestionStatus, Status, TopicSummary, visible,
};
use crate::person::person_key;
use crate::store::{ContactSummary, QuestionFilter};

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
/// empty without a contact or for a Dropped topic.
pub fn report_segment(to: Option<&ContactRef>, state: ReportState) -> String {
    let Some(to) = to else {
        return String::new();
    };
    let name = person_short(&to.name);
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
        parts.push(person_short(&p.name));
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
        head.push_str(&person_short(&p.name));
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
            (
                "asked_to",
                q.asked_to.as_ref().map(|c| c.name.clone()).into(),
            ),
            (
                "asked_contact",
                q.asked_to.as_ref().map(|c| c.id.to_string()).into(),
            ),
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
        person: Option<ContactId>,
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
            contact: person,
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
            head: Vec::new(),
        })
    }

    /// How a `report_to` or `asked_to` argument names a contact seen from
    /// `place`: `""` clears (`None`); `C-n` must be a live contact visible
    /// there; a name must resolve on its key to exactly one visible
    /// contact. A name never creates one. `what` names the target record.
    pub(super) fn person_ref(
        &self,
        text: &str,
        place: Place,
        what: &str,
    ) -> Result<Option<ContactId>, String> {
        let t = text.trim();
        if t.is_empty() {
            return Ok(None);
        }
        if let Some(id) = ContactId::parse(t).filter(|_| t.to_ascii_uppercase().starts_with("C-")) {
            let c = self.store.contact(id).map_err(err)?;
            if !visible(c.level, place) {
                return Err(format!(
                    "{id} is not visible from {what} (it belongs to {})",
                    crate::store::level_name(c.level)
                ));
            }
            return Ok(Some(id));
        }
        let shown = render::clip(&render::one_line(t), 40);
        if render::chars(t) > crate::person::MAX_CHARS {
            return Err(render::over_limit(
                "a person",
                crate::person::MAX_CHARS,
                render::chars(t),
            ));
        }
        let key = person_key(t);
        let all = self.store.contacts_visible(place).map_err(err)?;
        let exact: Vec<&ContactSummary> = all
            .iter()
            .filter(|c| person_key(&c.contact.name) == key)
            .collect();
        match exact.len() {
            1 => Ok(Some(exact[0].contact.id)),
            0 => {
                let near = near_matches(&all, &key);
                let advice = "create(kind=\"contact\") only if the user named someone new";
                if near.is_empty() {
                    Err(format!("no contact \"{shown}\" here; {advice}"))
                } else {
                    Err(format!(
                        "no contact \"{shown}\" here; {advice}. Near:\n{}",
                        super::render::Fence::new().wrap(&near.join(" · "))
                    ))
                }
            }
            n => {
                let ids: Vec<String> = exact
                    .iter()
                    .map(|c| format!("{} ({})", c.contact.id, level_short(c.contact.level)))
                    .collect();
                Err(format!(
                    "{n} contacts \"{shown}\" here: {}; pass the id",
                    ids.join(", ")
                ))
            }
        }
    }

    /// Where a find reads contacts from: the focus, else the project.
    fn find_place(&self, project: ProjectId) -> Place {
        let record = match self.session.focus {
            Some(Owner::Topic(t)) => crate::model::Record::Topic(t),
            Some(Owner::Epic(e)) => crate::model::Record::Epic(e),
            _ => crate::model::Record::Project(project),
        };
        self.store.place(record).unwrap_or(Place {
            project,
            epic: None,
        })
    }

    /// `find(kind="contact")`: the contacts in scope, nearest level first;
    /// `query` keeps those whose name, job title or company has every word.
    pub(super) fn find_contacts(
        &self,
        project: ProjectId,
        query: Option<&str>,
        page: PageReq,
    ) -> Result<FindPage, String> {
        let mut all = self
            .store
            .contacts_visible(self.find_place(project))
            .map_err(err)?;
        if let Some(q) = query {
            let words: Vec<String> = person_key(q).split(' ').map(str::to_owned).collect();
            all.retain(|c| {
                let hay = person_key(&format!(
                    "{} {} {}",
                    c.contact.name,
                    c.contact.job_title.as_deref().unwrap_or(""),
                    c.contact.company.as_deref().unwrap_or("")
                ));
                words.iter().all(|w| hay.contains(w.as_str()))
            });
        }
        let total = all.len() as i64;
        let hits = all
            .iter()
            .skip(page.offset as usize)
            .take(page.limit as usize)
            .map(|c| contact_hit(&c.contact, Some(c.open)))
            .collect();
        Ok(FindPage {
            hits,
            offset: page.offset,
            total,
            note: None,
            head: Vec::new(),
        })
    }

    /// `find(person=...)`: one contact (`C-n`, or a name resolved as
    /// `report_to` resolves it) and everything open that names them in the
    /// project: open questions asked to them, then topics and epics to
    /// report to them (done and not reported first, then open work), then
    /// open notes. `kind` narrows. The header line gives the details, then
    /// the address note when set. An unknown name answers with its near
    /// matches, fenced (`FindPage::note`).
    pub(super) fn find_person(
        &self,
        project: ProjectId,
        name: &str,
        kind: Option<&str>,
        page: PageReq,
    ) -> Result<FindPage, String> {
        if name.trim().is_empty() {
            return Err("person must not be empty".into());
        }
        let place = self.find_place(project);
        let id = match self.person_ref(name, place, "here") {
            Ok(Some(id)) => id,
            Ok(None) => return Err("person must not be empty".into()),
            Err(e) if e.starts_with("no contact") => {
                let all = self.store.contacts_visible(place).map_err(err)?;
                let near = near_matches(&all, &person_key(name));
                let asked = render::clip(&render::one_line(name), 40);
                let line = if near.is_empty() {
                    format!("no contact \"{asked}\" here")
                } else {
                    format!("no contact \"{asked}\" here · near: {}", near.join(", "))
                };
                return Ok(FindPage {
                    hits: Vec::new(),
                    offset: 0,
                    total: 0,
                    note: Some(line),
                    head: Vec::new(),
                });
            }
            Err(e) => return Err(e),
        };
        let today = self.session.today();
        let c = self.store.contact(id).map_err(err)?;
        let mut head = vec![contact_line(&c, None)];
        if let Some(a) = c.address_note.as_deref().filter(|a| !a.trim().is_empty()) {
            head.push(format!("address: {}", cut_words(a, 300)));
        }
        let wants = |k: &str| kind.is_none_or(|x| x == k);
        let mut hits: Vec<FindHit> = Vec::new();
        if wants("question") {
            let filter = QuestionFilter {
                statuses: vec![QuestionStatus::Open],
                contact: Some(id),
                ..QuestionFilter::default()
            };
            for q in self.store.find_questions(project, &filter).map_err(err)? {
                hits.push(question_hit(&q, today));
            }
        }
        let targets = self.store.report_targets(project, id).map_err(err)?;
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
            head,
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

/// Up to 5 contacts whose key contains `key` or shares a word with it:
/// `C-3 Marie`.
fn near_matches(all: &[ContactSummary], key: &str) -> Vec<String> {
    let words: Vec<&str> = key.split(' ').filter(|w| w.len() >= 2).collect();
    all.iter()
        .filter(|c| {
            let k = person_key(&c.contact.name);
            k.contains(key) || k.split(' ').any(|w| words.contains(&w))
        })
        .take(5)
        .map(|c| format!("{} {}", c.contact.id, person_short(&c.contact.name)))
        .collect()
}

/// `E-3`, `P-1`, `all`.
pub fn level_short(level: ContactLevel) -> String {
    level.to_string()
}

/// `Head of legal, Acme`, cut to 40; empty without details.
pub fn details(c: &Contact) -> String {
    let parts: Vec<&str> = [c.job_title.as_deref(), c.company.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    render::clip(&render::one_line(&parts.join(", ")), 40)
}

/// `C-3 Marie · Head of legal, Acme · E-3 · 2 open`.
pub fn contact_line(c: &Contact, open: Option<i64>) -> String {
    let mut parts = vec![format!(
        "{} {}",
        c.id,
        render::clip(&render::one_line(&c.name), 80)
    )];
    let d = details(c);
    if !d.is_empty() {
        parts.push(d);
    }
    parts.push(level_short(c.level));
    if let Some(n) = open.filter(|n| *n > 0) {
        parts.push(format!("{n} open"));
    }
    parts.join(" · ")
}

fn contact_hit(c: &Contact, open: Option<i64>) -> FindHit {
    FindHit {
        id: c.id.to_string(),
        kind: "contact",
        title: Some(c.name.clone()),
        line: contact_line(c, open),
        extra: vec![
            ("job_title", c.job_title.clone().into()),
            ("company", c.company.clone().into()),
            ("level", level_short(c.level).into()),
        ],
        ..FindHit::default()
    }
}

/// A brief header's segment: ` · report to Marie (Head of legal, Acme)`,
/// ` · reported to Marie`; the name and the details pair cut to 40.
pub fn header_segment(to: &ContactRef, details: &str, reported: bool) -> String {
    let word = if reported { "reported to" } else { "report to" };
    let mut s = person_short(&to.name);
    if !details.is_empty() {
        s.push_str(&format!(" ({details})"));
    }
    format!(" · {word} {}", render::clip(&s, 40 + PERSON_IN_LINE))
}

/// A note line of `find`: `N-3 note  text · report Marie`.
pub(super) fn note_hit(n: &Note) -> FindHit {
    let seg = report_segment(n.report_to.as_ref(), ReportState::Open);
    FindHit {
        id: n.id.to_string(),
        kind: "note",
        text: Some(n.text.clone()),
        line: format!(
            "{} note  {}{seg}",
            n.id,
            render::clip(&render::one_line(&n.text), 200)
        ),
        extra: vec![(
            "report_to",
            n.report_to.as_ref().map(|c| c.name.clone()).into(),
        )],
        ..FindHit::default()
    }
}

/// The person segment of a topic line.
pub fn topic_segment(t: &TopicSummary) -> String {
    report_segment(
        t.report_to.as_ref(),
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
        let c = |name: &str| ContactRef {
            id: ContactId(3),
            name: name.into(),
            in_trash: false,
        };
        let marie = c("Marie");
        assert_eq!(report_segment(None, ReportState::Open), "");
        assert_eq!(
            report_segment(Some(&marie), ReportState::Open),
            " · report Marie"
        );
        assert_eq!(
            report_segment(Some(&marie), ReportState::NotYet),
            " · report Marie (not yet)"
        );
        assert_eq!(
            report_segment(Some(&marie), ReportState::Reported),
            " · reported Marie"
        );
        assert_eq!(report_segment(Some(&marie), ReportState::Dropped), "");
        let long = c(&"Alexandre ".repeat(8));
        assert!(render::chars(&report_segment(Some(&long), ReportState::NotYet)) <= 40);
        assert_eq!(
            header_segment(&marie, "Head of legal, Acme", false),
            " · report to Marie (Head of legal, Acme)"
        );
        // A brief header grows by at most 43 chars plus the name.
        let d = "x".repeat(80);
        assert!(render::chars(&header_segment(&long, &render::clip(&d, 40), true)) <= 75);
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
