//! The Settings page (docs/design/components-app.md 5, Settings page; 8.12):
//! Appearance, Claude Code, Working folders, Data, Chat and About.
//! Everything applies the moment it changes; there is no Save button.
//!
//! Left out: the "Turn off Claude Code's own memory in attached folders"
//! checkbox. Only `acta link` writes `autoMemoryEnabled` today; the app
//! attaches folders without touching them, so the setting would have
//! nothing to turn off.
//!
//! Both controls take keyboard focus (`Tab`), with the focus ring of
//! docs/design/components.md 5 shown for keyboard focus only. On the theme control `Left` /
//! `Right` move between segments; on the language dropdown `Enter`, `Space`
//! or `Down` open the menu, `Up` / `Down` move in it, `Enter` / `Space`
//! choose and `Esc` closes.

use crate::zoom::px;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Task, Window, div,
};

use crate::i18n::{Key, Locale, text};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_SM, ThemeSetting, Tokens};
use crate::type_scale;
use crate::ui;
use crate::workspace::Workspace;

const THEMES: [ThemeSetting; 3] = [
    ThemeSetting::System,
    ThemeSetting::Light,
    ThemeSetting::Dark,
];

/// The theme `key` moves to from `current` on the segmented control, if any.
/// The ends do not wrap; a key pressed with a modifier is not ours.
fn theme_after_key(current: ThemeSetting, key: &str, modified: bool) -> Option<ThemeSetting> {
    after_key(&THEMES, current, key, modified)
}

/// What an arrow key does in a segmented control of `all`: the neighbour,
/// or nothing at either end.
fn after_key<T: Copy + PartialEq>(all: &[T], current: T, key: &str, modified: bool) -> Option<T> {
    if modified {
        return None;
    }
    let at = all.iter().position(|value| *value == current)?;
    let to = match key {
        "left" => at.checked_sub(1)?,
        "right" => at + 1,
        _ => return None,
    };
    all.get(to).copied()
}

/// A segmented control over `options` (components.md 5) with its own focus
/// ring and arrow keys; `set` applies a value.
fn choice_control<T: Copy + PartialEq + 'static>(
    id: &'static str,
    focus: &FocusHandle,
    options: Vec<(T, &'static str, String)>,
    current: T,
    set: fn(T, &mut gpui_kit::App),
    t: &Tokens,
) -> impl IntoElement {
    let all: Vec<T> = options.iter().map(|(value, _, _)| *value).collect();
    let segments = options.into_iter().map(|(value, segment_id, label)| {
        let selected = value == current;
        let hover = t.fill_hover;
        let role = if selected {
            type_scale::UI_STRONG
        } else {
            type_scale::UI
        };
        role.el()
            .id(segment_id)
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
            .on_click(move |_, _, cx| set(value, cx))
            .child(label)
    });
    focus_ring(id, focus, RADIUS_SM, t)
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if let Some(next) = after_key(
                &all,
                current,
                &event.keystroke.key,
                event.keystroke.modifiers.modified(),
            ) {
                set(next, cx);
                cx.stop_propagation();
            }
        })
        .child(
            div()
                .flex()
                .flex_row()
                .h(px(28.))
                .rounded(px(RADIUS_SM))
                .border(px(1.))
                .border_color(t.border_strong)
                .overflow_hidden()
                .children(segments),
        )
}

/// What a key does to a dropdown menu of `count` items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuKey {
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
/// Keys pressed with a modifier are not the menu's.
pub(crate) fn menu_key(
    open: bool,
    selected: usize,
    highlighted: usize,
    count: usize,
    key: &str,
    modified: bool,
) -> MenuKey {
    if modified {
        return MenuKey::Ignore;
    }
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
    text_size_focus: FocusHandle,
    reading_width_focus: FocusHandle,
    language_focus: FocusHandle,
    language_menu_open: bool,
    language_highlighted: usize,
    language_anchor: crate::menu::AnchorBounds,
    /// The setup checks unfolded or folded by the reader (`None`: as the
    /// status asks), and "What setup changes" unfolded.
    checks_open: Option<bool>,
    setup_what_open: bool,
    _language_blur: Subscription,
    ws: Option<Entity<Workspace>>,
    facts: Option<DataFacts>,
    seen_revision: u64,
    _load: Task<()>,
    _ws_sub: Option<Subscription>,
    /// The `acta` command, found off the UI thread.
    cli: Option<std::path::PathBuf>,
    _cli: Task<()>,
    /// Settings > Profile (components-contacts.md 5): the human's name
    /// and job title, stored in the database, and as last read.
    profile_name: Entity<InputState>,
    profile_job: Entity<InputState>,
    profile: acta_core::model::Profile,
    /// A profile was read: the fields show it at the next frame.
    profile_fresh: bool,
    _profile: Vec<Subscription>,
    _profile_load: Task<()>,
    /// The page scroll, and a pending scroll to the Ready block (the
    /// status bar opens Settings there).
    scroll: gpui_kit::ScrollHandle,
    reveal_ready: std::rc::Rc<std::cell::Cell<u8>>,
    /// Settings > Voice (components-voice.md 5), and a pending scroll to
    /// it ("Voice settings").
    voice: Option<Entity<crate::voice::settings_view::VoiceSettings>>,
    reveal_voice: std::rc::Rc<std::cell::Cell<u8>>,
}

impl EventEmitter<crate::ready::Request> for SettingsPage {}

/// How many frames a scroll to the Ready block waits for the page layout.
const REVEAL_FRAMES: u8 = 10;

impl SettingsPage {
    #[cfg(windows)]
    /// The language menu is open (the chat's web view hides under it).
    pub fn menu_open(&self) -> bool {
        self.language_menu_open
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let language_focus = cx.focus_handle();
        // The menu belongs to its control: it closes when focus leaves it.
        let language_blur = cx.on_blur(&language_focus, window, |page, _, cx| {
            if page.language_menu_open {
                page.language_menu_open = false;
                cx.notify();
            }
        });
        let find = cx
            .background_executor()
            .spawn(async move { crate::claude_setup::cli_path() });
        let cli_task = cx.spawn(async move |page, cx| {
            let path = find.await;
            let _ = page.update(cx, |page, cx| {
                page.cli = path;
                cx.notify();
            });
        });
        let locale = settings::locale(cx);
        let placeholder = crate::when::system_name(locale);
        let profile_name = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let profile_job = cx.new(|cx| InputState::new(window, cx));
        let commit = |page: &mut Self,
                      _: &Entity<InputState>,
                      event: &InputEvent,
                      _: &mut Window,
                      cx: &mut Context<Self>| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                page.save_profile(cx);
            }
        };
        let profile_subs = vec![
            cx.subscribe_in(&profile_name, window, commit),
            cx.subscribe_in(&profile_job, window, commit),
        ];
        Self {
            profile_name,
            profile_job,
            profile: acta_core::model::Profile::default(),
            profile_fresh: false,
            _profile: profile_subs,
            _profile_load: Task::ready(()),
            theme_focus: cx.focus_handle(),
            text_size_focus: cx.focus_handle(),
            reading_width_focus: cx.focus_handle(),
            language_focus,
            language_menu_open: false,
            language_highlighted: 0,
            language_anchor: Default::default(),
            checks_open: None,
            setup_what_open: false,
            _language_blur: language_blur,
            ws: None,
            facts: None,
            seen_revision: 0,
            _load: Task::ready(()),
            _ws_sub: None,
            cli: None,
            _cli: cli_task,
            scroll: gpui_kit::ScrollHandle::new(),
            reveal_ready: Default::default(),
            voice: crate::voice::get(cx).map(|d| {
                cx.new(|cx| crate::voice::settings_view::VoiceSettings::new(d, window, cx))
            }),
            reveal_voice: Default::default(),
        }
    }

    /// Scroll to the Ready for Claude Code block once it is laid out (the
    /// status bar opens Settings there while a required row waits).
    pub fn reveal_ready(&mut self, cx: &mut Context<Self>) {
        // Tried for a few frames: the page may not be laid out yet.
        self.reveal_ready.set(REVEAL_FRAMES);
        cx.notify();
    }

    /// Scroll to Settings > Voice once it is laid out.
    pub fn reveal_voice(&mut self, cx: &mut Context<Self>) {
        self.reveal_voice.set(REVEAL_FRAMES);
        cx.notify();
    }

    /// Debug builds: the page scrolled to its end (the Data section), for
    /// captures.
    #[cfg(debug_assertions)]
    pub fn dev_scroll_end(&mut self, cx: &mut Context<Self>) {
        let max = self.scroll.max_offset();
        self.scroll
            .set_offset(gpui_kit::point(gpui_kit::px(0.), -max.y));
        cx.notify();
    }

    /// Debug builds: the language menu open, for captures.
    #[cfg(debug_assertions)]
    pub fn dev_open_language(&mut self, cx: &mut Context<Self>) {
        self.language_menu_open = true;
        cx.notify();
    }

    fn choose_language(&mut self, index: usize, cx: &mut Context<Self>) {
        self.language_menu_open = false;
        if let Some(locale) = Locale::ALL.get(index) {
            settings::set_language(*locale, cx);
        }
        cx.notify();
    }
}

use crate::ui::focus_ring;

/// A setting row: label and description at the left, the control at the
/// right; at least 44 high with 8 of vertical padding; no boxes, no lines.
pub(crate) fn setting_row(
    label: String,
    description: String,
    control: impl IntoElement,
    t: &Tokens,
) -> Div {
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
                // No description, no empty line under the label: the label
                // then centres on the control like the others.
                .when(!description.is_empty(), |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .max_w(px(420.))
                            .text_color(t.text_tertiary)
                            .child(description),
                    )
                }),
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
    /// The segmented control (docs/design/components.md 5): 28 high, a 1px `border.strong`
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
                if let Some(next) = theme_after_key(
                    current,
                    &event.keystroke.key,
                    event.keystroke.modifiers.modified(),
                ) {
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
                    .border(px(1.))
                    .border_color(t.border_strong)
                    .overflow_hidden()
                    .children(segments),
            )
    }

    /// The language dropdown: a secondary button showing the current value,
    /// opening a menu of the two languages (docs/design/components.md 5, menus).
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
            .border(px(1.))
            .border_color(t.border_strong)
            .text_color(t.text_primary)
            .hover(move |style| style.bg(hover))
            .on_click(cx.listener(move |page, _, _, cx| {
                page.language_menu_open = !page.language_menu_open;
                page.language_highlighted = selected;
                cx.notify();
            }))
            .relative()
            .child(locale.native_name())
            .child(icon(Icon::ChevronDown, 16., t.text_secondary))
            .child(self.language_anchor.measure());

        let control = focus_ring("language-control", &self.language_focus, RADIUS_SM, t)
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, _, cx| {
                let key = menu_key(
                    page.language_menu_open,
                    selected,
                    page.language_highlighted,
                    Locale::ALL.len(),
                    &event.keystroke.key,
                    event.keystroke.modifiers.modified(),
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
            crate::menu::panel("language-menu", t)
                .on_mouse_down_out(cx.listener(|page, _, _, cx| {
                    page.language_menu_open = false;
                    cx.notify();
                }))
                .children(items)
        });

        // Under the control, right edges aligned: the control ends the row.
        let menu =
            menu.and_then(|panel| self.language_anchor.below(panel, crate::menu::Align::End));
        div().child(control).children(menu)
    }
}

fn locale_and_theme(cx: &App) -> (Locale, ThemeSetting) {
    let current = settings::current(cx);
    (current.language, current.theme)
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.profile_fresh) {
            let focused =
                |input: &Entity<InputState>| input.read(cx).focus_handle(cx).is_focused(window);
            if !focused(&self.profile_name) && !focused(&self.profile_job) {
                self.restore_profile(window, cx);
            }
        }
        let t = cx.global::<Tokens>().clone();
        let (locale, theme) = locale_and_theme(cx);
        let theme_control = self.theme_control(locale, theme, &t);
        let prefs = settings::current(cx);
        let text_size_control = choice_control(
            "text-size-control",
            &self.text_size_focus,
            crate::type_scale::TextSize::ALL
                .into_iter()
                .zip(["text-size-small", "text-size-default", "text-size-large"])
                .map(|(size, id)| (size, id, text(locale, crate::commands::text_size_key(size))))
                .collect(),
            prefs.text_size,
            settings::set_text_size,
            &t,
        );
        let reading_width_control = choice_control(
            "reading-width-control",
            &self.reading_width_focus,
            crate::ui::ReadingWidth::ALL
                .into_iter()
                .zip(["width-narrow", "width-default", "width-wide"])
                .map(|(width, id)| {
                    (
                        width,
                        id,
                        text(locale, crate::commands::reading_width_key(width)),
                    )
                })
                .collect(),
            prefs.reading_width,
            settings::set_reading_width,
            &t,
        );
        let language = self.language_control(locale, &t, cx);
        let text_size = self.text_size_control(&t, cx);
        let zoom_control = zoom_control(window, &t, cx);
        let claude = self.claude_section(locale, &t, cx);
        // Voice, after Claude Code and before Data (components-voice.md 5).
        let voice = self.voice.clone().map(|voice| {
            let reveal = self.reveal_voice.clone();
            let scroll = self.scroll.clone();
            div()
                .relative()
                .child(voice)
                .child(ui::measure(move |bounds, window| {
                    let left = reveal.get();
                    if left == 0 {
                        return;
                    }
                    if f32::from(scroll.max_offset().y) > 0. || left == 1 {
                        ui::scroll_into_view(&scroll, bounds, ui::Reveal::Top);
                        reveal.set(0);
                    } else {
                        reveal.set(left - 1);
                    }
                    window.refresh();
                }))
        });
        let data = self.data_sections(locale, &t, cx);
        let about = about_section(locale, &t);
        let profile = self.profile_section(locale, &t, cx);

        div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .items_center()
            .px(px(ui::page_padding()))
            .child(
                div()
                    .w_full()
                    .max_w(px(crate::ui::column_max()))
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
                    .children(profile)
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
                        String::new(),
                        theme_control,
                        &t,
                    ))
                    .child(setting_row(
                        text(locale, Key::SettingsLanguage),
                        text(locale, Key::SettingsLanguageHint),
                        language,
                        &t,
                    ))
                    // layout.md 4.9: after Language, before Text size.
                    .child(setting_row(
                        text(locale, Key::SettingsZoom),
                        text(locale, Key::SettingsZoomHint),
                        zoom_control,
                        &t,
                    ))
                    .child(setting_row(
                        text(locale, Key::SettingsTextSize),
                        text(locale, Key::SettingsTextSizeHint),
                        text_size_control,
                        &t,
                    ))
                    .child(setting_row(
                        text(locale, Key::SettingsReadingWidth),
                        String::new(),
                        reading_width_control,
                        &t,
                    ))
                    .child(setting_row(
                        text(locale, Key::SettingsLayout),
                        text(locale, Key::SettingsLayoutDesc),
                        // workbench.md 4.8.13: undoable, so it asks nothing.
                        ui::button(
                            "settings-layout-reset",
                            text(locale, Key::CmdLayoutReset),
                            ui::Variant::Secondary,
                            false,
                            &t,
                        )
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(crate::shell::ResetLayout), cx)
                        }),
                        &t,
                    ))
                    .child(setting_row(
                        text(locale, Key::SettingsTerminalSize),
                        String::new(),
                        text_size,
                        &t,
                    ))
                    .children(claude)
                    .children(voice)
                    .children(data)
                    .children(about),
            )
    }
}

/// The Zoom row's stepper (layout.md 4.9), the Terminal text size row's:
/// "[- 150% +]", minus off at 90%, plus off at the largest step the
/// window's work area holds.
fn zoom_control(window: &Window, t: &Tokens, cx: &mut Context<SettingsPage>) -> Div {
    let applied = (crate::zoom::factor() * 100.).round() as u16;
    let max = window.display(cx).map_or(crate::zoom::STEPS[6], |d| {
        let a = d.visible_bounds().size;
        crate::zoom::largest_fitting(f32::from(a.width), f32::from(a.height))
    });
    let locale = settings::locale(cx);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .child(
            ui::ghost_icon_button(
                "zoom-down",
                Icon::Minus,
                24.,
                false,
                applied <= crate::zoom::STEPS[0],
                true,
                t,
            )
            .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::shell::ZoomOut), cx)),
        )
        .child(
            type_scale::UI
                .el()
                .min_w(px(44.))
                .flex()
                .justify_center()
                .font_features(type_scale::tabular())
                .text_color(t.text_primary)
                .child(crate::i18n::percent(locale, u64::from(applied))),
        )
        .child(
            ui::ghost_icon_button("zoom-up", Icon::Plus, 24., false, applied >= max, true, t)
                .on_click(|_, window, cx| {
                    window.dispatch_action(Box::new(crate::shell::ZoomIn), cx)
                }),
        )
}

/// What the Working folders and Data sections show (docs/design/data.md 11:
/// export history, the snapshot list, the trash, the storage location, and
/// every folder not found on this computer).
#[derive(Clone, Debug, Default)]
pub struct DataFacts {
    pub last_export: Option<i64>,
    pub snapshots: usize,
    pub newest_snapshot: Option<i64>,
    pub trash: i64,
    pub database: String,
    /// Attached files, each content counted once (ADR 0012, 2).
    pub files: acta_core::store::FileTotals,
    /// Folders not found: id, path, owner name.
    pub missing: Vec<(acta_core::model::FolderId, String, String)>,
    /// "Commit in my name" by default (Settings > Claude Code, ADR 0011).
    pub as_user_default: bool,
}

/// Read the facts off the UI thread (existence checks touch the disk).
pub fn load_facts(store: &acta_core::Store) -> acta_core::Result<DataFacts> {
    use acta_core::model::{Owner, PageReq};
    let snapshots = store.snapshots()?;
    let newest = snapshots.iter().map(|s| s.taken_at).max();
    let trash = store.trash(PageReq::first(1))?.total;
    // Every folder with its owner; the caller keeps those not found,
    // checking the disk once the store is released.
    let mut missing = Vec::new();
    for folder in store.all_folders()? {
        let owner = match folder.owner {
            Owner::Project(id) => store.project(id).map(|p| p.title),
            Owner::Epic(id) => store.epic(id).map(|e| e.title),
            Owner::Topic(id) => store.topic(id).map(|t| t.title),
        }
        .unwrap_or_default();
        missing.push((folder.id, folder.path, owner));
    }
    Ok(DataFacts {
        last_export: store.last_export_at()?,
        snapshots: snapshots.len(),
        newest_snapshot: newest,
        trash,
        database: store.db_path().to_string_lossy().into_owned(),
        files: store.attachment_totals(None)?,
        missing,
        as_user_default: acta_core::claude_settings::as_user_default(store)?,
    })
}

impl SettingsPage {
    /// Give the page the open store; the data sections appear.
    pub fn attach(&mut self, ws: Entity<Workspace>, cx: &mut Context<Self>) {
        self._ws_sub = Some(cx.observe(&ws, |page, ws, cx| {
            let revision = ws.read(cx).revision;
            if revision != page.seen_revision {
                page.seen_revision = revision;
                page.reload(cx);
            }
        }));
        self.ws = Some(ws);
        self.reload(cx);
    }

    /// Apply the profile on `Enter` or when a field loses focus, as every
    /// setting (no Save button). An empty name returns to the system name.
    fn save_profile(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = self.ws.clone() else { return };
        let name = self.profile_name.read(cx).value().trim().to_owned();
        let job = self.profile_job.read(cx).value().trim().to_owned();
        let name: String = name.chars().take(80).collect();
        let job: String = job.chars().take(80).collect();
        let now = acta_core::model::Profile {
            name: (!name.is_empty()).then(|| name.clone()),
            job_title: (!job.is_empty()).then(|| job.clone()),
        };
        if now == self.profile {
            return;
        }
        self.profile = now;
        ws.update(cx, |ws, cx| {
            ws.write_detached(cx, Key::ErrorSave, move |store| {
                store.set_profile(Some(&name), Some(&job))
            })
        });
    }

    /// `Esc` in a profile field: the stored value comes back.
    fn restore_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.profile.name.clone().unwrap_or_default();
        let job = self.profile.job_title.clone().unwrap_or_default();
        self.profile_name
            .update(cx, |s, cx| s.set_value(name, window, cx));
        self.profile_job
            .update(cx, |s, cx| s.set_value(job, window, cx));
    }

    /// "Set your name", "Set your job title": Settings focused on the field.
    pub fn focus_profile(&mut self, name: bool, window: &mut Window, cx: &mut Context<Self>) {
        let input = if name {
            &self.profile_name
        } else {
            &self.profile_job
        };
        input.update(cx, |s, cx| s.focus(window, cx));
    }

    fn profile_section(
        &self,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Vec<gpui_kit::AnyElement> {
        if self.ws.is_none() {
            return Vec::new();
        }
        let field = |input: &Entity<InputState>| {
            div()
                .w(px(240.))
                .h(px(28.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(crate::theme::RADIUS_SM))
                .bg(t.bg_raised)
                .border(px(1.))
                .border_color(t.border_default)
                .child(
                    Input::new(input)
                        .appearance(false)
                        .px(px(0.))
                        .text_size(px(type_scale::UI.size())),
                )
        };
        vec![
            type_scale::SUBTITLE
                .el()
                .mt(px(32.))
                .text_color(t.text_primary)
                .child(text(locale, Key::SettingsProfile))
                .into_any_element(),
            type_scale::SMALL
                .el()
                .mt(px(4.))
                .mb(px(8.))
                .text_color(t.text_tertiary)
                .child(text(locale, Key::SettingsProfileHint))
                .into_any_element(),
            div()
                .capture_action(cx.listener(
                    |page, _: &gpui_kit::component::input::Escape, window, cx| {
                        page.restore_profile(window, cx);
                    },
                ))
                .child(setting_row(
                    text(locale, Key::SettingsProfileName),
                    text(locale, Key::SettingsProfileNameHint),
                    field(&self.profile_name),
                    t,
                ))
                .child(setting_row(
                    text(locale, Key::SettingsProfileJobTitle),
                    String::new(),
                    field(&self.profile_job),
                    t,
                ))
                .into_any_element(),
        ]
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = &self.ws else { return };
        let profile = ws.read(cx).db.run(cx, |store| store.profile());
        let loading = cx.spawn(async move |page, cx| {
            if let Ok(profile) = profile.await {
                let _ = page.update(cx, |page, cx| {
                    page.profile = profile;
                    page.profile_fresh = true;
                    cx.notify();
                });
            }
        });
        self._profile_load = loading;
        let task = ws.read(cx).db.run(cx, |store| load_facts(store));
        self._load = cx.spawn(async move |page, cx| {
            if let Ok(mut facts) = task.await {
                let all = std::mem::take(&mut facts.missing);
                facts.missing = cx
                    .background_executor()
                    .spawn(async move {
                        all.into_iter()
                            .filter(|(_, path, _)| {
                                !acta_core::folder_identity::is_dir(std::path::Path::new(path))
                            })
                            .collect()
                    })
                    .await;
                let _ = page.update(cx, |page, cx| {
                    page.facts = Some(facts);
                    cx.notify();
                });
            }
        });
    }

    fn locate(&mut self, id: acta_core::model::FolderId, cx: &mut Context<Self>) {
        let Some(ws) = self.ws.clone() else { return };
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.pop() else { return };
            // The shared Locate (undo and the bulk offer); folders across
            // projects sit on no project's undo stack.
            ws.update(cx, |ws, cx| {
                ws.locate(crate::workspace::FolderRef::Id(id), path, None, cx)
                    .detach()
            });
        })
        .detach();
    }

    fn text_size_control(&self, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let size = settings::current(cx)
            .terminal_text_size
            .unwrap_or(acta_terminal::view::DEFAULT_TEXT_SIZE as u8);
        let step = |by: i8| {
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                let action: Box<dyn gpui_kit::Action> = if by > 0 {
                    Box::new(acta_terminal::view::IncreaseTextSize)
                } else {
                    Box::new(acta_terminal::view::DecreaseTextSize)
                };
                window.dispatch_action(action, cx);
                window.refresh();
            }
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .child(
                ui::ghost_icon_button(
                    "text-size-down",
                    Icon::Minus,
                    24.,
                    false,
                    size <= 11,
                    true,
                    t,
                )
                .on_click(step(-1)),
            )
            .child(
                type_scale::UI
                    .el()
                    .w(px(24.))
                    .flex()
                    .justify_center()
                    .font_features(type_scale::tabular())
                    .text_color(t.text_primary)
                    .child(size.to_string()),
            )
            .child(
                ui::ghost_icon_button("text-size-up", Icon::Plus, 24., false, size >= 18, true, t)
                    .on_click(step(1)),
            )
    }

    fn data_sections(
        &self,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Vec<gpui_kit::AnyElement> {
        let Some(facts) = &self.facts else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let subtitle = |key: Key| {
            type_scale::SUBTITLE
                .el()
                .mt(px(32.))
                .mb(px(8.))
                .text_color(t.text_primary)
                .child(text(locale, key))
                .into_any_element()
        };
        let action = |id: &'static str, label: String, act: Box<dyn gpui_kit::Action>| {
            ui::button(id, label, ui::Variant::Secondary, false, t)
                .on_click(move |_, window, cx| window.dispatch_action(act.boxed_clone(), cx))
        };
        // Working folders: the one place folders not found are listed
        // across projects (present only when some exist).
        if !facts.missing.is_empty() {
            out.push(subtitle(Key::SettingsFolders));
            let home = crate::paths_view::home();
            let mut list = div().flex().flex_col();
            for (i, (id, path, owner)) in facts.missing.iter().take(6).enumerate() {
                let id = *id;
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .child(icon(Icon::FolderX, 16., t.warning))
                        .child(
                            type_scale::CODE
                                .el()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(t.text_secondary)
                                .child(crate::paths_view::shorten_home(path, home.as_deref())),
                        )
                        .child(
                            type_scale::SMALL
                                .el()
                                .text_color(t.text_tertiary)
                                .child(owner.clone()),
                        )
                        .child(
                            ui::button(
                                ("settings-locate", i),
                                text(locale, Key::FoldersLocate),
                                ui::Variant::Ghost,
                                false,
                                t,
                            )
                            .on_click(cx.listener(move |page, _, _, cx| page.locate(id, cx))),
                        ),
                );
            }
            out.push(
                setting_row(
                    text(locale, Key::SettingsNotFound),
                    crate::i18n::number(locale, facts.missing.len() as u64),
                    div(),
                    t,
                )
                .into_any_element(),
            );
            out.push(list.into_any_element());
        }
        out.push(subtitle(Key::SettingsData));
        let export_line = match facts.last_export {
            Some(at) => {
                let days = crate::i18n::time::days_ago(locale, crate::when::days_between(at));
                crate::i18n::tr(
                    locale,
                    Key::SettingsExportLast,
                    &[("days", crate::i18n::Arg::Text(&days))],
                )
            }
            None => text(locale, Key::SettingsExportNever),
        };
        out.push(
            setting_row(
                text(locale, Key::ExportTitle),
                export_line,
                action(
                    "settings-export",
                    text(locale, Key::CmdExport),
                    Box::new(crate::shell::ExportData),
                ),
                t,
            )
            .into_any_element(),
        );
        out.push(
            setting_row(
                text(locale, Key::ImportConfirm),
                String::new(),
                action(
                    "settings-import",
                    text(locale, Key::CmdImport),
                    Box::new(crate::shell::ImportData),
                ),
                t,
            )
            .into_any_element(),
        );
        let snapshots = match facts.newest_snapshot {
            Some(at) if crate::when::days_between(at) == 0 => crate::i18n::tr(
                locale,
                Key::SettingsSnapshotsToday,
                &[
                    ("n", crate::i18n::Arg::Count(facts.snapshots as u64)),
                    (
                        "time",
                        crate::i18n::Arg::Text(&crate::i18n::time::clock(crate::when::local(at))),
                    ),
                ],
            ),
            Some(at) => crate::i18n::tr(
                locale,
                Key::SettingsSnapshotsOlder,
                &[
                    ("n", crate::i18n::Arg::Count(facts.snapshots as u64)),
                    (
                        "datetime",
                        crate::i18n::Arg::Text(&crate::when::datetime(locale, at)),
                    ),
                ],
            ),
            None => String::new(),
        };
        out.push(
            setting_row(
                text(locale, Key::SettingsSnapshots),
                snapshots,
                action(
                    "settings-snapshots",
                    text(locale, Key::SettingsRestore),
                    Box::new(crate::shell::RestoreSnapshot),
                ),
                t,
            )
            .into_any_element(),
        );
        out.push(
            setting_row(
                text(locale, Key::TreeTrash),
                crate::i18n::tr(
                    locale,
                    Key::SettingsTrashCount,
                    &[("n", crate::i18n::Arg::Count(facts.trash.max(0) as u64))],
                ),
                action(
                    "settings-trash",
                    text(locale, Key::CommonOpen),
                    Box::new(crate::shell::OpenTrash),
                ),
                t,
            )
            .into_any_element(),
        );
        out.push(
            setting_row(
                text(locale, Key::SettingsResetLabel),
                String::new(),
                action(
                    "settings-reset",
                    text(locale, Key::SettingsReset),
                    Box::new(crate::shell::ResetData),
                ),
                t,
            )
            .into_any_element(),
        );
        // The path, middle-truncated by folder, in full in the tooltip;
        // "Show in Explorer" (Finder, file manager) reveals the file.
        let database = facts.database.clone();
        let shown = crate::paths_view::fit(&database, crate::paths_view::budget(420.));
        let reveal_key = if cfg!(windows) {
            Key::FoldersShowW
        } else if cfg!(target_os = "macos") {
            Key::FoldersShowM
        } else {
            Key::FoldersShowL
        };
        let reveal_path = database.clone();
        out.push(
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
                        .child(
                            type_scale::UI
                                .el()
                                .text_color(t.text_primary)
                                .child(text(locale, Key::SettingsDatabase)),
                        )
                        .child(
                            type_scale::CODE
                                .el()
                                .id("settings-database-path")
                                .truncate()
                                .text_color(t.text_secondary)
                                .tooltip(ui::tip(database))
                                .child(shown),
                        )
                        // "Attached files: 312 MB in 48 files", so growth
                        // is visible (ADR 0012, 2).
                        .when(facts.files.files > 0, |this| {
                            this.child(
                                type_scale::SMALL
                                    .el()
                                    .mt(px(4.))
                                    .font_features(type_scale::tabular())
                                    .text_color(t.text_secondary)
                                    .child(crate::attach::totals_line(
                                        locale,
                                        facts.files.bytes.max(0) as u64,
                                        facts.files.files.max(0) as u64,
                                    )),
                            )
                        }),
                )
                .child(
                    ui::button(
                        "settings-database-show",
                        text(locale, reveal_key),
                        ui::Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(move |_, _, cx| cx.reveal_path(std::path::Path::new(&reveal_path))),
                )
                .into_any_element(),
        );
        // components-app.md 5, Settings: Chat.
        out.push(subtitle(Key::SettingsChat));
        let chat_row = if cfg!(windows) {
            setting_row(
                text(locale, Key::SettingsChatData),
                String::new(),
                action(
                    "settings-chat-signout",
                    format!("{}\u{2026}", text(locale, Key::ChatSignoutConfirm)),
                    Box::new(crate::shell::ChatSignOut),
                ),
                t,
            )
        } else {
            setting_row(
                text(locale, Key::ChatBrowser),
                String::new(),
                action(
                    "settings-chat-browser",
                    text(locale, Key::ChatOpenBrowser),
                    Box::new(crate::shell::OpenChatInBrowser),
                ),
                t,
            )
        };
        out.push(chat_row.into_any_element());
        out
    }
}

/// The one status row of Acta's setup (components-app.md 5, Settings):
/// nothing until the first check answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupStatus {
    Pending,
    Ready,
    /// This many of the three steps are done.
    Partial(u64),
    /// The check could not run.
    Unknown,
}

/// The status of the three steps setup itself makes (plugin files, plugin,
/// permission rule). Whether Claude Code is installed is a fact about the
/// computer, shown with the checks, not a step of setup.
fn setup_status(checked: bool, answer: Option<crate::claude_setup::Check>) -> SetupStatus {
    match (checked, answer) {
        (false, _) => SetupStatus::Pending,
        (true, None) => SetupStatus::Unknown,
        (true, Some(check)) => {
            let done = [check.files, check.plugin, check.rule]
                .into_iter()
                .filter(|d| *d)
                .count() as u64;
            if done == 3 {
                SetupStatus::Ready
            } else {
                SetupStatus::Partial(done)
            }
        }
    }
}

/// The checks start unfolded while a step is missing, folded otherwise;
/// the reader's own choice wins.
fn checks_unfolded(status: SetupStatus, chosen: Option<bool>) -> bool {
    chosen.unwrap_or(matches!(status, SetupStatus::Partial(_)))
}

/// The word after a setup check: nothing until the first check answers;
/// "Could not check" when the check itself could not run (the `acta`
/// command missing next to the app), rather than claiming a step is
/// missing; then Done or Missing.
fn check_word(checked: bool, answered: bool, done: bool) -> Option<Key> {
    match (checked, answered, done) {
        (_, _, true) => Some(Key::SettingsCheckDone),
        (false, _, false) => None,
        (true, false, false) => Some(Key::SettingsCheckUnknown),
        (true, true, false) => Some(Key::SettingsCheckMissing),
    }
}

/// A ghost disclosure button: the label, then `chevron-down` / `chevron-up`.
fn disclosure(id: &'static str, label: String, open: bool, t: &Tokens) -> gpui_kit::Stateful<Div> {
    ui::button(id, label, ui::Variant::Ghost, false, t).child(icon(
        if open {
            Icon::ChevronUp
        } else {
            Icon::ChevronDown
        },
        14.,
        t.text_tertiary,
    ))
}

impl SettingsPage {
    /// Claude Code (components-app.md 5, Settings): one status row with
    /// its checks behind "Details", then Set up with one plain line and
    /// "What setup changes", Remove, and the commit and session settings.
    fn claude_section(
        &self,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Vec<gpui_kit::AnyElement> {
        let state = crate::claude_setup::state(cx);
        let answer = state.and_then(|s| s.check);
        let check = answer.unwrap_or_default();
        let checked = state.is_some_and(|s| s.checked);
        let busy = state.is_some_and(|s| s.busy);
        let status = setup_status(checked, answer);
        let mut out = vec![
            type_scale::SUBTITLE
                .el()
                .mt(px(32.))
                .mb(px(8.))
                .text_color(t.text_primary)
                .child(text(locale, Key::SettingsClaude))
                .into_any_element(),
        ];
        // Ready for Claude Code (ADR 0013) heads the section; the status
        // bar can ask to scroll here.
        let reveal = self.reveal_ready.clone();
        let scroll = self.scroll.clone();
        out.push(
            div()
                .relative()
                .flex()
                .flex_col()
                .children(crate::ready::block::<Self, crate::ready::Request>(
                    false, locale, t, cx,
                ))
                .child(ui::measure(move |bounds, window| {
                    let left = reveal.get();
                    if left == 0 {
                        return;
                    }
                    if f32::from(scroll.max_offset().y) > 0. || left == 1 {
                        ui::scroll_into_view(&scroll, bounds, ui::Reveal::Top);
                        reveal.set(0);
                    } else {
                        reveal.set(left - 1);
                    }
                    window.refresh();
                }))
                .into_any_element(),
        );
        let ready_primary = crate::ready::has_primary(cx, false);
        // Acta's own setup, 16 below.
        out.push(div().h(px(16.)).into_any_element());
        // The status row.
        let (glyph, color, words) = match status {
            SetupStatus::Ready => (
                Icon::CircleCheck,
                t.success,
                text(locale, Key::SettingsStatusReady),
            ),
            SetupStatus::Partial(n) => (
                Icon::CircleDashed,
                t.text_tertiary,
                crate::i18n::tr(
                    locale,
                    Key::SettingsStatusPartial,
                    &[("n", crate::i18n::Arg::Count(n))],
                ),
            ),
            SetupStatus::Unknown => (
                Icon::CircleSlash,
                t.text_tertiary,
                text(locale, Key::SettingsStatusUnknown),
            ),
            SetupStatus::Pending => (Icon::CircleDashed, t.text_tertiary, String::new()),
        };
        let unfolded = checks_unfolded(status, self.checks_open);
        out.push(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .min_h(px(44.))
                .child(icon(glyph, 16., color))
                .child(
                    type_scale::UI
                        .el()
                        .flex_1()
                        .min_w_0()
                        .text_color(t.text_primary)
                        .child(words),
                )
                .when(status == SetupStatus::Unknown, |this| {
                    this.child(
                        ui::button(
                            "settings-check-again",
                            text(locale, Key::CommonTryAgain),
                            ui::Variant::Ghost,
                            busy,
                            t,
                        )
                        .when(!busy, |this| {
                            this.on_click(|_, _, cx| crate::claude_setup::refresh(cx))
                        }),
                    )
                })
                .child(
                    disclosure(
                        "settings-check-details",
                        text(locale, Key::CommonDetails),
                        unfolded,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.checks_open = Some(!unfolded);
                        cx.notify();
                    })),
                )
                .into_any_element(),
        );
        if unfolded {
            let rows = [
                (Key::SettingsCheckFiles, check.files),
                (Key::SettingsCheckPlugin, check.plugin),
                (Key::SettingsCheckRule, check.rule),
            ];
            let mut list = div().flex().flex_col().pl(px(24.)).mb(px(8.));
            for (key, done) in rows {
                let word = check_word(checked, answer.is_some(), done);
                let (glyph, color) = if done {
                    (Icon::Check, t.success)
                } else {
                    (Icon::CircleDashed, t.text_tertiary)
                };
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .child(icon(glyph, 14., color))
                        .child(
                            type_scale::UI
                                .el()
                                .flex_1()
                                .min_w_0()
                                .text_color(t.text_primary)
                                .child(text(locale, key)),
                        )
                        .children(word.map(|w| {
                            type_scale::UI
                                .el()
                                .text_color(if done {
                                    t.text_primary
                                } else {
                                    t.text_tertiary
                                })
                                .child(text(locale, w))
                        })),
                );
            }
            // The `acta` command, for running setup by hand.
            if let Some(path) = &self.cli {
                let full = path.to_string_lossy().into_owned();
                let copy = full.clone();
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .child(
                            type_scale::CODE
                                .el()
                                .id("settings-cli-path")
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(t.text_secondary)
                                .tooltip(ui::tip(full.clone()))
                                .child(crate::paths_view::fit(
                                    &full,
                                    crate::paths_view::budget(400.),
                                )),
                        )
                        .child(
                            ui::button(
                                "settings-cli-copy",
                                text(locale, Key::FoldersCopyPath),
                                ui::Variant::Ghost,
                                false,
                                t,
                            )
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                    copy.clone(),
                                ))
                            }),
                        ),
                );
            }
            out.push(list.into_any_element());
        }
        // Set up, with one plain line; the mechanics behind "What setup
        // changes" (copy.md 7).
        let (setup_key, setup_variant) = if check.complete() {
            (Key::SettingsSetupAgain, ui::Variant::Secondary)
        } else if ready_primary {
            // The Ready block has the primary until its required rows are
            // ready.
            (Key::SettingsSetup, ui::Variant::Secondary)
        } else {
            (Key::SettingsSetup, ui::Variant::Primary)
        };
        // The row says what setup does for the person; the button names the
        // action (a label never repeats on its own button, copy.md 7).
        out.push(
            setting_row(
                text(locale, Key::SettingsSetupHint),
                String::new(),
                ui::button(
                    "settings-setup",
                    text(locale, setup_key),
                    setup_variant,
                    busy,
                    t,
                )
                .when(!busy, |this| {
                    this.on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(crate::shell::SetUpClaude), cx)
                    })
                }),
                t,
            )
            .into_any_element(),
        );
        let what = self.setup_what_open;
        out.push(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .ml(px(-12.))
                .child(
                    disclosure(
                        "settings-setup-what",
                        text(locale, Key::SettingsSetupWhat),
                        what,
                        t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.setup_what_open = !page.setup_what_open;
                        cx.notify();
                    })),
                )
                .child(
                    ui::button(
                        "settings-setup-remove",
                        text(locale, Key::SettingsSetupRemove),
                        ui::Variant::Ghost,
                        busy,
                        t,
                    )
                    .when(!busy, |this| {
                        // A confirmed case of interaction.md 6.4.
                        this.on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(crate::shell::RemoveClaudeSetup), cx)
                        })
                    }),
                )
                .into_any_element(),
        );
        if what {
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .mt(px(4.))
                    .max_w(px(420.))
                    .child(
                        type_scale::SMALL
                            .el()
                            .text_color(t.text_tertiary)
                            .child(text(locale, Key::SettingsSetupDetails)),
                    )
                    .child(
                        type_scale::SMALL
                            .el()
                            .text_color(t.text_tertiary)
                            .child(text(locale, Key::SettingsPluginHint)),
                    )
                    .into_any_element(),
            );
        }
        // Automatic commits (ADR 0011): the default of "Commit in my name";
        // changing it rewrites every automatic folder that follows it. The
        // checkbox's own label wraps inside its column, never past it.
        let as_user = self.facts.as_ref().is_some_and(|f| f.as_user_default);
        let ws = self.ws.clone();
        out.push(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(24.))
                .min_h(px(44.))
                .py(px(8.))
                .mt(px(16.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            type_scale::UI
                                .el()
                                .text_color(t.text_primary)
                                .child(text(locale, Key::SettingsAutoCommit)),
                        )
                        .child(
                            type_scale::SMALL
                                .el()
                                .text_color(t.text_tertiary)
                                .child(text(locale, Key::SettingsAutoCommitHint)),
                        ),
                )
                .child(
                    div()
                        .id("settings-auto-commit")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .flex_shrink(1.)
                        .min_w_0()
                        .max_w(px(260.))
                        .child(div().flex_shrink_0().child(ui::checkbox(as_user, t)))
                        .child(
                            type_scale::UI
                                .el()
                                .flex_1()
                                .min_w_0()
                                .text_color(t.text_primary)
                                .child(text(locale, Key::FoldersCommitAsUser)),
                        )
                        .on_click(move |_, _, cx| {
                            if let Some(ws) = &ws {
                                ws.update(cx, |ws, cx| {
                                    // Toggled from the value the store holds
                                    // now, not the one this frame showed.
                                    ws.write_detached(cx, Key::ErrorSave, |store| {
                                        let on =
                                            acta_core::claude_settings::as_user_default(store)?;
                                        acta_core::claude_settings::set_as_user_default(store, !on)
                                    })
                                });
                            }
                        }),
                )
                .into_any_element(),
        );
        // How Acta starts and shows Claude Code (components-app.md 5,
        // Claude Code sessions and Plan usage): labels that say it all.
        let prefs = crate::settings::current(cx).clone();
        out.push(
            setting_row(
                text(locale, Key::SettingsResume),
                String::new(),
                checkbox("settings-resume", !prefs.new_conversations, t).on_click(|_, _, cx| {
                    crate::settings::change(cx, |p| p.new_conversations = !p.new_conversations);
                    cx.refresh_windows();
                }),
                t,
            )
            .into_any_element(),
        );
        // Until a report has arrived, the row says why the bar is empty.
        let no_usage = crate::sessions::plan_usage(cx).is_none();
        out.push(
            setting_row(
                text(locale, Key::SettingsUsage),
                String::new(),
                checkbox("settings-usage", !prefs.hide_usage, t).on_click(|_, _, cx| {
                    crate::settings::change(cx, |p| p.hide_usage = !p.hide_usage);
                    cx.refresh_windows();
                }),
                t,
            )
            .id("settings-usage-row")
            .when(no_usage, |this| {
                this.tooltip(ui::tip(text(locale, Key::UsageNoDataTip)))
            })
            .into_any_element(),
        );
        if cfg!(windows) {
            out.push(
                setting_row(
                    text(locale, Key::SettingsFlashWaiting),
                    text(locale, Key::SettingsFlashWaitingHint),
                    checkbox("settings-flash", prefs.flash_waiting, t).on_click(|_, _, cx| {
                        crate::settings::change(cx, |p| p.flash_waiting = !p.flash_waiting);
                        cx.refresh_windows();
                    }),
                    t,
                )
                .into_any_element(),
            );
        }
        out
    }
}

/// The checkbox of components.md 5 as a setting's control: 16, `radius.xs`,
/// `border.strong`; checked, `accent` with a `check` in `text.on-accent`.
pub(crate) fn checkbox(id: &'static str, checked: bool, t: &Tokens) -> gpui_kit::Stateful<Div> {
    let hover = t.fill_hover;
    div()
        .id(id)
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(crate::theme::RADIUS_XS))
        .when(checked, |this| {
            this.bg(t.accent)
                .child(icon(Icon::Check, 12., t.text_on_accent))
        })
        .when(!checked, |this| {
            this.border(px(1.))
                .border_color(t.border_strong)
                .hover(move |s| s.bg(hover))
        })
}

/// The Acta repository's releases: "Release notes".
pub const RELEASES: &str = "https://github.com/Alegzandr/acta/releases";

/// About (components-app.md 5, Settings): the version, the release notes
/// and the licences of what Acta ships.
fn about_section(locale: Locale, t: &Tokens) -> Vec<gpui_kit::AnyElement> {
    let version = crate::i18n::tr(
        locale,
        Key::StatusbarVersion,
        &[("version", crate::i18n::Arg::Text(acta_core::VERSION))],
    );
    vec![
        type_scale::SUBTITLE
            .el()
            .mt(px(32.))
            .mb(px(8.))
            .text_color(t.text_primary)
            .child(text(locale, Key::SettingsAbout))
            .into_any_element(),
        setting_row(
            version,
            String::new(),
            ui::button(
                "settings-release-notes",
                text(locale, Key::SettingsReleaseNotes),
                ui::Variant::Secondary,
                false,
                t,
            )
            .on_click(|_, _, cx| cx.open_url(RELEASES)),
            t,
        )
        .into_any_element(),
        setting_row(
            text(locale, Key::CmdShortcuts),
            String::new(),
            ui::button(
                "settings-shortcuts",
                text(locale, Key::CommonOpen),
                ui::Variant::Secondary,
                false,
                t,
            )
            .on_click(|_, window, cx| {
                window.dispatch_action(Box::new(crate::shell::OpenShortcuts), cx)
            }),
            t,
        )
        .into_any_element(),
        setting_row(
            text(locale, Key::SettingsLicences),
            // Only the fonts this build ships (typography.md 3.1).
            text(
                locale,
                if cfg!(any(target_os = "windows", target_os = "macos")) {
                    Key::SettingsLicencesHint
                } else {
                    Key::SettingsLicencesHintLinux
                },
            ),
            div(),
            t,
        )
        .into_any_element(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_that_could_not_run_claims_nothing_missing() {
        // The `acta` command is not next to the app: no answer.
        assert_eq!(
            check_word(true, false, false),
            Some(Key::SettingsCheckUnknown)
        );
        assert_eq!(
            check_word(true, true, false),
            Some(Key::SettingsCheckMissing)
        );
        assert_eq!(check_word(true, true, true), Some(Key::SettingsCheckDone));
        assert_eq!(check_word(false, false, false), None);
    }

    #[test]
    fn the_status_row_counts_the_three_steps_of_setup() {
        use crate::claude_setup::Check;
        assert_eq!(setup_status(false, None), SetupStatus::Pending);
        assert_eq!(setup_status(true, None), SetupStatus::Unknown);
        let all = Check {
            files: true,
            plugin: true,
            rule: true,
        };
        assert_eq!(setup_status(true, Some(all)), SetupStatus::Ready);
        let one = Check {
            plugin: false,
            rule: false,
            ..all
        };
        assert_eq!(setup_status(true, Some(one)), SetupStatus::Partial(1));
        assert_eq!(
            setup_status(true, Some(Check::default())),
            SetupStatus::Partial(0)
        );
    }

    #[test]
    fn the_checks_unfold_while_a_step_is_missing_unless_folded_by_hand() {
        assert!(checks_unfolded(SetupStatus::Partial(2), None));
        assert!(!checks_unfolded(SetupStatus::Ready, None));
        assert!(!checks_unfolded(SetupStatus::Unknown, None));
        assert!(!checks_unfolded(SetupStatus::Partial(2), Some(false)));
        assert!(checks_unfolded(SetupStatus::Ready, Some(true)));
    }

    #[test]
    fn arrows_move_between_theme_segments_without_wrapping() {
        use ThemeSetting::*;
        assert_eq!(theme_after_key(System, "right", false), Some(Light));
        assert_eq!(theme_after_key(Light, "right", false), Some(Dark));
        assert_eq!(theme_after_key(Dark, "right", false), None);
        assert_eq!(theme_after_key(Dark, "left", false), Some(Light));
        assert_eq!(theme_after_key(System, "left", false), None);
        assert_eq!(theme_after_key(System, "a", false), None);
        assert_eq!(
            theme_after_key(System, "right", true),
            None,
            "Ctrl+Right is not ours"
        );
    }

    #[test]
    fn the_dropdown_opens_moves_chooses_and_closes_by_keyboard() {
        // Closed: Enter, Space and Down open it on the current value.
        for key in ["enter", "space", "down"] {
            assert_eq!(menu_key(false, 1, 0, 2, key, false), MenuKey::Open(1));
        }
        assert_eq!(menu_key(false, 1, 0, 2, "escape", false), MenuKey::Ignore);
        // Open: the highlight moves and stops at the ends.
        assert_eq!(
            menu_key(true, 0, 0, 2, "down", false),
            MenuKey::Highlight(1)
        );
        assert_eq!(
            menu_key(true, 0, 1, 2, "down", false),
            MenuKey::Highlight(1)
        );
        assert_eq!(menu_key(true, 0, 0, 2, "up", false), MenuKey::Highlight(0));
        assert_eq!(menu_key(true, 0, 1, 2, "enter", false), MenuKey::Choose(1));
        assert_eq!(menu_key(true, 0, 1, 2, "space", false), MenuKey::Choose(1));
        assert_eq!(menu_key(true, 0, 1, 2, "escape", false), MenuKey::Close);
        assert_eq!(menu_key(true, 0, 1, 2, "x", false), MenuKey::Ignore);
        assert_eq!(menu_key(false, 0, 0, 2, "down", true), MenuKey::Ignore);
        assert_eq!(menu_key(true, 0, 1, 2, "enter", true), MenuKey::Ignore);
    }
}
