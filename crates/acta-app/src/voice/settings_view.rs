//! Settings > Voice (components-voice.md 5; components-app.md 5): the
//! engine, the model list with its downloads and deletions, the graphics
//! card where it applies, the language, the microphone with its level
//! bar, and the shortcut. Everything applies at once.

use crate::zoom::px;
use acta_core::voice::engine::Lang;
use acta_core::voice::hardware::Reason;
use acta_core::voice::models::ModelId;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Styled, Window, actions, div,
};

use super::view::{self, RowState};
use super::{Dictation, EngineChoice};
use crate::i18n::{Arg, Key, Locale, percent, text, tr};
use crate::icons::{Icon, icon};
use crate::menu::{Align, AnchorBounds};
use crate::settings;
use crate::settings_page::{MenuKey, menu_key, setting_row};
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;

actions!(
    acta_voice,
    [
        /// "Delete model" was chosen: the shell asks first (6.4).
        ConfirmDeleteModel
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    Engine,
    Language,
    Microphone,
    Model(ModelId),
}

pub struct VoiceSettings {
    d: Entity<Dictation>,
    open: Option<Menu>,
    highlighted: usize,
    engine_focus: FocusHandle,
    language_focus: FocusHandle,
    microphone_focus: FocusHandle,
    anchors: [AnchorBounds; 3],
    model_anchor: AnchorBounds,
    /// The pointer is over the Microphone row (its level bar runs).
    mic_hovered: bool,
}

/// Debug captures: each model's row state and the free space, forced
/// (none in a release build).
#[cfg(not(debug_assertions))]
type ForcedRows = Option<(fn(ModelId) -> RowState, u64)>;

const LANGUAGES: [Lang; 3] = [Lang::Detect, Lang::English, Lang::French];

fn language_key(l: Lang) -> Key {
    match l {
        Lang::Detect => Key::VoiceLanguageAuto,
        Lang::English => Key::VoiceLanguageEn,
        Lang::French => Key::VoiceLanguageFr,
    }
}

/// The Dictation dropdown: the macOS recogniser only where it works on the
/// device (or while it is the choice, so the dropdown shows what is set).
fn engines(d: &Dictation, chosen: Option<EngineChoice>) -> Vec<EngineChoice> {
    let mac = d.system.any() || chosen == Some(EngineChoice::MacOs);
    [
        EngineChoice::Whisper,
        EngineChoice::MacOs,
        EngineChoice::Off,
    ]
    .into_iter()
    .filter(|e| *e != EngineChoice::MacOs || mac)
    .collect()
}

fn engine_key(e: EngineChoice) -> Key {
    match e {
        EngineChoice::Whisper => Key::VoiceEngineWhisper,
        EngineChoice::MacOs => Key::VoiceEngineMacos,
        EngineChoice::Off => Key::VoiceEngineOff,
    }
}

impl VoiceSettings {
    pub fn new(d: Entity<Dictation>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let engine_focus = cx.focus_handle();
        let language_focus = cx.focus_handle();
        let microphone_focus = cx.focus_handle();
        for focus in [&engine_focus, &language_focus, &microphone_focus] {
            cx.on_blur(focus, window, |s: &mut Self, _, cx| {
                if !matches!(s.open, Some(Menu::Model(_)) | None) {
                    s.open = None;
                    cx.notify();
                }
            })
            .detach();
        }
        VoiceSettings {
            d,
            open: None,
            highlighted: 0,
            engine_focus,
            language_focus,
            microphone_focus,
            anchors: Default::default(),
            model_anchor: AnchorBounds::default(),
            mic_hovered: false,
        }
    }

    /// The items of a menu and the selected one.
    fn items(&self, menu: Menu, locale: Locale, cx: &App) -> (Vec<String>, usize) {
        let prefs = settings::current(cx).voice;
        match menu {
            Menu::Engine => {
                let list = engines(self.d.read(cx), prefs.engine);
                let chosen = prefs.engine.unwrap_or(EngineChoice::Whisper);
                (
                    list.iter().map(|e| text(locale, engine_key(*e))).collect(),
                    list.iter().position(|e| *e == chosen).unwrap_or(0),
                )
            }
            Menu::Language => (
                LANGUAGES
                    .iter()
                    .map(|l| text(locale, language_key(*l)))
                    .collect(),
                LANGUAGES
                    .iter()
                    .position(|l| *l == prefs.language)
                    .unwrap_or(0),
            ),
            Menu::Microphone => {
                let d = self.d.read(cx);
                let devices = d.devices.clone().unwrap_or_default();
                let mut items = vec![text(locale, Key::VoiceSettingsMicrophoneDefault)];
                items.extend(devices.iter().map(|dev| dev.name.clone()));
                let selected = d
                    .microphone
                    .as_ref()
                    .and_then(|id| devices.iter().position(|dev| &dev.id == id))
                    .map_or(0, |i| i + 1);
                (items, selected)
            }
            Menu::Model(_) => (vec![text(locale, Key::VoiceModelDelete)], 0),
        }
    }

    fn choose(&mut self, menu: Menu, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        let d = self.d.clone();
        match menu {
            Menu::Engine => {
                let chosen = settings::current(cx).voice.engine;
                if let Some(e) = engines(d.read(cx), chosen).get(index).copied() {
                    d.update(cx, |d, cx| d.set_engine(e, cx));
                }
            }
            Menu::Language => {
                if let Some(l) = LANGUAGES.get(index) {
                    d.update(cx, |d, cx| d.set_language(*l, cx));
                }
            }
            Menu::Microphone => {
                let id = if index == 0 {
                    None
                } else {
                    d.read(cx)
                        .devices
                        .as_ref()
                        .and_then(|list| list.get(index - 1))
                        .map(|dev| dev.id.clone())
                };
                d.update(cx, |d, cx| d.set_microphone(id, cx));
            }
            Menu::Model(model) => {
                d.update(cx, |d, _| d.to_delete = Some(model));
                window.dispatch_action(Box::new(ConfirmDeleteModel), cx);
            }
        }
        cx.notify();
    }

    /// A dropdown (components.md 5): a secondary button showing the value,
    /// opening a menu under it; keys as the language dropdown's.
    fn dropdown(
        &self,
        menu: Menu,
        slot: usize,
        focus: &FocusHandle,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Div {
        let (items, selected) = self.items(menu, locale, cx);
        let count = items.len();
        let open = self.open == Some(menu);
        let hover = t.fill_hover;
        let label = items.get(selected).cloned().unwrap_or_default();
        let id = match menu {
            Menu::Engine => "voice-engine",
            Menu::Language => "voice-language",
            _ => "voice-microphone",
        };
        let button = type_scale::UI
            .el()
            .id(id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .max_w(px(280.))
            .px(px(12.))
            .rounded(px(RADIUS_SM))
            .border(px(1.))
            .border_color(t.border_strong)
            .text_color(t.text_primary)
            .hover(move |style| style.bg(hover))
            .on_click(cx.listener(move |s, _, _, cx| {
                s.open = if s.open == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                s.highlighted = selected;
                cx.notify();
            }))
            .relative()
            .child(div().min_w_0().truncate().child(label))
            .child(icon(Icon::ChevronDown, 16., t.text_secondary))
            .child(self.anchors[slot].measure());
        let control = ui::focus_ring(id, focus, RADIUS_SM, t)
            .on_key_down(cx.listener(move |s, event: &KeyDownEvent, window, cx| {
                let key = menu_key(
                    s.open == Some(menu),
                    selected,
                    s.highlighted,
                    count,
                    &event.keystroke.key,
                    event.keystroke.modifiers.modified(),
                );
                match key {
                    MenuKey::Open(i) => {
                        s.open = Some(menu);
                        s.highlighted = i;
                    }
                    MenuKey::Highlight(i) => s.highlighted = i,
                    MenuKey::Choose(i) => s.choose(menu, i, window, cx),
                    MenuKey::Close => s.open = None,
                    MenuKey::Ignore => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(button);
        let layer = open.then(|| self.menu_panel(menu, items, selected, t, cx));
        let layer = layer.and_then(|panel| self.anchors[slot].below(panel, Align::End));
        div().child(control).children(layer)
    }

    fn menu_panel(
        &self,
        menu: Menu,
        items: Vec<String>,
        selected: usize,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let highlighted = self.highlighted;
        let rows = items.into_iter().enumerate().map(|(i, label)| {
            let trailing =
                (i == selected && !matches!(menu, Menu::Model(_))).then(|| crate::menu::check(t));
            crate::menu::item(
                ("voice-menu-item", i),
                None,
                label,
                trailing,
                i == highlighted,
                t,
            )
            .on_click(cx.listener(move |s, _, window, cx| s.choose(menu, i, window, cx)))
        });
        crate::menu::panel("voice-menu", t)
            .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                s.open = None;
                cx.notify();
            }))
            .children(rows)
            .into_any_element()
    }

    fn model_row(
        &self,
        model: ModelId,
        state: RowState,
        recommended: bool,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Div {
        let size = view::size(locale, model.file().size);
        let hint = tr(
            locale,
            view::model_hint(model),
            &[("size", Arg::Text(&size))],
        );
        let name = div()
            .flex()
            .flex_row()
            .items_baseline()
            .gap(px(8.))
            .flex_shrink_0()
            .child(
                type_scale::UI
                    .el()
                    .text_color(t.text_primary)
                    .child(view::model_name(model)),
            )
            .when(recommended, |this| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_secondary)
                        .child(text(locale, Key::VoiceModelRecommended)),
                )
            });
        let d = self.d.clone();
        let ghost = |id: &'static str, key: Key| {
            ui::button(
                (id, model as usize),
                text(locale, key),
                ui::Variant::Ghost,
                false,
                t,
            )
        };
        let menu_button = |s: &Self, cx: &mut Context<Self>| {
            let open = s.open == Some(Menu::Model(model));
            div()
                .relative()
                .child(
                    ui::ghost_icon_button(
                        format!("voice-model-menu-{}", model.key()),
                        Icon::Ellipsis,
                        28.,
                        open,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.open = if s.open == Some(Menu::Model(model)) {
                            None
                        } else {
                            Some(Menu::Model(model))
                        };
                        s.highlighted = 0;
                        cx.notify();
                    }))
                    .child(s.model_anchor.measure_keyed(model as u64)),
                )
                .children(
                    open.then(|| {
                        let panel = s.menu_panel(
                            Menu::Model(model),
                            vec![text(locale, Key::VoiceModelDelete)],
                            usize::MAX,
                            t,
                            cx,
                        );
                        s.model_anchor
                            .below_keyed(model as u64, panel, Align::End, 1)
                    })
                    .flatten(),
                )
        };
        let right: AnyElement = match state {
            RowState::NotDownloaded => ghost("voice-model-download", Key::VoiceModelDownload)
                .on_click(move |_, _, cx| d.update(cx, |d, cx| d.start_download(model, false, cx)))
                .into_any_element(),
            RowState::Downloading(p) => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(80.))
                        .h(px(4.))
                        .rounded(px(crate::theme::RADIUS_XS))
                        .bg(t.fill_selected)
                        .child(
                            div()
                                .h_full()
                                .w(px(80. * f32::from(p) / 100.))
                                .bg(t.text_secondary),
                        ),
                )
                .child(
                    type_scale::SMALL
                        .el()
                        .w(px(40.))
                        .font_features(type_scale::tabular())
                        .text_color(t.text_secondary)
                        .child(percent(locale, u64::from(p))),
                )
                .child(
                    ghost("voice-model-cancel", Key::CommonCancel)
                        .on_click(move |_, _, cx| d.update(cx, |d, _| d.cancel_download())),
                )
                .into_any_element(),
            RowState::InUse => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(icon(Icon::Check, 14., t.success))
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_secondary)
                        .child(text(locale, Key::VoiceModelInUse)),
                )
                .child(menu_button(self, cx))
                .into_any_element(),
            RowState::Downloaded => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(
                    ghost("voice-model-use", Key::VoiceModelUse)
                        .on_click(move |_, _, cx| d.update(cx, |d, cx| d.use_model(model, cx))),
                )
                .child(menu_button(self, cx))
                .into_any_element(),
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .h(px(44.))
            .pl(px(16.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(px(16.))
                    .min_w_0()
                    .child(name)
                    .child(
                        type_scale::SMALL
                            .el()
                            .min_w_0()
                            .truncate()
                            .text_color(t.text_tertiary)
                            .child(hint),
                    ),
            )
            .child(div().flex_shrink_0().child(right))
    }
}

impl Render for VoiceSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let prefs = settings::current(cx).voice;
        let d_entity = self.d.clone();
        d_entity.update(cx, |d, cx| {
            d.detect(cx);
            d.refresh_models(false, cx);
            d.refresh_devices(cx);
        });
        let d = self.d.read(cx);
        let rec = d.recommendation();
        let whisper = !matches!(prefs.engine, Some(EngineChoice::Off | EngineChoice::MacOs));
        let downloading = d.download_progress().map(|(m, p)| {
            (
                m,
                p.map_or(100, |(done, total)| view::percent_of(done, total)),
            )
        });
        #[cfg(debug_assertions)]
        let rows_override = d.dev.rows();
        #[cfg(not(debug_assertions))]
        let rows_override: ForcedRows = None;
        let free = rows_override.map(|(_, f)| f).or(d.free);
        let models_size = if rows_override.is_some() {
            ModelId::Small.file().size
        } else {
            d.models_size()
        };
        let states: Vec<(ModelId, RowState)> = ModelId::ALL
            .into_iter()
            .map(|m| {
                let s = match rows_override {
                    Some((f, _)) => f(m),
                    None => view::row_state(m, d.on_disk(m), prefs.model, downloading),
                };
                (m, s)
            })
            .collect();
        let gpu_row = d.machine.is_some_and(|m| m.gpu());
        let refusal = d.refusal;
        let level = d.level();
        let recommended = rec.and_then(|r| r.model);
        let gpu_tier = rec.is_some_and(|r| r.reason == Reason::Gpu);

        let engine = self.dropdown(Menu::Engine, 0, &self.engine_focus.clone(), locale, &t, cx);
        let mut out: Vec<AnyElement> = vec![
            type_scale::SUBTITLE
                .el()
                .id("settings-voice")
                .mt(px(32.))
                .mb(px(8.))
                .text_color(t.text_primary)
                .child(text(locale, Key::VoiceSettingsTitle))
                .into_any_element(),
            setting_row(
                text(locale, Key::VoiceSettingsEngine),
                text(locale, Key::VoiceSettingsEngineHint),
                engine,
                &t,
            )
            .into_any_element(),
        ];
        if whisper {
            let hint = if gpu_tier {
                text(locale, Key::VoiceSettingsModelHintGpu)
            } else {
                let name = recommended.map(view::model_name).unwrap_or_default();
                tr(
                    locale,
                    Key::VoiceSettingsModelHint,
                    &[("model", Arg::Text(name))],
                )
            };
            out.push(
                setting_row(
                    text(locale, Key::VoiceSettingsModel),
                    if recommended.is_some() {
                        hint
                    } else {
                        String::new()
                    },
                    div(),
                    &t,
                )
                .into_any_element(),
            );
            for (m, s) in states {
                out.push(
                    self.model_row(m, s, recommended == Some(m), locale, &t, cx)
                        .into_any_element(),
                );
            }
            // Where the models are, the free space, what they take.
            let dir = Dictation::models_dir();
            let path = dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let free_text = free.map(|f| view::size(locale, f)).unwrap_or_default();
            // The path in `type.code` inside the sentence: the sentence is
            // cut where the path goes.
            const MARK: &str = "\u{1}";
            let sentence = tr(
                locale,
                Key::VoiceModelsWhere,
                &[("path", Arg::Text(MARK)), ("free", Arg::Text(&free_text))],
            );
            let (before, after) = sentence.split_once(MARK).unwrap_or((&sentence, ""));
            let where_text = div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .max_w(px(480.))
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(before.trim_end().to_owned() + " "),
                )
                .child(
                    // Middle-truncated by folder, in full in the tooltip.
                    type_scale::CODE_SMALL
                        .el()
                        .id("voice-models-path")
                        .text_color(t.text_secondary)
                        .tooltip(ui::tip(path.clone()))
                        .child(crate::paths_view::fit(
                            &path,
                            crate::paths_view::budget(300.),
                        )),
                )
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(after.to_owned()),
                );
            let reveal_key = if cfg!(windows) {
                Key::FoldersShowW
            } else if cfg!(target_os = "macos") {
                Key::FoldersShowM
            } else {
                Key::FoldersShowL
            };
            let used = (models_size > 0).then(|| {
                tr(
                    locale,
                    Key::VoiceModelsUsed,
                    &[("size", Arg::Text(&view::size(locale, models_size)))],
                )
            });
            out.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(24.))
                    .py(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(where_text)
                            .children(used.map(|u| {
                                type_scale::SMALL.el().text_color(t.text_tertiary).child(u)
                            }))
                            .children(refusal.map(|k| {
                                type_scale::SMALL
                                    .el()
                                    .text_color(t.text_primary)
                                    .child(text(locale, k))
                            })),
                    )
                    .child(
                        ui::button(
                            "voice-models-show",
                            text(locale, reveal_key),
                            ui::Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(move |_, _, cx| {
                            if let Some(dir) = &dir {
                                let _ = std::fs::create_dir_all(dir);
                                cx.reveal_path(dir);
                            }
                        }),
                    )
                    .into_any_element(),
            );
            if gpu_row {
                let d = self.d.clone();
                let on = prefs.gpu;
                out.push(
                    setting_row(
                        text(locale, Key::VoiceSettingsGpu),
                        text(locale, Key::VoiceSettingsGpuHint),
                        crate::settings_page::checkbox("voice-gpu", on, &t)
                            .on_click(move |_, _, cx| d.update(cx, |d, cx| d.set_gpu(!on, cx))),
                        &t,
                    )
                    .into_any_element(),
                );
            }
            let language = self.dropdown(
                Menu::Language,
                1,
                &self.language_focus.clone(),
                locale,
                &t,
                cx,
            );
            out.push(
                setting_row(
                    text(locale, Key::VoiceSettingsLanguage),
                    text(locale, Key::VoiceSettingsLanguageHint),
                    language,
                    &t,
                )
                .into_any_element(),
            );
            let microphone = self.dropdown(
                Menu::Microphone,
                2,
                &self.microphone_focus.clone(),
                locale,
                &t,
                cx,
            );
            // The level bar opens the microphone only while the person is
            // at this row: the pointer over it, or its dropdown focused or
            // open. Settings being open is not enough (privacy: the
            // system's microphone sign and permission prompt).
            let at_row = self.mic_hovered
                || self.microphone_focus.contains_focused(window, cx)
                || self.open == Some(Menu::Microphone);
            if at_row {
                window.defer_preview(self.d.clone());
            }
            // The row's description is the button's level bar, so the
            // right device can be checked by speaking.
            out.push(
                div()
                    .id("voice-microphone-row")
                    .on_hover(cx.listener(|s, hovered: &bool, _, cx| {
                        s.mic_hovered = *hovered;
                        cx.notify();
                    }))
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
                            .gap(px(6.))
                            .child(
                                type_scale::UI
                                    .el()
                                    .text_color(t.text_primary)
                                    .child(text(locale, Key::VoiceSettingsMicrophone)),
                            )
                            .child(super::ui::level_bar(
                                acta_core::voice::level::bar_fraction(level),
                                &t,
                            )),
                    )
                    .child(div().flex_shrink_0().child(microphone))
                    .into_any_element(),
            );
            let keys = super::ui::shortcut_keys(locale);
            out.push(
                setting_row(
                    text(locale, Key::VoiceSettingsShortcut),
                    text(locale, Key::VoiceSettingsShortcutHint),
                    ui::kbd(&keys, &t),
                    &t,
                )
                .into_any_element(),
            );
        }
        let _ = window;
        div().flex().flex_col().children(out)
    }
}

/// Starting the Settings level bar from a measure callback, which runs
/// while the frame is drawn: deferred to after it.
trait DeferPreview {
    fn defer_preview(&mut self, d: Entity<Dictation>);
}

impl DeferPreview for Window {
    fn defer_preview(&mut self, d: Entity<Dictation>) {
        self.on_next_frame(move |_, cx| d.update(cx, |d, cx| d.preview_shown(cx)));
    }
}
