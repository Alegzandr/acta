//! The Sessions view (docs/design/components-app.md 5, Claude Code sessions):
//! every live Claude Code session, its focus, state and last activity, with
//! the actions Acta can take on the ones running in its own terminal.

use acta_core::model::{Owner, ProjectId};
use acta_core::presence::LiveSession;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};

use crate::i18n::{Arg, Key, text, tr};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::Tokens;
use crate::type_scale;
use crate::ui;
use crate::workspace::Workspace;

pub enum SessionsEvent {
    /// Open the page of the session's focus.
    Open(Owner),
    /// Show the terminal running the session (a session in Acta).
    ShowTerminal(ProjectId),
    /// End the terminal's process.
    End(ProjectId),
}

pub struct SessionsView {
    ws: Entity<Workspace>,
    focus: FocusHandle,
    selected: usize,
}

impl EventEmitter<SessionsEvent> for SessionsView {}

impl Focusable for SessionsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// The title of a record from the loaded tree.
pub fn title_of(ws: &Workspace, owner: Owner) -> Option<String> {
    for tree in &ws.projects {
        match owner {
            Owner::Project(id) if tree.project.id == id => return Some(tree.project.title.clone()),
            Owner::Epic(id) => {
                if let Some((e, _)) = tree.epics.iter().find(|(e, _)| e.id == id) {
                    return Some(e.title.clone());
                }
            }
            Owner::Topic(id) => {
                if let Some(t) = tree
                    .loose_topics
                    .iter()
                    .chain(tree.epics.iter().flat_map(|(_, t)| t))
                    .find(|t| t.id == id)
                {
                    return Some(t.title.clone());
                }
            }
            _ => {}
        }
    }
    None
}

impl SessionsView {
    pub fn new(ws: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        Self {
            ws,
            focus: cx.focus_handle(),
            selected: 0,
        }
    }

    /// The header's context: "3 running".
    pub fn header_context(&self, cx: &App) -> Option<AnyElement> {
        let n = crate::sessions::live(cx).len();
        if n == 0 {
            return None;
        }
        let t = cx.global::<Tokens>();
        let locale = settings::locale(cx);
        Some(
            type_scale::SMALL
                .el()
                .text_color(t.text_tertiary)
                .child(tr(
                    locale,
                    Key::SessionsRunning,
                    &[("n", Arg::Count(n as u64))],
                ))
                .into_any_element(),
        )
    }

    fn row(
        &self,
        index: usize,
        s: &LiveSession,
        in_acta: Option<ProjectId>,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locale = settings::locale(cx);
        let owner = crate::sessions::place(s);
        let name = title_of(self.ws.read(cx), owner).unwrap_or_default();
        let label = crate::terminal_dock::focus_label(locale, owner, &name);
        let color = crate::sessions::state_color(s.state, t);
        let selected = index == self.selected;
        let id = SharedString::from(format!("session-row-{index}"));
        let hover = t.fill_hover;
        div()
            .id(id.clone())
            .group(id.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(44.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(crate::theme::RADIUS_SM))
            .when(selected, |this| this.bg(t.fill_selected))
            .when(!selected, |this| this.hover(move |st| st.bg(hover)))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.selected = index;
                cx.emit(SessionsEvent::Open(owner));
            }))
            .child(icon(Icon::Bot, 14., color))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .min_w_0()
                            .child(
                                type_scale::UI
                                    .el()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(t.text_primary)
                                    .child(label),
                            )
                            .child(
                                type_scale::SMALL
                                    .el()
                                    .flex_shrink_0()
                                    .text_color(color)
                                    .child(crate::sessions::state_word(locale, s.state)),
                            ),
                    )
                    .child(
                        type_scale::SMALL
                            .el()
                            .min_w_0()
                            .truncate()
                            .text_color(t.text_tertiary)
                            .child(crate::sessions::detail(locale, s, true)),
                    ),
            )
            .when_some(in_acta, |this, project| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.))
                        .when(!selected, |d| {
                            d.invisible().group_hover(id.clone(), |st| st.visible())
                        })
                        .child(
                            ui::ghost_icon_button(
                                format!("session-terminal-{index}"),
                                Icon::SquareTerminal,
                                24.,
                                false,
                                false,
                                true,
                                t,
                            )
                            .tooltip(ui::tip(text(locale, Key::SessionsShowTerminal)))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.stop_propagation();
                                    cx.emit(SessionsEvent::ShowTerminal(project));
                                },
                            )),
                        )
                        .child(
                            ui::ghost_icon_button(
                                format!("session-end-{index}"),
                                Icon::X,
                                24.,
                                false,
                                false,
                                true,
                                t,
                            )
                            .tooltip(ui::tip(text(locale, Key::SessionsEnd)))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.stop_propagation();
                                    cx.emit(SessionsEvent::End(project));
                                },
                            )),
                        ),
                )
            })
            .into_any_element()
    }
}

impl Render for SessionsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let list: Vec<LiveSession> = crate::sessions::live(cx).to_vec();
        let terminals = cx
            .try_global::<TerminalTokens>()
            .map(|g| g.0.clone())
            .unwrap_or_default();
        self.selected = self.selected.min(list.len().saturating_sub(1));
        let body: Vec<AnyElement> = if list.is_empty() {
            vec![
                type_scale::SMALL
                    .el()
                    .p(px(12.))
                    .max_w(px(480.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::SessionsEmpty))
                    .into_any_element(),
            ]
        } else {
            list.iter()
                .enumerate()
                .map(|(i, s)| {
                    let in_acta = s
                        .launch
                        .as_deref()
                        .and_then(|tok| terminals.iter().find(|(t, _)| t == tok).map(|(_, p)| *p));
                    self.row(i, s, in_acta, &t, cx)
                })
                .collect()
        };
        div()
            .id("sessions-view")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .bg(t.bg_sunken)
            .p(px(4.))
            .on_key_down(cx.listener(move |view, e: &gpui_kit::KeyDownEvent, _, cx| {
                let n = crate::sessions::live(cx).len();
                match e.keystroke.key.as_str() {
                    "down" if view.selected + 1 < n => view.selected += 1,
                    "up" => view.selected = view.selected.saturating_sub(1),
                    "enter" => {
                        if let Some(s) = crate::sessions::live(cx).get(view.selected) {
                            cx.emit(SessionsEvent::Open(crate::sessions::place(s)));
                        }
                    }
                    _ => return,
                }
                cx.notify();
            }))
            .children(body)
    }
}

/// The launch tokens of the terminals running now, with their project,
/// set by the shell from the dock each frame (the view never reads the
/// dock itself).
#[derive(Default)]
pub struct TerminalTokens(pub Vec<(String, ProjectId)>);

impl gpui_kit::Global for TerminalTokens {}
