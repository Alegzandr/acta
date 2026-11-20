//! The workbench window (DESIGN.md 4.2): titlebar, the tree sidebar, the page
//! area, the docks and the status bar.
//!
//! As far as M1 goes: there are no projects yet, so the tree is empty and,
//! as on first run (DESIGN.md 8.5), the sidebar starts hidden; `Mod+B` or the
//! titlebar toggle shows it with its pinned Trash and Settings rows. The
//! page is the first-run page or Settings. The docks have no tool window to
//! hold yet and are not drawn. The status bar has no active project, and
//! the Claude Code status reads "not set up" until the setup check of ADR
//! 0005 exists to report anything else.

use gpui_kit::component::window_border;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyView, App, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Subscription, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions,
    actions, div, point, px, size,
};

use crate::first_run::FirstRun;
use crate::i18n::{Arg, Key, text, tr};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::settings_page::SettingsPage;
use crate::theme::{self, RADIUS_SM, Tokens};
use crate::titlebar;
use crate::type_scale;
use crate::ui;

/// DESIGN.md 4.2: default 1440 x 900, minimum 880 x 560.
pub const DEFAULT_SIZE: (f32, f32) = (1440., 900.);
pub const MIN_SIZE: (f32, f32) = (880., 560.);
/// DESIGN.md 4.2: the sidebar is 260 wide.
const SIDEBAR_WIDTH: f32 = 260.;
/// DESIGN.md 4.6: the status bar is 24 high.
const STATUS_BAR_HEIGHT: f32 = 24.;

actions!(
    acta,
    [
        /// `Mod+B`, palette "Toggle sidebar".
        ToggleSidebar,
        /// `Mod+,`, palette "Open settings".
        OpenSettings,
    ]
);

/// The keys of the shell (DESIGN.md 6.1).
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(key_bindings());
}

fn key_bindings() -> [KeyBinding; 2] {
    // `Mod` is Cmd on macOS and Ctrl elsewhere. The pinned GPUI only reads
    // "secondary" when the binding names its OS, so it is spelled out.
    let (toggle_sidebar, open_settings) = if cfg!(target_os = "macos") {
        ("cmd-b", "cmd-,")
    } else {
        ("ctrl-b", "ctrl-,")
    };
    [
        KeyBinding::new(toggle_sidebar, ToggleSidebar, None),
        KeyBinding::new(open_settings, OpenSettings, None),
    ]
}

/// What the page area shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    FirstRun,
    Settings,
}

enum Content {
    Workbench {
        page: Page,
        first_run: Entity<FirstRun>,
        settings: Entity<SettingsPage>,
    },
    /// The type specimen, a development aid: no sidebar, no status bar.
    Specimen(AnyView),
}

pub struct Shell {
    content: Content,
    sidebar_visible: bool,
    focus: FocusHandle,
    /// A press on the titlebar that has not moved yet (macOS and Linux,
    /// where the app starts the window move itself).
    pub titlebar_should_move: bool,
    _appearance: Subscription,
}

/// How a workbench window starts. Development aids aside, it is always the
/// first-run page with the sidebar hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    pub page: Page,
    pub sidebar: bool,
}

impl Default for Start {
    fn default() -> Self {
        Self {
            page: Page::FirstRun,
            sidebar: false,
        }
    }
}

impl Shell {
    pub fn workbench(start: Start, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let content = Content::Workbench {
            page: start.page,
            first_run: cx.new(|_| FirstRun),
            settings: cx.new(SettingsPage::new),
        };
        Self::new(content, start.sidebar, window, cx)
    }

    pub fn specimen(view: AnyView, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Content::Specimen(view), false, window, cx)
    }

    fn new(content: Content, sidebar: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // System follows the OS live (DESIGN.md 2).
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            theme::os_appearance_changed(window.appearance(), cx);
        });
        // The OS may have answered before this window existed (the Linux
        // portal replies asynchronously and only notifies open windows).
        theme::os_appearance_changed(window.appearance(), cx);
        let focus = cx.focus_handle();
        // Focus inside the window, so its keys reach the shell. This does
        // not raise or activate the window.
        window.focus(&focus, cx);
        Self {
            content,
            sidebar_visible: sidebar,
            focus,
            titlebar_should_move: false,
            _appearance: appearance,
        }
    }

    pub fn sidebar_visible(&self) -> bool {
        self.sidebar_visible
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        if matches!(self.content, Content::Workbench { .. }) {
            self.sidebar_visible = !self.sidebar_visible;
            cx.notify();
        }
    }

    pub fn open(&mut self, target: Page, cx: &mut Context<Self>) {
        if let Content::Workbench { page, .. } = &mut self.content {
            *page = target;
            cx.notify();
        }
    }

    fn page(&self) -> Option<Page> {
        match &self.content {
            Content::Workbench { page, .. } => Some(*page),
            Content::Specimen(_) => None,
        }
    }

    /// The breadcrumb's last part: the page itself.
    fn breadcrumb(&self, cx: &App) -> SharedString {
        let locale = settings::locale(cx);
        match self.page() {
            Some(Page::FirstRun) => acta_core::APP_NAME.into(),
            Some(Page::Settings) => text(locale, Key::TreeSettings).into(),
            None => "Type specimen".into(),
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tokens = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let on_settings = self.page() == Some(Page::Settings);
        // Pinned under the tree, outside its scroll (DESIGN.md 4.2). Trash
        // has no page yet, so its row is inert.
        let pinned = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .p(px(8.))
            .border_t_1()
            .border_color(tokens.border_subtle)
            .child(sidebar_row(
                "sidebar-trash",
                Icon::Trash2,
                text(locale, Key::TreeTrash),
                false,
                &tokens,
            ))
            .child(
                sidebar_row(
                    "sidebar-settings",
                    Icon::Settings,
                    text(locale, Key::TreeSettings),
                    on_settings,
                    &tokens,
                )
                .on_click(cx.listener(|shell, _, _, cx| shell.open(Page::Settings, cx))),
            );
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(tokens.bg_sunken)
            .border_r_1()
            .border_color(tokens.border_subtle)
            // The tree: every project. None exists yet, and an empty tree
            // shows nothing (DESIGN.md 1: a surface with nothing on it is
            // correct).
            .child(div().id("tree").flex_1().min_h_0().p(px(8.)))
            .child(pinned)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tokens = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let version = tr(
            locale,
            Key::StatusbarVersion,
            &[("version", Arg::Text(acta_core::VERSION))],
        );
        div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(STATUS_BAR_HEIGHT))
            .bg(tokens.bg_sunken)
            .border_t_1()
            .border_color(tokens.border_subtle)
            .text_color(tokens.text_tertiary)
            .child(
                // No project item: there is no project (DESIGN.md 4.6).
                ui::bar_item("status-claude", &tokens)
                    .child(icon(Icon::CircleDashed, 14., tokens.text_tertiary))
                    .child(
                        type_scale::SMALL
                            .el()
                            .child(text(locale, Key::StatusbarNotSetUp)),
                    )
                    .on_click(cx.listener(|shell, _, _, cx| shell.open(Page::Settings, cx))),
            )
            .child(
                ui::bar_item("status-version", &tokens)
                    .child(
                        type_scale::SMALL
                            .el()
                            .font_features(type_scale::tabular())
                            .child(version),
                    )
                    .on_click(cx.listener(|shell, _, _, cx| shell.open(Page::Settings, cx))),
            )
    }
}

/// A pinned row of the sidebar (DESIGN.md 5, list / tree row).
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
    let role = if selected {
        type_scale::UI_STRONG
    } else {
        type_scale::UI
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
            role.el()
                .min_w_0()
                .truncate()
                .text_color(rest)
                .group_hover(id, move |style| style.text_color(primary))
                .child(label),
        )
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = cx.global::<Tokens>().clone();
        // Windows: 1px outline when not maximized (DESIGN.md 4.3).
        let outlined = cfg!(target_os = "windows") && !window.is_maximized();
        let breadcrumb = self.breadcrumb(cx);
        let titlebar = titlebar::render(self, breadcrumb, window, cx);

        let body = match &self.content {
            Content::Workbench {
                page,
                first_run,
                settings,
            } => {
                let page_view: AnyView = match page {
                    Page::FirstRun => first_run.clone().into(),
                    Page::Settings => settings.clone().into(),
                };
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .when(self.sidebar_visible, |this| this.child(self.sidebar(cx)))
                    .child(div().flex_1().min_w_0().h_full().child(page_view))
                    .into_any_element()
            }
            Content::Specimen(view) => div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(view.clone())
                .into_any_element(),
        };
        let status_bar = self.page().map(|_| self.status_bar(cx));

        let frame = type_scale::UI
            .el()
            .id("shell")
            .track_focus(&self.focus)
            .on_action(cx.listener(|shell, _: &ToggleSidebar, _, cx| shell.toggle_sidebar(cx)))
            .on_action(cx.listener(|shell, _: &OpenSettings, _, cx| shell.open(Page::Settings, cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(tokens.bg_base)
            .text_color(tokens.text_primary)
            .when(outlined, |this| {
                this.border_1().border_color(tokens.border_subtle)
            })
            .child(titlebar)
            .child(body)
            .children(status_bar);

        if cfg!(target_os = "linux") {
            // Client-side decorations: resize edges and the window shadow.
            window_border().child(frame).into_any_element()
        } else {
            frame.into_any_element()
        }
    }
}

/// Options of an Acta window of the given default size, centred. `focus`
/// is false only for the test hook that must not take the foreground.
pub fn window_options(default_size: (f32, f32), focus: bool, cx: &App) -> WindowOptions {
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
        focus,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_bindings_parse() {
        // `KeyBinding::new` panics on a keystroke the pinned GPUI rejects.
        assert_eq!(key_bindings().len(), 2);
    }
}
