//! The Projects view (docs/design/layout.md 4.2.1, components.md 5 Tree
//! row): the tree's rows, drawn only near the viewport, the sticky project
//! row, the reveal of the open page's row, and the pinned Trash and
//! Settings rows. What the rows are and where they sit is `tree.rs`.

use std::rc::Rc;

use acta_core::model::{EpicId, Owner, ProjectId, Status};
use gpui_kit::component::input::{Escape, Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, Hsla, InteractiveElement, IntoElement, ParentElement,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div, point, px,
};

use super::{
    DismissSidebar, Shell, TreeDelete, TreeDown, TreeEnd, TreeHome, TreeKey, TreeLeft,
    TreeMoveDown, TreeMoveUp, TreeOpen, TreeOpenTab, TreeRight, TreeUp, busy_dot, tree_menu,
};
use crate::i18n::{Arg, Key, Locale, short_count, text, tr};
use crate::icons::{Icon, icon};
use crate::palette::CreateKind;
use crate::route::Route;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::tree::{self, Due, Layout, Row, RowKey, Signal};
use crate::type_scale::{self, TypeRole};
use crate::ui;

/// The tree's state beside its folds and cursor.
#[derive(Default)]
pub(super) struct TreeState {
    scroll: ScrollHandle,
    /// The rows and their layout for the model revision and the folds
    /// they were built from: built once per change, not per frame.
    cache: Option<Cache>,
    /// The row to scroll into view on the next frame, the least amount;
    /// `true` scrolls even while the pointer is over the tree (a keyboard
    /// target).
    reveal: Option<(RowKey, bool)>,
    /// The row to put at the viewport's top on the next frame (the sticky
    /// row's chevron folded its project).
    to_top: Option<RowKey>,
    hovered: bool,
}

impl TreeState {
    pub(super) fn clear_hover(&mut self) {
        self.hovered = false;
    }
}

struct Cache {
    revision: u64,
    folds: tree::Folds,
    rows: Rc<Vec<Row>>,
    layout: Rc<Layout>,
}

/// Where the open page is: the active project and the open page's epic,
/// whose rows are `text.primary` (4.2.1, "Where am I?").
#[derive(Clone, Copy, Default)]
struct Path {
    project: Option<ProjectId>,
    epic: Option<EpicId>,
}

/// How a row is drawn beyond what it is.
struct Look {
    selected: bool,
    on_path: bool,
    unread: bool,
    cursor: bool,
    /// The copy of the project row stuck at the viewport's top.
    sticky: bool,
    /// The view's width, for the title tooltip (shown when truncated).
    width: f32,
}

impl Shell {
    /// The visible rows (built once per model or fold change).
    pub(super) fn rows(&mut self, cx: &mut Context<Self>) -> Rc<Vec<Row>> {
        self.tree_rows(cx).0
    }

    fn tree_rows(&mut self, cx: &mut Context<Self>) -> (Rc<Vec<Row>>, Rc<Layout>) {
        let Some(bench) = self.bench() else {
            return Default::default();
        };
        let ws = bench.ws.read(cx);
        let state = &mut bench.tree;
        if let Some(cache) = &state.cache
            && cache.revision == ws.revision
            && cache.folds == bench.folds
        {
            return (cache.rows.clone(), cache.layout.clone());
        }
        let rows = Rc::new(tree::rows(&ws.projects, &bench.folds));
        let layout = Rc::new(Layout::of(&rows));
        // interaction.md 6.5, rule 2: rows that come and go above the
        // first row in view do not move the rows on screen.
        if let Some(old) = &state.cache {
            let scroll_top = -f32::from(state.scroll.offset().y);
            if let Some(top) = old
                .layout
                .anchor(&old.rows, scroll_top)
                .and_then(|anchor| layout.anchored(&rows, anchor))
                && top != scroll_top
            {
                state.scroll.set_offset(point(px(0.), px(-top)));
            }
        }
        state.cache = Some(Cache {
            revision: ws.revision,
            folds: bench.folds.clone(),
            rows: rows.clone(),
            layout: layout.clone(),
        });
        (rows, layout)
    }

    pub(super) fn toggle_fold(&mut self, key: String, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench.folds.toggle(key);
        }
        self.save_layout(cx);
        cx.notify();
    }

    /// "Collapse all" (4.2.1): every project and epic folds, the "N
    /// closed" folds stay.
    pub(super) fn collapse_all(&mut self, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            let projects = bench.ws.read(cx).projects.clone();
            bench.folds.collapse_all(&projects);
        }
        self.save_layout(cx);
        cx.notify();
    }

    /// Reveal the open page's row (4.2.1): unfold its project, its epic
    /// and its "N closed" row, and scroll it into view on the next frame,
    /// unless the pointer is over the tree and `force` is off.
    pub(super) fn reveal_in_tree(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(route) = self.tree_route(cx) else {
            return;
        };
        let Some(bench) = self.bench() else { return };
        let projects = bench.ws.read(cx).projects.clone();
        bench.folds.reveal(&projects, route);
        bench.tree.reveal = Some((RowKey::Route(route), force));
    }

    /// Scroll the keyboard cursor's row into view on the next frame.
    pub(super) fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        if let Some(bench) = self.bench()
            && let Some(row) = rows.get(bench.cursor.index(&rows))
        {
            bench.tree.reveal = Some((row.key(), true));
        }
    }

    /// "New topic" from a tree row's `plus` or ghost row: the palette in
    /// create mode, in that project (and that epic).
    pub(super) fn new_topic_in(
        &mut self,
        project: ProjectId,
        epic: Option<EpicId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bench) = self.bench() {
            bench.create_in = Some((project, epic));
        }
        self.open_palette(None, Some(CreateKind::Topic), window, cx);
    }

    /// Apply the scroll the tree owes (reveal, sticky fold) and return the
    /// scroll top to draw at.
    fn settle_scroll(&mut self, rows: &[Row], layout: &Layout, viewport: f32) -> f32 {
        let Some(bench) = self.bench() else { return 0. };
        let state = &mut bench.tree;
        let mut scroll_top = -f32::from(state.scroll.offset().y);
        // Before the first layout the viewport is unknown: wait for it.
        if viewport <= 0. {
            return scroll_top;
        }
        let find = |key: RowKey| rows.iter().position(|r| r.key() == key);
        let mut target = None;
        if let Some(key) = state.to_top.take()
            && let Some(i) = find(key)
        {
            target = Some(layout.top(i));
        }
        if let Some((key, force)) = state.reveal.take()
            && (force || !state.hovered)
            && let Some(i) = find(key)
        {
            target = layout.reveal(rows, i, target.unwrap_or(scroll_top), viewport);
        }
        if let Some(top) = target {
            let max = (layout.height - viewport).max(0.);
            scroll_top = top.clamp(0., max);
            state.scroll.set_offset(point(px(0.), px(-scroll_top)));
        }
        scroll_top
    }

    /// The Projects view (4.8.3): the tree, with Trash and Settings pinned
    /// at its bottom; it fills its group.
    pub(super) fn projects_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tokens = cx.global::<Tokens>().clone();
        let t = &tokens;
        let locale = settings::locale(cx);
        let (rows, layout) = self.tree_rows(cx);
        let current = self.tree_route(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let handle = bench.tree.scroll.clone();
        let bounds = handle.bounds();
        let viewport = f32::from(bounds.size.height);
        let width = f32::from(bounds.size.width);
        // Before the first layout the viewport is unknown; a pending
        // reveal or jump asks for the frame that knows it.
        if viewport <= 0. && (bench.tree.reveal.is_some() || bench.tree.to_top.is_some()) {
            window.request_animation_frame();
        }
        let scroll_top = self.settle_scroll(&rows, &layout, viewport);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let cursor = bench.cursor.index(&rows);
        let tree_focus = bench.tree_focus.clone();
        // components.md 5: the keyboard cursor is a focus ring, which
        // pointer input never draws (interaction.md 6.2).
        let ring = tree_focus.is_focused(window) && window.last_input_was_keyboard();
        let menu = bench.tree_menu;
        let tree_due = bench
            .tree_due
            .as_ref()
            .map(|d| (d.row, d.anchor.clone(), d.picker.clone()));
        let renaming = bench.rename.as_ref().map(|r| (r.key, r.state.clone()));
        let active_project = bench.active_project;
        let menu_items = self.menu_items(&rows, cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let drawn = layout.drawn(scroll_top, viewport.max(560.));
        // A rename whose row scrolls or folds out of the drawn rows is
        // committed, not lost.
        if let Some((key, _)) = &renaming
            && !rows[drawn.clone()].iter().any(|r| r.key() == *key)
        {
            cx.defer_in(window, |shell, window, cx| shell.commit_rename(window, cx));
        }
        let sticky = layout.sticky(&rows, scroll_top);
        let ws = bench.ws.read(cx);
        let path = Path {
            project: active_project,
            epic: match current {
                Some(Route::Epic(id)) => Some(id),
                Some(Route::Topic(id)) => {
                    tree::locate_topic(&ws.projects, id).and_then(|l| l.epic.map(|e| e.0))
                }
                _ => None,
            },
        };
        let unread: Vec<bool> = rows[drawn.clone()]
            .iter()
            .map(|row| tree::row_unread(row, &ws.projects, &ws.unread))
            .collect();
        let sticky_unread =
            sticky.is_some_and(|(p, _)| tree::row_unread(&rows[p], &ws.projects, &ws.unread));
        let look = |row: &Row, i: usize, unread: bool, sticky: bool| Look {
            selected: row.route().is_some() && row.route() == current,
            on_path: match row {
                Row::Project { id, .. } => path.project == Some(*id),
                Row::Epic { id, .. } => path.epic == Some(*id),
                _ => false,
            },
            unread,
            cursor: !sticky && ring && i == cursor,
            sticky,
            width,
        };
        let mut items: Vec<AnyElement> = Vec::with_capacity(drawn.len() + 2);
        if !drawn.is_empty() {
            items.push(div().h(px(layout.top(drawn.start))).into_any_element());
        }
        for (n, i) in drawn.clone().enumerate() {
            let row = &rows[i];
            let key = row.key();
            let menu = menu.filter(|m| m.row == key).map(|m| {
                let status = match row {
                    Row::Topic { status, .. } => Some(*status),
                    _ => None,
                };
                tree_menu(&menu_items, m, status, locale, t, cx)
            });
            let menu = menu.or_else(|| {
                let (_, anchor, picker) = tree_due.as_ref().filter(|(row, _, _)| *row == key)?;
                // Anchored to the row's title (components-records.md 5).
                Some(
                    div()
                        .ml(px(24.))
                        .w(px(1.))
                        .h(px(tree::ROW))
                        .relative()
                        .child(anchor.measure())
                        .children(anchor.below(picker.clone().into_any_element()))
                        .into_any_element(),
                )
            });
            let edit = renaming
                .as_ref()
                .filter(|(k, _)| *k == key)
                .map(|(_, state)| state.clone());
            let mut look = look(row, i, unread[n], false);
            look.cursor &= edit.is_none();
            let gap = if n == 0 {
                0.
            } else {
                layout.top(i) - layout.top(i - 1) - tree::ROW
            };
            items.push(
                div()
                    .pt(px(gap))
                    .child(tree_row(row, &look, menu, edit, locale, t, cx))
                    .into_any_element(),
            );
        }
        if let Some(last) = drawn.clone().last() {
            let rest = layout.height - layout.top(last) - tree::ROW;
            items.push(div().h(px(rest)).into_any_element());
        }
        if rows.is_empty() {
            items.push(empty_tree(locale, t, cx));
        }
        let sticky = sticky.map(|(p, y)| {
            let row = &rows[p];
            div()
                .absolute()
                .top(px(y))
                .left_0()
                .right_0()
                .px(px(tree::PAD))
                .bg(t.bg_sunken)
                .border_b_1()
                .border_color(t.border_subtle)
                .child(tree_row(
                    row,
                    &look(row, p, sticky_unread, true),
                    None,
                    None,
                    locale,
                    t,
                    cx,
                ))
        });
        let on_trash = current == Some(Route::Trash);
        let on_settings = current == Some(Route::Settings);
        let pinned = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .p(px(8.))
            .border_t_1()
            .border_color(t.border_subtle)
            .child(
                sidebar_row(
                    "sidebar-trash",
                    Icon::Trash2,
                    text(locale, Key::TreeTrash),
                    on_trash,
                    t,
                )
                .on_click(cx.listener(|shell, _, window, cx| {
                    shell.navigate(Route::Trash, true, window, cx)
                })),
            )
            .child(
                sidebar_row(
                    "sidebar-settings",
                    Icon::Settings,
                    text(locale, Key::TreeSettings),
                    on_settings,
                    t,
                )
                .on_click(cx.listener(|shell, _, window, cx| shell.open_settings(window, cx))),
            );
        let tree = div()
            .id("tree")
            .key_context("Tree")
            .track_focus(&tree_focus)
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&handle)
            .px(px(tree::PAD))
            .on_action(cx.listener(|s, _: &TreeUp, w, cx| s.tree_key(TreeKey::Up, w, cx)))
            .on_action(cx.listener(|s, _: &TreeDown, w, cx| s.tree_key(TreeKey::Down, w, cx)))
            .on_action(cx.listener(|s, _: &TreeLeft, w, cx| s.tree_key(TreeKey::Left, w, cx)))
            .on_action(cx.listener(|s, _: &TreeRight, w, cx| s.tree_key(TreeKey::Right, w, cx)))
            .on_action(cx.listener(|s, _: &TreeOpen, w, cx| s.tree_key(TreeKey::Open, w, cx)))
            .on_action(cx.listener(|s, _: &TreeDelete, w, cx| s.tree_key(TreeKey::Delete, w, cx)))
            .on_action(cx.listener(|s, _: &TreeMoveUp, w, cx| s.tree_key(TreeKey::MoveUp, w, cx)))
            .on_action(
                cx.listener(|s, _: &TreeMoveDown, w, cx| s.tree_key(TreeKey::MoveDown, w, cx)),
            )
            .on_action(cx.listener(|s, _: &TreeHome, w, cx| s.tree_key(TreeKey::Home, w, cx)))
            .on_action(cx.listener(|s, _: &TreeEnd, w, cx| s.tree_key(TreeKey::End, w, cx)))
            .on_action(cx.listener(|s, _: &DismissSidebar, w, cx| {
                if !s.close_tree_menu(cx) {
                    s.dismiss_sidebar(w, cx)
                }
            }))
            .on_action(cx.listener(|s, _: &TreeOpenTab, w, cx| {
                let rows = s.rows(cx);
                let route = s
                    .bench()
                    .and_then(|b| rows.get(b.cursor.index(&rows)))
                    .and_then(Row::route);
                if let Some(route) = route {
                    s.open_in_tab(route, w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &Escape, w, cx| s.cancel_rename(w, cx)))
            .on_key_down(cx.listener(Self::tree_key_down))
            .children(items);
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg_sunken)
            .child(
                div()
                    .id("tree-viewport")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .on_hover(cx.listener(|shell, hovered: &bool, _, _| {
                        if let Some(bench) = shell.bench() {
                            bench.tree.hovered = *hovered;
                        }
                    }))
                    .child(tree)
                    .children(sticky),
            )
            .child(pinned)
            .into_any_element()
    }
}

/// A row's key as an element id: the route key ("T-12"), the group key of a
/// "N closed" row, or the epic of a "New topic" row.
fn key_slug(key: RowKey) -> String {
    match key {
        RowKey::Route(route) => route.key(),
        RowKey::Closed(group) => group.key(),
        RowKey::NewTopic(epic) => format!("new:{epic}"),
    }
}

/// The empty tree (4.2.1, Empty states): "No projects yet." and the
/// "New project..." button.
fn empty_tree(locale: Locale, t: &Tokens, cx: &mut Context<Shell>) -> AnyElement {
    div()
        .p(px(8.))
        .mt(px(tree::PAD))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(8.))
        .child(
            type_scale::UI
                .el()
                .text_color(t.text_secondary)
                .child(text(locale, Key::TreeEmpty)),
        )
        .child(
            ui::button(
                "tree-new-project",
                text(locale, Key::CmdNewProject),
                ui::Variant::Secondary,
                false,
                t,
            )
            .on_click(cx.listener(|shell, _, window, cx| shell.new_project(window, cx))),
        )
        .into_any_element()
}

/// A status glyph's colour in the tree (4.2.1, Status in the tree): only
/// Active and Blocked are coloured; closed topics are tertiary, not the
/// green of their Status property.
fn tree_status_color(status: Status, t: &Tokens) -> Hsla {
    match status {
        Status::Active => t.accent,
        Status::Blocked => t.warning,
        Status::Planned => t.text_secondary,
        Status::Draft | Status::Done | Status::Dropped => t.text_tertiary,
    }
}

/// The width of `label` in `role`, from the font's own advances.
fn text_width(role: TypeRole, label: &str, cx: &App) -> f32 {
    let system = cx.text_system();
    let mut font = gpui_kit::font(role.family());
    font.weight = role.weight;
    let font = system.resolve_font(&font);
    let size = px(role.size());
    label
        .chars()
        .map(|ch| {
            system
                .advance(font, size, ch)
                .map(|a| f32::from(a.width))
                .unwrap_or(role.size() * 0.6)
        })
        .sum()
}

/// The live Claude Code session on a row's record (components-app.md 5,
/// Claude Code sessions): the glyph's colour and its tooltip, "Claude Code
/// is working here · since 14:05".
fn session_here(row: &Row, locale: Locale, t: &Tokens, cx: &App) -> Option<(Hsla, String)> {
    let owner = match row {
        Row::Project { id, .. } => Owner::Project(*id),
        Row::Epic { id, .. } => Owner::Epic(*id),
        Row::Topic { id, .. } => Owner::Topic(*id),
        _ => return None,
    };
    let live = crate::sessions::live(cx);
    let here = crate::sessions::here(live, owner)?;
    let mut tip = crate::sessions::here_text(locale, here);
    if let Some(first) = live
        .iter()
        .filter(|s| crate::sessions::place(s) == owner)
        .min_by_key(|s| s.started_at)
    {
        tip.push_str(" \u{b7} ");
        tip.push_str(&crate::sessions::since(locale, first.started_at));
    }
    Some((crate::sessions::state_color(here.state, t), tip))
}

/// One row of the tree (components.md 5, Tree row): indent, leading slot
/// 16, 8, title, 8, trailing slot with one signal at most.
#[allow(clippy::too_many_arguments)]
fn tree_row(
    row: &Row,
    look: &Look,
    menu: Option<AnyElement>,
    edit: Option<Entity<InputState>>,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let selected = look.selected;
    let prefix = if look.sticky {
        "tree-sticky"
    } else {
        "tree-row"
    };
    // Element ids follow the row's record, not its place, so hover and
    // tooltip state stay with the row when rows come and go above it.
    let row_key = row.key();
    let slug = key_slug(row_key);
    let id = SharedString::from(format!("{prefix}-{slug}"));
    let (hover, primary) = (t.fill_hover, t.text_primary);
    // components.md 5: nothing tertiary on `fill.selected`.
    let lift = |color: Hsla| {
        if selected && color == t.text_tertiary {
            t.text_secondary
        } else {
            color
        }
    };
    let rest = if selected || look.on_path {
        t.text_primary
    } else {
        t.text_secondary
    };
    let indent = row.depth() as f32 * 16.;
    let mut el = div()
        .id(id.clone())
        .group(id.clone())
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(tree::ROW))
        .pl(px(8. + indent))
        .pr(px(8.))
        .rounded(px(RADIUS_SM))
        .when(selected, |this| this.bg(t.fill_selected))
        .when(!selected, |this| this.hover(move |s| s.bg(hover)))
        .when(look.cursor, |this| this.shadow(ui::ring_shadow(t, false)));
    // A leading icon: `text.primary` on hover (status glyphs keep theirs).
    let lead = |glyph: Icon, size: f32, color: Hsla| {
        div()
            .size(px(16.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .child(icon(glyph, size, color).group_hover(id.clone(), move |s| s.text_color(primary)))
    };
    // Chevrons and icons: `text.secondary`, `text.primary` when selected.
    let icon_rest = if selected { primary } else { t.text_secondary };
    let chevron = |expanded: bool, size: f32, color: Hsla| {
        lead(
            if expanded {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            size,
            color,
        )
    };
    let fold = |expanded: bool, key: String, cx: &mut Context<Shell>| {
        let sticky = look.sticky.then(|| row.key());
        div()
            .id(SharedString::from(format!("{prefix}-fold-{slug}")))
            .flex_shrink_0()
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                // The sticky row's chevron folds its project and puts the
                // project row where the sticky row was.
                if let Some(key) = sticky
                    && let Some(bench) = shell.bench()
                {
                    bench.tree.to_top = Some(key);
                }
                shell.toggle_fold(key.clone(), cx);
            }))
            .child(chevron(expanded, 16., icon_rest))
    };
    // The title, or the inline edit while renaming (components.md 5). A
    // truncated title shows its full text in a tooltip.
    let title = |role: TypeRole, label: String, color: Hsla, trailing: f32, cx: &App| match &edit {
        Some(state) => role
            .el()
            .flex_1()
            .min_w_0()
            .text_color(primary)
            .child(Input::new(state).appearance(false))
            .into_any_element(),
        None => {
            let room = tree::title_width(look.width, row.depth(), trailing);
            let truncated = look.width > 0. && text_width(role, &label, cx) > room;
            role.el()
                .id(SharedString::from(format!("{prefix}-title-{slug}")))
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(color)
                .group_hover(id.clone(), move |s| s.text_color(primary))
                .when(truncated, |this| this.tooltip(ui::tip(label.clone())))
                .child(label)
                .into_any_element()
        }
    };
    // The one signal of the trailing slot (4.2.1), in a 16 box with its
    // tooltip.
    let session = session_here(row, locale, t, cx);
    let today = crate::due::today(cx);
    let signal = tree::signal(session.is_some(), look.unread, tree::row_due(row, today));
    // The sentence form of copy.md 10.2.1 ("Overdue by 3 days, due 18 Nov
    // 2026", "Due today").
    let due_tip = || {
        tree::row_due_date(row)
            .map(|day| {
                let state = acta_core::date::due_state(Some(day), today, true);
                crate::due::sentence(locale, day, state)
            })
            .unwrap_or_default()
    };
    let signal_el = signal.map(|signal| {
        let (glyph, tip): (AnyElement, String) = match (signal, &session) {
            (Signal::Session, Some((color, tip))) => {
                (icon(Icon::Bot, 14., *color).into_any_element(), tip.clone())
            }
            (Signal::Session, None) => (div().into_any_element(), String::new()),
            (Signal::Unread, _) => (
                busy_dot(t).into_any_element(),
                text(locale, Key::TreeUnread),
            ),
            (Signal::Due(Due::Overdue), _) => (
                icon(Icon::ClockAlert, 14., t.text_secondary)
                    .group_hover(id.clone(), move |s| s.text_color(primary))
                    .into_any_element(),
                due_tip(),
            ),
            (Signal::Due(Due::Today), _) => (
                icon(Icon::Clock, 14., lift(t.text_tertiary))
                    .group_hover(id.clone(), move |s| s.text_color(primary))
                    .into_any_element(),
                due_tip(),
            ),
        };
        div()
            .id(SharedString::from(format!("{prefix}-signal-{slug}")))
            .size(px(16.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .when(!tip.is_empty(), |this| this.tooltip(ui::tip(tip)))
            .child(glyph)
    });
    let has_signal = signal_el.is_some();
    // Project and epic rows always reserve 24: the `plus` takes the
    // signal's place on hover without moving the title (components.md 5).
    let parent_trailing = |project: ProjectId,
                           epic: Option<EpicId>,
                           signal: Option<gpui_kit::Stateful<gpui_kit::Div>>,
                           cx: &mut Context<Shell>| {
        div()
            .relative()
            .w(px(tree::PARENT_TRAILING))
            .h(px(16.))
            .flex_shrink_0()
            .flex()
            .justify_end()
            .children(signal.map(|el| el.group_hover(id.clone(), |s| s.invisible())))
            .child(
                div()
                    .absolute()
                    .top(px(-4.))
                    .right(px(-4.))
                    .invisible()
                    .group_hover(id.clone(), |s| s.visible())
                    .child(
                        ui::ghost_icon_button(
                            format!("{prefix}-plus-{slug}"),
                            Icon::Plus,
                            24.,
                            false,
                            false,
                            true,
                            t,
                        )
                        .tooltip(ui::tip(text(locale, Key::TopicNew)))
                        .on_click(cx.listener(
                            move |shell, _, window, cx| {
                                cx.stop_propagation();
                                shell.new_topic_in(project, epic, window, cx);
                            },
                        )),
                    ),
            )
    };
    el = match row {
        Row::Project {
            title: name,
            expanded,
            id: pid,
            ..
        } => el
            .child(fold(*expanded, pid.to_string(), cx))
            .child(title(
                type_scale::UI_STRONG,
                name.clone(),
                rest,
                tree::PARENT_TRAILING,
                cx,
            ))
            .child(parent_trailing(*pid, None, signal_el, cx)),
        Row::Epic {
            title: name,
            expanded,
            id: eid,
            project,
            ..
        } => el
            .child(fold(*expanded, eid.to_string(), cx))
            .child(title(
                type_scale::UI_STRONG,
                name.clone(),
                rest,
                tree::PARENT_TRAILING,
                cx,
            ))
            .child(parent_trailing(*project, Some(*eid), signal_el, cx)),
        Row::Inbox { count: n, .. } => {
            let count = short_count((*n).max(0) as u64);
            let reserve = if *n > 0 {
                text_width(type_scale::SMALL, &count, cx)
            } else {
                0.
            };
            el.child(lead(Icon::Inbox, 16., icon_rest))
                .child(title(
                    type_scale::UI,
                    text(locale, Key::TreeInbox),
                    rest,
                    reserve,
                    cx,
                ))
                .when(*n > 0, |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .flex_shrink_0()
                            .font_features(type_scale::tabular())
                            .text_color(lift(t.text_tertiary))
                            .child(count),
                    )
                })
        }
        Row::Knowledge { .. } => el
            .child(lead(Icon::Library, 16., icon_rest))
            .child(title(
                type_scale::UI,
                text(locale, Key::TreeKnowledge),
                rest,
                if has_signal { 16. } else { 0. },
                cx,
            ))
            .children(signal_el),
        Row::Topic {
            title: name,
            status,
            ..
        } => {
            let color = if status.is_open() {
                rest
            } else {
                lift(t.text_tertiary)
            };
            // The 14 glyph sits in a 16 slot, so topic titles line up
            // with the Inbox and Knowledge titles above them.
            el.child(
                div()
                    .size(px(16.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(
                        ui::status_icon(*status),
                        14.,
                        lift(tree_status_color(*status, t)),
                    )),
            )
            .child(title(
                type_scale::UI,
                name.clone(),
                color,
                if has_signal { 16. } else { 0. },
                cx,
            ))
            .children(signal_el)
        }
        Row::Closed {
            count: n,
            group,
            shown,
            ..
        } => {
            let key = group.key();
            el.on_click(cx.listener(move |shell, _, _, cx| shell.toggle_fold(key.clone(), cx)))
                .child(chevron(*shown, 14., t.text_tertiary))
                .child(
                    type_scale::SMALL
                        .el()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_tertiary)
                        .group_hover(id.clone(), move |s| s.text_color(primary))
                        .child(tr(locale, Key::TreeClosed, &[("n", Arg::Count(*n as u64))])),
                )
                .children(signal_el)
        }
        Row::NewTopic { project, epic } => {
            let (project, epic) = (*project, *epic);
            el.on_click(cx.listener(move |shell, _, window, cx| {
                shell.new_topic_in(project, Some(epic), window, cx)
            }))
            .child(lead(Icon::Plus, 14., t.text_tertiary))
            .child(
                type_scale::SMALL
                    .el()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text_tertiary)
                    .group_hover(id.clone(), move |s| s.text_color(primary))
                    .child(text(locale, Key::TopicNew)),
            )
        }
    };
    if let Some(route) = row.route() {
        // Mod+click or a middle click opens the row in a new tab (4.4).
        el = el
            .on_click(
                cx.listener(move |shell, event: &gpui_kit::ClickEvent, window, cx| {
                    let rows = shell.rows(cx);
                    if let Some(bench) = shell.bench() {
                        // The cursor goes to the real row (the sticky row
                        // is not a keyboard stop).
                        bench.cursor.put(&rows, route);
                        bench.tree_menu = None;
                    }
                    if event.modifiers().secondary(cx) {
                        shell.open_in_tab(route, window, cx)
                    } else {
                        shell.navigate(route, true, window, cx)
                    }
                }),
            )
            .on_mouse_down(
                gpui_kit::MouseButton::Middle,
                cx.listener(move |shell, _, window, cx| shell.open_in_tab(route, window, cx)),
            );
        if !look.sticky {
            // A right click opens the row's menu (components.md 5).
            el = el.on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(move |shell, _, window, cx| {
                    let rows = shell.rows(cx);
                    let Some(bench) = shell.bench() else { return };
                    bench.cursor.put(&rows, route);
                    let handle = bench.tree_focus.clone();
                    window.focus(&handle, cx);
                    shell.open_tree_menu(super::MenuKind::Row, &rows, cx);
                }),
            );
        }
    }
    el.children(menu.map(|menu| div().absolute().top_0().left(px(8. + indent)).child(menu)))
        .into_any_element()
}

/// A pinned row of the sidebar (components.md 5, Tree row): weight 400 in
/// every state.
fn sidebar_row(
    id: &'static str,
    glyph: Icon,
    label: String,
    selected: bool,
    tokens: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (hover, primary) = (tokens.fill_hover, tokens.text_primary);
    let rest = if selected {
        tokens.text_primary
    } else {
        tokens.text_secondary
    };
    div()
        .id(id)
        .group(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .when(selected, |this| this.bg(tokens.fill_selected))
        .when(!selected, |this| this.hover(move |style| style.bg(hover)))
        .child(icon(glyph, 16., rest).group_hover(id, move |style| style.text_color(primary)))
        .child(
            type_scale::UI
                .el()
                .min_w_0()
                .truncate()
                .text_color(rest)
                .group_hover(id, move |style| style.text_color(primary))
                .child(label),
        )
}
