//! The workbench layout in the window (docs/design/workbench.md 4.8): the
//! areas and their groups, area headers and view tabs, page groups and
//! their tab bars, resize handles, dragging views and tabs with drop zones
//! and the drop preview, the Layout and view menus, the move toast, and
//! every layout change as one undo step. The rules themselves are pure
//! (`layout.rs`, `layout_frame.rs`); this module draws them and wires the
//! pointer and the keyboard.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, Context, CursorStyle, DispatchPhase, Entity,
    FocusHandle, Focusable, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, SharedString, StatefulInteractiveElement,
    Styled, Window, anchored, canvas, deferred, div, point, px,
};

use super::{Content, PageSlot, PageView, Shell};
use crate::i18n::{Arg, Key, KeyName, text, tr};
use crate::icons::{Icon, icon};
use crate::inbox_page::InboxPage;
use crate::kb_model::Filters;
use crate::kb_page::KnowledgePage;
use crate::layout::{Direction, Layout, Place, Side, Spot, Tab, TabId, Target, ViewAction, ViewId};
use crate::layout_frame::{self, Dragged, Rect, Region, Zone};
use crate::menu;
use crate::route::{History, Route};
use crate::settings;
use crate::theme::{RADIUS_MD, RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;
use crate::workspace::{Toast, ToastKind};

/// Whether this build has the chat panel (ADR 0009: Windows only).
pub(super) const CHAT: bool = cfg!(windows);
/// 4.8.4: area headers are 28 high; page tab bars 32 (4.4); the view
/// strip under a page-area tab bar 28 (4.8.3).
const HEADER: f32 = 28.;
const TAB_BAR: f32 = 32.;
const VIEW_STRIP: f32 = 28.;
/// 4.8.7: a press becomes a drag after 4 of movement.
const DRAG_START: f32 = 4.;
/// 4.8.8: under 452 of group width, pages drop their side padding to 16.
const NARROW_GROUP: f32 = 452.;
/// 4.8.10: the sidebar shown over the page keeps 48 of the window free.
const OVERLAY_GAP: f32 = 48.;

/// What the last frame drew: the groups, their tabs and the body, in
/// window coordinates. Drops and resizes read it between frames.
#[derive(Default)]
pub(super) struct Geometry {
    groups: HashMap<(PlaceKey, usize), Rect>,
    headers: HashMap<(PlaceKey, usize), Rect>,
    tabs: HashMap<(PlaceKey, usize, usize), Rect>,
    areas: HashMap<PlaceKey, Rect>,
    body: Rect,
    /// Each page group's width in the frame before, for its padding.
    page_widths: [f32; 3],
    /// Every group's width in the frame before (the view tabs that fit).
    group_widths: HashMap<(PlaceKey, usize), f32>,
}

/// `Place` as a map key (it is `Hash`, but a tuple keeps the code short).
type PlaceKey = u8;

fn place_key(place: Place) -> PlaceKey {
    match place {
        Place::Side(Side::Left) => 0,
        Place::Side(Side::Right) => 1,
        Place::Side(Side::Bottom) => 2,
        Place::Page => 3,
    }
}

impl Geometry {
    fn clear(&mut self) {
        for (g, w) in self.page_widths.iter_mut().enumerate() {
            *w = self
                .groups
                .get(&(place_key(Place::Page), g))
                .map_or(f32::MAX, |r| r.w);
        }
        self.group_widths = self.groups.iter().map(|(k, r)| (*k, r.w)).collect();
        self.groups.clear();
        self.headers.clear();
        self.tabs.clear();
        self.areas.clear();
    }

    fn regions(&self, layout: &Layout) -> Vec<Region> {
        let mut out = Vec::new();
        for place in Layout::PLACES {
            let key = place_key(place);
            for group in 0..layout.area(place).groups.len() {
                let Some(bounds) = self.groups.get(&(key, group)) else {
                    continue;
                };
                let count = layout.area(place).groups[group].tabs.len();
                let tabs = (0..count)
                    .filter_map(|i| self.tabs.get(&(key, group, i)).copied())
                    .collect::<Vec<_>>();
                out.push(Region {
                    place,
                    group,
                    bounds: *bounds,
                    header: self.headers.get(&(key, group)).copied(),
                    // Only a complete row tells where a tab would go.
                    tabs: if tabs.len() == count {
                        tabs
                    } else {
                        Vec::new()
                    },
                });
            }
        }
        out
    }
}

fn rect_of(bounds: Bounds<Pixels>) -> Rect {
    Rect::new(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}

/// An invisible child that records its parent's bounds each frame.
fn measure(
    geometry: &Rc<RefCell<Geometry>>,
    record: impl Fn(&mut Geometry, Rect) + 'static,
) -> AnyElement {
    let geometry = Rc::clone(geometry);
    canvas(
        move |bounds, _, _| record(&mut geometry.borrow_mut(), rect_of(bounds)),
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// A press on something draggable, not yet a drag.
#[derive(Clone, Copy, Debug)]
pub(super) struct Press {
    dragged: Dragged,
    x: f32,
    y: f32,
}

/// A view or tab being dragged (4.8.7).
#[derive(Clone, Copy, Debug)]
pub(super) struct Drag {
    dragged: Dragged,
    x: f32,
    y: f32,
    zone: Option<Zone>,
}

/// A resize handle (4.8.6): a side area's, or the one between groups
/// `at` and `at + 1` of an area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Handle {
    Side(Side),
    Groups(Place, usize),
}

/// A handle being dragged: the layout before the drag, for its one undo
/// step and for snap to hide.
#[derive(Clone, Debug)]
pub(super) struct Resize {
    handle: Handle,
    before: Layout,
}

/// The menus the layout opens (4.8.4, 4.8.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LayoutMenu {
    Layout,
    View(ViewId),
    /// The tab menu of an Inbox or Knowledge page tab.
    PageTab(TabId, ViewId),
    /// The tab bar's list of every group's tabs while folded (4.8.10).
    Folded,
    /// The header's list of a folded second group's views.
    FoldedSide(Side),
    /// The view tabs of an area group that do not fit (4.8.4): every view
    /// of the group, the active one checked.
    ViewTabs(Place, usize),
}

/// Width of the header's chevron that replaces the tabs that do not fit.
const OVERFLOW_CHEVRON: f32 = 24.;

/// Which view tabs of a header fit in `available` (4.8.4), in order: as
/// many as fit from the first, the active one always, with room left for
/// the chevron when some are left out. `None` when all fit.
fn fitting_tabs(widths: &[f32], active: usize, available: f32) -> Option<Vec<usize>> {
    const GAP: f32 = 4.;
    let total: f32 = widths.iter().sum::<f32>() + GAP * widths.len().saturating_sub(1) as f32;
    if total <= available {
        return None;
    }
    let mut room = available - OVERFLOW_CHEVRON - GAP;
    if let Some(w) = widths.get(active) {
        room -= w + GAP;
    }
    let mut shown = Vec::new();
    for (i, w) in widths.iter().enumerate() {
        if i == active {
            shown.push(i);
            continue;
        }
        if room >= w + GAP {
            room -= w + GAP;
            shown.push(i);
        } else {
            room = f32::MIN;
        }
    }
    Some(shown)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Menu {
    pub kind: LayoutMenu,
    x: f32,
    y: f32,
}

/// The layout state of a workbench window that is not the layout itself.
pub(super) struct LayoutUi {
    pub geometry: Rc<RefCell<Geometry>>,
    pub press: Option<Press>,
    pub drag: Option<Drag>,
    pub resize: Option<Resize>,
    pub menu: Option<Menu>,
    /// 4.8.10, step 1: the auto-hidden left sidebar shown over the page.
    pub left_overlay: bool,
    /// "Maximize focused area": that group fills the body until toggled.
    pub zoomed: Option<(Place, usize)>,
    /// The move the no-room toast offers to make with the right sidebar
    /// hidden.
    pub no_room: Option<(Dragged, Target)>,
    /// The views the shell last made visible (terminal started, chat
    /// shown).
    pub terminal_shown: bool,
    #[cfg(windows)]
    pub chat_shown: bool,
    epoch: Instant,
}

impl LayoutUi {
    pub fn new() -> Self {
        Self {
            geometry: Rc::default(),
            press: None,
            drag: None,
            resize: None,
            menu: None,
            left_overlay: false,
            zoomed: None,
            no_room: None,
            terminal_shown: false,
            #[cfg(windows)]
            chat_shown: false,
            epoch: Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    /// What is drawn over or instead of the layout.
    pub fn shown(&self) -> layout_frame::Shown {
        layout_frame::Shown {
            zoomed: self.zoomed,
            left_overlay: self.left_overlay,
        }
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
}

/// The catalogue key of a view's name (`{view}` of 10.4.1).
pub(crate) fn view_name(view: ViewId) -> Key {
    match view {
        ViewId::Projects => Key::ViewProjects,
        ViewId::Inbox => Key::TreeInbox,
        ViewId::Knowledge => Key::TreeKnowledge,
        ViewId::Terminal => Key::TerminalTitle,
        ViewId::Chat => Key::ChatTitle,
        ViewId::AgentBrowser => Key::ViewAgentBrowser,
        ViewId::Sessions => Key::ViewSessions,
    }
}

pub(crate) fn view_icon(view: ViewId) -> Icon {
    match view {
        ViewId::Projects => Icon::ListTree,
        ViewId::Inbox => Icon::Inbox,
        ViewId::Knowledge => Icon::Library,
        ViewId::Terminal => Icon::SquareTerminal,
        ViewId::Chat => Icon::MessageCircle,
        ViewId::AgentBrowser => Icon::Globe,
        ViewId::Sessions => Icon::Bot,
    }
}

/// The agent browser is working (a call, or a handoff waiting), or the
/// terminal's Claude Code session waits for the user: its tab and toggle
/// show [`busy_dot`] while it is not on screen (ADR 0010;
/// components-app.md 5, Claude Code sessions).
fn view_busy(bench: &super::Bench, view: ViewId, cx: &App) -> bool {
    match view {
        #[cfg(windows)]
        ViewId::AgentBrowser => bench.browser.read(cx).busy(),
        ViewId::Terminal => bench.active_project.is_some_and(|p| {
            bench.dock.read(cx).session_state(p, cx)
                == Some(acta_core::presence::SessionState::Waiting)
        }),
        _ => false,
    }
}

/// A 6 dot in `accent` (components-app.md 5, Agent browser).
pub(crate) fn busy_dot(t: &Tokens) -> gpui_kit::Div {
    div()
        .flex_shrink_0()
        .size(px(6.))
        .rounded(px(3.))
        .bg(t.accent)
}

impl Shell {
    /// Whether the titlebar toggle of `view` shows the busy dot.
    pub(crate) fn view_busy_hidden(&self, view: ViewId, window: &Window, cx: &App) -> bool {
        let Content::Workbench(bench) = &self.content else {
            return false;
        };
        !self.view_drawn(view, window) && view_busy(bench, view, cx)
    }
}

fn area_name(place: Place) -> Key {
    match place {
        Place::Side(Side::Left) => Key::AreaLeft,
        Place::Side(Side::Right) => Key::AreaRight,
        Place::Side(Side::Bottom) => Key::AreaBottom,
        Place::Page => Key::AreaPage,
    }
}

fn moved_key(place: Place) -> Key {
    match place {
        Place::Side(Side::Left) => Key::LayoutMovedLeft,
        Place::Side(Side::Right) => Key::LayoutMovedRight,
        Place::Side(Side::Bottom) => Key::LayoutMovedBottom,
        Place::Page => Key::LayoutMovedPage,
    }
}

/// The views this build has, in titlebar order.
pub(crate) fn views() -> impl Iterator<Item = ViewId> {
    ViewId::ALL.into_iter().filter(|v| CHAT || !v.is_web())
}

/// The keys of a view's toggle (6.1), for tooltips and menus.
pub(crate) fn view_keys(
    view: ViewId,
    macos: bool,
    locale: crate::i18n::Locale,
) -> Vec<&'static str> {
    let m = KeyName::Mod.label(locale, macos);
    let shift = KeyName::Shift.label(locale, macos);
    match view {
        ViewId::Projects => vec![m, "1"],
        ViewId::Inbox => vec![m, "I"],
        ViewId::Knowledge => vec![m, "L"],
        ViewId::Terminal => vec![KeyName::Ctrl.label(locale, macos), "`"],
        ViewId::Chat => vec![m, shift, "H"],
        // No key of its own (interaction.md 6.1): the toggle, the palette.
        ViewId::AgentBrowser | ViewId::Sessions => Vec::new(),
    }
}

pub(crate) fn area_keys(side: Side, macos: bool, locale: crate::i18n::Locale) -> Vec<&'static str> {
    let m = KeyName::Mod.label(locale, macos);
    let shift = KeyName::Shift.label(locale, macos);
    match side {
        Side::Left => vec![m, "B"],
        Side::Right => vec![m, shift, "B"],
        Side::Bottom => vec![m, "J"],
    }
}

impl Shell {
    // ---- the layout as a whole ----

    fn layout_ui(&mut self) -> Option<&mut LayoutUi> {
        self.bench().map(|b| &mut b.ui)
    }

    /// Apply `f` to the layout as one undo step (4.8.11, rule 3), then
    /// bring the window in line and save. `coalesce` joins keyboard steps
    /// within a second. Returns whether anything changed.
    pub(super) fn change_layout(
        &mut self,
        coalesce: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Layout) -> bool,
    ) -> bool {
        let Some(bench) = self.bench() else {
            return false;
        };
        let before = bench.layout.clone();
        let applied = f(&mut bench.layout);
        if !applied || bench.layout == before {
            bench.layout = before;
            return false;
        }
        let now = bench.ui.now_ms();
        let seq = crate::workspace::next_undo_seq();
        bench
            .layout_undo
            .record(before, seq, coalesce.map(|key| (key, now)));
        self.after_layout(window, cx);
        true
    }

    /// The window after any layout change: the layout made whole, the
    /// focused group's page checked out, every shown page and view alive,
    /// the terminal started and the chat shown where they became visible,
    /// the preferences saved.
    pub(super) fn after_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        // The tree moved, hid or resized under the pointer: it is hovered
        // again only when the pointer next moves over it.
        bench.tree.clear_hover();
        // Pages the change closed leave their slots; their history is kept
        // so that undoing the change brings them back.
        let named = bench.layout.page_tabs();
        let closed: Vec<TabId> = bench
            .slots
            .keys()
            .copied()
            .filter(|id| !named.contains(id))
            .collect();
        for id in closed {
            if let Some(slot) = bench.slots.remove(&id) {
                bench.retired.insert(id, slot.history);
            }
        }
        if !named.contains(&bench.current) {
            bench.retired.insert(bench.current, bench.history.clone());
        }
        let mut live: Vec<TabId> = bench.slots.keys().copied().collect();
        if named.contains(&bench.current) {
            live.push(bench.current);
        }
        bench.layout.repair(&live, CHAT);
        let tabs = bench.layout.page_tabs();
        bench.ui.menu = None;
        // A change ends any resize drag; a maximized group whose area hid or
        // whose group went away gives way.
        bench.ui.resize = None;
        if let Some((place, group)) = bench.ui.zoomed
            && (group >= bench.layout.area(place).groups.len() || !bench.layout.area(place).shown)
        {
            bench.ui.zoomed = None;
        }
        let want = bench.layout.focused_page();
        let current = bench.current;
        let stale = !tabs.contains(&current)
            || matches!(bench.layout.focused_tab(), Some(Tab::Page(id)) if id != current);
        if stale && let Some(want) = want {
            self.check_out(want, window, cx);
        }
        self.sync_page_views(window, cx);
        self.sync_tool_views(window, cx);
        self.mark_shown_seen(cx);
        self.save_layout(cx);
        self.refocus_lost_view(window, cx);
        cx.notify();
    }

    /// Keyboard focus left in a view the change took off screen returns to
    /// the page, so the next key acts (a web view gives the keyboard back
    /// to the window first).
    fn refocus_lost_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lost) =
            views().find(|v| self.view_has_focus(*v, window, cx) && !self.view_drawn(*v, window))
        else {
            return;
        };
        #[cfg(windows)]
        if let Some(bench) = self.bench() {
            match lost {
                ViewId::Chat => bench.chat.read(cx).release_keyboard(cx),
                ViewId::AgentBrowser => bench.browser.read(cx).release_keyboard(cx),
                _ => {}
            }
        }
        self.focus_page(window, cx);
        // A page that keeps no focus of its own: its group takes it.
        if self.view_has_focus(lost, window, cx)
            && let Some(bench) = self.bench()
        {
            let handle = bench.group_focus[bench.layout.focused.min(2)].clone();
            window.focus(&handle, cx);
        }
    }

    /// Make `tab` the page the window acts on (the focused group's page):
    /// the current one goes back to its slot (dropped if it was closed),
    /// `tab` comes out of its slot, and opens if it had no view.
    pub(super) fn check_out(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if tab == bench.current {
            return;
        }
        let Some(slot) = bench.slots.remove(&tab) else {
            return;
        };
        let placeholder = PageView::FirstRun(bench.first_run.clone());
        let old_view = std::mem::replace(&mut bench.page, slot.view.unwrap_or(placeholder));
        let had_view = !matches!(bench.page, PageView::FirstRun(_));
        let old = PageSlot {
            history: std::mem::replace(&mut bench.history, slot.history),
            view: Some(old_view),
            sub: std::mem::replace(&mut bench._page_sub, slot.sub),
        };
        let old_id = std::mem::replace(&mut bench.current, tab);
        if bench.layout.page_tabs().contains(&old_id) {
            bench.slots.insert(old_id, old);
        }
        let route = bench.history.current();
        match route {
            Some(route) if !had_view => self.show(route, window, cx),
            Some(route) => self.follow_route(route, false, window, cx),
            None => {}
        }
    }

    /// Every page group's active page has a view; pages no group shows
    /// drop theirs (they open again when shown, as tabs always have).
    fn sync_page_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let active: Vec<TabId> = bench
            .layout
            .page
            .groups
            .iter()
            .filter_map(|g| match g.active_tab() {
                Some(Tab::Page(id)) => Some(id),
                _ => None,
            })
            .collect();
        let mut missing = Vec::new();
        for (id, slot) in bench.slots.iter_mut() {
            if active.contains(id) {
                if slot.view.is_none()
                    && let Some(route) = slot.history.current()
                {
                    missing.push((*id, route));
                }
            } else {
                slot.view = None;
                slot.sub = None;
            }
        }
        for (id, route) in missing {
            let (view, sub) = self.make_view(route, id, window, cx);
            if let Some(bench) = self.bench()
                && let Some(slot) = bench.slots.get_mut(&id)
            {
                slot.view = Some(view);
                slot.sub = sub;
            }
        }
    }

    /// The terminal starts the first time it becomes visible; the Inbox and
    /// Knowledge views exist while they are placed, for the active project.
    fn sync_tool_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let origin = self.origin(cx);
        let Some(bench) = self.bench() else { return };
        let terminal = bench.layout.is_visible(ViewId::Terminal);
        if terminal && !bench.ui.terminal_shown {
            if let Some(origin) = origin {
                let dock = bench.dock.clone();
                dock.update(cx, |dock, cx| dock.show(origin, window, cx));
                bench.ui.terminal_shown = true;
            }
        } else if !terminal {
            bench.ui.terminal_shown = false;
        }
        self.sync_page_kind_views(window, cx);
    }

    /// The Inbox and Knowledge views follow the active project (4.8.3).
    pub(super) fn sync_page_kind_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let project = bench.active_project;
        let ws = bench.ws.clone();
        // A hidden view keeps running (4.8.1); only a view that became a
        // page again lets its entity go.
        let is_view = |v| bench.layout.is_placed(v) || bench.layout.last.contains_key(&v);
        let inbox_placed = is_view(ViewId::Inbox);
        let kb_placed = is_view(ViewId::Knowledge);
        let inbox_stale = bench
            .inbox_view
            .as_ref()
            .is_none_or(|v| Some(v.read(cx).project) != project);
        let kb_stale = bench
            .knowledge_view
            .as_ref()
            .is_none_or(|v| Some(v.read(cx).project) != project);
        if !inbox_placed {
            bench.inbox_view = None;
        }
        if !kb_placed {
            bench.knowledge_view = None;
        }
        let Some(project) = project else { return };
        if inbox_placed && inbox_stale {
            let view = cx.new(|cx| InboxPage::new_view(ws.clone(), project, window, cx));
            let sub = cx.subscribe_in(&view, window, |shell, _, event, window, cx| {
                shell.on_view_page(event, window, cx)
            });
            if let Some(bench) = self.bench() {
                bench.inbox_view = Some(view);
                bench.view_subs.insert(ViewId::Inbox, sub);
            }
        }
        if kb_placed && kb_stale {
            let view = cx.new(|cx| {
                KnowledgePage::new_view(ws.clone(), project, Filters::default(), window, cx)
            });
            let sub = cx.subscribe_in(&view, window, |shell, _, event, window, cx| {
                shell.on_view_page(event, window, cx)
            });
            if let Some(bench) = self.bench() {
                bench.knowledge_view = Some(view);
                bench.view_subs.insert(ViewId::Knowledge, sub);
            }
        }
    }

    /// The focus handle of a view (what `Mod+1`, `F6` and showing focus).
    pub(super) fn view_focus(&self, view: ViewId, cx: &App) -> Option<FocusHandle> {
        let Content::Workbench(bench) = &self.content else {
            return None;
        };
        Some(match view {
            ViewId::Projects => bench.tree_focus.clone(),
            ViewId::Terminal => bench.dock.read(cx).focus_handle(cx),
            ViewId::Sessions => bench.sessions.read(cx).focus_handle(cx),
            #[cfg(windows)]
            ViewId::Chat => bench.chat.read(cx).focus_handle(cx),
            #[cfg(windows)]
            ViewId::AgentBrowser => bench.browser.read(cx).focus_handle(cx),
            #[cfg(not(windows))]
            ViewId::Chat | ViewId::AgentBrowser => return None,
            ViewId::Inbox => bench.inbox_focus.clone(),
            ViewId::Knowledge => bench.knowledge_focus.clone(),
        })
    }

    /// Whether keyboard focus is inside `view`.
    fn view_has_focus(&self, view: ViewId, window: &Window, cx: &App) -> bool {
        self.view_focus(view, cx)
            .is_some_and(|h| h.contains_focused(window, cx))
    }

    /// Focus `view` (shown already).
    pub(super) fn focus_view(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(windows)]
        if view == ViewId::Chat {
            if let Some(bench) = self.bench() {
                let chat = bench.chat.clone();
                chat.update(cx, |chat, cx| {
                    chat.show(window, cx);
                    chat.focus(window, cx)
                });
            }
            return;
        }
        if let Some(handle) = self.view_focus(view, cx) {
            window.focus(&handle, cx);
        }
    }

    /// 4.8.5, the toggle rule: a hidden view is shown where it was last and
    /// focused; a visible view without focus is focused; a focused view is
    /// hidden and focus returns to the page. Inbox and Knowledge while they
    /// are pages: their page in the focused group.
    pub(crate) fn toggle_view(
        &mut self,
        view: ViewId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.content, Content::Workbench(_)) || (view.is_web() && !CHAT) {
            return;
        }
        let Some(bench) = self.bench() else { return };
        if view.is_page_kind()
            && !bench.layout.is_placed(view)
            && !bench.layout.last.contains_key(&view)
        {
            let route = match view {
                ViewId::Inbox => Route::Inbox,
                _ => Route::Knowledge,
            };
            return self.go_project_page(route, window, cx);
        }
        let drawn = self.view_drawn(view, window);
        if drawn && self.view_has_focus(view, window, cx) {
            self.change_layout(None, window, cx, |l| l.hide_view(view));
            self.focus_page(window, cx);
            return;
        }
        if !drawn {
            self.bring_on_screen(view, window, cx);
        }
        self.focus_view(view, window, cx);
    }

    /// Whether `view` is drawn now (4.8.5: a toggle is selected only then).
    pub(crate) fn view_drawn(&self, view: ViewId, window: &Window) -> bool {
        let Content::Workbench(bench) = &self.content else {
            return false;
        };
        let (w, h) = body_size(window);
        let f = layout_frame::frame(&bench.layout, w, h);
        layout_frame::on_screen(&bench.layout, &f, bench.ui.shown(), view)
    }

    /// Show `view` where it was last and bring it on screen: unfolded,
    /// over the page when its sidebar is auto-hidden, out from under a
    /// maximized group (4.8.5, 4.8.10).
    fn bring_on_screen(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let (w, h) = body_size(window);
        let Some(bench) = self.bench() else { return };
        let mut shown = bench.ui.shown();
        self.change_layout(None, window, cx, |l| {
            if l.show_view(view).is_none() {
                return false;
            }
            let f = layout_frame::frame(l, w, h);
            layout_frame::reveal(l, &f, &mut shown, view);
            true
        });
        if let Some(bench) = self.bench() {
            bench.ui.zoomed = shown.zoomed;
            bench.ui.left_overlay = shown.left_overlay;
        }
        cx.notify();
    }

    /// Show or hide a side area with everything in it; showing focuses
    /// nothing (4.8.5). While the left sidebar is auto-hidden its toggle
    /// shows it over the page (4.8.10).
    pub(crate) fn toggle_area(&mut self, side: Side, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if side == Side::Left && bench.layout.left.shown && self.left_auto_hidden(window) {
            if let Some(bench) = self.bench() {
                bench.ui.left_overlay = !bench.ui.left_overlay;
            }
            cx.notify();
            return;
        }
        let Some(bench) = self.bench() else { return };
        // An area toggle always shows the whole layout again.
        bench.ui.zoomed = None;
        let shown = bench.layout.side(side).shown;
        self.change_layout(None, window, cx, |l| l.set_area_shown(side, !shown));
        cx.notify();
        if shown {
            // Focus inside the hidden area returns to the page.
            let lost = self
                .bench()
                .is_some_and(|b| b.layout.side(side).active_views().count() > 0);
            if lost {
                self.focus_page(window, cx);
            }
        }
    }

    /// Whether the left sidebar is shown in the layout but has no room.
    pub(super) fn left_auto_hidden(&mut self, window: &Window) -> bool {
        let (w, h) = body_size(window);
        self.bench()
            .is_some_and(|b| layout_frame::frame(&b.layout, w, h).left_auto_hidden)
    }

    /// A view action from a view menu, a tab or the palette (4.8.4, 4.8.7).
    /// Moves and hides from a menu or the palette toast with Undo and
    /// Reset layout (4.8.11).
    pub(super) fn view_action(
        &mut self,
        view: ViewId,
        action: ViewAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if view.is_web() && !CHAT {
            return;
        }
        let locale = settings::locale(cx);
        let name = text(locale, view_name(view));
        match action {
            ViewAction::Show => {
                let Some(bench) = self.bench() else { return };
                if view.is_page_kind()
                    && !bench.layout.is_placed(view)
                    && !bench.layout.last.contains_key(&view)
                {
                    return self.toggle_view(view, window, cx);
                }
                self.bring_on_screen(view, window, cx);
                self.focus_view(view, window, cx);
            }
            ViewAction::Hide => {
                let had_focus = self.view_has_focus(view, window, cx);
                if self.change_layout(None, window, cx, |l| l.hide_view(view)) {
                    if had_focus {
                        self.focus_page(window, cx);
                    }
                    let message = tr(locale, Key::LayoutHidden, &[("view", Arg::Text(&name))]);
                    self.layout_toast(message, cx);
                }
            }
            ViewAction::MoveTo(place) => {
                let target = match place {
                    Place::Side(side) => Target::Area(side),
                    Place::Page => Target::Into {
                        place: Place::Page,
                        group: self.bench().map_or(0, |b| b.layout.focused),
                        index: None,
                    },
                };
                let dragged = Dragged {
                    tab: Tab::View(view),
                    as_view: None,
                };
                self.move_by_command(dragged, target, window, cx);
            }
            ViewAction::SplitOff => {
                if self.change_layout(None, window, cx, |l| l.split_off(view).is_ok()) {
                    self.focus_view(view, window, cx);
                }
            }
        }
        let _ = name;
    }

    /// A move by the view menu, a page tab's menu or the palette (4.8.7):
    /// the view lands as the last tab of the target area, on screen. When
    /// the target cannot be shown beside what is there, nothing moves and
    /// the toast offers the move with the right sidebar hidden, as one
    /// step.
    fn move_by_command(
        &mut self,
        dragged: Dragged,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = match dragged.tab {
            Tab::View(v) => v,
            Tab::Page(_) => match dragged.as_view {
                Some(v) => v,
                None => return,
            },
        };
        let place = match target {
            Target::Into { place, .. } | Target::Split { place, .. } => place,
            Target::Area(side) => Place::Side(side),
        };
        if self.drop_view(dragged, target, window, cx) {
            self.after_move(view, place, window, cx);
            return;
        }
        // Refused for want of room only: the same move, room or not, works.
        let no_room = place == Place::Side(Side::Left)
            && self.bench().is_some_and(|b| {
                layout_frame::place(&b.layout, dragged, target, None, TabId(u64::MAX)).is_some()
            });
        if !no_room {
            return;
        }
        let locale = settings::locale(cx);
        let reason = text(locale, Key::LayoutNoRoomLeft);
        let message = tr(locale, Key::LayoutNoRoom, &[("reason", Arg::Text(&reason))]);
        if let Some(bench) = self.bench() {
            bench.ui.no_room = Some((dragged, target));
        }
        self.show_toast(
            Toast {
                message,
                kind: ToastKind::LayoutNoRoom,
            },
            cx,
        );
    }

    /// The no-room toast's action: the right sidebar hidden and the move
    /// made, as one undo step.
    pub(super) fn hide_right_and_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let width = self.body_width(window);
        let Some(bench) = self.bench() else { return };
        bench.toast = None;
        let Some((dragged, target)) = bench.ui.no_room.take() else {
            return;
        };
        let view = match dragged.tab {
            Tab::View(v) => Some(v),
            Tab::Page(_) => dragged.as_view,
        };
        let had_focus = self
            .bench()
            .map(|b| {
                b.layout
                    .side(Side::Right)
                    .active_views()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_iter()
            .any(|v| self.view_has_focus(v, window, cx));
        let done = self.change_layout(None, window, cx, |l| {
            let mut hidden = l.clone();
            hidden.set_area_shown(Side::Right, false);
            match layout_frame::simulate(&hidden, dragged, target, None, TabId(u64::MAX), width) {
                Some((after, _)) => {
                    *l = after;
                    true
                }
                None => false,
            }
        });
        if had_focus {
            self.focus_page(window, cx);
        }
        if done && let Some(view) = view {
            self.after_move(view, Place::Side(Side::Left), window, cx);
        }
        cx.notify();
    }

    /// The width the layout lays out in: the body as last drawn.
    fn body_width(&mut self, window: &Window) -> f32 {
        let drawn = self.bench().map_or(0., |b| b.ui.geometry.borrow().body.w);
        if drawn > 0. {
            drawn
        } else {
            body_size(window).0
        }
    }

    /// After a view moved: on screen, focused, and the move toast.
    fn after_move(
        &mut self,
        view: ViewId,
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A maximized other group gives way to what just moved.
        if let Some(bench) = self.bench()
            && let Some(spot) = bench.layout.locate(Tab::View(view))
            && bench
                .ui
                .zoomed
                .is_some_and(|z| z != (spot.place, spot.group))
        {
            bench.ui.zoomed = None;
        }
        if !(view.is_page_kind() && place == Place::Page) {
            self.focus_view(view, window, cx);
        }
        let locale = settings::locale(cx);
        let name = text(locale, view_name(view));
        let message = tr(locale, moved_key(place), &[("view", Arg::Text(&name))]);
        self.layout_toast(message, cx);
    }

    /// Drop (or move by command) `dragged` at `target`. A view of Inbox or
    /// Knowledge landing in the page area opens the active project's page
    /// there; a page of them landing outside becomes the view.
    fn drop_view(
        &mut self,
        dragged: Dragged,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(bench) = self.bench() else {
            return false;
        };
        let mut new_tab = None;
        if let Tab::View(view) = dragged.tab
            && view.is_page_kind()
            && matches!(
                target,
                Target::Into {
                    place: Place::Page,
                    ..
                } | Target::Split {
                    place: Place::Page,
                    ..
                }
            )
        {
            let Some(project) = bench.active_project else {
                return false;
            };
            let route = match view {
                ViewId::Inbox => Route::Inbox(project),
                _ => Route::Knowledge(project),
            };
            // A page is open once (4.4): the open one closes first.
            let open = bench.find_tab(route);
            let id = bench.new_tab_id();
            bench.slots.insert(
                id,
                PageSlot {
                    history: History::at(route),
                    view: None,
                    sub: None,
                },
            );
            new_tab = Some((id, open));
        }
        let id = new_tab.map_or(TabId(u64::MAX), |(id, _)| id);
        let closing = new_tab.and_then(|(_, open)| open);
        let width = self.body_width(window);
        // The very simulation the preview showed (4.8.11, rule 6).
        let done = self.change_layout(None, window, cx, |l| {
            match layout_frame::simulate(l, dragged, target, closing, id, width) {
                Some((after, _)) => {
                    *l = after;
                    true
                }
                None => false,
            }
        });
        if !done
            && let Some((id, _)) = new_tab
            && let Some(bench) = self.bench()
        {
            bench.slots.remove(&id);
        }
        done
    }

    /// The open page an Inbox or Knowledge view closes when it lands in the
    /// page area as that page.
    fn page_to_close(&mut self, dragged: Dragged) -> Option<TabId> {
        let Tab::View(view) = dragged.tab else {
            return None;
        };
        if !view.is_page_kind() {
            return None;
        }
        let bench = self.bench()?;
        let project = bench.active_project?;
        bench.find_tab(match view {
            ViewId::Inbox => Route::Inbox(project),
            _ => Route::Knowledge(project),
        })
    }

    /// "Reset layout" (4.8.13): one undoable step, toasted.
    pub(super) fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench.ui.zoomed = None;
            bench.ui.left_overlay = false;
        }
        if self.change_layout(None, window, cx, |l| {
            l.reset(CHAT);
            true
        }) {
            let message = text(settings::locale(cx), Key::LayoutResetDone);
            self.show_toast(
                Toast {
                    message,
                    kind: ToastKind::LayoutReset,
                },
                cx,
            );
        }
    }

    /// "Undo layout change": the newest layout step, whatever has focus.
    pub(super) fn undo_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_layout(false, window, cx);
    }

    /// `Mod+Shift+Z` when the newest undone step is a layout change.
    pub(super) fn redo_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_layout(true, window, cx);
    }

    fn step_layout(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let seq = crate::workspace::next_undo_seq();
        let current = bench.layout.clone();
        let target = if redo {
            bench.layout_undo.redo(current, seq)
        } else {
            bench.layout_undo.undo(current, seq)
        };
        let Some(before) = target else {
            return;
        };
        // Pages the step closed come back; pages opened since stay.
        for id in before.page_tabs() {
            if id != bench.current
                && !bench.slots.contains_key(&id)
                && let Some(history) = bench.retired.remove(&id)
            {
                bench.slots.insert(
                    id,
                    PageSlot {
                        history,
                        view: None,
                        sub: None,
                    },
                );
            }
        }
        let mut live: Vec<TabId> = bench.slots.keys().copied().collect();
        live.push(bench.current);
        bench.layout = before;
        bench.layout.repair(&live, CHAT);
        bench.toast = None;
        self.after_layout(window, cx);
    }

    fn layout_toast(&mut self, message: String, cx: &mut Context<Self>) {
        self.show_toast(
            Toast {
                message,
                kind: ToastKind::Layout,
            },
            cx,
        );
    }

    /// "Split right" / "Split down" (4.8.8): when the page area has room for
    /// both groups at their minimums.
    pub(super) fn split_page(
        &mut self,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_split_page(direction, window) {
            return;
        }
        self.change_layout(None, window, cx, |l| l.split_active(direction).is_ok());
        self.focus_page(window, cx);
    }

    pub(super) fn can_split_page(&self, direction: Direction, window: &Window) -> bool {
        let (w, h) = body_size(window);
        let Content::Workbench(bench) = &self.content else {
            return false;
        };
        let f = layout_frame::frame(&bench.layout, w, h);
        layout_frame::split_fits(&bench.layout, &f, direction)
    }

    pub(super) fn move_tab_to_group(
        &mut self,
        step: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_layout(None, window, cx, |l| l.move_active_to_group(step).is_ok());
        self.focus_page(window, cx);
    }

    pub(super) fn join_groups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(None, window, cx, |l| l.join_groups().is_ok());
        self.focus_page(window, cx);
    }

    /// The area or group keyboard focus is in.
    pub(super) fn focused_group(&self, window: &Window, cx: &App) -> Option<(Place, usize)> {
        let Content::Workbench(bench) = &self.content else {
            return None;
        };
        for place in Layout::PLACES {
            for (g, group) in bench.layout.area(place).groups.iter().enumerate() {
                if let Some(Tab::View(v)) = group.active_tab()
                    && self.view_has_focus(v, window, cx)
                {
                    return Some((place, g));
                }
            }
        }
        for (g, handle) in bench.group_focus.iter().enumerate() {
            if g < bench.layout.page.groups.len() && handle.contains_focused(window, cx) {
                return Some((Place::Page, g));
            }
        }
        Some((Place::Page, bench.layout.focused))
    }

    /// "Grow focused area" / "Shrink focused area" (4.8.6): steps of 40,
    /// a sidebar's width, the bottom panel's height, a group's share.
    pub(super) fn step_area(&mut self, grow: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((place, group)) = self.focused_group(window, cx) else {
            return;
        };
        let (w, h) = body_size(window);
        let by = if grow {
            layout_frame::STEP
        } else {
            -layout_frame::STEP
        };
        self.change_layout(Some("step-area"), window, cx, |l| match place {
            Place::Side(side) => {
                let f = layout_frame::frame(l, w, h);
                let now = f.side(side).unwrap_or(l.side(side).size);
                let min = layout_frame::side_min(l, side);
                l.side_mut(side).size = (now + by).max(min);
                // Never past what the window draws (60% of it, 70% of the
                // body, the page's room).
                let drawn = match side {
                    Side::Bottom => layout_frame::bottom_height(l, h),
                    _ => layout_frame::sidebar_width(l, side, w),
                };
                l.side_mut(side).size = drawn;
                true
            }
            Place::Page => {
                let n = l.page.groups.len();
                if n < 2 {
                    return false;
                }
                let f = layout_frame::frame(l, w, h);
                let total = match l.page.direction {
                    Direction::Row => f.page_w,
                    Direction::Column => f.page_h,
                };
                let mins = layout_frame::page_mins(l);
                // The handle after the group, or before the last one.
                let (at, delta) = if group + 1 < n {
                    (group, by)
                } else {
                    (group - 1, -by)
                };
                l.page.shares =
                    layout_frame::resize_shares(&l.page.shares, at, delta, total, &mins);
                true
            }
        });
    }

    /// `F6` / `Shift+F6` (6.2): the next shown area or group, in reading
    /// order: left sidebar, page groups, bottom panel, right sidebar.
    pub(super) fn cycle_area(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (w, h) = body_size(window);
        let Some(bench) = self.bench() else { return };
        let f = layout_frame::frame(&bench.layout, w, h);
        let mut stops: Vec<(Place, usize)> = Vec::new();
        let side_stops = |side: Side, stops: &mut Vec<(Place, usize)>| {
            let shown = match side {
                Side::Left => f.left.is_some() || bench.ui.left_overlay,
                _ => f.side(side).is_some(),
            };
            if shown {
                let n = bench.layout.side(side).groups.len();
                let n = if f.side_folded(side) { n.min(1) } else { n };
                stops.extend((0..n).map(|g| (Place::Side(side), g)));
            }
        };
        side_stops(Side::Left, &mut stops);
        let n = bench.layout.page.groups.len();
        if f.page_folded {
            stops.push((Place::Page, bench.layout.focused.min(n - 1)));
        } else {
            stops.extend((0..n).map(|g| (Place::Page, g)));
        }
        side_stops(Side::Bottom, &mut stops);
        side_stops(Side::Right, &mut stops);
        let here = self.focused_group(window, cx);
        let at = here.and_then(|h| stops.iter().position(|s| *s == h));
        let next = match at {
            Some(i) if forward => (i + 1) % stops.len(),
            Some(i) => (i + stops.len() - 1) % stops.len(),
            None => 0,
        };
        if let Some(&(place, group)) = stops.get(next) {
            self.focus_group(place, group, window, cx);
        }
    }

    /// Focus the active tab of `group` in `place`.
    pub(super) fn focus_group(
        &mut self,
        place: Place,
        group: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        let Some(tab) = bench
            .layout
            .area(place)
            .groups
            .get(group)
            .and_then(|g| g.active_tab())
        else {
            return;
        };
        match tab {
            Tab::View(v) => {
                if place == Place::Page {
                    bench.layout.focused = group;
                }
                self.focus_view(v, window, cx)
            }
            Tab::Page(id) => {
                bench.layout.focused = group;
                self.check_out(id, window, cx);
                self.focus_page(window, cx);
                cx.notify();
            }
        }
    }

    /// A click anywhere in page group `group` makes it the focused group.
    fn click_group(&mut self, group: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if bench.layout.focused == group {
            return;
        }
        bench.layout.focused = group;
        if let Some(Tab::Page(id)) = bench
            .layout
            .page
            .groups
            .get(group)
            .and_then(|g| g.active_tab())
        {
            self.check_out(id, window, cx);
        }
        self.save_layout(cx);
        cx.notify();
    }

    /// Activate tab `index` of `group` in `place` (a click on a tab).
    pub(super) fn activate_tab(
        &mut self,
        place: Place,
        group: usize,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        let Some(tab) = bench
            .layout
            .area(place)
            .groups
            .get(group)
            .and_then(|g| g.tabs.get(index).copied())
        else {
            return;
        };
        bench.layout.activate(Spot {
            place,
            group,
            index,
        });
        self.after_layout(window, cx);
        match tab {
            Tab::View(v) => self.focus_view(v, window, cx),
            Tab::Page(_) => self.focus_page(window, cx),
        }
    }

    /// "Maximize focused area" (`Mod+Shift+Enter`): the focused group fills
    /// the body, and back. Not saved, not an undo step.
    pub(super) fn toggle_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let here = self.focused_group(window, cx);
        let Some(bench) = self.bench() else { return };
        bench.ui.zoomed = match bench.ui.zoomed {
            Some(_) => None,
            None => here,
        };
        cx.notify();
    }

    // ---- dragging (4.8.7) ----

    fn press(&mut self, dragged: Dragged, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if event.button != MouseButton::Left {
            return;
        }
        if let Some(ui) = self.layout_ui() {
            ui.press = Some(Press {
                dragged,
                x: f32::from(event.position.x),
                y: f32::from(event.position.y),
            });
        }
        cx.notify();
    }

    fn pointer_moved(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
        let Some(bench) = self.bench() else { return };
        if let Some(press) = bench.ui.press {
            if event.pressed_button != Some(MouseButton::Left) {
                bench.ui.press = None;
                return;
            }
            if (x - press.x).hypot(y - press.y) < DRAG_START {
                return;
            }
            bench.ui.press = None;
            bench.ui.menu = None;
            bench.ui.drag = Some(Drag {
                dragged: press.dragged,
                x,
                y,
                zone: None,
            });
            self.set_rearranging(true, window, cx);
        }
        let Some(drag) = self.bench().and_then(|b| b.ui.drag) else {
            return;
        };
        let close = self.page_to_close(drag.dragged);
        let Some(bench) = self.bench() else { return };
        let geometry = bench.ui.geometry.borrow();
        let regions = geometry.regions(&bench.layout);
        let page = geometry
            .areas
            .get(&place_key(Place::Page))
            .copied()
            .unwrap_or_default();
        let body = geometry.body;
        let mut strips = layout_frame::strips(&bench.layout, body, page);
        // An area shown while empty is a drop target like any area (4.8.2).
        for side in Side::ALL {
            let area = bench.layout.side(side);
            if area.shown
                && area.groups.is_empty()
                && let Some(rect) = geometry.areas.get(&place_key(Place::Side(side)))
            {
                strips.push(layout_frame::Strip {
                    side,
                    hit: *rect,
                    preview: *rect,
                });
            }
        }
        drop(geometry);
        let ctx = layout_frame::DropContext { body, close };
        let zone = layout_frame::zone_at(&bench.layout, &regions, &strips, drag.dragged, ctx, x, y);
        bench.ui.drag = Some(Drag { x, y, zone, ..drag });
        cx.notify();
    }

    fn pointer_released(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.ui.press = None;
        let Some(drag) = bench.ui.drag.take() else {
            return;
        };
        let Some(zone) = drag.zone else {
            // 4.8.11, rule 5: an invalid drop changes nothing.
            self.set_rearranging(false, window, cx);
            cx.notify();
            return;
        };
        let view = match drag.dragged.tab {
            Tab::View(v) => Some(v),
            Tab::Page(_) => drag.dragged.as_view,
        };
        let place = match zone.target {
            Target::Into { place, .. } | Target::Split { place, .. } => place,
            Target::Area(side) => Place::Side(side),
        };
        // A tab moved or split within the page area, page or view, is a tab
        // move: no toast (4.8.11).
        let from = bench.layout.locate(drag.dragged.tab).map(|s| s.place);
        let is_view_move = from != Some(Place::Page) || place != Place::Page;
        let dropped = self.drop_view(drag.dragged, zone.target, window, cx);
        // The web views return at their new bounds once the layout settled.
        self.set_rearranging(false, window, cx);
        if dropped {
            match (view, is_view_move) {
                (Some(view), true) => self.after_move(view, place, window, cx),
                (Some(view), false) if !view.is_page_kind() => self.focus_view(view, window, cx),
                _ => self.focus_page(window, cx),
            }
        }
        cx.notify();
    }

    /// Cancel a drag (`Esc`, the window losing focus): nothing changes.
    pub(super) fn cancel_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(bench) = self.bench() else {
            return false;
        };
        bench.ui.press = None;
        if bench.ui.drag.take().is_none() {
            return false;
        }
        self.set_rearranging(false, window, cx);
        cx.notify();
        true
    }

    /// Every web view hides for the whole drag (4.8.9).
    fn set_rearranging(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(windows)]
        if let Some(bench) = self.bench() {
            let chat = bench.chat.clone();
            let browser = bench.browser.clone();
            chat.update(cx, |chat, cx| chat.set_rearranging(on, window, cx));
            browser.update(cx, |browser, cx| browser.set_rearranging(on, window, cx));
        }
        #[cfg(not(windows))]
        let _ = (on, window, cx);
    }

    // ---- resizing (4.8.6) ----

    fn resize_moved(&mut self, handle: Handle, x: f32, y: f32, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if bench.ui.resize.as_ref().is_none_or(|r| r.handle != handle) {
            bench.ui.resize = Some(Resize {
                handle,
                before: bench.layout.clone(),
            });
        }
        let geometry = bench.ui.geometry.borrow();
        let body = geometry.body;
        let page = geometry
            .areas
            .get(&place_key(Place::Page))
            .copied()
            .unwrap_or(body);
        let area_rect = |place: Place| geometry.areas.get(&place_key(place)).copied();
        match handle {
            Handle::Side(side) => {
                let wanted = match side {
                    Side::Left => x - body.x,
                    Side::Right => body.right() - x,
                    Side::Bottom => page.bottom() - y,
                };
                let before = bench.ui.resize.as_ref().map(|r| r.before.clone());
                let min = layout_frame::side_min(&bench.layout, side);
                match layout_frame::drag_size(wanted, min) {
                    Some(size) => {
                        let area = bench.layout.side_mut(side);
                        area.shown = true;
                        area.size = size;
                    }
                    None => {
                        // Snapped hidden: it keeps the size it had before.
                        let area = bench.layout.side_mut(side);
                        area.shown = false;
                        if let Some(before) = before {
                            area.size = before.side(side).size;
                        }
                    }
                }
            }
            Handle::Groups(place, at) => {
                let Some(rect) = area_rect(place) else { return };
                let area = bench.layout.area(place);
                let (total, start, pos) = match area.direction {
                    Direction::Row => (rect.w, rect.x, x),
                    Direction::Column => (rect.h, rect.y, y),
                };
                let mins = match place {
                    Place::Page => layout_frame::page_mins(&bench.layout),
                    Place::Side(side) => layout_frame::side_group_mins(&bench.layout, side),
                };
                let Some(edge) =
                    layout_frame::handle_position(start, total, &area.shares, &mins, at)
                else {
                    return;
                };
                let shares =
                    layout_frame::resize_shares(&area.shares, at, pos - edge, total, &mins);
                drop(geometry);
                bench.layout.area_mut(place).shares = shares;
                cx.notify();
                return;
            }
        }
        drop(geometry);
        cx.notify();
    }

    /// The end of a resize drag: one undo step, saved now (never during).
    pub(super) fn resize_ended(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let Some(resize) = bench.ui.resize.take() else {
            return;
        };
        if bench.layout != resize.before {
            let seq = crate::workspace::next_undo_seq();
            bench.layout_undo.record(resize.before, seq, None);
            self.after_layout(window, cx);
        }
    }

    /// Double-click on a handle: the area's default size, or equal shares.
    fn reset_handle(&mut self, handle: Handle, window: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(None, window, cx, |l| {
            match handle {
                Handle::Side(side) => {
                    let area = l.side_mut(side);
                    area.size = side.default_size();
                    area.shown = true;
                }
                Handle::Groups(place, _) => {
                    let n = l.area(place).groups.len();
                    l.area_mut(place).shares = crate::layout::equal(n);
                }
            }
            true
        });
    }

    // ---- menus ----

    pub(crate) fn open_menu(&mut self, kind: LayoutMenu, x: f32, y: f32, cx: &mut Context<Self>) {
        if let Some(ui) = self.layout_ui() {
            ui.menu = match ui.menu {
                Some(m) if m.kind == kind => None,
                _ => Some(Menu { kind, x, y }),
            };
        }
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(ui) = self.layout_ui() {
            ui.menu = None;
        }
        cx.notify();
    }

    // ---- rendering ----

    /// The workbench between the titlebar and the status bar (4.8).
    pub(super) fn workbench_body(
        &mut self,
        outlined: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let frame_px = if outlined { 2. } else { 0. };
        let (width, body_h) = body_size(window);
        let (width, body_h) = (width - frame_px, body_h - frame_px);
        let first_run = matches!(self.bench().map(|b| &b.page), Some(PageView::FirstRun(_)));
        let toast = self.toast_element(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        bench.ui.geometry.borrow_mut().clear();
        let geometry = Rc::clone(&bench.ui.geometry);
        if first_run {
            let page = bench.page.view();
            return div()
                .relative()
                .flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(div().size_full().child(page))
                .children(toast)
                .into_any_element();
        }
        let f = layout_frame::frame(&bench.layout, width, body_h);
        if !f.left_auto_hidden {
            bench.ui.left_overlay = false;
        }
        let zoomed = bench.ui.zoomed;
        self.sync_chat_visibility(&f, window, cx);
        let bulk = self.bulk_notice(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let left_overlay = f.left_auto_hidden && bench.ui.left_overlay;
        let body: AnyElement = if let Some((place, group)) = zoomed {
            let content = match place {
                Place::Page => self.page_group(group, true, bulk, window, cx),
                Place::Side(side) => self.side_group(side, group, false, window, cx),
            };
            div()
                .flex()
                .flex_1()
                .size_full()
                .child(content)
                .into_any_element()
        } else {
            let left = f
                .left
                .map(|w| self.side_area(Side::Left, w, &f, window, cx));
            let right = f
                .right
                .map(|w| self.side_area(Side::Right, w, &f, window, cx));
            let bottom = f
                .bottom
                .map(|h| self.side_area(Side::Bottom, h, &f, window, cx));
            let page = self.page_area(&f, bulk, toast, window, cx);
            let column = div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(page)
                .children(bottom.map(|b| {
                    div()
                        .flex()
                        .flex_col()
                        .flex_shrink_0()
                        .w_full()
                        .child(self.handle(Handle::Side(Side::Bottom), &t, cx))
                        .child(b)
                }));
            div()
                .flex()
                .flex_row()
                .size_full()
                .children(left.map(|l| {
                    div()
                        .flex()
                        .flex_row()
                        .flex_shrink_0()
                        .h_full()
                        .child(l)
                        .child(self.handle(Handle::Side(Side::Left), &t, cx))
                }))
                .child(column)
                .children(right.map(|r| {
                    div()
                        .flex()
                        .flex_row()
                        .flex_shrink_0()
                        .h_full()
                        .child(self.handle(Handle::Side(Side::Right), &t, cx))
                        .child(r)
                }))
                .into_any_element()
        };
        let overlay = left_overlay.then(|| {
            let w = layout_frame::sidebar_width(
                &self.bench().expect("workbench").layout,
                Side::Left,
                width,
            )
            .min(width - OVERLAY_GAP);
            let area = self.side_area(Side::Left, w, &f, window, cx);
            deferred(
                div()
                    .id("sidebar-overlay")
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .border_r_1()
                    .border_color(t.border_default)
                    .shadow(vec![t.shadow_overlay()])
                    .on_mouse_down_out(
                        cx.listener(|shell, _, window, cx| shell.dismiss_sidebar(window, cx)),
                    )
                    .child(area),
            )
            .with_priority(1)
            .into_any_element()
        });
        let drag_layer = self.drag_layer(&t, cx);
        let menu = self.menu_element(window, cx);
        let pointer = self.pointer_listener(cx);
        let body_measure = measure(&geometry, |g, r| g.body = r);
        div()
            .id("workbench-body")
            .relative()
            .flex()
            .flex_row()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(body_measure)
            .child(body)
            .children(overlay)
            .children(drag_layer)
            .children(menu)
            .child(pointer)
            .into_any_element()
    }

    /// The chat is shown while it is on screen: visible in the layout, not
    /// in a folded group, not under a maximized other area.
    fn sync_chat_visibility(
        &mut self,
        f: &layout_frame::Frame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(windows)]
        {
            let browser_on_screen = self.view_on_screen(ViewId::AgentBrowser, f);
            let browser_covered = self.web_covered(ViewId::AgentBrowser, cx);
            if let Some(bench) = self.bench() {
                let browser = bench.browser.clone();
                browser.update(cx, |browser, cx| {
                    browser.set_shown(browser_on_screen, window, cx);
                    browser.set_covered(browser_covered, window, cx);
                });
            }
            let on_screen = self.view_on_screen(ViewId::Chat, f);
            let covered = self.web_covered(ViewId::Chat, cx);
            let Some(bench) = self.bench() else { return };
            let chat = bench.chat.clone();
            if bench.ui.chat_shown != on_screen {
                bench.ui.chat_shown = on_screen;
                chat.update(cx, |chat, cx| {
                    if on_screen {
                        chat.show(window, cx)
                    } else {
                        chat.hide(cx)
                    }
                });
            }
            chat.update(cx, |chat, cx| chat.set_covered(covered, window, cx));
        }
        #[cfg(not(windows))]
        let _ = (f, window, cx);
    }

    /// Whether `view` is drawn this frame.
    #[cfg(windows)]
    fn view_on_screen(&mut self, view: ViewId, f: &layout_frame::Frame) -> bool {
        let Some(bench) = self.bench() else {
            return false;
        };
        layout_frame::on_screen(&bench.layout, f, bench.ui.shown(), view)
    }

    /// A side area: its groups along its direction, or the empty line.
    fn side_area(
        &mut self,
        side: Side,
        size: f32,
        f: &layout_frame::Frame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let geometry = Rc::clone(&bench.ui.geometry);
        let place = Place::Side(side);
        let n = bench.layout.side(side).groups.len();
        let folded = f.side_folded(side);
        let shares = bench.layout.side(side).shares.clone();
        let mins = layout_frame::side_group_mins(&bench.layout, side);
        let container = div()
            .relative()
            .flex()
            .flex_shrink_0()
            .bg(t.bg_sunken)
            .map(|this| match side {
                Side::Left | Side::Right => this.flex_col().w(px(size)).h_full(),
                Side::Bottom => this.flex_row().h(px(size)).w_full(),
            })
            .child(measure(&geometry, move |g, r| {
                g.areas.insert(place_key(place), r);
            }));
        if n == 0 {
            return container
                .child(
                    type_scale::SMALL
                        .el()
                        .p(px(12.))
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::AreaEmpty)),
                )
                .into_any_element();
        }
        let shown = if folded { 1 } else { n };
        let mut out = container;
        for (g, share) in shares.iter().copied().enumerate().take(shown) {
            if g > 0 {
                out = out.child(self.handle(Handle::Groups(place, g - 1), &t, cx));
            }
            let group = self.side_group(side, g, folded, window, cx);
            let share = if shown == 1 { 1. } else { share };
            let _ = &mins;
            out = out.child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .min_h_0()
                    .flex_basis(gpui_kit::relative(0.))
                    .flex_grow(share)
                    .child(group),
            );
        }
        out.into_any_element()
    }

    /// One group of a side area: its header and its active view.
    fn side_group(
        &mut self,
        side: Side,
        group: usize,
        folded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let place = Place::Side(side);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let geometry = Rc::clone(&bench.ui.geometry);
        let Some(view) =
            bench
                .layout
                .side(side)
                .groups
                .get(group)
                .and_then(|g| match g.active_tab() {
                    Some(Tab::View(v)) => Some(v),
                    _ => None,
                })
        else {
            return div().into_any_element();
        };
        let header = self.area_header(place, group, folded && group == 0, window, cx);
        let content = self.view_content(view, place, window, cx);
        let key = place_key(place);
        div()
            .id(("side-group", (key as usize) * 4 + group))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .child(measure(&geometry, move |g, r| {
                g.groups.insert((key, group), r);
                g.headers
                    .insert((key, group), Rect::new(r.x, r.y, r.w, HEADER));
            }))
            .child(header)
            .child(div().flex().flex_col().flex_1().min_h_0().child(content))
            .into_any_element()
    }

    /// 4.8.4: the area header of a group: the view's name or the view tabs,
    /// the context, the view's buttons, the view menu and `minus`.
    fn area_header(
        &mut self,
        place: Place,
        group: usize,
        folded_more: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let geometry = Rc::clone(&bench.ui.geometry);
        let dragging = bench.ui.drag.map(|d| d.dragged.tab);
        let g = bench.layout.area(place).groups[group].clone();
        let Some(Tab::View(active)) = g.active_tab() else {
            return div().into_any_element();
        };
        let key = place_key(place);
        let single = g.tabs.len() == 1;
        // The tabs keep their room (they are fitted to the header below);
        // the context takes what is left and gives way inside it.
        let mut left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .flex_shrink_0();
        let mut left_width = 0.;
        if single {
            let label = text(locale, view_name(active));
            left_width = ui::text_width(type_scale::LABEL, &label, cx);
            let name = ui::label(label, &t)
                .flex_shrink_0()
                .relative()
                .child(measure(&geometry, move |g, r| {
                    g.tabs.insert((key, group, 0), r);
                }));
            left = left.child(name);
        } else {
            // 4.8.4: the tabs that do not fit give way to a chevron. The
            // room is the group's width less the padding and the buttons.
            // The view's own buttons, its menu, and the area's `minus`.
            let own = match active {
                ViewId::Terminal => 1,
                ViewId::Chat | ViewId::AgentBrowser => 2,
                _ => 0,
            };
            let buttons_n = own + 1 + usize::from(matches!(place, Place::Side(_)));
            let fit = self.bench().and_then(|bench| {
                let width = *bench.ui.geometry.borrow().group_widths.get(&(key, group))?;
                // The terminal's context keeps its smallest form (layout.md
                // 4.5): the tabs give way first.
                let reserve = if active == ViewId::Terminal {
                    crate::terminal_dock::HEADER_RESERVE
                } else {
                    0.
                };
                let available = width - 16. - buttons_n as f32 * 32. - 8. - reserve;
                let widths: Vec<f32> = g
                    .tabs
                    .iter()
                    .map(|tab| match tab {
                        // Measured: padding 8 + 8 around the label; a busy
                        // dot (gap 4, dot 6) is allowed for.
                        Tab::View(v) => {
                            ui::text_width(type_scale::LABEL, &text(locale, view_name(*v)), cx)
                                + 16.
                                + if *v == active { 0. } else { 10. }
                        }
                        Tab::Page(_) => 0.,
                    })
                    .collect();
                let fit = fitting_tabs(&widths, g.active, available);
                Some((fit, widths))
            });
            let (fit, widths) = match fit {
                Some((fit, widths)) => (fit, Some(widths)),
                None => (None, None),
            };
            if let Some(widths) = &widths {
                let shown: Vec<f32> = widths
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| fit.as_ref().is_none_or(|f| f.contains(i)))
                    .map(|(_, w)| *w)
                    .collect();
                left_width = shown.iter().sum::<f32>()
                    + 4. * shown.len().saturating_sub(1) as f32
                    + fit.as_ref().map_or(0., |_| OVERFLOW_CHEVRON + 4.);
            }
            for (i, tab) in g.tabs.iter().enumerate() {
                let Tab::View(v) = *tab else { continue };
                if fit.as_ref().is_some_and(|shown| !shown.contains(&i)) {
                    continue;
                }
                left = left.child(self.view_tab(
                    place,
                    group,
                    i,
                    v,
                    i == g.active,
                    dragging == Some(*tab),
                    &t,
                    cx,
                ));
            }
            if fit.is_some() {
                left = left.child(
                    ui::ghost_icon_button(
                        format!("view-tabs-more-{key}-{group}"),
                        Icon::ChevronDown,
                        OVERFLOW_CHEVRON,
                        false,
                        false,
                        true,
                        &t,
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(
                        move |shell, e: &gpui_kit::ClickEvent, _, cx| {
                            let p = e.position();
                            shell.open_menu(
                                LayoutMenu::ViewTabs(place, group),
                                f32::from(p.x) - 12.,
                                f32::from(p.y) + 12.,
                                cx,
                            )
                        },
                    )),
                );
            }
        }
        // The group's width less its padding (16), the tabs, the gaps around
        // the context and the spacer (3 x 8), the buttons (24 + 8 each) and
        // the folded chevron.
        let available = {
            let own = match active {
                ViewId::Terminal => 1,
                ViewId::Chat | ViewId::AgentBrowser => 2,
                _ => 0,
            };
            let buttons_n = own
                + usize::from(!active.is_web())
                + usize::from(matches!(place, Place::Side(_)))
                + usize::from(folded_more);
            geometry
                .borrow()
                .group_widths
                .get(&(key, group))
                .map(|w| (w - 16. - left_width - 24. - buttons_n as f32 * 32.).max(0.))
        };
        let context = self.view_context(active, available, &t, cx);
        let buttons = self.view_buttons(active, place, cx);
        let side = match place {
            Place::Side(side) => Some(side),
            Place::Page => None,
        };
        let hide_tip = side.map(|s| {
            let key = match s {
                Side::Left => Key::AreaHideLeft,
                Side::Right => Key::AreaHideRight,
                Side::Bottom => Key::AreaHideBottom,
            };
            (text(locale, key), area_keys(s, macos, locale))
        });
        let has_own_menu = active.is_web();
        let id = SharedString::from(format!("area-header-{key}-{group}"));
        let dragged = Dragged {
            tab: Tab::View(active),
            as_view: None,
        };
        let header = div()
            .id(id)
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .h(px(HEADER))
            .px(px(8.))
            .bg(t.bg_sunken)
            .border_b_1()
            .border_color(t.border_subtle)
            .when(single, |this| {
                // A header whose group has one view drags that view.
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |shell, e: &MouseDownEvent, _, cx| {
                        shell.press(dragged, e, cx)
                    }),
                )
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    shell.open_menu(
                        LayoutMenu::View(active),
                        f32::from(e.position.x),
                        f32::from(e.position.y),
                        cx,
                    )
                }),
            )
            .child(left)
            .children(context)
            .child(div().flex_1())
            .when(folded_more, |this| {
                this.child(
                    ui::ghost_icon_button(
                        format!("folded-{key}"),
                        Icon::ChevronDown,
                        24.,
                        false,
                        false,
                        true,
                        &t,
                    )
                    .on_click(cx.listener(
                        move |shell, e: &gpui_kit::ClickEvent, _, cx| {
                            let p = e.position();
                            if let Place::Side(side) = place {
                                shell.open_menu(
                                    LayoutMenu::FoldedSide(side),
                                    f32::from(p.x) - 180.,
                                    f32::from(p.y) + 12.,
                                    cx,
                                )
                            }
                        },
                    )),
                )
            })
            .children(buttons)
            .when(!has_own_menu, |this| {
                this.child(
                    ui::ghost_icon_button(
                        format!("view-menu-{key}-{group}"),
                        Icon::Ellipsis,
                        24.,
                        false,
                        false,
                        true,
                        &t,
                    )
                    .tooltip(ui::tip(text(locale, Key::ViewActions)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(
                        move |shell, e: &gpui_kit::ClickEvent, _, cx| {
                            let p = e.position();
                            shell.open_menu(
                                LayoutMenu::View(active),
                                f32::from(p.x) - 200.,
                                f32::from(p.y) + 12.,
                                cx,
                            )
                        },
                    )),
                )
            })
            .children(side.zip(hide_tip).map(|(side, (tip, keys))| {
                ui::ghost_icon_button(
                    format!("area-hide-{key}-{group}"),
                    Icon::Minus,
                    24.,
                    false,
                    false,
                    true,
                    &t,
                )
                .tooltip(ui::tip_keys(tip, keys))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.change_layout(None, window, cx, |l| l.set_area_shown(side, false));
                    shell.focus_page(window, cx);
                }))
            }));
        let _ = window;
        header.into_any_element()
    }

    /// A view tab (4.8.4): 24 high, `radius.sm`, `type.label`; tone carries
    /// the active one.
    #[allow(clippy::too_many_arguments)]
    fn view_tab(
        &mut self,
        place: Place,
        group: usize,
        index: usize,
        view: ViewId,
        active: bool,
        dragged: bool,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locale = settings::locale(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let geometry = Rc::clone(&bench.ui.geometry);
        let busy = view_busy(bench, view, cx);
        let key = place_key(place);
        let (hover, pressed, secondary) = (t.fill_hover, t.fill_pressed, t.text_secondary);
        let drag = Dragged {
            tab: Tab::View(view),
            as_view: None,
        };
        ui::focusable(
            div().id(SharedString::from(format!(
                "view-tab-{key}-{group}-{index}"
            ))),
            t,
            false,
        )
        .relative()
        .flex()
        .items_center()
        .flex_shrink_0()
        .h(px(24.))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .when(active, |this| {
            this.bg(t.fill_selected).text_color(t.text_primary)
        })
        .when(!active, |this| {
            this.text_color(t.text_tertiary)
                .hover(move |s| s.bg(hover).text_color(secondary))
                .active(move |s| s.bg(pressed).text_color(secondary))
        })
        .when(dragged, |this| this.opacity(0.6))
        .child(type_scale::LABEL.el().child(text(locale, view_name(view))))
        .when(!active && busy, |this| this.gap(px(4.)).child(busy_dot(t)))
        .child(measure(&geometry, move |g, r| {
            g.tabs.insert((key, group, index), r);
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |shell, e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                shell.press(drag, e, cx)
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |shell, e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                shell.open_menu(
                    LayoutMenu::View(view),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                    cx,
                )
            }),
        )
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.activate_tab(place, group, index, window, cx)
        }))
        // 4.8.4: `Left` / `Right` move between view tabs, `Shift+F10` or
        // the menu key opens the view menu; `Enter` and `Space` click.
        .on_key_down(
            cx.listener(move |shell, e: &gpui_kit::KeyDownEvent, window, cx| {
                let k = &e.keystroke;
                match k.key.as_str() {
                    "left" if !k.modifiers.modified() => window.focus_prev(cx),
                    "right" if !k.modifiers.modified() => window.focus_next(cx),
                    "f10" if k.modifiers.shift => {
                        let at = shell.bench().and_then(|b| {
                            b.ui.geometry
                                .borrow()
                                .tabs
                                .get(&(key, group, index))
                                .copied()
                        });
                        let (x, y) = at.map_or((0., 0.), |r| (r.x, r.bottom()));
                        shell.open_menu(LayoutMenu::View(view), x, y, cx);
                    }
                    "menu" => {
                        let at = shell.bench().and_then(|b| {
                            b.ui.geometry
                                .borrow()
                                .tabs
                                .get(&(key, group, index))
                                .copied()
                        });
                        let (x, y) = at.map_or((0., 0.), |r| (r.x, r.bottom()));
                        shell.open_menu(LayoutMenu::View(view), x, y, cx);
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }),
        )
        .into_any_element()
    }

    /// The context text of a view's header or strip (4.8.4).
    /// `available`: the width the header leaves the context, once the
    /// tabs and the buttons have theirs (`None` before the first layout).
    fn view_context(
        &mut self,
        view: ViewId,
        available: Option<f32>,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let bench = self.bench()?;
        match view {
            ViewId::Terminal => {
                let dock = bench.dock.clone();
                dock.update(cx, |dock, cx| dock.header_context(available, cx))
            }
            #[cfg(windows)]
            ViewId::Chat => Some(
                type_scale::SMALL
                    .el()
                    .text_color(t.text_tertiary)
                    .child("claude.ai")
                    .into_any_element(),
            ),
            #[cfg(windows)]
            ViewId::AgentBrowser => bench.browser.read(cx).header_context(cx),
            ViewId::Sessions => bench.sessions.read(cx).header_context(cx),
            ViewId::Inbox | ViewId::Knowledge => {
                let project = bench.active_project?;
                let name = bench
                    .ws
                    .read(cx)
                    .projects
                    .iter()
                    .find(|p| p.project.id == project)
                    .map(|p| p.project.title.clone())?;
                Some(
                    type_scale::SMALL
                        .el()
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_tertiary)
                        .child(name)
                        .into_any_element(),
                )
            }
            _ => None,
        }
    }

    /// The view's own header buttons (terminal: Restart here; chat: Open in
    /// browser, Reload, and its menu holding the view menu).
    fn view_buttons(
        &mut self,
        view: ViewId,
        place: Place,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let bench = self.bench()?;
        match view {
            ViewId::Terminal => {
                let dock = bench.dock.clone();
                Some(dock.update(cx, |dock, cx| dock.header_buttons(cx)))
            }
            #[cfg(windows)]
            ViewId::Chat => {
                let can_split = bench.layout.can_split_off(ViewId::Chat);
                let chat = bench.chat.clone();
                Some(chat.update(cx, |chat, cx| chat.header_buttons(place, can_split, cx)))
            }
            #[cfg(windows)]
            ViewId::AgentBrowser => {
                let can_split = bench.layout.can_split_off(ViewId::AgentBrowser);
                let browser = bench.browser.clone();
                Some(browser.update(cx, |b, cx| b.header_buttons(place, can_split, cx)))
            }
            _ => {
                let _ = place;
                None
            }
        }
    }

    /// What a view shows below its header.
    fn view_content(
        &mut self,
        view: ViewId,
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        match view {
            ViewId::Projects => {
                let _ = place;
                self.projects_view(window, cx)
            }
            ViewId::Terminal => bench.dock.clone().into_any_element(),
            ViewId::Sessions => bench.sessions.clone().into_any_element(),
            #[cfg(windows)]
            ViewId::Chat => bench.chat.clone().into_any_element(),
            #[cfg(windows)]
            ViewId::AgentBrowser => bench.browser.clone().into_any_element(),
            #[cfg(not(windows))]
            ViewId::Chat | ViewId::AgentBrowser => div().into_any_element(),
            ViewId::Inbox | ViewId::Knowledge => {
                let focus = match view {
                    ViewId::Inbox => bench.inbox_focus.clone(),
                    _ => bench.knowledge_focus.clone(),
                };
                let page = match view {
                    ViewId::Inbox => bench.inbox_view.clone().map(|v| v.into_any_element()),
                    _ => bench.knowledge_view.clone().map(|v| v.into_any_element()),
                };
                // A side area's width less the view's padding of 12 a side;
                // the bottom panel is as wide as the window.
                let column = match place {
                    Place::Side(Side::Left | Side::Right) => bench.layout.area(place).size - 24.,
                    _ => f32::MAX,
                };
                let page = page.map(|p| ui::NarrowPage::new(p, false).column(column));
                div()
                    .id("view-page")
                    .track_focus(&focus)
                    .size_full()
                    .bg(t.bg_sunken)
                    .children(page)
                    .into_any_element()
            }
        }
    }

    /// The page area: its groups along its direction, or only the focused
    /// one while folded (4.8.10).
    fn page_area(
        &mut self,
        f: &layout_frame::Frame,
        bulk: Option<AnyElement>,
        toast: Option<AnyElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let geometry = Rc::clone(&bench.ui.geometry);
        let n = bench.layout.page.groups.len();
        let direction = bench.layout.page.direction;
        let shares = bench.layout.page.shares.clone();
        let focused = bench.layout.focused.min(n - 1);
        let mut area = div()
            .id("page-area")
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .map(|this| match direction {
                Direction::Row => this.flex_row(),
                Direction::Column => this.flex_col(),
            })
            .child(measure(&geometry, |g, r| {
                g.areas.insert(place_key(Place::Page), r);
            }));
        let mut bulk = bulk;
        if f.page_folded {
            area = area.child(self.page_group(focused, true, bulk.take(), window, cx));
        } else {
            for (g, share) in shares.iter().copied().enumerate() {
                if g > 0 {
                    area = area.child(self.handle(Handle::Groups(Place::Page, g - 1), &t, cx));
                }
                let group = self.page_group(
                    g,
                    false,
                    if g == focused { bulk.take() } else { None },
                    window,
                    cx,
                );
                let share = if n == 1 { 1. } else { share };
                area = area.child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .min_h_0()
                        .flex_basis(gpui_kit::relative(0.))
                        .flex_grow(share)
                        .child(group),
                );
            }
        }
        area.children(toast).into_any_element()
    }

    /// One page group: its tab bar (when shown), the view strip of a view
    /// tab, and the active tab's page or view.
    fn page_group(
        &mut self,
        group: usize,
        folded: bool,
        bulk: Option<AnyElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let geometry = Rc::clone(&bench.ui.geometry);
        let n = bench.layout.page.groups.len();
        let total_tabs: usize = bench.layout.page.groups.iter().map(|g| g.tabs.len()).sum();
        let show_bar = n >= 2 || total_tabs >= 2;
        let Some(g) = bench.layout.page.groups.get(group).cloned() else {
            return div().into_any_element();
        };
        let focus = bench.group_focus[group.min(2)].clone();
        let bar = show_bar.then(|| self.tab_bar(group, folded, &t, cx));
        let (content, strip): (AnyElement, Option<AnyElement>) = match g.active_tab() {
            Some(Tab::Page(id)) => {
                let Some(bench) = self.bench() else {
                    return div().into_any_element();
                };
                let view = if id == bench.current {
                    Some(bench.page.view())
                } else {
                    bench
                        .slots
                        .get(&id)
                        .and_then(|s| s.view.as_ref())
                        .map(|v| v.view())
                };
                (div().size_full().children(view).into_any_element(), None)
            }
            Some(Tab::View(v)) => {
                let strip = matches!(v, ViewId::Terminal | ViewId::Chat | ViewId::AgentBrowser)
                    .then(|| self.view_strip(v, &t, cx));
                (self.view_content(v, Place::Page, window, cx), strip)
            }
            None => (div().into_any_element(), None),
        };
        let header_h = if show_bar { TAB_BAR } else { 0. };
        // 4.8.8: under 452 of group width, pages pad 16 a side.
        let group_width = geometry.borrow().page_widths[group.min(2)];
        let narrow = group_width < NARROW_GROUP;
        div()
            .id(("page-group", group))
            .track_focus(&focus)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg_base)
            .capture_any_mouse_down(
                cx.listener(move |shell, _, window, cx| shell.click_group(group, window, cx)),
            )
            .child(measure(&geometry, move |gm, r| {
                gm.groups.insert((place_key(Place::Page), group), r);
                if header_h > 0. {
                    gm.headers.insert(
                        (place_key(Place::Page), group),
                        Rect::new(r.x, r.y, r.w, header_h),
                    );
                }
            }))
            .children(bar)
            .children(strip)
            .children(bulk)
            .child(
                div().flex_1().min_h_0().child(
                    ui::NarrowPage::new(content, narrow)
                        .column(ui::page_column_width(group_width, narrow)),
                ),
            )
            .into_any_element()
    }

    /// 4.8.3: the 28 strip under a page-area tab bar for a view tab: its
    /// context and its buttons.
    fn view_strip(&mut self, view: ViewId, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let context = self.view_context(view, None, t, cx);
        let buttons = self.view_buttons(view, Place::Page, cx);
        div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .h(px(VIEW_STRIP))
            .px(px(8.))
            .bg(t.bg_sunken)
            .border_b_1()
            .border_color(t.border_subtle)
            .children(context)
            .child(div().flex_1())
            .children(buttons)
            .into_any_element()
    }

    /// The tab bar of a page group (4.4, 4.8.8).
    fn tab_bar(
        &mut self,
        group: usize,
        folded: bool,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locale = settings::locale(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let g = bench.layout.page.groups[group].clone();
        let dragging = bench.ui.drag.map(|d| d.dragged.tab);
        let geometry = Rc::clone(&bench.ui.geometry);
        let mut bar = div()
            .id(("tab-bar", group))
            .flex()
            .flex_row()
            .flex_shrink_0()
            .h(px(TAB_BAR))
            .w_full()
            .overflow_x_scroll()
            .bg(t.bg_sunken)
            .border_b_1()
            .border_color(t.border_subtle);
        let ws = bench.ws.clone();
        for (i, tab) in g.tabs.iter().enumerate() {
            let is_active = i == g.active;
            // 4.4: the dot of a background tab whose page another process
            // changed; the close button takes its place on hover.
            let unread = !is_active
                && match tab {
                    Tab::Page(id) => match self.route_of(*id) {
                        Some(Route::Topic(t)) => {
                            ws.read(cx).is_unread(acta_core::model::Record::Topic(t))
                        }
                        Some(Route::Kb(k)) => {
                            ws.read(cx).is_unread(acta_core::model::Record::Kb(k))
                        }
                        _ => false,
                    },
                    Tab::View(_) => false,
                };
            let (leading, title, view_kind, closable) = match *tab {
                Tab::Page(id) => {
                    let route = self.route_of(id);
                    let Some(route) = route else { continue };
                    let (glyph, status, title) =
                        self.tab_label(route, self.is_current(id), locale, cx);
                    let leading = match status {
                        Some(status) => ui::status_glyph(status, 14., t).into_any_element(),
                        None => icon(glyph, 14., t.text_secondary).into_any_element(),
                    };
                    let kind = match route {
                        Route::Inbox(_) => Some(ViewId::Inbox),
                        Route::Knowledge(_) => Some(ViewId::Knowledge),
                        _ => None,
                    };
                    (leading, title, kind, true)
                }
                Tab::View(v) => (
                    icon(view_icon(v), 14., t.text_secondary).into_any_element(),
                    text(locale, view_name(v)),
                    None,
                    true,
                ),
            };
            let tab_v = *tab;
            let hover_group = SharedString::from(format!("tab-{group}-{i}"));
            let hover_group_dot = hover_group.clone();
            let hover = t.fill_hover;
            let close = ui::ghost_icon_button(
                format!("tab-close-{group}-{i}"),
                Icon::X,
                16.,
                false,
                false,
                true,
                t,
            )
            .when(!is_active, |this| {
                this.invisible()
                    .group_hover(hover_group.clone(), |s| s.visible())
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, window, cx| {
                cx.stop_propagation();
                shell.close_tab_of(tab_v, window, cx)
            }));
            let drag = Dragged {
                tab: tab_v,
                as_view: view_kind,
            };
            let key = place_key(Place::Page);
            bar = bar.child(
                ui::focusable(
                    div().id(SharedString::from(format!("tab-{group}-{i}"))),
                    t,
                    false,
                )
                .group(hover_group)
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h_full()
                .px(px(12.))
                .min_w(px(120.))
                .max_w(px(220.))
                .flex_shrink(1.)
                .when(is_active, |this| this.bg(t.bg_base))
                .when(!is_active, |this| this.hover(move |s| s.bg(hover)))
                .when(dragging == Some(tab_v), |this| this.opacity(0.6))
                .child(measure(&geometry, move |g, r| {
                    g.tabs.insert((key, group, i), r);
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |shell, e: &MouseDownEvent, _, cx| shell.press(drag, e, cx)),
                )
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell.activate_tab(Place::Page, group, i, window, cx)
                }))
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |shell, _, window, cx| shell.close_tab_of(tab_v, window, cx)),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |shell, e: &MouseDownEvent, _, cx| {
                        let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
                        match (tab_v, view_kind) {
                            (Tab::View(v), _) => shell.open_menu(LayoutMenu::View(v), x, y, cx),
                            (Tab::Page(id), Some(v)) => {
                                shell.open_menu(LayoutMenu::PageTab(id, v), x, y, cx)
                            }
                            _ => {}
                        }
                    }),
                )
                .child(leading)
                .child(
                    type_scale::UI
                        .el()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(if is_active {
                            t.text_primary
                        } else {
                            t.text_secondary
                        })
                        .child(title),
                )
                .map(|this| {
                    if !closable {
                        this
                    } else if unread {
                        this.child(
                            div()
                                .relative()
                                .flex_shrink_0()
                                .size(px(16.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    busy_dot(t)
                                        .group_hover(hover_group_dot.clone(), |s| s.invisible()),
                                )
                                .child(div().absolute().top_0().left_0().child(close)),
                        )
                    } else {
                        this.child(close)
                    }
                }),
            );
        }
        if folded {
            bar = bar.child(div().flex_1()).child(
                div().flex().items_center().px(px(4.)).child(
                    ui::ghost_icon_button(
                        format!("folded-tabs-{group}"),
                        Icon::ChevronDown,
                        24.,
                        false,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(
                        |shell, e: &gpui_kit::ClickEvent, _, cx| {
                            let p = e.position();
                            shell.open_menu(
                                LayoutMenu::Folded,
                                f32::from(p.x) - 200.,
                                f32::from(p.y) + 12.,
                                cx,
                            )
                        },
                    )),
                ),
            );
        }
        bar.into_any_element()
    }

    /// Whether `tab` is the checked-out page tab.
    fn is_current(&self, tab: TabId) -> bool {
        matches!(&self.content, Content::Workbench(b) if b.current == tab)
    }

    /// The route a page tab shows.
    pub(super) fn route_of(&self, tab: TabId) -> Option<Route> {
        let Content::Workbench(bench) = &self.content else {
            return None;
        };
        if tab == bench.current {
            bench.history.current()
        } else {
            bench.slots.get(&tab).and_then(|s| s.history.current())
        }
    }

    /// Close a tab: a page closes (the last page of the window cannot); a
    /// view tab hides its view (4.8.8).
    pub(super) fn close_tab_of(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        match tab {
            Tab::View(v) => self.view_action(v, ViewAction::Hide, window, cx),
            Tab::Page(id) => self.close_page(id, window, cx),
        }
    }

    /// A resize handle (components.md 5, 4.8.6).
    fn handle(&mut self, handle: Handle, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let vertical = match handle {
            Handle::Side(Side::Bottom) => false,
            Handle::Side(_) => true,
            Handle::Groups(place, _) => {
                let dir = self
                    .bench()
                    .map_or(Direction::Row, |b| b.layout.area(place).direction);
                dir == Direction::Row
            }
        };
        let resizing = self
            .bench()
            .and_then(|b| b.ui.resize.as_ref().map(|r| r.handle))
            == Some(handle);
        let id = SharedString::from(format!("resize-{handle:?}"));
        let accent = t.accent;
        let reach = crate::layout_frame::HANDLE_HIT - 1.;
        // The hit area lies over the line and the area before it, so a web
        // view after it never covers it (4.8.6). For the right sidebar and
        // the bottom panel "before" is the page.
        let line = div()
            .absolute()
            .map(|this| {
                if vertical {
                    this.top_0().bottom_0().right_0().w(px(2.))
                } else {
                    this.left_0().right_0().bottom_0().h(px(2.))
                }
            })
            .when(resizing, |this| this.bg(accent))
            .group_hover(id.clone(), move |s| s.bg(accent));
        let hit = div()
            .id(id.clone())
            .group(id)
            .absolute()
            .map(|this| {
                if vertical {
                    this.top_0()
                        .bottom_0()
                        .left(px(-reach))
                        .w(px(crate::layout_frame::HANDLE_HIT))
                        .cursor(CursorStyle::ResizeLeftRight)
                } else {
                    this.left_0()
                        .right_0()
                        .top(px(-reach))
                        .h(px(crate::layout_frame::HANDLE_HIT))
                        .cursor(CursorStyle::ResizeUpDown)
                }
            })
            .on_drag(ResizeDrag(handle), |_, _, _, cx| {
                cx.new(|_| gpui_kit::EmptyView)
            })
            .on_click(
                cx.listener(move |shell, event: &gpui_kit::ClickEvent, window, cx| {
                    if event.click_count() >= 2 {
                        shell.reset_handle(handle, window, cx);
                    }
                }),
            )
            .child(line);
        div()
            .relative()
            .flex_shrink_0()
            .map(|this| {
                if vertical {
                    this.w(px(1.)).h_full()
                } else {
                    this.h(px(1.)).w_full()
                }
            })
            .bg(t.border_subtle)
            .child(hit)
            .into_any_element()
    }

    /// A drag's chip, preview and insertion line, over everything.
    fn drag_layer(&mut self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let locale = settings::locale(cx);
        let bench = self.bench()?;
        let drag = bench.ui.drag?;
        let (ox, oy) = {
            let g = bench.ui.geometry.borrow();
            (g.body.x, g.body.y)
        };
        let (glyph, name) = match drag.dragged.tab {
            Tab::View(v) => (view_icon(v), text(locale, view_name(v))),
            Tab::Page(id) => {
                let route = self.route_of(id)?;
                let (glyph, _, title) = self.tab_label(route, self.is_current(id), locale, cx);
                (glyph, title)
            }
        };
        let preview = drag.zone.map(|z| {
            let r = z.preview;
            div()
                .absolute()
                .left(px(r.x - ox + 4.))
                .top(px(r.y - oy + 4.))
                .w(px((r.w - 8.).max(0.)))
                .h(px((r.h - 8.).max(0.)))
                .rounded(px(RADIUS_MD))
                .border_2()
                .border_color(t.accent)
                .bg(t.fill_hover)
        });
        let line = drag.zone.and_then(|z| z.line).map(|r| {
            div()
                .absolute()
                .left(px(r.x - ox))
                .top(px(r.y - oy))
                .w(px(r.w))
                .h(px(r.h))
                .bg(t.accent)
        });
        let chip = type_scale::SMALL
            .el()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .h(px(24.))
            .px(px(8.))
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_SM))
            .shadow(vec![t.shadow_overlay()])
            .text_color(t.text_primary)
            .whitespace_nowrap()
            .child(icon(glyph, 14., t.text_primary))
            .child(name);
        // 12 right and 12 below the pointer, kept inside the window.
        let chip = anchored()
            .position(point(px(drag.x + 12.), px(drag.y + 12.)))
            .snap_to_window()
            .child(chip);
        Some(
            deferred(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .children(preview)
                    .children(line)
                    .child(chip),
            )
            .with_priority(4)
            .into_any_element(),
        )
    }

    /// While a press or drag is on, the pointer is followed everywhere in
    /// the window, the closed hand (or not-allowed) cursor shown.
    fn pointer_listener(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let active = bench.ui.press.is_some() || bench.ui.drag.is_some();
        let cursor = bench.ui.drag.map(|d| {
            if d.zone.is_some() {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OperationNotAllowed
            }
        });
        let weak = cx.entity().downgrade();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                if let Some(cursor) = cursor {
                    window.set_window_cursor_style(cursor);
                }
                if !active {
                    return;
                }
                let moved = weak.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        let _ = moved.update(cx, |shell, cx| shell.pointer_moved(e, window, cx));
                    }
                });
                let released = weak.clone();
                window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && e.button == MouseButton::Left {
                        let _ = released.update(cx, |shell, cx| shell.pointer_released(window, cx));
                    }
                });
            },
        )
        .absolute()
        .size_0()
        .into_any_element()
    }

    // ---- menus ----

    fn menu_element(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let (can_right, can_down) = (
            self.can_split_page(Direction::Row, window),
            self.can_split_page(Direction::Column, window),
        );
        // Inbox and Knowledge while pages: checked when the focused group
        // shows that page, as their toggles are selected (4.8.5).
        let page_shown: Vec<ViewId> = views()
            .filter(|v| v.is_page_kind() && self.view_selected(*v, window))
            .collect();
        let bench = self.bench()?;
        let menu = bench.ui.menu?;
        let layout = bench.layout.clone();
        let can_undo = !bench.layout_undo.is_empty();
        let mut panel = menu::panel("layout-menu", &t)
            .min_w(px(240.))
            .on_mouse_down_out(cx.listener(|shell, _, _, cx| shell.close_menu(cx)));
        match menu.kind {
            LayoutMenu::Layout => {
                for view in views() {
                    let visible = layout.is_visible(view) || page_shown.contains(&view);
                    panel = panel.child(
                        menu_row(
                            SharedString::from(format!("lm-{}", view.key())),
                            text(locale, view_name(view)),
                            visible,
                            Some(view_keys(view, macos, locale)),
                            true,
                            &t,
                        )
                        .on_click(cx.listener(
                            move |shell, _, window, cx| {
                                shell.close_menu(cx);
                                shell.toggle_from_menu(view, window, cx)
                            },
                        )),
                    );
                }
                panel = panel.child(menu::separator(&t));
                for side in [Side::Left, Side::Bottom, Side::Right] {
                    let shown = layout.side(side).shown;
                    panel = panel.child(
                        menu_row(
                            SharedString::from(format!("lm-{}", side.key())),
                            text(locale, area_name(Place::Side(side))),
                            shown,
                            Some(area_keys(side, macos, locale)),
                            true,
                            &t,
                        )
                        .on_click(cx.listener(
                            move |shell, _, window, cx| {
                                shell.close_menu(cx);
                                shell.toggle_area(side, window, cx)
                            },
                        )),
                    );
                }
                panel = panel.child(menu::separator(&t));
                panel = panel
                    .child(
                        menu_row(
                            "lm-split-right",
                            text(locale, Key::CmdSplitRight),
                            false,
                            Some(vec![KeyName::Mod.label(locale, macos), "\\"]),
                            can_right,
                            &t,
                        )
                        .when(can_right, |this| {
                            this.on_click(cx.listener(|shell, _, window, cx| {
                                shell.close_menu(cx);
                                shell.split_page(Direction::Row, window, cx)
                            }))
                        }),
                    )
                    .child(
                        menu_row(
                            "lm-split-down",
                            text(locale, Key::CmdSplitDown),
                            false,
                            None,
                            can_down,
                            &t,
                        )
                        .when(can_down, |this| {
                            this.on_click(cx.listener(|shell, _, window, cx| {
                                shell.close_menu(cx);
                                shell.split_page(Direction::Column, window, cx)
                            }))
                        }),
                    );
                let split = layout.page.groups.len() > 1;
                panel = panel.child(
                    menu_row(
                        "lm-join",
                        text(locale, Key::CmdJoinGroups),
                        false,
                        None,
                        split,
                        &t,
                    )
                    .when(split, |this| {
                        this.on_click(cx.listener(|shell, _, window, cx| {
                            shell.close_menu(cx);
                            shell.join_groups(window, cx)
                        }))
                    }),
                );
                panel = panel.child(menu::separator(&t));
                panel = panel
                    .child(
                        menu_row(
                            "lm-undo",
                            text(locale, Key::CmdLayoutUndo),
                            false,
                            None,
                            can_undo,
                            &t,
                        )
                        .when(can_undo, |this| {
                            this.on_click(cx.listener(|shell, _, window, cx| {
                                shell.close_menu(cx);
                                shell.undo_layout(window, cx)
                            }))
                        }),
                    )
                    .child(
                        menu_row(
                            "lm-reset",
                            text(locale, Key::CmdLayoutReset),
                            false,
                            None,
                            true,
                            &t,
                        )
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.close_menu(cx);
                            shell.reset_layout(window, cx)
                        })),
                    );
            }
            LayoutMenu::View(view) => {
                // The Projects view's own item, above the view menu and a
                // separator (layout.md 4.2.1, as the chat's, 4.8.4).
                if view == ViewId::Projects {
                    panel = panel
                        .child(
                            menu_row(
                                "vm-collapse-all",
                                text(locale, Key::TreeCollapseAll),
                                false,
                                None,
                                true,
                                &t,
                            )
                            .on_click(cx.listener(
                                |shell, _, _, cx| {
                                    shell.close_menu(cx);
                                    shell.collapse_all(cx)
                                },
                            )),
                        )
                        .child(menu::separator(&t));
                }
                let place = layout.locate(Tab::View(view)).map(|s| s.place);
                panel = move_items(panel, place, view, None, &t, locale, cx);
                let can_split = layout.can_split_off(view);
                panel = panel
                    .child(
                        menu_row(
                            "vm-split",
                            text(locale, Key::ViewSplitOff),
                            false,
                            None,
                            can_split,
                            &t,
                        )
                        .when(can_split, |this| {
                            this.on_click(cx.listener(move |shell, _, window, cx| {
                                shell.close_menu(cx);
                                shell.view_action(view, ViewAction::SplitOff, window, cx)
                            }))
                        }),
                    )
                    .child(menu::separator(&t))
                    .child(
                        menu_row(
                            "vm-hide",
                            text(locale, Key::ViewHide),
                            false,
                            None,
                            true,
                            &t,
                        )
                        .on_click(cx.listener(
                            move |shell, _, window, cx| {
                                shell.close_menu(cx);
                                shell.view_action(view, ViewAction::Hide, window, cx)
                            },
                        )),
                    );
            }
            LayoutMenu::PageTab(tab, view) => {
                panel = move_items(panel, Some(Place::Page), view, Some(tab), &t, locale, cx);
            }
            LayoutMenu::Folded => {
                for (gi, g) in layout.page.groups.iter().enumerate() {
                    panel = panel.child(
                        type_scale::LABEL
                            .el()
                            .px(px(8.))
                            .pt(px(8.))
                            .pb(px(4.))
                            .text_color(t.text_tertiary)
                            .child(tr(
                                locale,
                                Key::LayoutGroup,
                                &[("n", Arg::Count(gi as u64 + 1))],
                            )),
                    );
                    for (i, tab) in g.tabs.iter().enumerate() {
                        let label = match *tab {
                            Tab::View(v) => text(locale, view_name(v)),
                            Tab::Page(id) => match self.route_of(id) {
                                Some(route) => {
                                    self.tab_label(route, self.is_current(id), locale, cx).2
                                }
                                None => continue,
                            },
                        };
                        let current = gi == layout.focused && i == g.active;
                        panel = panel.child(
                            menu_row(
                                SharedString::from(format!("fold-{gi}-{i}")),
                                label,
                                current,
                                None,
                                true,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |shell, _, window, cx| {
                                    shell.close_menu(cx);
                                    shell.activate_tab(Place::Page, gi, i, window, cx)
                                },
                            )),
                        );
                    }
                }
            }
            LayoutMenu::ViewTabs(place, gi) => {
                if let Some(g) = layout.area(place).groups.get(gi) {
                    for (i, tab) in g.tabs.iter().enumerate() {
                        let Tab::View(v) = *tab else { continue };
                        panel = panel.child(
                            menu_row(
                                SharedString::from(format!("view-tabs-{i}")),
                                text(locale, view_name(v)),
                                i == g.active,
                                None,
                                true,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |shell, _, window, cx| {
                                    shell.close_menu(cx);
                                    shell.activate_tab(place, gi, i, window, cx)
                                },
                            )),
                        );
                    }
                }
            }
            LayoutMenu::FoldedSide(side) => {
                for (gi, g) in layout.side(side).groups.iter().enumerate() {
                    for (i, tab) in g.tabs.iter().enumerate() {
                        let Tab::View(v) = *tab else { continue };
                        let current = gi == 0 && i == g.active;
                        panel = panel.child(
                            menu_row(
                                SharedString::from(format!("fold-{}-{gi}-{i}", side.key())),
                                text(locale, view_name(v)),
                                current,
                                None,
                                true,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |shell, _, window, cx| {
                                    shell.close_menu(cx);
                                    // The folded group's view shows in the
                                    // first group's place.
                                    shell.change_layout(None, window, cx, |l| {
                                        if gi == 0 {
                                            l.activate(Spot {
                                                place: Place::Side(side),
                                                group: 0,
                                                index: i,
                                            });
                                            true
                                        } else {
                                            l.move_tab(
                                                Tab::View(v),
                                                Target::Into {
                                                    place: Place::Side(side),
                                                    group: 0,
                                                    index: None,
                                                },
                                            )
                                            .is_ok()
                                        }
                                    });
                                    shell.focus_view(v, window, cx);
                                },
                            )),
                        );
                    }
                }
            }
        }
        Some(
            deferred(
                anchored()
                    .position(point(px(menu.x), px(menu.y)))
                    .snap_to_window()
                    .child(panel),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }

    /// A view chosen in the Layout menu: shown and focused, or hidden.
    fn toggle_from_menu(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.bench().is_some_and(|b| b.layout.is_visible(view));
        if visible {
            if self.change_layout(None, window, cx, |l| l.hide_view(view)) {
                self.focus_page(window, cx);
            }
        } else {
            self.view_action(view, ViewAction::Show, window, cx);
        }
    }

    /// A move from a view menu or a page tab's menu.
    fn menu_move(
        &mut self,
        view: ViewId,
        page: Option<TabId>,
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match page {
            Some(tab) => {
                let target = match place {
                    Place::Side(side) => Target::Area(side),
                    Place::Page => return,
                };
                let dragged = Dragged {
                    tab: Tab::Page(tab),
                    as_view: Some(view),
                };
                self.move_by_command(dragged, target, window, cx);
            }
            None => self.view_action(view, ViewAction::MoveTo(place), window, cx),
        }
    }

    /// The view page (Inbox, Knowledge as views) asked for something.
    fn on_view_page(
        &mut self,
        event: &crate::record_page::PageEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::record_page::PageEvent;
        match event {
            PageEvent::Open(route) => self.navigate(*route, true, window, cx),
            PageEvent::NewKb(draft) => self.new_kb_draft(*draft, window, cx),
            PageEvent::ShowTerminal(project) => self.show_session_terminal(*project, window, cx),
            PageEvent::Pick(picker, on_pick) => {
                self.open_picker(picker.clone(), on_pick.clone(), window, cx)
            }
            PageEvent::Triage(project) => self.file_with_claude(*project, window, cx),
            PageEvent::Confirm(confirm) => {
                self.open_dialog(crate::dialogs::Kind::Confirm(confirm.clone()), window, cx)
            }
            PageEvent::StaleMemories(project) => self.open_stale_memories(*project, window, cx),
            PageEvent::Saved(_) | PageEvent::Located(_) => cx.notify(),
        }
    }
}

/// The value of a resize drag.
pub(super) struct ResizeDrag(pub Handle);

/// Route the GPUI drag of a resize handle to the shell.
pub(super) fn on_resize_drag(
    shell: &mut Shell,
    handle: Handle,
    x: f32,
    y: f32,
    cx: &mut Context<Shell>,
) {
    shell.resize_moved(handle, x, y, cx);
}

/// The four Move items of a view menu (4.8.4): the current area's item
/// disabled with a trailing check.
fn move_items(
    mut panel: gpui_kit::Stateful<gpui_kit::Div>,
    current: Option<Place>,
    view: ViewId,
    page: Option<TabId>,
    t: &Tokens,
    locale: crate::i18n::Locale,
    cx: &mut Context<Shell>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let items = [
        (Place::Side(Side::Left), Key::ViewMoveLeft),
        (Place::Side(Side::Right), Key::ViewMoveRight),
        (Place::Side(Side::Bottom), Key::ViewMoveBottom),
        (Place::Page, Key::ViewMovePage),
    ];
    for (place, key) in items {
        let here = current == Some(place);
        panel = panel.child(
            menu_row(
                SharedString::from(format!("vm-{}", place.key())),
                text(locale, key),
                here,
                None,
                !here,
                t,
            )
            .when(!here, |this| {
                this.on_click(cx.listener(move |shell, _, window, cx| {
                    shell.close_menu(cx);
                    shell.menu_move(view, page, place, window, cx)
                }))
            }),
        );
    }
    panel
}

/// A menu row with an optional check and kbd hint; disabled rows are
/// `text.disabled` and take no pointer.
pub(super) fn menu_row(
    id: impl Into<SharedString>,
    label: String,
    checked: bool,
    keys: Option<Vec<&'static str>>,
    enabled: bool,
    t: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let trailing = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .children(keys.map(|k| ui::kbd(&k, t)))
        .when(checked, |this| this.child(menu::check(t)));
    if enabled {
        menu::item(
            id.into(),
            None,
            label,
            Some(trailing.into_any_element()),
            false,
            t,
        )
    } else {
        menu::disabled_item(id.into(), label, Some(trailing.into_any_element()), t)
    }
}

/// The body's size: the window between the titlebar and the status bar.
pub(super) fn body_size(window: &Window) -> (f32, f32) {
    let viewport = window.viewport_size();
    (
        f32::from(viewport.width),
        f32::from(viewport.height) - crate::titlebar::HEIGHT - super::STATUS_BAR_HEIGHT,
    )
}

/// The entity of a page or view, for the type checker in closures.
#[allow(dead_code)]
fn _entity_of<T: 'static>(_: &Entity<T>) {}

#[cfg(test)]
mod overflow_tests {
    use super::*;

    #[test]
    fn view_tabs_that_do_not_fit_give_way_to_the_chevron_and_the_active_stays() {
        let widths = [70., 70., 70., 70.];
        // All fit: no chevron.
        assert_eq!(fitting_tabs(&widths, 0, 400.), None);
        // Room for two besides the chevron: the first ones, in order.
        assert_eq!(fitting_tabs(&widths, 0, 200.), Some(vec![0, 1]));
        // The active tab is always shown, whatever its place.
        assert_eq!(fitting_tabs(&widths, 3, 200.), Some(vec![0, 3]));
        // Hardly any room: the active one alone.
        assert_eq!(fitting_tabs(&widths, 2, 60.), Some(vec![2]));
    }

    #[test]
    fn a_view_or_tab_drag_starts_after_4_px() {
        // 4.8.7: GPUI's own drag starts at 2; view and tab drags use
        // their own press and move tracking.
        assert_eq!(DRAG_START, 4.);
    }
}
