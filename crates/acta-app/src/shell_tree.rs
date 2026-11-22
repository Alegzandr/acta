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
    /// A press on a project, epic or topic row, not yet a drag; and the
    /// drag (components.md 5, Tree row, Drag to reorder).
    press: Option<TreePress>,
    drag: Option<TreeDrag>,
    /// A drag just ended: the click its release makes opens nothing.
    swallow_click: bool,
}

/// A press that becomes a drag after 4 of movement.
#[derive(Clone, Copy, Debug)]
struct TreePress {
    dragged: tree::Dragged,
    key: RowKey,
    x: f32,
    y: f32,
}

/// A row being dragged, with the pointer in window coordinates; for a
/// topic, the epic it was in (a drag whose record another process moved
/// or deleted ends as cancelled).
#[derive(Clone, Copy, Debug)]
struct TreeDrag {
    dragged: tree::Dragged,
    key: RowKey,
    epic: Option<Option<EpicId>>,
    x: f32,
    y: f32,
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
    /// The row being dragged (60%), and the row a drop would land on.
    dragged: bool,
    target: bool,
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

    // ---- dragging rows (components.md 5, Tree row) ----

    fn tree_press(&mut self, row: &Row, event: &gpui_kit::MouseDownEvent, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.tree.swallow_click = false;
        let dragged = tree::Dragged::of(&bench.ws.read(cx).projects, row);
        bench.tree.press = dragged.map(|dragged| TreePress {
            dragged,
            key: row.key(),
            x: f32::from(event.position.x),
            y: f32::from(event.position.y),
        });
        // The next frame follows the pointer.
        cx.notify();
    }

    fn tree_pointer_moved(&mut self, event: &gpui_kit::MouseMoveEvent, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
        let Some(bench) = self.bench() else { return };
        if let Some(press) = bench.tree.press {
            if event.pressed_button != Some(gpui_kit::MouseButton::Left) {
                bench.tree.press = None;
                return;
            }
            if (x - press.x).hypot(y - press.y) < 4. {
                return;
            }
            bench.tree.press = None;
            bench.tree_menu = None;
            let epic = match press.dragged {
                tree::Dragged::Topic(_, id) => {
                    tree::locate_topic(&bench.ws.read(cx).projects, id).map(|l| l.epic.map(|e| e.0))
                }
                _ => None,
            };
            bench.tree.drag = Some(TreeDrag {
                dragged: press.dragged,
                key: press.key,
                epic,
                x,
                y,
            });
        }
        if let Some(drag) = bench.tree.drag.as_mut() {
            drag.x = x;
            drag.y = y;
            cx.notify();
        }
    }

    fn tree_pointer_released(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.tree.press = None;
        if bench.tree.drag.is_none() {
            return;
        }
        let drop = self.tree_drop(cx);
        let Some(bench) = self.bench() else { return };
        let Some(drag) = bench.tree.drag.take() else {
            return;
        };
        bench.tree.swallow_click = true;
        cx.notify();
        // A drop outside the tree, on no target or on the row's own place
        // changes nothing.
        let Some(drop) = drop else { return };
        let ws = bench.ws.clone();
        // One write, one undo step, no toast: the row is visibly where it
        // was dropped.
        ws.update(cx, |ws, cx| match drag.dragged {
            tree::Dragged::Topic(project, id) if drag.epic != Some(drop.epic) => {
                let _ = project;
                ws.place_topic(id, drop.epic, drop.index, cx)
            }
            tree::Dragged::Topic(project, _) | tree::Dragged::Epic(project, _) => {
                ws.reorder(drag.dragged.record(), drop.index, Some(project), cx)
            }
            tree::Dragged::Project(project) => {
                ws.reorder(drag.dragged.record(), drop.index, Some(project), cx)
            }
        });
    }

    /// `Esc` during a drag: nothing changes. Whether a drag was on.
    pub(super) fn cancel_tree_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(bench) = self.bench() else {
            return false;
        };
        bench.tree.press = None;
        let was = bench.tree.drag.take().is_some();
        if was {
            bench.tree.swallow_click = true;
            cx.notify();
        }
        was
    }

    /// Where the dragged row would land now, from the pointer's place in
    /// the tree's viewport; `None` outside the tree or on no target.
    fn tree_drop(&mut self, cx: &mut Context<Self>) -> Option<tree::Drop> {
        let (rows, layout) = self.tree_rows(cx);
        let bench = self.bench()?;
        let drag = bench.tree.drag?;
        let bounds = bench.tree.scroll.bounds();
        let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        if drag.x < left || drag.x > left + w || drag.y < top || drag.y > top + h {
            return None;
        }
        let scroll_top = -f32::from(bench.tree.scroll.offset().y);
        let projects = &bench.ws.read(cx).projects;
        tree::drop_at(
            projects,
            &rows,
            &layout,
            drag.dragged,
            drag.y - top + scroll_top,
        )
    }

    /// While a press or a drag is on, the pointer is followed everywhere in
    /// the window, the cursor showing whether a drop would land.
    fn tree_pointer_listener(&mut self, drop: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let bench = self.bench()?;
        let dragging = bench.tree.drag.is_some();
        if bench.tree.press.is_none() && !dragging {
            return None;
        }
        let weak = cx.entity().downgrade();
        Some(
            gpui_kit::canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    if dragging {
                        window.set_window_cursor_style(if drop {
                            gpui_kit::CursorStyle::ClosedHand
                        } else {
                            gpui_kit::CursorStyle::OperationNotAllowed
                        });
                    }
                    let moved = weak.clone();
                    window.on_mouse_event(move |e: &gpui_kit::MouseMoveEvent, phase, _, cx| {
                        if phase == gpui_kit::DispatchPhase::Capture {
                            let _ = moved.update(cx, |shell, cx| shell.tree_pointer_moved(e, cx));
                        }
                    });
                    let released = weak.clone();
                    window.on_mouse_event(move |e: &gpui_kit::MouseUpEvent, phase, _, cx| {
                        if phase == gpui_kit::DispatchPhase::Capture
                            && e.button == gpui_kit::MouseButton::Left
                        {
                            let _ =
                                released.update(cx, |shell, cx| shell.tree_pointer_released(cx));
                        }
                    });
                },
            )
            .absolute()
            .size_0()
            .into_any_element(),
        )
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
        // A drag whose record another process deleted or moved ends as
        // cancelled; near the viewport's edges the tree scrolls.
        if let Some(drag) = bench.tree.drag {
            let projects = &bench.ws.read(cx).projects;
            let still = match drag.dragged {
                tree::Dragged::Topic(project, id) => {
                    tree::locate_topic(projects, id).is_some_and(|l| {
                        l.project.map(|p| p.0) == Some(project)
                            && Some(l.epic.map(|e| e.0)) == drag.epic
                    })
                }
                tree::Dragged::Epic(project, id) => tree::locate_epic(projects, id)
                    .is_some_and(|l| l.project.map(|p| p.0) == Some(project)),
                tree::Dragged::Project(id) => projects.iter().any(|t| t.project.id == id),
            };
            if !still {
                bench.tree.drag = None;
            } else if viewport > 0. {
                let delta = tree::auto_scroll(drag.y - f32::from(bounds.top()), viewport);
                if delta != 0. {
                    let max = (layout.height - viewport).max(0.);
                    let top = (-f32::from(handle.offset().y) + delta).clamp(0., max);
                    handle.set_offset(point(px(0.), px(-top)));
                    window.request_animation_frame();
                }
            }
        }
        let scroll_top = self.settle_scroll(&rows, &layout, viewport);
        let drop = self.tree_drop(cx);
        let pointer = self.tree_pointer_listener(drop.is_some(), cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let drag = bench.tree.drag;
        let cursor = bench.cursor.index(&rows);
        let tree_focus = bench.tree_focus.clone();
        // components.md 5: the keyboard cursor is a focus ring, which
        // pointer input never draws (interaction.md 6.2).
        let ring = tree_focus.is_focused(window) && window.last_input_was_keyboard();
        let menu = bench.tree_menu;
        let menu_anchor = bench.tree_menu_anchor.clone();
        let tree_due = bench
            .tree_due
            .as_ref()
            .map(|d| (d.row, d.anchor.clone(), d.picker.clone()));
        let renaming = bench.rename.as_ref().map(|r| (r.key, r.state.clone()));
        let active_project = bench.active_project;
        let has_contacts = bench.ws.read(cx).contacts > 0;
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
            dragged: !sticky && drag.is_some_and(|d| d.key == row.key()),
            target: !sticky && drop.is_some_and(|d| d.show == tree::DropShow::Onto(i)),
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
                tree_menu(&menu_anchor, &menu_items, m, status, locale, t, cx)
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
                        .children(anchor.below_at(picker.clone(), crate::menu::Align::Start, 2))
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
        // The insertion line (2 `accent`, `radius.xs` ends, from the
        // target depth's leading slot to the rows' content edge) and the
        // drag chip of 4.8.7 following the pointer.
        let line = drop.and_then(|d| match d.show {
            tree::DropShow::Line { y, depth } => Some(
                div()
                    .absolute()
                    .top(px(y - scroll_top - 1.))
                    .left(px(tree::PAD + 8. + f32::from(depth) * 16.))
                    .right(px(tree::PAD + 8.))
                    .h(px(2.))
                    .rounded(px(crate::theme::RADIUS_XS))
                    .bg(t.accent),
            ),
            tree::DropShow::Onto(_) => None,
        });
        let chip = drag.and_then(|d| {
            let row = rows.iter().find(|r| r.key() == d.key)?;
            let glyph = match row {
                Row::Topic { status, .. } => ui::status_icon(*status),
                Row::Project { expanded, .. } | Row::Epic { expanded, .. } if *expanded => {
                    Icon::ChevronDown
                }
                _ => Icon::ChevronRight,
            };
            let name = row.title().unwrap_or_default().to_owned();
            let chip = type_scale::SMALL
                .el()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .h(px(24.))
                .max_w(px(240.))
                .px(px(8.))
                .bg(t.bg_overlay)
                .border_1()
                .border_color(t.border_default)
                .rounded(px(RADIUS_SM))
                .shadow(vec![t.shadow_overlay()])
                .text_color(t.text_primary)
                .whitespace_nowrap()
                .child(icon(glyph, 14., t.text_primary))
                .child(div().min_w_0().truncate().child(name));
            Some(
                gpui_kit::deferred(
                    gpui_kit::anchored()
                        .position(point(px(d.x + 12.), px(d.y + 12.)))
                        .snap_to_window()
                        .child(chip),
                )
                .with_priority(4),
            )
        });
        let on_trash = current == Some(Route::Trash);
        let on_settings = current == Some(Route::Settings);
        let on_contacts = matches!(current, Some(Route::Contacts) | Some(Route::Contact(_)));
        // The Contacts row exists only once a contact does
        // (components-contacts.md 5).
        let pinned = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .p(px(8.))
            .border_t_1()
            .border_color(t.border_subtle)
            .when(has_contacts, |this| {
                this.child(
                    sidebar_row(
                        "sidebar-contacts",
                        Icon::Users,
                        text(locale, Key::ContactsTitle),
                        on_contacts,
                        t,
                    )
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.navigate(Route::Contacts, true, window, cx)
                    })),
                )
            })
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
                    .children(sticky)
                    .children(line)
                    .children(chip)
                    .children(pointer),
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
    let rest = if selected || look.on_path || look.target {
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
        .when(selected && !look.target, |this| this.bg(t.fill_selected))
        .when(!selected && !look.target, |this| {
            this.hover(move |s| s.bg(hover))
        })
        // The row a dragged topic would land on (2.2: nothing but
        // `text.primary` on `accent.subtle`).
        .when(look.target, |this| this.bg(t.accent_subtle))
        .when(look.dragged, |this| this.opacity(0.6))
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
                    // The release that ends a drag is not a click.
                    if let Some(bench) = shell.bench()
                        && std::mem::take(&mut bench.tree.swallow_click)
                    {
                        return;
                    }
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
        if !look.sticky
            && matches!(
                row,
                Row::Project { .. } | Row::Epic { .. } | Row::Topic { .. }
            )
        {
            // A press that moves 4 starts a drag (components.md 5).
            let pressed = row.clone();
            el = el.on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(move |shell, e: &gpui_kit::MouseDownEvent, _, cx| {
                    shell.tree_press(&pressed, e, cx)
                }),
            );
        }
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
    el.children(menu.map(|menu| {
        div()
            .absolute()
            .top_0()
            .left(px(8. + indent))
            .w(px(1.))
            .h(px(tree::ROW))
            .child(menu)
    }))
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
