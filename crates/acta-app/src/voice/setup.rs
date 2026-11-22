//! The first-use dialog (components-voice.md 5, First use; screens.md
//! 8.27): the recommendation with its size and the free space, then the
//! download's progress in the same dialog, then Ready. Listening never
//! starts by itself at the end.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};

use super::view::{self, Setup};
use crate::dialogs::{Dialog, DialogEvent};
use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_XS, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};

/// What the dialog shows now.
pub fn state(cx: &App) -> Setup {
    let Some(d) = super::get(cx) else {
        return Setup::Detecting;
    };
    let d = d.read(cx);
    #[cfg(debug_assertions)]
    if let Some(s) = d.dev.setup() {
        return s;
    }
    if let Some((model, progress)) = d.download_progress() {
        return match progress {
            Some((done, total)) => Setup::Downloading { model, done, total },
            None => Setup::Checking { model },
        };
    }
    match d.finished {
        Some(super::Finished::Ready(_)) => return Setup::Ready,
        Some(super::Finished::Failed(model, error)) => return Setup::Failed { model, error },
        None => {}
    }
    let chosen = settings::current(cx).voice.model.filter(|m| !d.on_disk(*m));
    let memory = d.machine.map_or(0, |m| m.memory);
    view::offer(d.recommendation(), memory, chosen, d.free)
}

fn whisper_name(model: acta_core::voice::models::ModelId) -> String {
    format!("Whisper {}", view::model_name(model))
}

/// The bar of the updater window: full width, 4 high, never eased.
fn bar(fraction: f32, t: &Tokens) -> gpui_kit::Div {
    div()
        .w_full()
        .h(px(4.))
        .rounded(px(RADIUS_XS))
        .bg(t.fill_selected)
        .child(
            div()
                .h_full()
                .w(gpui_kit::relative(fraction.clamp(0., 1.)))
                .bg(t.text_secondary),
        )
}

/// The dialog's title, body, buttons and width.
pub fn body(
    safe: &FocusHandle,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<Dialog>,
) -> (String, AnyElement, Vec<AnyElement>, f32) {
    // The machine and the disk are looked at once, off the UI thread.
    if let Some(d) = super::get(cx) {
        d.update(cx, |d, cx| {
            d.detect(cx);
            d.refresh_models(false, cx);
        });
    }
    let state = state(cx);
    let title = text(locale, Key::VoiceSetupTitle);
    let small = |s: String, color| type_scale::SMALL.el().text_color(color).child(s);
    let ui_line = |s: String, color| type_scale::UI.el().text_color(color).child(s);
    let close = |id: &'static str, key: Key, cx: &mut Context<Dialog>| {
        ui::button(id, text(locale, key), Variant::Secondary, false, t)
            .track_focus(safe)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(DialogEvent::Close)))
            .into_any_element()
    };
    let more = type_scale::SMALL
        .el()
        .id("voice-setup-more")
        .text_color(t.text_tertiary)
        .cursor_pointer()
        .hover(|s| s.underline())
        .on_click(cx.listener(|_, _, window, cx| {
            cx.emit(DialogEvent::Close);
            window.dispatch_action(Box::new(super::OpenVoiceSettings), cx);
        }))
        .child(text(locale, Key::VoiceSetupMore));
    let intro = ui_line(text(locale, Key::VoiceSetupBody), t.text_secondary);
    let column = || div().flex().flex_col().gap(px(4.));
    match state {
        Setup::Offer {
            model,
            gpu,
            free,
            enough,
        } => {
            let size = view::size(locale, model.file().size);
            let what = tr(
                locale,
                if gpu {
                    Key::VoiceSetupGpu
                } else {
                    Key::VoiceSetupCpu
                },
                &[("size", Arg::Text(&size))],
            );
            let free_line: Option<AnyElement> = free.map(|f| {
                let free_text = view::size(locale, f);
                if enough {
                    small(
                        tr(
                            locale,
                            Key::VoiceSetupFree,
                            &[("free", Arg::Text(&free_text))],
                        ),
                        t.text_tertiary,
                    )
                    .into_any_element()
                } else {
                    // The model's size, as the spec words it (the check
                    // itself also keeps 1 GB free).
                    let needed = view::size(locale, model.file().size);
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .child(icon(Icon::TriangleAlert, 14., t.warning))
                        .child(small(
                            tr(
                                locale,
                                Key::VoiceSetupNoSpace,
                                &[
                                    ("size", Arg::Text(&needed)),
                                    ("free", Arg::Text(&free_text)),
                                ],
                            ),
                            t.warning,
                        ))
                        .into_any_element()
                }
            });
            let body = div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(intro)
                .child(
                    column()
                        .child(
                            type_scale::UI_STRONG
                                .el()
                                .text_color(t.text_primary)
                                .child(whisper_name(model)),
                        )
                        .child(small(what, t.text_secondary))
                        .children(free_line),
                )
                .child(more)
                .into_any_element();
            let download = ui::button(
                "voice-setup-download",
                tr(
                    locale,
                    Key::VoiceSetupDownload,
                    &[("size", Arg::Text(&size))],
                ),
                Variant::Primary,
                !enough,
                t,
            )
            .when(enough, |b| {
                b.on_click(move |_, _, cx| {
                    if let Some(d) = super::get(cx) {
                        d.update(cx, |d, cx| d.start_download(model, true, cx));
                    }
                })
            })
            .into_any_element();
            (
                title,
                body,
                vec![close("dialog-cancel", Key::CommonCancel, cx), download],
                440.,
            )
        }
        Setup::Detecting => (
            title,
            intro.into_any_element(),
            vec![close("dialog-cancel", Key::CommonCancel, cx)],
            440.,
        ),
        Setup::TooLittleMemory { memory } => {
            let memory = view::size(locale, memory);
            let body = div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(intro)
                .child(ui::notice(
                    Some((Icon::Info, t.text_secondary)),
                    tr(
                        locale,
                        Key::VoiceSetupUnavailable,
                        &[("memory", Arg::Text(&memory))],
                    ),
                    t,
                ))
                .into_any_element();
            (
                title,
                body,
                vec![close("dialog-close", Key::CommonClose, cx)],
                440.,
            )
        }
        Setup::UnsupportedCpu => {
            let body = div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(intro)
                .child(ui::notice(
                    Some((Icon::Info, t.text_secondary)),
                    text(locale, Key::VoiceSetupUnsupportedCpu),
                    t,
                ))
                .into_any_element();
            (
                title,
                body,
                vec![close("dialog-close", Key::CommonClose, cx)],
                440.,
            )
        }
        Setup::Downloading { model, done, total } => {
            let fraction = if total == 0 {
                0.
            } else {
                done as f32 / total as f32
            };
            let bytes = tr(
                locale,
                Key::VoiceDownloadBytes,
                &[
                    ("done", Arg::Text(&view::size(locale, done))),
                    ("size", Arg::Text(&view::size(locale, total))),
                ],
            );
            let body = column()
                .gap(px(8.))
                .child(ui_line(
                    tr(
                        locale,
                        Key::VoiceDownloadProgressTitle,
                        &[("model", Arg::Text(view::model_name(model)))],
                    ),
                    t.text_primary,
                ))
                .child(bar(fraction, t))
                .child(
                    type_scale::SMALL
                        .el()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(bytes),
                )
                .into_any_element();
            (title, body, progress_buttons(locale, safe, t, cx), 440.)
        }
        Setup::Checking { model } => {
            let body = column()
                .gap(px(8.))
                .child(ui_line(
                    tr(
                        locale,
                        Key::VoiceDownloadProgressTitle,
                        &[("model", Arg::Text(view::model_name(model)))],
                    ),
                    t.text_primary,
                ))
                .child(bar(1., t))
                .child(small(
                    text(locale, Key::VoiceDownloadChecking),
                    t.text_tertiary,
                ))
                .into_any_element();
            (title, body, progress_buttons(locale, safe, t, cx), 440.)
        }
        Setup::Ready => {
            let body = ui_line(
                tr(
                    locale,
                    Key::VoiceDownloadReady,
                    &[("shortcut", Arg::Text(&super::ui::shortcut_text(locale)))],
                ),
                t.text_secondary,
            )
            .into_any_element();
            (
                title,
                body,
                vec![close("dialog-close", Key::CommonClose, cx)],
                440.,
            )
        }
        Setup::Failed { model, error } => {
            let (key, reason) = view::download_failure(error);
            let line = match key {
                Key::VoiceSetupNoSpace => {
                    let (needed, free) = match error {
                        acta_core::voice::download::DownloadError::NoSpace { needed, free } => {
                            (needed, free)
                        }
                        _ => (0, 0),
                    };
                    tr(
                        locale,
                        key,
                        &[
                            ("size", Arg::Text(&view::size(locale, needed))),
                            ("free", Arg::Text(&view::size(locale, free))),
                        ],
                    )
                }
                _ => {
                    let reason = reason.map(|k| text(locale, k)).unwrap_or_default();
                    tr(locale, key, &[("reason", Arg::Text(&reason))])
                        .trim_end()
                        .to_owned()
                }
            };
            let body =
                ui::notice(Some((Icon::TriangleAlert, t.danger)), line, t).into_any_element();
            let retry = ui::button(
                "voice-setup-retry",
                text(locale, Key::CommonRetry),
                Variant::Primary,
                false,
                t,
            )
            .on_click(move |_, _, cx| {
                if let Some(d) = super::get(cx) {
                    d.update(cx, |d, cx| d.start_download(model, true, cx));
                }
            })
            .into_any_element();
            (
                title,
                body,
                vec![close("dialog-close", Key::CommonClose, cx), retry],
                440.,
            )
        }
    }
}

/// "Cancel download" and "Hide" (the download goes on; its progress is
/// in Settings > Voice and the microphone's tooltip).
fn progress_buttons(
    locale: Locale,
    safe: &FocusHandle,
    t: &Tokens,
    cx: &mut Context<Dialog>,
) -> Vec<AnyElement> {
    vec![
        ui::button(
            "voice-download-cancel",
            text(locale, Key::VoiceDownloadCancel),
            Variant::Secondary,
            false,
            t,
        )
        .on_click(|_, _, cx| {
            if let Some(d) = super::get(cx) {
                d.update(cx, |d, _| d.cancel_download());
            }
        })
        .into_any_element(),
        ui::button(
            "voice-download-hide",
            text(locale, Key::VoiceDownloadHide),
            Variant::Secondary,
            false,
            t,
        )
        .track_focus(safe)
        .on_click(cx.listener(|_, _, _, cx| cx.emit(DialogEvent::Close)))
        .into_any_element(),
    ]
}
