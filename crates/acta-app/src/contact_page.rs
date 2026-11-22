//! The contact page (docs/design/components-contacts.md 5; screens.md
//! 8.23): the name, the details as properties shown only when set, the
//! duplicate notice, how to address them, and what is open with them over
//! the contact's scope: the questions asked of them and what is to be
//! reported to them. Keyed by the contact's id, so it comes back after a
//! restart.

use std::collections::HashMap;

use acta_core::model::{
    Contact, ContactId, ContactLevel, Note, Owner, ProjectId, Question, Record, Status,
    TopicSummary,
};
use acta_core::store::{ContactPatch, ContactSummary, QuestionFilter};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Task, Window, div, px,
};

use crate::contacts_model as cm;
use crate::contacts_page;
use crate::due;
use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::menu::{Align, AnchorBounds};
use crate::question_view::{self, RowLook};
use crate::questions_page::{owner_label, owner_route};
use crate::record_page::PageEvent;
use crate::route::Route;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{Undo, Workspace, put_report, report_state};

/// One thing to report: a topic, an epic or a note.
#[derive(Clone, Debug)]
enum ReportItem {
    Topic(TopicSummary),
    Epic(acta_core::model::EpicSummary),
    Note(Note),
}

#[derive(Clone, Debug, Default)]
struct Loaded {
    contact: Option<Contact>,
    all: Vec<ContactSummary>,
    distinct: Vec<(ContactId, ContactId)>,
    asked: Vec<Question>,
    names: HashMap<Owner, String>,
    /// Not yet reported, then reported.
    to_report: Vec<ReportItem>,
    reported: Vec<ReportItem>,
    /// Projects with something open with them.
    projects_used: usize,
}

fn load(store: &acta_core::Store, id: ContactId) -> acta_core::Result<Loaded> {
    let contact = store.contact_any(id)?;
    let all = store.contacts_all()?;
    let key = acta_core::person::person_key(&contact.name);
    let mut distinct = Vec::new();
    for c in &all {
        if c.contact.id != id
            && acta_core::person::person_key(&c.contact.name) == key
            && store.is_distinct(id, c.contact.id)?
        {
            distinct.push((id, c.contact.id));
        }
    }
    let projects: Vec<ProjectId> = match contact.level.project() {
        Some(p) => vec![p],
        None => store.projects()?.into_iter().map(|p| p.id).collect(),
    };
    let mut asked = Vec::new();
    let mut names = HashMap::new();
    let mut to_report = Vec::new();
    let mut reported = Vec::new();
    let mut used = std::collections::HashSet::new();
    for p in projects {
        let tree = store.tree(p)?;
        names.insert(Owner::Project(p), tree.project.title.clone());
        for t in &tree.loose_topics {
            names.insert(Owner::Topic(t.id), t.title.clone());
        }
        for (e, topics) in &tree.epics {
            names.insert(Owner::Epic(e.id), e.title.clone());
            for t in topics {
                names.insert(Owner::Topic(t.id), t.title.clone());
            }
        }
        let qs = store.find_questions(
            p,
            &QuestionFilter {
                contact: Some(id),
                ..QuestionFilter::default()
            },
        )?;
        if qs.iter().any(Question::is_open) {
            used.insert(p);
        }
        asked.extend(qs);
        let names_it =
            |c: &Option<acta_core::model::ContactRef>| c.as_ref().is_some_and(|c| c.id == id);
        for t in tree
            .loose_topics
            .iter()
            .chain(tree.epics.iter().flat_map(|(_, ts)| ts))
            .filter(|t| names_it(&t.report_to))
        {
            if t.reported {
                reported.push(ReportItem::Topic(t.clone()));
            } else if t.status != Status::Dropped {
                used.insert(p);
                to_report.push(ReportItem::Topic(t.clone()));
            }
        }
        for (e, _) in tree.epics.iter().filter(|(e, _)| names_it(&e.report_to)) {
            if e.reported {
                reported.push(ReportItem::Epic(e.clone()));
            } else {
                used.insert(p);
                to_report.push(ReportItem::Epic(e.clone()));
            }
        }
        for n in store.report_targets(p, id)?.notes {
            used.insert(p);
            to_report.push(ReportItem::Note(n));
        }
    }
    Ok(Loaded {
        contact: Some(contact),
        all,
        distinct,
        asked,
        names,
        to_report,
        reported,
        projects_used: used.len(),
    })
}

/// A detail being edited in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Name,
    JobTitle,
    Company,
}

pub struct ContactPage {
    ws: Entity<Workspace>,
    pub id: ContactId,
    data: Loaded,
    gone: bool,
    edit: Option<(Field, Entity<InputState>)>,
    address: Option<Entity<TextareaState>>,
    /// The page's menu (Rename, Move to..., Merge into..., Delete).
    menu: Option<usize>,
    menu_anchor: AnchorBounds,
    asked_closed: bool,
    reported_open: bool,
    props_hover: bool,
    name_error: bool,
    focus: FocusHandle,
    row_focus: Vec<FocusHandle>,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PageEvent> for ContactPage {}

impl Focusable for ContactPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuItem {
    Rename,
    Move,
    Merge,
    Delete,
}

const MENU: [MenuItem; 4] = [
    MenuItem::Rename,
    MenuItem::Move,
    MenuItem::Merge,
    MenuItem::Delete,
];

impl ContactPage {
    pub fn new(ws: Entity<Workspace>, id: ContactId, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&ws, |page, ws, cx| {
            let revision = ws.read(cx).revision;
            if revision != page.seen_revision {
                page.seen_revision = revision;
                page.reload(cx);
            }
        })];
        let mut page = Self {
            ws,
            id,
            data: Loaded::default(),
            gone: false,
            edit: None,
            address: None,
            menu: None,
            menu_anchor: AnchorBounds::default(),
            asked_closed: false,
            reported_open: false,
            props_hover: false,
            name_error: false,
            focus: cx.focus_handle(),
            row_focus: Vec::new(),
            seen_revision: 0,
            _load: Task::ready(()),
            _subs: subs,
        };
        page.reload(cx);
        page
    }

    /// The name, for the tab.
    pub fn title(&self) -> Option<String> {
        self.data.contact.as_ref().map(|c| c.name.clone())
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let id = self.id;
        let task = self.ws.read(cx).db.run(cx, move |store| {
            let loaded = load(store, id)?;
            // Shown: its unread dot goes on the Contacts page.
            store.mark_seen(Record::Contact(id))?;
            Ok(loaded)
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                match loaded {
                    Ok(data) => {
                        page.gone = data
                            .contact
                            .as_ref()
                            .is_some_and(|c| c.stamp.deleted_at.is_some());
                        page.data = data;
                    }
                    Err(_) => page.gone = true,
                }
                cx.notify();
            });
        });
    }

    fn contact(&self) -> Option<&Contact> {
        self.data.contact.as_ref()
    }

    fn start_edit(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.contact() else { return };
        let value = match field {
            Field::Name => c.name.clone(),
            Field::JobTitle => c.job_title.clone().unwrap_or_default(),
            Field::Company => c.company.clone().unwrap_or_default(),
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        input.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.subscribe_in(
            &input,
            window,
            move |page, input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    let value = input.read(cx).value().trim().to_owned();
                    page.commit_edit(field, value, window, cx);
                }
                _ => {}
            },
        );
        self._subs.push(sub);
        self.edit = Some((field, input));
        self.name_error = false;
        cx.notify();
    }

    fn commit_edit(
        &mut self,
        field: Field,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.as_ref().map(|(f, _)| *f) != Some(field) {
            return;
        }
        let Some(c) = self.contact().cloned() else {
            return;
        };
        let patch = match field {
            Field::Name => {
                if value.is_empty() {
                    // "A contact needs a name.": the old name is kept.
                    self.name_error = true;
                    self.edit = None;
                    cx.notify();
                    return;
                }
                (value != c.name).then(|| ContactPatch {
                    name: Some(value.chars().take(cm::NAME_MAX).collect()),
                    ..ContactPatch::default()
                })
            }
            Field::JobTitle => (Some(value.as_str()) != c.job_title.as_deref().or(Some(""))
                || value.is_empty() != c.job_title.is_none())
            .then(|| ContactPatch {
                job_title: Some((!value.is_empty()).then(|| value.clone())),
                ..ContactPatch::default()
            }),
            Field::Company => (Some(value.as_str()) != c.company.as_deref().or(Some(""))
                || value.is_empty() != c.company.is_none())
            .then(|| ContactPatch {
                company: Some((!value.is_empty()).then(|| value.clone())),
                ..ContactPatch::default()
            }),
        };
        self.edit = None;
        if let Some(patch) = patch {
            let ws = self.ws.clone();
            contacts_page::patch(&ws, c.id, patch, cx);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn start_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.contact() else { return };
        let locale = settings::locale(cx);
        let value = c.address_note.clone().unwrap_or_default();
        let editor = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 12)
                .placeholder(text(locale, Key::ContactAddressPlaceholder))
                .default_value(value)
        });
        editor.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.subscribe_in(
            &editor,
            window,
            |page, editor, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter {
                    secondary: true, ..
                } => {
                    let note = editor.read(cx).value().trim().to_owned();
                    page.save_address(note, window, cx);
                }
                InputEvent::Change => {
                    let v = editor.read(cx).value().to_string();
                    if v.chars().count() > cm::ADDRESS_MAX {
                        let cut: String = v.chars().take(cm::ADDRESS_MAX).collect();
                        editor.update(cx, |s, cx| s.set_value(cut, window, cx));
                    }
                    cx.notify();
                }
                _ => {}
            },
        );
        self._subs.push(sub);
        self.address = Some(editor);
        cx.notify();
    }

    fn save_address(&mut self, note: String, window: &mut Window, cx: &mut Context<Self>) {
        self.address = None;
        let Some(c) = self.contact().cloned() else {
            return;
        };
        if c.address_note.as_deref().unwrap_or("") != note {
            let ws = self.ws.clone();
            contacts_page::patch(
                &ws,
                c.id,
                ContactPatch {
                    address_note: Some((!note.is_empty()).then_some(note)),
                    ..ContactPatch::default()
                },
                cx,
            );
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn choose(&mut self, item: MenuItem, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let ws = self.ws.clone();
        match item {
            MenuItem::Rename => self.start_edit(Field::Name, window, cx),
            MenuItem::Move => contacts_page::pick_level(&ws, self.id, cx),
            MenuItem::Merge => contacts_page::pick_merge(&ws, self.id, cx),
            MenuItem::Delete => contacts_page::delete(&ws, self.id, cx),
        }
        cx.notify();
    }

    /// `Space` on a To report row: mark it reported (or not), one step.
    fn toggle_reported(&mut self, record: Record, reported: bool, cx: &mut Context<Self>) {
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, None, None, move |store| {
                let back = report_state(store, record)?;
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

    // ---- rendering ----

    fn render_title(
        &self,
        c: &Contact,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some((Field::Name, input)) = &self.edit {
            return type_scale::TITLE
                .el()
                .border_b_1()
                .border_color(t.border_strong)
                .child(ui::role_input(Input::new(input), type_scale::TITLE))
                .into_any_element();
        }
        let menu = self.menu.map(|hl| {
            let mut panel = crate::menu::panel("contact-page-menu", t).on_mouse_down_out(
                cx.listener(|page, _, _, cx| {
                    page.menu = None;
                    cx.notify();
                }),
            );
            for (i, item) in MENU.iter().enumerate() {
                let (label, hint) = match item {
                    MenuItem::Rename => (text(locale, Key::ContactRename), Some("F2")),
                    MenuItem::Move => (text(locale, Key::ContactMove), Some("M")),
                    MenuItem::Merge => (text(locale, Key::ContactMerge), None),
                    MenuItem::Delete => (text(locale, Key::ContactDelete), None),
                };
                if *item == MenuItem::Delete {
                    panel = panel.child(crate::menu::separator(t));
                }
                let item = *item;
                panel = panel.child(
                    crate::menu::item(
                        ("contact-page-item", i),
                        None,
                        label,
                        hint.map(|h| crate::menu::hint(h, t)),
                        i == hl,
                        t,
                    )
                    .on_click(
                        cx.listener(move |page, _, window, cx| page.choose(item, window, cx)),
                    ),
                );
            }
            self.menu_anchor
                .below(panel, Align::End)
                .unwrap_or_else(|| div().into_any_element())
        });
        div()
            .id("contact-title-row")
            .relative()
            .group("contact-title")
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .child(
                type_scale::TITLE
                    .el()
                    .id("contact-title")
                    .flex_1()
                    .min_w_0()
                    .text_color(t.text_primary)
                    .cursor_text()
                    .on_click(
                        cx.listener(|page, _, window, cx| page.start_edit(Field::Name, window, cx)),
                    )
                    .child(c.name.clone()),
            )
            .child(
                div()
                    .relative()
                    .invisible()
                    .group_hover("contact-title", |s| s.visible())
                    .when(self.menu.is_some(), |d| d.visible())
                    .child(
                        ui::ghost_icon_button(
                            "contact-menu-button",
                            Icon::Ellipsis,
                            28.,
                            false,
                            false,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|page, _, _, cx| {
                            page.menu = Some(0);
                            cx.notify();
                        })),
                    ),
            )
            .child(self.menu_anchor.measure())
            .children(menu)
            .into_any_element()
    }

    fn render_properties(
        &self,
        c: &Contact,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let projects = self.ws.read(cx).projects.clone();
        let mut rows: Vec<(String, AnyElement)> = Vec::new();
        let field_value = |page: &Self,
                           field: Field,
                           value: &str,
                           id: &'static str,
                           cx: &mut Context<Self>|
         -> AnyElement {
            if let Some((f, input)) = &page.edit
                && *f == field
            {
                return div().flex_1().child(Input::new(input)).into_any_element();
            }
            ui::ghost_value(id, t)
                .on_click(
                    cx.listener(move |page, _, window, cx| page.start_edit(field, window, cx)),
                )
                .child(value.to_owned())
                .into_any_element()
        };
        let editing = |f: Field| self.edit.as_ref().is_some_and(|(x, _)| *x == f);
        if c.job_title.is_some() || editing(Field::JobTitle) {
            rows.push((
                text(locale, Key::ContactPropJobTitle),
                field_value(
                    self,
                    Field::JobTitle,
                    c.job_title.as_deref().unwrap_or(""),
                    "contact-job",
                    cx,
                ),
            ));
        }
        if c.company.is_some() || editing(Field::Company) {
            rows.push((
                text(locale, Key::ContactPropCompany),
                field_value(
                    self,
                    Field::Company,
                    c.company.as_deref().unwrap_or(""),
                    "contact-company",
                    cx,
                ),
            ));
        }
        let level_name = cm::level_name(locale, c.level, &projects);
        let level: AnyElement = match c.level {
            ContactLevel::All => div().child(level_name).into_any_element(),
            ContactLevel::Project(p) => {
                question_view::name_link("contact-level", &level_name, false, locale, t)
                    .on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Project(p)))),
                    )
                    .into_any_element()
            }
            ContactLevel::Epic(_, e) => {
                question_view::name_link("contact-level", &level_name, false, locale, t)
                    .on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Epic(e)))),
                    )
                    .into_any_element()
            }
        };
        rows.push((text(locale, Key::ContactPropLevel), level));
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        let width = ui::label_width(&labels, cx);
        let chips: Vec<(Key, &'static str, u8)> = [
            (
                c.job_title.is_none() && !editing(Field::JobTitle),
                Key::ContactAddJobTitle,
                "chip-job",
                0u8,
            ),
            (
                c.company.is_none() && !editing(Field::Company),
                Key::ContactAddCompany,
                "chip-company",
                1,
            ),
            (
                c.address_note.is_none() && self.address.is_none(),
                Key::ContactAddAddress,
                "chip-address",
                2,
            ),
        ]
        .into_iter()
        .filter(|(on, ..)| *on)
        .map(|(_, k, id, n)| (k, id, n))
        .collect();
        let hover = t.fill_hover;
        let chips = (self.props_hover && !chips.is_empty()).then(|| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .children(chips.into_iter().map(|(key, id, n)| {
                    ui::focusable(type_scale::SMALL.el().id(id), t, false)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.))
                        .h(px(24.))
                        .px(px(8.))
                        .rounded(px(RADIUS_SM))
                        .text_color(t.text_tertiary)
                        .hover(move |s| s.bg(hover))
                        .on_click(cx.listener(move |page, _, window, cx| match n {
                            0 => page.start_edit(Field::JobTitle, window, cx),
                            1 => page.start_edit(Field::Company, window, cx),
                            _ => page.start_address(window, cx),
                        }))
                        .child(icon(Icon::Plus, 14., t.text_tertiary))
                        .child(text(locale, key))
                }))
        });
        let added = when::date(locale, c.stamp.created_at);
        let author = when::author(locale, c.created_by);
        let meta = if c.stamp.updated_at > c.stamp.created_at + 60_000 {
            tr(
                locale,
                Key::ContactMetaUpdated,
                &[
                    ("id", Arg::Text(&c.id.to_string())),
                    ("date", Arg::Text(&added)),
                    ("author", Arg::Text(&author)),
                    ("when", Arg::Text(&when::long(locale, c.stamp.updated_at))),
                ],
            )
        } else {
            tr(
                locale,
                Key::ContactMeta,
                &[
                    ("id", Arg::Text(&c.id.to_string())),
                    ("date", Arg::Text(&added)),
                    ("author", Arg::Text(&author)),
                ],
            )
        };
        div()
            .id("contact-properties")
            .mt(px(12.))
            .flex()
            .flex_col()
            .on_hover(cx.listener(|page, hovered: &bool, _, cx| {
                if page.props_hover != *hovered {
                    page.props_hover = *hovered;
                    cx.notify();
                }
            }))
            .children(rows.into_iter().map(|(l, v)| ui::property(l, width, v, t)))
            .children(chips)
            .child(
                type_scale::SMALL
                    .el()
                    .mt(px(4.))
                    .min_h(px(20.))
                    .text_color(t.text_tertiary)
                    .font_features(type_scale::tabular())
                    .child(meta),
            )
            .into_any_element()
    }

    fn render_report_row(
        &self,
        item: &ReportItem,
        index: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (glyph, title, right, record, route, reported): (
            AnyElement,
            String,
            String,
            Option<Record>,
            Option<Route>,
            bool,
        ) = match item {
            ReportItem::Topic(topic) => (
                ui::status_glyph(topic.status, 14., t).into_any_element(),
                topic.title.clone(),
                if topic.reported {
                    String::new()
                } else if topic.status == Status::Done {
                    text(locale, Key::ReportPending)
                } else {
                    String::new()
                },
                Some(Record::Topic(topic.id)),
                Some(Route::Topic(topic.id)),
                topic.reported,
            ),
            ReportItem::Epic(epic) => (
                icon(Icon::Layers, 14., t.text_secondary).into_any_element(),
                epic.title.clone(),
                tr(
                    locale,
                    Key::TableGroupProgress,
                    &[
                        ("done", Arg::Count(epic.progress.done as u64)),
                        ("total", Arg::Count(epic.progress.total as u64)),
                    ],
                ),
                Some(Record::Epic(epic.id)),
                Some(Route::Epic(epic.id)),
                epic.reported,
            ),
            ReportItem::Note(note) => (
                icon(Icon::StickyNote, 14., t.text_secondary).into_any_element(),
                note.text.lines().next().unwrap_or("").to_owned(),
                when::compact(locale, note.stamp.created_at),
                None,
                Some(Route::Inbox(note.project)),
                false,
            ),
        };
        let hover = t.fill_hover;
        let mut row = ui::focusable(div().id(("contact-report", index)), t, false)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(8.))
            .mx(px(-8.))
            .rounded(px(RADIUS_SM))
            .hover(move |s| s.bg(hover))
            .when_some(route, |this, route| {
                this.on_click(cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(route))))
            })
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, _, cx| {
                let k = &event.keystroke;
                match k.key.as_str() {
                    "enter" if !k.modifiers.modified() => {
                        if let Some(route) = route {
                            cx.emit(PageEvent::Open(route));
                        }
                    }
                    "space" if !k.modifiers.modified() => {
                        if let Some(record) = record {
                            page.toggle_reported(record, !reported, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(div().w(px(16.)).flex().justify_center().child(glyph))
            .child(
                type_scale::UI
                    .el()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(if reported {
                        t.text_secondary
                    } else {
                        t.text_primary
                    })
                    .child(title),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .text_color(t.text_secondary)
                    .font_features(type_scale::tabular())
                    .child(right),
            );
        if let Some(h) = self.row_focus.get(index) {
            row = row.track_focus(h);
        }
        row.into_any_element()
    }

    fn folded_row(
        id: &'static str,
        label: String,
        open: bool,
        t: &Tokens,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let hover = t.fill_hover;
        ui::focusable(div().id(id), t, false)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(8.))
            .mx(px(-8.))
            .rounded(px(RADIUS_SM))
            .hover(move |s| s.bg(hover))
            .child(icon(
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                14.,
                t.text_tertiary,
            ))
            .child(
                type_scale::SMALL
                    .el()
                    .text_color(t.text_tertiary)
                    .child(label),
            )
    }
}

fn section_header(label: String, t: &Tokens) -> gpui_kit::Div {
    div()
        .mt(px(32.))
        .h(px(28.))
        .flex()
        .items_center()
        .child(ui::section_header(label, t))
}

impl Render for ContactPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let today = due::today(cx);
        let mut column = ui::column();
        let Some(c) = self.data.contact.clone() else {
            return ui::page_scroll("contact-page")
                .track_focus(&self.focus)
                .child(column);
        };
        let projects = self.ws.read(cx).projects.clone();
        if self.gone {
            column = column.child(
                ui::notice(
                    Some((Icon::Info, t.text_secondary)),
                    text(locale, Key::ContactInTrash),
                    &t,
                )
                .mb(px(16.)),
            );
        }
        column = column.child(self.render_title(&c, locale, &t, cx));
        if self.name_error {
            column = column.child(
                type_scale::SMALL
                    .el()
                    .mt(px(4.))
                    .flex()
                    .flex_row()
                    .gap(px(4.))
                    .text_color(t.danger)
                    .child(icon(Icon::TriangleAlert, 14., t.danger))
                    .child(text(locale, Key::ContactNameRequired)),
            );
        }
        column = column.child(self.render_properties(&c, locale, &t, cx));
        // The scope line.
        let open_asked = self.data.asked.iter().filter(|q| q.is_open()).count();
        let open_report = self.data.to_report.len();
        let asked = tr(
            locale,
            Key::CountPersonAsked,
            &[("n", Arg::Count(open_asked as u64))],
        );
        let report = tr(
            locale,
            Key::CountPersonReport,
            &[("n", Arg::Count(open_report as u64))],
        );
        let scope = if c.level == ContactLevel::All && self.data.projects_used > 1 {
            tr(
                locale,
                Key::ContactScopeProjects,
                &[
                    ("asked", Arg::Text(&asked)),
                    ("report", Arg::Text(&report)),
                    ("n", Arg::Count(self.data.projects_used as u64)),
                ],
            )
        } else {
            tr(
                locale,
                Key::ContactScope,
                &[("asked", Arg::Text(&asked)), ("report", Arg::Text(&report))],
            )
        };
        column = column.child(
            type_scale::SMALL
                .el()
                .mt(px(8.))
                .text_color(t.text_tertiary)
                .font_features(type_scale::tabular())
                .child(scope),
        );
        // The duplicate notice.
        let dupes: Vec<Contact> = cm::duplicates(&c, &self.data.all, &self.data.distinct)
            .into_iter()
            .cloned()
            .collect();
        if let Some(other) = dupes.first().cloned()
            && !self.gone
        {
            let level = cm::level_name(locale, other.level, &projects);
            let other_id = other.id;
            let me = c.id;
            column = column.child(
                div().mt(px(24.)).child(
                    ui::notice(
                        Some((Icon::Info, t.text_secondary)),
                        tr(
                            locale,
                            Key::ContactDuplicate,
                            &[
                                ("name", Arg::Text(&other.name)),
                                ("level", Arg::Text(&level)),
                            ],
                        ),
                        &t,
                    )
                    .child(
                        ui::button(
                            "dup-merge",
                            text(locale, Key::ContactMergeIntoIt),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(move |page, _, _, cx| {
                            let ws = page.ws.clone();
                            let projects = ws.read(cx).projects.clone();
                            let others = vec![other.clone()];
                            contacts_page::open_merge_picker(
                                &ws, me, others, locale, &projects, cx,
                            );
                        })),
                    )
                    .child(
                        ui::button(
                            "dup-keep",
                            text(locale, Key::ContactKeepBoth),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(move |page, _, _, cx| {
                            page.ws.update(cx, |ws, cx| {
                                ws.write_detached(cx, Key::ErrorSave, move |store| {
                                    store.mark_distinct(me, other_id)
                                })
                            });
                        })),
                    ),
                ),
            );
        }
        // How to address them.
        if let Some(editor) = &self.address {
            let n = editor.read(cx).value().chars().count();
            column = column
                .child(section_header(text(locale, Key::ContactSectionAddress), &t))
                .child(
                    div()
                        .px(px(8.))
                        .rounded(px(RADIUS_SM))
                        .bg(t.bg_raised)
                        .border_1()
                        .border_color(t.border_default)
                        .child(
                            type_scale::READING.el().child(
                                Textarea::new(editor)
                                    .appearance(false)
                                    .text_size(px(type_scale::READING.size()))
                                    .line_height(px(type_scale::READING.line())),
                            ),
                        ),
                )
                .when(n > cm::ADDRESS_COUNTER, |col| {
                    col.child(
                        type_scale::SMALL
                            .el()
                            .mt(px(4.))
                            .text_color(t.text_tertiary)
                            .font_features(type_scale::tabular())
                            .child(tr(
                                locale,
                                Key::ContactAddressCounter,
                                &[("n", Arg::Count(n as u64))],
                            )),
                    )
                });
        } else if let Some(note) = &c.address_note {
            column = column
                .child(section_header(text(locale, Key::ContactSectionAddress), &t))
                .child(
                    type_scale::READING
                        .el()
                        .id("contact-address")
                        .whitespace_normal()
                        .text_color(t.text_primary)
                        .cursor_text()
                        .on_click(cx.listener(|page, _, window, cx| page.start_address(window, cx)))
                        .children(note.lines().map(|l| div().child(l.to_owned()))),
                );
        }
        // What is open with them.
        let rows = self.data.asked.len() + self.data.to_report.len() + self.data.reported.len();
        while self.row_focus.len() < rows {
            self.row_focus.push(cx.focus_handle());
        }
        let (open_q, closed_q): (Vec<Question>, Vec<Question>) =
            self.data.asked.iter().cloned().partition(Question::is_open);
        if open_q.is_empty()
            && closed_q.is_empty()
            && self.data.to_report.is_empty()
            && self.data.reported.is_empty()
        {
            column = column.child(
                type_scale::READING
                    .el()
                    .mt(px(32.))
                    .text_color(t.text_secondary)
                    .child(tr(
                        locale,
                        Key::PersonEmpty,
                        &[("name", Arg::Text(&c.name))],
                    )),
            );
        }
        let mut index = 0;
        if !open_q.is_empty() || !closed_q.is_empty() {
            column = column.child(section_header(text(locale, Key::PersonSectionAsked), &t));
            let several = c.level == ContactLevel::All && self.data.projects_used > 1;
            let mut shown: Vec<Question> = open_q.clone();
            if self.asked_closed {
                shown.extend(closed_q.iter().cloned());
            }
            for (i, q) in shown.iter().enumerate() {
                if i == open_q.len() && !closed_q.is_empty() {
                    column = column.child(
                        Self::folded_row(
                            "asked-closed",
                            tr(
                                locale,
                                Key::QuestionsClosed,
                                &[("n", Arg::Count(closed_q.len() as u64))],
                            ),
                            true,
                            &t,
                        )
                        .on_click(cx.listener(|page, _, _, cx| {
                            page.asked_closed = false;
                            cx.notify();
                        })),
                    );
                }
                let owner = q.owner;
                let name = self.data.names.get(&owner).cloned().unwrap_or_default();
                let mut owner_line = tr(
                    locale,
                    Key::QuestionIn,
                    &[("level", Arg::Text(&owner_label(locale, owner, &name)))],
                );
                if several {
                    let project = self
                        .data
                        .names
                        .get(&Owner::Project(q.project))
                        .cloned()
                        .unwrap_or_default();
                    owner_line = format!(
                        "{owner_line} \u{b7} {}",
                        tr(
                            locale,
                            Key::QuestionIn,
                            &[(
                                "level",
                                Arg::Text(&owner_label(
                                    locale,
                                    Owner::Project(q.project),
                                    &project
                                ))
                            )]
                        )
                    );
                }
                let look = RowLook {
                    index,
                    unread: false,
                    owner: Some(owner_line),
                    show_asked: false,
                    today,
                };
                let page = cx.entity().downgrade();
                let mut row = question_view::row(
                    q,
                    &look,
                    None,
                    |_, _, _| {},
                    move |_, _, cx| {
                        let _ =
                            page.update(cx, |_, cx| cx.emit(PageEvent::Open(owner_route(owner))));
                    },
                    locale,
                    &t,
                )
                .on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(owner_route(owner)))),
                )
                .on_key_down(cx.listener(move |_, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter" && !event.keystroke.modifiers.modified() {
                        cx.emit(PageEvent::Open(owner_route(owner)));
                        cx.stop_propagation();
                    }
                }));
                if let Some(h) = self.row_focus.get(index) {
                    row = row.track_focus(h);
                }
                column = column.child(row);
                index += 1;
            }
            if !self.asked_closed && !closed_q.is_empty() {
                column = column.child(
                    Self::folded_row(
                        "asked-closed",
                        tr(
                            locale,
                            Key::QuestionsClosed,
                            &[("n", Arg::Count(closed_q.len() as u64))],
                        ),
                        false,
                        &t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.asked_closed = true;
                        cx.notify();
                    })),
                );
            }
        }
        if !self.data.to_report.is_empty() || !self.data.reported.is_empty() {
            column = column.child(section_header(text(locale, Key::PersonSectionReport), &t));
            let items = self.data.to_report.clone();
            for item in &items {
                column = column.child(self.render_report_row(item, index, locale, &t, cx));
                index += 1;
            }
            if !self.data.reported.is_empty() {
                let open = self.reported_open;
                column = column.child(
                    Self::folded_row(
                        "reported-fold",
                        tr(
                            locale,
                            Key::ReportReportedCount,
                            &[("n", Arg::Count(self.data.reported.len() as u64))],
                        ),
                        open,
                        &t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.reported_open = !page.reported_open;
                        cx.notify();
                    })),
                );
                if open {
                    let items = self.data.reported.clone();
                    for item in &items {
                        column = column.child(self.render_report_row(item, index, locale, &t, cx));
                        index += 1;
                    }
                }
            }
        }
        let _ = window;
        ui::page_scroll("contact-page")
            .track_focus(&self.focus)
            .key_context("ContactPage")
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                if page.edit.is_some() || page.address.is_some() {
                    if k.key == "escape" {
                        page.edit = None;
                        page.address = None;
                        window.focus(&page.focus, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                    return;
                }
                if let Some(hl) = page.menu {
                    match k.key.as_str() {
                        "up" => page.menu = Some(hl.saturating_sub(1)),
                        "down" => page.menu = Some((hl + 1).min(MENU.len() - 1)),
                        "enter" | "space" => page.choose(MENU[hl], window, cx),
                        "escape" => page.menu = None,
                        _ => return,
                    }
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if !page.focus.is_focused(window) {
                    return;
                }
                let ws = page.ws.clone();
                match k.key.as_str() {
                    "f2" => page.start_edit(Field::Name, window, cx),
                    "m" if !k.modifiers.modified() => contacts_page::pick_level(&ws, page.id, cx),
                    "e" if !k.modifiers.modified() => page.start_address(window, cx),
                    "f10" if k.modifiers.shift => page.menu = Some(0),
                    "menu" => page.menu = Some(0),
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(column)
    }
}

/// The palette's contact commands on this page.
impl ContactPage {
    pub fn run(
        &mut self,
        command: crate::commands::ContactCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::commands::ContactCommand as C;
        let ws = self.ws.clone();
        match command {
            C::Rename => self.start_edit(Field::Name, window, cx),
            C::JobTitle => self.start_edit(Field::JobTitle, window, cx),
            C::Company => self.start_edit(Field::Company, window, cx),
            C::Address => self.start_address(window, cx),
            C::Move => contacts_page::pick_level(&ws, self.id, cx),
            C::Merge => contacts_page::pick_merge(&ws, self.id, cx),
            C::Delete => contacts_page::delete(&ws, self.id, cx),
        }
    }
}
