//! What the contact picker, the Contacts page and the contact page show
//! (docs/design/components-contacts.md 5), computed from the store's rows
//! with no GPUI: the picker's groups and ranking, the Contacts page's
//! groups, the levels a contact can move to with the places that refuse a
//! move down, possible duplicates and the words of a lift.

use acta_core::model::{
    Contact, ContactId, ContactLevel, ContactUse, EpicId, ProjectId, ProjectTree,
};
use acta_core::person::person_key;
use acta_core::store::ContactSummary;

use crate::i18n::{Arg, Key, Locale, text, tr};

/// The picker shows at most 8 contact rows (components-contacts.md 5).
pub const PICKER_ROWS: usize = 8;
/// A contact's name, at most 80 characters typed.
pub const NAME_MAX: usize = 80;
/// The address note's counter shows over 400; typing stops at 500.
pub const ADDRESS_COUNTER: usize = 400;
pub const ADDRESS_MAX: usize = 500;
/// At most three places in a refusal, then "+4 more".
const REFUSAL_PLACES: usize = 3;

/// "Head of legal · Acme": job title and company, whichever exist.
pub fn details(contact: &Contact) -> String {
    [contact.job_title.as_deref(), contact.company.as_deref()]
        .into_iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// "All projects", "Project Acta", "Epic Terminal" for a level, with the
/// names from the loaded trees.
pub fn level_name(locale: Locale, level: ContactLevel, projects: &[ProjectTree]) -> String {
    match level {
        ContactLevel::All => text(locale, Key::ContactLevelAll),
        ContactLevel::Project(p) => {
            let name = project_title(projects, p);
            tr(locale, Key::LevelProject, &[("name", Arg::Text(&name))])
        }
        ContactLevel::Epic(p, e) => {
            let name = epic_title(projects, p, e);
            tr(locale, Key::LevelEpic, &[("name", Arg::Text(&name))])
        }
    }
}

pub fn project_title(projects: &[ProjectTree], id: ProjectId) -> String {
    projects
        .iter()
        .find(|t| t.project.id == id)
        .map(|t| t.project.title.clone())
        .unwrap_or_else(|| id.to_string())
}

pub fn epic_title(projects: &[ProjectTree], project: ProjectId, id: EpicId) -> String {
    projects
        .iter()
        .find(|t| t.project.id == project)
        .and_then(|t| t.epics.iter().find(|(e, _)| e.id == id))
        .map(|(e, _)| e.title.clone())
        .unwrap_or_else(|| id.to_string())
}

/// Whether one level sees the other (one is at or above the other): two
/// contacts named alike at such levels are possible duplicates.
pub fn overlaps(a: ContactLevel, b: ContactLevel) -> bool {
    contains(a, b) || contains(b, a)
}

fn contains(outer: ContactLevel, inner: ContactLevel) -> bool {
    match (outer, inner) {
        (ContactLevel::All, _) => true,
        (ContactLevel::Project(p), ContactLevel::Project(q) | ContactLevel::Epic(q, _)) => p == q,
        (ContactLevel::Epic(_, e), ContactLevel::Epic(_, f)) => e == f,
        _ => false,
    }
}

// ---- the picker ----

/// One contact row of the picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerRow {
    pub id: ContactId,
    pub name: String,
    pub details: String,
    /// The characters of the name the typed text matched (weight 600).
    pub matched: Vec<usize>,
}

/// The rows of one level, nearest level first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerGroup {
    pub level: ContactLevel,
    pub rows: Vec<PickerRow>,
}

/// What the picker lists for the typed text: its groups, and the "New
/// contact" row (the text as typed) whenever there is text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PickerList {
    pub groups: Vec<PickerGroup>,
    pub new_row: Option<String>,
}

/// A row the keyboard can highlight: a contact, or "New contact".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Contact(ContactId, String),
    New(String),
}

impl PickerList {
    /// Every row in order, headers left out: what `Up` / `Down` move over.
    pub fn flat(&self) -> Vec<Pick> {
        let mut out: Vec<Pick> = self
            .groups
            .iter()
            .flat_map(|g| g.rows.iter().map(|r| Pick::Contact(r.id, r.name.clone())))
            .collect();
        if let Some(name) = &self.new_row {
            out.push(Pick::New(name.clone()));
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.new_row.is_none()
    }

    /// Whether group headers show: not with one group only.
    pub fn headers(&self) -> bool {
        self.groups.len() > 1
    }
}

/// How well a contact's key matches the typed key: 0 equal, 1 a word
/// starts with it, 2 it contains it; `None` no match.
fn rank(key: &str, typed: &str) -> Option<u8> {
    if key == typed {
        Some(0)
    } else if key.split(' ').any(|w| w.starts_with(typed)) || key.starts_with(typed) {
        Some(1)
    } else if key.contains(typed) {
        Some(2)
    } else {
        None
    }
}

/// The characters of `name` covered by the first match of `typed` (a
/// key), compared character by character on their keys.
pub fn matched_chars(name: &str, typed: &str) -> Vec<usize> {
    if typed.is_empty() {
        return Vec::new();
    }
    let folded: Vec<String> = name.chars().map(|c| person_key(&c.to_string())).collect();
    let want: Vec<char> = typed.chars().collect();
    'start: for start in 0..folded.len() {
        let mut at = 0;
        let mut hit = Vec::new();
        for (i, f) in folded.iter().enumerate().skip(start) {
            if at == want.len() {
                break;
            }
            // A space folds to nothing: it matches a typed space.
            let fc: Vec<char> = if f.is_empty() {
                vec![' ']
            } else {
                f.chars().collect()
            };
            for c in fc {
                if at < want.len() && want[at] == c {
                    at += 1;
                } else {
                    continue 'start;
                }
            }
            hit.push(i);
        }
        if at == want.len() {
            return hit;
        }
    }
    Vec::new()
}

/// The picker's list for `typed` over the contacts visible from a record
/// (`visible`, as the store returns them: nearest level first, then open
/// items, then most recently named). Inside each group: equal key first,
/// then a word starting with the text, then containing it; ties keep the
/// store's order. With no text: each group's contacts in the store's
/// order, 8 rows in all, and no "New contact" row.
pub fn picker_list(visible: &[ContactSummary], typed: &str) -> PickerList {
    let typed_key = person_key(typed);
    let mut scored: Vec<(u8, usize, &ContactSummary)> = Vec::new();
    for (order, c) in visible.iter().enumerate() {
        if typed_key.is_empty() {
            scored.push((0, order, c));
        } else if let Some(r) = rank(&person_key(&c.contact.name), &typed_key) {
            scored.push((r, order, c));
        }
    }
    // Groups nearest first; inside, by rank then the store's order.
    scored.sort_by_key(|(r, order, c)| (c.contact.level.width(), *r, *order));
    let mut groups: Vec<PickerGroup> = Vec::new();
    for (_, _, c) in scored.into_iter().take(PICKER_ROWS) {
        let row = PickerRow {
            id: c.contact.id,
            name: c.contact.name.clone(),
            details: details(&c.contact),
            matched: matched_chars(&c.contact.name, &typed_key),
        };
        match groups.last_mut() {
            Some(g) if g.level == c.contact.level => g.rows.push(row),
            _ => groups.push(PickerGroup {
                level: c.contact.level,
                rows: vec![row],
            }),
        }
    }
    let shown = typed.split_whitespace().collect::<Vec<_>>().join(" ");
    PickerList {
        groups,
        new_row: (!shown.is_empty()).then_some(shown),
    }
}

/// The inbox's Report to filter searches its contacts when there are more
/// than this many (components-contacts.md 5, Inbox filter).
pub const FILTER_SEARCH_OVER: usize = 8;

/// The inbox filter's rows for `typed`, as indexes into `names` (the
/// contacts of the listed notes, first seen first): the picker's ranking
/// (equal key, then a word starting with it, then containing it; ties keep
/// the given order) and no "New contact" row, since a filter does not
/// create. No text: every row in order.
pub fn filter_rows(names: &[&str], typed: &str) -> Vec<usize> {
    let typed_key = person_key(typed);
    let mut scored: Vec<(u8, usize)> = names
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            if typed_key.is_empty() {
                Some((0, i))
            } else {
                rank(&person_key(name), &typed_key).map(|r| (r, i))
            }
        })
        .collect();
    scored.sort();
    scored.into_iter().map(|(_, i)| i).collect()
}

/// The level a contact created from a record's picker gets: the record's
/// project (components-contacts.md 5, Levels).
pub fn default_level(project: ProjectId) -> ContactLevel {
    ContactLevel::Project(project)
}

// ---- the Contacts page ----

/// The Contacts page's filter (components-contacts.md 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageFilter {
    /// What the active project sees.
    #[default]
    ThisProject,
    All,
}

/// One group of the Contacts page: a level and its contacts by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageGroup {
    pub level: ContactLevel,
    pub contacts: Vec<ContactSummary>,
}

/// Whether `query` (a key) finds a contact by name, job title or company.
fn page_match(c: &Contact, query: &str) -> bool {
    query.is_empty()
        || [
            Some(c.name.as_str()),
            c.job_title.as_deref(),
            c.company.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|v| person_key(v).contains(query))
}

/// The Contacts page's groups in tree order: All projects, then each
/// project and its epics; inside a group, by name. `active` is the
/// project "This project" keeps (with All projects and its epics).
pub fn page_groups(
    all: &[ContactSummary],
    projects: &[ProjectTree],
    filter: PageFilter,
    active: Option<ProjectId>,
    query: &str,
) -> Vec<PageGroup> {
    let query = person_key(query);
    let mut levels = vec![ContactLevel::All];
    for tree in projects {
        let p = tree.project.id;
        if filter == PageFilter::ThisProject && Some(p) != active {
            continue;
        }
        levels.push(ContactLevel::Project(p));
        levels.extend(tree.epics.iter().map(|(e, _)| ContactLevel::Epic(p, e.id)));
    }
    levels
        .into_iter()
        .filter_map(|level| {
            let mut contacts: Vec<ContactSummary> = all
                .iter()
                .filter(|c| c.contact.level == level && page_match(&c.contact, &query))
                .cloned()
                .collect();
            if contacts.is_empty() {
                return None;
            }
            contacts.sort_by(|a, b| {
                person_key(&a.contact.name)
                    .cmp(&person_key(&b.contact.name))
                    .then(a.contact.id.0.cmp(&b.contact.id.0))
            });
            Some(PageGroup { level, contacts })
        })
        .collect()
}

// ---- moving ----

/// A level a contact can move to, as the palette lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveChoice {
    pub level: ContactLevel,
    /// "All projects", "Acta", "Acta / Terminal".
    pub title: String,
    pub current: bool,
    /// The uses a move there would leave out of scope: refused when any.
    pub outside: Vec<ContactUse>,
}

/// "All projects", then each project followed by its epics, with what
/// would refuse each (components-contacts.md 5, Moving a contact).
pub fn move_choices(
    locale: Locale,
    contact: &Contact,
    uses: &[ContactUse],
    projects: &[ProjectTree],
) -> Vec<MoveChoice> {
    let mut levels = vec![(ContactLevel::All, text(locale, Key::ContactLevelAll))];
    for tree in projects {
        let p = tree.project.id;
        levels.push((ContactLevel::Project(p), tree.project.title.clone()));
        for (e, _) in &tree.epics {
            levels.push((
                ContactLevel::Epic(p, e.id),
                format!("{} / {}", tree.project.title, e.title),
            ));
        }
    }
    levels
        .into_iter()
        .map(|(level, title)| MoveChoice {
            level,
            title,
            current: level == contact.level,
            outside: uses
                .iter()
                .filter(|u| !acta_core::model::visible(level, u.place))
                .cloned()
                .collect(),
        })
        .collect()
}

/// The refusal under the palette's list: "Named outside Epic Terminal:
/// T-51 Encrypted backups (Report to), Q-9 Who signs? (Asked). Change
/// those first."
pub fn refusal(locale: Locale, level_name: &str, outside: &[ContactUse]) -> String {
    let mut places: Vec<String> = outside
        .iter()
        .take(REFUSAL_PLACES)
        .map(|u| {
            let field = text(
                locale,
                if u.field == "asked" {
                    Key::QuestionFieldAsked
                } else {
                    Key::PropReportTo
                },
            );
            tr(
                locale,
                Key::ContactMovePlace,
                &[
                    ("id", Arg::Text(&u.record.to_string())),
                    ("title", Arg::Text(&short_title(&u.title))),
                    ("field", Arg::Text(&field)),
                ],
            )
        })
        .collect();
    if outside.len() > REFUSAL_PLACES {
        places.push(tr(
            locale,
            Key::ContactMoveMore,
            &[("n", Arg::Count((outside.len() - REFUSAL_PLACES) as u64))],
        ));
    }
    tr(
        locale,
        Key::ContactMoveRefused,
        &[
            ("level", Arg::Text(level_name)),
            ("places", Arg::Text(&places.join(", "))),
        ],
    )
}

/// A title or a question in a refusal: at most 40 characters.
fn short_title(title: &str) -> String {
    const MAX: usize = 40;
    if title.chars().count() <= MAX {
        return title.to_owned();
    }
    let cut: String = title.chars().take(MAX - 1).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// The toast after a move lifted a contact: "Marie is now a contact of all
/// projects, so Encrypted backups still sees them."
pub fn lifted(
    locale: Locale,
    name: &str,
    to: ContactLevel,
    record: &str,
    projects: &[ProjectTree],
) -> String {
    match to {
        ContactLevel::Project(p) => {
            let project = project_title(projects, p);
            tr(
                locale,
                Key::ContactLiftedProject,
                &[
                    ("name", Arg::Text(name)),
                    ("project", Arg::Text(&project)),
                    ("record", Arg::Text(record)),
                ],
            )
        }
        _ => tr(
            locale,
            Key::ContactLifted,
            &[("name", Arg::Text(name)), ("record", Arg::Text(record))],
        ),
    }
}

/// Undoing a tree drag that lifted a contact: the topic goes back, the
/// contact stays at its wider level, and the toast says so.
pub fn lift_kept(locale: Locale, name: &str, at: ContactLevel, projects: &[ProjectTree]) -> String {
    match at {
        ContactLevel::Project(p) => tr(
            locale,
            Key::ContactLiftKeptProject,
            &[
                ("name", Arg::Text(name)),
                ("project", Arg::Text(&project_title(projects, p))),
            ],
        ),
        _ => tr(locale, Key::ContactLiftKept, &[("name", Arg::Text(name))]),
    }
}

/// The other live contacts with the same key as `contact` whose level
/// overlaps its own (one sees the other), not kept apart by "Keep both":
/// the duplicate notice (components-contacts.md 5).
pub fn duplicates<'a>(
    contact: &Contact,
    all: &'a [ContactSummary],
    distinct: &[(ContactId, ContactId)],
) -> Vec<&'a Contact> {
    let key = person_key(&contact.name);
    all.iter()
        .map(|c| &c.contact)
        .filter(|c| c.id != contact.id)
        .filter(|c| person_key(&c.name) == key)
        .filter(|c| overlaps(c.level, contact.level))
        .filter(|c| {
            !distinct
                .iter()
                .any(|(a, b)| (*a == c.id && *b == contact.id) || (*b == c.id && *a == contact.id))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{Author, Place, Record, Stamp, TopicId};

    fn stamp() -> Stamp {
        Stamp {
            uuid: String::new(),
            created_at: 0,
            updated_at: 0,
            updated_by: Author::Human,
            deleted_at: None,
            deleted_by: None,
        }
    }

    fn c(id: i64, name: &str, level: ContactLevel, job: Option<&str>) -> ContactSummary {
        ContactSummary {
            contact: Contact {
                id: ContactId(id),
                name: name.into(),
                job_title: job.map(Into::into),
                company: None,
                address_note: None,
                level,
                created_by: Author::Human,
                merged_into: None,
                stamp: stamp(),
            },
            open: 0,
            last_named: 0,
        }
    }

    const P: ProjectId = ProjectId(1);
    const E: EpicId = EpicId(3);

    /// The contacts a topic of epic Terminal of Acta sees, as the store
    /// lists them: nearest first.
    fn visible() -> Vec<ContactSummary> {
        vec![
            c(
                5,
                "Sami",
                ContactLevel::Epic(P, E),
                Some("Windows packaging"),
            ),
            c(1, "Marius", ContactLevel::Project(P), Some("Accounting")),
            c(2, "Marie", ContactLevel::Project(P), Some("Head of legal")),
            c(3, "Legal team", ContactLevel::Project(P), None),
            c(4, "Marie", ContactLevel::All, Some("Head of legal")),
        ]
    }

    fn names(list: &PickerList) -> Vec<(ContactLevel, Vec<&str>)> {
        list.groups
            .iter()
            .map(|g| (g.level, g.rows.iter().map(|r| r.name.as_str()).collect()))
            .collect()
    }

    #[test]
    fn the_picker_groups_nearest_first_and_ranks_inside_each_group() {
        let list = picker_list(&visible(), "mar");
        // No Epic Terminal group: nothing of it matches.
        assert_eq!(
            names(&list),
            [
                (ContactLevel::Project(P), vec!["Marius", "Marie"]),
                (ContactLevel::All, vec!["Marie"]),
            ]
        );
        assert_eq!(list.new_row.as_deref(), Some("mar"));
        assert!(list.headers());
        // An equal key comes first in its group.
        let list = picker_list(&visible(), "MARIE");
        assert_eq!(list.groups[0].rows[0].name, "Marie");
        // A word starting with the text beats containing it.
        let list = picker_list(&visible(), "team");
        assert_eq!(list.groups[0].rows[0].name, "Legal team");
        assert_eq!(list.groups[0].rows[0].matched, vec![6, 7, 8, 9]);
        assert!(!list.headers(), "one group: no header");
    }

    #[test]
    fn an_unknown_name_leaves_only_the_new_contact_row() {
        let list = picker_list(&visible(), "  Paul ");
        assert!(list.groups.is_empty());
        assert_eq!(list.flat(), [Pick::New("Paul".into())]);
    }

    #[test]
    fn with_no_text_the_picker_lists_eight_rows_and_no_new_row() {
        let many: Vec<ContactSummary> = (0..12)
            .map(|i| c(i, &format!("C{i}"), ContactLevel::Project(P), None))
            .collect();
        let list = picker_list(&many, "");
        assert_eq!(list.flat().len(), PICKER_ROWS);
        assert_eq!(list.new_row, None);
        // The store's order is kept (open items, then most recently named).
        assert_eq!(list.groups[0].rows[0].name, "C0");
    }

    #[test]
    fn matched_characters_fold_case_and_accents() {
        assert_eq!(matched_chars("Hélène", "hel"), vec![0, 1, 2]);
        assert_eq!(matched_chars("Marie Dubois", "dub"), vec![6, 7, 8]);
        assert_eq!(matched_chars("Marie", "x"), Vec::<usize>::new());
    }

    #[test]
    fn details_join_what_exists() {
        let mut m = visible()[2].contact.clone();
        assert_eq!(details(&m), "Head of legal");
        m.company = Some("Acme".into());
        assert_eq!(details(&m), "Head of legal \u{b7} Acme");
        m.job_title = None;
        assert_eq!(details(&m), "Acme");
    }

    #[test]
    fn levels_overlap_only_along_one_branch() {
        let p2 = ProjectId(2);
        assert!(overlaps(ContactLevel::All, ContactLevel::Epic(P, E)));
        assert!(overlaps(ContactLevel::Project(P), ContactLevel::Epic(P, E)));
        assert!(overlaps(ContactLevel::Epic(P, E), ContactLevel::Epic(P, E)));
        assert!(!overlaps(
            ContactLevel::Project(P),
            ContactLevel::Project(p2)
        ));
        assert!(!overlaps(
            ContactLevel::Epic(P, E),
            ContactLevel::Epic(P, EpicId(4))
        ));
    }

    #[test]
    fn duplicates_are_same_key_at_overlapping_levels_not_kept_apart() {
        let mut all = visible();
        all.push(c(9, "marie", ContactLevel::Project(ProjectId(2)), None));
        let marie = all[1 + 1].contact.clone();
        let dupes = duplicates(&marie, &all, &[]);
        assert_eq!(
            dupes.iter().map(|c| c.id).collect::<Vec<_>>(),
            [ContactId(4)]
        );
        let kept = duplicates(&marie, &all, &[(ContactId(4), ContactId(2))]);
        assert!(kept.is_empty(), "Keep both removes the notice");
    }

    fn uses() -> Vec<ContactUse> {
        let place = |epic| Place { project: P, epic };
        vec![
            ContactUse {
                record: Record::Topic(TopicId(51)),
                place: place(None),
                title: "Encrypted backups".into(),
                field: "report_to",
            },
            ContactUse {
                record: Record::Question(acta_core::model::QuestionId(9)),
                place: place(Some(E)),
                title: "Who signs the export policy?".into(),
                field: "asked",
            },
        ]
    }

    #[test]
    fn moving_down_is_refused_with_the_places_named_outside() {
        let marie = visible()[2].contact.clone();
        let projects = Vec::new();
        let choices = move_choices(Locale::En, &marie, &uses(), &projects);
        assert_eq!(choices[0].level, ContactLevel::All);
        assert!(choices[0].outside.is_empty(), "up is never refused");
        let text = refusal(Locale::En, "Epic Terminal", &uses()[..1]);
        assert_eq!(
            text,
            "Named outside Epic Terminal: T-51 Encrypted backups (Report to). Change those first."
        );
        let five: Vec<ContactUse> = uses().into_iter().cycle().take(5).collect();
        let text = refusal(Locale::En, "Epic Terminal", &five);
        assert!(text.contains("+2 more"), "{text}");
        assert!(
            text.contains("Q-9 Who signs the export policy? (Asked)"),
            "{text}"
        );
    }

    #[test]
    fn the_contacts_page_groups_in_tree_order_and_sorts_by_name() {
        let all = visible();
        let groups = page_groups(&all, &[], PageFilter::All, None, "");
        // No trees loaded: only All projects is a known level.
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].contacts[0].contact.name, "Marie");
        let found = page_groups(&all, &[], PageFilter::All, None, "legal");
        // "Head of legal" is a job title: found by details too.
        assert_eq!(found[0].contacts.len(), 1);
    }

    #[test]
    fn a_lift_names_the_project_when_it_stops_there() {
        let to_all = lifted(Locale::En, "Marie", ContactLevel::All, "Backups", &[]);
        assert_eq!(
            to_all,
            "Marie is now a contact of all projects, so Backups still sees them."
        );
        let to_p = lifted(
            Locale::Fr,
            "Marie",
            ContactLevel::Project(P),
            "Backups",
            &[],
        );
        assert!(to_p.ends_with(": Marie."), "{to_p}");
    }

    #[test]
    fn undoing_a_drag_says_the_lifted_contact_stays() {
        assert_eq!(
            lift_kept(Locale::En, "Marie", ContactLevel::All, &[]),
            "Marie stays a contact of all projects."
        );
        assert!(lift_kept(Locale::Fr, "Marie", ContactLevel::All, &[]).ends_with("Marie."));
    }

    #[test]
    fn the_inbox_filter_ranks_as_the_picker_and_never_creates() {
        let names = ["Marie Dubois", "Alexandre", "Anne-Marie", "Paul", "marie"];
        assert_eq!(filter_rows(&names, ""), vec![0, 1, 2, 3, 4]);
        // Equal key first, then a word starting with it, then containing.
        assert_eq!(filter_rows(&names, "marie"), vec![4, 0, 2]);
        assert_eq!(filter_rows(&names, "  MARIE "), vec![4, 0, 2]);
        assert_eq!(filter_rows(&names, "zz"), Vec::<usize>::new());
        assert_eq!(filter_rows(&names, "alex"), vec![1]);
    }
}
