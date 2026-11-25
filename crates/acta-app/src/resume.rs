//! Resume where you left off (docs/design/interaction.md 6.6): what a page
//! keeps across a restart and how it is named. Pure, so the restore model
//! is tested without a window.
//!
//! `preferences.json` gets numbers, record ids and fixed keys only: each
//! open tab's scroll and focused element ([`PageState`], [`focus_key`]),
//! and the pages whose Claude Code session was running ([`read_sessions`]).
//! Drafts are content: they go to the encrypted database under
//! [`draft_key`], never to the file.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use acta_core::model::{EntryId, Owner};

use crate::fold::Section;
use crate::record::Block;

/// At most this many pages keep a state, and this many sessions restart.
pub const PAGES_MAX: usize = 64;
pub const SESSIONS_MAX: usize = 16;

static QUITTING: AtomicBool = AtomicBool::new(false);

/// Acta is quitting: the sessions that end now still restart next time.
pub fn set_quitting() {
    QUITTING.store(true, Ordering::Relaxed);
}

pub fn quitting() -> bool {
    QUITTING.load(Ordering::Relaxed)
}

/// The kept state of one page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageState {
    /// The scroll, in effective pixels (at 100% zoom), so a zoom change
    /// between two runs lands on the same content.
    pub scroll: u32,
    /// The focused element, as [`focus_key`] writes it.
    pub focus: Option<String>,
}

/// Whether `key` is a route key or a record id as `preferences.json` keeps
/// them: short, ASCII letters, digits, `-` and `:` only.
pub fn is_reference(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 32
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':')
}

/// The fixed name of a focused block of a record page.
pub fn focus_key(block: Block) -> String {
    match block {
        Block::Title => "title".into(),
        Block::Folders => "folders".into(),
        Block::State => "state".into(),
        Block::Description => "description".into(),
        Block::Questions => "questions".into(),
        Block::Fold(section) => format!("fold:{}", section_name(section)),
        Block::Entry(id) => format!("entry:{}", id.0),
    }
}

/// The block a kept focus key names, or `None` for anything else (a value
/// edited by hand, a key of a later build).
pub fn focus_block(key: &str) -> Option<Block> {
    Some(match key {
        "title" => Block::Title,
        "folders" => Block::Folders,
        "state" => Block::State,
        "description" => Block::Description,
        "questions" => Block::Questions,
        _ => {
            if let Some(name) = key.strip_prefix("fold:") {
                Block::Fold(section_of(name)?)
            } else {
                let id = key.strip_prefix("entry:")?.parse::<i64>().ok();
                Block::Entry(EntryId(id.filter(|n| *n > 0)?))
            }
        }
    })
}

const SECTIONS: [(Section, &str); 9] = [
    (Section::Overview, "overview"),
    (Section::Description, "description"),
    (Section::Questions, "questions"),
    (Section::Knowledge, "knowledge"),
    (Section::Timeline, "timeline"),
    (Section::Topics, "topics"),
    (Section::Address, "address"),
    (Section::Asked, "asked"),
    (Section::Report, "report"),
];

fn section_name(section: Section) -> &'static str {
    SECTIONS
        .iter()
        .find(|(s, _)| *s == section)
        .map_or("", |(_, n)| n)
}

fn section_of(name: &str) -> Option<Section> {
    SECTIONS.iter().find(|(_, n)| *n == name).map(|(s, _)| *s)
}

/// The pages' states as read from the file: route keys and focus keys of
/// the right shape only, at most [`PAGES_MAX`]; anything else is dropped
/// and so never written back.
pub fn read_pages(stored: BTreeMap<String, PageState>) -> BTreeMap<String, PageState> {
    stored
        .into_iter()
        .filter(|(route, _)| is_reference(route))
        .map(|(route, mut state)| {
            state.focus = state.focus.filter(|f| focus_block(f).is_some());
            state.scroll = state.scroll.min(1_000_000);
            (route, state)
        })
        .take(PAGES_MAX)
        .collect()
}

/// Keep only the pages still open in a tab: a closed tab's place is not
/// worth keeping, and the map stays as small as the tabs.
pub fn prune(pages: &mut BTreeMap<String, PageState>, open: &[String]) {
    pages.retain(|route, _| open.contains(route));
}

/// Record a page's scroll; whether it changed.
pub fn set_scroll(pages: &mut BTreeMap<String, PageState>, route: &str, scroll: f32) -> bool {
    if !is_reference(route) {
        return false;
    }
    let scroll = scroll.max(0.).round() as u32;
    let state = pages.entry(route.to_owned()).or_default();
    let changed = state.scroll != scroll;
    state.scroll = scroll;
    changed
}

/// Record a page's focused element (`None`: the page itself); whether it
/// changed.
pub fn set_focus(
    pages: &mut BTreeMap<String, PageState>,
    route: &str,
    focus: Option<Block>,
) -> bool {
    if !is_reference(route) {
        return false;
    }
    let focus = focus.map(focus_key);
    let state = pages.entry(route.to_owned()).or_default();
    let changed = state.focus != focus;
    state.focus = focus;
    changed
}

/// The record ids of the pages whose session restarts: ids only, once
/// each, in their order, at most [`SESSIONS_MAX`].
pub fn read_sessions(stored: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in stored {
        if parse_owner(&id).is_some() && !out.contains(&id) && out.len() < SESSIONS_MAX {
            out.push(id);
        }
    }
    out
}

/// Where quick capture's draft is kept (one, like the overlay).
pub const CAPTURE_DRAFT_KEY: &str = "draft.capture";

/// The record a kept id names (`T-42`, `E-3`, `P-1`).
pub fn parse_owner(id: &str) -> Option<Owner> {
    use acta_core::model::{EpicId, ProjectId, TopicId};
    if !id.contains('-') {
        return None;
    }
    ProjectId::parse(id)
        .map(Owner::Project)
        .or_else(|| EpicId::parse(id).map(Owner::Epic))
        .or_else(|| TopicId::parse(id).map(Owner::Topic))
}

/// An editor whose text survives a restart (data.md request 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftField {
    State,
    Description,
}

impl DraftField {
    fn name(self) -> &'static str {
        match self {
            DraftField::State => "state",
            DraftField::Description => "description",
        }
    }
}

/// Where a record's draft is kept: a local value of the encrypted
/// database, never exported.
pub fn draft_key(owner: Owner, field: DraftField) -> String {
    format!("{}{}", draft_prefix(owner), field.name())
}

/// The prefix of every draft of `owner`.
pub fn draft_prefix(owner: Owner) -> String {
    format!("draft.{owner}.")
}

/// The field a kept draft key of `owner` names.
pub fn draft_field(owner: Owner, key: &str) -> Option<DraftField> {
    match key.strip_prefix(&draft_prefix(owner))? {
        "state" => Some(DraftField::State),
        "description" => Some(DraftField::Description),
        _ => None,
    }
}

/// What to keep for an editor holding `typed` over the stored `stored`:
/// `None` when it says the same (no draft, the key is removed).
pub fn draft_value(typed: &str, stored: &str) -> Option<String> {
    (typed.trim_end() != stored.trim_end()).then(|| typed.to_owned())
}

/// Where a page's agent browsers keep their last addresses (encrypted
/// database, local): one per browser, in order.
#[cfg(any(windows, test))]
pub fn browser_key(owner: Owner) -> String {
    format!("browser.addresses.{owner}")
}

/// The addresses as stored.
#[cfg(any(windows, test))]
pub fn encode_addresses(addresses: &[String]) -> String {
    serde_json::to_string(addresses).unwrap_or_else(|_| "[]".into())
}

/// The addresses a page's browsers reopen on: web addresses only (the
/// agent browser's policy checks them again when they load), at most
/// `max`. A damaged value reads as none.
#[cfg(any(windows, test))]
pub fn decode_addresses(raw: &str, max: usize) -> Vec<String> {
    let list: Vec<String> = serde_json::from_str(raw).unwrap_or_default();
    list.into_iter()
        .filter(|a| a.starts_with("https://") || a.starts_with("http://"))
        .take(max)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{ProjectId, TopicId};

    #[test]
    fn focus_keys_round_trip_and_reject_anything_else() {
        let blocks = [
            Block::Title,
            Block::Folders,
            Block::State,
            Block::Description,
            Block::Questions,
            Block::Fold(Section::Timeline),
            Block::Fold(Section::Address),
            Block::Entry(EntryId(12)),
        ];
        for block in blocks {
            assert_eq!(focus_block(&focus_key(block)), Some(block), "{block:?}");
        }
        assert_eq!(focus_key(Block::Entry(EntryId(12))), "entry:12");
        for bad in [
            "",
            "fold:",
            "fold:whatever",
            "entry:",
            "entry:x",
            "Alexandre",
        ] {
            assert_eq!(focus_block(bad), None, "{bad}");
        }
    }

    #[test]
    fn pages_keep_route_keys_and_known_focus_only() {
        let mut stored = BTreeMap::new();
        stored.insert(
            "T-4".to_owned(),
            PageState {
                scroll: 320,
                focus: Some("entry:3".into()),
            },
        );
        stored.insert(
            "Alexandre's notes".to_owned(),
            PageState {
                scroll: 10,
                focus: None,
            },
        );
        stored.insert(
            "inbox:P-1".to_owned(),
            PageState {
                scroll: 5,
                focus: Some("a draft someone typed".into()),
            },
        );
        let read = read_pages(stored);
        assert_eq!(read.len(), 2);
        assert_eq!(read["T-4"].focus.as_deref(), Some("entry:3"));
        assert_eq!(read["inbox:P-1"].focus, None);
        let many: BTreeMap<String, PageState> = (0..100)
            .map(|i| (format!("T-{i}"), PageState::default()))
            .collect();
        assert_eq!(read_pages(many).len(), PAGES_MAX);
    }

    #[test]
    fn a_page_records_its_scroll_and_focus_and_closed_tabs_are_pruned() {
        let mut pages = BTreeMap::new();
        assert!(set_scroll(&mut pages, "T-4", 319.6));
        assert!(!set_scroll(&mut pages, "T-4", 320.2));
        assert!(set_focus(&mut pages, "T-4", Some(Block::State)));
        assert!(!set_focus(&mut pages, "T-4", Some(Block::State)));
        // A new page at the top: nothing to change, kept at 0.
        assert!(!set_scroll(&mut pages, "P-1", -5.));
        assert_eq!(pages["P-1"].scroll, 0);
        assert!(!set_scroll(&mut pages, "not a key!", 3.));
        assert_eq!(
            pages["T-4"],
            PageState {
                scroll: 320,
                focus: Some("state".into())
            }
        );
        prune(&mut pages, &["T-4".to_owned(), "settings".to_owned()]);
        assert_eq!(pages.keys().collect::<Vec<_>>(), vec!["T-4"]);
    }

    #[test]
    fn sessions_are_record_ids_once_each() {
        let read = read_sessions(vec![
            "T-4".into(),
            "P-1".into(),
            "T-4".into(),
            "settings".into(),
            "claude --resume x".into(),
        ]);
        assert_eq!(read, vec!["T-4".to_owned(), "P-1".to_owned()]);
        assert_eq!(
            parse_owner("E-3"),
            Some(Owner::Epic(acta_core::model::EpicId(3)))
        );
        assert_eq!(parse_owner("TE-3"), None);
        assert_eq!(parse_owner("3"), None);
        let many: Vec<String> = (1..40).map(|i| format!("T-{i}")).collect();
        assert_eq!(read_sessions(many).len(), SESSIONS_MAX);
    }

    #[test]
    fn browsers_keep_web_addresses_only() {
        let kept = vec![
            "https://example.com/a".to_owned(),
            "about:blank".to_owned(),
            "file:///C:/Users/Alexandre/secret.txt".to_owned(),
            "http://localhost:3000".to_owned(),
        ];
        let raw = encode_addresses(&kept);
        assert_eq!(
            decode_addresses(&raw, 4),
            vec![
                "https://example.com/a".to_owned(),
                "http://localhost:3000".to_owned()
            ]
        );
        assert_eq!(decode_addresses(&raw, 1).len(), 1);
        assert!(decode_addresses("not json", 4).is_empty());
        assert_eq!(
            browser_key(Owner::Topic(TopicId(4))),
            "browser.addresses.T-4"
        );
    }

    #[test]
    fn drafts_are_keyed_by_record_and_field_and_an_unchanged_text_is_none() {
        let topic = Owner::Topic(TopicId(42));
        assert_eq!(draft_key(topic, DraftField::State), "draft.T-42.state");
        assert_eq!(
            draft_field(topic, "draft.T-42.description"),
            Some(DraftField::Description)
        );
        assert_eq!(draft_field(topic, "draft.T-4.state"), None);
        assert_eq!(
            draft_field(Owner::Project(ProjectId(1)), "draft.T-42.state"),
            None
        );
        assert_eq!(draft_value("Same\n", "Same"), None);
        assert_eq!(
            draft_value("Alexandre's draft", "Stored").as_deref(),
            Some("Alexandre's draft")
        );
    }
}
