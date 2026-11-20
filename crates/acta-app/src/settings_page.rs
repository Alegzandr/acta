//! The Settings page (DESIGN.md 5, Settings page; 8.12), as far as M1 goes:
//! the Appearance section with Theme and Language. Everything applies the
//! moment it changes; there is no Save button.
//!
//! Left out until they can work, since DESIGN.md defines no "not available"
//! state for a setting: Terminal text size (no terminal), and the Working
//! folders, Claude Code, Data, Chat and About sections.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Div, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px,
};

use crate::first_run::COLUMN_MAX;
use crate::i18n::{Key, Locale, text};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_MD, RADIUS_SM, ThemeSetting, Tokens};
use crate::type_scale;

pub struct SettingsPage {
    language_menu_open: bool,
}

impl SettingsPage {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            language_menu_open: false,
        }
    }
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

/// The segmented control (DESIGN.md 5): 28 high, a 1px `border.strong`
/// outline, segments sized to their label.
fn theme_control(locale: Locale, current: ThemeSetting, t: &Tokens) -> impl IntoElement {
    let segments = [
        ("theme-system", ThemeSetting::System),
        ("theme-light", ThemeSetting::Light),
        ("theme-dark", ThemeSetting::Dark),
    ]
    .into_iter()
    .map(|(id, setting)| {
        let selected = setting == current;
        let role = if selected {
            type_scale::UI_STRONG
        } else {
            type_scale::UI
        };
        let hover = t.fill_hover;
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
    div()
        .flex()
        .flex_row()
        .h(px(28.))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(t.border_strong)
        .overflow_hidden()
        .children(segments)
}

impl SettingsPage {
    /// The language dropdown: a secondary button showing the current value,
    /// opening a menu of the two languages (DESIGN.md 5, menus).
    fn language_control(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let hover = t.fill_hover;
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
            .on_click(cx.listener(|page, _, _, cx| {
                page.language_menu_open = !page.language_menu_open;
                cx.notify();
            }))
            .child(locale.native_name())
            .child(icon(Icon::ChevronDown, 16., t.text_secondary));

        let menu = self.language_menu_open.then(|| {
            let selected_fill = t.fill_selected;
            let items = Locale::ALL.into_iter().map(|option| {
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
                    .hover(move |style| style.bg(selected_fill))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.language_menu_open = false;
                        settings::set_language(option, cx);
                        cx.notify();
                    }))
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

        div().child(button).children(menu)
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
                        theme_control(locale, theme, &t),
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
