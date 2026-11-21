//! The workbench layout (docs/design/workbench.md 4.8): four areas (left
//! sidebar, right sidebar, bottom panel, page area), the views and page tabs
//! in their groups, where each hidden view was last, and the undo steps of
//! the window. Pure data: no GPUI type, so every rule of 4.8 is tested
//! without a window.
//!
//! A page tab is a [`TabId`]; the shell keeps what each one shows. The
//! preferences file holds the layout with each page tab as its route key
//! (4.8.12), through [`Saved`].

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

/// 4.8.3: the movable views. Chat and Agent browser are Windows only
/// (ADR 0009, ADR 0010): native web views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViewId {
    Projects,
    Inbox,
    Knowledge,
    Terminal,
    Chat,
    AgentBrowser,
}

impl ViewId {
    /// In the order of 4.8.3 (the titlebar's view toggles).
    pub const ALL: [ViewId; 6] = [
        ViewId::Projects,
        ViewId::Inbox,
        ViewId::Knowledge,
        ViewId::Terminal,
        ViewId::Chat,
        ViewId::AgentBrowser,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ViewId::Projects => "projects",
            ViewId::Inbox => "inbox",
            ViewId::Knowledge => "knowledge",
            ViewId::Terminal => "terminal",
            ViewId::Chat => "chat",
            ViewId::AgentBrowser => "agent_browser",
        }
    }

    pub fn parse(key: &str) -> Option<ViewId> {
        ViewId::ALL.into_iter().find(|v| v.key() == key)
    }

    /// 4.8.3: minimum width and height wherever the view is.
    pub fn min(self) -> (f32, f32) {
        match self {
            ViewId::Projects => (200., 160.),
            ViewId::Inbox | ViewId::Knowledge => (280., 200.),
            ViewId::Terminal => (400., 160.),
            ViewId::Chat | ViewId::AgentBrowser => (400., 240.),
        }
    }

    /// Inbox and Knowledge are pages in the page area and views elsewhere
    /// (4.8.3).
    pub fn is_page_kind(self) -> bool {
        matches!(self, ViewId::Inbox | ViewId::Knowledge)
    }

    /// A native web view: Windows only for now (ADR 0009, ADR 0010).
    pub fn is_web(self) -> bool {
        matches!(self, ViewId::Chat | ViewId::AgentBrowser)
    }
}

/// The three areas around the page (4.8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Left,
    Right,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 3] = [Side::Left, Side::Right, Side::Bottom];

    pub fn key(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Bottom => "bottom",
        }
    }

    pub fn parse(key: &str) -> Option<Side> {
        Side::ALL.into_iter().find(|s| s.key() == key)
    }

    /// 4.8.2: 260 wide, 648 wide (80 terminal columns), 320 high.
    pub fn default_size(self) -> f32 {
        match self {
            Side::Left => 260.,
            Side::Right => 648.,
            Side::Bottom => 320.,
        }
    }

    /// Two groups in a sidebar stack top and bottom; in the bottom panel
    /// they sit side by side (4.8.2).
    pub fn direction(self) -> Direction {
        match self {
            Side::Left | Side::Right => Direction::Column,
            Side::Bottom => Direction::Row,
        }
    }
}

/// An area (4.8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Side(Side),
    Page,
}

impl Place {
    pub fn key(self) -> &'static str {
        match self {
            Place::Side(side) => side.key(),
            Place::Page => "page",
        }
    }

    pub fn parse(key: &str) -> Option<Place> {
        if key == "page" {
            return Some(Place::Page);
        }
        Side::parse(key).map(Place::Side)
    }

    /// 4.8.2: two groups in a side area, three in the page area.
    pub fn max_groups(self) -> usize {
        match self {
            Place::Side(_) => 2,
            Place::Page => 3,
        }
    }
}

/// How groups share an area: `Row` side by side, `Column` stacked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Row,
    Column,
}

/// A page tab, as the shell knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(pub u64);

/// What a group holds: a page (page area only) or a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Page(TabId),
    View(ViewId),
}

/// A set of tabs, one of them active (4.8.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl Group {
    fn of(tab: Tab) -> Group {
        Group {
            tabs: vec![tab],
            active: 0,
        }
    }

    pub fn active_tab(&self) -> Option<Tab> {
        self.tabs.get(self.active).copied()
    }
}

/// An area's state. For the page area `shown` is always true and `size`
/// unused.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub shown: bool,
    pub size: f32,
    pub direction: Direction,
    pub groups: Vec<Group>,
    /// Each group's share of the area, summing to 1.
    pub shares: Vec<f32>,
}

impl Area {
    fn side(side: Side, shown: bool, groups: Vec<Group>) -> Area {
        let n = groups.len();
        Area {
            shown,
            size: side.default_size(),
            direction: side.direction(),
            groups,
            shares: equal(n),
        }
    }

    /// The active view of every group.
    pub fn active_views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.groups.iter().filter_map(|g| match g.active_tab() {
            Some(Tab::View(v)) => Some(v),
            _ => None,
        })
    }
}

/// Equal shares for `n` groups.
pub fn equal(n: usize) -> Vec<f32> {
    vec![1. / n.max(1) as f32; n]
}

/// Where a tab is, or was (4.8.11, rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub place: Place,
    pub group: usize,
    pub index: usize,
}

/// An edge of a group, where a drop splits it (4.8.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub fn direction(self) -> Direction {
        match self {
            Edge::Left | Edge::Right => Direction::Row,
            Edge::Top | Edge::Bottom => Direction::Column,
        }
    }

    /// The new group goes after the old one.
    pub fn after(self) -> bool {
        matches!(self, Edge::Right | Edge::Bottom)
    }
}

/// Where a moved tab lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Into an existing group, at `index` (the end when `None`).
    Into {
        place: Place,
        group: usize,
        index: Option<usize>,
    },
    /// A new group on `edge` of `group`.
    Split {
        place: Place,
        group: usize,
        edge: Edge,
    },
    /// A side area as a whole: the end of its last group (a new group when
    /// it has none), the area shown. Drops on a hidden area's strip and
    /// every move by command.
    Area(Side),
}

/// What a view menu, a view tab or the palette asks of a view (4.8.4,
/// 4.8.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewAction {
    Show,
    Hide,
    MoveTo(Place),
    SplitOff,
}

/// Why a change did not happen. Nothing is changed in either case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The tab would land where it already is (4.8.7).
    NoChange,
    /// The move breaks a rule of 4.8 (a page out of the page area, a third
    /// group in a sidebar, the last page closed).
    Invalid,
}

/// The whole layout of a window (4.8.12).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub left: Area,
    pub right: Area,
    pub bottom: Area,
    pub page: Area,
    /// The focused page group (4.8.8).
    pub focused: usize,
    /// The last place of every hidden view.
    pub last: BTreeMap<ViewId, Spot>,
}

impl Layout {
    /// 4.8.13: Projects alone on the left; Terminal, Chat and Agent browser
    /// in the hidden right sidebar (`chat`: this build has the web views);
    /// nothing below; one page group holding `pages`.
    pub fn defaults(pages: Vec<TabId>, chat: bool) -> Layout {
        let mut right = vec![Tab::View(ViewId::Terminal)];
        if chat {
            right.push(Tab::View(ViewId::Chat));
            right.push(Tab::View(ViewId::AgentBrowser));
        }
        let page_group = Group {
            tabs: pages.into_iter().map(Tab::Page).collect(),
            active: 0,
        };
        Layout {
            left: Area::side(
                Side::Left,
                true,
                vec![Group::of(Tab::View(ViewId::Projects))],
            ),
            right: Area::side(
                Side::Right,
                false,
                vec![Group {
                    tabs: right,
                    active: 0,
                }],
            ),
            bottom: Area::side(Side::Bottom, false, Vec::new()),
            page: Area {
                shown: true,
                size: 0.,
                direction: Direction::Row,
                groups: vec![page_group],
                shares: vec![1.],
            },
            focused: 0,
            last: BTreeMap::new(),
        }
    }

    pub fn area(&self, place: Place) -> &Area {
        match place {
            Place::Side(Side::Left) => &self.left,
            Place::Side(Side::Right) => &self.right,
            Place::Side(Side::Bottom) => &self.bottom,
            Place::Page => &self.page,
        }
    }

    pub fn area_mut(&mut self, place: Place) -> &mut Area {
        match place {
            Place::Side(Side::Left) => &mut self.left,
            Place::Side(Side::Right) => &mut self.right,
            Place::Side(Side::Bottom) => &mut self.bottom,
            Place::Page => &mut self.page,
        }
    }

    pub fn side(&self, side: Side) -> &Area {
        self.area(Place::Side(side))
    }

    pub fn side_mut(&mut self, side: Side) -> &mut Area {
        self.area_mut(Place::Side(side))
    }

    pub const PLACES: [Place; 4] = [
        Place::Side(Side::Left),
        Place::Page,
        Place::Side(Side::Bottom),
        Place::Side(Side::Right),
    ];

    /// Where `tab` is.
    pub fn locate(&self, tab: Tab) -> Option<Spot> {
        for place in Self::PLACES {
            for (group, g) in self.area(place).groups.iter().enumerate() {
                if let Some(index) = g.tabs.iter().position(|t| *t == tab) {
                    return Some(Spot {
                        place,
                        group,
                        index,
                    });
                }
            }
        }
        None
    }

    /// Every page tab, in group order.
    pub fn page_tabs(&self) -> Vec<TabId> {
        self.page
            .groups
            .iter()
            .flat_map(|g| g.tabs.iter())
            .filter_map(|t| match t {
                Tab::Page(id) => Some(*id),
                Tab::View(_) => None,
            })
            .collect()
    }

    /// The active tab of the focused page group.
    pub fn focused_tab(&self) -> Option<Tab> {
        self.page.groups.get(self.focused)?.active_tab()
    }

    /// The page the focused group last showed: its active tab when that is
    /// a page, else the group's first page, else any page (the breadcrumb
    /// keeps the last page under a view tab, 4.8.8).
    pub fn focused_page(&self) -> Option<TabId> {
        if let Some(Tab::Page(id)) = self.focused_tab() {
            return Some(id);
        }
        let in_group = self.page.groups.get(self.focused).and_then(|g| {
            g.tabs.iter().find_map(|t| match t {
                Tab::Page(id) => Some(*id),
                Tab::View(_) => None,
            })
        });
        in_group.or_else(|| self.page_tabs().first().copied())
    }

    /// Whether `view` is on screen in the model: in a shown area, the active
    /// tab of its group.
    pub fn is_visible(&self, view: ViewId) -> bool {
        let Some(spot) = self.locate(Tab::View(view)) else {
            return false;
        };
        let area = self.area(spot.place);
        area.shown && area.groups[spot.group].active == spot.index
    }

    /// Whether `view` sits in any group (shown or not).
    pub fn is_placed(&self, view: ViewId) -> bool {
        self.locate(Tab::View(view)).is_some()
    }

    // ---- primitive edits ----

    /// Take the tab at `spot` out. A group left empty is removed (its share
    /// going to its neighbours); a side area left with no group hides.
    fn take(&mut self, spot: Spot) -> Tab {
        let page = spot.place == Place::Page;
        let area = self.area_mut(spot.place);
        let group = &mut area.groups[spot.group];
        let tab = group.tabs.remove(spot.index);
        if group.active > spot.index || group.active >= group.tabs.len() {
            group.active = group.active.saturating_sub(1);
        }
        let emptied = group.tabs.is_empty();
        if emptied && (!page || area.groups.len() > 1) {
            area.groups.remove(spot.group);
            area.shares.remove(spot.group);
            normalize(&mut area.shares);
            if !page && area.groups.is_empty() {
                area.shown = false;
            }
            if page {
                if self.focused > spot.group {
                    self.focused -= 1;
                }
                self.focused = self.focused.min(self.page.groups.len() - 1);
            }
        }
        tab
    }

    /// Put `tab` into `group` of `place` at `index` (clamped), active.
    fn put(&mut self, place: Place, group: usize, index: Option<usize>, tab: Tab) -> Spot {
        let area = self.area_mut(place);
        let g = &mut area.groups[group];
        let index = index.unwrap_or(g.tabs.len()).min(g.tabs.len());
        g.tabs.insert(index, tab);
        g.active = index;
        area.shown = true;
        Spot {
            place,
            group,
            index,
        }
    }

    /// A new group holding `tab`, on `edge` of `group`, sized half of it.
    fn split_with(&mut self, place: Place, group: usize, edge: Edge, tab: Tab) -> Spot {
        let page = place == Place::Page;
        let area = self.area_mut(place);
        if area.groups.len() == 1 {
            area.direction = edge.direction();
        }
        let at = if edge.after() { group + 1 } else { group };
        let half = area.shares[group] / 2.;
        area.shares[group] = half;
        area.shares.insert(at, half);
        area.groups.insert(at, Group::of(tab));
        area.shown = true;
        if page && self.focused >= at {
            self.focused += 1;
        }
        Spot {
            place,
            group: at,
            index: 0,
        }
    }

    /// Whether `edge` of `group` in `place` may take a new group by the
    /// rules of 4.8.7 that do not depend on pixels: the group count and
    /// the direction. `leaving` is the tab that moves, when it comes from
    /// this area (its group may close).
    pub fn may_split(&self, place: Place, group: usize, edge: Edge, leaving: Option<Spot>) -> bool {
        let area = self.area(place);
        let Some(g) = area.groups.get(group) else {
            return false;
        };
        // A group cannot be split by its only tab.
        if let Some(from) = leaving
            && from.place == place
            && from.group == group
            && g.tabs.len() == 1
        {
            return false;
        }
        let closes = leaving.is_some_and(|from| {
            from.place == place && from.group != group && area.groups[from.group].tabs.len() == 1
        });
        let count = area.groups.len() - usize::from(closes);
        if count >= place.max_groups() {
            return false;
        }
        match place {
            Place::Side(side) => edge.direction() == side.direction(),
            Place::Page => count <= 1 || edge.direction() == area.direction,
        }
    }

    // ---- moves (4.8.7, 4.8.8) ----

    /// Move `tab` to `target`. Views go anywhere; pages stay in the page
    /// area (and Inbox or Knowledge cross with [`Layout::view_to_page`] and
    /// [`Layout::page_to_view`]). The moved tab is active where it lands;
    /// in the page area its group becomes the focused one.
    pub fn move_tab(&mut self, tab: Tab, target: Target) -> Result<Spot, Refused> {
        let from = self.locate(tab);
        let target_place = match target {
            Target::Into { place, .. } | Target::Split { place, .. } => place,
            Target::Area(side) => Place::Side(side),
        };
        match tab {
            Tab::Page(_) if target_place != Place::Page => return Err(Refused::Invalid),
            Tab::View(v) if v.is_page_kind() && target_place == Place::Page => {
                return Err(Refused::Invalid);
            }
            _ => {}
        }
        self.check(target, from)?;
        let mut next = self.clone();
        if let Some(from) = from {
            next.take(from);
        }
        let target = adjust(target, from, &self.area(target_place).groups, &next);
        let spot = next.land(target, tab);
        if let Tab::View(v) = tab {
            next.last.remove(&v);
        }
        *self = next;
        Ok(spot)
    }

    /// Refuse a target that does not exist, breaks a rule, or is where the
    /// tab already is.
    fn check(&self, target: Target, from: Option<Spot>) -> Result<(), Refused> {
        match target {
            Target::Into {
                place,
                group,
                index,
            } => {
                let area = self.area(place);
                let Some(g) = area.groups.get(group) else {
                    return Err(Refused::Invalid);
                };
                if let Some(from) = from
                    && from.place == place
                    && from.group == group
                {
                    let end = g.tabs.len();
                    let to = index.unwrap_or(end).min(end);
                    // Before or just after itself: the same position.
                    let same = to == from.index || to == from.index + 1;
                    if same || (index.is_none() && from.index + 1 == end) || g.tabs.len() == 1 {
                        return Err(Refused::NoChange);
                    }
                }
                Ok(())
            }
            Target::Split { place, group, edge } => {
                if self.may_split(place, group, edge, from) {
                    Ok(())
                } else {
                    Err(Refused::Invalid)
                }
            }
            Target::Area(side) => {
                if let Some(from) = from
                    && from.place == Place::Side(side)
                {
                    let area = self.side(side);
                    let last = area.groups.len() - 1;
                    if from.group == last && from.index + 1 == area.groups[last].tabs.len() {
                        return Err(Refused::NoChange);
                    }
                }
                Ok(())
            }
        }
    }

    fn land(&mut self, target: Target, tab: Tab) -> Spot {
        let spot = match target {
            Target::Into {
                place,
                group,
                index,
            } => self.put(place, group, index, tab),
            Target::Split { place, group, edge } => self.split_with(place, group, edge, tab),
            Target::Area(side) => {
                let place = Place::Side(side);
                if self.side(side).groups.is_empty() {
                    let area = self.side_mut(side);
                    area.groups.push(Group::of(tab));
                    area.shares = vec![1.];
                    area.shown = true;
                    Spot {
                        place,
                        group: 0,
                        index: 0,
                    }
                } else {
                    let last = self.side(side).groups.len() - 1;
                    self.put(place, last, None, tab)
                }
            }
        };
        if spot.place == Place::Page {
            self.focused = spot.group;
        }
        spot
    }

    /// The Inbox or Knowledge view moved into the page area: it becomes the
    /// page `tab` (the shell opens the active project's page in it), where
    /// it was dropped (4.8.3).
    pub fn view_to_page(
        &mut self,
        view: ViewId,
        tab: TabId,
        target: Target,
    ) -> Result<Spot, Refused> {
        if !view.is_page_kind() {
            return Err(Refused::Invalid);
        }
        let place = match target {
            Target::Into { place, .. } | Target::Split { place, .. } => place,
            Target::Area(_) => return Err(Refused::Invalid),
        };
        if place != Place::Page {
            return Err(Refused::Invalid);
        }
        let from = self.locate(Tab::View(view));
        self.check(target, None)?;
        let mut next = self.clone();
        if let Some(from) = from {
            next.take(from);
        }
        let spot = next.land(target, Tab::Page(tab));
        next.last.remove(&view);
        *self = next;
        Ok(spot)
    }

    /// The Inbox or Knowledge page `tab` dragged out of the page area: the
    /// page closes and the view lands at `target` (moved there if it was
    /// already shown elsewhere: a view is never duplicated). The last page
    /// of the window never leaves (4.8.8): it stays open and the view
    /// lands beside it, as the palette's move does.
    pub fn page_to_view(
        &mut self,
        tab: TabId,
        view: ViewId,
        target: Target,
    ) -> Result<Spot, Refused> {
        if !view.is_page_kind() {
            return Err(Refused::Invalid);
        }
        let last_page = self.page_tabs().len() <= 1;
        let place = match target {
            Target::Into { place, .. } | Target::Split { place, .. } => place,
            Target::Area(side) => Place::Side(side),
        };
        if place == Place::Page {
            return Err(Refused::Invalid);
        }
        let Some(page_spot) = self.locate(Tab::Page(tab)) else {
            return Err(Refused::Invalid);
        };
        let from = self.locate(Tab::View(view));
        self.check(target, from)?;
        let mut next = self.clone();
        if !last_page {
            next.take(page_spot);
        }
        if let Some(from) = next.locate(Tab::View(view)) {
            next.take(from);
        }
        let target = adjust(target, from, &self.area(place).groups, &next);
        let spot = next.land(target, Tab::View(view));
        next.last.remove(&view);
        *self = next;
        Ok(spot)
    }

    /// Open a new page tab next to the focused group's active tab (a new
    /// tab, 4.4), active.
    pub fn add_page(&mut self, tab: TabId) -> Spot {
        let group = self.focused.min(self.page.groups.len() - 1);
        let index = self.page.groups[group].active + 1;
        self.put(Place::Page, group, Some(index), Tab::Page(tab))
    }

    /// Close a page tab. The last page of the window stays (4.8.8).
    pub fn close_page(&mut self, tab: TabId) -> Result<(), Refused> {
        if self.page_tabs().len() <= 1 {
            return Err(Refused::Invalid);
        }
        let spot = self.locate(Tab::Page(tab)).ok_or(Refused::Invalid)?;
        self.take(spot);
        Ok(())
    }

    /// Make the tab at `spot` its group's active tab; a page group becomes
    /// the focused one.
    pub fn activate(&mut self, spot: Spot) {
        let area = self.area_mut(spot.place);
        if let Some(g) = area.groups.get_mut(spot.group)
            && spot.index < g.tabs.len()
        {
            g.active = spot.index;
        }
        if spot.place == Place::Page && spot.group < self.page.groups.len() {
            self.focused = spot.group;
        }
    }

    // ---- showing and hiding (4.8.5) ----

    /// Hide `view`: it leaves its group, its place is remembered, and its
    /// area hides if it was the last view there.
    pub fn hide_view(&mut self, view: ViewId) -> bool {
        let Some(spot) = self.locate(Tab::View(view)) else {
            return false;
        };
        self.take(spot);
        self.last.insert(view, spot);
        true
    }

    /// Show `view` where it was last (4.8.11, rule 4): its area shown, the
    /// view active in its group. `None` for Inbox and Knowledge while they
    /// are pages (the shell opens the page instead).
    pub fn show_view(&mut self, view: ViewId) -> Option<Spot> {
        if let Some(spot) = self.locate(Tab::View(view)) {
            self.activate(spot);
            self.area_mut(spot.place).shown = true;
            return Some(spot);
        }
        let last = self.last.remove(&view).or_else(|| default_spot(view))?;
        Some(self.restore(view, last))
    }

    /// Put `view` back at `spot`, or as near it as the layout now allows:
    /// the same area, the nearest group, the same index or the end.
    fn restore(&mut self, view: ViewId, spot: Spot) -> Spot {
        let place = spot.place;
        let groups = self.area(place).groups.len();
        let landed = if groups == 0 {
            let area = self.area_mut(place);
            area.groups.push(Group::of(Tab::View(view)));
            area.shares = vec![1.];
            area.shown = true;
            Spot {
                place,
                group: 0,
                index: 0,
            }
        } else {
            let group = spot.group.min(groups - 1);
            self.put(place, group, Some(spot.index), Tab::View(view))
        };
        if place == Place::Page {
            self.focused = landed.group;
        }
        landed
    }

    /// Show or hide a side area with everything in it.
    pub fn set_area_shown(&mut self, side: Side, shown: bool) -> bool {
        let area = self.side_mut(side);
        let changed = area.shown != shown;
        area.shown = shown;
        changed
    }

    /// "Split from group": the view leaves its group for a new one beside
    /// it (after it, along the area's direction).
    pub fn split_off(&mut self, view: ViewId) -> Result<Spot, Refused> {
        let spot = self.locate(Tab::View(view)).ok_or(Refused::Invalid)?;
        let area = self.area(spot.place);
        let edge = match (spot.place, area.direction, area.groups.len()) {
            (Place::Side(side), _, _) => after_edge(side.direction()),
            (Place::Page, dir, n) if n > 1 => after_edge(dir),
            (Place::Page, _, _) => Edge::Right,
        };
        self.move_tab(
            Tab::View(view),
            Target::Split {
                place: spot.place,
                group: spot.group,
                edge,
            },
        )
    }

    /// Whether "Split from group" applies (4.8.4): the view shares its group
    /// and the area may take another group.
    pub fn can_split_off(&self, view: ViewId) -> bool {
        let Some(spot) = self.locate(Tab::View(view)) else {
            return false;
        };
        let area = self.area(spot.place);
        let edge = match spot.place {
            Place::Side(side) => after_edge(side.direction()),
            Place::Page if area.groups.len() > 1 => after_edge(area.direction),
            Place::Page => Edge::Right,
        };
        area.groups[spot.group].tabs.len() > 1
            && self.may_split(spot.place, spot.group, edge, Some(spot))
    }

    // ---- page groups (4.8.8) ----

    /// "Split right" / "Split down": the focused group's active tab moves
    /// into a new group after it.
    pub fn split_active(&mut self, direction: Direction) -> Result<Spot, Refused> {
        let group = self.focused;
        let tab = self.page.groups[group]
            .active_tab()
            .ok_or(Refused::Invalid)?;
        self.move_tab(
            tab,
            Target::Split {
                place: Place::Page,
                group,
                edge: after_edge(direction),
            },
        )
    }

    /// Whether "Split right" or "Split down" may run (pixels aside).
    pub fn can_split_active(&self, direction: Direction) -> bool {
        let group = self.focused;
        let g = &self.page.groups[group];
        let Some(tab) = g.active_tab() else {
            return false;
        };
        g.tabs.len() >= 2
            && self.may_split(Place::Page, group, after_edge(direction), self.locate(tab))
    }

    /// "Move tab to next group" (`step` 1) or previous (-1): the focused
    /// group's active tab goes to the end of that group, which takes focus.
    /// At an edge group it does nothing.
    pub fn move_active_to_group(&mut self, step: isize) -> Result<Spot, Refused> {
        let to = self.focused as isize + step;
        if to < 0 || to as usize >= self.page.groups.len() {
            return Err(Refused::NoChange);
        }
        let tab = self.page.groups[self.focused]
            .active_tab()
            .ok_or(Refused::Invalid)?;
        self.move_tab(
            tab,
            Target::Into {
                place: Place::Page,
                group: to as usize,
                index: None,
            },
        )
    }

    /// "Join groups": every tab gathered into the focused group, in group
    /// order; the focused group's active tab stays active.
    pub fn join_groups(&mut self) -> Result<(), Refused> {
        if self.page.groups.len() < 2 {
            return Err(Refused::NoChange);
        }
        let keep = self.focused_tab();
        let tabs: Vec<Tab> = self
            .page
            .groups
            .iter()
            .flat_map(|g| g.tabs.clone())
            .collect();
        let active = keep
            .and_then(|k| tabs.iter().position(|t| *t == k))
            .unwrap_or(0);
        self.page.groups = vec![Group { tabs, active }];
        self.page.shares = vec![1.];
        self.focused = 0;
        Ok(())
    }

    // ---- reset (4.8.13) ----

    /// Every area, view, size and split back to the defaults; the open
    /// pages kept in one group, in group order, the focused group's active
    /// page staying active.
    pub fn reset(&mut self, chat: bool) {
        let keep = self.focused_page();
        let pages = self.page_tabs();
        let active = keep
            .and_then(|k| pages.iter().position(|p| *p == k))
            .unwrap_or(0);
        let mut next = Layout::defaults(pages, chat);
        next.page.groups[0].active = active;
        *self = next;
    }

    // ---- repair (4.8.11, rule 7; 4.8.12) ----

    /// Make the layout whole: each view at most once, no unknown page tab,
    /// every live page tab somewhere, no empty group, the group limits, the
    /// active indexes and shares in range, at least one page group. `live`
    /// is every page tab the shell holds; `chat` whether this build has the
    /// web views (elsewhere the chat and the agent browser keep only their
    /// last place).
    pub fn repair(&mut self, live: &[TabId], chat: bool) {
        for web in [ViewId::Chat, ViewId::AgentBrowser] {
            if !chat && let Some(spot) = self.locate(Tab::View(web)) {
                // Ignored, not removed: where it was is kept as its last
                // place, so a shared file does not lose it.
                self.last.insert(web, spot);
            }
        }
        let mut seen_views = Vec::new();
        let mut seen_pages = Vec::new();
        for place in Self::PLACES {
            let page = place == Place::Page;
            let area = self.area_mut(place);
            for g in &mut area.groups {
                g.tabs.retain(|t| match t {
                    Tab::View(v) => {
                        let keep = !seen_views.contains(v)
                            && !(page && v.is_page_kind())
                            && (chat || !v.is_web());
                        if keep {
                            seen_views.push(*v);
                        }
                        keep
                    }
                    Tab::Page(id) => {
                        let keep = page && live.contains(id) && !seen_pages.contains(id);
                        if keep {
                            seen_pages.push(*id);
                        }
                        keep
                    }
                });
            }
            area.groups.retain(|g| !g.tabs.is_empty());
            let max = place.max_groups();
            if area.groups.len() > max {
                let extra: Vec<Tab> = area.groups.drain(max..).flat_map(|g| g.tabs).collect();
                area.groups[max - 1].tabs.extend(extra);
            }
            for g in &mut area.groups {
                g.active = g.active.min(g.tabs.len() - 1);
            }
            if area.shares.len() != area.groups.len()
                || area.shares.iter().any(|s| !s.is_finite() || *s <= 0.)
            {
                area.shares = equal(area.groups.len());
            }
            normalize(&mut area.shares);
            if !area.size.is_finite() {
                area.size = 0.;
            }
            if let Place::Side(side) = place {
                area.direction = side.direction();
                area.size = if area.size <= 0. {
                    side.default_size()
                } else {
                    area.size.clamp(SIZE_MIN, SIZE_MAX)
                };
            }
        }
        if self.page.groups.len() < 2 {
            self.page.direction = Direction::Row;
        }
        self.page.shown = true;
        // Live pages the layout does not name go to the focused group.
        let missing: Vec<TabId> = live
            .iter()
            .filter(|id| !seen_pages.contains(id))
            .copied()
            .collect();
        if self.page.groups.is_empty() {
            self.page.groups.push(Group {
                tabs: Vec::new(),
                active: 0,
            });
            self.page.shares = vec![1.];
        }
        self.focused = self.focused.min(self.page.groups.len() - 1);
        let focused = self.focused;
        self.page.groups[focused]
            .tabs
            .extend(missing.into_iter().map(Tab::Page));
        // A placed view has no last place; a stray one is dropped.
        let placed: Vec<ViewId> = seen_views;
        self.last.retain(|v, spot| {
            !placed.contains(v) && !(spot.place == Place::Page && v.is_page_kind())
        });
    }
}

/// Sizes outside this range are clamped on load (4.8.12).
const SIZE_MIN: f32 = 120.;
const SIZE_MAX: f32 = 4000.;

fn after_edge(direction: Direction) -> Edge {
    match direction {
        Direction::Row => Edge::Right,
        Direction::Column => Edge::Bottom,
    }
}

/// 4.8.13: where a view goes when it was never placed. Inbox and Knowledge
/// have none: they are pages.
fn default_spot(view: ViewId) -> Option<Spot> {
    let (place, index) = match view {
        ViewId::Projects => (Place::Side(Side::Left), 0),
        ViewId::Terminal => (Place::Side(Side::Right), 0),
        ViewId::Chat => (Place::Side(Side::Right), 1),
        ViewId::AgentBrowser => (Place::Side(Side::Right), 2),
        ViewId::Inbox | ViewId::Knowledge => return None,
    };
    Some(Spot {
        place,
        group: 0,
        index,
    })
}

fn normalize(shares: &mut [f32]) {
    let sum: f32 = shares.iter().sum();
    if sum > 0. {
        for s in shares.iter_mut() {
            *s /= sum;
        }
    }
}

/// A target named before the moved tab left its place, as it reads after:
/// groups after an emptied one shift down, an index after the tab's old
/// position in the same group shifts down.
fn adjust(target: Target, from: Option<Spot>, before: &[Group], after: &Layout) -> Target {
    let Some(from) = from else { return target };
    let place_of = |t: &Target| match t {
        Target::Into { place, .. } | Target::Split { place, .. } => Some(*place),
        Target::Area(_) => None,
    };
    if place_of(&target) != Some(from.place) {
        return target;
    }
    let emptied =
        before[from.group].tabs.len() == 1 && after.area(from.place).groups.len() < before.len();
    let shift = |group: usize| {
        if emptied && group > from.group {
            group - 1
        } else {
            group
        }
    };
    match target {
        Target::Into {
            place,
            group,
            index,
        } => {
            let index = match index {
                Some(i) if group == from.group && i > from.index => Some(i - 1),
                other => other,
            };
            Target::Into {
                place,
                group: shift(group),
                index,
            }
        }
        Target::Split { place, group, edge } => Target::Split {
            place,
            group: shift(group),
            edge,
        },
        Target::Area(side) => Target::Area(side),
    }
}

// ---- persistence (4.8.12) ----

/// The layout as `preferences.json` holds it: the layout, and each page
/// tab's route key (a record id or a fixed page key, never a title).
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub layout: Layout,
    pub pages: Vec<(TabId, String)>,
}

/// The schema of the `layout` key.
const STORED_VERSION: u64 = 1;
/// The prefix of a view tab in the page area.
const VIEW_TAB: &str = "view:";

impl Saved {
    /// The `layout` value of the preferences file.
    pub fn to_value(&self) -> Value {
        let key_of = |tab: &Tab| -> Option<String> {
            match tab {
                Tab::View(v) => Some(format!("{VIEW_TAB}{}", v.key())),
                Tab::Page(id) => self
                    .pages
                    .iter()
                    .find(|(p, _)| p == id)
                    .map(|(_, k)| k.clone()),
            }
        };
        let groups = |area: &Area, field: &str| -> Value {
            Value::Array(
                area.groups
                    .iter()
                    .map(|g| {
                        let tabs: Vec<Value> = g
                            .tabs
                            .iter()
                            .filter_map(|t| match (field, t) {
                                ("views", Tab::View(v)) => Some(Value::from(v.key())),
                                ("views", Tab::Page(_)) => None,
                                _ => key_of(t).map(Value::from),
                            })
                            .collect();
                        json!({ field: tabs, "active": g.active })
                    })
                    .collect(),
            )
        };
        let rounded = |v: f32| f64::from((v * 1000.).round() / 1000.);
        let mut areas = Map::new();
        for side in Side::ALL {
            let area = self.layout.side(side);
            let mut a = Map::new();
            a.insert("shown".into(), Value::from(area.shown));
            a.insert("size".into(), Value::from(area.size.round() as i64));
            a.insert("groups".into(), groups(area, "views"));
            if area.groups.len() > 1 {
                a.insert(
                    "shares".into(),
                    Value::from(area.shares.iter().map(|s| rounded(*s)).collect::<Vec<_>>()),
                );
            }
            areas.insert(side.key().into(), Value::Object(a));
        }
        let page = &self.layout.page;
        let mut last = Map::new();
        for (view, spot) in &self.layout.last {
            last.insert(
                view.key().into(),
                json!({ "area": spot.place.key(), "group": spot.group, "index": spot.index }),
            );
        }
        let direction = match page.direction {
            Direction::Row => "row",
            Direction::Column => "column",
        };
        json!({
            "v": STORED_VERSION,
            "areas": areas,
            "page": {
                "direction": direction,
                "shares": page.shares.iter().map(|s| rounded(*s)).collect::<Vec<_>>(),
                "focused": self.layout.focused,
                "groups": groups(page, "tabs"),
            },
            "last": last,
        })
    }

    /// Read a `layout` value, forgivingly (4.8.12): unknown views and page
    /// keys that `is_page_key` refuses are dropped, a view named twice
    /// keeps its first place, sizes are clamped, empty groups removed.
    /// `None` when nothing of it can be read (the defaults then apply).
    pub fn from_value(value: &Value, is_page_key: impl Fn(&str) -> bool) -> Option<Saved> {
        let root = value.as_object()?;
        if root.get("v").and_then(Value::as_u64) != Some(STORED_VERSION) {
            return None;
        }
        let mut pages: Vec<(TabId, String)> = Vec::new();
        let mut layout = Layout::defaults(Vec::new(), true);
        let areas = root.get("areas").and_then(Value::as_object);
        for side in Side::ALL {
            let Some(a) = areas
                .and_then(|a| a.get(side.key()))
                .and_then(Value::as_object)
            else {
                continue;
            };
            let area = layout.side_mut(side);
            if let Some(shown) = a.get("shown").and_then(Value::as_bool) {
                area.shown = shown;
            }
            if let Some(size) = a.get("size").and_then(Value::as_f64) {
                area.size = size as f32;
            }
            area.groups = read_groups(a.get("groups"), "views", |s| {
                ViewId::parse(s).map(Tab::View)
            });
            area.shares = read_shares(a.get("shares"));
        }
        if let Some(p) = root.get("page").and_then(Value::as_object) {
            layout.page.direction = match p.get("direction").and_then(Value::as_str) {
                Some("column") => Direction::Column,
                _ => Direction::Row,
            };
            layout.page.shares = read_shares(p.get("shares"));
            layout.focused = p.get("focused").and_then(Value::as_u64).unwrap_or(0) as usize;
            let mut next = 1;
            layout.page.groups = read_groups(p.get("groups"), "tabs", |s| {
                if let Some(view) = s.strip_prefix(VIEW_TAB) {
                    return ViewId::parse(view).map(Tab::View);
                }
                if !is_page_key(s) || pages.iter().any(|(_, k)| k == s) {
                    return None;
                }
                let id = TabId(next);
                next += 1;
                pages.push((id, s.to_owned()));
                Some(Tab::Page(id))
            });
        }
        if let Some(last) = root.get("last").and_then(Value::as_object) {
            for (key, spot) in last {
                let (Some(view), Some(spot)) = (ViewId::parse(key), spot.as_object()) else {
                    continue;
                };
                let place = spot
                    .get("area")
                    .and_then(Value::as_str)
                    .and_then(Place::parse);
                let number = |k: &str| spot.get(k).and_then(Value::as_u64).map(|n| n as usize);
                if let (Some(place), Some(group), Some(index)) =
                    (place, number("group"), number("index"))
                {
                    layout.last.insert(
                        view,
                        Spot {
                            place,
                            group,
                            index,
                        },
                    );
                }
            }
        }
        let live: Vec<TabId> = pages.iter().map(|(id, _)| *id).collect();
        layout.repair(&live, true);
        Some(Saved { layout, pages })
    }
}

fn read_shares(value: Option<&Value>) -> Vec<f32> {
    value
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_f64)
                .map(|s| s as f32)
                .collect()
        })
        .unwrap_or_default()
}

fn read_groups(
    value: Option<&Value>,
    field: &str,
    mut tab: impl FnMut(&str) -> Option<Tab>,
) -> Vec<Group> {
    let Some(groups) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    groups
        .iter()
        .filter_map(Value::as_object)
        .map(|g| {
            let tabs: Vec<Tab> = g
                .get(field)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .filter_map(&mut tab)
                        .collect()
                })
                .unwrap_or_default();
            let active = g.get("active").and_then(Value::as_u64).unwrap_or(0) as usize;
            Group { tabs, active }
        })
        .collect()
}

/// The fields M7 wrote before the layout existed (4.8.12, Migration).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Old {
    pub sidebar_hidden: bool,
    pub sidebar_width: Option<u16>,
    pub terminal_width: Option<u16>,
    pub chat_width: Option<u16>,
    pub chat_bottom: bool,
    pub chat_height: Option<u16>,
    /// The open tabs when there were two or more, and the active one.
    pub tabs: Vec<String>,
    pub active_tab: usize,
    /// The page shown at quit (with a single tab).
    pub page: Option<String>,
}

impl Old {
    pub fn is_empty(&self) -> bool {
        *self == Old::default()
    }
}

/// The layout the M7 fields describe: the sidebar's width and visibility,
/// the terminal's (or the chat's) width as the right sidebar's, the chat
/// alone in the bottom panel when it was docked there, and the tabs as the
/// one page group.
pub fn migrate(old: &Old) -> Saved {
    let keys: Vec<String> = if old.tabs.is_empty() {
        old.page.iter().cloned().collect()
    } else {
        old.tabs.clone()
    };
    let pages: Vec<(TabId, String)> = keys
        .into_iter()
        .enumerate()
        .map(|(i, k)| (TabId(i as u64 + 1), k))
        .collect();
    let mut layout = Layout::defaults(pages.iter().map(|(id, _)| *id).collect(), true);
    layout.page.groups[0].active = if old.tabs.is_empty() {
        0
    } else {
        old.active_tab.min(pages.len().saturating_sub(1))
    };
    layout.left.shown = !old.sidebar_hidden;
    if let Some(w) = old.sidebar_width {
        layout.left.size = f32::from(w);
    }
    if let Some(w) = old.terminal_width.or(old.chat_width) {
        layout.right.size = f32::from(w);
    }
    if let Some(h) = old.chat_height {
        layout.bottom.size = f32::from(h);
    }
    if old.chat_bottom {
        layout.right.groups[0]
            .tabs
            .retain(|t| *t != Tab::View(ViewId::Chat));
        layout.bottom.groups = vec![Group::of(Tab::View(ViewId::Chat))];
        layout.bottom.shares = vec![1.];
    }
    let live: Vec<TabId> = pages.iter().map(|(id, _)| *id).collect();
    layout.repair(&live, true);
    Saved { layout, pages }
}

// ---- undo (4.8.11, rule 3) ----

/// Whether `Mod+Z` takes the newest layout step rather than the newest
/// write (interaction.md 6.4: whichever of the two is most recent).
pub fn layout_step_first(layout: Option<u64>, write: Option<u64>) -> bool {
    layout.is_some() && layout > write
}

/// 4.8.11: the last 50 steps per window, for the session.
const UNDO_DEPTH: usize = 50;
/// Keyboard resize steps within one second are one step.
const COALESCE_MS: u64 = 1000;

#[derive(Clone, Debug)]
struct UndoStep {
    before: Layout,
    /// The order among every undo step of the window, writes included.
    seq: u64,
    coalesce: Option<(&'static str, u64)>,
}

/// The layout steps `Mod+Z` and "Undo layout change" take back.
#[derive(Clone, Debug, Default)]
pub struct UndoLog {
    steps: Vec<UndoStep>,
    /// The layouts undone, newest last, for `Mod+Shift+Z`. A new change
    /// clears them.
    redo: Vec<UndoStep>,
}

impl UndoLog {
    /// Record the layout as it was `before` a change. A change with the
    /// same `coalesce` key as the newest step, within a second of it
    /// (`(key, now in ms)`), joins that step.
    pub fn record(&mut self, before: Layout, seq: u64, coalesce: Option<(&'static str, u64)>) {
        self.redo.clear();
        if let (Some((key, now)), Some(last)) = (coalesce, self.steps.last_mut())
            && let Some((last_key, at)) = last.coalesce
            && last_key == key
            && now.saturating_sub(at) <= COALESCE_MS
        {
            last.coalesce = Some((key, now));
            return;
        }
        self.steps.push(UndoStep {
            before,
            seq,
            coalesce,
        });
        if self.steps.len() > UNDO_DEPTH {
            self.steps.remove(0);
        }
    }

    /// The layout before the newest step, which leaves the log; `current`
    /// (the layout the step left) becomes the newest redo, ordered `seq`.
    pub fn undo(&mut self, current: Layout, seq: u64) -> Option<Layout> {
        let step = self.steps.pop()?;
        self.redo.push(UndoStep {
            before: current,
            seq,
            coalesce: None,
        });
        Some(step.before)
    }

    /// The layout the newest undo took back; `current` becomes an undo
    /// step again, ordered `seq`, without clearing the other redos.
    pub fn redo(&mut self, current: Layout, seq: u64) -> Option<Layout> {
        let step = self.redo.pop()?;
        self.steps.push(UndoStep {
            before: current,
            seq,
            coalesce: None,
        });
        Some(step.before)
    }

    /// The order of the newest step, to compare with the writes' undo.
    pub fn newest(&self) -> Option<u64> {
        self.steps.last().map(|s| s.seq)
    }

    /// The order of the newest redo, to compare with the writes' redo.
    pub fn newest_redo(&self) -> Option<u64> {
        self.redo.last().map(|s| s.seq)
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P1: TabId = TabId(1);
    const P2: TabId = TabId(2);
    const P3: TabId = TabId(3);
    const P4: TabId = TabId(4);

    fn base() -> Layout {
        Layout::defaults(vec![P1, P2, P3], true)
    }

    /// Every rule of 4.8.11 rule 7 and 4.8.12 that must hold after any
    /// change: each view at most once, never both placed and remembered,
    /// no empty group, the group limits, active indexes and shares in
    /// range, pages only in the page area, at least one page.
    fn assert_whole(l: &Layout) {
        let mut views = Vec::new();
        for place in Layout::PLACES {
            let area = l.area(place);
            assert!(area.groups.len() <= place.max_groups(), "{place:?}");
            assert_eq!(area.shares.len(), area.groups.len(), "{place:?}");
            if !area.groups.is_empty() {
                let sum: f32 = area.shares.iter().sum();
                assert!((sum - 1.).abs() < 1e-3, "{place:?} shares {sum}");
            }
            for g in &area.groups {
                assert!(!g.tabs.is_empty(), "{place:?} empty group");
                assert!(g.active < g.tabs.len());
                for t in &g.tabs {
                    match t {
                        Tab::View(v) => {
                            assert!(!views.contains(v), "{v:?} twice");
                            assert!(!(place == Place::Page && v.is_page_kind()));
                            views.push(*v);
                        }
                        Tab::Page(_) => assert_eq!(place, Place::Page),
                    }
                }
            }
        }
        for v in l.last.keys() {
            assert!(!views.contains(v), "{v:?} placed and remembered");
        }
        assert!(!l.page_tabs().is_empty());
        assert!(l.focused < l.page.groups.len());
    }

    fn into(place: Place, group: usize, index: Option<usize>) -> Target {
        Target::Into {
            place,
            group,
            index,
        }
    }

    const LEFT: Place = Place::Side(Side::Left);
    const RIGHT: Place = Place::Side(Side::Right);
    const BOTTOM: Place = Place::Side(Side::Bottom);

    #[test]
    fn the_defaults_are_the_workbench_of_4_2() {
        let l = base();
        assert_whole(&l);
        assert!(l.left.shown && !l.right.shown && !l.bottom.shown);
        assert_eq!(
            (l.left.size, l.right.size, l.bottom.size),
            (260., 648., 320.)
        );
        assert_eq!(
            l.right.groups[0].tabs,
            [
                Tab::View(ViewId::Terminal),
                Tab::View(ViewId::Chat),
                Tab::View(ViewId::AgentBrowser)
            ]
        );
        assert!(l.is_visible(ViewId::Projects));
        assert!(!l.is_visible(ViewId::Terminal), "its area is hidden");
        let no_chat = Layout::defaults(vec![P1], false);
        assert_eq!(no_chat.right.groups[0].tabs, [Tab::View(ViewId::Terminal)]);
    }

    #[test]
    fn hiding_a_view_remembers_its_place_and_showing_brings_it_back() {
        let mut l = base();
        assert!(l.show_view(ViewId::Chat).is_some());
        assert!(l.right.shown && l.is_visible(ViewId::Chat));
        assert!(l.hide_view(ViewId::Chat));
        assert_whole(&l);
        assert_eq!(
            l.last[&ViewId::Chat],
            Spot {
                place: RIGHT,
                group: 0,
                index: 1
            }
        );
        assert!(l.right.shown, "the terminal is still there");
        assert!(l.hide_view(ViewId::Terminal));
        assert!(l.hide_view(ViewId::AgentBrowser));
        assert!(!l.right.shown, "an area left empty hides");
        assert!(l.right.groups.is_empty());
        // Showing the chat again puts it back in the right sidebar, the
        // only area it was in (rule 4).
        let spot = l.show_view(ViewId::Chat).unwrap();
        assert_eq!(spot.place, RIGHT);
        assert!(l.right.shown);
        let spot = l.show_view(ViewId::Terminal).unwrap();
        assert_eq!((spot.place, spot.group, spot.index), (RIGHT, 0, 0));
        assert_eq!(
            l.right.groups[0].tabs,
            [Tab::View(ViewId::Terminal), Tab::View(ViewId::Chat)]
        );
        assert_whole(&l);
    }

    #[test]
    fn a_view_reappears_in_the_group_nearest_its_own() {
        let mut l = base();
        l.move_tab(
            Tab::View(ViewId::Chat),
            Target::Split {
                place: RIGHT,
                group: 0,
                edge: Edge::Bottom,
            },
        )
        .unwrap();
        assert_eq!(l.right.groups.len(), 2);
        l.hide_view(ViewId::Chat);
        assert_eq!(l.last[&ViewId::Chat].group, 1);
        assert_eq!(l.right.groups.len(), 1, "its group closed");
        let spot = l.show_view(ViewId::Chat).unwrap();
        assert_eq!((spot.place, spot.group), (RIGHT, 0));
        assert_whole(&l);
    }

    #[test]
    fn showing_a_view_in_a_hidden_area_shows_the_area() {
        let mut l = base();
        l.show_view(ViewId::Terminal);
        assert!(l.set_area_shown(Side::Right, false));
        assert!(!l.is_visible(ViewId::Terminal));
        assert!(
            l.is_placed(ViewId::Terminal),
            "hiding an area keeps its views"
        );
        l.show_view(ViewId::Chat);
        assert!(l.right.shown && l.is_visible(ViewId::Chat) && !l.is_visible(ViewId::Terminal));
    }

    #[test]
    fn inbox_and_knowledge_are_pages_until_moved() {
        let mut l = base();
        assert_eq!(
            l.show_view(ViewId::Inbox),
            None,
            "a page: the shell opens it"
        );
        assert_eq!(
            l.move_tab(Tab::View(ViewId::Inbox), into(Place::Page, 0, None)),
            Err(Refused::Invalid)
        );
        // The inbox page tab P2 dragged into the left sidebar's header.
        let spot = l
            .page_to_view(P2, ViewId::Inbox, into(LEFT, 0, None))
            .unwrap();
        assert_eq!((spot.place, spot.index), (LEFT, 1));
        assert_eq!(l.page_tabs(), [P1, P3], "the page closed");
        assert_whole(&l);
        // Hidden, then shown: back in the left sidebar.
        l.hide_view(ViewId::Inbox);
        assert_eq!(l.show_view(ViewId::Inbox).map(|s| s.place), Some(LEFT));
        // Back into the page area: it becomes a page where dropped.
        let spot = l
            .view_to_page(ViewId::Inbox, P4, into(Place::Page, 0, Some(1)))
            .unwrap();
        assert_eq!((spot.place, spot.index), (Place::Page, 1));
        assert!(!l.is_placed(ViewId::Inbox) && !l.last.contains_key(&ViewId::Inbox));
        assert_eq!(l.page_tabs(), [P1, P4, P3]);
        assert_whole(&l);
    }

    #[test]
    fn the_last_page_never_leaves_or_closes() {
        let mut l = Layout::defaults(vec![P1], true);
        // Moved out from its tab menu or by a drag, the last page stays
        // open and its view lands where asked.
        let spot = l
            .page_to_view(P1, ViewId::Inbox, Target::Area(Side::Bottom))
            .unwrap();
        assert_eq!(spot.place, Place::Side(Side::Bottom));
        assert_eq!(l.page_tabs(), [P1]);
        assert!(l.is_visible(ViewId::Inbox));
        assert_eq!(l.close_page(P1), Err(Refused::Invalid));
        // A page never goes to a side area as a page.
        assert_eq!(
            l.move_tab(Tab::Page(P1), Target::Area(Side::Left)),
            Err(Refused::Invalid)
        );
        assert_whole(&l);
    }

    #[test]
    fn a_move_lands_where_asked_and_its_own_place_changes_nothing() {
        let mut l = base();
        let before = l.clone();
        // The terminal onto its own place: nothing.
        assert_eq!(
            l.move_tab(Tab::View(ViewId::Terminal), into(RIGHT, 0, Some(0))),
            Err(Refused::NoChange)
        );
        assert_eq!(
            l.move_tab(Tab::View(ViewId::Terminal), into(RIGHT, 0, Some(1))),
            Err(Refused::NoChange)
        );
        assert_eq!(
            l.move_tab(Tab::View(ViewId::Projects), Target::Area(Side::Left)),
            Err(Refused::NoChange)
        );
        assert_eq!(l, before);
        // Reordered: the chat before the terminal.
        l.move_tab(Tab::View(ViewId::Chat), into(RIGHT, 0, Some(0)))
            .unwrap();
        assert_eq!(
            l.right.groups[0].tabs,
            [
                Tab::View(ViewId::Chat),
                Tab::View(ViewId::Terminal),
                Tab::View(ViewId::AgentBrowser)
            ]
        );
        // The terminal to the bottom panel (the hidden area's strip).
        let spot = l
            .move_tab(Tab::View(ViewId::Terminal), Target::Area(Side::Bottom))
            .unwrap();
        assert_eq!(spot.place, BOTTOM);
        assert!(l.bottom.shown && l.is_visible(ViewId::Terminal));
        assert_whole(&l);
        // The projects tree into the page area as a view tab.
        l.move_tab(Tab::View(ViewId::Projects), into(Place::Page, 0, None))
            .unwrap();
        assert!(!l.left.shown, "the left sidebar emptied and hid");
        assert_eq!(l.focused_tab(), Some(Tab::View(ViewId::Projects)));
        assert_eq!(
            l.focused_page(),
            Some(P1),
            "the breadcrumb keeps the last page"
        );
        assert_whole(&l);
    }

    #[test]
    fn reordering_page_tabs_accounts_for_the_tab_leaving() {
        let mut l = base();
        l.move_tab(Tab::Page(P1), into(Place::Page, 0, Some(3)))
            .unwrap();
        assert_eq!(l.page_tabs(), [P2, P3, P1]);
        l.move_tab(Tab::Page(P1), into(Place::Page, 0, Some(1)))
            .unwrap();
        assert_eq!(l.page_tabs(), [P2, P1, P3]);
        assert_eq!(l.page.groups[0].active, 1, "the moved tab is active");
    }

    #[test]
    fn page_groups_split_move_and_join() {
        let mut l = base();
        assert!(l.can_split_active(Direction::Row));
        l.page.groups[0].active = 2;
        let spot = l.split_active(Direction::Row).unwrap();
        assert_eq!((spot.group, l.focused), (1, 1));
        assert_eq!(l.page.groups[1].tabs, [Tab::Page(P3)]);
        assert_eq!(l.page.shares, [0.5, 0.5]);
        // The focused group has one tab: no split from it.
        assert!(!l.can_split_active(Direction::Row));
        // Along the other direction no split while split.
        l.focused = 0;
        assert!(!l.can_split_active(Direction::Column));
        assert!(l.can_split_active(Direction::Row));
        l.split_active(Direction::Row).unwrap();
        assert_eq!(l.page.groups.len(), 3);
        // Three groups: the most.
        l.focused = 0;
        l.page.groups[0].tabs.push(Tab::View(ViewId::Projects));
        l.left.groups.clear();
        l.left.shares.clear();
        assert!(!l.can_split_active(Direction::Row));
        assert_whole(&l);
        // Moving the last tab out of a group closes it.
        l.focused = 1;
        l.move_active_to_group(1).unwrap();
        assert_eq!(l.page.groups.len(), 2);
        assert_eq!(l.focused, 1);
        assert_eq!(
            l.move_active_to_group(1),
            Err(Refused::NoChange),
            "edge group"
        );
        assert_whole(&l);
        l.join_groups().unwrap();
        assert_eq!(l.page.groups.len(), 1);
        assert_eq!(l.page.shares, [1.]);
        assert_eq!(
            l.focused_tab(),
            Some(Tab::Page(P2)),
            "the focused tab stays active"
        );
        assert_eq!(l.join_groups(), Err(Refused::NoChange));
        assert_whole(&l);
    }

    #[test]
    fn a_sidebar_splits_only_along_its_long_axis_and_into_two_groups() {
        let l = base();
        let terminal = l.locate(Tab::View(ViewId::Terminal));
        assert!(l.may_split(RIGHT, 0, Edge::Bottom, None));
        assert!(!l.may_split(RIGHT, 0, Edge::Right, None));
        assert!(l.may_split(RIGHT, 0, Edge::Top, terminal));
        let mut l = l;
        l.split_off(ViewId::Chat).unwrap();
        assert_eq!(l.right.groups.len(), 2);
        assert!(!l.can_split_off(ViewId::Terminal), "two groups already");
        assert!(!l.may_split(RIGHT, 0, Edge::Bottom, None));
        assert!(!l.can_split_off(ViewId::Projects), "alone in its group");
        // The bottom panel splits side by side.
        l.move_tab(Tab::View(ViewId::Terminal), Target::Area(Side::Bottom))
            .unwrap();
        assert!(l.may_split(BOTTOM, 0, Edge::Right, None));
        assert!(!l.may_split(BOTTOM, 0, Edge::Bottom, None));
        assert_whole(&l);
    }

    #[test]
    fn a_group_cannot_be_split_by_its_only_tab() {
        let mut l = base();
        let projects = l.locate(Tab::View(ViewId::Projects));
        assert!(!l.may_split(LEFT, 0, Edge::Bottom, projects));
        assert_eq!(
            l.move_tab(
                Tab::View(ViewId::Projects),
                Target::Split {
                    place: LEFT,
                    group: 0,
                    edge: Edge::Bottom
                }
            ),
            Err(Refused::Invalid)
        );
    }

    #[test]
    fn reset_keeps_the_pages_and_nothing_else() {
        let mut l = base();
        l.move_tab(Tab::View(ViewId::Terminal), Target::Area(Side::Bottom))
            .unwrap();
        l.page.groups[0].active = 1;
        l.split_active(Direction::Row).unwrap();
        l.page_to_view(P3, ViewId::Inbox, Target::Area(Side::Right))
            .unwrap();
        l.left.size = 333.;
        l.hide_view(ViewId::Projects);
        l.reset(true);
        let mut fresh = Layout::defaults(vec![P1, P2], true);
        fresh.page.groups[0].active = 1;
        assert_eq!(l, fresh, "the focused group's active page stays active");
        assert_whole(&l);
    }

    #[test]
    fn repair_places_every_view_once_and_every_live_page() {
        let mut l = base();
        // A view named twice and a page that is gone.
        l.right.groups[0].tabs.push(Tab::View(ViewId::Projects));
        l.page.groups[0].tabs.push(Tab::Page(TabId(99)));
        l.page.groups.push(Group {
            tabs: vec![Tab::View(ViewId::Inbox)],
            active: 5,
        });
        l.page.shares = vec![0.3];
        l.last.insert(
            ViewId::Projects,
            Spot {
                place: BOTTOM,
                group: 0,
                index: 0,
            },
        );
        l.focused = 7;
        l.left.size = 9000.;
        l.repair(&[P1, P2, P3, P4], true);
        assert_whole(&l);
        assert_eq!(l.locate(Tab::View(ViewId::Projects)).unwrap().place, LEFT);
        assert_eq!(
            l.page.groups.len(),
            1,
            "a group with nothing valid is removed"
        );
        assert_eq!(l.page_tabs(), [P1, P2, P3, P4]);
        assert!(l.left.size <= SIZE_MAX);
    }

    #[test]
    fn without_a_chat_panel_the_chat_keeps_its_place_as_its_last() {
        let mut l = base();
        l.repair(&[P1, P2, P3], false);
        assert!(!l.is_placed(ViewId::Chat));
        assert_eq!(
            l.last[&ViewId::Chat],
            Spot {
                place: RIGHT,
                group: 0,
                index: 1
            }
        );
        assert_whole(&l);
    }

    #[test]
    fn hide_and_show_never_lose_a_view_whatever_the_order() {
        // Every view hidden and shown in several orders, with moves between.
        let orders: [[ViewId; 6]; 3] = [
            ViewId::ALL,
            [
                ViewId::Chat,
                ViewId::AgentBrowser,
                ViewId::Projects,
                ViewId::Terminal,
                ViewId::Inbox,
                ViewId::Knowledge,
            ],
            [
                ViewId::Terminal,
                ViewId::Chat,
                ViewId::Knowledge,
                ViewId::Projects,
                ViewId::AgentBrowser,
                ViewId::Inbox,
            ],
        ];
        for order in orders {
            let mut l = base();
            l.page_to_view(P2, ViewId::Inbox, Target::Area(Side::Bottom))
                .unwrap();
            l.move_tab(Tab::View(ViewId::Terminal), into(Place::Page, 0, None))
                .unwrap();
            for v in order {
                l.hide_view(v);
                assert_whole(&l);
            }
            for v in order {
                l.show_view(v);
                assert_whole(&l);
            }
            for v in [
                ViewId::Projects,
                ViewId::Terminal,
                ViewId::Chat,
                ViewId::Inbox,
            ] {
                assert!(l.is_visible(v) || l.is_placed(v), "{v:?} lost");
            }
            assert_eq!(
                l.locate(Tab::View(ViewId::Terminal)).unwrap().place,
                Place::Page
            );
            assert_eq!(l.locate(Tab::View(ViewId::Inbox)).unwrap().place, BOTTOM);
        }
    }

    #[test]
    fn the_layout_round_trips_through_its_stored_form() {
        let mut l = base();
        l.move_tab(Tab::View(ViewId::Chat), Target::Area(Side::Bottom))
            .unwrap();
        l.page.groups[0].active = 1;
        l.split_active(Direction::Row).unwrap();
        l.move_tab(Tab::View(ViewId::Terminal), into(Place::Page, 1, None))
            .unwrap();
        l.hide_view(ViewId::Projects);
        l.left.size = 300.;
        l.page.shares = vec![0.25, 0.75];
        let saved = Saved {
            layout: l.clone(),
            pages: vec![
                (P1, "T-42".into()),
                (P2, "inbox:P-1".into()),
                (P3, "settings".into()),
            ],
        };
        let value = saved.to_value();
        let text = value.to_string();
        assert!(text.contains("\"view:terminal\""));
        assert!(!text.contains("Alexandre"));
        let back = Saved::from_value(&value, |_| true).unwrap();
        // Page tabs get fresh ids in their order of appearance.
        let key = |s: &Saved, id: TabId| s.pages.iter().find(|(p, _)| *p == id).unwrap().1.clone();
        let keys = |s: &Saved| -> Vec<String> {
            s.layout
                .page_tabs()
                .into_iter()
                .map(|id| key(s, id))
                .collect()
        };
        assert_eq!(keys(&back), keys(&saved));
        assert_eq!(back.layout.left, saved.layout.left);
        assert_eq!(back.layout.bottom, saved.layout.bottom);
        assert_eq!(back.layout.last, saved.layout.last);
        assert_eq!(back.layout.page.shares, saved.layout.page.shares);
        assert_eq!(back.layout.focused, saved.layout.focused);
        assert_eq!(
            back.layout.page.groups[1].tabs[1],
            Tab::View(ViewId::Terminal)
        );
    }

    #[test]
    fn loading_is_forgiving() {
        let value: Value = serde_json::from_str(
            r#"{"v":1,
            "areas":{
              "left":{"shown":true,"size":12,"groups":[{"views":["projects","maps","projects"],"active":9}]},
              "right":{"shown":"yes","size":648,"groups":[{"views":["terminal","chat"],"active":1}]},
              "bottom":{"shown":true,"size":320,"groups":[{"views":["nonsense"]}]}
            },
            "page":{"direction":"diagonal","shares":[2],"focused":4,
              "groups":[{"tabs":["T-1","My secret title","view:inbox","view:projects","T-1"],"active":3},{"tabs":[]}]},
            "last":{"chat":{"area":"right","group":0,"index":1},"browser":{"area":"left"}}
            }"#,
        )
        .unwrap();
        let saved = Saved::from_value(&value, |k| !k.contains(' ')).unwrap();
        let l = &saved.layout;
        assert_whole(l);
        assert_eq!(saved.pages, [(TabId(1), "T-1".to_owned())]);
        assert_eq!(l.left.groups[0].tabs, [Tab::View(ViewId::Projects)]);
        assert_eq!(l.left.size, SIZE_MIN, "a size out of range is clamped");
        assert!(
            !l.right.shown,
            "a field of the wrong type keeps its default"
        );
        assert_eq!(l.right.groups[0].active, 1);
        assert!(
            l.bottom.groups.is_empty(),
            "a group with nothing valid is removed"
        );
        assert_eq!(l.page.groups.len(), 1);
        assert_eq!(l.page.direction, Direction::Row);
        assert!(!l.last.contains_key(&ViewId::Chat), "the chat is placed");
        // Nothing readable at all: the defaults.
        assert_eq!(Saved::from_value(&Value::from(3), |_| true), None);
        assert_eq!(Saved::from_value(&json!({"v": 2}), |_| true), None);
    }

    #[test]
    fn the_m7_fields_migrate() {
        let saved = migrate(&Old {
            sidebar_hidden: true,
            sidebar_width: Some(300),
            terminal_width: None,
            chat_width: Some(520),
            chat_bottom: true,
            chat_height: Some(240),
            tabs: vec!["T-42".into(), "settings".into()],
            active_tab: 1,
            page: Some("T-1".into()),
        });
        let l = &saved.layout;
        assert_whole(l);
        assert!(!l.left.shown);
        assert_eq!(
            (l.left.size, l.right.size, l.bottom.size),
            (300., 520., 240.)
        );
        assert_eq!(
            l.right.groups[0].tabs,
            [Tab::View(ViewId::Terminal), Tab::View(ViewId::AgentBrowser)]
        );
        assert_eq!(l.bottom.groups[0].tabs, [Tab::View(ViewId::Chat)]);
        assert!(!l.bottom.shown);
        assert_eq!(
            saved
                .pages
                .iter()
                .map(|(_, k)| k.as_str())
                .collect::<Vec<_>>(),
            ["T-42", "settings"]
        );
        assert_eq!(l.focused_tab(), Some(Tab::Page(TabId(2))));
        // One tab: the page shown at quit.
        let single = migrate(&Old {
            page: Some("T-1".into()),
            terminal_width: Some(500),
            ..Old::default()
        });
        assert_eq!(single.pages, [(TabId(1), "T-1".to_owned())]);
        assert_eq!(single.layout.right.size, 500.);
        assert!(single.layout.left.shown);
        assert!(Old::default().is_empty());
    }

    #[test]
    fn undo_takes_back_one_step_and_coalesces_keyboard_resizes() {
        let mut log = UndoLog::default();
        let a = base();
        let mut b = a.clone();
        b.left.size = 300.;
        log.record(a.clone(), 1, None);
        log.record(b.clone(), 2, Some(("grow", 10_000)));
        log.record(b.clone(), 3, Some(("grow", 10_500)));
        log.record(b.clone(), 4, Some(("grow", 11_400)));
        assert_eq!(
            log.newest(),
            Some(2),
            "within a second of each other: one step"
        );
        log.record(b.clone(), 5, Some(("grow", 13_000)));
        assert_eq!(log.newest(), Some(5));
        let now = base();
        assert_eq!(log.undo(now.clone(), 6).map(|l| l.left.size), Some(300.));
        assert_eq!(log.undo(now.clone(), 7).map(|l| l.left.size), Some(300.));
        assert_eq!(log.undo(now.clone(), 8), Some(a));
        assert!(log.undo(now.clone(), 9).is_none() && log.is_empty());
        for i in 0..60 {
            log.record(base(), i, None);
        }
        let mut n = 0;
        while log.undo(base(), 100).is_some() {
            n += 1;
        }
        assert_eq!(n, UNDO_DEPTH);
    }

    #[test]
    fn redo_puts_back_what_undo_took_and_a_new_change_clears_it() {
        let mut log = UndoLog::default();
        let a = base();
        let mut b = a.clone();
        b.left.size = 300.;
        let mut c = b.clone();
        c.left.size = 320.;
        // a -> b -> c, then undo twice.
        log.record(a.clone(), 1, None);
        log.record(b.clone(), 2, None);
        assert_eq!(log.undo(c.clone(), 3), Some(b.clone()));
        assert_eq!(log.undo(b.clone(), 4), Some(a.clone()));
        assert_eq!(log.newest_redo(), Some(4));
        // Redo walks forward in order, and each redo is undoable again.
        assert_eq!(log.redo(a.clone(), 5), Some(b.clone()));
        assert_eq!(log.newest(), Some(5));
        assert_eq!(log.newest_redo(), Some(3));
        assert_eq!(log.redo(b.clone(), 6), Some(c.clone()));
        assert!(log.redo(c.clone(), 7).is_none());
        assert_eq!(log.undo(c.clone(), 8), Some(b.clone()));
        // A new change drops what was left to redo.
        log.record(b.clone(), 9, None);
        assert!(log.newest_redo().is_none());
    }
}
