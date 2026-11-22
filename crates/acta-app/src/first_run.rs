//! First run, docs/design/screens.md 8.5: no sidebar until a project exists;
//! "Choose a folder" creates the project named after the folder, attaches
//! the folder as its primary working folder and opens its inbox. The three
//! "Then" steps are plain text rows, not a checklist. While Claude Code
//! is not ready, the Ready for Claude Code block heads the column (ADR
//! 0013; components-app.md 5).

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::i18n::{Key, KeyName, text};
use crate::settings;
use crate::theme::Tokens;
use crate::type_scale;
use crate::ui::{self, Variant};

pub enum FirstRunEvent {
    ChooseFolder,
    /// A button of the Ready for Claude Code block.
    Ready(crate::ready::Request),
}

impl From<crate::ready::Request> for FirstRunEvent {
    fn from(request: crate::ready::Request) -> Self {
        FirstRunEvent::Ready(request)
    }
}

pub struct FirstRun;

impl EventEmitter<FirstRunEvent> for FirstRun {}

impl Render for FirstRun {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let m = KeyName::Mod.label(locale, macos);
        let ctrl = KeyName::Ctrl.label(locale, macos);
        let ready = crate::ready::first_run_shows(cx);
        let block = if ready {
            crate::ready::block::<Self, FirstRunEvent>(true, locale, &t, cx)
        } else {
            Vec::new()
        };
        // The step labels' column is as wide as the longest label in this
        // language, so the hints line up without a fixed width.
        let labels = [Key::FirstrunStep1, Key::FirstrunStep2, Key::FirstrunStep3]
            .map(|key| text(locale, key));
        let label_width =
            ui::label_width(&labels.iter().map(String::as_str).collect::<Vec<_>>(), cx);
        let step = |n: &'static str, key: Key, hint: gpui_kit::AnyElement| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .h(px(24.))
                .child(
                    type_scale::UI
                        .el()
                        .w(px(12.))
                        .text_color(t.text_tertiary)
                        .child(n),
                )
                .child(
                    type_scale::UI
                        .el()
                        .w(px(label_width))
                        .text_color(t.text_secondary)
                        .child(text(locale, key)),
                )
                .child(hint)
        };
        div()
            .id("first-run")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .px(px(32.))
            .child(
                // The column is centred; everything inside it is left-aligned.
                div()
                    .w_full()
                    .max_w(px(crate::ui::column_max()))
                    .pt(px(96.))
                    .pb(px(48.))
                    .flex()
                    .flex_col()
                    // Ready for Claude Code (ADR 0013), while a required or
                    // recommended row is not ready; GitHub CLI never here.
                    .when(ready, |this| this.children(block).child(div().h(px(32.))))
                    .child(
                        type_scale::TITLE
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::FirstrunTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(12.))
                            .max_w(px(480.))
                            .text_color(t.text_secondary)
                            .child(text(locale, Key::FirstrunBody)),
                    )
                    .child(
                        div().mt(px(24.)).flex().flex_row().child(
                            ui::button(
                                "first-run-choose",
                                text(locale, Key::FirstrunChoose),
                                Variant::Primary,
                                false,
                                &t,
                            )
                            .child(ui::kbd_on_accent(&[m, "O"], &t))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(FirstRunEvent::ChooseFolder)),
                            ),
                        ),
                    )
                    .child(
                        type_scale::LABEL
                            .el()
                            .mt(px(32.))
                            .mb(px(8.))
                            .text_color(t.text_secondary)
                            .child(text(locale, Key::FirstrunThen)),
                    )
                    .child(step(
                        "1",
                        Key::FirstrunStep1,
                        type_scale::CODE
                            .el()
                            .text_color(t.text_secondary)
                            .child("acta setup")
                            .into_any_element(),
                    ))
                    .child(step(
                        "2",
                        Key::FirstrunStep2,
                        ui::kbd(&[m, "N"], &t).into_any_element(),
                    ))
                    .child(step(
                        "3",
                        Key::FirstrunStep3,
                        ui::kbd(&[ctrl, "`"], &t).into_any_element(),
                    )),
            )
    }
}
