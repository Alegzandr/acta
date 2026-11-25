//! Section folds (docs/design/components-records.md 5, Section fold): which
//! sections of which page kind fold, their fixed keys in `preferences.json`
//! (`folds`, layout.md 4.7), Collapse all and Expand all, reveal, the unread
//! dot of a folded section and the scroll anchoring of a fold change.
//!
//! A fold is remembered per page kind and section, never per record, so the
//! stored keys are fixed strings (`topic.timeline`) and no record id reaches
//! the file. Pure: the record and contact pages draw from it.

/// The pages whose sections fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Topic,
    Epic,
    Project,
    Contact,
}

/// A section that folds, on the pages that have it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Section {
    Overview,
    Description,
    Questions,
    Knowledge,
    Timeline,
    Topics,
    /// A contact's "How to address them".
    Address,
    /// A contact's "Questions asked".
    Asked,
    /// A contact's "To report".
    Report,
}

impl Kind {
    /// The kind's foldable sections, in reading order (the audit table of
    /// components-records.md 5).
    pub fn sections(self) -> &'static [Section] {
        use Section::*;
        match self {
            Kind::Topic => &[Description, Questions, Knowledge, Timeline],
            Kind::Epic => &[Description, Questions, Knowledge, Topics],
            Kind::Project => &[Overview, Description, Questions, Knowledge, Topics],
            Kind::Contact => &[Address, Asked, Report],
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Kind::Topic => "topic",
            Kind::Epic => "epic",
            Kind::Project => "project",
            Kind::Contact => "contact",
        }
    }
}

impl Section {
    fn suffix(self) -> &'static str {
        match self {
            Section::Overview => "overview",
            Section::Description => "description",
            Section::Questions => "questions",
            Section::Knowledge => "knowledge",
            Section::Timeline => "timeline",
            Section::Topics => "topics",
            Section::Address => "address",
            Section::Asked => "asked",
            Section::Report => "report",
        }
    }
}

/// The stored key of `section` on a page of `kind` (`topic.timeline`), or
/// `None` when that page has no such foldable section. The epic's Needs
/// attention stays per record in `overview_open`, so it has no key here.
pub fn key(kind: Kind, section: Section) -> Option<String> {
    kind.sections()
        .contains(&section)
        .then(|| format!("{}.{}", kind.prefix(), section.suffix()))
}

/// Every key this build knows.
fn known() -> impl Iterator<Item = String> {
    [Kind::Topic, Kind::Epic, Kind::Project, Kind::Contact]
        .into_iter()
        .flat_map(|kind| kind.sections().iter().filter_map(move |s| key(kind, *s)))
}

/// The stored list as read: known keys only, once each, sorted. A key of a
/// later build or a hand-edited value is dropped, so nothing else is ever
/// written back.
pub fn read(stored: Vec<String>) -> Vec<String> {
    let known: Vec<String> = known().collect();
    let mut out: Vec<String> = stored.into_iter().filter(|k| known.contains(k)).collect();
    out.sort();
    out.dedup();
    out
}

/// Whether `section` is folded on pages of `kind`.
pub fn is_folded(folds: &[String], kind: Kind, section: Section) -> bool {
    key(kind, section).is_some_and(|k| folds.contains(&k))
}

/// Fold or unfold `section` on pages of `kind`; whether anything changed.
pub fn set(folds: &mut Vec<String>, kind: Kind, section: Section, folded: bool) -> bool {
    let Some(k) = key(kind, section) else {
        return false;
    };
    let had = folds.contains(&k);
    if folded == had {
        return false;
    }
    if folded {
        folds.push(k);
        folds.sort();
    } else {
        folds.retain(|f| *f != k);
    }
    true
}

/// "Collapse all sections": every key of the page kind.
pub fn collapse_all(folds: &mut Vec<String>, kind: Kind) {
    for section in kind.sections() {
        set(folds, kind, *section, true);
    }
}

/// "Expand all sections": the page kind's keys removed.
pub fn expand_all(folds: &mut Vec<String>, kind: Kind) {
    for section in kind.sections() {
        set(folds, kind, *section, false);
    }
}

/// Anything that takes the reader inside a folded section unfolds it, and
/// it stays unfolded as if the human had unfolded it (Reveal). Whether the
/// folds changed (and so must be stored).
pub fn reveal(folds: &mut Vec<String>, kind: Kind, section: Section) -> bool {
    set(folds, kind, section, false)
}

/// What another process wrote in a section, for its unread dot: each
/// item's id and the time it last changed, for the items Claude Code
/// changed last, in any order.
pub type Stamps = Vec<(i64, i64)>;

/// Whether a reload found a write inside a section: an item added or
/// changed since the stamps seen before. A removal is not a write to mark
/// (nothing is left to read), and the human's own edits are not in the
/// stamps. `None` before: the first load, which is never a write.
pub fn wrote_inside(before: Option<&Stamps>, after: &Stamps) -> bool {
    let Some(before) = before else {
        return false;
    };
    after.iter().any(|s| !before.contains(s))
}

/// The section that anchors Collapse all and Expand all: the one that holds
/// the top of the viewport, that is the last header at or above `top`, else
/// the first one. `headers` are the drawn headers with their y in the
/// page's content, in reading order.
pub fn anchor(headers: &[(Section, f32)], top: f32) -> Option<Section> {
    headers
        .iter()
        .rev()
        .find(|(_, y)| *y <= top)
        .or(headers.first())
        .map(|(s, _)| *s)
}

/// The page's scroll position (the content y at the top of the viewport)
/// after a fold change, so that the anchor header keeps its screen y: it
/// moved from `before` to `after` in the content. `None` after: the header
/// is gone and the page goes to its top. Clamped to what the page can
/// scroll.
pub fn anchored_top(top: f32, before: f32, after: Option<f32>, max: f32) -> f32 {
    match after {
        Some(after) => (top + after - before).clamp(0., max.max(0.)),
        None => 0.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn keys_are_fixed_per_page_kind_and_absent_where_a_page_does_not_fold() {
        assert_eq!(
            key(Kind::Topic, Section::Timeline).as_deref(),
            Some("topic.timeline")
        );
        assert_eq!(
            key(Kind::Contact, Section::Address).as_deref(),
            Some("contact.address")
        );
        assert_eq!(key(Kind::Topic, Section::Topics), None);
        assert_eq!(key(Kind::Epic, Section::Overview), None);
        assert_eq!(key(Kind::Epic, Section::Timeline), None);
        // The sixteen keys of components-records.md 5, Memory.
        let all: Vec<String> = known().collect();
        assert_eq!(all.len(), 16);
        for k in [
            "topic.description",
            "topic.questions",
            "topic.knowledge",
            "topic.timeline",
            "epic.description",
            "epic.questions",
            "epic.knowledge",
            "epic.topics",
            "project.overview",
            "project.description",
            "project.questions",
            "project.knowledge",
            "project.topics",
            "contact.address",
            "contact.asked",
            "contact.report",
        ] {
            assert!(all.contains(&k.to_string()), "{k}");
        }
    }

    #[test]
    fn unknown_and_repeated_keys_are_dropped_on_read() {
        let read = read(keys(&[
            "topic.timeline",
            "topic.timeline",
            "T-42",
            "topic.whatever",
            "Alexandre's notes",
            "epic.topics",
        ]));
        assert_eq!(read, keys(&["epic.topics", "topic.timeline"]));
    }

    #[test]
    fn a_fold_is_per_page_kind_not_per_record() {
        let mut folds = Vec::new();
        assert!(set(&mut folds, Kind::Topic, Section::Timeline, true));
        assert!(!set(&mut folds, Kind::Topic, Section::Timeline, true));
        assert!(is_folded(&folds, Kind::Topic, Section::Timeline));
        assert!(!is_folded(&folds, Kind::Epic, Section::Description));
        assert!(!set(&mut folds, Kind::Epic, Section::Timeline, true));
        assert!(set(&mut folds, Kind::Topic, Section::Timeline, false));
        assert!(folds.is_empty());
    }

    #[test]
    fn collapse_all_writes_every_key_of_the_kind_and_expand_all_removes_them() {
        let mut folds = keys(&["contact.asked"]);
        collapse_all(&mut folds, Kind::Project);
        assert_eq!(
            folds,
            keys(&[
                "contact.asked",
                "project.description",
                "project.knowledge",
                "project.overview",
                "project.questions",
                "project.topics",
            ])
        );
        expand_all(&mut folds, Kind::Project);
        assert_eq!(folds, keys(&["contact.asked"]));
    }

    #[test]
    fn reveal_unfolds_and_reports_a_change_to_store() {
        let mut folds = Vec::new();
        collapse_all(&mut folds, Kind::Topic);
        assert!(reveal(&mut folds, Kind::Topic, Section::Questions));
        assert!(!is_folded(&folds, Kind::Topic, Section::Questions));
        assert!(is_folded(&folds, Kind::Topic, Section::Timeline));
        // Already unfolded: nothing to store.
        assert!(!reveal(&mut folds, Kind::Topic, Section::Questions));
    }

    #[test]
    fn folds_round_trip_through_the_preferences_file() {
        let mut prefs = crate::preferences::Preferences::default();
        collapse_all(&mut prefs.folds, Kind::Epic);
        set(&mut prefs.folds, Kind::Topic, Section::Timeline, true);
        let text = crate::preferences::serialize(&prefs);
        let back = crate::preferences::parse(&text).unwrap();
        assert_eq!(back.folds, prefs.folds);
        assert!(is_folded(&back.folds, Kind::Epic, Section::Topics));
        // Unknown keys in the file are ignored, the rest kept.
        let text = text.replace("\"topic.timeline\"", "\"topic.timeline\", \"T-1\", \"x.y\"");
        let back = crate::preferences::parse(&text).unwrap();
        assert_eq!(back.folds, prefs.folds);
    }

    #[test]
    fn a_write_inside_a_section_is_seen_on_reload_but_not_on_the_first_load() {
        let before: Stamps = vec![(1, 100), (2, 200)];
        assert!(!wrote_inside(None, &before));
        assert!(!wrote_inside(Some(&before), &vec![(2, 200), (1, 100)]));
        // Another process added an entry, or edited one.
        assert!(wrote_inside(
            Some(&before),
            &vec![(1, 100), (2, 200), (3, 300)]
        ));
        assert!(wrote_inside(Some(&before), &vec![(1, 100), (2, 250)]));
        // A removal, or the human taking an entry over, is no write to mark.
        assert!(!wrote_inside(Some(&before), &vec![(1, 100)]));
    }

    #[test]
    fn anchoring_keeps_the_header_y_or_goes_to_the_top() {
        use Section::*;
        let headers = [(Description, 300.), (Questions, 600.), (Timeline, 900.)];
        assert_eq!(anchor(&headers, 0.), Some(Description));
        assert_eq!(anchor(&headers, 650.), Some(Questions));
        assert_eq!(anchor(&headers, 2000.), Some(Timeline));
        assert_eq!(anchor(&[], 10.), None);
        // Collapse all from 650: Questions moved from 600 to 360, so the
        // page scrolls by the same 240 and its header stays put.
        assert_eq!(anchored_top(650., 600., Some(360.), 5000.), 410.);
        // A toggle whose header did not move keeps the scroll.
        assert_eq!(anchored_top(650., 600., Some(600.), 5000.), 650.);
        // Clamped to what the page can scroll; the header gone: the top.
        assert_eq!(anchored_top(650., 600., Some(100.), 5000.), 150.);
        assert_eq!(anchored_top(650., 600., Some(900.), 700.), 700.);
        assert_eq!(anchored_top(650., 600., None, 700.), 0.);
    }
}
