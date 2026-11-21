//! The knowledge section and knowledge base page as data (docs/design/
//! components-knowledge.md 5): own entries first, documents before
//! memories, each by last update; inherited entries grouped by the level
//! they come from, nearest first; and search excerpts split into plain and
//! matched runs.

use acta_core::model::{KbKind, KbSummary, Level, Owner};
use acta_core::store::{SNIPPET_CLOSE, SNIPPET_OPEN};

/// How many own entries the section shows before "Show all 14".
pub const SECTION_LIMIT: usize = 6;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub own: Vec<KbSummary>,
    /// Inherited groups, nearest level first.
    pub inherited: Vec<(Owner, Vec<KbSummary>)>,
}

impl Section {
    pub fn inherited_count(&self) -> usize {
        self.inherited
            .iter()
            .map(|(_, entries)| entries.len())
            .sum()
    }
}

fn level_rank(owner: Owner) -> u8 {
    match owner.level() {
        Level::Topic => 0,
        Level::Epic => 1,
        Level::Project => 2,
    }
}

/// Documents before memories, then newest update first.
pub fn sort_own(entries: &mut [KbSummary]) {
    entries.sort_by(|a, b| {
        let kind = |k: KbKind| matches!(k, KbKind::Memory) as u8;
        kind(a.kind)
            .cmp(&kind(b.kind))
            .then(b.updated_at.cmp(&a.updated_at))
    });
}

/// Split what the store's scope query returned for `owner` (its entries,
/// then those of the levels above) into the section's parts.
pub fn section(owner: Owner, scope: Vec<KbSummary>) -> Section {
    let mut own = Vec::new();
    let mut inherited: Vec<(Owner, Vec<KbSummary>)> = Vec::new();
    for entry in scope {
        if entry.owner == owner {
            own.push(entry);
        } else if let Some((_, group)) = inherited.iter_mut().find(|(o, _)| *o == entry.owner) {
            group.push(entry);
        } else {
            inherited.push((entry.owner, vec![entry]));
        }
    }
    sort_own(&mut own);
    for (_, group) in &mut inherited {
        sort_own(group);
    }
    inherited.sort_by_key(|(o, _)| level_rank(*o));
    Section { own, inherited }
}

/// A search excerpt as runs of `(text, matched)`.
pub fn excerpt_runs(snippet: &str) -> Vec<(String, bool)> {
    let mut runs = Vec::new();
    let mut current = String::new();
    let mut matched = false;
    for c in snippet.chars() {
        if c == SNIPPET_OPEN || c == SNIPPET_CLOSE {
            if !current.is_empty() {
                runs.push((std::mem::take(&mut current), matched));
            }
            matched = c == SNIPPET_OPEN;
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        runs.push((current, matched));
    }
    runs
}

/// The excerpt as plain text (palette rows, tooltips).
pub fn excerpt_plain(snippet: &str) -> String {
    snippet
        .chars()
        .filter(|c| *c != SNIPPET_OPEN && *c != SNIPPET_CLOSE)
        .collect()
}

/// The knowledge base page's filters (docs/design/components-knowledge.md 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    pub level: Option<Level>,
    pub kind: Option<KbKind>,
    pub author: Option<acta_core::model::Author>,
}

impl Filters {
    pub fn is_default(&self) -> bool {
        *self == Filters::default()
    }

    pub fn keeps(&self, entry: &KbSummary) -> bool {
        self.level.is_none_or(|l| entry.owner.level() == l)
            && self.kind.is_none_or(|k| entry.kind == k)
            && self.author.is_none_or(|a| entry.created_by == a)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use acta_core::model::{Author, EpicId, KbId, ProjectId, TopicId};

    pub fn entry(id: i64, owner: Owner, kind: KbKind, updated_at: i64) -> KbSummary {
        KbSummary {
            id: KbId(id),
            kind,
            owner,
            title: format!("K{id}"),
            summary: String::new(),
            rev: 1,
            size: 10,
            created_by: Author::Claude,
            updated_at,
            updated_by: Author::Claude,
        }
    }

    #[test]
    fn own_entries_first_documents_before_memories_then_nearest_levels() {
        let topic = Owner::Topic(TopicId(1));
        let epic = Owner::Epic(EpicId(1));
        let project = Owner::Project(ProjectId(1));
        let scope = vec![
            entry(1, topic, KbKind::Memory, 30),
            entry(2, topic, KbKind::Document, 10),
            entry(3, topic, KbKind::Memory, 40),
            entry(4, project, KbKind::Memory, 50),
            entry(5, epic, KbKind::Document, 5),
        ];
        let s = section(topic, scope);
        let own: Vec<i64> = s.own.iter().map(|e| e.id.0).collect();
        assert_eq!(own, [2, 3, 1]);
        let levels: Vec<Owner> = s.inherited.iter().map(|(o, _)| *o).collect();
        assert_eq!(levels, [epic, project]);
        assert_eq!(s.inherited_count(), 2);
    }

    #[test]
    fn excerpts_split_into_matched_runs() {
        let runs = excerpt_runs("the grid \u{2}resize\u{3}s on every \u{2}resize\u{3}");
        assert_eq!(
            runs,
            [
                ("the grid ".to_string(), false),
                ("resize".to_string(), true),
                ("s on every ".to_string(), false),
                ("resize".to_string(), true),
            ]
        );
        assert_eq!(excerpt_plain("a \u{2}b\u{3} c"), "a b c");
    }

    #[test]
    fn filters_combine_and_roll_ups_group_widest_first() {
        let topic = Owner::Topic(TopicId(1));
        let project = Owner::Project(ProjectId(1));
        let entries = [
            entry(1, topic, KbKind::Memory, 1),
            entry(2, project, KbKind::Document, 2),
            entry(3, topic, KbKind::Document, 3),
        ];
        let filters = Filters {
            kind: Some(KbKind::Memory),
            author: Some(Author::Claude),
            ..Filters::default()
        };
        assert!(!filters.is_default());
        assert_eq!(entries.iter().filter(|e| filters.keeps(e)).count(), 1);
    }
}
