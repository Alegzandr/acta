//! Client-side titlebar, docs/design/layout.md 4.3: 36 high, `bg.sunken`, bottom border
//! `border.subtle`, the whole bar a drag region except its controls.
//!
//! Left to right: back and forward, the breadcrumb, the drag region,
//! "Search" with the palette shortcut, the view toggles, the area toggles
//! and the Layout menu (workbench.md 4.8.5), then the platform's window
//! controls. A toggle whose view or area is visible is drawn selected;
//! every toggle is always shown, because it is the way back (4.8.11).
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
    Context, Decorations, Hsla, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, PathBuilder, Pixels, Point, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, WindowControlArea, canvas, div, point, px,
};

use crate::i18n::{Key, KeyName, text};
use crate::icons::{Icon, icon};
use crate::layout::Side;
use crate::route::Route;
use crate::settings;
use crate::shell::Shell;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;

pub const HEIGHT: f32 = 36.;
/// macOS: where the traffic lights sit, and where content starts after them.
pub const TRAFFIC_LIGHT_INSET: f32 = 12.;
const MACOS_CONTENT_START: f32 = 76.;
const CONTENT_START: f32 = 12.;

/// The app icon of the titlebar (layout.md 4.3; Windows and Linux): the
/// hand-rendered bitmaps of `assets/icons/hicolor`, one per scale, so the
/// 16 logical pixels are drawn from a bitmap of their device size and never
/// from a downscaled large one.
const ICON_16: &[u8] = include_bytes!("../../../assets/icons/hicolor/16x16/apps/acta.png");
const ICON_24: &[u8] = include_bytes!("../../../assets/icons/hicolor/24x24/apps/acta.png");
const ICON_32: &[u8] = include_bytes!("../../../assets/icons/hicolor/32x32/apps/acta.png");

/// The bitmap for a scale factor: 16 at 1x, 24 at 1.5x, 32 at 2x and above.
pub fn icon_size_for_scale(scale: f32) -> u32 {
    if scale >= 1.75 {
        32
    } else if scale >= 1.25 {
        24
    } else {
        16
    }
}

fn app_icon_image(scale: f32) -> std::sync::Arc<gpui_kit::Image> {
    use std::sync::{Arc, OnceLock};
    static IMAGES: [OnceLock<Arc<gpui_kit::Image>>; 3] =
        [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let (slot, bytes) = match icon_size_for_scale(scale) {
        32 => (&IMAGES[2], ICON_32),
        24 => (&IMAGES[1], ICON_24),
        _ => (&IMAGES[0], ICON_16),
    };
    slot.get_or_init(|| {
        Arc::new(gpui_kit::Image::from_bytes(
            gpui_kit::ImageFormat::Png,
            bytes.to_vec(),
        ))
    })
    .clone()
}

/// The 16 app icon, part of the drag region, with no hover state; at 60%
/// opacity when the window is inactive, as the titlebar's text dims.
fn app_icon(window: &Window) -> gpui_kit::AnyElement {
    div()
        .id("titlebar-app-icon")
        .flex_shrink_0()
        .size(px(16.))
        .window_control_area(WindowControlArea::Drag)
        .when(!window.is_window_active(), |this| this.opacity(0.6))
        .child(gpui_kit::img(app_icon_image(window.scale_factor())).size(px(16.)))
        .into_any_element()
}
const GLYPH: f32 = 10.;

/// Size of the titlebar's icon buttons: compact on macOS (docs/design/layout.md 4.1).
const BUTTON: f32 = if cfg!(target_os = "macos") { 24. } else { 28. };
/// layout.md 4.3: under 1040 of window width, "Search" is its icon only.
const SEARCH_LABEL_MIN: f32 = 1040.;

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

/// One part of the breadcrumb: a project or epic link, or the page itself.
pub struct Crumb {
    pub label: SharedString,
    pub route: Option<Route>,
}

/// The titlebar of `shell`'s window. `crumbs` is the active page's
/// location, the last part being the page itself (docs/design/layout.md 4.3).
pub fn render(
    shell: &Shell,
    crumbs: Vec<Crumb>,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> impl IntoElement + use<> {
    let tokens = cx.global::<Tokens>().clone();
    let locale = settings::locale(cx);
    let windows = cfg!(target_os = "windows");
    let macos = cfg!(target_os = "macos");

    // Inactive window: text and icons drop to text.tertiary, nothing else.
    let active = window.is_window_active();
    let tertiary_if_inactive = |color: Hsla| if active { color } else { tokens.text_tertiary };
    let icon_rest = tertiary_if_inactive(tokens.text_secondary);

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

    // First run, key loss and open failed (screens.md 8.5, 8.9): only the
    // name and the window controls. The specimen has no workbench.
    let bare = shell.bare_titlebar() || !shell.is_workbench();
    let button = |id, glyph, selected, disabled| {
        ui::ghost_icon_button(id, glyph, BUTTON, selected, disabled, active, &tokens)
    };

    // Windows and Linux show the app icon first; macOS has its traffic
    // lights there and no icon.
    let show_icon = !macos;
    let leading = div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .gap(px(4.))
        .pl(px(content_start))
        .when(show_icon, |this| {
            this.child(app_icon(window)).child(div().w(px(4.)))
        })
        .child(
            button(
                "titlebar-back",
                Icon::ArrowLeft,
                false,
                !shell.can_go_back(),
            )
            .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::shell::Back), cx)),
        )
        .child(
            button(
                "titlebar-forward",
                Icon::ArrowRight,
                false,
                !shell.can_go_forward(),
            )
            .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::shell::Forward), cx)),
        );

    let drag_area = div()
        .id("titlebar-drag")
        .flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .items_center()
        .pl(px(if bare && !show_icon {
            content_start
        } else {
            8.
        }))
        .window_control_area(WindowControlArea::Drag)
        .when(!windows, |this| {
            // Windows moves and zooms the window itself from the drag
            // area; elsewhere the app does it.
            this.on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, _| shell.titlebar_should_move = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|shell, _, _, _| shell.titlebar_should_move = false),
            )
            .on_mouse_down_out(cx.listener(|shell, _, _, _| shell.titlebar_should_move = false))
            .on_mouse_move(cx.listener(|shell, event: &MouseMoveEvent, window, _| {
                if shell.titlebar_should_move && event.dragging() {
                    shell.titlebar_should_move = false;
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
        .child(breadcrumb(
            crumbs,
            &tokens,
            tertiary_if_inactive(tokens.text_secondary),
            cx,
        ));

    let narrow = f32::from(window.viewport_size().width) < SEARCH_LABEL_MIN;
    let search_group = "titlebar-search";
    let hover = tokens.fill_hover;
    let label_hover = tertiary_if_inactive(tokens.text_primary);
    let search = div()
        .id(search_group)
        .group(search_group)
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .gap(px(8.))
        .h(px(BUTTON))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .hover(move |style| style.bg(hover))
        .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::shell::OpenPalette), cx))
        .child(icon(Icon::Search, 16., icon_rest))
        .when(!narrow, |this| {
            this.child(
                type_scale::UI
                    .el()
                    .text_color(icon_rest)
                    .group_hover(search_group, move |style| style.text_color(label_hover))
                    .child(text(locale, Key::CommonSearch)),
            )
            .child(ui::kbd(&[KeyName::Mod.label(locale, macos), "K"], &tokens))
        });

    // 4.8.5: one toggle per view (the chat on Windows only, ADR 0009),
    // then the area toggles and the Layout menu. Right click on a view
    // toggle opens its view menu; on an area toggle, the Layout menu.
    let menu_at = |e: &MouseDownEvent| (f32::from(e.position.x), f32::from(e.position.y) + 8.);
    let mut views = div().flex().flex_row().items_center().gap(px(4.));
    for view in crate::shell::views() {
        let name = text(locale, crate::shell::view_name(view));
        views = views.child(
            ui::ghost_icon_button(
                format!("titlebar-view-{}", view.key()),
                crate::shell::view_icon(view),
                BUTTON,
                shell.view_selected(view, window),
                false,
                active,
                &tokens,
            )
            .tooltip(ui::tip_keys(
                name,
                crate::shell::view_keys(view, macos, locale),
            ))
            .on_click(cx.listener(move |shell, _, window, cx| shell.toggle_view(view, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, e: &MouseDownEvent, _, cx| {
                    let (x, y) = menu_at(e);
                    shell.open_menu(crate::shell::LayoutMenu::View(view), x - 200., y, cx)
                }),
            ),
        );
    }
    let mut areas = div().flex().flex_row().items_center().gap(px(4.));
    for (side, glyph, key) in [
        (Side::Left, Icon::PanelLeft, Key::AreaLeft),
        (Side::Bottom, Icon::PanelBottom, Key::AreaBottom),
        (Side::Right, Icon::PanelRight, Key::AreaRight),
    ] {
        areas = areas.child(
            ui::ghost_icon_button(
                format!("titlebar-area-{}", side.key()),
                glyph,
                BUTTON,
                shell.area_shown(side, window),
                false,
                active,
                &tokens,
            )
            .tooltip(ui::tip_keys(
                text(locale, key),
                crate::shell::area_keys(side, macos, locale),
            ))
            .on_click(cx.listener(move |shell, _, window, cx| shell.toggle_area(side, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, e: &MouseDownEvent, _, cx| {
                    let (x, y) = menu_at(e);
                    shell.open_menu(crate::shell::LayoutMenu::Layout, x - 240., y, cx)
                }),
            ),
        );
    }
    let layout_menu = button("titlebar-layout", Icon::PanelsTopLeft, false, false)
        .tooltip(ui::tip(text(locale, Key::LayoutMenu)))
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(cx.listener(move |shell, e: &gpui_kit::ClickEvent, _, cx| {
            let p = e.position();
            shell.open_menu(
                crate::shell::LayoutMenu::Layout,
                f32::from(p.x) - 240.,
                f32::from(p.y) + 12.,
                cx,
            )
        }));
    let trailing = div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .gap(px(4.))
        .pr(px(8.))
        .child(search)
        .child(div().w(px(4.)))
        .child(views)
        .child(div().w(px(4.)))
        .child(areas)
        .child(layout_menu);

    div()
        .id("titlebar")
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .w_full()
        .h(px(HEIGHT))
        .bg(tokens.bg_sunken)
        .border_b_1()
        .border_color(tokens.border_subtle)
        .when(!bare, |this| this.child(leading))
        .when(bare && show_icon, |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .h_full()
                    .pl(px(content_start))
                    .child(app_icon(window)),
            )
        })
        .child(drag_area)
        .when(!bare, |this| this.child(trailing))
        .children(
            controls
                .into_iter()
                .map(|control| control_button(control, &tokens, icon_rest)),
        )
}

/// The breadcrumb: `type.small`, parts separated by " / " in
/// `text.tertiary`; every part but the last is a link to that page.
fn breadcrumb(
    crumbs: Vec<Crumb>,
    tokens: &Tokens,
    last_color: Hsla,
    cx: &mut Context<Shell>,
) -> impl IntoElement + use<> {
    let count = crumbs.len();
    let tertiary = tokens.text_tertiary;
    let primary = tokens.text_primary;
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .min_w_0()
        .overflow_hidden();
    for (i, crumb) in crumbs.into_iter().enumerate() {
        if i > 0 {
            row = row.child(
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .text_color(tertiary)
                    .child("\u{a0}/\u{a0}"),
            );
        }
        let last = i + 1 == count;
        row =
            match (crumb.route, last) {
                (Some(route), false) => row.child(
                    type_scale::SMALL
                        .el()
                        .id(("crumb", i))
                        .flex_shrink_0()
                        .text_color(tertiary)
                        .cursor_pointer()
                        .hover(move |s| s.text_color(primary).underline())
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            shell.open_crumb(route, window, cx)
                        }))
                        .child(crumb.label),
                ),
                _ => row.child(
                    type_scale::SMALL
                        .el()
                        .min_w_0()
                        .truncate()
                        .text_color(if last { last_color } else { tertiary })
                        .child(crumb.label),
                ),
            };
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_icon_bitmap_follows_the_scale() {
        assert_eq!(icon_size_for_scale(1.0), 16);
        assert_eq!(icon_size_for_scale(1.25), 24);
        assert_eq!(icon_size_for_scale(1.5), 24);
        assert_eq!(icon_size_for_scale(2.0), 32);
    }
}
