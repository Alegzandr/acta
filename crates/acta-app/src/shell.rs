//! The window frame shared by every Acta window: titlebar on top, one view
//! below on `bg.base` (DESIGN.md 4.2; no sidebar or terminal yet).

use gpui_kit::component::window_border;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyView, App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Subscription, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions, div,
    point, px, size,
};

use crate::theme::{self, Tokens};
use crate::titlebar::{self, TitleBar};
use crate::type_scale;

/// DESIGN.md 4.2: default 1440 x 900, minimum 880 x 560.
pub const DEFAULT_SIZE: (f32, f32) = (1440., 900.);
pub const MIN_SIZE: (f32, f32) = (880., 560.);

pub struct Shell {
    titlebar: Entity<TitleBar>,
    content: AnyView,
    _appearance: Subscription,
}

impl Shell {
    pub fn new(content: AnyView, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // System follows the OS live (DESIGN.md 2).
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            theme::os_appearance_changed(window.appearance(), cx);
        });
        Self {
            titlebar: cx.new(|_| TitleBar::new(acta_core::APP_NAME)),
            content,
            _appearance: appearance,
        }
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = cx.global::<Tokens>();
        // Windows: 1px outline when not maximized (DESIGN.md 4.3).
        let outlined = cfg!(target_os = "windows") && !window.is_maximized();

        let frame = type_scale::UI
            .el()
            .size_full()
            .flex()
            .flex_col()
            .bg(tokens.bg_base)
            .text_color(tokens.text_primary)
            .when(outlined, |this| {
                this.border_1().border_color(tokens.border_subtle)
            })
            .child(self.titlebar.clone())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.content.clone()),
            );

        if cfg!(target_os = "linux") {
            // Client-side decorations: resize edges and the window shadow.
            window_border().child(frame).into_any_element()
        } else {
            frame.into_any_element()
        }
    }
}

/// Options of an Acta window of the given default size, centred.
pub fn window_options(default_size: (f32, f32), cx: &App) -> WindowOptions {
    let inset = px(titlebar::TRAFFIC_LIGHT_INSET);
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(acta_core::APP_NAME.into()),
            appears_transparent: true,
            traffic_light_position: Some(point(inset, inset)),
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(default_size.0), px(default_size.1)),
            cx,
        ))),
        window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
        window_decorations: Some(WindowDecorations::Client),
        // The titlebar starts window moves itself on macOS (see titlebar.rs).
        app_owns_titlebar_drag: true,
        app_id: Some("acta".into()),
        ..Default::default()
    }
}
