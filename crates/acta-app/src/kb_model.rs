//! The knowledge section and knowledge base page as data (docs/design/
//! components-knowledge.md 5): own entries first, documents before
//! memories, each by last update; inherited entries grouped by the level
//! they come from, nearest first; and search excerpts split into plain and
//! matched runs.

use acta_core::model::{KbKind, KbSummary, Level, Owner, ProjectTree};
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

/// The knowledge base page's one filter chip (components-knowledge.md 5,
/// Memory quality), set from the Overview, the palette or the list's last
/// row; never saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chip {
    /// Entries with a reason to check, ungrouped, first reason first.
    ToCheck,
    /// Superseded and retired entries, newest state change first.
    Inactive,
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

/// A document's body cut at its headings (the outline of
/// `acta_core::markdown`, headings in code fences left alone), so the
/// reading view can scroll to each one: the text before the first heading
/// (if any), then one part per heading, each paired with the index of its
/// heading in the outline.
pub fn body_parts(body: &str) -> Vec<(Option<usize>, String)> {
    let outline = acta_core::markdown::outline(body);
    let lines: Vec<&str> = body.lines().collect();
    let starts: Vec<usize> = outline.iter().map(|s| s.start).collect();
    let mut out = Vec::new();
    let first = starts.first().copied().unwrap_or(lines.len());
    if first > 0 {
        let head = lines[..first].join("\n");
        if !head.trim().is_empty() {
            out.push((None, head));
        }
    }
    for (i, start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(lines.len());
        out.push((Some(i), lines[*start..end].join("\n")));
    }
    out
}

/// The suggested name of "Save as file": the title with what file systems
/// refuse replaced by `-`, at most 80 characters, then `.md`.
pub fn file_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '-'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "document.md".to_owned()
    } else {
        format!("{cleaned}.md")
    }
}

/// What "Save as file" writes: the body as stored, under the title as a
/// first-level heading unless the body already starts with one.
pub fn file_text(title: &str, body: &str) -> String {
    let body = body.trim_end();
    if body.trim_start().starts_with("# ") {
        format!("{body}\n")
    } else {
        format!("# {title}\n\n{body}\n")
    }
}

/// Where "Move to..." can send an entry of `tree`'s project: the project,
/// each epic, each topic (loose first, then by epic), without `current`.
/// Each with its name and, for epics and topics, the path above it.
pub fn move_targets(tree: &ProjectTree, current: Owner) -> Vec<(Owner, String, String)> {
    let project = &tree.project.title;
    let mut out = vec![(
        Owner::Project(tree.project.id),
        project.clone(),
        String::new(),
    )];
    for (epic, _) in &tree.epics {
        out.push((Owner::Epic(epic.id), epic.title.clone(), project.clone()));
    }
    for topic in &tree.loose_topics {
        out.push((Owner::Topic(topic.id), topic.title.clone(), project.clone()));
    }
    for (epic, topics) in &tree.epics {
        for topic in topics {
            out.push((
                Owner::Topic(topic.id),
                topic.title.clone(),
                format!("{project} / {}", epic.title),
            ));
        }
    }
    out.retain(|(owner, ..)| *owner != current);
    out
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
            verified_at: updated_at,
            state: acta_core::model::KbState::Live,
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

    #[test]
    fn a_body_is_cut_at_its_headings_and_fences_are_left_alone() {
        let body =
            "Intro by Alexandre\n\n# Storage\nKeys.\n```\n# not a heading\n```\n## Keys\nMore.";
        let parts = body_parts(body);
        let heads: Vec<_> = parts.iter().map(|(h, _)| *h).collect();
        assert_eq!(heads, [None, Some(0), Some(1)]);
        assert!(parts[1].1.contains("# not a heading"));
        assert_eq!(parts[2].1, "## Keys\nMore.");
        assert_eq!(body_parts("# Only").len(), 1);
        assert!(body_parts("").is_empty());
    }

    #[test]
    fn a_saved_file_is_named_from_the_title_and_starts_with_it() {
        assert_eq!(
            file_name("Storage: keys/SQLCipher?"),
            "Storage- keys-SQLCipher-.md"
        );
        assert_eq!(file_name("  ...  "), "document.md");
        assert_eq!(file_text("Storage", "Keys."), "# Storage\n\nKeys.\n");
        assert_eq!(
            file_text("Storage", "# Storage\nKeys.\n\n"),
            "# Storage\nKeys.\n"
        );
    }

    #[test]
    fn an_entry_moves_to_any_other_level_of_its_project() {
        let tree = &crate::tree::sample::acta()[0];
        let targets = move_targets(tree, Owner::Topic(acta_core::model::TopicId(2)));
        assert_eq!(targets[0].0, Owner::Project(tree.project.id));
        assert_eq!(targets[1].1, "Terminal");
        assert!(
            !targets
                .iter()
                .any(|(o, ..)| *o == Owner::Topic(acta_core::model::TopicId(2)))
        );
        assert!(
            targets
                .iter()
                .any(|(_, name, path)| name == "Scrollback" && path == "Acta / Terminal")
        );
    }
}
