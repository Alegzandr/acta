//! Quick capture (docs/design/interaction.md 6.1, `Mod+N`): a palette-sized
//! overlay with one editor and the project as a chip. `Mod+Enter` files the
//! note in that project's inbox and closes, with no toast; `Esc` closes and
//! keeps the draft. It never navigates away from what was being read.

use acta_core::model::{AttachOwner, Place, ProjectId};
use gpui_kit::component::input::{Escape, IndentInline, InputEvent, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};

use crate::attach_ui::{self, Tray};
use crate::contact_picker::{ContactPicker, Mode, PickerEvent, Value};

use crate::i18n::{Arg, Key, KeyName, text, tr};
use crate::settings;
use crate::theme::{RADIUS_SM, RADIUS_XL, Tokens};
use crate::type_scale;
use crate::ui;
use crate::workspace::Workspace;

pub enum CaptureEvent {
    /// Closed; the draft (possibly empty) and its files, if any, are kept
    /// by the shell.
    Close(String, Option<Entity<Tray>>),
    /// After closing: the store refused the note's contact (it moved out
    /// of the project meanwhile). The shell opens the capture again with
    /// the text, the files and the picker's inline error
    /// (components-contacts.md 5, Errors).
    Refused {
        body: String,
        name: String,
        tray: Entity<Tray>,
    },
}

pub struct Capture {
    ws: Entity<Workspace>,
    project: ProjectId,
    project_title: String,
    editor: Entity<TextareaState>,
    /// Report to (components-questions.md 5, Notes): empty each time.
    report: Entity<ContactPicker>,
    /// The files pasted, dropped or chosen (components-attachments.md 5).
    pub tray: Entity<Tray>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<CaptureEvent> for Capture {}

impl Capture {
    pub fn new(
        ws: Entity<Workspace>,
        project: ProjectId,
        draft: String,
        tray: Option<Entity<Tray>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tray = tray.unwrap_or_else(|| {
            let ws = ws.clone();
            cx.new(|cx| Tray::new(ws, None, Vec::new(), cx))
        });
        let locale = settings::locale(cx);
        let project_title = ws
            .read(cx)
            .projects
            .iter()
            .find(|t| t.project.id == project)
            .map(|t| t.project.title.clone())
            .unwrap_or_default();
        let placeholder = tr(
            locale,
            Key::InboxPlaceholder,
            &[("project", Arg::Text(&project_title))],
        );
        let editor = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 12)
                .placeholder(placeholder)
                .default_value(draft)
        });
        editor.update(cx, |state, cx| state.focus(window, cx));
        crate::voice::attach(&editor, window, cx);
        let place = Place {
            project,
            epic: None,
        };
        let report = {
            let ws = ws.clone();
            cx.new(|cx| ContactPicker::new(ws, place, Value::None, Mode::Inline, window, cx))
        };
        let subs = vec![
            cx.observe(&tray, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &editor,
                window,
                |capture, _, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter {
                        secondary: true, ..
                    } = event
                    {
                        capture.save(window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &report,
                window,
                |capture, _, event: &PickerEvent, window, cx| match event {
                    PickerEvent::Submit => capture.save(window, cx),
                    PickerEvent::Cancel => capture.close(cx),
                    PickerEvent::Tab(true) | PickerEvent::Taken(_, Some(true)) => {
                        capture.editor.update(cx, |s, cx| s.focus(window, cx))
                    }
                    _ => cx.notify(),
                },
            ),
        ];
        Self {
            ws,
            project,
            project_title,
            editor,
            report,
            tray,
            _subs: subs,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.editor.read(cx).value().trim().to_owned();
        if body.is_empty() {
            return;
        }
        self.editor
            .update(cx, |state, cx| state.set_value("", window, cx));
        let project = self.project;
        let person = self.report.read(cx).value().clone();
        let person_name = person.name().to_owned();
        let kept = body.clone();
        let task = self.ws.update(cx, |ws, cx| {
            ws.write_picked(cx, move |store| {
                crate::inbox_page::add_note_with(store, project, &body, person)
            })
        });
        // The files get their rows once the note exists; saving does not
        // wait for them (components-attachments.md 5, While adding).
        let tray = self.tray.clone();
        let (body, name) = (kept, person_name);
        // Held until the write answers, so a refusal still reaches the
        // shell after the overlay closed.
        let this = cx.entity();
        cx.spawn(async move |_, cx| match task.await {
            Ok(note) => {
                tray.update(cx, |tray, cx| {
                    tray.commit(AttachOwner::Note(note), project, cx)
                });
            }
            Err(crate::workspace::WriteError::OutOfScope) => {
                this.update(cx, |_, cx| {
                    cx.emit(CaptureEvent::Refused { body, name, tray })
                });
            }
            Err(_) => {}
        })
        .detach();
        cx.emit(CaptureEvent::Close(String::new(), None));
    }

    /// Opened again after a refusal: the picker shows the inline error.
    pub fn refused(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let place = Place {
            project: self.project,
            epic: None,
        };
        self.report
            .update(cx, |p, cx| p.refuse(name, place, window, cx));
        self.report.update(cx, |p, cx| p.focus(window, cx));
    }

    /// Debug builds: a note typed and a screenshot pasted (the paste hook
    /// with a clipboard holding an image and no text), for captures.
    #[cfg(debug_assertions)]
    pub fn dev_fill(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |s, cx| {
            s.set_value(
                "Error on the export screen, and the log Alexandre sent.",
                window,
                cx,
            )
        });
        let clip = attach_ui::Clip {
            text: false,
            files: Vec::new(),
            image: Some(crate::dev_seed::sample_png(1280, 720, true)),
        };
        self.tray.update(cx, |tray, cx| tray.paste(clip, cx));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        let draft = self.editor.read(cx).value().to_string();
        let tray = (!self.tray.read(cx).is_empty()).then(|| self.tray.clone());
        cx.emit(CaptureEvent::Close(draft, tray));
    }
}

impl Render for Capture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The palette's "Attach files..." finds this editor's tray.
        attach_ui::pair(&self.editor, &self.tray, cx);
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let keys = format!(
            "{}+{}",
            KeyName::Mod.label(locale, macos),
            KeyName::Enter.label(locale, macos)
        );
        let width = (window.viewport_size().width - px(48.)).min(px(640.));
        let strip = self.tray.update(cx, |tray, cx| tray.render_strip(cx));
        let body = div()
            .occlude()
            .w(width)
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_XL))
            .shadow(vec![t.shadow_overlay()])
            .capture_action(cx.listener(|capture, _: &Escape, window, cx| {
                // The picker's own Esc closes its list first.
                if !capture.report.focus_handle(cx).contains_focused(window, cx) {
                    capture.close(cx);
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|capture, _: &IndentInline, window, cx| {
                // In capture, Tab moves to Report to (it does not indent).
                if capture.editor.read(cx).focus_handle(cx).is_focused(window) {
                    capture.report.update(cx, |p, cx| p.focus(window, cx));
                    cx.stop_propagation();
                }
            }))
            .child(
                div().flex().flex_row().child(
                    type_scale::SMALL
                        .el()
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(RADIUS_SM))
                        .bg(t.accent_subtle)
                        .text_color(t.text_primary)
                        .child(self.project_title.clone()),
                ),
            )
            .child(
                type_scale::READING.el().mx(px(-10.)).my(px(-8.)).child(
                    Textarea::new(&self.editor)
                        .appearance(false)
                        .on_paste(attach_ui::on_paste(&self.tray))
                        .text_size(px(type_scale::READING.size()))
                        .line_height(px(type_scale::READING.line())),
                ),
            )
            .children(strip)
            .child(
                // The editor's footer (components.md 5, Multi-line editor):
                // the save hint and the primary button.
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .h(px(28.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .flex_1()
                            .child(
                                type_scale::SMALL
                                    .el()
                                    .flex_shrink_0()
                                    .text_color(t.text_tertiary)
                                    .child(text(locale, Key::PropReportTo)),
                            )
                            .child(div().w(px(240.)).child(self.report.clone())),
                    )
                    .child(attach_ui::attach_button("capture", &self.tray, cx))
                    // Dictation (components-voice.md 5): the microphone after
                    // the paperclip, left of the save hint, which yields
                    // while a take shows its status.
                    .children(crate::voice::ui::mic_for(&self.editor, cx))
                    .when(!crate::voice::ui::listening_for(&self.editor, cx), |this| {
                        this.child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                            locale,
                            Key::ComposerHint,
                            &[("keys", Arg::Text(&keys))],
                        )))
                    })
                    .child(
                        ui::button(
                            "capture-add",
                            // A note, not a timeline entry: "Add".
                            text(locale, Key::InboxAdd),
                            ui::Variant::Primary,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(|capture, _, window, cx| capture.save(window, cx))),
                    ),
            )
            .children(crate::voice::ui::line_for(&self.editor, cx));
        attach_ui::drop_zone("capture", &self.tray, RADIUS_XL, body, cx)
    }
}
