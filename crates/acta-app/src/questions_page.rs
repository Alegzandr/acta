//! The Questions page (docs/design/components-questions.md 5; screens.md
//! 8.22): every question of a project, an epic or a topic, grouped by
//! owner widest first, with the Open / Overdue / Closed / All segments.
//! `Enter` on a row opens its owner's page; `E` edits an open question in
//! place with the composer's three fields (components-questions.md 5).

use std::collections::{HashMap, HashSet};

use acta_core::date::Date;
use acta_core::model::{
    Author, EpicId, Owner, Place, ProjectId, Question, QuestionId, Record, TopicId,
};
use acta_core::store::QuestionFilter;
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Task, Window, div, px,
};

use crate::contact_picker::{ContactPicker, Mode, PickerEvent, Value};
use crate::due_picker::{AnchorBounds, DuePicker, DuePickerEvent};

use crate::due;
use crate::i18n::{Arg, Key, Locale, number, text, tr};
use crate::question_view::{self, RowLook};
use crate::questions_model::{self as qm, Filter};
use crate::record_page::PageEvent;
use crate::route::Route;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;
use crate::workspace::{Undo, Workspace, WriteError};

/// What the page shows, read in one go.
#[derive(Clone, Debug)]
struct Loaded {
    project: (ProjectId, String),
    questions: Vec<Question>,
    order: HashMap<Owner, usize>,
    names: HashMap<Owner, String>,
    unread: HashSet<QuestionId>,
    /// Each topic's epic: where a question's contact picker looks from.
    epic_of: HashMap<TopicId, EpicId>,
}

pub struct QuestionsPage {
    ws: Entity<Workspace>,
    pub scope: Owner,
    filter: Filter,
    data: Option<Loaded>,
    focus: FocusHandle,
    row_focus: Vec<FocusHandle>,
    /// An open question edited in place (`E`).
    edit: Option<Edit>,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

/// The in-place editor: the question, Asked, Answer by.
struct Edit {
    id: QuestionId,
    place: Place,
    text: Entity<TextareaState>,
    picker: Entity<ContactPicker>,
    date: Option<Date>,
    date_picker: Option<Entity<DuePicker>>,
    date_anchor: AnchorBounds,
    date_focus: FocusHandle,
    _subs: Vec<Subscription>,
}

/// Where a question's contact picker looks from: its owner's project and
/// epic.
fn place_of(q: &Question, epic_of: &HashMap<TopicId, EpicId>) -> Place {
    Place {
        project: q.project,
        epic: match q.owner {
            Owner::Project(_) => None,
            Owner::Epic(e) => Some(e),
            Owner::Topic(t) => epic_of.get(&t).copied(),
        },
    }
}

impl EventEmitter<PageEvent> for QuestionsPage {}

impl Focusable for QuestionsPage {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

fn load(store: &acta_core::Store, scope: Owner) -> acta_core::Result<Loaded> {
    let project = match scope {
        Owner::Project(p) => p,
        Owner::Epic(e) => store.epic(e)?.project,
        Owner::Topic(t) => store.topic(t)?.project,
    };
    let title = store.project(project)?.title;
    let tree = store.tree(project)?;
    let order = qm::owner_order(&tree);
    let mut names = HashMap::new();
    names.insert(Owner::Project(project), title.clone());
    let mut epic_of: HashMap<TopicId, EpicId> = HashMap::new();
    for t in &tree.loose_topics {
        names.insert(Owner::Topic(t.id), t.title.clone());
    }
    for (e, topics) in &tree.epics {
        names.insert(Owner::Epic(e.id), e.title.clone());
        for t in topics {
            names.insert(Owner::Topic(t.id), t.title.clone());
            epic_of.insert(t.id, e.id);
        }
    }
    let questions = store
        .find_questions(project, &QuestionFilter::default())?
        .into_iter()
        .filter(|q| qm::in_scope(q, scope, &epic_of))
        .collect();
    let unread = store
        .unread_questions()?
        .into_iter()
        .map(|(q, _)| q)
        .collect();
    Ok(Loaded {
        project: (project, title),
        questions,
        order,
        names,
        unread,
        epic_of,
    })
}

/// "Project Acta", "Epic Terminal", "Topic Embed the terminal".
pub fn owner_label(locale: Locale, owner: Owner, name: &str) -> String {
    let key = match owner {
        Owner::Project(_) => Key::LevelProject,
        Owner::Epic(_) => Key::LevelEpic,
        Owner::Topic(_) => Key::LevelTopic,
    };
    tr(locale, key, &[("name", Arg::Text(name))])
}

pub fn owner_route(owner: Owner) -> Route {
    match owner {
        Owner::Project(p) => Route::Project(p),
        Owner::Epic(e) => Route::Epic(e),
        Owner::Topic(t) => Route::Topic(t),
    }
}

/// "Logged as a decision": the topic with that entry focused; without an
/// entry (or off a topic), the owner's page.
pub fn logged_event(owner: Owner, entry: Option<acta_core::model::EntryId>) -> PageEvent {
    match (owner, entry) {
        (Owner::Topic(t), Some(e)) => PageEvent::OpenEntry(t, e),
        _ => PageEvent::Open(owner_route(owner)),
    }
}

impl QuestionsPage {
    pub fn new(
        ws: Entity<Workspace>,
        scope: Owner,
        filter: Filter,
        cx: &mut Context<Self>,
    ) -> Self {
        let subs = vec![
            cx.observe(&ws, |page, ws, cx| {
                let revision = ws.read(cx).revision;
                if revision != page.seen_revision {
                    page.seen_revision = revision;
                    page.reload(cx);
                }
            }),
            cx.observe_global::<due::Today>(|page, cx| page.reload(cx)),
        ];
        let mut page = Self {
            ws,
            scope,
            filter,
            data: None,
            focus: cx.focus_handle(),
            row_focus: Vec::new(),
            edit: None,
            seen_revision: 0,
            _load: Task::ready(()),
            _subs: subs,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let scope = self.scope;
        let task = self.ws.read(cx).db.run(cx, move |store| load(store, scope));
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok(data) = loaded {
                    page.data = Some(data);
                }
                cx.notify();
            });
        });
    }

    /// `E` on an open question: its row becomes the composer's three
    /// fields; `Enter` or `Mod+Enter` saves, `Esc` cancels.
    fn start_edit(&mut self, q: &Question, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let place = place_of(q, &data.epic_of);
        let locale = settings::locale(cx);
        let text = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 4)
                .submit_on_enter(true)
                .placeholder(text(locale, Key::QuestionPlaceholder))
                .default_value(q.text.clone())
        });
        text.update(cx, |s, cx| s.focus(window, cx));
        let value = q
            .asked_to
            .as_ref()
            .map(|c| Value::Contact(c.id, c.name.clone()))
            .unwrap_or_default();
        let ws = self.ws.clone();
        let picker = cx.new(|cx| ContactPicker::new(ws, place, value, Mode::Inline, window, cx));
        let subs = vec![
            cx.subscribe_in(
                &text,
                window,
                |page, editor, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => page.save_edit(window, cx),
                    InputEvent::Change => {
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
                    PickerEvent::Submit => page.save_edit(window, cx),
                    PickerEvent::Cancel => page.close_edit(window, cx),
                    PickerEvent::Taken(_, Some(false)) | PickerEvent::Tab(false) => {
                        if let Some(e) = &page.edit {
                            window.focus(&e.date_focus, cx);
                        }
                    }
                    PickerEvent::Tab(true) | PickerEvent::Taken(_, Some(true)) => {
                        if let Some(e) = &page.edit {
                            e.text.update(cx, |s, cx| s.focus(window, cx));
                        }
                    }
                    PickerEvent::Taken(_, None) => cx.notify(),
                },
            ),
        ];
        self.edit = Some(Edit {
            id: q.id,
            place,
            text,
            picker,
            date: q.expected_on,
            date_picker: None,
            date_anchor: AnchorBounds::default(),
            date_focus: cx.focus_handle(),
            _subs: subs,
        });
        cx.notify();
    }

    fn close_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.edit.take().map(|e| e.id);
        // The keyboard goes back to the row.
        let row = id.and_then(|id| {
            let data = self.data.as_ref()?;
            let groups = qm::page(&data.questions, self.filter, due::today(cx), &data.order);
            groups
                .iter()
                .flat_map(|g| g.questions.iter())
                .position(|q| q.id == id)
        });
        match row.and_then(|i| self.row_focus.get(i)) {
            Some(h) => window.focus(h, cx),
            None => window.focus(&self.focus, cx),
        }
        cx.notify();
    }

    /// Save the edit: one undo step (a contact made inline is created in
    /// it); a contact that moved out of sight meanwhile brings the editor
    /// back with the picker's inline error.
    fn save_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &self.edit else { return };
        let text = qm::clean_text(&e.text.read(cx).value());
        if text.is_empty() {
            return;
        }
        let value = e.picker.read(cx).value().clone();
        let (id, date, place) = (e.id, e.date, e.place);
        let name = value.name().to_owned();
        let project = Some(place.project);
        let written = text.clone();
        let task = self.ws.update(cx, |ws, cx| {
            ws.write_undoable_picked(cx, project, move |store| {
                let me = Author::Human;
                let mut steps = Vec::new();
                let asked = match value {
                    Value::None => None,
                    Value::Contact(c, _) => Some(c),
                    Value::New(name) => {
                        let c = store.create_contact(
                            &name,
                            crate::contacts_model::default_level(place.project),
                            me,
                        )?;
                        steps.push(Undo::Delete(Record::Contact(c)));
                        Some(c)
                    }
                };
                let back = store.question(id)?;
                store.edit_question(id, Some(&written), Some(asked), me)?;
                store.set_expected(id, date, me)?;
                steps.insert(
                    0,
                    Undo::Question {
                        back: Box::new(back),
                        expect: Box::new(store.question(id)?),
                    },
                );
                let undo = if steps.len() == 1 {
                    steps.remove(0)
                } else {
                    Undo::Many(steps)
                };
                Ok((undo, ()))
            })
        });
        cx.spawn_in(window, async move |page, cx| {
            if let Err(WriteError::OutOfScope) = task.await {
                let _ = page.update_in(cx, |page, window, cx| {
                    let q = page
                        .data
                        .as_ref()
                        .and_then(|d| d.questions.iter().find(|q| q.id == id).cloned());
                    let Some(q) = q else { return };
                    page.start_edit(&q, window, cx);
                    if let Some(e) = &mut page.edit {
                        e.text.update(cx, |s, cx| s.set_value(text, window, cx));
                        e.date = date;
                        e.picker
                            .update(cx, |p, cx| p.refuse(&name, place, window, cx));
                    }
                });
            }
        })
        .detach();
        self.close_edit(window, cx);
    }

    fn open_edit_date(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let today = due::today(cx);
        let Some(current) = self.edit.as_ref().map(|e| e.date) else {
            return;
        };
        let picker = cx.new(|cx| DuePicker::new(today, current, window, cx));
        let sub = cx.subscribe_in(
            &picker,
            window,
            |page, _, event: &DuePickerEvent, window, cx| {
                let DuePickerEvent::Done(outcome) = *event;
                if let Some(e) = &mut page.edit {
                    if let crate::due::Outcome::Set(date) = outcome {
                        e.date = date;
                    }
                    e.date_picker = None;
                    window.focus(&e.date_focus, cx);
                }
                cx.notify();
            },
        );
        if let Some(e) = &mut self.edit {
            e.date_picker = Some(picker);
            e._subs.push(sub);
        }
        cx.notify();
    }

    /// The three fields in place of the row.
    fn render_edit(
        &self,
        locale: Locale,
        t: &Tokens,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(e) = &self.edit else {
            return div().into_any_element();
        };
        let date_label = match e.date {
            Some(d) => due::date(locale, d),
            None => text(locale, Key::DueNone),
        };
        let date_focused = e.date_focus.is_focused(window);
        let date_button = ui::ghost_value("qp-date", t)
            .relative()
            .track_focus(&e.date_focus)
            .when(e.date.is_none(), |this| this.text_color(t.text_tertiary))
            .when(date_focused, |this| this.shadow(ui::ring_shadow(t, false)))
            .on_click(cx.listener(|page, _, window, cx| page.open_edit_date(window, cx)))
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                match k.key.as_str() {
                    "enter" | "space" if !k.modifiers.modified() => page.open_edit_date(window, cx),
                    "enter" if k.modifiers.control || k.modifiers.platform => {
                        page.save_edit(window, cx)
                    }
                    "escape" => page.close_edit(window, cx),
                    "tab" if k.modifiers.shift => {
                        if let Some(e) = &page.edit {
                            e.picker.update(cx, |p, cx| p.focus(window, cx));
                        }
                    }
                    "tab" => {
                        if let Some(e) = &page.edit {
                            e.text.update(cx, |s, cx| s.focus(window, cx));
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(date_label)
            .child(e.date_anchor.measure());
        let date_picker = e.date_picker.as_ref().and_then(|p| {
            e.date_anchor
                .below_at(p.clone(), crate::menu::Align::Start, 2)
        });
        let label = |key: Key| {
            type_scale::SMALL
                .el()
                .flex_shrink_0()
                .text_color(t.text_tertiary)
                .child(text(locale, key))
        };
        let narrow = ui::column_width() < 400.;
        let picker_focus = e.picker.focus_handle(cx);
        div()
            .id("qp-edit")
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .px(px(8.))
            .py(px(6.))
            .mx(px(-8.))
            .capture_action(cx.listener(
                move |page, _: &gpui_kit::component::input::Escape, window, cx| {
                    if !picker_focus.contains_focused(window, cx) {
                        page.close_edit(window, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .capture_action(cx.listener(
                |page, _: &gpui_kit::component::input::IndentInline, window, cx| {
                    if let Some(e) = &page.edit
                        && e.text.read(cx).focus_handle(cx).is_focused(window)
                    {
                        e.picker.update(cx, |p, cx| p.focus(window, cx));
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
                    .child(question_view::glyph(
                        acta_core::model::QuestionStatus::Open,
                        t,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        type_scale::UI
                            .el()
                            .text_color(t.text_primary)
                            .child(Textarea::new(&e.text).appearance(false)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap(px(8.))
                            .child(label(Key::QuestionFieldAsked))
                            .child(
                                div()
                                    .when(narrow, |d| d.flex_1())
                                    .when(!narrow, |d| d.w(px(240.)))
                                    .child(e.picker.clone()),
                            )
                            .child(div().w(px(8.)))
                            .child(label(Key::QuestionFieldBy))
                            .child(div().relative().child(date_button).children(date_picker)),
                    ),
            )
            .into_any_element()
    }

    fn set_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    fn scope_name(&self, locale: Locale, data: &Loaded) -> String {
        match self.scope {
            Owner::Project(_) => data.project.1.clone(),
            owner => owner_label(
                locale,
                owner,
                data.names.get(&owner).map(String::as_str).unwrap_or(""),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        q: &Question,
        index: usize,
        owner_named: bool,
        data: &Loaded,
        today: Date,
        locale: Locale,
        t: &Tokens,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = q.owner;
        let look = RowLook {
            index,
            unread: data.unread.contains(&q.id),
            owner: owner_named.then(|| {
                let label = owner_label(
                    locale,
                    owner,
                    data.names.get(&owner).map(String::as_str).unwrap_or(""),
                );
                tr(locale, Key::QuestionIn, &[("level", Arg::Text(&label))])
            }),
            show_asked: true,
            today,
        };
        if self.edit.as_ref().is_some_and(|e| e.id == q.id) {
            return self.render_edit(locale, t, window, cx);
        }
        let page = cx.entity().downgrade();
        let page2 = page.clone();
        let logged = q.decision_entry;
        let edits = q.is_open().then(|| q.clone());
        let mut row = question_view::row(
            q,
            &look,
            None,
            move |c, _, cx| {
                let _ = page.update(cx, |_, cx| cx.emit(PageEvent::Open(Route::Contact(c))));
            },
            move |_, _, cx| {
                let _ = page2.update(cx, |_, cx| cx.emit(logged_event(owner, logged)));
            },
            locale,
            t,
        )
        .on_click(cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(owner_route(owner)))))
        .on_key_down(cx.listener(move |page, event: &KeyDownEvent, window, cx| {
            let k = &event.keystroke;
            if k.modifiers.modified() {
                return;
            }
            match k.key.as_str() {
                // `E` edits an open question in place; a closed one (its
                // answer) is edited on its owner's page.
                "e" if edits.is_some() => {
                    if let Some(q) = &edits {
                        page.start_edit(q, window, cx);
                    }
                }
                "enter" | "e" => cx.emit(PageEvent::Open(owner_route(owner))),
                "up" | "down" => {
                    let to = if k.key == "up" {
                        index.checked_sub(1)
                    } else {
                        Some(index + 1)
                    };
                    if let Some(h) = to.and_then(|i| page.row_focus.get(i)) {
                        window.focus(h, cx);
                    }
                }
                _ => return,
            }
            cx.stop_propagation();
        }));
        if let Some(h) = self.row_focus.get(index) {
            row = row.track_focus(h);
        }
        row.into_any_element()
    }
}

impl Render for QuestionsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let today = due::today(cx);
        let mut column = ui::column().child(
            type_scale::TITLE
                .el()
                .text_color(t.text_primary)
                .child(text(locale, Key::SectionQuestions)),
        );
        let Some(data) = self.data.clone() else {
            return ui::page_scroll("questions-page")
                .track_focus(&self.focus)
                .child(column);
        };
        let (open, closed) = qm::counts(&data.questions);
        let counts = format!(
            "{}, {}",
            tr(
                locale,
                Key::CountQuestionsOpen,
                &[("n", Arg::Count(open as u64))]
            ),
            tr(
                locale,
                Key::CountQuestionsClosed,
                &[("n", Arg::Count(closed as u64))]
            )
        );
        let scope_line = tr(
            locale,
            Key::QuestionsScope,
            &[
                ("scope", Arg::Text(&self.scope_name(locale, &data))),
                ("counts", Arg::Text(&counts)),
            ],
        );
        let project = data.project.0;
        let narrower = !matches!(self.scope, Owner::Project(_));
        column = column.child(
            type_scale::SMALL
                .el()
                .mt(px(8.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .text_color(t.text_tertiary)
                .font_features(type_scale::tabular())
                .child(scope_line)
                .when(narrower, |this| {
                    this.child(
                        div()
                            .id("q-whole")
                            .text_color(t.accent)
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(PageEvent::Open(Route::Questions(Owner::Project(project))))
                            }))
                            .child(text(locale, Key::QuestionsWholeProject)),
                    )
                }),
        );
        // Overdue only while an open question is late; the filter falls
        // back to Open when it goes.
        let late = qm::has_overdue(&data.questions, today);
        if self.filter == Filter::Overdue && !late {
            self.filter = Filter::Open;
        }
        let mut seg = ui::segmented(&t);
        for f in Filter::ALL {
            if f == Filter::Overdue && !late {
                continue;
            }
            let key = match f {
                Filter::Open => Key::TableFilterOpen,
                Filter::Overdue => Key::TableFilterOverdue,
                Filter::Closed => Key::TableFilterClosed,
                Filter::All => Key::TableFilterAll,
            };
            seg = seg.child(
                ui::segment(
                    ("q-seg", f as usize),
                    text(locale, key),
                    self.filter == f,
                    &t,
                )
                .on_click(cx.listener(move |page, _, _, cx| page.set_filter(f, cx))),
            );
        }
        column = column.child(
            div()
                .mt(px(24.))
                .h(px(28.))
                .flex()
                .items_center()
                .child(seg),
        );
        let groups = qm::page(&data.questions, self.filter, today, &data.order);
        let rows: usize = groups.iter().map(|g| g.questions.len()).sum();
        while self.row_focus.len() < rows {
            self.row_focus.push(cx.focus_handle());
        }
        if groups.is_empty() {
            let empty = match self.filter {
                Filter::Open | Filter::All => tr(
                    locale,
                    Key::QuestionsNoneOpen,
                    &[("name", Arg::Text(&self.scope_name(locale, &data)))],
                ),
                Filter::Overdue => text(locale, Key::QuestionsNoneOverdue),
                Filter::Closed => text(locale, Key::QuestionsNoneClosed),
            };
            column = column.child(
                type_scale::READING
                    .el()
                    .mt(px(16.))
                    .text_color(t.text_secondary)
                    .child(empty),
            );
        }
        let mut index = 0;
        for group in &groups {
            if let Some(owner) = group.owner {
                let name = data.names.get(&owner).cloned().unwrap_or_default();
                let hover = t.fill_hover;
                column = column.child(
                    div()
                        .mt(px(16.))
                        .h(px(28.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .id(("q-group", index))
                                .px(px(4.))
                                .mx(px(-4.))
                                .rounded(px(RADIUS_SM))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(PageEvent::Open(owner_route(owner)))
                                }))
                                .child(
                                    type_scale::LABEL
                                        .el()
                                        .text_color(t.text_secondary)
                                        .child(owner_label(locale, owner, &name)),
                                ),
                        )
                        .child(div().flex_1())
                        .child(
                            type_scale::SMALL
                                .el()
                                .text_color(t.text_tertiary)
                                .font_features(type_scale::tabular())
                                .child(number(locale, group.questions.len() as u64)),
                        ),
                );
            } else {
                column = column.child(div().mt(px(16.)));
            }
            for q in &group.questions {
                column = column.child(self.row(
                    q,
                    index,
                    group.owner.is_none(),
                    &data,
                    today,
                    locale,
                    &t,
                    window,
                    cx,
                ));
                index += 1;
            }
        }
        ui::page_scroll("questions-page")
            .track_focus(&self.focus)
            .child(column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{EntryId, EpicId, QuestionStatus, Stamp, TopicId};

    fn sample() -> Question {
        Question {
            id: QuestionId(1),
            project: ProjectId(1),
            owner: Owner::Project(ProjectId(1)),
            text: "Can Alexandre keep the v1 export until March?".into(),
            asked_to: None,
            expected_on: None,
            status: QuestionStatus::Open,
            answer: None,
            resolved_at: None,
            resolved_by: None,
            decision_entry: None,
            position: 0,
            created_by: Author::Human,
            stamp: Stamp {
                uuid: String::new(),
                created_at: 0,
                updated_at: 0,
                updated_by: Author::Human,
                deleted_at: None,
                deleted_by: None,
            },
        }
    }

    #[test]
    fn a_question_is_edited_from_the_place_of_its_owner() {
        let mut epics = HashMap::new();
        epics.insert(TopicId(4), EpicId(2));
        let q = |owner| Question { owner, ..sample() };
        let project = sample().project;
        assert_eq!(
            place_of(&q(Owner::Topic(TopicId(4))), &epics),
            Place {
                project,
                epic: Some(EpicId(2))
            }
        );
        assert_eq!(
            place_of(&q(Owner::Topic(TopicId(5))), &epics),
            Place {
                project,
                epic: None
            }
        );
        assert_eq!(
            place_of(&q(Owner::Epic(EpicId(3))), &epics).epic,
            Some(EpicId(3))
        );
    }

    #[test]
    fn the_logged_link_focuses_the_entry_on_its_topic() {
        assert!(matches!(
            logged_event(Owner::Topic(TopicId(4)), Some(EntryId(9))),
            PageEvent::OpenEntry(TopicId(4), EntryId(9))
        ));
        assert!(matches!(
            logged_event(Owner::Topic(TopicId(4)), None),
            PageEvent::Open(Route::Topic(TopicId(4)))
        ));
        assert!(matches!(
            logged_event(Owner::Epic(EpicId(2)), Some(EntryId(9))),
            PageEvent::Open(Route::Epic(EpicId(2)))
        ));
    }
}
