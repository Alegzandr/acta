//! The command palette as data (docs/design/components.md 5, Command
//! palette; screens.md 8.3): matching, the fixed group order and the scope
//! chip. Topics and projects are matched in memory against the loaded tree
//! (16 ms per key over 10,000 topics); knowledge comes from the store's
//! full-text search, off the UI thread.

use acta_core::model::{KbId, KbKind, ProjectId, Status, TopicId};

use crate::commands::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Recent,
    Topics,
    Knowledge,
    Commands,
    Projects,
}

/// The order groups appear in, fixed so results do not jump.
pub const ORDER: [Group; 5] = [
    Group::Recent,
    Group::Topics,
    Group::Knowledge,
    Group::Commands,
    Group::Projects,
];

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Topic(TopicId, Status),
    Project(ProjectId),
    Kb(KbId, KbKind),
    Command(Command),
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

/// Order items: fixed group order, then score, then their given order
/// (recency for topics), and keep at most `limit` per group. A scope to
/// topics keeps the recent ones, which are topics too.
pub fn arrange(mut items: Vec<Item>, scope: Option<Group>, limit: usize) -> Vec<Item> {
    if let Some(scope) = scope {
        items.retain(|i| i.group == scope || (scope == Group::Topics && i.group == Group::Recent));
    }
    // Stable: equal scores keep their given order.
    items.sort_by(|a, b| {
        let ga = ORDER.iter().position(|g| *g == a.group);
        let gb = ORDER.iter().position(|g| *g == b.group);
        ga.cmp(&gb).then(b.score.cmp(&a.score))
    });
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
        let arranged = arrange(items.clone(), None, 10);
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
        let topics = arrange(items.clone(), Some(Group::Topics), 10);
        assert_eq!(topics.len(), 2);
        assert_eq!(arrange(items, None, 1).len(), 3);
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
        let titles: Vec<_> = arrange(items, Some(Group::Topics), 10)
            .into_iter()
            .map(|i| i.title)
            .collect();
        assert_eq!(titles, ["Opened lately", "Newer", "Older"]);
    }

    #[test]
    fn up_and_down_wrap() {
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 0, true), 0);
    }
}
