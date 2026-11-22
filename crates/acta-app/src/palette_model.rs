//! The command palette as data (docs/design/components.md 5, Command
//! palette; screens.md 8.3): matching, the fixed group order and the scope
//! chip. Topics and projects are matched in memory against the loaded tree
//! (16 ms per key over 10,000 topics); knowledge comes from the store's
//! full-text search, off the UI thread.

use acta_core::model::{EpicId, KbId, KbKind, ProjectId, Status, TopicId};

use crate::commands::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Recent,
    Topics,
    Epics,
    Knowledge,
    Commands,
    Projects,
    /// The rows of a picker (a destination, a heading): one group.
    Choices,
}

/// The order groups appear in, fixed so results do not jump, except that a
/// title equal to the query leads (see [`arrange`]).
pub const ORDER: [Group; 7] = [
    Group::Recent,
    Group::Topics,
    Group::Epics,
    Group::Knowledge,
    Group::Commands,
    Group::Projects,
    Group::Choices,
];

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Topic(TopicId, Status),
    Epic(EpicId),
    Project(ProjectId),
    Kb(KbId, KbKind),
    Command(Command),
    /// The row of a picker, by its index in the choices given.
    Choice(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub group: Group,
    pub target: Target,
    pub title: String,
    /// "Acta / Terminal", or a knowledge entry's summary or excerpt.
    pub detail: String,
    /// Character indices of `title` that matched.
    pub matched: Vec<usize>,
    pub score: i64,
}

/// A case-insensitive subsequence match. Higher is better: consecutive
/// characters, word starts and an early first match score more. `None`
/// when `query` is not a subsequence of `text`.
pub fn fuzzy(query: &str, text: &str) -> Option<(i64, Vec<usize>)> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    // A run of the query as typed wins: at a word start first, then
    // anywhere (so "term" marks "terminal", not the "t" of "the").
    let n = query.len();
    if n <= lower.len() {
        let starts: Vec<usize> = (0..=lower.len() - n)
            .filter(|&i| lower[i..i + n] == query[..])
            .collect();
        let word_start = |i: usize| i == 0 || !chars[i - 1].is_alphanumeric();
        if let Some(&at) = starts.iter().find(|&&i| word_start(i)).or(starts.first()) {
            let bonus = if word_start(at) { 40 } else { 20 };
            let exact = if n == chars.len() { 20 } else { 0 };
            let score = n as i64 * 6 + bonus + exact - at as i64 / 4;
            return Some((score, (at..at + n).collect()));
        }
    }
    let mut matched = Vec::with_capacity(query.len());
    let mut score = 0i64;
    let mut q = 0;
    let mut last: Option<usize> = None;
    for (i, c) in lower.iter().enumerate() {
        if q < query.len() && *c == query[q] {
            let word_start = i == 0 || !chars[i - 1].is_alphanumeric();
            score += 1;
            if word_start {
                score += 8;
            }
            if last == Some(i.wrapping_sub(1)) {
                score += 5;
            }
            matched.push(i);
            last = Some(i);
            q += 1;
        }
    }
    if q < query.len() {
        return None;
    }
    score -= matched.first().copied().unwrap_or(0) as i64 / 4;
    // A title that is exactly the query wins over one that merely holds it.
    if matched.len() == chars.len() {
        score += 20;
    }
    Some((score, matched))
}

/// Where a topic goes while the query is empty, with its score: under
/// Recent when it was opened lately (`recent_rank`); otherwise, in a
/// palette scoped to topics ("Go to topic...", `Mod+P`), under Topics with
/// the newest write first, so the list is never empty.
pub fn idle_topic(
    recent_rank: Option<usize>,
    scope: Option<Group>,
    updated_at: i64,
) -> Option<(Group, i64)> {
    match recent_rank {
        Some(rank) => Some((Group::Recent, -(rank as i64))),
        None if scope == Some(Group::Topics) => Some((Group::Topics, updated_at)),
        None => None,
    }
}

/// Whether `title` is the query itself, case and surrounding spaces
/// aside: what someone who types a name exactly means to open.
pub fn is_exact(query: &str, title: &str) -> bool {
    let query = query.trim();
    !query.is_empty() && title.trim().to_lowercase() == query.to_lowercase()
}

/// Order items: fixed group order, then score, then their given order
/// (recency for topics), and keep at most `limit` per group. A scope to
/// topics keeps the recent ones, which are topics too.
///
/// A title equal to `query` ranks first across groups (lead decision,
/// QA 2026-11-22): its group moves to the top and the exact row leads it,
/// so `Enter` opens what was named and never creates a duplicate.
pub fn arrange(mut items: Vec<Item>, scope: Option<Group>, limit: usize, query: &str) -> Vec<Item> {
    if let Some(scope) = scope {
        items.retain(|i| i.group == scope || (scope == Group::Topics && i.group == Group::Recent));
    }
    let lead = items
        .iter()
        .filter(|i| is_exact(query, &i.title))
        .map(|i| i.group)
        .min_by_key(|g| ORDER.iter().position(|o| o == g));
    let rank = |item: &Item| {
        let group = ORDER
            .iter()
            .position(|g| *g == item.group)
            .unwrap_or(ORDER.len());
        let first = lead.is_some_and(|g| g == item.group);
        (!first, group, !is_exact(query, &item.title))
    };
    // Stable: equal scores keep their given order.
    items.sort_by(|a, b| rank(a).cmp(&rank(b)).then(b.score.cmp(&a.score)));
    let mut out = Vec::with_capacity(items.len());
    let mut current: Option<Group> = None;
    let mut count = 0;
    for item in items {
        if current != Some(item.group) {
            current = Some(item.group);
            count = 0;
        }
        if count < limit {
            out.push(item);
            count += 1;
        }
    }
    out
}

/// The rows of a picker for `query`: every choice whose title (or detail)
/// matches, best first; with no query, all of them in the order given.
pub fn choices(choices: &[(String, String)], query: &str) -> Vec<Item> {
    choices
        .iter()
        .enumerate()
        .filter_map(|(i, (title, detail))| {
            let (score, matched) = match fuzzy(query, title) {
                Some(hit) => hit,
                // A match in the detail ("Acta / Terminal") ranks below
                // any match in a title.
                None => (fuzzy(query, detail)?.0 - 1_000, Vec::new()),
            };
            Some(Item {
                group: Group::Choices,
                target: Target::Choice(i),
                title: title.clone(),
                detail: detail.clone(),
                matched,
                score: if query.is_empty() { -(i as i64) } else { score },
            })
        })
        .collect()
}

/// `Up` / `Down` wrap (screens.md 8.3).
pub fn step(selected: usize, len: usize, down: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if down {
        (selected + 1) % len
    } else {
        (selected + len - 1) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(group: Group, title: &str, score: i64) -> Item {
        Item {
            group,
            target: Target::Project(ProjectId(1)),
            title: title.into(),
            detail: String::new(),
            matched: vec![],
            score,
        }
    }

    #[test]
    fn choices_keep_their_order_until_typed_and_details_match_last() {
        let all = [
            ("New topic".to_owned(), String::new()),
            (
                "Terminal".to_owned(),
                "New topic in Acta / Terminal".to_owned(),
            ),
            (
                "Scrollback".to_owned(),
                "Entry in Acta / Terminal".to_owned(),
            ),
        ];
        let idle = arrange(choices(&all, ""), Some(Group::Choices), 10, "");
        let order: Vec<_> = idle.iter().map(|i| i.target.clone()).collect();
        assert_eq!(
            order,
            [Target::Choice(0), Target::Choice(1), Target::Choice(2)]
        );
        let typed = arrange(choices(&all, "term"), Some(Group::Choices), 10, "term");
        assert_eq!(typed[0].target, Target::Choice(1), "title match first");
        assert_eq!(typed[1].target, Target::Choice(2), "then the detail");
        assert_eq!(typed.len(), 2);
    }

    #[test]
    fn subsequences_match_and_word_starts_rank_higher() {
        let (start, idx) = fuzzy("term", "Embed the terminal").unwrap();
        assert_eq!(idx, [10, 11, 12, 13]);
        let (scattered, _) = fuzzy("term", "The rest of my code").unwrap();
        assert!(start > scattered);
        assert!(fuzzy("xyz", "Embed the terminal").is_none());
        assert_eq!(fuzzy("", "anything"), Some((0, vec![])));
        assert!(fuzzy("ETT", "embed the terminal").is_some());
    }

    #[test]
    fn groups_keep_their_order_and_the_scope_narrows() {
        let items = vec![
            item(Group::Commands, "Toggle terminal", 9),
            item(Group::Topics, "Scrollback", 3),
            item(Group::Topics, "Embed the terminal", 7),
            item(Group::Projects, "Acta", 50),
        ];
        let arranged = arrange(items.clone(), None, 10, "");
        let titles: Vec<_> = arranged.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Embed the terminal",
                "Scrollback",
                "Toggle terminal",
                "Acta"
            ]
        );
        let topics = arrange(items.clone(), Some(Group::Topics), 10, "");
        assert_eq!(topics.len(), 2);
        assert_eq!(arrange(items, None, 1, "").len(), 3);
    }

    #[test]
    fn go_to_topic_lists_topics_before_anything_is_typed() {
        // Recent first, then every other topic, newest write first.
        assert_eq!(idle_topic(Some(1), None, 5), Some((Group::Recent, -1)));
        assert_eq!(idle_topic(None, None, 5), None);
        assert_eq!(
            idle_topic(None, Some(Group::Topics), 5),
            Some((Group::Topics, 5))
        );
        let items = vec![
            item(Group::Topics, "Older", 1),
            item(Group::Recent, "Opened lately", 0),
            item(Group::Topics, "Newer", 2),
            item(Group::Commands, "Toggle terminal", 0),
        ];
        let titles: Vec<_> = arrange(items, Some(Group::Topics), 10, "")
            .into_iter()
            .map(|i| i.title)
            .collect();
        assert_eq!(titles, ["Opened lately", "Newer", "Older"]);
    }

    #[test]
    fn an_exact_title_leads_across_groups() {
        // "Acta" names the project; a memory and a command merely hold it.
        let items = vec![
            item(Group::Topics, "Acta release notes", 90),
            item(Group::Knowledge, "Acta memory", 80),
            item(Group::Projects, "Acta", 50),
            item(Group::Commands, "Open Acta folder", 70),
        ];
        let arranged = arrange(items, None, 10, "acta");
        assert_eq!(arranged[0].title, "Acta");
        assert_eq!(arranged[0].group, Group::Projects);
        // The rest keep the fixed order.
        let rest: Vec<_> = arranged[1..].iter().map(|i| i.group).collect();
        assert_eq!(rest, vec![Group::Topics, Group::Knowledge, Group::Commands]);
    }

    #[test]
    fn an_epic_named_exactly_is_found_first_so_enter_opens_it() {
        let items = vec![
            item(Group::Topics, "Core UI polish", 120),
            item(Group::Epics, "Core UI", 60),
        ];
        let arranged = arrange(items, None, 10, " Core UI ");
        assert_eq!(arranged[0].group, Group::Epics);
        assert!(is_exact("core ui", "Core UI"));
        assert!(!is_exact("", ""));
        assert!(!is_exact("core", "Core UI"));
    }

    #[test]
    fn within_a_group_the_exact_title_comes_before_a_better_score() {
        let items = vec![
            item(Group::Topics, "Terminal tabs", 200),
            item(Group::Topics, "Terminal", 10),
        ];
        let arranged = arrange(items, None, 10, "terminal");
        assert_eq!(arranged[0].title, "Terminal");
    }

    #[test]
    fn up_and_down_wrap() {
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 0, true), 0);
    }
}
