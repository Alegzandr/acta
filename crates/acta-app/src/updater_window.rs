//! The updater window (docs/design/components-app.md 5 and 8.16): shown when the main window is
//! not ready 300 ms after launch because an update is being checked or
//! downloaded. 360 x 132, no caption buttons, draggable anywhere, nothing to
//! press. It is the one place the app mark is shown.

use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Bounds, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    Styled, Subscription, Window, WindowBounds, WindowControlArea, WindowDecorations,
    WindowOptions, div, img, px, relative, size,
};

use crate::i18n::{Key, percent, text};
use crate::icons::APP_MARK;
use crate::settings;
use crate::theme::{self, RADIUS_XL, RADIUS_XS, Tokens};
use crate::type_scale;
use crate::update::{LaunchState, Status};

pub const SIZE: (f32, f32) = (360., 132.);

pub struct UpdaterWindow {
    state: Arc<LaunchState>,
    _appearance: Subscription,
}

impl UpdaterWindow {
    pub fn new(state: Arc<LaunchState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // In the theme of the preferences file, System following the OS
        // (docs/design/components-app.md 5), also when the OS answered before this window.
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            theme::os_appearance_changed(window.appearance(), cx);
        });
        theme::os_appearance_changed(window.appearance(), cx);
        Self {
            state,
            _appearance: appearance,
        }
    }
}

pub fn window_options(focus: bool, cx: &App) -> WindowOptions {
    WindowOptions {
        // No caption, no caption buttons.
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(SIZE.0), px(SIZE.1)),
            cx,
        ))),
        window_decorations: Some(WindowDecorations::Client),
        is_resizable: false,
        is_minimizable: false,
        app_id: Some("acta".into()),
        focus,
        ..Default::default()
    }
}

impl Render for UpdaterWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let status = self.state.status();
        let (line, fraction) = match status {
            Status::Checking => (Key::UpdaterChecking, None),
            Status::Downloading(percent) => {
                (Key::UpdaterDownloading, Some(f32::from(percent) / 100.))
            }
            Status::Restarting => (Key::UpdaterRestarting, Some(1.)),
        };
        let percent_text = fraction.map(|f| percent(locale, (f * 100.).round() as u64));

        div()
            .id("updater")
            .size_full()
            .flex()
            .flex_col()
            .p(px(24.))
            .bg(t.bg_base)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_XL))
            .text_color(t.text_primary)
            // Draggable anywhere: the system moves it on Windows; elsewhere
            // the app starts the move.
            .window_control_area(WindowControlArea::Drag)
            .when(!cfg!(target_os = "windows"), |this| {
                this.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(img(APP_MARK).size(px(24.)))
                    .child(type_scale::UI_STRONG.el().child(acta_core::APP_NAME)),
            )
            .child(type_scale::UI.el().mt(px(16.)).child(text(locale, line)))
            .when_some(fraction, |this, fraction| {
                this.child(
                    div()
                        .mt(px(12.))
                        .w_full()
                        .h(px(4.))
                        .rounded(px(RADIUS_XS))
                        .bg(t.fill_selected)
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .rounded(px(RADIUS_XS))
                                .bg(t.text_secondary),
                        ),
                )
            })
            .when_some(percent_text, |this, percent_text| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .flex()
                        .justify_end()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(percent_text),
                )
            })
    }
}
