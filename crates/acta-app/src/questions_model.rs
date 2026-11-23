//! What the Questions section, the Questions page and the topic table's
//! two columns show (docs/design/components-questions.md 5), computed with
//! no GPUI: which rows a section draws, its roll-up, the page's groups and
//! filters, and the counts.

use std::collections::HashMap;

use acta_core::date::{Date, DueState, due_state};
use acta_core::model::{EpicId, Owner, ProjectTree, Question, TopicId, TopicSummary};

/// A section draws the first 6 open questions, then "Show all 14".
pub const SECTION_OPEN: usize = 6;
/// A question is at most 300 characters; the counter shows over 240.
pub const TEXT_MAX: usize = 300;
pub const COUNTER_FROM: usize = 240;

/// The rows of a record page's Questions section.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Section {
    /// Open questions in stored order, at most [`SECTION_OPEN`].
    pub open: Vec<Question>,
    /// Every open question of the record (the header's count).
    pub open_total: usize,
    /// Answered and dropped, newest closed first.
    pub closed: Vec<Question>,
}

impl Section {
    /// From the owner's own questions as the store lists them (open in
    /// stored order, then closed newest first).
    pub fn of(questions: Vec<Question>) -> Section {
        let (open, closed): (Vec<Question>, Vec<Question>) =
            questions.into_iter().partition(Question::is_open);
        Section {
            open_total: open.len(),
            open: open.into_iter().take(SECTION_OPEN).collect(),
            closed,
        }
    }

    /// Nothing of any state: the section is its header and one line.
    pub fn is_empty(&self) -> bool {
        self.open_total == 0 && self.closed.is_empty()
    }

    /// "Show all 14" when more than six are open.
    pub fn more(&self) -> Option<usize> {
        (self.open_total > SECTION_OPEN).then_some(self.open_total)
    }
}

/// The roll-up row of an epic or a project page: open questions below the
/// page's own level, and how many of them are overdue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rollup {
    pub open: usize,
    pub overdue: usize,
}

/// Count the open questions of `scope` (every level of an epic or a
/// project) owned below `owner`.
pub fn rollup(owner: Owner, scope: &[Question], today: Date) -> Rollup {
    let mut out = Rollup::default();
    for q in scope.iter().filter(|q| q.is_open() && q.owner != owner) {
        out.open += 1;
        if is_overdue(q, today) {
            out.overdue += 1;
        }
    }
    out
}

/// An open question whose answer date is before today. A closed one is
/// never overdue.
pub fn is_overdue(q: &Question, today: Date) -> bool {
    matches!(
        due_state(q.expected_on, today, q.is_open()),
        DueState::Overdue(_)
    )
}

/// The Questions page's segments (components-questions.md 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    Open,
    Overdue,
    Closed,
    All,
}

impl Filter {
    pub const ALL: [Filter; 4] = [Filter::Open, Filter::Overdue, Filter::Closed, Filter::All];
}

/// One group of the Questions page: an owner and its rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// `None`: the ungrouped Overdue list.
    pub owner: Option<Owner>,
    pub questions: Vec<Question>,
}

/// The owners of a project in tree order, widest first: the project, its
/// epics in stored order, then its topics (loose ones first, then each
/// epic's), each with its place.
pub fn owner_order(tree: &ProjectTree) -> HashMap<Owner, usize> {
    let mut order = HashMap::new();
    order.insert(Owner::Project(tree.project.id), 0);
    for (e, _) in &tree.epics {
        let n = order.len();
        order.insert(Owner::Epic(e.id), n);
    }
    let topics = tree
        .loose_topics
        .iter()
        .chain(tree.epics.iter().flat_map(|(_, t)| t));
    for t in topics {
        let n = order.len();
        order.insert(Owner::Topic(t.id), n);
    }
    order
}

/// Whether `q` is in a scope narrower than its project: an epic (its own
/// and its topics') or a topic.
pub fn in_scope(q: &Question, scope: Owner, epic_of: &HashMap<TopicId, EpicId>) -> bool {
    match scope {
        Owner::Project(_) => true,
        Owner::Epic(e) => match q.owner {
            Owner::Epic(x) => x == e,
            Owner::Topic(t) => epic_of.get(&t) == Some(&e),
            Owner::Project(_) => false,
        },
        Owner::Topic(t) => q.owner == Owner::Topic(t),
    }
}

/// The page's list for a filter: grouped by owner widest first, open ones
/// in stored order then closed newest first; Overdue ungrouped, earliest
/// answer date first. `questions` is the scope's.
pub fn page(
    questions: &[Question],
    filter: Filter,
    today: Date,
    order: &HashMap<Owner, usize>,
) -> Vec<Group> {
    let keep = |q: &&Question| match filter {
        Filter::Open => q.is_open(),
        Filter::Overdue => is_overdue(q, today),
        Filter::Closed => !q.is_open(),
        Filter::All => true,
    };
    let mut rows: Vec<Question> = questions.iter().filter(keep).cloned().collect();
    if filter == Filter::Overdue {
        rows.sort_by_key(|q| (q.expected_on, q.id.0));
        return if rows.is_empty() {
            Vec::new()
        } else {
            vec![Group {
                owner: None,
                questions: rows,
            }]
        };
    }
    let place = |o: &Owner| order.get(o).copied().unwrap_or(usize::MAX);
    rows.sort_by(|a, b| {
        place(&a.owner)
            .cmp(&place(&b.owner))
            .then(b.is_open().cmp(&a.is_open()))
            .then_with(|| {
                if a.is_open() {
                    a.position.cmp(&b.position)
                } else {
                    b.resolved_at.cmp(&a.resolved_at)
                }
            })
            .then(a.id.0.cmp(&b.id.0))
    });
    let mut groups: Vec<Group> = Vec::new();
    for q in rows {
        match groups.last_mut() {
            Some(g) if g.owner == Some(q.owner) => g.questions.push(q),
            _ => groups.push(Group {
                owner: Some(q.owner),
                questions: vec![q],
            }),
        }
    }
    groups
}

/// Open and closed counts of a scope ("Acta · 7 open, 12 closed").
pub fn counts(questions: &[Question]) -> (usize, usize) {
    let open = questions.iter().filter(|q| q.is_open()).count();
    (open, questions.len() - open)
}

/// The segment Overdue is shown only while an open question is late.
pub fn has_overdue(questions: &[Question], today: Date) -> bool {
    questions.iter().any(|q| is_overdue(q, today))
}

/// The topic table's optional columns (components-questions.md 5, Signals
/// elsewhere): Questions while a listed topic has an open question, Report
/// to while one names a contact.
pub fn table_columns(topics: &[TopicSummary], open: &HashMap<TopicId, (i64, i64)>) -> (bool, bool) {
    let questions = topics
        .iter()
        .any(|t| open.get(&t.id).is_some_and(|(n, _)| *n > 0));
    let report = topics.iter().any(|t| t.report_to.is_some());
    (questions, report)
}

/// The new index of an open question moved one step up or down among the
/// owner's open questions (`Alt+Up` / `Alt+Down`); `None` at an end.
pub fn step(index: usize, len: usize, down: bool) -> Option<usize> {
    if down {
        (index + 1 < len).then_some(index + 1)
    } else {
        index.checked_sub(1)
    }
}

/// Dragging an open question (components-questions.md 5): the gap a
/// pointer at `y` falls in among the open rows drawn at `rows` (each top
/// and bottom), as an insertion index from 0 (before the first) to the
/// number of rows (after the last).
pub fn drop_gap(rows: &[(f32, f32)], y: f32) -> usize {
    rows.iter()
        .position(|(top, bottom)| y < (top + bottom) / 2.)
        .unwrap_or(rows.len())
}

/// The index `reorder_question` takes for the row at `from` dropped in
/// `gap`; `None` on its own place (just above or below itself).
pub fn gap_target(from: usize, gap: usize) -> Option<usize> {
    if gap == from || gap == from + 1 {
        None
    } else if gap > from {
        Some(gap - 1)
    } else {
        Some(gap)
    }
}

/// A question's text as typed: one paragraph (line breaks become spaces,
/// as the store keeps it), cut at 300 characters.
pub fn clean_text(text: &str) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    one.chars().take(TEXT_MAX).collect()
}

/// The counter "264 / 300", over 240 characters only.
pub fn counter(text: &str) -> Option<usize> {
    let n = text.chars().count();
    (n > COUNTER_FROM).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{Author, ProjectId, QuestionId, QuestionStatus, Stamp};

    fn q(id: i64, owner: Owner, status: QuestionStatus, by: Option<&str>, pos: i64) -> Question {
        Question {
            id: QuestionId(id),
            project: ProjectId(1),
            owner,
            text: format!("Q{id}?"),
            asked_to: None,
            expected_on: by.map(|d| Date::parse_iso(d).unwrap()),
            status,
            answer: None,
            resolved_at: (status != QuestionStatus::Open).then_some(id * 10),
            resolved_by: None,
            decision_entry: None,
            position: pos,
            created_by: Author::Human,
            stamp: Stamp {
                uuid: String::new(),
                created_at: 0,
                updated_at: 0,
                updated_by: Author::Human,
                deleted_at: None,
                deleted_by: None,
            },
        }
    }

    fn today() -> Date {
        Date::parse_iso("2026-11-21").unwrap()
    }

    const P: Owner = Owner::Project(ProjectId(1));
    const E: Owner = Owner::Epic(EpicId(3));
    const T: Owner = Owner::Topic(TopicId(42));
    use QuestionStatus::{Answered, Dropped, Open};

    #[test]
    fn a_section_shows_six_open_and_folds_the_closed() {
        let mut all: Vec<Question> = (0..8).map(|i| q(i, T, Open, None, i)).collect();
        all.push(q(20, T, Answered, None, 0));
        all.push(q(21, T, Dropped, None, 0));
        let s = Section::of(all);
        assert_eq!(s.open.len(), 6);
        assert_eq!(s.open_total, 8);
        assert_eq!(s.more(), Some(8));
        assert_eq!(s.closed.len(), 2);
        assert!(!s.is_empty());
        assert!(Section::of(Vec::new()).is_empty());
        assert_eq!(Section::of(vec![q(1, T, Open, None, 0)]).more(), None);
    }

    #[test]
    fn the_rollup_counts_open_questions_below_the_page_and_the_late_ones() {
        let scope = vec![
            q(1, E, Open, Some("2026-11-01"), 0),
            q(2, T, Open, Some("2026-11-20"), 0),
            q(3, T, Open, Some("2026-11-21"), 1),
            q(4, T, Answered, Some("2026-11-01"), 2),
            q(5, T, Open, None, 3),
        ];
        // On the epic page, its own question is in the section, not rolled up.
        assert_eq!(
            rollup(E, &scope, today()),
            Rollup {
                open: 3,
                overdue: 1
            }
        );
        // Due today is not late; a closed question never is.
        assert_eq!(rollup(P, &scope, today()).overdue, 2);
    }

    fn tree() -> HashMap<Owner, usize> {
        HashMap::from([(P, 0), (E, 1), (T, 2)])
    }

    #[test]
    fn the_page_groups_by_owner_widest_first() {
        let all = vec![
            q(1, T, Open, None, 1),
            q(2, P, Open, None, 0),
            q(3, T, Open, None, 0),
            q(4, E, Answered, None, 0),
            q(5, T, Answered, None, 0),
        ];
        let open = page(&all, Filter::Open, today(), &tree());
        let owners: Vec<_> = open.iter().map(|g| g.owner).collect();
        assert_eq!(owners, [Some(P), Some(T)]);
        // Stored order inside a group, never by date.
        assert_eq!(
            open[1].questions.iter().map(|q| q.id.0).collect::<Vec<_>>(),
            [3, 1]
        );
        let every = page(&all, Filter::All, today(), &tree());
        // Open first, then closed newest first.
        assert_eq!(
            every[2]
                .questions
                .iter()
                .map(|q| q.id.0)
                .collect::<Vec<_>>(),
            [3, 1, 5]
        );
        assert_eq!(counts(&all), (3, 2));
    }

    #[test]
    fn overdue_is_one_list_earliest_first() {
        let all = vec![
            q(1, T, Open, Some("2026-11-18"), 0),
            q(2, P, Open, Some("2026-11-01"), 0),
            q(3, E, Open, Some("2026-11-21"), 0),
        ];
        let late = page(&all, Filter::Overdue, today(), &tree());
        assert_eq!(late.len(), 1);
        assert_eq!(late[0].owner, None);
        assert_eq!(
            late[0].questions.iter().map(|q| q.id.0).collect::<Vec<_>>(),
            [2, 1]
        );
        assert!(has_overdue(&all, today()));
        assert!(page(&all[2..], Filter::Overdue, today(), &tree()).is_empty());
    }

    #[test]
    fn a_scope_keeps_an_epics_topics_and_nothing_else() {
        let epic_of = HashMap::from([(TopicId(42), EpicId(3))]);
        let other = Owner::Topic(TopicId(7));
        assert!(in_scope(&q(1, T, Open, None, 0), E, &epic_of));
        assert!(in_scope(&q(1, E, Open, None, 0), E, &epic_of));
        assert!(!in_scope(&q(1, other, Open, None, 0), E, &epic_of));
        assert!(!in_scope(&q(1, P, Open, None, 0), E, &epic_of));
        assert!(in_scope(&q(1, other, Open, None, 0), P, &epic_of));
    }

    #[test]
    fn text_is_one_paragraph_and_counts_over_240() {
        assert_eq!(clean_text("Keep v1\nuntil  March?"), "Keep v1 until March?");
        assert_eq!(clean_text(&"a".repeat(400)).chars().count(), 300);
        assert_eq!(counter(&"a".repeat(240)), None);
        assert_eq!(counter(&"a".repeat(264)), Some(264));
    }

    #[test]
    fn a_dragged_question_lands_in_the_gap_under_the_pointer() {
        let rows = [(0., 32.), (32., 80.), (80., 112.)];
        assert_eq!(drop_gap(&rows, 5.), 0);
        assert_eq!(drop_gap(&rows, 20.), 1);
        assert_eq!(drop_gap(&rows, 60.), 2);
        assert_eq!(drop_gap(&rows, 200.), 3);
        // The second row dropped above the first, after the last, or on
        // its own place.
        assert_eq!(gap_target(1, 0), Some(0));
        assert_eq!(gap_target(1, 3), Some(2));
        assert_eq!(gap_target(1, 1), None);
        assert_eq!(gap_target(1, 2), None);
        assert_eq!(gap_target(0, 3), Some(2));
    }

    #[test]
    fn reordering_stops_at_the_ends() {
        assert_eq!(step(0, 3, false), None);
        assert_eq!(step(0, 3, true), Some(1));
        assert_eq!(step(2, 3, true), None);
    }

    #[test]
    fn table_columns_exist_only_while_used() {
        let topic = |id, report| TopicSummary {
            id: TopicId(id),
            epic: None,
            title: String::new(),
            status: acta_core::model::Status::Active,
            position: 0,
            entries: 0,
            due_on: None,
            updated_at: 0,
            updated_by: Author::Human,
            report_to: report,
            reported: false,
        };
        let none = vec![topic(1, None)];
        assert_eq!(table_columns(&none, &HashMap::new()), (false, false));
        let open = HashMap::from([(TopicId(1), (2, 1))]);
        assert_eq!(table_columns(&none, &open), (true, false));
        let named = vec![topic(
            1,
            Some(acta_core::model::ContactRef {
                id: acta_core::model::ContactId(1),
                name: "Marie".into(),
                in_trash: false,
            }),
        )];
        assert_eq!(table_columns(&named, &HashMap::new()), (false, true));
    }
}
