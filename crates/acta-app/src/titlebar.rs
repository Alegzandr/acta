//! Client-side titlebar, DESIGN.md 4.3: 36 high, `bg.sunken`, bottom border
//! `border.subtle`, the whole bar a drag region except its controls.
//!
//! gpui-component's `TitleBar` is not used: it is 34 high with 34 wide
//! controls, a gradient fill and the `danger` colour on the close button,
//! none of which can be themed to the spec.
//!
//! Per platform:
//! - Windows: caption buttons drawn here, 46 x 36, reported to the system as
//!   native caption areas (so Snap Layouts works); dragging and double-click
//!   are handled by the system through the drag area.
//! - macOS: native traffic lights, content starts at x = 76 (12 in full screen).
//! - Linux: 36 x 36 controls when the window is client-decorated; with
//!   server-side decorations this is a plain toolbar without controls.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, Decorations, Hsla, InteractiveElement, IntoElement, MouseButton, MouseMoveEvent,
    ParentElement, PathBuilder, Pixels, Point, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, WindowControlArea, canvas, div, point, px,
};

use crate::theme::Tokens;
use crate::type_scale;

pub const HEIGHT: f32 = 36.;
/// macOS: where the traffic lights sit, and where content starts after them.
pub const TRAFFIC_LIGHT_INSET: f32 = 12.;
const MACOS_CONTENT_START: f32 = 76.;
const CONTENT_START: f32 = 12.;
const GLYPH: f32 = 10.;

pub struct TitleBar {
    title: SharedString,
    /// A press on the bar that has not moved yet (macOS and Linux, where the
    /// app starts the window move itself).
    should_move: bool,
}

impl TitleBar {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            should_move: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Control {
    fn id(self) -> &'static str {
        match self {
            Control::Minimize => "caption-minimize",
            Control::Maximize => "caption-maximize",
            Control::Restore => "caption-restore",
            Control::Close => "caption-close",
        }
    }

    fn area(self) -> WindowControlArea {
        match self {
            Control::Minimize => WindowControlArea::Min,
            Control::Maximize | Control::Restore => WindowControlArea::Max,
            Control::Close => WindowControlArea::Close,
        }
    }
}

/// The 10px, 1px-stroke caption glyph, in the current text colour.
fn glyph(control: Control) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let color = window.text_style().color;
            let at = |x: f32, y: f32| -> Point<Pixels> {
                point(bounds.origin.x + px(x), bounds.origin.y + px(y))
            };
            let mut path = PathBuilder::stroke(px(1.));
            match control {
                Control::Minimize => {
                    path.move_to(at(0., 5.5));
                    path.line_to(at(10., 5.5));
                }
                Control::Maximize => {
                    path.move_to(at(0.5, 0.5));
                    path.line_to(at(9.5, 0.5));
                    path.line_to(at(9.5, 9.5));
                    path.line_to(at(0.5, 9.5));
                    path.close();
                }
                Control::Restore => {
                    path.move_to(at(0.5, 2.5));
                    path.line_to(at(7.5, 2.5));
                    path.line_to(at(7.5, 9.5));
                    path.line_to(at(0.5, 9.5));
                    path.close();
                    path.move_to(at(2.5, 2.5));
                    path.line_to(at(2.5, 0.5));
                    path.line_to(at(9.5, 0.5));
                    path.line_to(at(9.5, 7.5));
                    path.line_to(at(7.5, 7.5));
                }
                Control::Close => {
                    path.move_to(at(0.5, 0.5));
                    path.line_to(at(9.5, 9.5));
                    path.move_to(at(9.5, 0.5));
                    path.line_to(at(0.5, 9.5));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size(px(GLYPH))
}

fn control_button(control: Control, tokens: &Tokens, rest: Hsla) -> Stateful<gpui_kit::Div> {
    let windows = cfg!(target_os = "windows");
    let width = if windows { 46. } else { HEIGHT };
    let (hover_bg, hover_fg, pressed_bg) = if control == Control::Close {
        (
            tokens.caption_close_hover,
            tokens.caption_close_glyph,
            tokens.caption_close_hover,
        )
    } else {
        (tokens.fill_hover, tokens.text_primary, tokens.fill_pressed)
    };

    div()
        .id(control.id())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(px(width))
        .h_full()
        .text_color(rest)
        .hover(|style| style.bg(hover_bg).text_color(hover_fg))
        .active(|style| style.bg(pressed_bg).text_color(hover_fg))
        .map(|this| {
            if windows {
                // The system performs the action; it only needs the area.
                this.window_control_area(control.area())
            } else {
                this.on_mouse_down(MouseButton::Left, |_, window, cx| {
                    // Not the start of a window move.
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    match control {
                        Control::Minimize => window.minimize_window(),
                        Control::Maximize | Control::Restore => window.zoom_window(),
                        Control::Close => window.remove_window(),
                    }
                })
            }
        })
        .child(glyph(control))
}

impl Render for TitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = cx.global::<Tokens>().clone();
        let windows = cfg!(target_os = "windows");
        let macos = cfg!(target_os = "macos");

        // Inactive window: text and icons drop to text.tertiary, nothing else.
        let active = window.is_window_active();
        let text = if active {
            tokens.text_primary
        } else {
            tokens.text_tertiary
        };
        let icon = if active {
            tokens.text_secondary
        } else {
            tokens.text_tertiary
        };

        let content_start = if macos && !window.is_fullscreen() {
            MACOS_CONTENT_START
        } else {
            CONTENT_START
        };

        // Which controls this platform wants from us.
        let client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let supported = window.window_controls();
        let controls: Vec<Control> = if macos || !(windows || client_decorated) {
            Vec::new()
        } else {
            let zoom = if window.is_maximized() {
                Control::Restore
            } else {
                Control::Maximize
            };
            [
                supported.minimize.then_some(Control::Minimize),
                supported.maximize.then_some(zoom),
                Some(Control::Close),
            ]
            .into_iter()
            .flatten()
            .collect()
        };

        let drag_area = div()
            .id("titlebar-drag")
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .pl(px(content_start))
            .window_control_area(WindowControlArea::Drag)
            .when(!windows, |this| {
                // Windows moves and zooms the window itself from the drag
                // area; elsewhere the app does it.
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|bar, _, _, _| bar.should_move = true),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|bar, _, _, _| bar.should_move = false),
                )
                .on_mouse_down_out(cx.listener(|bar, _, _, _| bar.should_move = false))
                .on_mouse_move(cx.listener(|bar, event: &MouseMoveEvent, window, _| {
                    if bar.should_move && event.dragging() {
                        bar.should_move = false;
                        window.start_window_move();
                    }
                }))
                .on_click(move |event, window, _| {
                    if event.click_count() == 2 {
                        if macos {
                            window.titlebar_double_click();
                        } else {
                            window.zoom_window();
                        }
                    }
                })
            })
            .child(
                type_scale::UI_STRONG
                    .el()
                    .text_color(text)
                    .child(self.title.clone()),
            );

        div()
            .id("titlebar")
            .flex()
            .flex_row()
            .flex_shrink_0()
            .w_full()
            .h(px(HEIGHT))
            .bg(tokens.bg_sunken)
            .border_b_1()
            .border_color(tokens.border_subtle)
            .child(drag_area)
            .children(
                controls
                    .into_iter()
                    .map(|control| control_button(control, &tokens, icon)),
            )
    }
}
