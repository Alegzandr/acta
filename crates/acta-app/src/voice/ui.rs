//! The dictation elements of an editor (components-voice.md 5): the
//! microphone button with what sits next to it in the footer, the level
//! bar, and the line under the footer. Each editor adds [`mic`] to its
//! footer and [`line`] under it; both read the shared [`super::Dictation`].

use std::time::Instant;

use gpui_kit::component::input::TextareaState;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Div, Entity, InteractiveElement as _, IntoElement as _, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::Mic;
use super::view::{self, Look, Status, Tip, Tone};
use crate::i18n::{Arg, Key, KeyName, Locale, percent, text, tr};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_SM, RADIUS_XS, Tokens};
use crate::type_scale;

/// The shortcut as the kbd hint names it on this platform.
pub fn shortcut_keys(locale: Locale) -> Vec<&'static str> {
    let macos = cfg!(target_os = "macos");
    vec![
        KeyName::Mod.label(locale, macos),
        KeyName::Shift.label(locale, macos),
        "Space",
    ]
}

/// The shortcut in a sentence ("Ctrl+Shift+Space").
pub fn shortcut_text(locale: Locale) -> String {
    shortcut_keys(locale).join("+")
}

/// The level bar: 32 x 4, `radius.xs`, track `fill.selected`, fill
/// `text.secondary` from the left. Follows the data; never eased.
pub fn level_bar(fraction: f32, t: &Tokens) -> Div {
    let width = 32.;
    div()
        .flex_shrink_0()
        .w(px(width))
        .h(px(4.))
        .rounded(px(RADIUS_XS))
        .bg(t.fill_selected)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(px(width * fraction.clamp(0., 1.)))
                .bg(t.text_secondary),
        )
}

/// Something sits next to the microphone ("Starting...", the level bar,
/// "Writing text..."): the footer's save hint yields its place to it
/// (components-voice.md 5), so the primary button never moves.
pub fn listening(m: Mic, cx: &App) -> bool {
    super::get(cx).is_some_and(|d| {
        view::mic(d.read(cx).phase(m.0, cx), Instant::now())
            .is_some_and(|v| v.status != Status::None)
    })
}

/// [`mic`] for an editor joined with [`super::attach`].
pub fn mic_for(state: &Entity<TextareaState>, cx: &App) -> Option<AnyElement> {
    mic(super::mic_of(state, cx)?, cx)
}

/// [`line`] for an editor joined with [`super::attach`].
pub fn line_for(state: &Entity<TextareaState>, cx: &App) -> Option<AnyElement> {
    line(super::mic_of(state, cx)?, cx)
}

/// The line inside a 44 bar (the document editor's), level with it.
pub fn line_in_bar_for(state: &Entity<TextareaState>, cx: &App) -> Option<AnyElement> {
    line_el(super::mic_of(state, cx)?, false, cx)
}

/// [`listening`] for an editor joined with [`super::attach`].
pub fn listening_for(state: &Entity<TextareaState>, cx: &App) -> bool {
    super::mic_of(state, cx).is_some_and(|m| listening(m, cx))
}

/// The microphone button and its status, for the editor's footer;
/// nothing when dictation is off.
pub fn mic(m: Mic, cx: &App) -> Option<AnyElement> {
    let d = super::get(cx)?;
    let phase = d.read(cx).phase(m.0, cx);
    let v = view::mic(phase, Instant::now())?;
    let t = cx.global::<Tokens>().clone();
    let locale = settings::locale(cx);
    let editor = m.0;
    let id = ("voice-mic", editor as usize);
    let (glyph, color) = match v.look {
        Look::Mic => (Icon::Mic, t.text_secondary),
        Look::Stop => (Icon::Square, t.text_primary),
        Look::Busy => (Icon::Mic, t.text_disabled),
    };
    let group: gpui_kit::SharedString = format!("voice-mic-{editor}").into();
    let primary = t.text_primary;
    let glyph = icon(glyph, 16., color).when(v.look == Look::Mic, |this| {
        this.group_hover(group.clone(), move |s| s.text_color(primary))
    });
    let mut button = div()
        .id(id)
        .group(group)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded(px(RADIUS_SM))
        .when(v.look == Look::Stop, |this| this.bg(t.fill_selected))
        .child(glyph);
    if v.look != Look::Busy {
        let (hover, pressed) = (t.fill_hover, t.fill_pressed);
        let down = d.clone();
        let up = d.clone();
        let out = d.clone();
        button = button
            .when(v.look == Look::Mic, |this| this.hover(move |s| s.bg(hover)))
            .active(move |s| s.bg(pressed))
            // The editor keeps the focus: a click never moves it here.
            .capture_any_mouse_down(|_, window, _| window.prevent_default())
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                down.update(cx, |d, cx| d.mouse_down(editor, window, cx));
                cx.stop_propagation();
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                up.update(cx, |d, cx| d.mouse_up(cx));
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                out.update(cx, |d, cx| d.mouse_up(cx));
            });
    }
    if let Some(tip) = v.tip {
        button = match tip {
            Tip::Dictate => button.tooltip(crate::ui::tip_keys(
                text(locale, Key::VoiceButton),
                shortcut_keys(locale),
            )),
            Tip::Setup => button.tooltip(crate::ui::tip(text(locale, Key::VoiceButtonSetup))),
            Tip::Downloading(p) => button.tooltip(crate::ui::tip(tr(
                locale,
                Key::VoiceButtonDownloading,
                &[("percent", Arg::Text(&percent(locale, u64::from(p))))],
            ))),
            Tip::Stop => button.tooltip(crate::ui::tip_keys(
                text(locale, Key::VoiceStop),
                shortcut_keys(locale),
            )),
        };
    }
    let small = |key: Key, color| {
        type_scale::SMALL
            .el()
            .flex_shrink_0()
            .text_color(color)
            .child(text(locale, key))
    };
    let status: Option<AnyElement> = match v.status {
        Status::None => None,
        Status::Starting => Some(small(Key::VoiceStarting, t.text_tertiary).into_any_element()),
        Status::Writing => Some(small(Key::VoiceWriting, t.text_tertiary).into_any_element()),
        Status::Listening { fraction, elapsed } => Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(level_bar(fraction, &t))
                .child(small(Key::VoiceListening, t.text_secondary))
                .child(
                    type_scale::SMALL
                        .el()
                        .flex_shrink_0()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(view::elapsed(elapsed)),
                )
                .into_any_element(),
        ),
    };
    Some(
        div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .child(button)
            .children(status)
            .into_any_element(),
    )
}

/// The line under the footer (components-voice.md 5, Errors): 4 below,
/// `type.small`, at most one ghost action. Nothing when there is none.
pub fn line(m: Mic, cx: &App) -> Option<AnyElement> {
    line_el(m, true, cx)
}

fn line_el(m: Mic, under: bool, cx: &App) -> Option<AnyElement> {
    let d = super::get(cx)?;
    let l = d.read(cx).line(m.0)?;
    let t = cx.global::<Tokens>().clone();
    let locale = settings::locale(cx);
    let (key, tone, action) = view::line(l, cfg!(target_os = "linux"));
    let (glyph, color) = match tone {
        Tone::Error => (Some((Icon::TriangleAlert, t.danger)), t.text_primary),
        Tone::Info => (Some((Icon::Info, t.text_tertiary)), t.text_tertiary),
        Tone::Quiet => (None, t.text_tertiary),
    };
    let editor = m.0;
    let action = action.map(|a| {
        let d = d.clone();
        crate::ui::button(
            ("voice-line-action", editor as usize),
            text(locale, a.key(cfg!(target_os = "macos"))),
            crate::ui::Variant::Ghost,
            false,
            &t,
        )
        .on_click(move |_, window, cx| run_action(a, editor, &d, window, cx))
    });
    Some(
        div()
            .when(under, |this| this.mt(px(4.)))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .children(glyph.map(|(g, c)| icon(g, 14., c)))
            .child(
                type_scale::SMALL
                    .el()
                    .text_color(color)
                    .child(text(locale, key)),
            )
            .children(action)
            .into_any_element(),
    )
}

fn run_action(
    a: view::Action,
    editor: acta_core::voice::take::EditorId,
    d: &gpui_kit::Entity<super::Dictation>,
    window: &mut gpui_kit::Window,
    cx: &mut App,
) {
    match a {
        view::Action::OpenPrivacy => {
            if cfg!(target_os = "macos") {
                cx.open_url(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
                );
            } else {
                cx.open_url("ms-settings:privacy-microphone");
            }
        }
        view::Action::OpenSpeechPrivacy => cx.open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition",
        ),
        view::Action::VoiceSettings => {
            window.dispatch_action(Box::new(super::OpenVoiceSettings), cx)
        }
        view::Action::DownloadAgain => {
            let model = settings::current(cx).voice.model;
            if let Some(model) = model {
                d.update(cx, |d, cx| {
                    d.clear_line(editor);
                    d.start_download(model, true, cx)
                });
            }
        }
        view::Action::TryAgain => d.update(cx, |d, cx| {
            d.clear_line(editor);
            d.press(editor, window, cx)
        }),
    }
}
