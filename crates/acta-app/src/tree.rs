//! The sidebar tree as rows (docs/design/layout.md 4.2.1, components.md 5
//! Tree row): every project; under an expanded project its Inbox and
//! Knowledge rows, the topics outside any epic, then the epics with their
//! topics. Closed topics collapse under a "3 closed" row at the end of their
//! group; an expanded epic with no topic shows a "New topic" ghost row.
//! Also the geometry the view needs without a window: where each row sits,
//! which rows are drawn, the sticky project row, how far a reveal scrolls,
//! and the one signal of a row's trailing slot. Pure, so all of it is
//! tested without a window.

use std::collections::HashSet;
use std::ops::Range;

use acta_core::date::{Date, DueState, due_state};
use acta_core::model::{EpicId, ProjectId, ProjectTree, Status, TopicId, TopicSummary};
use acta_core::store::Unread;

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
        /// Its own due date while it is open; see [`row_due`].
        due: Option<Date>,
    },
    Inbox {
        project: ProjectId,
        count: i64,
    },
    Knowledge {
        project: ProjectId,
    },
    Epic {
        id: EpicId,
        project: ProjectId,
        title: String,
        expanded: bool,
        due: Option<Date>,
    },
    Topic {
        id: TopicId,
        title: String,
        status: Status,
        depth: u8,
        due: Option<Date>,
    },
    Closed {
        group: Group,
        count: usize,
        shown: bool,
        depth: u8,
    },
    /// The ghost row of an expanded epic with no topic at all.
    NewTopic {
        project: ProjectId,
        epic: EpicId,
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
            Row::NewTopic { .. } => 2,
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
            Row::Closed { .. } | Row::NewTopic { .. } => None,
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
            (Row::NewTopic { epic, .. }, None) => RowKey::NewTopic(*epic),
            (_, None) => unreachable!("only the ghost rows open no page"),
        }
    }

    pub fn is_project(&self) -> bool {
        matches!(self, Row::Project { .. })
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

/// Whether a row shows the unread dot (interaction.md 6.5, rule 3): a
/// topic Claude Code changed since it was last shown; a collapsed epic or
/// project holding one (so the dot is never folded away); the Knowledge
/// row of a project with an entry Claude Code changed.
pub fn row_unread(row: &Row, projects: &[ProjectTree], unread: &Unread) -> bool {
    if unread.topics.is_empty() && unread.kb.is_empty() {
        return false;
    }
    match row {
        Row::Topic { id, .. } => unread.topics.contains(id),
        Row::Epic {
            id,
            expanded: false,
            ..
        } => projects
            .iter()
            .flat_map(|t| &t.epics)
            .filter(|(e, _)| e.id == *id)
            .flat_map(|(_, topics)| topics)
            .any(|t| unread.topics.contains(&t.id)),
        Row::Project {
            id,
            expanded: false,
            ..
        } => projects.iter().filter(|t| t.project.id == *id).any(|t| {
            unread.project_has_kb(*id)
                || t.loose_topics
                    .iter()
                    .chain(t.epics.iter().flat_map(|(_, topics)| topics))
                    .any(|topic| unread.topics.contains(&topic.id))
        }),
        Row::Knowledge { project } => unread.project_has_kb(*project),
        Row::Closed {
            group,
            shown: false,
            ..
        } => group_topics(projects, *group)
            .iter()
            .any(|t| !t.status.is_open() && unread.topics.contains(&t.id)),
        _ => false,
    }
}

/// The topics of a group, closed ones included.
fn group_topics(projects: &[ProjectTree], group: Group) -> &[TopicSummary] {
    let found = match group {
        Group::Loose(id) => projects
            .iter()
            .find(|t| t.project.id == id)
            .map(|t| t.loose_topics.as_slice()),
        Group::Epic(id) => projects
            .iter()
            .flat_map(|t| &t.epics)
            .find(|(e, _)| e.id == id)
            .map(|(_, topics)| topics.as_slice()),
    };
    found.unwrap_or_default()
}

/// The identity of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKey {
    Route(Route),
    Closed(Group),
    NewTopic(EpicId),
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
    /// `D`: the due date picker of a project, epic or topic row.
    Due,
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
        'd' => Letter::Due,
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

    /// "Collapse all" (4.2.1): every project and every epic folds; the
    /// "N closed" folds are kept.
    pub fn collapse_all(&mut self, projects: &[ProjectTree]) {
        for tree in projects {
            self.collapsed.insert(tree.project.id.to_string());
            for (epic, _) in &tree.epics {
                self.collapsed.insert(epic.id.to_string());
            }
        }
    }

    /// Unfold what hides the row of `route` (4.2.1, reveal): its project,
    /// its epic, and for a closed topic its "N closed" row. They stay
    /// unfolded, as if the user had unfolded them.
    pub fn reveal(&mut self, projects: &[ProjectTree], route: Route) {
        let location = match route {
            Route::Project(id) | Route::Inbox(id) | Route::Knowledge(id) => {
                locate_project(projects, id)
            }
            Route::Epic(id) => locate_epic(projects, id),
            Route::Topic(id) => locate_topic(projects, id),
            Route::Kb(_)
            | Route::Trash
            | Route::Settings
            | Route::Shortcuts
            | Route::Questions(_)
            | Route::Contacts
            | Route::Contact(_) => None,
        };
        let Some(location) = location else { return };
        if let Some((project, _)) = location.project {
            self.expand(&project.to_string());
        }
        if let Some((epic, _)) = location.epic {
            self.expand(&epic.to_string());
        }
        if let (Route::Topic(id), Some((project, _))) = (route, location.project) {
            let group = match location.epic {
                Some((epic, _)) => Group::Epic(epic),
                None => Group::Loose(project),
            };
            let closed = group_topics(projects, group)
                .iter()
                .any(|t| t.id == id && !t.status.is_open());
            if closed {
                self.closed_shown.insert(group.key());
            }
        }
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
        due: topic.due_on.filter(|_| topic.status.is_open()),
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
            due: project.due_on.filter(|_| project.progress.is_open()),
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
                due: epic.due_on.filter(|_| epic.progress.is_open()),
            });
            if expanded && topics.is_empty() {
                rows.push(Row::NewTopic {
                    project: project.id,
                    epic: epic.id,
                });
            } else if expanded {
                push_topics(&mut rows, topics, Group::Epic(epic.id), 2, folds);
            }
        }
    }
    rows
}

/// A row's height (4.2.1, Geometry).
pub const ROW: f32 = 28.;
/// The tree viewport's padding, on every side.
pub const PAD: f32 = 8.;
/// A row's own horizontal padding inside its pill.
const ROW_PAD: f32 = 8.;
/// The leading slot, and the gap after it and before the trailing slot.
const SLOT: f32 = 16.;
const GAP: f32 = 8.;

/// Where the rows sit in the tree's scrolled content: each row's top, and
/// the content's height. Rows are 28; 4 of space follow the Knowledge row,
/// 8 precede a project row that follows an expanded project (folded
/// projects stack with no space), 8 pad the top and the bottom.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    tops: Vec<f32>,
    pub height: f32,
}

impl Layout {
    pub fn of(rows: &[Row]) -> Layout {
        let mut tops = Vec::with_capacity(rows.len());
        let mut y = PAD;
        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                let previous = &rows[i - 1];
                if row.is_project() && !previous.is_project() {
                    y += 8.;
                } else if matches!(previous, Row::Knowledge { .. }) {
                    y += 4.;
                }
            }
            tops.push(y);
            y += ROW;
        }
        Layout {
            tops,
            height: y + PAD,
        }
    }

    pub fn top(&self, i: usize) -> f32 {
        self.tops[i]
    }

    /// The first row whose bottom is below `y` (the row at that height,
    /// or the next one when `y` is in a space).
    fn first_below(&self, y: f32) -> usize {
        self.tops.partition_point(|top| top + ROW <= y)
    }

    /// The rows to lay out for a viewport at `scroll_top`, `viewport` high:
    /// those in it, plus one screen above and below (4.2.1, Long trees).
    pub fn drawn(&self, scroll_top: f32, viewport: f32) -> Range<usize> {
        let start = self.first_below(scroll_top - viewport);
        let end = self
            .tops
            .partition_point(|top| *top < scroll_top + 2. * viewport);
        start..end.max(start)
    }

    /// The first row in view and how far the viewport's top is below its
    /// top: what keeps the rows on screen in place when rows come and go
    /// above them (interaction.md 6.5, rule 2).
    pub fn anchor(&self, rows: &[Row], scroll_top: f32) -> Option<(RowKey, f32)> {
        let i = self.first_below(scroll_top);
        let row = rows.get(i)?;
        Some((row.key(), scroll_top - self.tops[i]))
    }

    /// The scroll top that puts the anchor's row back where it was; `None`
    /// when that row is gone.
    pub fn anchored(&self, rows: &[Row], (key, delta): (RowKey, f32)) -> Option<f32> {
        let i = rows.iter().position(|r| r.key() == key)?;
        Some((self.tops[i] + delta).max(0.))
    }

    /// The sticky project row at `scroll_top` (4.2.1, Long trees): the
    /// expanded project whose row is above the viewport while some of its
    /// rows are still in it, as its index and its y in the viewport (0, or
    /// less while the next project's row pushes it up).
    pub fn sticky(&self, rows: &[Row], scroll_top: f32) -> Option<(usize, f32)> {
        let above = self.tops.partition_point(|top| *top < scroll_top);
        let p = (0..above).rev().find(|&i| rows[i].is_project())?;
        if rows[p].is_expanded() != Some(true) {
            return None;
        }
        let next = (p + 1..rows.len()).find(|&i| rows[i].is_project());
        let last = next.unwrap_or(rows.len()) - 1;
        if last == p || self.tops[last] + ROW <= scroll_top {
            return None;
        }
        let y = next.map_or(0., |q| (self.tops[q] - scroll_top - ROW).min(0.));
        Some((p, y))
    }

    /// The scroll top that shows row `i` whole below the sticky row, moving
    /// the least (4.2.1, reveal); `None` when it is shown already.
    pub fn reveal(&self, rows: &[Row], i: usize, scroll_top: f32, viewport: f32) -> Option<f32> {
        let top = *self.tops.get(i)?;
        // Above a project row nothing sticks; above any other row the
        // sticky copy of its project may.
        let cover = if rows[i].is_project() { 0. } else { ROW };
        if top - cover < scroll_top {
            return Some(if i == 0 { 0. } else { (top - cover).max(0.) });
        }
        let bottom = top + ROW;
        if bottom > scroll_top + viewport {
            return Some(bottom - viewport);
        }
        None
    }
}

/// The width of a row's title box in a view `width` wide (4.2.1,
/// Geometry), given what its trailing slot reserves (0 for nothing).
pub fn title_width(width: f32, depth: u8, trailing: f32) -> f32 {
    let trailing = if trailing > 0. { GAP + trailing } else { 0. };
    width - 2. * PAD - 2. * ROW_PAD - SLOT * f32::from(depth) - SLOT - GAP - trailing
}

/// What a project or epic row's trailing slot always reserves: the `plus`
/// appears there on hover without moving the title.
pub const PARENT_TRAILING: f32 = 24.;

/// A due mark (4.2.1, Due marks): an open record due before today, or
/// today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    Overdue,
    Today,
}

/// The date behind a row's due mark: a project, epic or topic row carries
/// its own date only while the record is open (a topic not closed, an epic
/// or project not complete).
pub fn row_due_date(row: &Row) -> Option<Date> {
    match row {
        Row::Project { due, .. } | Row::Epic { due, .. } | Row::Topic { due, .. } => *due,
        _ => None,
    }
}

/// The due mark of a row on `today`: `Overdue` before today, `Today` on
/// it, nothing otherwise. Never rolled up into a folded parent: each row
/// shows its own date only. `today` is passed in, so the marks change at
/// midnight without rebuilding the rows.
pub fn row_due(row: &Row, today: Date) -> Option<Due> {
    match due_state(row_due_date(row), today, true) {
        DueState::Overdue(_) => Some(Due::Overdue),
        DueState::Today => Some(Due::Today),
        _ => None,
    }
}

/// The one signal of a row's trailing slot at rest (4.2.1, "One signal, in
/// this order"); on hover a project or epic row shows its `plus` instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// A live Claude Code session focuses the row's record
    /// (components-app.md 5, Claude Code sessions).
    Session,
    Unread,
    Due(Due),
}

/// The signal a row shows: a live session first (while Claude Code works
/// on a record it keeps writing to it, so its unread dot would say less),
/// then the unread dot, then overdue, then due today.
pub fn signal(session: bool, unread: bool, due: Option<Due>) -> Option<Signal> {
    if session {
        Some(Signal::Session)
    } else if unread {
        Some(Signal::Unread)
    } else {
        due.map(Signal::Due)
    }
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

/// A tree row being dragged (components.md 5, Tree row, Drag to
/// reorder): the pointer path of `Alt+Up` / `Alt+Down` and `M`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dragged {
    Project(ProjectId),
    Epic(ProjectId, EpicId),
    Topic(ProjectId, TopicId),
}

impl Dragged {
    /// The row a drag starts from: projects, epics and topics only.
    pub fn of(projects: &[ProjectTree], row: &Row) -> Option<Dragged> {
        match row {
            Row::Project { id, .. } => Some(Dragged::Project(*id)),
            Row::Epic { id, project, .. } => Some(Dragged::Epic(*project, *id)),
            Row::Topic { id, .. } => {
                let project = locate_topic(projects, *id)?.project?.0;
                Some(Dragged::Topic(project, *id))
            }
            _ => None,
        }
    }

    pub fn record(self) -> acta_core::model::Record {
        use acta_core::model::Record;
        match self {
            Dragged::Project(id) => Record::Project(id),
            Dragged::Epic(_, id) => Record::Epic(id),
            Dragged::Topic(_, id) => Record::Topic(id),
        }
    }
}

/// Where a drop lands: for a topic, the epic it ends in (`None`: in no
/// epic); the index to give `Store::reorder` among its new siblings; and
/// what is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drop {
    pub epic: Option<EpicId>,
    pub index: usize,
    pub show: DropShow,
}

/// The insertion line (its y in the tree's content and the depth it is
/// drawn at), or a row taking the drop-target state (its index).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropShow {
    Line { y: f32, depth: u8 },
    Onto(usize),
}

/// The topics of a group in stored order, closed ones included.
fn group_of(tree: &ProjectTree, epic: Option<EpicId>) -> Vec<TopicId> {
    match epic {
        None => tree.loose_topics.iter().map(|t| t.id).collect(),
        Some(e) => tree
            .epics
            .iter()
            .find(|(x, _)| x.id == e)
            .map(|(_, ts)| ts.iter().map(|t| t.id).collect())
            .unwrap_or_default(),
    }
}

/// The group a topic is in.
fn epic_of(tree: &ProjectTree, topic: TopicId) -> Option<Option<EpicId>> {
    if tree.loose_topics.iter().any(|t| t.id == topic) {
        return Some(None);
    }
    tree.epics
        .iter()
        .find(|(_, ts)| ts.iter().any(|t| t.id == topic))
        .map(|(e, _)| Some(e.id))
}

/// Where `dragged` lands with the pointer at `y` in the tree's content:
/// a topic between two topics of any group of its own project, onto an
/// epic row (the end of its topics) or onto its project row (the end of
/// the topics in no epic); an epic between the epics of its project; a
/// project between projects. `None`: no target, or the row's own place.
pub fn drop_at(
    projects: &[ProjectTree],
    rows: &[Row],
    layout: &Layout,
    dragged: Dragged,
    y: f32,
) -> Option<Drop> {
    let i = layout.first_below(y);
    let row = rows.get(i)?;
    let upper = y < layout.top(i) + ROW / 2.;
    // The last row of the block a header row at `h` starts: every row
    // after it deeper than `depth`.
    let block_end = |h: usize, depth: u8| {
        let mut j = h;
        while rows.get(j + 1).is_some_and(|r| r.depth() > depth) {
            j += 1;
        }
        j
    };
    // `ids` without the dragged one, the dragged one's index among all
    // `ids`, and the index it lands at before or after `other`.
    fn place<T: PartialEq + Copy>(
        ids: &[T],
        me: T,
        other: T,
        after: bool,
    ) -> Option<(usize, usize)> {
        let cur = ids.iter().position(|x| *x == me);
        let rest: Vec<T> = ids.iter().copied().filter(|x| *x != me).collect();
        let at = rest.iter().position(|x| *x == other)? + usize::from(after);
        Some((cur.unwrap_or(usize::MAX), at))
    }
    match dragged {
        Dragged::Topic(project, me) => {
            let tree = projects.iter().find(|t| t.project.id == project)?;
            let mine = epic_of(tree, me)?;
            let (epic, index, show) = match row {
                Row::Topic { id, depth, .. } => {
                    if *id == me {
                        return None;
                    }
                    let epic = epic_of(tree, *id)?;
                    let (_, at) = place(&group_of(tree, epic), me, *id, !upper)?;
                    let y = if upper {
                        layout.top(i)
                    } else {
                        layout.top(i) + ROW
                    };
                    (epic, at, DropShow::Line { y, depth: *depth })
                }
                Row::Epic { id, project: p, .. } if *p == project => {
                    let rest = group_of(tree, Some(*id))
                        .iter()
                        .filter(|t| **t != me)
                        .count();
                    (Some(*id), rest, DropShow::Onto(i))
                }
                Row::Project { id, .. } if *id == project => {
                    let rest = group_of(tree, None).iter().filter(|t| **t != me).count();
                    (None, rest, DropShow::Onto(i))
                }
                _ => return None,
            };
            let cur = group_of(tree, mine).iter().position(|t| *t == me)?;
            if epic == mine && index == cur {
                return None;
            }
            Some(Drop { epic, index, show })
        }
        Dragged::Epic(project, me) => {
            // The epic block under the pointer: its row, or a topic,
            // "N closed" or "New topic" row inside it (then after it).
            let h = (0..=i)
                .rev()
                .find(|&k| rows[k].depth() <= 1)
                .filter(|&k| k == i || row.depth() == 2)?;
            let Row::Epic { id, project: p, .. } = &rows[h] else {
                return None;
            };
            if *p != project || *id == me {
                return None;
            }
            let after = h != i || !upper;
            let tree = projects.iter().find(|t| t.project.id == project)?;
            let ids: Vec<EpicId> = tree.epics.iter().map(|(e, _)| e.id).collect();
            let (cur, at) = place(&ids, me, *id, after)?;
            if cur == at {
                return None;
            }
            let y = if after {
                layout.top(block_end(h, 1)) + ROW
            } else {
                layout.top(h)
            };
            Some(Drop {
                epic: None,
                index: at,
                show: DropShow::Line { y, depth: 1 },
            })
        }
        Dragged::Project(me) => {
            let h = (0..=i).rev().find(|&k| rows[k].is_project())?;
            let Row::Project { id, .. } = &rows[h] else {
                return None;
            };
            if *id == me {
                return None;
            }
            let after = h != i || !upper;
            let ids: Vec<ProjectId> = projects.iter().map(|t| t.project.id).collect();
            let (cur, at) = place(&ids, me, *id, after)?;
            if cur == at {
                return None;
            }
            let y = if after {
                layout.top(block_end(h, 0)) + ROW
            } else {
                layout.top(h)
            };
            Some(Drop {
                epic: None,
                index: at,
                show: DropShow::Line { y, depth: 0 },
            })
        }
    }
}

/// While dragging, within 28 of the viewport's top or bottom edge the tree
/// scrolls, up to 12 per frame, faster the nearer the edge: the change of
/// the scroll top for a pointer at `y` in a viewport `viewport` high.
pub fn auto_scroll(y: f32, viewport: f32) -> f32 {
    const EDGE: f32 = 28.;
    const MOST: f32 = 12.;
    if viewport <= 2. * EDGE {
        return 0.;
    }
    if y < EDGE {
        -MOST * (EDGE - y.max(0.)) / EDGE
    } else if y > viewport - EDGE {
        MOST * (y.min(viewport) - (viewport - EDGE)) / EDGE
    } else {
        0.
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
            due_on: None,
            updated_at: 0,
            updated_by: Author::Human,
            report_to: None,
            reported: false,
        }
    }

    pub fn epic(id: i64, title: &str) -> EpicSummary {
        EpicSummary {
            id: EpicId(id),
            title: title.into(),
            position: id,
            progress: Progress::default(),
            due_on: None,
            updated_at: 0,
            report_to: None,
            reported: false,
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
            due_on: None,
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

    #[test]
    fn the_unread_dot_follows_topics_folded_parents_and_knowledge() {
        let projects = acta();
        let unread = Unread {
            topics: [TopicId(3)].into_iter().collect(),
            kb: vec![(acta_core::model::KbId(9), ProjectId(1))],
        };
        let topic = |id| Row::Topic {
            id: TopicId(id),
            title: String::new(),
            status: Status::Active,
            depth: 2,
            due: None,
        };
        let epic = |expanded| Row::Epic {
            id: EpicId(1),
            project: ProjectId(1),
            title: "Terminal".into(),
            expanded,
            due: None,
        };
        let project = |id, expanded| Row::Project {
            id: ProjectId(id),
            title: String::new(),
            expanded,
            due: None,
        };
        assert!(row_unread(&topic(3), &projects, &unread));
        assert!(!row_unread(&topic(2), &projects, &unread));
        assert!(row_unread(&epic(false), &projects, &unread), "folded");
        assert!(
            !row_unread(&epic(true), &projects, &unread),
            "the topic shows it"
        );
        assert!(row_unread(&project(1, false), &projects, &unread));
        assert!(!row_unread(&project(1, true), &projects, &unread));
        assert!(!row_unread(&project(2, false), &projects, &unread));
        let knowledge = |p| Row::Knowledge {
            project: ProjectId(p),
        };
        assert!(row_unread(&knowledge(1), &projects, &unread));
        assert!(!row_unread(&knowledge(2), &projects, &unread));
        assert!(!row_unread(&topic(3), &projects, &Unread::default()));
    }

    fn labels(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                Row::Project { title, .. } => format!("P {title}"),
                Row::Inbox { count, .. } => format!("  inbox {count}"),
                Row::Knowledge { .. } => "  knowledge".to_string(),
                Row::Epic { title, .. } => format!("  E {title}"),
                Row::Topic { title, depth, .. } => {
                    format!("{}{title}", " ".repeat(*depth as usize * 2))
                }
                Row::Closed { count, depth, .. } => {
                    format!("{}{count} closed", " ".repeat(*depth as usize * 2))
                }
                Row::NewTopic { .. } => "    + New topic".to_string(),
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
                "  knowledge",
                "  Naming",
                "  E Terminal",
                "    Embed the terminal",
                "    Scrollback",
                "    2 closed",
                "P Atlas",
                "  inbox 4",
                "  knowledge",
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
        assert_eq!(letter("", 'D'), Letter::Due);
        assert_eq!(letter("de", 'd'), Letter::Find("ded".into()));
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
    fn an_empty_epic_shows_the_new_topic_ghost_row() {
        let mut projects = acta();
        projects[1].epics.push((sample::epic(2, "Storage"), vec![]));
        let rows = rows(&projects, &Folds::default());
        let at = rows
            .iter()
            .position(|r| matches!(r, Row::NewTopic { .. }))
            .unwrap();
        assert!(matches!(rows[at - 1], Row::Epic { id, .. } if id == EpicId(2)));
        assert_eq!(rows[at].depth(), 2);
        assert_eq!(rows[at].key(), RowKey::NewTopic(EpicId(2)));
        assert_eq!(rows[at].route(), None);
        assert_eq!(parent_index(&rows, at), Some(at - 1));
        // Folded, or an epic with topics: no ghost row.
        let mut folds = Folds::default();
        folds.toggle("E-2".into());
        let ghost = |rows: &[Row]| rows.iter().any(|r| matches!(r, Row::NewTopic { .. }));
        assert!(!ghost(&super::rows(&projects, &folds)));
        assert!(!ghost(&super::rows(&acta(), &Folds::default())));
    }

    #[test]
    fn a_folded_closed_row_holds_the_dot_of_its_closed_topics() {
        let projects = acta();
        let unread = Unread {
            topics: [TopicId(4)].into_iter().collect(),
            kb: vec![],
        };
        let closed = |shown| Row::Closed {
            group: Group::Epic(EpicId(1)),
            count: 2,
            shown,
            depth: 2,
        };
        assert!(row_unread(&closed(false), &projects, &unread));
        assert!(
            !row_unread(&closed(true), &projects, &unread),
            "the topic shows it"
        );
        let open_only = Unread {
            topics: [TopicId(2)].into_iter().collect(),
            kb: vec![],
        };
        assert!(!row_unread(&closed(false), &projects, &open_only));
    }

    #[test]
    fn spaces_follow_knowledge_and_expanded_projects_only() {
        let rows = rows(&acta(), &Folds::default());
        let layout = Layout::of(&rows);
        assert_eq!(layout.top(0), PAD);
        assert_eq!(layout.top(1), PAD + ROW, "Inbox right under its project");
        assert_eq!(layout.top(3), PAD + 3. * ROW + 4., "4 below Knowledge");
        let atlas = rows.iter().rposition(Row::is_project).unwrap();
        assert_eq!(
            layout.top(atlas),
            layout.top(atlas - 1) + ROW + 8.,
            "8 after an expanded project"
        );
        assert_eq!(layout.height, layout.top(rows.len() - 1) + ROW + PAD);
        // Folded projects stack with no space.
        let mut folds = Folds::default();
        folds.collapse_all(&acta());
        let folded = super::rows(&acta(), &folds);
        assert_eq!(folded.len(), 2);
        assert_eq!(Layout::of(&folded).top(1), PAD + ROW);
    }

    /// Project "Acta" with `n` topics in no epic, then project "Atlas".
    fn long(n: i64) -> Vec<ProjectTree> {
        let mut projects = acta();
        projects[0].loose_topics = (100..100 + n)
            .map(|i| sample::topic(i, None, "Topic", Status::Planned))
            .collect();
        projects
    }

    #[test]
    fn only_the_rows_near_the_viewport_are_drawn() {
        let rows = rows(&long(10_000), &Folds::default());
        let layout = Layout::of(&rows);
        let drawn = layout.drawn(28_000., 560.);
        // One screen above and below: about 60 rows of 10,000.
        assert!(drawn.len() <= 62, "{drawn:?}");
        assert!(layout.top(drawn.start) + ROW > 28_000. - 560.);
        assert!(layout.top(drawn.end - 1) < 28_000. + 2. * 560.);
        assert_eq!(layout.drawn(0., 560.).start, 0);
    }

    #[test]
    fn the_sticky_row_names_the_project_and_is_pushed_up_by_the_next() {
        let rows = rows(&long(40), &Folds::default());
        let layout = Layout::of(&rows);
        assert_eq!(layout.sticky(&rows, 0.), None, "the project row is in view");
        assert_eq!(layout.sticky(&rows, 300.), Some((0, 0.)));
        let atlas = rows.iter().rposition(Row::is_project).unwrap();
        let next = layout.top(atlas);
        // The next project's row arrives at the sticky row's bottom.
        assert_eq!(layout.sticky(&rows, next - ROW), Some((0, 0.)));
        assert_eq!(layout.sticky(&rows, next - 10.), Some((0, -18.)));
        assert_eq!(layout.sticky(&rows, next), None, "Atlas is at the top");
        // Folded: nothing of it is in view, nothing sticks.
        let mut folds = Folds::default();
        folds.toggle("P-1".into());
        let folded = super::rows(&long(40), &folds);
        assert_eq!(Layout::of(&folded).sticky(&folded, 20.), None);
    }

    #[test]
    fn reveal_scrolls_the_least_and_clears_the_sticky_row() {
        let rows = rows(&long(100), &Folds::default());
        let layout = Layout::of(&rows);
        let i = 60;
        let top = layout.top(i);
        // In view: no scroll.
        assert_eq!(layout.reveal(&rows, i, top - 100., 560.), None);
        // Below: its bottom lands on the viewport's bottom.
        assert_eq!(layout.reveal(&rows, i, 0., 560.), Some(top + ROW - 560.));
        // Above, or under the sticky row: it lands right below the sticky row.
        assert_eq!(layout.reveal(&rows, i, top + 200., 560.), Some(top - ROW));
        assert_eq!(layout.reveal(&rows, i, top - 10., 560.), Some(top - ROW));
        // A project row needs no room for the sticky row; the first one
        // shows the padding above it too.
        let atlas = rows.iter().rposition(Row::is_project).unwrap();
        let atlas_top = layout.top(atlas);
        assert_eq!(
            layout.reveal(&rows, atlas, atlas_top + 50., 560.),
            Some(atlas_top)
        );
        assert_eq!(layout.reveal(&rows, 0, 300., 560.), Some(0.));
    }

    #[test]
    fn the_rows_on_screen_keep_their_place_when_rows_appear_above() {
        let projects = long(30);
        let before = rows(&projects, &Folds::default());
        let layout = Layout::of(&before);
        let scroll_top = layout.top(20) + 5.;
        let anchor = layout.anchor(&before, scroll_top).unwrap();
        assert_eq!(anchor, (before[20].key(), 5.));
        // Another process adds a topic above it.
        let mut more = projects.clone();
        more[0]
            .loose_topics
            .insert(0, sample::topic(99, None, "New", Status::Draft));
        let after = rows(&more, &Folds::default());
        let moved = Layout::of(&after).anchored(&after, anchor).unwrap();
        assert_eq!(moved, scroll_top + ROW);
        // The anchor's row gone: no correction.
        let mut gone = projects.clone();
        gone[0].loose_topics.clear();
        let after = rows(&gone, &Folds::default());
        assert_eq!(Layout::of(&after).anchored(&after, anchor), None);
    }

    #[test]
    fn title_space_matches_the_spec_at_260_and_200() {
        // Projects and epics with their reserved 24; topics without and
        // with a dot.
        let at = |w| {
            [
                title_width(w, 0, PARENT_TRAILING),
                title_width(w, 1, PARENT_TRAILING),
                title_width(w, 1, 0.),
                title_width(w, 1, 16.),
                title_width(w, 2, 0.),
                title_width(w, 2, 16.),
            ]
        };
        assert_eq!(at(260.), [172., 156., 188., 164., 172., 148.]);
        assert_eq!(at(200.), [112., 96., 128., 104., 112., 88.]);
    }

    #[test]
    fn one_signal_in_the_order_of_the_spec() {
        let overdue = Some(Due::Overdue);
        assert_eq!(signal(true, true, overdue), Some(Signal::Session));
        assert_eq!(signal(false, true, overdue), Some(Signal::Unread));
        assert_eq!(
            signal(false, false, overdue),
            Some(Signal::Due(Due::Overdue))
        );
        assert_eq!(
            signal(false, false, Some(Due::Today)),
            Some(Signal::Due(Due::Today))
        );
        assert_eq!(signal(false, false, None), None);
        let today = day("2026-11-21");
        let rows = rows(&acta(), &Folds::default());
        assert!(rows.iter().all(|r| row_due(r, today).is_none()), "no dates");
    }

    fn day(iso: &str) -> Date {
        Date::parse_iso(iso).unwrap()
    }

    /// The tree of 4.2.1 with dates (today is Saturday 21 Nov 2026): an
    /// overdue loose topic, a topic due tomorrow, a blocked overdue one, a
    /// done one with a past date, an epic due today and an overdue project.
    fn dated() -> Vec<ProjectTree> {
        use acta_core::model::Progress;
        let mut projects = acta();
        let tree = &mut projects[0];
        tree.project.due_on = Some(day("2026-11-01"));
        tree.project.progress = Progress { done: 1, total: 4 };
        tree.loose_topics[0].due_on = Some(day("2026-11-18"));
        let (epic, topics) = &mut tree.epics[0];
        epic.due_on = Some(day("2026-11-21"));
        epic.progress = Progress { done: 1, total: 3 };
        topics[0].due_on = Some(day("2026-11-22"));
        topics[1].due_on = Some(day("2026-11-20"));
        topics[2].due_on = Some(day("2026-11-01"));
        projects
    }

    fn row_of(rows: &[Row], route: Route) -> &Row {
        rows.iter().find(|r| r.route() == Some(route)).unwrap()
    }

    #[test]
    fn due_marks_show_overdue_and_today_on_open_records_only() {
        use acta_core::model::Progress;
        let today = day("2026-11-21");
        let mut folds = Folds::default();
        folds.closed_shown.insert(Group::Epic(EpicId(1)).key());
        let rows = rows(&dated(), &folds);
        let mark = |route| row_due(row_of(&rows, route), today);
        assert_eq!(mark(Route::Project(ProjectId(1))), Some(Due::Overdue));
        assert_eq!(mark(Route::Epic(EpicId(1))), Some(Due::Today));
        assert_eq!(mark(Route::Topic(TopicId(1))), Some(Due::Overdue));
        // Tomorrow is "due soon": the tree draws nothing for it.
        assert_eq!(mark(Route::Topic(TopicId(2))), None);
        // Blocked and overdue: the glyph leads, the mark trails.
        assert_eq!(mark(Route::Topic(TopicId(3))), Some(Due::Overdue));
        // Done with a past date: never overdue.
        assert_eq!(mark(Route::Topic(TopicId(4))), None);
        assert_eq!(mark(Route::Project(ProjectId(2))), None);
        // A complete epic or project is closed: no mark.
        let mut complete = dated();
        complete[0].epics[0].0.progress = Progress { done: 3, total: 3 };
        complete[0].project.progress = Progress { done: 3, total: 3 };
        let rows = super::rows(&complete, &folds);
        assert_eq!(
            row_due(row_of(&rows, Route::Project(ProjectId(1))), today),
            None
        );
        assert_eq!(row_due(row_of(&rows, Route::Epic(EpicId(1))), today), None);
        // An epic or project with no topic is open.
        let mut empty = dated();
        empty[0].project.progress = Progress::default();
        let rows = super::rows(&empty, &folds);
        assert_eq!(
            row_due(row_of(&rows, Route::Project(ProjectId(1))), today),
            Some(Due::Overdue)
        );
    }

    #[test]
    fn due_marks_change_at_midnight_without_rebuilding_the_rows() {
        let rows = rows(&dated(), &Folds::default());
        let tomorrow = row_of(&rows, Route::Topic(TopicId(2)));
        assert_eq!(row_due(tomorrow, day("2026-11-21")), None);
        assert_eq!(row_due(tomorrow, day("2026-11-22")), Some(Due::Today));
        assert_eq!(row_due(tomorrow, day("2026-11-23")), Some(Due::Overdue));
    }

    #[test]
    fn due_marks_are_never_rolled_up_and_unread_wins() {
        let today = day("2026-11-21");
        let mut folds = Folds::default();
        folds.toggle("E-1".into());
        let rows = rows(&dated(), &folds);
        // The folded epic shows its own date (today), not its topics'
        // overdue ones.
        assert_eq!(
            row_due(row_of(&rows, Route::Epic(EpicId(1))), today),
            Some(Due::Today)
        );
        let mut undated = dated();
        undated[0].epics[0].0.due_on = None;
        let rows = super::rows(&undated, &folds);
        assert_eq!(row_due(row_of(&rows, Route::Epic(EpicId(1))), today), None);
        // An unread overdue topic shows the dot; once read, the mark.
        let mark = Some(Due::Overdue);
        assert_eq!(signal(false, true, mark), Some(Signal::Unread));
        assert_eq!(signal(false, false, mark), Some(Signal::Due(Due::Overdue)));
    }

    #[test]
    fn reveal_unfolds_the_project_the_epic_and_the_closed_row() {
        let projects = acta();
        let mut folds = Folds::default();
        folds.collapse_all(&projects);
        assert!(folds.collapsed.contains("P-1") && folds.collapsed.contains("E-1"));
        folds.reveal(&projects, Route::Topic(TopicId(4)));
        let rows = rows(&projects, &folds);
        assert!(
            rows.iter()
                .any(|r| r.route() == Some(Route::Topic(TopicId(4))))
        );
        assert!(
            folds.collapsed.contains("P-2"),
            "other projects stay folded"
        );
        // An open topic leaves the closed row folded; collapse all keeps it.
        let mut folds = Folds::default();
        folds.reveal(&projects, Route::Topic(TopicId(2)));
        assert!(folds.closed_shown.is_empty());
        folds.closed_shown.insert(Group::Epic(EpicId(1)).key());
        folds.collapse_all(&projects);
        assert_eq!(folds.closed_shown.len(), 1);
        folds.reveal(&projects, Route::Knowledge(ProjectId(2)));
        assert!(!folds.collapsed.contains("P-2"));
    }

    #[test]
    fn a_topic_drops_between_topics_onto_epics_and_its_project_only() {
        let projects = acta();
        let rows = rows(&projects, &Folds::default());
        let layout = Layout::of(&rows);
        let upper = |i: usize| layout.top(i) + 4.;
        let lower = |i: usize| layout.top(i) + ROW - 4.;
        let embed = Dragged::Topic(ProjectId(1), TopicId(2));
        let drop = |y| drop_at(&projects, &rows, &layout, embed, y);
        // Its own row, and just above the next topic (its own place).
        assert_eq!(drop(upper(topic_at(&rows, 2))), None);
        assert_eq!(drop(upper(topic_at(&rows, 3))), None);
        // Below Scrollback, in its epic: index 1 among Terminal's topics.
        let below = drop(lower(topic_at(&rows, 3))).unwrap();
        assert_eq!((below.epic, below.index), (Some(EpicId(1)), 1));
        assert_eq!(
            below.show,
            DropShow::Line {
                y: layout.top(topic_at(&rows, 3)) + ROW,
                depth: 2
            }
        );
        // Above Naming: out of the epic, first of the loose topics.
        let naming = drop(upper(topic_at(&rows, 1))).unwrap();
        assert_eq!((naming.epic, naming.index), (None, 0));
        // Onto its project row: the end of the loose topics.
        let p = rows.iter().position(|r| r.is_project()).unwrap();
        let onto = drop(upper(p)).unwrap();
        assert_eq!((onto.epic, onto.index), (None, 1));
        assert_eq!(onto.show, DropShow::Onto(p));
        // Onto its own epic's row: the end of its topics (4 of them).
        let e = rows
            .iter()
            .position(|r| matches!(r, Row::Epic { .. }))
            .unwrap();
        let end = drop(upper(e)).unwrap();
        assert_eq!(
            (end.epic, end.index, end.show),
            (Some(EpicId(1)), 3, DropShow::Onto(e))
        );
        // Inbox, the "N closed" row and another project are no target.
        let inbox = rows
            .iter()
            .position(|r| matches!(r, Row::Inbox { .. }))
            .unwrap();
        assert_eq!(drop(upper(inbox)), None);
        let closed = rows
            .iter()
            .position(|r| matches!(r, Row::Closed { .. }))
            .unwrap();
        assert_eq!(drop(upper(closed)), None);
        let atlas = rows
            .iter()
            .position(|r| matches!(r, Row::Project { id, .. } if *id == ProjectId(2)))
            .unwrap();
        assert_eq!(drop(upper(atlas)), None);
    }

    #[test]
    fn an_epic_drops_before_or_after_a_whole_epic_block() {
        let mut projects = acta();
        projects[0]
            .epics
            .push((super::sample::epic(2, "Notes"), Vec::new()));
        let rows = rows(&projects, &Folds::default());
        let layout = Layout::of(&rows);
        let terminal = Dragged::Epic(ProjectId(1), EpicId(1));
        let notes = rows
            .iter()
            .position(|r| matches!(r, Row::Epic { id, .. } if *id == EpicId(2)))
            .unwrap();
        // The lower half of Notes' row: after its block ("New topic").
        let drop = drop_at(
            &projects,
            &rows,
            &layout,
            terminal,
            layout.top(notes) + ROW - 2.,
        )
        .unwrap();
        assert_eq!(drop.index, 1);
        assert_eq!(
            drop.show,
            DropShow::Line {
                y: layout.top(notes + 1) + ROW,
                depth: 1
            }
        );
        // The upper half of Notes' row is Terminal's own place; a topic of
        // Terminal is inside its own block.
        assert_eq!(
            drop_at(&projects, &rows, &layout, terminal, layout.top(notes) + 2.),
            None
        );
        assert_eq!(
            drop_at(
                &projects,
                &rows,
                &layout,
                terminal,
                layout.top(topic_at(&rows, 3)) + 2.
            ),
            None
        );
        // Notes dragged into Terminal's block: before nothing, after
        // Terminal is its own place; above Terminal's row it goes first.
        let notes_drag = Dragged::Epic(ProjectId(1), EpicId(2));
        let t = rows
            .iter()
            .position(|r| matches!(r, Row::Epic { id, .. } if *id == EpicId(1)))
            .unwrap();
        let first = drop_at(&projects, &rows, &layout, notes_drag, layout.top(t) + 2.).unwrap();
        assert_eq!(first.index, 0);
        assert_eq!(
            drop_at(
                &projects,
                &rows,
                &layout,
                notes_drag,
                layout.top(topic_at(&rows, 2)) + 2.
            ),
            None
        );
    }

    #[test]
    fn projects_drop_between_projects_after_a_whole_block() {
        let projects = acta();
        let rows = rows(&projects, &Folds::default());
        let layout = Layout::of(&rows);
        let atlas = rows
            .iter()
            .position(|r| matches!(r, Row::Project { id, .. } if *id == ProjectId(2)))
            .unwrap();
        let acta_drag = Dragged::Project(ProjectId(1));
        // Acta in the lower half of Atlas' row: after Atlas' block.
        let drop = drop_at(
            &projects,
            &rows,
            &layout,
            acta_drag,
            layout.top(atlas) + ROW - 2.,
        )
        .unwrap();
        assert_eq!(drop.index, 1);
        assert_eq!(
            drop.show,
            DropShow::Line {
                y: layout.top(rows.len() - 1) + ROW,
                depth: 0
            }
        );
        // The upper half of Atlas' row is Acta's own place.
        assert_eq!(
            drop_at(&projects, &rows, &layout, acta_drag, layout.top(atlas) + 2.),
            None
        );
        // Atlas above Acta's row: first.
        let first = drop_at(
            &projects,
            &rows,
            &layout,
            Dragged::Project(ProjectId(2)),
            layout.top(0) + 2.,
        )
        .unwrap();
        assert_eq!(first.index, 0);
    }

    #[test]
    fn the_tree_scrolls_near_its_edges_faster_the_nearer() {
        assert_eq!(auto_scroll(300., 600.), 0.);
        assert_eq!(auto_scroll(0., 600.), -12.);
        assert_eq!(auto_scroll(14., 600.), -6.);
        assert_eq!(auto_scroll(600., 600.), 12.);
        assert!(auto_scroll(590., 600.) > auto_scroll(580., 600.));
        // A viewport too short to have edges never scrolls.
        assert_eq!(auto_scroll(10., 40.), 0.);
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
