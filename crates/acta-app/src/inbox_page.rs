//! A project's notes inbox (docs/design/screens.md 8.2): the capture editor
//! (focused when the inbox opens), the Open / Filed filter, and the notes,
//! newest first. Filing a note by hand ("Make topic") is here; letting
//! Claude Code file them is M6.

use acta_core::model::{Author, Note, NoteId, PageReq, ProjectId, Record, Status};
use acta_core::store::NewTopic;
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
    Window, div, px,
};

use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::icons::Icon;
use crate::markdown_view;
use crate::record_page::PageEvent;
use crate::route::Route;
use crate::settings;
use crate::theme::{RADIUS_MD, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{Undo, Workspace};

/// A note's title when it becomes a topic: its first line, at most 80
/// characters, without Markdown heading or list markers.
pub fn title_from_note(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let line = line.trim_start_matches(['#', '-', '*', '>', ' ']);
    let mut title: String = line.chars().take(80).collect();
    if line.chars().count() > 80 {
        title.push('\u{2026}');
    }
    title
}

pub struct InboxPage {
    ws: Entity<Workspace>,
    pub project: ProjectId,
    project_title: String,
    notes: Vec<Note>,
    total_open: i64,
    show_filed: bool,
    capture: Entity<TextareaState>,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PageEvent> for InboxPage {}

impl InboxPage {
    pub fn new(
        ws: Entity<Workspace>,
        project: ProjectId,
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
        let capture = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 16)
                .placeholder(placeholder)
        });
        // The capture editor is focused when the inbox opens (8.2).
        capture.update(cx, |state, cx| state.focus(window, cx));
        let subs = vec![
            cx.observe(&ws, |page, ws, cx| {
                let revision = ws.read(cx).revision;
                if revision != page.seen_revision {
                    page.seen_revision = revision;
                    page.reload(cx);
                }
            }),
            cx.subscribe_in(
                &capture,
                window,
                |page, _, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter {
                        secondary: true, ..
                    } = event
                    {
                        page.add(window, cx);
                    }
                },
            ),
        ];
        let mut page = Self {
            ws,
            project,
            project_title,
            notes: Vec::new(),
            total_open: 0,
            show_filed: false,
            capture,
            seen_revision: 0,
            _load: Task::ready(()),
            _subs: subs,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let project = self.project;
        let task = self.ws.read(cx).db.run(cx, move |store| {
            let notes = store.inbox(project, PageReq::first(PageReq::MAX_LIMIT))?;
            Ok(notes)
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok(notes) = loaded {
                    page.total_open = notes
                        .items
                        .iter()
                        .filter(|n| n.filed_topic.is_none())
                        .count() as i64;
                    page.notes = notes.items;
                }
                cx.notify();
            });
        });
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.capture.read(cx).value().trim().to_owned();
        if body.is_empty() {
            return;
        }
        self.capture
            .update(cx, |state, cx| state.set_value("", window, cx));
        let project = self.project;
        let restore = body.clone();
        let task = self.ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| {
                store.add_note(project, &body, Author::Human)
            })
        });
        cx.spawn_in(window, async move |page, cx| {
            if task.await.is_none() {
                let _ = page.update_in(cx, |page, window, cx| {
                    page.capture
                        .update(cx, |state, cx| state.set_value(restore, window, cx));
                });
            }
        })
        .detach();
    }

    fn delete(&mut self, id: NoteId, cx: &mut Context<Self>) {
        let record = Record::Note(id);
        let locale = settings::locale(cx);
        let message = text(locale, Key::CommonDelete);
        self.notes.retain(|n| n.id != id);
        cx.notify();
        self.ws.update(cx, |ws, cx| {
            ws.write_undoable(cx, Undo::Restore(record), Some(message), move |store| {
                store.delete(record, Author::Human)
            });
        });
    }

    /// File a note by hand: a Draft topic titled from its first line, with
    /// the note filed into it.
    fn make_topic(&mut self, note: &Note, cx: &mut Context<Self>) {
        let project = self.project;
        let id = note.id;
        let title = title_from_note(&note.text);
        let description = note.text.clone();
        let task = self.ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| {
                let topic = store.create_topic(
                    NewTopic {
                        project,
                        epic: None,
                        title,
                        description,
                        status: Status::Draft,
                    },
                    Author::Human,
                )?;
                store.file_notes(&[id], topic, Author::Human)?;
                Ok(topic)
            })
        });
        cx.spawn(async move |page, cx| {
            if let Some(topic) = task.await {
                let _ = page.update(cx, |_, cx| cx.emit(PageEvent::Open(Route::Topic(topic))));
            }
        })
        .detach();
    }

    fn card(
        &self,
        note: &Note,
        index: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filed = note.filed_topic.is_some();
        let group = SharedString::from(format!("note-{index}"));
        let id = note.id;
        let for_topic = note.clone();
        let mut meta = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(24.))
            .child(
                type_scale::SMALL
                    .el()
                    .font_features(type_scale::tabular())
                    .text_color(t.text_tertiary)
                    .child(when::long_alone(locale, note.stamp.created_at)),
            );
        if let Some(topic) = note.filed_topic {
            meta = meta
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::InboxFiledAs)),
                )
                .child(
                    div()
                        .id(("note-topic", index))
                        .text_color(t.accent)
                        .text_size(px(12.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(PageEvent::Open(Route::Topic(topic)))
                        }))
                        .child(topic.to_string()),
                );
        }
        let actions = div()
            .flex()
            .flex_row()
            .gap(px(4.))
            .invisible()
            .group_hover(group.clone(), |s| s.visible())
            .when(!filed, |this| {
                this.child(
                    ui::ghost_icon_button(
                        format!("note-make-{index}"),
                        Icon::CornerDownRight,
                        24.,
                        false,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| page.make_topic(&for_topic, cx))),
                )
            })
            .child(
                ui::ghost_icon_button(
                    format!("note-delete-{index}"),
                    Icon::Trash2,
                    24.,
                    false,
                    false,
                    true,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| page.delete(id, cx))),
            );
        div()
            .id(("note", index))
            .group(group)
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(16.))
            .py(px(12.))
            .rounded(px(RADIUS_MD))
            .bg(t.bg_raised)
            .border_1()
            .border_color(t.border_subtle)
            .child(
                div()
                    .max_h(px(144.))
                    .overflow_hidden()
                    .text_color(if filed {
                        t.text_secondary
                    } else {
                        t.text_primary
                    })
                    .child(markdown_view::short(
                        ("note-body", index),
                        note.text.clone(),
                        t,
                    )),
            )
            .child(meta.child(div().flex_1()).child(actions))
            .into_any_element()
    }
}

impl Render for InboxPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let keys = format!(
            "{}+{}",
            KeyName::Mod.label(locale, macos),
            KeyName::Enter.label(locale, macos)
        );
        let count = tr(
            locale,
            Key::InboxCount,
            &[("n", Arg::Count(self.total_open as u64))],
        );
        let filters = [false, true]
            .into_iter()
            .enumerate()
            .map(|(i, filed)| {
                let key = if filed {
                    Key::InboxFilterFiled
                } else {
                    Key::InboxFilterOpen
                };
                ui::segment(
                    ("inbox-filter", i),
                    text(locale, key),
                    self.show_filed == filed,
                    &t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.show_filed = filed;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let capture = div()
            .flex()
            .flex_col()
            .mt(px(24.))
            .rounded(px(RADIUS_MD))
            .bg(t.bg_raised)
            .border_1()
            .border_color(t.border_strong)
            .px(px(16.))
            .py(px(12.))
            .child(
                type_scale::READING
                    .el()
                    .child(Textarea::new(&self.capture).appearance(false)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .h(px(28.))
                    .mt(px(8.))
                    .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                        locale,
                        Key::ComposerHint,
                        &[("keys", Arg::Text(&keys))],
                    )))
                    .child(
                        ui::button(
                            "inbox-add",
                            text(locale, Key::InboxAdd),
                            Variant::Primary,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(|page, _, window, cx| page.add(window, cx))),
                    ),
            );
        let shown: Vec<Note> = self
            .notes
            .iter()
            .filter(|n| n.filed_topic.is_some() == self.show_filed)
            .cloned()
            .collect();
        let mut list = div().flex().flex_col().gap(px(8.)).mt(px(12.));
        if shown.is_empty() && !self.show_filed {
            list = list.child(
                type_scale::READING
                    .el()
                    .max_w(px(480.))
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::InboxEmpty)),
            );
        }
        for (i, note) in shown.iter().enumerate() {
            list = list.child(self.card(note, i, locale, &t, cx));
        }
        ui::page_scroll("inbox-page").child(
            ui::column()
                .child(
                    type_scale::TITLE
                        .el()
                        .text_color(t.text_primary)
                        .child(text(locale, Key::TreeInbox)),
                )
                .child(
                    // "4 notes · Acta": the separator is 8 of space (8.2).
                    type_scale::SMALL
                        .el()
                        .mt(px(8.))
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .text_color(t.text_tertiary)
                        .child(count)
                        .child(self.project_title.clone()),
                )
                .child(capture)
                .child(
                    div()
                        .mt(px(24.))
                        .flex()
                        .child(ui::segmented(&t).children(filters)),
                )
                .child(list),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_becomes_a_title_from_its_first_line() {
        assert_eq!(
            title_from_note("\n# Terminal should remember its width\nper project"),
            "Terminal should remember its width"
        );
        assert_eq!(title_from_note("- weekly digest"), "weekly digest");
        let long = "x".repeat(100);
        assert_eq!(title_from_note(&long).chars().count(), 81);
    }
}
