//! The topic table of epic and project pages (docs/design/components-
//! records.md 5): filter Open / All / Closed, sort by a column, and the
//! default order, which is the tree's. Pure, so the rules are tested.

use acta_core::model::{EpicId, TopicSummary};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    Open,
    All,
    Closed,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::Open, Filter::All, Filter::Closed];

    pub fn keeps(self, topic: &TopicSummary) -> bool {
        match self {
            Filter::Open => topic.status.is_open(),
            Filter::All => true,
            Filter::Closed => !topic.status.is_open(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Status,
    Title,
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

/// The rows to show, in order. Topics come in the tree's order.
pub fn rows(topics: &[TopicSummary], filter: Filter, sort: Option<Sort>) -> Vec<&TopicSummary> {
    let mut rows: Vec<&TopicSummary> = topics.iter().filter(|t| filter.keeps(t)).collect();
    if let Some(sort) = sort {
        rows.sort_by(|a, b| {
            let order = match sort.column {
                Column::Status => status_rank(a).cmp(&status_rank(b)),
                Column::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                Column::Entries => a.entries.cmp(&b.entries),
                Column::Updated => a.updated_at.cmp(&b.updated_at),
            };
            let order = if sort.ascending {
                order
            } else {
                order.reverse()
            };
            // Ties keep the tree's order.
            order.then(a.position.cmp(&b.position))
        });
    }
    rows
}

fn status_rank(topic: &TopicSummary) -> usize {
    acta_core::model::Status::ALL
        .iter()
        .position(|s| *s == topic.status)
        .unwrap_or(0)
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

    #[test]
    fn filters_split_open_and_closed() {
        let topics = sample();
        assert_eq!(rows(&topics, Filter::Open, None).len(), 3);
        assert_eq!(rows(&topics, Filter::Closed, None).len(), 2);
        assert_eq!(rows(&topics, Filter::All, None).len(), 5);
    }

    #[test]
    fn sorting_by_a_column_and_flipping_it() {
        let topics = sample();
        let by_title = next_sort(None, Column::Title);
        assert!(by_title.ascending);
        assert_eq!(
            titles(&rows(&topics, Filter::Open, Some(by_title))),
            ["Embed the terminal", "Links", "Scrollback"]
        );
        let flipped = next_sort(Some(by_title), Column::Title);
        assert!(!flipped.ascending);
        let updated = next_sort(Some(flipped), Column::Updated);
        assert!(!updated.ascending, "newest first");
        assert_eq!(
            titles(&rows(&topics, Filter::All, Some(updated)))[0],
            "Embed the terminal"
        );
        let entries = next_sort(None, Column::Entries);
        assert_eq!(
            titles(&rows(&topics, Filter::Open, Some(entries))),
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
