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

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Div, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Task, Window,
    anchored, deferred, div, point, px,
};

use crate::first_run::COLUMN_MAX;
use crate::i18n::{Key, Locale, text};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_MD, RADIUS_SM, ThemeSetting, Tokens};
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
    if modified {
        return None;
    }
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
/// Keys pressed with a modifier are not the menu's.
fn menu_key(
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
    language_focus: FocusHandle,
    language_menu_open: bool,
    language_highlighted: usize,
    _language_blur: Subscription,
    ws: Option<Entity<Workspace>>,
    facts: Option<DataFacts>,
    seen_revision: u64,
    _load: Task<()>,
    _ws_sub: Option<Subscription>,
    /// The `acta` command, found off the UI thread.
    cli: Option<std::path::PathBuf>,
    _cli: Task<()>,
}

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
        Self {
            theme_focus: cx.focus_handle(),
            language_focus,
            language_menu_open: false,
            language_highlighted: 0,
            _language_blur: language_blur,
            ws: None,
            facts: None,
            seen_revision: 0,
            _load: Task::ready(()),
            _ws_sub: None,
            cli: None,
            _cli: cli_task,
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

use crate::ui::focus_ring;

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
                    .border_1()
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
        let text_size = self.text_size_control(&t, cx);
        let claude = self.claude_section(locale, &t, cx);
        let data = self.data_sections(locale, &t, cx);
        let about = about_section(locale, &t);

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
                    ))
                    .child(setting_row(
                        text(locale, Key::SettingsTerminalSize),
                        String::new(),
                        text_size,
                        &t,
                    ))
                    .children(claude)
                    .children(data)
                    .children(about),
            )
    }
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
    /// Folders not found: id, path, owner name.
    pub missing: Vec<(acta_core::model::FolderId, String, String)>,
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
        missing,
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

    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = &self.ws else { return };
        let task = ws.read(cx).db.run(cx, |store| load_facts(store));
        self._load = cx.spawn(async move |page, cx| {
            if let Ok(mut facts) = task.await {
                let all = std::mem::take(&mut facts.missing);
                facts.missing = cx
                    .background_executor()
                    .spawn(async move {
                        all.into_iter()
                            .filter(|(_, path, _)| !std::path::Path::new(path).is_dir())
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
                    text(locale, Key::CmdSnapshot),
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
                    text(locale, Key::CmdTrash),
                    Box::new(crate::shell::OpenTrash),
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
                        ),
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
                text(locale, Key::ChatSignout),
                text(locale, Key::ChatSignoutBody),
                action(
                    "settings-chat-signout",
                    text(locale, Key::ChatSignoutConfirm),
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

impl SettingsPage {
    /// Claude Code (components-app.md 5, Settings): the four checks of
    /// `acta setup --check`, then Set up (primary while anything is
    /// missing, then a secondary "Run setup again") and Remove.
    fn claude_section(
        &self,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Vec<gpui_kit::AnyElement> {
        let state = crate::claude_setup::state(cx);
        let check = state.and_then(|s| s.check).unwrap_or_default();
        let checked = state.is_some_and(|s| s.checked);
        let busy = state.is_some_and(|s| s.busy);
        let mut out = vec![
            type_scale::SUBTITLE
                .el()
                .mt(px(32.))
                .mb(px(8.))
                .text_color(t.text_primary)
                .child(text(locale, Key::SettingsClaude))
                .into_any_element(),
        ];
        let rows = [
            (Key::SettingsCheckCli, check.claude),
            (Key::SettingsCheckFiles, check.files),
            (Key::SettingsCheckPlugin, check.plugin),
            (Key::SettingsCheckRule, check.rule),
        ];
        for (key, done) in rows {
            // Until the first check answers, nothing is claimed either way.
            let (glyph, color, word) = if done {
                (Icon::Check, t.success, Some(Key::SettingsCheckDone))
            } else if checked {
                (
                    Icon::CircleDashed,
                    t.text_tertiary,
                    Some(Key::SettingsCheckMissing),
                )
            } else {
                (Icon::CircleDashed, t.text_tertiary, None)
            };
            out.push(
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
                    }))
                    .into_any_element(),
            );
        }
        let complete = check.complete();
        let (setup_key, setup_variant) = if complete {
            (Key::SettingsSetupAgain, ui::Variant::Secondary)
        } else {
            (Key::SettingsSetup, ui::Variant::Primary)
        };
        out.push(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .mt(px(8.))
                .child(
                    ui::button(
                        "settings-setup",
                        text(locale, setup_key),
                        setup_variant,
                        busy,
                        t,
                    )
                    .when(!busy, |this| {
                        this.on_click(|_, _, cx| crate::claude_setup::run_install(cx))
                    }),
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
                        this.on_click(|_, _, cx| crate::claude_setup::run_remove(cx))
                    }),
                )
                .into_any_element(),
        );
        out.push(
            type_scale::SMALL
                .el()
                .mt(px(8.))
                .max_w(px(420.))
                .text_color(t.text_tertiary)
                .child(text(locale, Key::SettingsSetupHint))
                .into_any_element(),
        );
        if let Some(path) = &self.cli {
            let full = path.to_string_lossy().into_owned();
            let copy = full.clone();
            out.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .mt(px(8.))
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
                                crate::paths_view::budget(420.),
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
                            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(copy.clone()))
                        }),
                    )
                    .into_any_element(),
            );
        }
        out
    }
}

/// The Acta repository's releases: "Release notes".
const RELEASES: &str = "https://github.com/Alegzandr/acta/releases";

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
            text(locale, Key::SettingsLicences),
            text(locale, Key::SettingsLicencesHint),
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
