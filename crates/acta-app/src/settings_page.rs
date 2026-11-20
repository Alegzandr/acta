//! The Settings page (DESIGN.md 5, Settings page; 8.12), as far as M1 goes:
//! the Appearance section with Theme and Language. Everything applies the
//! moment it changes; there is no Save button.
//!
//! Left out until they can work, since DESIGN.md defines no "not available"
//! state for a setting: Terminal text size (no terminal), and the Working
//! folders, Claude Code, Data, Chat and About sections.
//!
//! Both controls take keyboard focus (`Tab`), with the focus ring of
//! DESIGN.md 5 shown for keyboard focus only. On the theme control `Left` /
//! `Right` move between segments; on the language dropdown `Enter`, `Space`
//! or `Down` open the menu, `Up` / `Down` move in it, `Enter` / `Space`
//! choose and `Esc` closes.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Div, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, Stateful, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point,
    px, transparent_black,
};

use crate::first_run::COLUMN_MAX;
use crate::i18n::{Key, Locale, text};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_MD, RADIUS_SM, ThemeSetting, Tokens};
use crate::type_scale;

const THEMES: [ThemeSetting; 3] = [
    ThemeSetting::System,
    ThemeSetting::Light,
    ThemeSetting::Dark,
];

/// The theme `key` moves to from `current` on the segmented control, if any.
/// The ends do not wrap.
fn theme_after_key(current: ThemeSetting, key: &str) -> Option<ThemeSetting> {
    let at = THEMES.iter().position(|theme| *theme == current)?;
    let to = match key {
        "left" => at.checked_sub(1)?,
        "right" => at + 1,
        _ => return None,
    };
    THEMES.get(to).copied()
}

/// What a key does to a dropdown menu of `count` items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuKey {
    /// Open it with this item highlighted.
    Open(usize),
    /// Move the highlight.
    Highlight(usize),
    /// Choose this item and close.
    Choose(usize),
    Close,
    Ignore,
}

/// `selected` is the current value's item, `highlighted` the open menu's.
fn menu_key(open: bool, selected: usize, highlighted: usize, count: usize, key: &str) -> MenuKey {
    match (open, key) {
        (false, "enter" | "space" | "down") => MenuKey::Open(selected),
        (false, _) => MenuKey::Ignore,
        (true, "up") => MenuKey::Highlight(highlighted.saturating_sub(1)),
        (true, "down") => MenuKey::Highlight((highlighted + 1).min(count.saturating_sub(1))),
        (true, "enter" | "space") => MenuKey::Choose(highlighted),
        (true, "escape") => MenuKey::Close,
        (true, _) => MenuKey::Ignore,
    }
}

pub struct SettingsPage {
    theme_focus: FocusHandle,
    language_focus: FocusHandle,
    language_menu_open: bool,
    language_highlighted: usize,
}

impl SettingsPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            theme_focus: cx.focus_handle(),
            language_focus: cx.focus_handle(),
            language_menu_open: false,
            language_highlighted: 0,
        }
    }

    fn choose_language(&mut self, index: usize, cx: &mut Context<Self>) {
        self.language_menu_open = false;
        if let Some(locale) = Locale::ALL.get(index) {
            settings::set_language(*locale, cx);
        }
        cx.notify();
    }
}

/// The focus ring of DESIGN.md 5 around a control of radius `radius`: 2px
/// `focus.ring`, 1px outside the control's edge, radius plus 1 outside it,
/// shown for keyboard focus only. It takes no layout space.
fn focus_ring(id: &'static str, handle: &FocusHandle, radius: f32, t: &Tokens) -> Stateful<Div> {
    let ring = t.focus_ring;
    div()
        .id(id)
        .track_focus(handle)
        .tab_index(0)
        .m(px(-3.))
        .p(px(1.))
        .border_2()
        .border_color(transparent_black())
        .rounded(px(radius + 3.))
        .focus_visible(move |style| style.border_color(ring))
}

/// A setting row: label and description at the left, the control at the
/// right; at least 44 high with 8 of vertical padding; no boxes, no lines.
fn setting_row(label: String, description: String, control: impl IntoElement, t: &Tokens) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(24.))
        .min_h(px(44.))
        .py(px(8.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(type_scale::UI.el().text_color(t.text_primary).child(label))
                .child(
                    type_scale::SMALL
                        .el()
                        .max_w(px(420.))
                        .text_color(t.text_tertiary)
                        .child(description),
                ),
        )
        .child(div().flex_shrink_0().child(control))
}

fn theme_label(setting: ThemeSetting) -> Key {
    match setting {
        ThemeSetting::System => Key::SettingsThemeSystem,
        ThemeSetting::Light => Key::SettingsThemeLight,
        ThemeSetting::Dark => Key::SettingsThemeDark,
    }
}

impl SettingsPage {
    /// The segmented control (DESIGN.md 5): 28 high, a 1px `border.strong`
    /// outline, segments sized to their label.
    fn theme_control(&self, locale: Locale, current: ThemeSetting, t: &Tokens) -> impl IntoElement {
        let segments = THEMES.into_iter().map(|setting| {
            let selected = setting == current;
            let role = if selected {
                type_scale::UI_STRONG
            } else {
                type_scale::UI
            };
            let hover = t.fill_hover;
            let id = match setting {
                ThemeSetting::System => "theme-system",
                ThemeSetting::Light => "theme-light",
                ThemeSetting::Dark => "theme-dark",
            };
            role.el()
                .id(id)
                .flex()
                .items_center()
                .h_full()
                .px(px(12.))
                .text_color(if selected {
                    t.text_primary
                } else {
                    t.text_secondary
                })
                .when(selected, |this| this.bg(t.fill_selected))
                .when(!selected, |this| this.hover(move |style| style.bg(hover)))
                .on_click(move |_, _, cx| settings::set_theme(setting, cx))
                .child(text(locale, theme_label(setting)))
        });
        focus_ring("theme-control", &self.theme_focus, RADIUS_SM, t)
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if let Some(next) = theme_after_key(current, &event.keystroke.key) {
                    settings::set_theme(next, cx);
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h(px(28.))
                    .rounded(px(RADIUS_SM))
                    .border_1()
                    .border_color(t.border_strong)
                    .overflow_hidden()
                    .children(segments),
            )
    }

    /// The language dropdown: a secondary button showing the current value,
    /// opening a menu of the two languages (DESIGN.md 5, menus).
    fn language_control(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let hover = t.fill_hover;
        let selected = Locale::ALL.iter().position(|l| *l == locale).unwrap_or(0);
        let button = type_scale::UI
            .el()
            .id("language")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(12.))
            .rounded(px(RADIUS_SM))
            .border_1()
            .border_color(t.border_strong)
            .text_color(t.text_primary)
            .hover(move |style| style.bg(hover))
            .on_click(cx.listener(move |page, _, _, cx| {
                page.language_menu_open = !page.language_menu_open;
                page.language_highlighted = selected;
                cx.notify();
            }))
            .child(locale.native_name())
            .child(icon(Icon::ChevronDown, 16., t.text_secondary));

        let control = focus_ring("language-control", &self.language_focus, RADIUS_SM, t)
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, _, cx| {
                let key = menu_key(
                    page.language_menu_open,
                    selected,
                    page.language_highlighted,
                    Locale::ALL.len(),
                    &event.keystroke.key,
                );
                match key {
                    MenuKey::Open(index) => {
                        page.language_menu_open = true;
                        page.language_highlighted = index;
                    }
                    MenuKey::Highlight(index) => page.language_highlighted = index,
                    MenuKey::Choose(index) => page.choose_language(index, cx),
                    MenuKey::Close => page.language_menu_open = false,
                    MenuKey::Ignore => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(button);

        let menu = self.language_menu_open.then(|| {
            let selected_fill = t.fill_selected;
            let highlighted = self.language_highlighted;
            let items = Locale::ALL.into_iter().enumerate().map(|(index, option)| {
                type_scale::UI
                    .el()
                    .id(option.native_name())
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .h(px(32.))
                    .px(px(8.))
                    .rounded(px(RADIUS_SM))
                    .text_color(t.text_primary)
                    .when(index == highlighted, |this| this.bg(selected_fill))
                    .hover(move |style| style.bg(selected_fill))
                    .on_click(cx.listener(move |page, _, _, cx| page.choose_language(index, cx)))
                    .child(option.native_name())
                    .when(option == locale, |this| {
                        this.child(icon(Icon::Check, 16., t.text_secondary))
                    })
            });
            deferred(
                anchored().offset(point(px(0.), px(32.))).child(
                    div()
                        .id("language-menu")
                        .occlude()
                        .flex()
                        .flex_col()
                        .min_w(px(200.))
                        .p(px(4.))
                        .bg(t.bg_overlay)
                        .border_1()
                        .border_color(t.border_default)
                        .rounded(px(RADIUS_MD))
                        .shadow(vec![t.shadow_overlay()])
                        .on_mouse_down_out(cx.listener(|page, _, _, cx| {
                            page.language_menu_open = false;
                            cx.notify();
                        }))
                        .children(items),
                ),
            )
        });

        div().child(control).children(menu)
    }
}

fn locale_and_theme(cx: &App) -> (Locale, ThemeSetting) {
    let current = settings::current(cx);
    (current.language, current.theme)
}

impl Render for SettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let (locale, theme) = locale_and_theme(cx);
        let theme_control = self.theme_control(locale, theme, &t);
        let language = self.language_control(locale, &t, cx);

        div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .px(px(32.))
            .child(
                div()
                    .w_full()
                    .max_w(px(COLUMN_MAX))
                    .pt(px(32.))
                    .pb(px(48.))
                    .flex()
                    .flex_col()
                    .child(
                        type_scale::TITLE
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::TreeSettings)),
                    )
                    .child(
                        type_scale::SUBTITLE
                            .el()
                            .mt(px(32.))
                            .mb(px(8.))
                            .text_color(t.text_primary)
                            .child(text(locale, Key::SettingsAppearance)),
                    )
                    .child(setting_row(
                        text(locale, Key::SettingsTheme),
                        text(locale, Key::SettingsThemeHint),
                        theme_control,
                        &t,
                    ))
                    .child(setting_row(
                        text(locale, Key::SettingsLanguage),
                        text(locale, Key::SettingsLanguageHint),
                        language,
                        &t,
                    )),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_move_between_theme_segments_without_wrapping() {
        use ThemeSetting::*;
        assert_eq!(theme_after_key(System, "right"), Some(Light));
        assert_eq!(theme_after_key(Light, "right"), Some(Dark));
        assert_eq!(theme_after_key(Dark, "right"), None);
        assert_eq!(theme_after_key(Dark, "left"), Some(Light));
        assert_eq!(theme_after_key(System, "left"), None);
        assert_eq!(theme_after_key(System, "a"), None);
    }

    #[test]
    fn the_dropdown_opens_moves_chooses_and_closes_by_keyboard() {
        // Closed: Enter, Space and Down open it on the current value.
        for key in ["enter", "space", "down"] {
            assert_eq!(menu_key(false, 1, 0, 2, key), MenuKey::Open(1));
        }
        assert_eq!(menu_key(false, 1, 0, 2, "escape"), MenuKey::Ignore);
        // Open: the highlight moves and stops at the ends.
        assert_eq!(menu_key(true, 0, 0, 2, "down"), MenuKey::Highlight(1));
        assert_eq!(menu_key(true, 0, 1, 2, "down"), MenuKey::Highlight(1));
        assert_eq!(menu_key(true, 0, 0, 2, "up"), MenuKey::Highlight(0));
        assert_eq!(menu_key(true, 0, 1, 2, "enter"), MenuKey::Choose(1));
        assert_eq!(menu_key(true, 0, 1, 2, "space"), MenuKey::Choose(1));
        assert_eq!(menu_key(true, 0, 1, 2, "escape"), MenuKey::Close);
        assert_eq!(menu_key(true, 0, 1, 2, "x"), MenuKey::Ignore);
    }
}
