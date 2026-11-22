//! The Questions section and the Report to row of a record page
//! (docs/design/components-questions.md 5; screens.md 8.21): the rows, the
//! composer with its contact picker and answer date, the answer editor
//! with the decision offer, the row menu and keys, the roll-up row; the
//! Report to row, its popover and its menu. Every change is one undo step
//! with no toast but delete; a contact made inline is created in the same
//! step as the write that names it.

use std::collections::HashSet;

use acta_core::model::{Question, QuestionId, QuestionStatus};
use acta_core::store::NewQuestion as Ask;
use gpui_kit::component::input::Enter;

use super::*;
use crate::contact_picker::{ContactPicker, Mode, PickerEvent, Value};
use crate::question_view::{self, RowLook};
use crate::questions_model::{self as qm, Section};
use crate::workspace::{ReportState, put_report, report_state, with_lifts};

gpui_kit::actions!(
    acta_questions,
    [
        /// `Q`: a new question on the page (below the focused one).
        NewQuestion,
        /// `R`: the Report to popover.
        OpenReportTo,
    ]
);

/// The keys of the section (bound in the shell with the page's own).
pub fn bindings() -> Vec<gpui_kit::KeyBinding> {
    let page = Some("RecordPage && !Input");
    vec![
        gpui_kit::KeyBinding::new("q", NewQuestion, page),
        gpui_kit::KeyBinding::new("r", OpenReportTo, page),
    ]
}

/// The question composer: new (`editing: None`) or editing in place.
pub(super) struct Composer {
    editing: Option<QuestionId>,
    /// `Q` on a focused row: the new question goes below it.
    below: Option<usize>,
    text: Entity<TextareaState>,
    picker: Entity<ContactPicker>,
    date: Option<Date>,
    date_picker: Option<Entity<DuePicker>>,
    date_anchor: AnchorBounds,
    date_focus: FocusHandle,
    _subs: Vec<Subscription>,
}

/// The answer editor under a row.
pub(super) struct AnswerEdit {
    id: QuestionId,
    editor: Entity<TextareaState>,
    /// The decision checkbox is offered (an unlogged question on a topic).
    offer: bool,
    _sub: Subscription,
}

/// A row's answer date picker (`D`).
pub(super) struct RowDate {
    id: QuestionId,
    row: usize,
    picker: Entity<DuePicker>,
    _sub: Subscription,
}

/// The items of a row's menu, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QItem {
    Answer,
    Edit,
    SetDate,
    ClearDate,
    Drop,
    Move,
    Reopen,
    EditAnswer,
    Delete,
}

fn menu_items(q: &Question) -> Vec<QItem> {
    if q.is_open() {
        let mut out = vec![QItem::Answer, QItem::Edit, QItem::SetDate];
        if q.expected_on.is_some() {
            out.push(QItem::ClearDate);
        }
        out.extend([QItem::Drop, QItem::Move, QItem::Delete]);
        out
    } else if q.status == QuestionStatus::Answered {
        vec![QItem::Reopen, QItem::EditAnswer, QItem::Delete]
    } else {
        vec![QItem::Reopen, QItem::Delete]
    }
}

/// The Report to row's menu items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RItem {
    Change,
    Mark,
    Go,
    Clear,
}

#[derive(Default)]
pub(super) struct QuestionUi {
    composer: Option<Composer>,
    answer: Option<AnswerEdit>,
    /// A row's menu: the question and the highlighted item.
    menu: Option<(QuestionId, usize, usize)>,
    menu_anchor: AnchorBounds,
    row_focus: Vec<FocusHandle>,
    row_ids: std::cell::RefCell<Vec<QuestionId>>,
    date: Option<RowDate>,
    /// Unread dots stay for as long as the page is open.
    seen_unread: HashSet<QuestionId>,
    /// The Report to popover, its anchor, and the row's menu.
    report: Option<(Entity<ContactPicker>, Subscription)>,
    report_anchor: AnchorBounds,
    report_menu: Option<usize>,
    report_focus: Option<FocusHandle>,
    /// The composer, just opened, is brought into view once laid out.
    reveal: std::rc::Rc<std::cell::Cell<bool>>,
    /// Captures: the section is scrolled to the top of the page.
    reveal_section: std::rc::Rc<std::cell::Cell<bool>>,
}

impl RecordPage {
    pub(super) fn questions_menu_open(&self) -> bool {
        self.q.menu.is_some()
            || self.q.date.is_some()
            || self.q.report.is_some()
            || self.q.report_menu.is_some()
            || self
                .q
                .composer
                .as_ref()
                .is_some_and(|c| c.date_picker.is_some())
    }

    fn q_project(&self) -> Option<acta_core::model::ProjectId> {
        self.project_id()
    }

    fn q_place(&self) -> Option<acta_core::model::Place> {
        self.data.as_ref().map(|d| d.place)
    }

    /// Every question the section draws (open shown, then closed when
    /// unfolded), as their rows are numbered.
    fn drawn_questions(&self, cx: &App) -> Vec<Question> {
        let Some(data) = &self.data else {
            return Vec::new();
        };
        let s = &data.questions.section;
        let mut out = s.open.clone();
        if self.closed_unfolded(cx) {
            out.extend(s.closed.iter().cloned());
        }
        out
    }

    fn question(&self, id: QuestionId, cx: &App) -> Option<Question> {
        let data = self.data.as_ref()?;
        let s = &data.questions.section;
        s.open
            .iter()
            .chain(s.closed.iter())
            .find(|q| q.id == id)
            .cloned()
            .or_else(|| self.drawn_questions(cx).into_iter().find(|q| q.id == id))
    }

    /// The question row with keyboard focus.
    pub(super) fn focused_question(&self, window: &Window) -> Option<(usize, QuestionId)> {
        let row = self.q.row_focus.iter().position(|h| h.is_focused(window))?;
        let id = *self.q.row_ids.borrow().get(row)?;
        Some((row, id))
    }

    fn closed_key(&self) -> String {
        format!("questions:{}", self.owner)
    }

    fn closed_unfolded(&self, cx: &App) -> bool {
        let key = self.closed_key();
        settings::current(cx).closed_shown.contains(&key)
    }

    fn toggle_closed(&mut self, cx: &mut Context<Self>) {
        let key = self.closed_key();
        let open = !self.closed_unfolded(cx);
        settings::change(cx, |p| {
            p.closed_shown.retain(|k| *k != key);
            if open {
                p.closed_shown.push(key);
            }
        });
        cx.notify();
    }

    // ---- the composer ----

    /// `Q`, the header's `plus`, "New question": the composer as the last
    /// open row (below the focused row when `Q` came from one).
    pub fn new_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let below = self.focused_question(window).map(|(row, _)| row);
        self.open_composer(None, below, window, cx);
    }

    fn open_composer(
        &mut self,
        editing: Option<&Question>,
        below: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(place) = self.q_place() else { return };
        let locale = settings::locale(cx);
        let text = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 4)
                .submit_on_enter(true)
                .placeholder(text(locale, Key::QuestionPlaceholder))
                .default_value(editing.map(|q| q.text.clone()).unwrap_or_default())
        });
        text.update(cx, |s, cx| s.focus(window, cx));
        let value = editing
            .and_then(|q| q.asked_to.as_ref())
            .map(|c| Value::Contact(c.id, c.name.clone()))
            .unwrap_or_default();
        let ws = self.ws.clone();
        let picker = cx.new(|cx| ContactPicker::new(ws, place, value, Mode::Inline, window, cx));
        let subs = vec![
            cx.subscribe_in(
                &text,
                window,
                |page, editor, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => page.save_composer(window, cx),
                    InputEvent::Change => {
                        // One paragraph, at most 300 characters.
                        let value = editor.read(cx).value().to_string();
                        if value.chars().count() > qm::TEXT_MAX || value.contains('\n') {
                            let clean = qm::clean_text(&value);
                            editor.update(cx, |s, cx| s.set_value(clean, window, cx));
                        }
                        cx.notify();
                    }
                    _ => {}
                },
            ),
            cx.subscribe_in(
                &picker,
                window,
                |page, _, event: &PickerEvent, window, cx| match event {
                    PickerEvent::Submit => page.save_composer(window, cx),
                    PickerEvent::Cancel => page.close_composer(window, cx),
                    PickerEvent::Taken(_, Some(false)) | PickerEvent::Tab(false) => {
                        if let Some(c) = &page.q.composer {
                            window.focus(&c.date_focus, cx);
                        }
                    }
                    PickerEvent::Tab(true) | PickerEvent::Taken(_, Some(true)) => {
                        if let Some(c) = &page.q.composer {
                            c.text.update(cx, |s, cx| s.focus(window, cx));
                        }
                    }
                    PickerEvent::Taken(_, None) => cx.notify(),
                },
            ),
        ];
        self.q.composer = Some(Composer {
            editing: editing.map(|q| q.id),
            below,
            text,
            picker,
            date: editing.and_then(|q| q.expected_on),
            date_picker: None,
            date_anchor: AnchorBounds::default(),
            date_focus: cx.focus_handle(),
            _subs: subs,
        });
        self.q.reveal.set(true);
        cx.notify();
    }

    fn close_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.q.composer = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// `Enter` or `Mod+Enter`: save. A new question keeps the composer
    /// open and empty for the next; an edit closes it. No text: close.
    fn save_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &self.q.composer else { return };
        let text = qm::clean_text(&c.text.read(cx).value());
        if text.is_empty() {
            self.close_composer(window, cx);
            return;
        }
        let value = c.picker.read(cx).value().clone();
        let (date, editing, below) = (c.date, c.editing, c.below);
        let owner = self.owner;
        let project = self.q_project();
        let Some(place) = self.q_place() else { return };
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let me = Author::Human;
                let mut steps = Vec::new();
                let asked = match value {
                    Value::None => None,
                    Value::Contact(id, _) => Some(id),
                    Value::New(name) => {
                        let id = store.create_contact(
                            &name,
                            crate::contacts_model::default_level(place.project),
                            me,
                        )?;
                        steps.push(Undo::Delete(Record::Contact(id)));
                        Some(id)
                    }
                };
                let first = match editing {
                    None => {
                        let id = store.ask(
                            Ask {
                                owner,
                                text,
                                asked_to: asked,
                                expected_on: date,
                            },
                            me,
                        )?;
                        if let Some(row) = below {
                            store.reorder_question(id, row + 1, me)?;
                        }
                        Undo::Delete(Record::Question(id))
                    }
                    Some(id) => {
                        let back = store.question(id)?;
                        store.edit_question(id, Some(&text), Some(asked), me)?;
                        store.set_expected(id, date, me)?;
                        Undo::Question {
                            back: Box::new(back),
                            expect: Box::new(store.question(id)?),
                        }
                    }
                };
                // The question goes back first, then the contact it named.
                steps.insert(0, first);
                let undo = if steps.len() == 1 {
                    steps.remove(0)
                } else {
                    Undo::Many(steps)
                };
                Ok((undo, ()))
            });
        });
        match editing {
            Some(_) => self.close_composer(window, cx),
            None => {
                // Five questions after a meeting are five Enters.
                let Some(c) = &mut self.q.composer else {
                    return;
                };
                c.text.update(cx, |s, cx| s.set_value("", window, cx));
                c.picker
                    .update(cx, |p, cx| p.set_value(Value::None, window, cx));
                c.date = None;
                c.below = c.below.map(|r| r + 1);
                c.text.update(cx, |s, cx| s.focus(window, cx));
                cx.notify();
            }
        }
    }

    fn open_composer_date(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let today = due::today(cx);
        let Some(current) = self.q.composer.as_ref().map(|c| c.date) else {
            return;
        };
        let picker = cx.new(|cx| DuePicker::new(today, current, window, cx));
        let sub = cx.subscribe_in(
            &picker,
            window,
            |page, _, event: &DuePickerEvent, window, cx| {
                let DuePickerEvent::Done(outcome) = *event;
                if let Some(c) = &mut page.q.composer {
                    if let Outcome::Set(date) = outcome {
                        c.date = date;
                    }
                    c.date_picker = None;
                    window.focus(&c.date_focus, cx);
                }
                cx.notify();
            },
        );
        if let Some(c) = &mut self.q.composer {
            c.date_picker = Some(picker);
            c._subs.push(sub);
        }
        cx.notify();
    }

    // ---- answering ----

    fn start_answer(&mut self, id: QuestionId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(q) = self.question(id, cx) else {
            return;
        };
        let locale = settings::locale(cx);
        let offer = matches!(q.owner, Owner::Topic(_)) && q.decision_entry.is_none();
        let editor = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 12)
                .placeholder(text(locale, Key::QuestionAnswerPlaceholder))
                .default_value(q.answer.clone().unwrap_or_default())
        });
        editor.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.subscribe_in(
            &editor,
            window,
            |page, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    page.save_answer(window, cx);
                }
            },
        );
        self.q.menu = None;
        self.q.answer = Some(AnswerEdit {
            id,
            editor,
            offer,
            _sub: sub,
        });
        cx.notify();
    }

    fn save_answer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.q.answer.take() else {
            return;
        };
        let answer = edit.editor.read(cx).value().trim().to_owned();
        let log = edit.offer && !settings::current(cx).skip_decision;
        let id = edit.id;
        let project = self.q_project();
        // The keyboard goes to the next open question, else the header.
        let next = self
            .data
            .as_ref()
            .map(|d| d.questions.section.open.clone())
            .unwrap_or_default()
            .into_iter()
            .skip_while(|q| q.id != id)
            .nth(1)
            .map(|q| q.id);
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let back = store.question(id)?;
                let answer = (!answer.is_empty()).then_some(answer);
                let entry = store.answer(id, answer.as_deref(), log, Author::Human)?;
                let step = Undo::Question {
                    back: Box::new(back),
                    expect: Box::new(store.question(id)?),
                };
                // Answer and its decision entry are one step.
                let undo = match entry {
                    Some(e) => Undo::Many(vec![step, Undo::Delete(Record::Entry(e.id))]),
                    None => step,
                };
                Ok((undo, ()))
            });
        });
        let rows = self.q.row_ids.borrow().clone();
        let target = next.and_then(|n| rows.iter().position(|r| *r == n));
        match target.and_then(|i| self.q.row_focus.get(i)) {
            Some(h) => window.focus(h, cx),
            None => window.focus(&self.focus, cx),
        }
        cx.notify();
    }

    fn cancel_answer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.q.answer = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    // ---- other writes ----

    /// Any change to one question, undoable as the question before.
    fn change_question(
        &mut self,
        id: QuestionId,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut acta_core::Store) -> acta_core::Result<()> + Send + 'static,
    ) {
        let project = self.q_project();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let back = store.question(id)?;
                f(store)?;
                Ok((
                    Undo::Question {
                        back: Box::new(back),
                        expect: Box::new(store.question(id)?),
                    },
                    (),
                ))
            });
        });
    }

    fn set_question_date(&mut self, id: QuestionId, date: Option<Date>, cx: &mut Context<Self>) {
        self.change_question(id, cx, move |store| {
            store.set_expected(id, date, Author::Human)
        });
    }

    fn reorder_question(
        &mut self,
        id: QuestionId,
        down: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(data) = &self.data else { return };
        let open: Vec<QuestionId> = data.questions.section.open.iter().map(|q| q.id).collect();
        let Some(at) = open.iter().position(|q| *q == id) else {
            return;
        };
        let Some(to) = qm::step(at, data.questions.section.open_total, down) else {
            return;
        };
        let project = self.q_project();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                store.reorder_question(id, to, Author::Human)?;
                Ok((
                    Undo::QuestionOrder {
                        id,
                        back: at,
                        expect: to,
                    },
                    (),
                ))
            });
        });
        // The keyboard follows the row.
        if let Some(h) = self.q.row_focus.get(to) {
            window.focus(h, cx);
        }
    }

    fn delete_question(&mut self, id: QuestionId, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.q_project();
        self.ws
            .update(cx, |ws, cx| ws.delete(Record::Question(id), project, cx));
        self.q.menu = None;
        window.focus(&self.focus, cx);
    }

    /// `M`: the palette lists the topic, its epic and the project first,
    /// then any epic or topic of the project.
    fn pick_question_owner(&mut self, id: QuestionId, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let locale = settings::locale(cx);
        let project = data.project.0;
        let Some(tree) = self
            .ws
            .read(cx)
            .projects
            .iter()
            .find(|t| t.project.id == project)
            .cloned()
        else {
            return;
        };
        let mut owners: Vec<(Owner, String, crate::palette::Lead)> = Vec::new();
        let topic_lead =
            |t: &acta_core::model::TopicSummary| crate::palette::Lead::Status(t.status);
        let mut push = |o: Owner, title: String, lead| {
            if !owners.iter().any(|(x, _, _)| *x == o) {
                owners.push((o, title, lead));
            }
        };
        let page_first = [self.owner]
            .into_iter()
            .chain(data.epic.as_ref().map(|(e, _)| Owner::Epic(*e)))
            .chain([Owner::Project(project)]);
        let title_of = |o: Owner| -> Option<(String, crate::palette::Lead)> {
            match o {
                Owner::Project(_) => Some((
                    tree.project.title.clone(),
                    crate::palette::Lead::Icon(Icon::Folder),
                )),
                Owner::Epic(e) => tree.epics.iter().find(|(x, _)| x.id == e).map(|(x, _)| {
                    (
                        format!("{} / {}", tree.project.title, x.title),
                        crate::palette::Lead::Icon(Icon::Layers),
                    )
                }),
                Owner::Topic(t) => tree
                    .loose_topics
                    .iter()
                    .chain(tree.epics.iter().flat_map(|(_, ts)| ts))
                    .find(|x| x.id == t)
                    .map(|x| (x.title.clone(), topic_lead(x))),
            }
        };
        for o in page_first {
            if let Some((title, lead)) = title_of(o) {
                push(o, title, lead);
            }
        }
        for (e, topics) in &tree.epics {
            push(
                Owner::Epic(e.id),
                format!("{} / {}", tree.project.title, e.title),
                crate::palette::Lead::Icon(Icon::Layers),
            );
            for t in topics {
                push(Owner::Topic(t.id), t.title.clone(), topic_lead(t));
            }
        }
        for t in &tree.loose_topics {
            push(Owner::Topic(t.id), t.title.clone(), topic_lead(t));
        }
        let current = self.question(id, cx).map(|q| q.owner);
        let choices = owners
            .iter()
            .map(|(o, title, lead)| crate::palette::Choice {
                title: title.clone(),
                detail: String::new(),
                lead: *lead,
                trailing: if Some(*o) == current {
                    text(locale, Key::ContactLevelCurrent)
                } else {
                    String::new()
                },
                refusal: None,
            })
            .collect();
        let ids: Vec<Owner> = owners.iter().map(|(o, _, _)| *o).collect();
        let page = cx.entity().downgrade();
        let on_pick = crate::palette::OnPick::new(move |i, _, cx| {
            if let Some(to) = ids.get(i).copied() {
                let _ = page.update(cx, |page, cx| page.move_question(id, to, cx));
            }
        });
        cx.emit(PageEvent::Pick(
            crate::palette::Picker {
                label: text(locale, Key::CmdQuestionMove),
                choices,
            },
            on_pick,
        ));
    }

    fn move_question(&mut self, id: QuestionId, to: Owner, cx: &mut Context<Self>) {
        let project = self.q_project();
        let task = self.ws.update(cx, |ws, cx| {
            ws.write_undoable(cx, project, None, move |store| {
                let back = store.question(id)?;
                store.take_lifts();
                store.move_question(id, to, Author::Human)?;
                let moved = Undo::Question {
                    back: Box::new(back),
                    expect: Box::new(store.question(id)?),
                };
                with_lifts(store, moved)
            })
        });
        self.ws.update(cx, |ws, cx| ws.toast_lifts(task, cx));
    }

    fn choose_q_item(
        &mut self,
        id: QuestionId,
        item: QItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.q.menu = None;
        match item {
            QItem::Answer | QItem::EditAnswer => self.start_answer(id, window, cx),
            QItem::Edit => {
                if let Some(q) = self.question(id, cx) {
                    self.open_composer(Some(&q), None, window, cx);
                }
            }
            QItem::SetDate => self.open_question_date(id, window, cx),
            QItem::ClearDate => self.set_question_date(id, None, cx),
            QItem::Drop => {
                self.change_question(id, cx, move |s| s.drop_question(id, Author::Human))
            }
            QItem::Reopen => {
                self.change_question(id, cx, move |s| s.reopen_question(id, Author::Human))
            }
            QItem::Move => self.pick_question_owner(id, cx),
            QItem::Delete => self.delete_question(id, window, cx),
        }
        cx.notify();
    }

    /// `D` on a focused row: the due date picker for its answer date.
    pub(super) fn open_question_date(
        &mut self,
        id: QuestionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(q) = self.question(id, cx) else {
            return;
        };
        let row = self
            .q
            .row_ids
            .borrow()
            .iter()
            .position(|r| *r == id)
            .unwrap_or(0);
        let today = due::today(cx);
        let picker = cx.new(|cx| DuePicker::new(today, q.expected_on, window, cx));
        let sub = cx.subscribe_in(
            &picker,
            window,
            |page, _, event: &DuePickerEvent, window, cx| {
                let DuePickerEvent::Done(outcome) = *event;
                let Some(d) = page.q.date.take() else { return };
                if let Outcome::Set(date) = outcome {
                    page.set_question_date(d.id, date, cx);
                }
                if let Some(h) = page.q.row_focus.get(d.row) {
                    window.focus(h, cx);
                }
                cx.notify();
            },
        );
        self.q.menu = None;
        self.q.date = Some(RowDate {
            id,
            row,
            picker,
            _sub: sub,
        });
        cx.notify();
    }

    /// `E` on a focused row: edit it (an answered one: its answer).
    pub(super) fn edit_focused_question(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_, id)) = self.focused_question(window) else {
            return false;
        };
        let Some(q) = self.question(id, cx) else {
            return false;
        };
        if q.is_open() {
            self.open_composer(Some(&q), None, window, cx);
        } else if q.status == QuestionStatus::Answered {
            self.start_answer(id, window, cx);
        }
        true
    }

    fn on_question_key(
        &mut self,
        id: QuestionId,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &event.keystroke;
        let Some(q) = self.question(id, cx) else {
            return;
        };
        if let Some((qid, row, hl)) = self.q.menu {
            let items = self
                .question(qid, cx)
                .map(|q| menu_items(&q))
                .unwrap_or_default();
            match k.key.as_str() {
                "up" => self.q.menu = Some((qid, row, hl.saturating_sub(1))),
                "down" => {
                    self.q.menu = Some((qid, row, (hl + 1).min(items.len().saturating_sub(1))))
                }
                "enter" | "space" => {
                    if let Some(item) = items.get(hl).copied() {
                        self.choose_q_item(qid, item, window, cx);
                    }
                }
                "escape" => self.q.menu = None,
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let alt = k.modifiers.alt;
        let plain = !k.modifiers.modified();
        match k.key.as_str() {
            "a" if plain && q.is_open() => self.start_answer(id, window, cx),
            "enter" if plain => {
                if q.is_open() {
                    self.open_composer(Some(&q), None, window, cx);
                } else if q.status == QuestionStatus::Answered {
                    self.start_answer(id, window, cx);
                }
            }
            "delete" if plain => self.delete_question(id, window, cx),
            "backspace" if k.modifiers.platform => self.delete_question(id, window, cx),
            "m" if plain => self.pick_question_owner(id, cx),
            "up" if alt && q.is_open() => self.reorder_question(id, false, window, cx),
            "down" if alt && q.is_open() => self.reorder_question(id, true, window, cx),
            "up" | "down" if plain => {
                let rows = self.q.row_ids.borrow().clone();
                let at = rows.iter().position(|r| *r == id).unwrap_or(0);
                let to = if k.key == "up" {
                    at.checked_sub(1)
                } else {
                    (at + 1 < rows.len()).then_some(at + 1)
                };
                if let Some(h) = to.and_then(|i| self.q.row_focus.get(i)) {
                    window.focus(h, cx);
                }
            }
            "f10" if k.modifiers.shift => {
                let row = self
                    .q
                    .row_ids
                    .borrow()
                    .iter()
                    .position(|r| *r == id)
                    .unwrap_or(0);
                self.q.menu = Some((id, row, 0));
            }
            "menu" => {
                let row = self
                    .q
                    .row_ids
                    .borrow()
                    .iter()
                    .position(|r| *r == id)
                    .unwrap_or(0);
                self.q.menu = Some((id, row, 0));
            }
            "escape" => window.focus(&self.focus, cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// The palette's question commands: the focused row's state.
    pub fn question_context(&self, window: &Window) -> Option<crate::commands::FocusedQuestion> {
        let (_, id) = self.focused_question(window)?;
        let data = self.data.as_ref()?;
        let s = &data.questions.section;
        let q = s.open.iter().chain(s.closed.iter()).find(|q| q.id == id)?;
        Some(crate::commands::FocusedQuestion {
            open: q.is_open(),
            dated: q.expected_on.is_some(),
        })
    }

    /// A palette command on the focused question.
    pub fn run_question(
        &mut self,
        command: crate::commands::QuestionCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::commands::QuestionCommand as C;
        let Some((_, id)) = self.focused_question(window) else {
            return;
        };
        let item = match command {
            C::Answer => QItem::Answer,
            C::Edit => {
                self.edit_focused_question(window, cx);
                return;
            }
            C::Date => QItem::SetDate,
            C::ClearDate => QItem::ClearDate,
            C::Drop => QItem::Drop,
            C::Reopen => QItem::Reopen,
            C::Move => QItem::Move,
            C::Delete => QItem::Delete,
        };
        self.choose_q_item(id, item, window, cx);
    }

    /// The palette's Report to commands: `None` on a project page.
    pub fn report_context(&self) -> Option<(bool, bool)> {
        self.report_record()?;
        let data = self.data.as_ref()?;
        Some((data.report.to.is_some(), data.report.at.is_some()))
    }

    // ---- Report to ----

    /// `R`, the add chip, "Set Report to...": the popover with the picker.
    pub fn open_report_to(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.owner, Owner::Project(_)) {
            return;
        }
        let Some(data) = &self.data else { return };
        let place = data.place;
        let value = data
            .report
            .to
            .as_ref()
            .filter(|c| !c.in_trash)
            .map(|c| Value::Contact(c.id, c.name.clone()))
            .unwrap_or_default();
        let ws = self.ws.clone();
        let picker = cx.new(|cx| ContactPicker::new(ws, place, value, Mode::Popover, window, cx));
        picker.update(cx, |p, cx| p.focus(window, cx));
        let sub = cx.subscribe_in(
            &picker,
            window,
            |page, _, event: &PickerEvent, window, cx| match event {
                PickerEvent::Taken(value, _) => {
                    let value = value.clone();
                    page.close_report(window, cx);
                    page.set_report(value, cx);
                }
                PickerEvent::Cancel => page.close_report(window, cx),
                _ => {}
            },
        );
        self.q.report = Some((picker, sub));
        self.q.report_menu = None;
        if self.q.report_focus.is_none() {
            self.q.report_focus = Some(cx.focus_handle());
        }
        cx.notify();
    }

    fn close_report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.q.report = None;
        match &self.q.report_focus {
            Some(h) => window.focus(h, cx),
            None => window.focus(&self.focus, cx),
        }
        cx.notify();
    }

    fn report_record(&self) -> Option<Record> {
        match self.owner {
            Owner::Topic(t) => Some(Record::Topic(t)),
            Owner::Epic(e) => Some(Record::Epic(e)),
            Owner::Project(_) => None,
        }
    }

    /// Set, change or clear the person: one undo step; a new contact is
    /// created in the same step.
    pub fn set_report(&mut self, value: Value, cx: &mut Context<Self>) {
        let Some(record) = self.report_record() else {
            return;
        };
        let Some(place) = self.q_place() else { return };
        let current = self
            .data
            .as_ref()
            .and_then(|d| d.report.to.as_ref())
            .map(|c| c.id);
        if let Value::Contact(id, _) = &value
            && Some(*id) == current
        {
            return;
        }
        if value == Value::None && current.is_none() {
            return;
        }
        let project = self.q_project();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let me = Author::Human;
                let back = report_state(store, record)?;
                let mut extra = Vec::new();
                let contact = match value {
                    Value::None => None,
                    Value::Contact(id, _) => Some(id),
                    Value::New(name) => {
                        let id = store.create_contact(
                            &name,
                            crate::contacts_model::default_level(place.project),
                            me,
                        )?;
                        extra.push(Undo::Delete(Record::Contact(id)));
                        Some(id)
                    }
                };
                store.set_report_to(record, contact, me)?;
                let mut steps = vec![Undo::ReportTo {
                    record,
                    back,
                    expect: report_state(store, record)?,
                }];
                steps.extend(extra);
                let undo = if steps.len() == 1 {
                    steps.remove(0)
                } else {
                    Undo::Many(steps)
                };
                Ok((undo, ()))
            });
        });
    }

    /// "Mark reported" / "Mark not reported".
    pub fn set_reported(&mut self, reported: bool, cx: &mut Context<Self>) {
        let Some(record) = self.report_record() else {
            return;
        };
        let project = self.q_project();
        if let Some(data) = self.data.as_mut() {
            if data.report.to.is_none() {
                return;
            }
            data.report.at = reported.then(acta_core::time::now_ms);
        }
        cx.notify();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let back: ReportState = report_state(store, record)?;
                put_report(store, record, (back.0, reported))?;
                Ok((
                    Undo::ReportTo {
                        record,
                        back,
                        expect: report_state(store, record)?,
                    },
                    (),
                ))
            });
        });
    }

    /// Whether the page's record has a person and is reported.
    pub fn report_target(&self) -> Option<bool> {
        let data = self.data.as_ref()?;
        self.report_record()?;
        data.report.to.as_ref().map(|_| data.report.at.is_some())
    }

    fn choose_r_item(&mut self, item: RItem, window: &mut Window, cx: &mut Context<Self>) {
        self.q.report_menu = None;
        match item {
            RItem::Change => self.open_report_to(window, cx),
            RItem::Mark => {
                let reported = self.report_target() == Some(true);
                self.set_reported(!reported, cx);
            }
            RItem::Go => {
                if let Some(c) = self.data.as_ref().and_then(|d| d.report.to.clone()) {
                    cx.emit(PageEvent::Open(Route::Contact(c.id)));
                }
            }
            RItem::Clear => self.set_report(Value::None, cx),
        }
        cx.notify();
    }

    #[cfg(debug_assertions)]
    /// Debug builds' captures (`ACTA_DEV_OVERLAY=q-*`, `report-popover`).
    pub fn dev_questions(
        &mut self,
        which: &str,
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let first = self
            .data
            .as_ref()
            .and_then(|d| d.questions.section.open.first().map(|q| q.id));
        self.q
            .reveal_section
            .set(which.starts_with("q-") && which != "q-composer");
        match which {
            "q-composer" => {
                self.open_composer(None, None, window, cx);
                if let Some(c) = &self.q.composer {
                    let typed = "Who signs the export policy?";
                    c.text.update(cx, |s, cx| s.set_value(typed, window, cx));
                    if !query.is_empty() {
                        c.picker.update(cx, |p, cx| {
                            p.focus(window, cx);
                            p.dev_type(query, window, cx)
                        });
                    }
                }
            }
            "q-answer" => {
                if let Some(id) = first {
                    self.start_answer(id, window, cx);
                    if let Some(a) = &self.q.answer {
                        a.editor.update(cx, |s, cx| {
                            s.set_value("Yes, until 31 March. After that only v2.", window, cx)
                        });
                    }
                }
            }
            "q-closed" => {
                if !self.closed_unfolded(cx) {
                    self.toggle_closed(cx);
                }
            }
            "q-menu" => {
                if let Some(id) = first {
                    self.q.menu = Some((id, 0, 0));
                }
            }
            "report-popover" => {
                self.open_report_to(window, cx);
                if !query.is_empty()
                    && let Some((picker, _)) = &self.q.report
                {
                    picker.update(cx, |p, cx| p.dev_type(query, window, cx));
                }
            }
            _ => {}
        }
        cx.notify();
    }

    // ---- rendering ----

    /// The Report to row (only while a person is set).
    pub(super) fn render_report(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let c = data.report.to.clone()?;
        let reported = data.report.at;
        let closed = match &data.topic {
            Some(topic) => match topic.status {
                Status::Done => Some(true),
                Status::Dropped => None,
                _ => Some(false),
            },
            None => {
                let (done, total) = data.progress();
                Some(total > 0 && done == total)
            }
        };
        let run = match (reported, closed) {
            (Some(at), _) => Some((
                tr(
                    locale,
                    Key::ReportDone,
                    &[("date", Arg::Text(&when::date(locale, at)))],
                ),
                t.text_tertiary,
            )),
            (None, Some(true)) => Some((text(locale, Key::ReportPending), t.text_secondary)),
            _ => None,
        };
        let id = c.id;
        let name = question_view::name_link("report-name", &c.name, c.in_trash, locale, t).when(
            !c.in_trash,
            |this| {
                this.on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Contact(id)))),
                )
            },
        );
        let (glyph, tip) = if reported.is_some() {
            (Icon::RotateCw, Key::ReportUnmark)
        } else {
            (Icon::Check, Key::ReportMark)
        };
        let mark = ui::ghost_icon_button("report-mark", glyph, 24., false, false, true, t)
            .tooltip(ui::tip(text(locale, tip)))
            .on_click(cx.listener(move |page, _, _, cx| {
                page.set_reported(reported.is_none(), cx);
            }));
        let focus = self.q.report_focus.clone();
        let menu = self
            .q
            .report_menu
            .map(|hl| self.render_report_menu(hl, reported.is_some(), &c.name, locale, t, cx));
        let popover = self.q.report.as_ref().and_then(|(picker, _)| {
            self.q.report_anchor.below_at(
                self.render_report_popover(picker, locale, t, cx),
                crate::menu::Align::Start,
                2,
            )
        });
        let mut row = div()
            .id("prop-report")
            .group("prop-report")
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .child(type_scale::UI.el().child(name))
            .children(run.map(|(words, color)| type_scale::UI.el().text_color(color).child(words)))
            .child(
                div()
                    .invisible()
                    .group_hover("prop-report", |s| s.visible())
                    .child(mark),
            )
            .child(self.q.report_anchor.measure())
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                if let Some(hl) = page.q.report_menu {
                    let items = page.report_items();
                    match k.key.as_str() {
                        "up" => page.q.report_menu = Some(hl.saturating_sub(1)),
                        "down" => page.q.report_menu = Some((hl + 1).min(items.len() - 1)),
                        "enter" | "space" => page.choose_r_item(items[hl], window, cx),
                        "escape" => page.q.report_menu = None,
                        _ => return,
                    }
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                match k.key.as_str() {
                    "delete" if !k.modifiers.modified() => page.set_report(Value::None, cx),
                    "enter" if !k.modifiers.modified() => page.open_report_to(window, cx),
                    "f10" if k.modifiers.shift => page.q.report_menu = Some(0),
                    "menu" => page.q.report_menu = Some(0),
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(|page, _, _, cx| {
                    page.q.report_menu = Some(0);
                    cx.notify();
                }),
            )
            .children(menu)
            .children(popover);
        if let Some(h) = &focus {
            row = row.track_focus(h);
        }
        Some(ui::focusable(row, t, false).into_any_element())
    }

    fn report_items(&self) -> Vec<RItem> {
        vec![RItem::Change, RItem::Mark, RItem::Go, RItem::Clear]
    }

    fn render_report_menu(
        &self,
        hl: usize,
        reported: bool,
        name: &str,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = self.report_items();
        let mut panel = crate::menu::panel("report-menu", t).on_mouse_down_out(cx.listener(
            |page, _, _, cx| {
                page.q.report_menu = None;
                cx.notify();
            },
        ));
        for (i, item) in items.iter().enumerate() {
            let label = match item {
                RItem::Change => text(locale, Key::ReportChange),
                RItem::Mark if reported => text(locale, Key::ReportUnmark),
                RItem::Mark => text(locale, Key::ReportMark),
                RItem::Go => tr(locale, Key::ContactGo, &[("name", Arg::Text(name))]),
                RItem::Clear => text(locale, Key::CommonClear),
            };
            let hint = matches!(item, RItem::Change).then(|| crate::menu::hint("R", t));
            let item = *item;
            panel = panel.child(
                crate::menu::item(("report-item", i), None, label, hint, i == hl, t).on_click(
                    cx.listener(move |page, _, window, cx| page.choose_r_item(item, window, cx)),
                ),
            );
        }
        self.q
            .report_anchor
            .below_at(panel, crate::menu::Align::Start, 2)
            .unwrap_or_else(|| div().into_any_element())
    }

    fn render_report_popover(
        &self,
        picker: &Entity<ContactPicker>,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let set = self.data.as_ref().is_some_and(|d| d.report.to.is_some());
        div()
            .id("report-popover")
            .occlude()
            .w(px(280.))
            .p(px(12.))
            .flex()
            .flex_col()
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_LG))
            .shadow(vec![t.shadow_overlay()])
            .on_mouse_down_out(cx.listener(|page, _, window, cx| page.close_report(window, cx)))
            .child(picker.clone())
            .when(set, |this| {
                this.child(
                    div().mt(px(8.)).h(px(28.)).flex().flex_row().child(
                        ui::button(
                            "report-clear",
                            text(locale, Key::CommonClear),
                            Variant::Ghost,
                            false,
                            t,
                        )
                        .ml(px(-12.))
                        .on_click(cx.listener(|page, _, window, cx| {
                            page.close_report(window, cx);
                            page.set_report(Value::None, cx);
                        })),
                    ),
                )
            })
            .into_any_element()
    }

    /// The "Report to" add chip (beside "Due date").
    pub(super) fn render_report_chip(
        &self,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hover = t.fill_hover;
        let popover = self.q.report.as_ref().and_then(|(picker, _)| {
            self.q.report_anchor.below_at(
                self.render_report_popover(picker, locale, t, cx),
                crate::menu::Align::Start,
                2,
            )
        });
        let mut chip = ui::focusable(type_scale::SMALL.el().id("prop-add-report"), t, false)
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .h(px(24.))
            .px(px(8.))
            .rounded(px(RADIUS_SM))
            .text_color(t.text_tertiary)
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(|page, _, window, cx| page.open_report_to(window, cx)))
            .child(icon(Icon::Plus, 14., t.text_tertiary))
            .child(text(locale, Key::PropAddReportTo))
            .child(self.q.report_anchor.measure());
        if let Some(h) = &self.q.report_focus {
            chip = chip.track_focus(h);
        }
        div().child(chip).children(popover).into_any_element()
    }

    pub(super) fn report_chip_wanted(&self, window: &Window) -> bool {
        self.q.report.is_some()
            || self
                .q
                .report_focus
                .as_ref()
                .is_some_and(|h| h.is_focused(window))
    }

    /// The Questions section (components-questions.md 5).
    pub(super) fn render_questions(
        &mut self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let today = due::today(cx);
        let s: &Section = &data.questions.section;
        self.q
            .seen_unread
            .extend(data.questions.unread.iter().copied());
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .child(ui::section_header(text(locale, Key::SectionQuestions), t))
            .when(s.open_total > 0, |this| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .font_features(type_scale::tabular())
                        .child(number(locale, s.open_total as u64)),
                )
            })
            .child(div().flex_1())
            .child(
                ui::ghost_icon_button("q-new", Icon::Plus, 24., false, false, true, t)
                    .tooltip(ui::tip_keys(text(locale, Key::QuestionsNew), vec!["Q"]))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_composer(None, None, window, cx)
                    })),
            );
        let reveal = std::rc::Rc::clone(&self.q.reveal_section);
        let scroll = self.scroll.clone();
        let mut section = div()
            .id("questions")
            .relative()
            .flex()
            .flex_col()
            .mt(px(32.))
            .child(header)
            .child(ui::measure(move |bounds, window| {
                if reveal.replace(false) {
                    ui::scroll_into_view(&scroll, bounds, ui::Reveal::Top);
                    window.refresh();
                }
            }));
        let unfolded = self.closed_unfolded(cx);
        let mut drawn: Vec<Question> = s.open.clone();
        if unfolded {
            drawn.extend(s.closed.iter().cloned());
        }
        while self.q.row_focus.len() < drawn.len() {
            self.q.row_focus.push(cx.focus_handle());
        }
        *self.q.row_ids.borrow_mut() = drawn.iter().map(|q| q.id).collect();
        if s.is_empty() && self.q.composer.is_none() {
            section = section.child(
                type_scale::READING
                    .el()
                    .mt(px(4.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::QuestionsEmpty)),
            );
        }
        let editing = self.q.composer.as_ref().and_then(|c| c.editing);
        let new_at = self
            .q
            .composer
            .as_ref()
            .filter(|c| c.editing.is_none())
            .map(|c| c.below.map(|r| r + 1).unwrap_or(usize::MAX));
        for (i, q) in s.open.iter().enumerate() {
            if new_at == Some(i) {
                section = section.child(self.render_composer(locale, t, window, cx));
            }
            if editing == Some(q.id) {
                section = section.child(self.render_composer(locale, t, window, cx));
                continue;
            }
            section = section.child(self.render_q_row(q, i, locale, t, today, cx));
        }
        if new_at.is_some_and(|at| at >= s.open.len()) {
            section = section.child(self.render_composer(locale, t, window, cx));
        }
        if let Some(n) = s.more() {
            let scope = self.owner;
            section = section.child(
                div().ml(px(-12.)).child(
                    ui::button(
                        "q-show-all",
                        tr(locale, Key::CommonShowAllN, &[("n", Arg::Count(n as u64))]),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(PageEvent::Open(Route::Questions(scope)))
                    })),
                ),
            );
        }
        if !s.closed.is_empty() {
            let hover = t.fill_hover;
            section = section.child(
                ui::focusable(div().id("q-closed"), t, false)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .px(px(8.))
                    .mx(px(-8.))
                    .rounded(px(RADIUS_SM))
                    .hover(move |st| st.bg(hover))
                    .on_click(cx.listener(|page, _, _, cx| page.toggle_closed(cx)))
                    .child(icon(
                        if unfolded {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        },
                        14.,
                        t.text_tertiary,
                    ))
                    .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                        locale,
                        Key::QuestionsClosed,
                        &[("n", Arg::Count(s.closed.len() as u64))],
                    ))),
            );
            if unfolded {
                let base = s.open.len();
                for (i, q) in s.closed.iter().enumerate() {
                    section = section.child(self.render_q_row(q, base + i, locale, t, today, cx));
                }
            }
        }
        if data.questions.rollup.open > 0 {
            section = section.child(self.render_rollup(data, locale, t, cx));
        }
        section.into_any_element()
    }

    fn render_rollup(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let r = data.questions.rollup;
        let epic = matches!(self.owner, Owner::Epic(_));
        let key = match (epic, r.overdue > 0) {
            (true, false) => Key::QuestionsMoreEpic,
            (true, true) => Key::QuestionsMoreEpicOverdue,
            (false, false) => Key::QuestionsMoreProject,
            (false, true) => Key::QuestionsMoreProjectOverdue,
        };
        const MARK: &str = "\u{1}";
        let late = number(locale, r.overdue as u64);
        let line = tr(
            locale,
            key,
            &[
                ("n", Arg::Count(r.open as u64)),
                ("late", Arg::Text(&format!("{MARK}{late}"))),
            ],
        );
        // The overdue clause is one run in `warning` (catalogue 10.4.6):
        // from the comma before the count to the end.
        let (plain, warn) = match line.find(MARK) {
            Some(at) => {
                let head = &line[..at];
                let cut = head.rfind(',').unwrap_or(at);
                (line[..cut].to_owned(), line[cut..].replace(MARK, ""))
            }
            None => (line, String::new()),
        };
        let scope = self.owner;
        let hover = t.fill_hover;
        ui::focusable(div().id("q-rollup"), t, false)
            .flex()
            .flex_row()
            .items_center()
            .h(px(28.))
            .px(px(8.))
            .mx(px(-8.))
            .rounded(px(RADIUS_SM))
            .hover(move |st| st.bg(hover))
            .on_click(
                cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Questions(scope)))),
            )
            .child(
                type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(plain),
            )
            .when(!warn.is_empty(), |this| {
                this.child(type_scale::UI.el().text_color(t.warning).child(warn))
            })
            .into_any_element()
    }

    fn render_q_row(
        &self,
        q: &Question,
        index: usize,
        locale: Locale,
        t: &Tokens,
        today: Date,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = q.id;
        let look = RowLook {
            index,
            unread: self.q.seen_unread.contains(&id),
            owner: None,
            show_asked: true,
            today,
        };
        let answer_btn = q.is_open().then(|| {
            ui::ghost_icon_button(
                format!("q-answer-{index}"),
                Icon::Check,
                24.,
                false,
                false,
                true,
                t,
            )
            .tooltip(ui::tip_keys(
                text(locale, Key::QuestionMarkAnswered),
                vec!["A"],
            ))
            .on_click(cx.listener(move |page, _, window, cx| page.start_answer(id, window, cx)))
        });
        let more = ui::ghost_icon_button(
            format!("q-more-{index}"),
            Icon::Ellipsis,
            24.,
            false,
            false,
            true,
            t,
        )
        .on_click(cx.listener(move |page, _, _, cx| {
            page.q.menu = Some((id, index, 0));
            cx.notify();
        }));
        let actions = div()
            .flex()
            .flex_row()
            .gap(px(4.))
            .children(answer_btn)
            .child(div().relative().child(more))
            .into_any_element();
        let topic = self.topic();
        let mut row = question_view::row(
            q,
            &look,
            Some(actions),
            {
                let page = cx.entity().downgrade();
                move |c, _, cx| {
                    let _ = page.update(cx, |_, cx| cx.emit(PageEvent::Open(Route::Contact(c))));
                }
            },
            {
                let page = cx.entity().downgrade();
                move |_, _, cx| {
                    if let Some(t) = topic {
                        let _ = page.update(cx, |_, cx| cx.emit(PageEvent::Open(Route::Topic(t))));
                    }
                }
            },
            locale,
            t,
        )
        .relative()
        // The row's menu and its answer date picker hang from the row (the
        // hover buttons are hidden while it has only the keyboard); only
        // the row whose layer is open measures, so rows never take turns.
        .when(
            self.q.menu.is_some_and(|(m, _, _)| m == id)
                || self.q.date.as_ref().is_some_and(|d| d.id == id),
            |this| this.child(self.q.menu_anchor.measure_keyed(index as u64)),
        )
        .on_key_down(cx.listener(move |page, event: &KeyDownEvent, window, cx| {
            page.on_question_key(id, event, window, cx)
        }))
        .on_mouse_down(
            gpui_kit::MouseButton::Right,
            cx.listener(move |page, _, _, cx| {
                page.q.menu = Some((id, index, 0));
                cx.notify();
            }),
        );
        if let Some(h) = self.q.row_focus.get(index) {
            row = row.track_focus(h);
        }
        let menu = self
            .q
            .menu
            .filter(|(m, _, _)| *m == id)
            .map(|(_, _, hl)| self.render_q_menu(q, index, hl, locale, t, cx));
        let date = self.q.date.as_ref().filter(|d| d.id == id).and_then(|d| {
            self.q.menu_anchor.below_keyed(
                index as u64,
                d.picker.clone(),
                crate::menu::Align::End,
                2,
            )
        });
        let answer = self
            .q
            .answer
            .as_ref()
            .filter(|a| a.id == id)
            .map(|a| self.render_answer(a, locale, t, cx));
        div()
            .flex()
            .flex_col()
            .child(row)
            .children(answer)
            .children(menu)
            .children(date)
            .into_any_element()
    }

    fn render_q_menu(
        &self,
        q: &Question,
        index: usize,
        hl: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = q.id;
        let mut panel =
            crate::menu::panel("q-menu", t).on_mouse_down_out(cx.listener(|page, _, _, cx| {
                page.q.menu = None;
                cx.notify();
            }));
        for (i, item) in menu_items(q).into_iter().enumerate() {
            let (label, key) = match item {
                QItem::Answer => (text(locale, Key::QuestionMarkAnswered), Some("A")),
                QItem::Edit => (text(locale, Key::CommonEdit), Some("E")),
                QItem::SetDate => (text(locale, Key::QuestionSetDate), Some("D")),
                QItem::ClearDate => (text(locale, Key::QuestionClearDate), None),
                QItem::Drop => (text(locale, Key::QuestionDrop), None),
                QItem::Move => (text(locale, Key::QuestionMove), Some("M")),
                QItem::Reopen => (text(locale, Key::QuestionReopen), None),
                QItem::EditAnswer => (text(locale, Key::QuestionEditAnswer), Some("E")),
                QItem::Delete => (text(locale, Key::CommonDelete), Some("Delete")),
            };
            if item == QItem::Delete {
                panel = panel.child(crate::menu::separator(t));
            }
            panel =
                panel.child(
                    crate::menu::item(
                        ("q-item", i),
                        None,
                        label,
                        key.map(|k| crate::menu::hint(k, t)),
                        i == hl,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.choose_q_item(id, item, window, cx)
                    })),
                );
        }
        self.q
            .menu_anchor
            .below_keyed(index as u64, panel, crate::menu::Align::End, 2)
            .unwrap_or_else(|| div().into_any_element())
    }

    fn render_answer(
        &self,
        a: &AnswerEdit,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checked = !settings::current(cx).skip_decision;
        let offer = a.offer.then(|| {
            ui::focusable(div().id("q-log-decision"), t, false)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    settings::change(cx, |p| p.skip_decision = checked);
                    cx.notify();
                }))
                .child(ui::checkbox(checked, t))
                .child(
                    type_scale::UI
                        .el()
                        .text_color(t.text_primary)
                        .child(text(locale, Key::QuestionLogDecision)),
                )
        });
        div()
            .pl(px(24.))
            .mt(px(8.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(editor_box(&a.editor, locale, t, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .children(offer)
                    .child(div().flex_1())
                    .child(
                        ui::button(
                            "q-answer-cancel",
                            text(locale, Key::CommonCancel),
                            Variant::Ghost,
                            false,
                            t,
                        )
                        .on_click(
                            cx.listener(|page, _, window, cx| page.cancel_answer(window, cx)),
                        ),
                    )
                    .child(
                        ui::button(
                            "q-answer-save",
                            text(locale, Key::QuestionSaveAnswer),
                            Variant::Primary,
                            false,
                            t,
                        )
                        .on_click(cx.listener(|page, _, window, cx| page.save_answer(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_composer(
        &self,
        locale: Locale,
        t: &Tokens,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(c) = &self.q.composer else {
            return div().into_any_element();
        };
        let value = c.text.read(cx).value().to_string();
        let counter = qm::counter(&value).map(|n| {
            type_scale::SMALL
                .el()
                .flex_shrink_0()
                .text_color(t.text_tertiary)
                .font_features(type_scale::tabular())
                .child(tr(
                    locale,
                    Key::QuestionCounter,
                    &[("n", Arg::Count(n as u64))],
                ))
        });
        let narrow = ui::column_width() < 400.;
        let date_label = match c.date {
            Some(d) => due::date(locale, d),
            None => text(locale, Key::DueNone),
        };
        let date_focused = c.date_focus.is_focused(window);
        let date_button = ui::ghost_value("q-date", t)
            .relative()
            .ml(px(0.))
            .track_focus(&c.date_focus)
            .when(c.date.is_none(), |this| this.text_color(t.text_tertiary))
            .when(date_focused, |this| this.shadow(ui::ring_shadow(t, false)))
            .on_click(cx.listener(|page, _, window, cx| page.open_composer_date(window, cx)))
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                match k.key.as_str() {
                    "enter" | "space" if !k.modifiers.modified() => {
                        page.open_composer_date(window, cx)
                    }
                    "enter" if k.modifiers.control || k.modifiers.platform => {
                        page.save_composer(window, cx)
                    }
                    "escape" => page.close_composer(window, cx),
                    "tab" if k.modifiers.shift => {
                        if let Some(c) = &page.q.composer {
                            c.picker.update(cx, |p, cx| p.focus(window, cx));
                        }
                    }
                    "tab" => {
                        if let Some(c) = &page.q.composer {
                            c.text.update(cx, |s, cx| s.focus(window, cx));
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(date_label)
            .child(c.date_anchor.measure());
        let date_picker = c.date_picker.as_ref().and_then(|p| {
            c.date_anchor
                .below_at(p.clone(), crate::menu::Align::Start, 2)
        });
        let asked = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::QuestionFieldAsked)),
            )
            .child(
                div()
                    .when(narrow, |d| d.flex_1())
                    .when(!narrow, |d| d.w(px(240.)))
                    .child(c.picker.clone()),
            );
        let by = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::QuestionFieldBy)),
            )
            .child(div().relative().child(date_button).children(date_picker));
        let reveal = std::rc::Rc::clone(&self.q.reveal);
        let scroll = self.scroll.clone();
        let picker_focus = c.picker.focus_handle(cx);
        div()
            .id("q-composer")
            .relative()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .px(px(8.))
            .py(px(6.))
            .mx(px(-8.))
            .capture_action(cx.listener(|_, action: &Enter, _, cx| {
                // A question is one paragraph: Shift+Enter does nothing.
                if action.shift && !action.secondary {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(
                move |page, _: &gpui_kit::component::input::Escape, window, cx| {
                    if !picker_focus.contains_focused(window, cx) {
                        page.close_composer(window, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .capture_action(cx.listener(
                |page, _: &gpui_kit::component::input::IndentInline, window, cx| {
                    if let Some(c) = &page.q.composer
                        && c.text.read(cx).focus_handle(cx).is_focused(window)
                    {
                        c.picker.update(cx, |p, cx| p.focus(window, cx));
                        cx.stop_propagation();
                    }
                },
            ))
            .child(
                div()
                    .w(px(16.))
                    .h(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(question_view::glyph(QuestionStatus::Open, t)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px(px(8.))
                                    .rounded(px(RADIUS_SM))
                                    .bg(t.bg_raised)
                                    .border_1()
                                    .border_color(t.border_default)
                                    .child(
                                        type_scale::UI.el().child(
                                            Textarea::new(&c.text)
                                                .appearance(false)
                                                .text_size(px(type_scale::UI.size()))
                                                .line_height(px(type_scale::UI.line())),
                                        ),
                                    ),
                            )
                            .children(counter),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap_x(px(16.))
                            .gap_y(px(8.))
                            .child(asked)
                            .child(by),
                    ),
            )
            .child(ui::measure(move |bounds, window| {
                if reveal.replace(false) {
                    ui::scroll_into_view(&scroll, bounds, ui::Reveal::Whole);
                    window.refresh();
                }
            }))
            .into_any_element()
    }
}
