//! The topic table of epic and project pages (docs/design/components-
//! records.md 5): filter Open / All / Closed, and Overdue / Next 7 days
//! while a listed topic has a due date; the filter chip set from the
//! Overview; sort by a column, and the default order, which is the
//! tree's. Pure, so the rules are tested.

use acta_core::date::Date;
use acta_core::model::{EpicId, TopicId, TopicSummary};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    Open,
    /// Open topics due before today.
    Overdue,
    /// Open topics due before today plus 7, overdue included: a rolling
    /// week, so that on a Sunday it still looks ahead.
    Week,
    All,
    Closed,
}

impl Filter {
    /// The segments shown: the two due segments only while a listed topic
    /// has a date.
    pub fn shown(dated: bool) -> &'static [Filter] {
        if dated {
            &[
                Filter::Open,
                Filter::Overdue,
                Filter::Week,
                Filter::All,
                Filter::Closed,
            ]
        } else {
            &[Filter::Open, Filter::All, Filter::Closed]
        }
    }

    /// The filter in force: a due segment falls back to Open once the
    /// last date is cleared.
    pub fn effective(self, dated: bool) -> Filter {
        match self {
            Filter::Overdue | Filter::Week if !dated => Filter::Open,
            other => other,
        }
    }

    pub fn keeps(self, topic: &TopicSummary, today: Date) -> bool {
        let open = topic.status.is_open();
        match self {
            Filter::Open => open,
            Filter::Overdue => open && topic.due_on.is_some_and(|d| d < today),
            Filter::Week => open && topic.due_on.is_some_and(|d| d < today.add_days(7)),
            Filter::All => true,
            Filter::Closed => !open,
        }
    }
}

/// Whether a listed topic, open or closed, has a due date: the Due column
/// and the two due segments exist only then.
pub fn has_dates(topics: &[TopicSummary]) -> bool {
    topics.iter().any(|t| t.due_on.is_some())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Status,
    Title,
    Due,
    Entries,
    Updated,
}

/// A sort the user chose by activating a column header; `None` is the
/// tree's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub ascending: bool,
}

/// Activating a header: a new column sorts ascending (Updated: newest
/// first), the same column flips.
pub fn next_sort(current: Option<Sort>, column: Column) -> Sort {
    match current {
        Some(sort) if sort.column == column => Sort {
            column,
            ascending: !sort.ascending,
        },
        _ => Sort {
            column,
            ascending: column != Column::Updated,
        },
    }
}

/// The rows to show, in order. Topics come in the tree's order; the due
/// segments list by due date, earliest first, until a header is activated.
pub fn rows(
    topics: &[TopicSummary],
    filter: Filter,
    sort: Option<Sort>,
    today: Date,
) -> Vec<&TopicSummary> {
    let rows: Vec<&TopicSummary> = topics.iter().filter(|t| filter.keeps(t, today)).collect();
    let sort = sort.or(match filter {
        Filter::Overdue | Filter::Week => Some(Sort {
            column: Column::Due,
            ascending: true,
        }),
        _ => None,
    });
    sorted(rows, sort)
}

/// The rows under a filter chip: the chip's topics in its list's order
/// (blocked in tree order, idle the longest first, ...); sorting by a
/// column still works.
pub fn chip_rows<'a>(
    topics: &'a [TopicSummary],
    ids: &[TopicId],
    sort: Option<Sort>,
) -> Vec<&'a TopicSummary> {
    let rows = ids
        .iter()
        .filter_map(|id| topics.iter().find(|t| t.id == *id))
        .collect();
    sorted(rows, sort)
}

fn sorted(mut rows: Vec<&TopicSummary>, sort: Option<Sort>) -> Vec<&TopicSummary> {
    let Some(sort) = sort else {
        return rows;
    };
    rows.sort_by(|a, b| {
        let flip = |order: std::cmp::Ordering| {
            if sort.ascending {
                order
            } else {
                order.reverse()
            }
        };
        let order = match sort.column {
            Column::Status => flip(status_rank(a).cmp(&status_rank(b))),
            Column::Title => flip(a.title.to_lowercase().cmp(&b.title.to_lowercase())),
            Column::Entries => flip(a.entries.cmp(&b.entries)),
            Column::Updated => flip(a.updated_at.cmp(&b.updated_at)),
            // No date comes last in both directions.
            Column::Due => match (a.due_on, b.due_on) {
                (Some(x), Some(y)) => flip(x.cmp(&y)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        };
        // Ties keep the tree's order (sort_by is stable).
        order
    });
    rows
}

fn status_rank(topic: &TopicSummary) -> usize {
    acta_core::model::Status::ALL
        .iter()
        .position(|s| *s == topic.status)
        .unwrap_or(0)
}

/// Whether the project table groups its rows by epic under `filter`: the
/// due segments list by date across epics instead (components-records.md
/// 5, "list by due date, earliest first").
pub fn grouped(filter: Filter) -> bool {
    !matches!(filter, Filter::Overdue | Filter::Week)
}

/// A group of the project table: its epic (none for "No epic") and topics.
pub type TableGroup<'a> = (Option<(EpicId, &'a str)>, &'a [TopicSummary]);

/// A project page groups its table by epic, topics outside any epic first
/// under "No epic" (docs/design/components-records.md 5).
pub fn groups<'a>(
    loose: &'a [TopicSummary],
    epics: &'a [(EpicId, String, Vec<TopicSummary>)],
) -> Vec<TableGroup<'a>> {
    let mut out: Vec<TableGroup<'a>> = Vec::new();
    if !loose.is_empty() {
        out.push((None, loose));
    }
    for (id, title, topics) in epics {
        out.push((Some((*id, title.as_str())), topics.as_slice()));
    }
    out
}

/// Done over all topics not Dropped (docs/design/data.md 11).
pub fn progress(topics: &[TopicSummary]) -> (i64, i64) {
    use acta_core::model::Status;
    let total = topics
        .iter()
        .filter(|t| t.status != Status::Dropped)
        .count() as i64;
    let done = topics.iter().filter(|t| t.status == Status::Done).count() as i64;
    (done, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_due_segments_list_across_epics_the_others_group() {
        assert!(!grouped(Filter::Overdue));
        assert!(!grouped(Filter::Week));
        assert!(grouped(Filter::Open));
        assert!(grouped(Filter::All));
        assert!(grouped(Filter::Closed));
    }
    use crate::tree::sample::topic;
    use acta_core::model::Status;

    fn sample() -> Vec<TopicSummary> {
        let mut topics = vec![
            topic(1, Some(1), "Embed the terminal", Status::Active),
            topic(2, Some(1), "Scrollback", Status::Blocked),
            topic(3, Some(1), "Links", Status::Planned),
            topic(4, Some(1), "Spike", Status::Done),
            topic(5, Some(1), "Winpty", Status::Dropped),
        ];
        for (n, t) in topics.iter_mut().enumerate() {
            t.entries = [14, 6, 0, 3, 1][n];
            t.updated_at = [50, 40, 30, 20, 10][n];
        }
        topics
    }

    fn titles(rows: &[&TopicSummary]) -> Vec<String> {
        rows.iter().map(|t| t.title.clone()).collect()
    }

    fn day(iso: &str) -> Date {
        Date::parse_iso(iso).unwrap()
    }

    /// Saturday 21 November 2026, the day of screens.md 8.14.
    fn today() -> Date {
        day("2026-11-21")
    }

    #[test]
    fn filters_split_open_and_closed() {
        let topics = sample();
        assert_eq!(rows(&topics, Filter::Open, None, today()).len(), 3);
        assert_eq!(rows(&topics, Filter::Closed, None, today()).len(), 2);
        assert_eq!(rows(&topics, Filter::All, None, today()).len(), 5);
    }

    /// The table of screens.md 8.14 plus a later date, a closed overdue
    /// topic and an open topic due in 7 days.
    fn dated() -> Vec<TopicSummary> {
        let mut topics = sample();
        topics[0].due_on = Some(day("2026-11-18"));
        topics[1].due_on = Some(day("2026-11-21"));
        topics[2].due_on = Some(day("2026-11-24"));
        topics[3].due_on = Some(day("2026-11-01"));
        topics.push(topic(6, Some(1), "Session restore", Status::Draft));
        topics.push(topic(7, Some(1), "Themes", Status::Planned));
        topics[6].due_on = Some(day("2026-11-28"));
        topics
    }

    #[test]
    fn due_segments_and_the_column_exist_only_with_a_date() {
        assert!(!has_dates(&sample()));
        assert_eq!(Filter::shown(false).len(), 3);
        assert_eq!(Filter::shown(true).len(), 5);
        assert!(has_dates(&dated()));
        // A closed topic's date counts too.
        let mut closed = sample();
        closed[4].due_on = Some(day("2026-11-01"));
        assert!(has_dates(&closed));
        // The last date cleared while a due segment is selected: Open.
        assert_eq!(Filter::Overdue.effective(false), Filter::Open);
        assert_eq!(Filter::Week.effective(false), Filter::Open);
        assert_eq!(Filter::Week.effective(true), Filter::Week);
        assert_eq!(Filter::Closed.effective(false), Filter::Closed);
    }

    #[test]
    fn overdue_and_next_7_days_list_open_topics_by_date() {
        let topics = dated();
        // Overdue: open and before today; the done one is never overdue.
        assert_eq!(
            titles(&rows(&topics, Filter::Overdue, None, today())),
            ["Embed the terminal"]
        );
        // Next 7 days: overdue included, today plus 6, earliest first;
        // the 28th is 7 days ahead and out.
        assert_eq!(
            titles(&rows(&topics, Filter::Week, None, today())),
            ["Embed the terminal", "Scrollback", "Links"]
        );
        // On the 22nd the 28th is in, and the count follows.
        let next = rows(&topics, Filter::Week, None, today().add_days(1));
        assert_eq!(next.len(), 4);
        // At midnight a topic due today becomes overdue.
        assert_eq!(
            rows(&topics, Filter::Overdue, None, today().add_days(1)).len(),
            2
        );
    }

    #[test]
    fn sorting_by_due_puts_no_date_last_both_ways_and_keeps_ties() {
        let mut topics = dated();
        topics[2].due_on = Some(day("2026-11-18"));
        let up = next_sort(None, Column::Due);
        assert!(up.ascending);
        assert_eq!(
            titles(&rows(&topics, Filter::All, Some(up), today())),
            [
                "Spike",
                "Embed the terminal",
                "Links",
                "Scrollback",
                "Themes",
                "Winpty",
                "Session restore"
            ]
        );
        let down = next_sort(Some(up), Column::Due);
        assert_eq!(
            titles(&rows(&topics, Filter::All, Some(down), today())),
            [
                "Themes",
                "Scrollback",
                "Embed the terminal",
                "Links",
                "Spike",
                "Winpty",
                "Session restore"
            ]
        );
    }

    #[test]
    fn a_chip_lists_its_topics_in_the_list_order_and_still_sorts() {
        let topics = sample();
        let ids = [TopicId(3), TopicId(1)];
        assert_eq!(
            titles(&chip_rows(&topics, &ids, None)),
            ["Links", "Embed the terminal"]
        );
        let by_title = next_sort(None, Column::Title);
        assert_eq!(
            titles(&chip_rows(&topics, &ids, Some(by_title))),
            ["Embed the terminal", "Links"]
        );
        // A topic gone since the list was read is skipped.
        assert_eq!(chip_rows(&topics, &[TopicId(99)], None).len(), 0);
    }

    #[test]
    fn sorting_by_a_column_and_flipping_it() {
        let topics = sample();
        let by_title = next_sort(None, Column::Title);
        assert!(by_title.ascending);
        assert_eq!(
            titles(&rows(&topics, Filter::Open, Some(by_title), today())),
            ["Embed the terminal", "Links", "Scrollback"]
        );
        let flipped = next_sort(Some(by_title), Column::Title);
        assert!(!flipped.ascending);
        let updated = next_sort(Some(flipped), Column::Updated);
        assert!(!updated.ascending, "newest first");
        assert_eq!(
            titles(&rows(&topics, Filter::All, Some(updated), today()))[0],
            "Embed the terminal"
        );
        let entries = next_sort(None, Column::Entries);
        assert_eq!(
            titles(&rows(&topics, Filter::Open, Some(entries), today())),
            ["Links", "Scrollback", "Embed the terminal"]
        );
    }

    #[test]
    fn progress_counts_done_over_not_dropped() {
        assert_eq!(progress(&sample()), (1, 4));
        assert_eq!(progress(&[]), (0, 0));
    }

    #[test]
    fn project_groups_put_no_epic_first_and_skip_it_when_empty() {
        let loose = vec![topic(9, None, "Naming", Status::Draft)];
        let epics = vec![(EpicId(1), "Terminal".to_string(), sample())];
        let g = groups(&loose, &epics);
        assert!(g[0].0.is_none());
        assert_eq!(g[1].0.unwrap().1, "Terminal");
        assert_eq!(groups(&[], &epics).len(), 1);
    }
}
