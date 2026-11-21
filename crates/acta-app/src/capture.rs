//! Quick capture (docs/design/interaction.md 6.1, `Mod+N`): a palette-sized
//! overlay with one editor and the project as a chip. `Mod+Enter` files the
//! note in that project's inbox and closes, with no toast; `Esc` closes and
//! keeps the draft. It never navigates away from what was being read.

use acta_core::model::{Author, ProjectId};
use gpui_kit::component::input::{Escape, InputEvent, Textarea, TextareaState};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};

use crate::i18n::{Arg, Key, KeyName, text, tr};
use crate::settings;
use crate::theme::{RADIUS_SM, RADIUS_XL, Tokens};
use crate::type_scale;
use crate::ui;
use crate::workspace::Workspace;

pub enum CaptureEvent {
    /// Closed; the draft (possibly empty) is kept by the shell.
    Close(String),
}

pub struct Capture {
    ws: Entity<Workspace>,
    project: ProjectId,
    project_title: String,
    editor: Entity<TextareaState>,
    _sub: Subscription,
}

impl EventEmitter<CaptureEvent> for Capture {}

impl Capture {
    pub fn new(
        ws: Entity<Workspace>,
        project: ProjectId,
        draft: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
        let sub = cx.subscribe_in(
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
        );
        Self {
            ws,
            project,
            project_title,
            editor,
            _sub: sub,
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
        self.ws.update(cx, |ws, cx| {
            ws.write_detached(cx, Key::ErrorSave, move |store| {
                store.add_note(project, &body, Author::Human)
            });
        });
        cx.emit(CaptureEvent::Close(String::new()));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        let draft = self.editor.read(cx).value().to_string();
        cx.emit(CaptureEvent::Close(draft));
    }
}

impl Render for Capture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let keys = format!(
            "{}+{}",
            KeyName::Mod.label(locale, macos),
            KeyName::Enter.label(locale, macos)
        );
        let width = (window.viewport_size().width - px(48.)).min(px(640.));
        div()
            .id("capture")
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
            .capture_action(cx.listener(|capture, _: &Escape, _, cx| {
                capture.close(cx);
                cx.stop_propagation();
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
                        .text_size(px(type_scale::READING.size()))
                        .line_height(px(type_scale::READING.line())),
                ),
            )
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
                    .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                        locale,
                        Key::ComposerHint,
                        &[("keys", Arg::Text(&keys))],
                    )))
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
    }
}
