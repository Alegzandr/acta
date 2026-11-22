//! A project's notes inbox (docs/design/screens.md 8.2): the capture editor
//! (focused when the inbox opens), the Open / Filed filter, and the notes,
//! newest first. Filing a note by hand is here: "Make topic" in one click,
//! or "File..." to choose where it goes (a new topic or epic, a topic of
//! an epic, an entry of a topic); Claude Code files them through the agent
//! tools (`create` with `from_notes`). Filed
//! notes link to where they went, say who filed them, and go back to the
//! inbox with one click.

use acta_core::Store;
use acta_core::model::{
    Author, EntryKind, EpicId, Note, NoteFate, NoteId, PageReq, ProjectId, ProjectTree, Record,
    Status, TopicId,
};
use acta_core::store::{NewEntry, NewTopic};
use gpui_kit::component::input::{Escape, InputEvent, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, div, px,
};

use crate::i18n::{Arg, Key, KeyName, Locale, number, text, tr};
use crate::icons::{Icon, icon};
use crate::markdown_view;
use crate::palette::{Choice, Lead, OnPick, Picker};
use crate::record_page::PageEvent;
use crate::route::Route;
use crate::settings;
use crate::theme::{RADIUS_LG, RADIUS_SM, Tokens};
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

/// Where "File..." can send a note (the rows of its picker, in order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    /// A Draft topic of the project, titled from the note.
    NewTopic,
    /// An epic of the project, titled from the note.
    NewEpic,
    /// A Draft topic in this epic.
    InEpic(EpicId),
    /// A Discussion entry of this topic, keeping the note's words.
    Entry(TopicId),
}

/// The picker of "File...": the two new records, then each epic, then
/// each topic (loose first, then by epic), with what choosing it does.
pub fn destinations(tree: &ProjectTree, locale: Locale) -> Vec<(Destination, Choice)> {
    let project = &tree.project.title;
    let mut out = vec![
        (
            Destination::NewTopic,
            Choice {
                title: text(locale, Key::TopicNew),
                detail: project.clone(),
                lead: Lead::Icon(Icon::CornerDownRight),
                trailing: String::new(),
                refusal: None,
            },
        ),
        (
            Destination::NewEpic,
            Choice {
                title: text(locale, Key::InboxPickNewEpic),
                detail: project.clone(),
                lead: Lead::Icon(Icon::Layers),
                trailing: String::new(),
                refusal: None,
            },
        ),
    ];
    for (epic, _) in &tree.epics {
        let path = format!("{project} / {}", epic.title);
        out.push((
            Destination::InEpic(epic.id),
            Choice {
                title: epic.title.clone(),
                detail: tr(locale, Key::InboxPickInEpic, &[("path", Arg::Text(&path))]),
                lead: Lead::Icon(Icon::Layers),
                trailing: String::new(),
                refusal: None,
            },
        ));
    }
    let loose = tree.loose_topics.iter().map(|t| (t, project.clone()));
    let in_epics = tree.epics.iter().flat_map(|(e, topics)| {
        topics
            .iter()
            .map(move |t| (t, format!("{project} / {}", e.title)))
    });
    for (topic, path) in loose.chain(in_epics) {
        out.push((
            Destination::Entry(topic.id),
            Choice {
                title: topic.title.clone(),
                detail: tr(locale, Key::InboxPickAsEntry, &[("path", Arg::Text(&path))]),
                lead: Lead::Status(topic.status),
                trailing: String::new(),
                refusal: None,
            },
        ));
    }
    out
}

/// File `note` (of `project`) at `to`, in one transaction, as the human.
/// Returns the undo step (which puts the note back in the inbox and sends
/// what filing created to the trash, as one step) and the page to open.
pub fn file_note(
    store: &mut Store,
    project: ProjectId,
    note: NoteId,
    to: Destination,
) -> acta_core::Result<(Undo, Option<Route>)> {
    let me = Author::Human;
    store.atomically(|store| {
        let current = store.note(note)?;
        let (created, route) = match to {
            Destination::NewTopic | Destination::InEpic(_) => {
                let epic = match to {
                    Destination::InEpic(e) => Some(e),
                    _ => None,
                };
                let topic = store.note_to_topic(
                    note,
                    NewTopic {
                        project,
                        epic,
                        title: title_from_note(&current.text),
                        description: current.text.clone(),
                        status: Status::Draft,
                    },
                    me,
                )?;
                (Record::Topic(topic), Some(Route::Topic(topic)))
            }
            Destination::NewEpic => {
                let epic = store.create_epic(
                    project,
                    &title_from_note(&current.text),
                    &current.text,
                    me,
                )?;
                store.file_notes_in_epic(&[note], epic, me)?;
                (Record::Epic(epic), Some(Route::Epic(epic)))
            }
            Destination::Entry(topic) => {
                // As Claude Code's triage does: the note's words and author.
                let entry = store.log(
                    topic,
                    NewEntry {
                        kind: EntryKind::Discussion,
                        text: current.text.clone(),
                        state: None,
                        status: None,
                    },
                    current.author,
                )?;
                store.file_notes(&[note], topic, me)?;
                (Record::Entry(entry.id), None)
            }
        };
        Ok((
            Undo::Many(vec![Undo::Unfile(note), Undo::Delete(created)]),
            route,
        ))
    })
}

/// Save a note with its Report to (components-questions.md 5, Notes): a
/// contact taken as "New contact" is created now, with its name only, at
/// the project's level.
pub fn add_note_with(
    store: &mut acta_core::Store,
    project: ProjectId,
    body: &str,
    report: crate::contact_picker::Value,
) -> acta_core::Result<NoteId> {
    use crate::contact_picker::Value;
    store.atomically(|store| {
        let id = store.add_note(project, body, Author::Human)?;
        let contact = match report {
            Value::None => None,
            Value::Contact(c, _) => Some(c),
            Value::New(name) => Some(store.create_contact(
                &name,
                acta_core::model::ContactLevel::Project(project),
                Author::Human,
            )?),
        };
        if contact.is_some() {
            store.set_report_to(acta_core::model::Record::Note(id), contact, Author::Human)?;
        }
        Ok(id)
    })
}

/// The contacts the listed notes name, with how many each, for the inbox's
/// Report to filter (first seen first).
pub fn report_people(notes: &[&Note]) -> Vec<(acta_core::model::ContactId, String, usize)> {
    let mut out: Vec<(acta_core::model::ContactId, String, usize)> = Vec::new();
    for n in notes {
        if let Some(c) = &n.report_to {
            match out.iter_mut().find(|(id, _, _)| *id == c.id) {
                Some(entry) => entry.2 += 1,
                None => out.push((c.id, c.name.clone(), 1)),
            }
        }
    }
    out
}

pub struct InboxPage {
    ws: Entity<Workspace>,
    pub project: ProjectId,
    project_title: String,
    notes: Vec<Note>,
    /// Filed and dismissed notes, most recently filed first, with the title
    /// of the topic or epic each went to.
    filed: Vec<(Note, Option<String>)>,
    total_open: i64,
    show_filed: bool,
    capture: Entity<TextareaState>,
    /// The capture's Report to (empty each time).
    report: Entity<crate::contact_picker::ContactPicker>,
    /// The Report to filter: `None` anyone; and its menu.
    report_filter: Option<acta_core::model::ContactId>,
    filter_menu: Option<usize>,
    filter_anchor: crate::menu::AnchorBounds,
    /// A note being edited in place, with its editor.
    editing: Option<(NoteId, Entity<TextareaState>)>,
    /// Shown as the Inbox view (workbench.md 4.8.3): no title, no meta
    /// line, the narrow form, following the active project.
    as_view: bool,
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
        let page = Self::build(ws, project, false, window, cx);
        // The capture editor is focused when the inbox opens (8.2).
        page.capture.update(cx, |state, cx| state.focus(window, cx));
        page
    }

    /// The Inbox as a view of a side area or the bottom panel: it takes no
    /// focus on its own.
    pub fn new_view(
        ws: Entity<Workspace>,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(ws, project, true, window, cx)
    }

    fn build(
        ws: Entity<Workspace>,
        project: ProjectId,
        as_view: bool,
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
        let place = acta_core::model::Place {
            project,
            epic: None,
        };
        let report = cx.new(|cx| {
            crate::contact_picker::ContactPicker::new(
                ws.clone(),
                place,
                crate::contact_picker::Value::None,
                crate::contact_picker::Mode::Inline,
                window,
                cx,
            )
        });
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
            cx.subscribe_in(
                &report,
                window,
                |page, _, event: &crate::contact_picker::PickerEvent, window, cx| {
                    use crate::contact_picker::PickerEvent;
                    match event {
                        PickerEvent::Submit => page.add(window, cx),
                        PickerEvent::Tab(true) | PickerEvent::Taken(_, Some(true)) => {
                            page.capture.update(cx, |s, cx| s.focus(window, cx))
                        }
                        _ => cx.notify(),
                    }
                },
            ),
        ];
        let mut page = Self {
            ws,
            project,
            project_title,
            notes: Vec::new(),
            filed: Vec::new(),
            total_open: 0,
            show_filed: false,
            capture,
            report,
            report_filter: None,
            filter_menu: None,
            filter_anchor: crate::menu::AnchorBounds::default(),
            editing: None,
            as_view,
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
            let open = store.inbox(project, PageReq::first(PageReq::MAX_LIMIT))?;
            let filed = store.filed_notes(project, PageReq::first(PageReq::MAX_LIMIT))?;
            // A target in the trash shows its id alone.
            let filed = filed
                .items
                .into_iter()
                .map(|n| {
                    let title = match n.fate() {
                        NoteFate::Topic(t) => store.topic(t).ok().map(|t| t.title),
                        NoteFate::Epic(e) => store.epic(e).ok().map(|e| e.title),
                        NoteFate::Open | NoteFate::Dismissed => None,
                    };
                    (n, title)
                })
                .collect::<Vec<_>>();
            Ok((open, filed))
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok((open, filed)) = loaded {
                    page.total_open = open.total;
                    page.notes = open.items;
                    page.filed = filed;
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
        // The field starts empty each time (components-questions.md 5).
        let person = self.report.read(cx).value().clone();
        self.report.update(cx, |p, cx| {
            p.set_value(crate::contact_picker::Value::None, window, cx)
        });
        let task = self.ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| {
                add_note_with(store, project, &body, person)
            })
        });
        cx.spawn_in(window, async move |page, cx| {
            if task.await.is_err() {
                let _ = page.update_in(cx, |page, window, cx| {
                    page.capture
                        .update(cx, |state, cx| state.set_value(restore, window, cx));
                });
            }
        })
        .detach();
    }

    /// "Report to: anyone": a compact dropdown of the listed notes'
    /// contacts with their counts.
    fn render_report_filter(
        &self,
        people: &[(acta_core::model::ContactId, String, usize)],
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chosen = self
            .report_filter
            .and_then(|c| people.iter().find(|(id, _, _)| *id == c));
        let label = match chosen {
            Some((_, name, _)) => tr(locale, Key::ReportFilter, &[("name", Arg::Text(name))]),
            None => text(locale, Key::ReportFilterAll),
        };
        let role = if chosen.is_some() {
            type_scale::UI_STRONG
        } else {
            type_scale::UI
        };
        let hover = t.fill_hover;
        let button = ui::focusable(role.el().id("inbox-report-filter"), t, false)
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .h(px(28.))
            .px(px(8.))
            .rounded(px(RADIUS_SM))
            .border_1()
            .border_color(t.border_default)
            .text_color(if chosen.is_some() {
                t.text_primary
            } else {
                t.text_secondary
            })
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(|page, _, _, cx| {
                page.filter_menu = Some(0);
                cx.notify();
            }))
            .child(label)
            .child(icon(Icon::ChevronDown, 14., t.text_tertiary))
            .child(self.filter_anchor.measure());
        let menu = self.filter_menu.map(|hl| {
            let mut panel = crate::menu::panel("inbox-report-menu", t).on_mouse_down_out(
                cx.listener(|page, _, _, cx| {
                    page.filter_menu = None;
                    cx.notify();
                }),
            );
            panel = panel.child(
                crate::menu::item(
                    ("report-filter", 0usize),
                    None,
                    text(locale, Key::ReportFilterAnyone),
                    self.report_filter.is_none().then(|| crate::menu::check(t)),
                    hl == 0,
                    t,
                )
                .on_click(cx.listener(|page, _, _, cx| {
                    page.report_filter = None;
                    page.filter_menu = None;
                    cx.notify();
                })),
            );
            for (i, (id, name, n)) in people.iter().enumerate() {
                let id = *id;
                panel = panel.child(
                    crate::menu::item(
                        ("report-filter", i + 1),
                        None,
                        name.clone(),
                        Some(crate::menu::hint(number(locale, *n as u64), t)),
                        hl == i + 1,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.report_filter = Some(id);
                        page.filter_menu = None;
                        cx.notify();
                    })),
                );
            }
            self.filter_anchor
                .below(panel, crate::menu::Align::Start)
                .unwrap_or_else(|| div().into_any_element())
        });
        div()
            .relative()
            .child(button)
            .children(menu)
            .into_any_element()
    }

    /// Edit (note card, 5): the body becomes the editor in place;
    /// `Mod+Enter` saves, `Esc` cancels.
    fn edit(&mut self, note: &Note, window: &mut Window, cx: &mut Context<Self>) {
        let id = note.id;
        let state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 16)
                .default_value(note.text.clone())
        });
        state.update(cx, |state, cx| state.focus(window, cx));
        let sub = cx.subscribe_in(
            &state,
            window,
            move |page, editor, event: &InputEvent, _, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    let text = editor.read(cx).value().trim().to_owned();
                    if text.is_empty() {
                        return;
                    }
                    page.editing = None;
                    let project = Some(page.project);
                    // Undoable: the text before is put back (Undo::Note).
                    page.ws.update(cx, |ws, cx| {
                        ws.undoable(cx, project, None, move |store| {
                            let back = store.note(id)?.text;
                            store.update_note(id, &text, Author::Human)?;
                            let expect = store.note(id)?.text;
                            Ok((Undo::Note { id, back, expect }, ()))
                        });
                    });
                    cx.notify();
                }
            },
        );
        self._subs.push(sub);
        self.editing = Some((id, state));
        cx.notify();
    }

    fn delete(&mut self, id: NoteId, cx: &mut Context<Self>) {
        let record = Record::Note(id);
        let project = Some(self.project);
        self.notes.retain(|n| n.id != id);
        self.filed.retain(|(n, _)| n.id != id);
        cx.notify();
        self.ws.update(cx, |ws, cx| ws.delete(record, project, cx));
    }

    /// Back to the inbox. What filing created (topic, epic, entries) stays:
    /// it may already hold the human's refinements.
    /// Undo files it again exactly as it was, if it is still open then.
    fn unfile(&mut self, id: NoteId, cx: &mut Context<Self>) {
        let project = self.project;
        self.ws.update(cx, |ws, cx| {
            ws.write_undoable(cx, Some(project), None, move |store| {
                let before = store.note(id)?;
                store.unfile_note(id, Author::Human)?;
                Ok((Undo::Refile(Box::new(before)), ()))
            })
            .detach();
        });
    }

    /// "Make topic": a Draft topic titled from the note's first line, with
    /// the note filed into it.
    fn make_topic(&mut self, note: &Note, cx: &mut Context<Self>) {
        self.file(note.id, Destination::NewTopic, cx);
    }

    /// File a note at `to`, undoably as one step; the topic or epic it
    /// creates opens.
    fn file(&mut self, id: NoteId, to: Destination, cx: &mut Context<Self>) {
        let project = self.project;
        let task = self.ws.update(cx, |ws, cx| {
            ws.write_undoable(cx, Some(project), None, move |store| {
                file_note(store, project, id, to)
            })
        });
        cx.spawn(async move |page, cx| {
            if let Ok(Some(route)) = task.await {
                let _ = page.update(cx, |_, cx| cx.emit(PageEvent::Open(route)));
            }
        })
        .detach();
    }

    /// "File...": the palette as a picker of where the note can go.
    fn pick_destination(&mut self, id: NoteId, cx: &mut Context<Self>) {
        let locale = settings::locale(cx);
        let Some(tree) = self
            .ws
            .read(cx)
            .projects
            .iter()
            .find(|t| t.project.id == self.project)
        else {
            return;
        };
        let (dests, choices): (Vec<Destination>, Vec<Choice>) =
            destinations(tree, locale).into_iter().unzip();
        let page = cx.entity().downgrade();
        let on_pick = OnPick::new(move |i, _, cx| {
            if let Some(to) = dests.get(i).copied() {
                let _ = page.update(cx, |page, cx| page.file(id, to, cx));
            }
        });
        let picker = Picker {
            label: text(locale, Key::InboxFileTo),
            choices,
        };
        cx.emit(PageEvent::Pick(picker, on_pick));
    }

    fn card(
        &self,
        note: &Note,
        target_title: Option<&str>,
        index: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fate = note.fate();
        let filed = fate != NoteFate::Open;
        let group = SharedString::from(format!("note-{index}"));
        let id = note.id;
        let for_topic = note.clone();
        let for_edit = note.clone();
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
        let target = match fate {
            NoteFate::Topic(t) => Some((t.to_string(), Route::Topic(t))),
            NoteFate::Epic(e) => Some((e.to_string(), Route::Epic(e))),
            NoteFate::Open | NoteFate::Dismissed => None,
        };
        if let Some((label, route)) = target {
            let label = match target_title {
                Some(title) => format!("{label} {title}"),
                None => label,
            };
            meta = meta
                .child(
                    type_scale::SMALL
                        .el()
                        .flex_none()
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::InboxFiledAs)),
                )
                .child(
                    type_scale::SMALL
                        .el()
                        .id(("note-target", index))
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(t.accent)
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(route))))
                        .child(label),
                );
        } else if fate == NoteFate::Dismissed {
            meta = meta.child(
                type_scale::SMALL
                    .el()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::InboxDismissed)),
            );
        }
        if let Some(c) = &note.report_to {
            let (before, after) = crate::question_view::around_name(locale, Key::ReportTo, &c.name);
            let id = c.id;
            let mut name = crate::question_view::name_link(
                ("note-report", index),
                &c.name,
                c.in_trash,
                locale,
                t,
            );
            if !c.in_trash {
                name = name.on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Contact(id)))),
                );
            }
            meta = meta.child(
                type_scale::SMALL
                    .el()
                    .flex()
                    .flex_row()
                    .min_w_0()
                    .text_color(t.text_tertiary)
                    .child(before)
                    .child(name)
                    .child(after),
            );
        }
        if filed && note.filed_by == Some(Author::Claude) {
            meta = meta.child(
                type_scale::SMALL
                    .el()
                    .flex_none()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::InboxByClaude)),
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
                .child(
                    ui::ghost_icon_button(
                        format!("note-file-{index}"),
                        Icon::ArrowRight,
                        24.,
                        false,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| page.pick_destination(id, cx))),
                )
            })
            .when(filed, |this| {
                this.child(
                    ui::ghost_icon_button(
                        format!("note-unfile-{index}"),
                        Icon::Inbox,
                        24.,
                        false,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| page.unfile(id, cx))),
                )
            })
            .child(
                ui::ghost_icon_button(
                    format!("note-edit-{index}"),
                    Icon::Pencil,
                    24.,
                    false,
                    false,
                    true,
                    t,
                )
                .on_click(cx.listener(move |page, _, window, cx| page.edit(&for_edit, window, cx))),
            )
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
        ui::focusable(div().id(("note", index)), t, false)
            .group(group)
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(16.))
            .py(px(12.))
            .rounded(px(RADIUS_LG))
            .bg(t.bg_raised)
            .border_1()
            .border_color(t.border_subtle)
            .child(match self.editing.as_ref().filter(|(e, _)| *e == id) {
                Some((_, editor)) => type_scale::READING
                    .el()
                    .mx(px(-10.))
                    .my(px(-8.))
                    .on_action(cx.listener(|page, _: &Escape, _, cx| {
                        page.editing = None;
                        cx.notify();
                    }))
                    .child(
                        Textarea::new(editor)
                            .appearance(false)
                            .text_size(px(type_scale::READING.size()))
                            .line_height(px(type_scale::READING.line())),
                    )
                    .into_any_element(),
                None => div()
                    .max_h(px(type_scale::READING.line() * 6.))
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
                    ))
                    .into_any_element(),
            })
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
            .rounded(px(RADIUS_LG))
            .bg(t.bg_raised)
            .border_1()
            .border_color(t.border_strong)
            .px(px(16.))
            .py(px(12.))
            .capture_action(cx.listener(
                |page, _: &gpui_kit::component::input::IndentInline, window, cx| {
                    // In capture, Tab moves to Report to (it does not indent).
                    if page.capture.read(cx).focus_handle(cx).is_focused(window) {
                        page.report.update(cx, |p, cx| p.focus(window, cx));
                        cx.stop_propagation();
                    }
                },
            ))
            .child(
                type_scale::READING.el().mx(px(-10.)).my(px(-8.)).child(
                    Textarea::new(&self.capture)
                        .appearance(false)
                        .text_size(px(type_scale::READING.size()))
                        .line_height(px(type_scale::READING.line())),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .min_h(px(28.))
                    .mt(px(8.))
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
        let listed: Vec<(Note, Option<String>)> = if self.show_filed {
            self.filed.clone()
        } else {
            self.notes.iter().map(|n| (n.clone(), None)).collect()
        };
        // The Report to filter exists while a listed note names someone.
        let people = report_people(&listed.iter().map(|(n, _)| n).collect::<Vec<_>>());
        if self
            .report_filter
            .is_some_and(|c| !people.iter().any(|(id, _, _)| *id == c))
        {
            self.report_filter = None;
        }
        let shown: Vec<(Note, Option<String>)> = match self.report_filter {
            Some(c) => listed
                .into_iter()
                .filter(|(n, _)| n.report_to.as_ref().is_some_and(|r| r.id == c))
                .collect(),
            None => listed,
        };
        let report_filter =
            (!people.is_empty()).then(|| self.render_report_filter(&people, locale, &t, cx));
        let mut list = div().flex().flex_col().gap(px(8.)).mt(px(12.));
        if shown.is_empty() {
            list = list.child(
                type_scale::READING
                    .el()
                    .max_w(px(480.))
                    .text_color(t.text_secondary)
                    .child(text(
                        locale,
                        if self.show_filed {
                            Key::InboxFiledEmpty
                        } else {
                            Key::InboxEmpty
                        },
                    )),
            );
        }
        for (i, (note, title)) in shown.iter().enumerate() {
            list = list.child(self.card(note, title.as_deref(), i, locale, &t, cx));
        }
        let column = if self.as_view {
            ui::view_column().child(capture.mt(px(0.)))
        } else {
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
        }
        .child(
            div()
                .mt(px(24.))
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::segmented(&t).children(filters))
                        .children(report_filter),
                )
                .child({
                    // 8.2: the single secondary button, disabled with the
                    // Open list empty.
                    let none = self.notes.is_empty();
                    let project = self.project;
                    ui::button(
                        "inbox-file-claude",
                        text(locale, Key::InboxFile),
                        Variant::Secondary,
                        none,
                        &t,
                    )
                    .when(!none, |this| {
                        this.on_click(
                            cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Triage(project))),
                        )
                    })
                }),
        )
        .child(list);
        if self.as_view {
            ui::view_scroll("inbox-view")
                .child(column)
                .into_any_element()
        } else {
            ui::page_scroll("inbox-page")
                .child(column)
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::temp_store;

    fn inbox() -> (
        crate::db::test_support::TempStore,
        ProjectId,
        EpicId,
        TopicId,
        NoteId,
    ) {
        let mut temp = temp_store();
        let s = &mut temp.store;
        let p = s.create_project("Acta", "", Author::Human).unwrap();
        let e = s.create_epic(p, "Terminal", "", Author::Human).unwrap();
        let t = s
            .create_topic(
                NewTopic {
                    project: p,
                    epic: Some(e),
                    title: "Scrollback".into(),
                    description: String::new(),
                    status: Status::Active,
                },
                Author::Human,
            )
            .unwrap();
        let n = s
            .add_note(p, "Alexandre: keep the width per project", Author::Human)
            .unwrap();
        (temp, p, e, t, n)
    }

    #[test]
    fn the_picker_lists_new_records_then_epics_then_topics() {
        let (temp, p, e, t, _) = inbox();
        let tree = temp.store.tree(p).unwrap();
        let rows = destinations(&tree, Locale::En);
        let dests: Vec<_> = rows.iter().map(|(d, _)| *d).collect();
        assert_eq!(
            dests,
            [
                Destination::NewTopic,
                Destination::NewEpic,
                Destination::InEpic(e),
                Destination::Entry(t)
            ]
        );
        assert_eq!(rows[2].1.detail, "New topic in Acta / Terminal");
        assert_eq!(rows[3].1.detail, "Entry in Acta / Terminal");
    }

    #[test]
    fn filing_as_an_entry_keeps_the_words_and_undoes_as_one_step() {
        let (mut temp, p, _, t, n) = inbox();
        let s = &mut temp.store;
        let (undo, route) = file_note(s, p, n, Destination::Entry(t)).unwrap();
        assert_eq!(route, None);
        assert_eq!(s.note(n).unwrap().fate(), NoteFate::Topic(t));
        let entries = s
            .entries(t, &Default::default(), PageReq::first(10))
            .unwrap();
        assert!(entries.items.iter().any(|e| e.kind == EntryKind::Discussion
            && e.text == "Alexandre: keep the width per project"));
        assert!(undo.apply(s).unwrap().is_some());
        assert!(s.note(n).unwrap().is_open());
    }

    #[test]
    fn filing_into_an_epic_or_as_an_epic() {
        let (mut temp, p, e, _, n) = inbox();
        let s = &mut temp.store;
        let (_, route) = file_note(s, p, n, Destination::InEpic(e)).unwrap();
        let Some(Route::Topic(topic)) = route else {
            panic!("a topic opens")
        };
        let topic = s.topic(topic).unwrap();
        assert_eq!(topic.epic, Some(e));
        assert_eq!(topic.status, Status::Draft);
        assert_eq!(topic.title, "Alexandre: keep the width per project");

        let m = s.add_note(p, "Notes\nmore", Author::Human).unwrap();
        let (undo, route) = file_note(s, p, m, Destination::NewEpic).unwrap();
        let Some(Route::Epic(epic)) = route else {
            panic!("an epic opens")
        };
        assert_eq!(s.note(m).unwrap().fate(), NoteFate::Epic(epic));
        assert!(undo.apply(s).unwrap().is_some());
        assert!(s.epic(epic).is_err(), "in the trash");
        assert!(s.note(m).unwrap().is_open());
    }

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
