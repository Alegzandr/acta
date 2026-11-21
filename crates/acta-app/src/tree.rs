//! The sidebar tree as rows (docs/design/layout.md 4.2, components.md 5 list /
//! tree row): every project; under an expanded project its Inbox and
//! Knowledge rows, the topics outside any epic, then the epics with their
//! topics. Closed topics collapse under a "3 closed" row at the end of their
//! group (color.md 2.3). Pure, so the order and the expansion rules are
//! tested without a window.

use std::collections::HashSet;

use acta_core::model::{EpicId, ProjectId, ProjectTree, Status, TopicId, TopicSummary};

use crate::route::Route;

/// The group a "3 closed" row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// The topics of a project outside any epic.
    Loose(ProjectId),
    Epic(EpicId),
}

impl Group {
    pub fn key(self) -> String {
        match self {
            Group::Loose(id) => format!("closed:{id}"),
            Group::Epic(id) => format!("closed:{id}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Project {
        id: ProjectId,
        title: String,
        expanded: bool,
    },
    Inbox {
        project: ProjectId,
        count: i64,
    },
    Knowledge {
        project: ProjectId,
        count: i64,
    },
    Epic {
        id: EpicId,
        project: ProjectId,
        title: String,
        expanded: bool,
    },
    Topic {
        id: TopicId,
        title: String,
        status: Status,
        depth: u8,
    },
    Closed {
        group: Group,
        count: usize,
        shown: bool,
        depth: u8,
    },
}

impl Row {
    /// Indentation level: project 0, epic and loose topic 1, topic in an
    /// epic 2 (16 per level).
    pub fn depth(&self) -> u8 {
        match self {
            Row::Project { .. } => 0,
            Row::Inbox { .. } | Row::Knowledge { .. } | Row::Epic { .. } => 1,
            Row::Topic { depth, .. } | Row::Closed { depth, .. } => *depth,
        }
    }

    /// The page the row opens, if it opens one.
    pub fn route(&self) -> Option<Route> {
        match self {
            Row::Project { id, .. } => Some(Route::Project(*id)),
            Row::Inbox { project, .. } => Some(Route::Inbox(*project)),
            Row::Knowledge { project, .. } => Some(Route::Knowledge(*project)),
            Row::Epic { id, .. } => Some(Route::Epic(*id)),
            Row::Topic { id, .. } => Some(Route::Topic(*id)),
            Row::Closed { .. } => None,
        }
    }

    /// The key of the row's expansion state, if it folds.
    pub fn fold_key(&self) -> Option<String> {
        match self {
            Row::Project { id, .. } => Some(id.to_string()),
            Row::Epic { id, .. } => Some(id.to_string()),
            Row::Closed { group, .. } => Some(group.key()),
            _ => None,
        }
    }

    pub fn is_expanded(&self) -> Option<bool> {
        match self {
            Row::Project { expanded, .. } | Row::Epic { expanded, .. } => Some(*expanded),
            Row::Closed { shown, .. } => Some(*shown),
            _ => None,
        }
    }

    /// What the row stands for, whatever its place: the keyboard cursor
    /// stays on it when rows move (interaction.md 6.1).
    pub fn key(&self) -> RowKey {
        match (self, self.route()) {
            (_, Some(route)) => RowKey::Route(route),
            (Row::Closed { group, .. }, None) => RowKey::Closed(*group),
            (_, None) => unreachable!("only the closed row opens no page"),
        }
    }

    /// The title type-ahead matches (records only).
    pub fn title(&self) -> Option<&str> {
        match self {
            Row::Project { title, .. } | Row::Epic { title, .. } | Row::Topic { title, .. } => {
                Some(title)
            }
            _ => None,
        }
    }
}

/// The identity of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKey {
    Route(Route),
    Closed(Group),
}

/// The keyboard cursor of the tree: the row it is on, and the index it
/// had, for when that row is gone (deleted, folded away).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    key: Option<RowKey>,
    index: usize,
}

impl Cursor {
    /// The cursor's row in `rows`: the same row wherever it moved, else
    /// the row now at its last index.
    pub fn index(&self, rows: &[Row]) -> usize {
        self.key
            .and_then(|key| rows.iter().position(|r| r.key() == key))
            .unwrap_or(self.index)
            .min(rows.len().saturating_sub(1))
    }

    /// Put the cursor on row `at`.
    pub fn set(&mut self, rows: &[Row], at: usize) {
        self.index = at.min(rows.len().saturating_sub(1));
        self.key = rows.get(self.index).map(Row::key);
    }

    /// Put the cursor on the row of `route` if the tree shows one.
    pub fn put(&mut self, rows: &[Row], route: Route) -> bool {
        match rows.iter().position(|r| r.route() == Some(route)) {
            Some(at) => {
                self.set(rows, at);
                true
            }
            None => false,
        }
    }
}

/// Type-ahead (interaction.md 6.1): the first row from `from` on, wrapping,
/// whose title starts with `query`, ignoring case.
pub fn type_ahead(rows: &[Row], from: usize, query: &str) -> Option<usize> {
    let query = query.to_lowercase();
    if query.is_empty() || rows.is_empty() {
        return None;
    }
    let n = rows.len();
    (0..n).map(|i| (from + i) % n).find(|&i| {
        rows[i]
            .title()
            .is_some_and(|t| t.to_lowercase().starts_with(&query))
    })
}

/// A printable key pressed in the tree (no modifier but Shift).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Letter {
    Up,
    Down,
    /// `S`: the status menu.
    Status,
    /// `E`: rename.
    Rename,
    /// `M`: move to epic.
    Move,
    /// `Space`: open in the active tab, focus stays in the tree (6.2).
    Preview,
    /// Type-ahead with the buffer so far.
    Find(String),
}

/// What `ch` does, given the type-ahead buffer typed in the last second:
/// while one is going every key extends it, so titles with a `j` or a
/// space can be typed; otherwise the letter keys of 6.1 act.
pub fn letter(buffer: &str, ch: char) -> Letter {
    if !buffer.is_empty() {
        return Letter::Find(format!("{buffer}{ch}"));
    }
    match ch.to_ascii_lowercase() {
        'j' => Letter::Down,
        'k' => Letter::Up,
        's' => Letter::Status,
        'e' => Letter::Rename,
        'm' => Letter::Move,
        ' ' => Letter::Preview,
        _ => Letter::Find(ch.to_string()),
    }
}

/// What the tree's menus offer (components.md 5, Context menu).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Open,
    OpenInNewTab,
    Rename,
    /// "Set status", opening the status menu.
    StatusMenu,
    /// "Move to epic", opening the epic menu.
    MoveMenu,
    Status(Status),
    /// Move the topic to this epic (`None`: out of any epic).
    MoveTo(Option<EpicId>, String),
    Delete,
}

/// Which menu of a row is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    /// `Shift+F10`, the menu key, a right click.
    Row,
    /// `S` on a topic.
    Status,
    /// `M` on a topic.
    Epics,
}

/// The items of `kind` for `row`; empty when the row has no such menu.
/// `no_epic` is the label of the "No epic" item.
pub fn menu_items(
    projects: &[ProjectTree],
    row: &Row,
    kind: MenuKind,
    no_epic: &str,
) -> Vec<MenuItem> {
    match (kind, row) {
        (MenuKind::Row, Row::Topic { .. }) => vec![
            MenuItem::Open,
            MenuItem::OpenInNewTab,
            MenuItem::Rename,
            MenuItem::StatusMenu,
            MenuItem::MoveMenu,
            MenuItem::Delete,
        ],
        (MenuKind::Row, Row::Project { .. } | Row::Epic { .. }) => vec![
            MenuItem::Open,
            MenuItem::OpenInNewTab,
            MenuItem::Rename,
            MenuItem::Delete,
        ],
        (MenuKind::Row, Row::Inbox { .. } | Row::Knowledge { .. }) => {
            vec![MenuItem::Open, MenuItem::OpenInNewTab]
        }
        (MenuKind::Status, Row::Topic { .. }) => {
            Status::ALL.into_iter().map(MenuItem::Status).collect()
        }
        (MenuKind::Epics, Row::Topic { id, .. }) => {
            let Some(at) = locate_topic(projects, *id) else {
                return Vec::new();
            };
            let current = at.epic.map(|e| e.0);
            let project = at.project.map(|p| p.0);
            let Some(tree) = projects.iter().find(|t| Some(t.project.id) == project) else {
                return Vec::new();
            };
            let mut out = Vec::new();
            if current.is_some() {
                out.push(MenuItem::MoveTo(None, no_epic.to_owned()));
            }
            out.extend(
                tree.epics
                    .iter()
                    .filter(|(e, _)| Some(e.id) != current)
                    .map(|(e, _)| MenuItem::MoveTo(Some(e.id), e.title.clone())),
            );
            out
        }
        _ => Vec::new(),
    }
}

/// What the user folded and unfolded. Projects and epics start expanded and
/// are listed when collapsed; closed groups start folded and are listed
/// when shown. Both hold record ids only (docs/design/layout.md 4.7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folds {
    pub collapsed: HashSet<String>,
    pub closed_shown: HashSet<String>,
}

impl Folds {
    /// Flip the fold of the row with `key` (a `closed:` key is a closed group).
    pub fn toggle(&mut self, key: String) {
        let set = if key.starts_with("closed:") {
            &mut self.closed_shown
        } else {
            &mut self.collapsed
        };
        if !set.remove(&key) {
            set.insert(key);
        }
    }

    /// Unfold a project or epic (the project of the page that opens).
    pub fn expand(&mut self, key: &str) {
        self.collapsed.remove(key);
    }
}

fn push_topics(
    rows: &mut Vec<Row>,
    topics: &[TopicSummary],
    group: Group,
    depth: u8,
    folds: &Folds,
) {
    let (open, closed): (Vec<_>, Vec<_>) = topics.iter().partition(|t| t.status.is_open());
    for topic in open {
        rows.push(topic_row(topic, depth));
    }
    if closed.is_empty() {
        return;
    }
    let shown = folds.closed_shown.contains(&group.key());
    rows.push(Row::Closed {
        group,
        count: closed.len(),
        shown,
        depth,
    });
    if shown {
        for topic in closed {
            rows.push(topic_row(topic, depth));
        }
    }
}

fn topic_row(topic: &TopicSummary, depth: u8) -> Row {
    Row::Topic {
        id: topic.id,
        title: topic.title.clone(),
        status: topic.status,
        depth,
    }
}

/// The visible rows, top to bottom.
pub fn rows(projects: &[ProjectTree], folds: &Folds) -> Vec<Row> {
    let mut rows = Vec::new();
    for tree in projects {
        let project = &tree.project;
        let expanded = !folds.collapsed.contains(&project.id.to_string());
        rows.push(Row::Project {
            id: project.id,
            title: project.title.clone(),
            expanded,
        });
        if !expanded {
            continue;
        }
        rows.push(Row::Inbox {
            project: project.id,
            count: project.inbox,
        });
        rows.push(Row::Knowledge {
            project: project.id,
            count: project.knowledge,
        });
        push_topics(
            &mut rows,
            &tree.loose_topics,
            Group::Loose(project.id),
            1,
            folds,
        );
        for (epic, topics) in &tree.epics {
            let expanded = !folds.collapsed.contains(&epic.id.to_string());
            rows.push(Row::Epic {
                id: epic.id,
                project: project.id,
                title: epic.title.clone(),
                expanded,
            });
            if expanded {
                push_topics(&mut rows, topics, Group::Epic(epic.id), 2, folds);
            }
        }
    }
    rows
}

/// Where the keyboard cursor goes on `Left`: the row's parent, or `None`
/// for a top-level row.
pub fn parent_index(rows: &[Row], at: usize) -> Option<usize> {
    let depth = rows.get(at)?.depth();
    if depth == 0 {
        return None;
    }
    (0..at).rev().find(|&i| rows[i].depth() < depth)
}

/// `Alt+Up` / `Alt+Down`: the index to give `Store::reorder` so that `id`
/// passes its visible neighbour. `stored` is every sibling in the stored
/// order (closed topics included), `visible` the siblings the tree shows,
/// in the same order. `None` at either end.
pub fn move_target<T: PartialEq>(stored: &[T], visible: &[T], id: &T, up: bool) -> Option<usize> {
    let at = visible.iter().position(|v| v == id)?;
    let neighbour = if up {
        visible.get(at.checked_sub(1)?)?
    } else {
        visible.get(at + 1)?
    };
    stored.iter().position(|s| s == neighbour)
}

/// The stored siblings of a tree record (topics of the same group, epics of
/// the same project, or every project), as records.
pub fn stored_siblings(projects: &[ProjectTree], row: &Row) -> Vec<acta_core::model::Record> {
    use acta_core::model::Record;
    match row {
        Row::Project { .. } => projects
            .iter()
            .map(|t| Record::Project(t.project.id))
            .collect(),
        Row::Epic { project, .. } => projects
            .iter()
            .find(|t| t.project.id == *project)
            .map(|t| t.epics.iter().map(|(e, _)| Record::Epic(e.id)).collect())
            .unwrap_or_default(),
        Row::Topic { id, .. } => projects
            .iter()
            .find_map(|t| {
                if t.loose_topics.iter().any(|x| x.id == *id) {
                    return Some(t.loose_topics.iter().map(|x| Record::Topic(x.id)).collect());
                }
                t.epics
                    .iter()
                    .find(|(_, topics)| topics.iter().any(|x| x.id == *id))
                    .map(|(_, topics)| topics.iter().map(|x| Record::Topic(x.id)).collect())
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The record of a row that can move or be deleted.
pub fn row_record(row: &Row) -> Option<acta_core::model::Record> {
    use acta_core::model::Record;
    match row {
        Row::Topic { id, .. } => Some(Record::Topic(*id)),
        Row::Epic { id, .. } => Some(Record::Epic(*id)),
        Row::Project { id, .. } => Some(Record::Project(*id)),
        _ => None,
    }
}

/// The project, epic and topic a page belongs to, as far as the tree knows:
/// the breadcrumb (docs/design/layout.md 4.3) and the active project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Location {
    pub project: Option<(ProjectId, String)>,
    pub epic: Option<(EpicId, String)>,
    pub topic: Option<(TopicId, String)>,
}

pub fn locate_topic(projects: &[ProjectTree], id: TopicId) -> Option<Location> {
    for tree in projects {
        let project = Some((tree.project.id, tree.project.title.clone()));
        if let Some(topic) = tree.loose_topics.iter().find(|t| t.id == id) {
            return Some(Location {
                project,
                epic: None,
                topic: Some((topic.id, topic.title.clone())),
            });
        }
        for (epic, topics) in &tree.epics {
            if let Some(topic) = topics.iter().find(|t| t.id == id) {
                return Some(Location {
                    project,
                    epic: Some((epic.id, epic.title.clone())),
                    topic: Some((topic.id, topic.title.clone())),
                });
            }
        }
    }
    None
}

pub fn locate_epic(projects: &[ProjectTree], id: EpicId) -> Option<Location> {
    projects.iter().find_map(|tree| {
        let (epic, _) = tree.epics.iter().find(|(e, _)| e.id == id)?;
        Some(Location {
            project: Some((tree.project.id, tree.project.title.clone())),
            epic: Some((epic.id, epic.title.clone())),
            topic: None,
        })
    })
}

pub fn locate_project(projects: &[ProjectTree], id: ProjectId) -> Option<Location> {
    projects
        .iter()
        .find(|t| t.project.id == id)
        .map(|tree| Location {
            project: Some((tree.project.id, tree.project.title.clone())),
            ..Location::default()
        })
}

#[cfg(test)]
pub mod sample {
    //! A tree like the wireframe of docs/design/screens.md 8.1.
    use super::*;
    use acta_core::model::{Author, EpicSummary, Progress, ProjectSummary};

    pub fn topic(id: i64, epic: Option<i64>, title: &str, status: Status) -> TopicSummary {
        TopicSummary {
            id: TopicId(id),
            epic: epic.map(EpicId),
            title: title.into(),
            status,
            position: id,
            entries: 0,
            updated_at: 0,
            updated_by: Author::Human,
        }
    }

    pub fn epic(id: i64, title: &str) -> EpicSummary {
        EpicSummary {
            id: EpicId(id),
            title: title.into(),
            position: id,
            progress: Progress::default(),
            updated_at: 0,
        }
    }

    pub fn project(id: i64, title: &str) -> ProjectSummary {
        ProjectSummary {
            id: ProjectId(id),
            title: title.into(),
            position: id,
            progress: Progress::default(),
            inbox: 4,
            knowledge: 55,
            updated_at: 0,
        }
    }

    pub fn acta() -> Vec<ProjectTree> {
        vec![
            ProjectTree {
                project: project(1, "Acta"),
                loose_topics: vec![topic(1, None, "Naming", Status::Draft)],
                epics: vec![(
                    epic(1, "Terminal"),
                    vec![
                        topic(2, Some(1), "Embed the terminal", Status::Active),
                        topic(3, Some(1), "Scrollback", Status::Blocked),
                        topic(4, Some(1), "Terminal spike", Status::Done),
                        topic(5, Some(1), "Old idea", Status::Dropped),
                    ],
                )],
            },
            ProjectTree {
                project: project(2, "Atlas"),
                loose_topics: vec![],
                epics: vec![],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::sample::acta;
    use super::*;

    fn labels(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                Row::Project { title, .. } => format!("P {title}"),
                Row::Inbox { count, .. } => format!("  inbox {count}"),
                Row::Knowledge { count, .. } => format!("  knowledge {count}"),
                Row::Epic { title, .. } => format!("  E {title}"),
                Row::Topic { title, depth, .. } => {
                    format!("{}{title}", " ".repeat(*depth as usize * 2))
                }
                Row::Closed { count, depth, .. } => {
                    format!("{}{count} closed", " ".repeat(*depth as usize * 2))
                }
            })
            .collect()
    }

    #[test]
    fn rows_follow_the_shell_order_and_fold_closed_topics() {
        let rows = rows(&acta(), &Folds::default());
        assert_eq!(
            labels(&rows),
            [
                "P Acta",
                "  inbox 4",
                "  knowledge 55",
                "  Naming",
                "  E Terminal",
                "    Embed the terminal",
                "    Scrollback",
                "    2 closed",
                "P Atlas",
                "  inbox 4",
                "  knowledge 55",
            ]
        );
    }

    #[test]
    fn folds_hide_and_show_and_round_trip() {
        let mut folds = Folds::default();
        folds.toggle("E-1".into());
        folds.toggle("P-2".into());
        let rows = rows(&acta(), &folds);
        assert!(!labels(&rows).iter().any(|l| l.contains("Embed")));
        assert_eq!(labels(&rows).last().unwrap(), "P Atlas");

        folds.toggle("E-1".into());
        folds.toggle(Group::Epic(EpicId(1)).key());
        let rows = super::rows(&acta(), &folds);
        let labels = labels(&rows);
        assert!(labels.contains(&"    Terminal spike".to_string()));
        assert!(labels.contains(&"    Old idea".to_string()));

        folds.expand("P-2");
        assert!(folds.collapsed.is_empty());
    }

    #[test]
    fn left_goes_to_the_parent_row() {
        let rows = rows(&acta(), &Folds::default());
        let embed = rows
            .iter()
            .position(|r| matches!(r, Row::Topic { id, .. } if *id == TopicId(2)))
            .unwrap();
        let epic = parent_index(&rows, embed).unwrap();
        assert!(matches!(rows[epic], Row::Epic { .. }));
        assert_eq!(parent_index(&rows, epic), Some(0));
        assert_eq!(parent_index(&rows, 0), None);
    }

    #[test]
    fn moving_passes_the_visible_neighbour_across_hidden_closed_topics() {
        // Stored: a, x (closed, folded away), b, c.
        let stored = ["a", "x", "b", "c"];
        let visible = ["a", "b", "c"];
        // b up passes a: index 0.
        assert_eq!(move_target(&stored, &visible, &"b", true), Some(0));
        // a down passes b: b's stored index (after removal, right after b).
        assert_eq!(move_target(&stored, &visible, &"a", false), Some(2));
        assert_eq!(move_target(&stored, &visible, &"a", true), None);
        assert_eq!(move_target(&stored, &visible, &"c", false), None);
        let projects = acta();
        let rows = rows(&projects, &Folds::default());
        let scroll = rows
            .iter()
            .find(|r| matches!(r, Row::Topic { id, .. } if *id == TopicId(3)))
            .unwrap();
        assert_eq!(
            stored_siblings(&projects, scroll).len(),
            4,
            "closed included"
        );
    }

    fn topic_at(rows: &[Row], id: i64) -> usize {
        rows.iter()
            .position(|r| matches!(r, Row::Topic { id: t, .. } if *t == TopicId(id)))
            .unwrap()
    }

    #[test]
    fn the_cursor_follows_its_row_when_the_rows_move() {
        // Alt+Down, then Mod+Z puts the topic back: the cursor goes with
        // the topic, not with the index.
        let mut projects = acta();
        let before = rows(&projects, &Folds::default());
        let mut cursor = Cursor::default();
        cursor.set(&before, topic_at(&before, 2));
        projects[0].epics[0].1.swap(0, 1);
        let after = rows(&projects, &Folds::default());
        assert_eq!(cursor.index(&after), topic_at(&after, 2));
        projects[0].epics[0].1.swap(0, 1);
        let undone = rows(&projects, &Folds::default());
        assert_eq!(cursor.index(&undone), topic_at(&undone, 2));
    }

    #[test]
    fn a_cursor_whose_row_is_gone_keeps_its_place() {
        let mut projects = acta();
        let before = rows(&projects, &Folds::default());
        let mut cursor = Cursor::default();
        let at = topic_at(&before, 2);
        cursor.set(&before, at);
        projects[0].epics[0].1.remove(0);
        let after = rows(&projects, &Folds::default());
        assert_eq!(cursor.index(&after), at);
        assert_eq!(cursor.index(&[]), 0);
    }

    #[test]
    fn the_cursor_goes_to_the_open_page() {
        // Mod+1 lands on the row of the page that is open.
        let rows = rows(&acta(), &Folds::default());
        let mut cursor = Cursor::default();
        assert!(cursor.put(&rows, Route::Topic(TopicId(3))));
        assert_eq!(cursor.index(&rows), topic_at(&rows, 3));
        assert!(!cursor.put(&rows, Route::Topic(TopicId(4))), "folded away");
        assert_eq!(cursor.index(&rows), topic_at(&rows, 3));
    }

    #[test]
    fn type_ahead_jumps_to_a_title_and_wraps() {
        let rows = rows(&acta(), &Folds::default());
        assert_eq!(type_ahead(&rows, 0, "sc"), Some(topic_at(&rows, 3)));
        assert_eq!(type_ahead(&rows, 0, "EMB"), Some(topic_at(&rows, 2)));
        // From past it, the search wraps to the top.
        assert_eq!(type_ahead(&rows, rows.len() - 1, "acta"), Some(0));
        assert_eq!(type_ahead(&rows, 0, "zz"), None);
    }

    #[test]
    fn letters_act_unless_a_type_ahead_is_going() {
        assert_eq!(letter("", 'j'), Letter::Down);
        assert_eq!(letter("", 'K'), Letter::Up);
        assert_eq!(letter("", 's'), Letter::Status);
        assert_eq!(letter("", 'e'), Letter::Rename);
        assert_eq!(letter("", 'm'), Letter::Move);
        assert_eq!(letter("", ' '), Letter::Preview);
        assert_eq!(letter("", 'n'), Letter::Find("n".into()));
        assert_eq!(letter("na", 'j'), Letter::Find("naj".into()));
        assert_eq!(letter("old", ' '), Letter::Find("old ".into()));
    }

    #[test]
    fn menus_offer_what_the_row_can_do() {
        let projects = acta();
        let rows = rows(&projects, &Folds::default());
        let embed = &rows[topic_at(&rows, 2)];
        let menu = menu_items(&projects, embed, MenuKind::Row, "No epic");
        assert!(menu.contains(&MenuItem::StatusMenu) && menu.contains(&MenuItem::MoveMenu));
        assert_eq!(menu.last(), Some(&MenuItem::Delete));
        assert_eq!(
            menu_items(&projects, embed, MenuKind::Status, "No epic").len(),
            6
        );
        // In epic Terminal: out of it; no other epic exists.
        assert_eq!(
            menu_items(&projects, embed, MenuKind::Epics, "No epic"),
            [MenuItem::MoveTo(None, "No epic".into())]
        );
        // A loose topic: into Terminal, and no "No epic".
        let naming = &rows[topic_at(&rows, 1)];
        assert_eq!(
            menu_items(&projects, naming, MenuKind::Epics, "No epic"),
            [MenuItem::MoveTo(Some(EpicId(1)), "Terminal".into())]
        );
        let inbox = &rows[1];
        assert!(menu_items(&projects, inbox, MenuKind::Status, "No epic").is_empty());
        assert!(
            !menu_items(&projects, inbox, MenuKind::Row, "No epic").contains(&MenuItem::Delete)
        );
    }

    #[test]
    fn locations_name_project_and_epic() {
        let projects = acta();
        let at = locate_topic(&projects, TopicId(2)).unwrap();
        assert_eq!(at.project.unwrap().1, "Acta");
        assert_eq!(at.epic.unwrap().1, "Terminal");
        let loose = locate_topic(&projects, TopicId(1)).unwrap();
        assert!(loose.epic.is_none());
        assert!(locate_topic(&projects, TopicId(99)).is_none());
        assert_eq!(
            locate_epic(&projects, EpicId(1))
                .unwrap()
                .project
                .unwrap()
                .0,
            ProjectId(1)
        );
        assert!(locate_project(&projects, ProjectId(2)).is_some());
    }
}
