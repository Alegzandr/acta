//! The knowledge base page of a project and the document reading view,
//! with its editor and history (docs/design/components-knowledge.md 5;
//! screens.md 8.7, 8.10).

use acta_core::model::{
    Author, ChangeKind, KbEntry, KbId, KbKind, KbSummary, Level, Owner, PageReq, ProjectId, Record,
};
use acta_core::store::{
    KbEdit, KbFilter, KbRevisionSummary, KbUpdate, NewKb, SearchKind, SearchQuery,
};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Task, Window,
    anchored, deferred, div, point, px,
};

use crate::diff::{self, Change};
use crate::i18n::{Arg, Key, KeyName, Locale, number, text, tr};
use crate::icons::{Icon, icon};
use crate::kb_model::{self, Filters};
use crate::markdown_view;
use crate::menu;
use crate::record_page::PageEvent;
use crate::route::Route;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{Undo, Workspace, WriteError};

/// The name of an owner, from the loaded tree.
fn owner_name(ws: &Workspace, owner: Owner) -> String {
    for tree in &ws.projects {
        match owner {
            Owner::Project(id) if tree.project.id == id => return tree.project.title.clone(),
            Owner::Epic(id) => {
                if let Some((e, _)) = tree.epics.iter().find(|(e, _)| e.id == id) {
                    return e.title.clone();
                }
            }
            Owner::Topic(id) => {
                let all = tree
                    .loose_topics
                    .iter()
                    .chain(tree.epics.iter().flat_map(|(_, t)| t.iter()));
                if let Some(t) = all.into_iter().find(|t| t.id == id) {
                    return t.title.clone();
                }
            }
            _ => {}
        }
    }
    String::new()
}

fn level_label(locale: Locale, owner: Owner, name: &str) -> String {
    let key = match owner {
        Owner::Project(_) => Key::LevelProject,
        Owner::Epic(_) => Key::LevelEpic,
        Owner::Topic(_) => Key::LevelTopic,
    };
    tr(locale, key, &[("name", Arg::Text(name))])
}

fn owner_route(owner: Owner) -> Route {
    match owner {
        Owner::Project(id) => Route::Project(id),
        Owner::Epic(id) => Route::Epic(id),
        Owner::Topic(id) => Route::Topic(id),
    }
}

/// A knowledge row: kind icon, title, summary (or excerpt), author mark,
/// compact time (docs/design/components-knowledge.md 5).
fn kb_row(
    entry: &KbSummary,
    excerpt: Option<&str>,
    index: usize,
    locale: Locale,
    t: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let human = when::initial(&when::human_name(locale));
    let summary: AnyElement = match excerpt {
        Some(snippet) => {
            let runs = kb_model::excerpt_runs(snippet);
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .children(runs.into_iter().map(|(run, matched)| {
                    let el = type_scale::UI.el().flex_shrink_0();
                    if matched {
                        el.text_color(t.text_primary)
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(run)
                    } else {
                        el.text_color(t.text_secondary).child(run)
                    }
                }))
                .into_any_element()
        }
        None => type_scale::UI
            .el()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_color(t.text_secondary)
            .child(entry.summary.clone())
            .into_any_element(),
    };
    div()
        .id(("kb-row", index))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(32.))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .hover({
            let hover = t.fill_hover;
            move |s| s.bg(hover)
        })
        .child(icon(
            match entry.kind {
                KbKind::Document => Icon::FileText,
                KbKind::Memory => Icon::Pin,
            },
            16.,
            t.text_secondary,
        ))
        .child(
            type_scale::UI
                .el()
                .flex_shrink_0()
                .max_w(gpui_kit::relative(0.4))
                .truncate()
                .text_color(t.text_primary)
                .child(entry.title.clone()),
        )
        .child(div().w(px(4.)))
        .child(summary)
        .child(ui::author_mark(entry.updated_by, &human, t))
        .child(
            type_scale::SMALL
                .el()
                .w(px(72.))
                .flex_shrink_0()
                .text_right()
                .font_features(type_scale::tabular())
                .text_color(t.text_tertiary)
                .child(when::compact(locale, entry.updated_at)),
        )
}

// ---- knowledge base page ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterMenu {
    Level,
    Kind,
    Author,
}

pub struct KnowledgePage {
    ws: Entity<Workspace>,
    pub project: ProjectId,
    entries: Vec<KbSummary>,
    /// Search hits: entry id and its excerpt; `None` when not searching.
    hits: Option<Vec<(KbId, String)>>,
    search: Entity<InputState>,
    filters: Filters,
    menu: Option<FilterMenu>,
    open_memory: Option<KbId>,
    seen_revision: u64,
    _load: Task<()>,
    _search: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PageEvent> for KnowledgePage {}

impl KnowledgePage {
    pub fn new(
        ws: Entity<Workspace>,
        project: ProjectId,
        filters: Filters,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let locale = settings::locale(cx);
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(text(locale, Key::KbSearch)));
        let subs = vec![
            cx.observe(&ws, |page, ws, cx| {
                let revision = ws.read(cx).revision;
                if revision != page.seen_revision {
                    page.seen_revision = revision;
                    page.reload(cx);
                }
            }),
            cx.subscribe_in(&search, window, |page, _, event: &InputEvent, _, cx| {
                if let InputEvent::Change = event {
                    page.run_search(cx);
                }
            }),
        ];
        let mut page = Self {
            ws,
            project,
            entries: Vec::new(),
            hits: None,
            search,
            filters,
            menu: None,
            open_memory: None,
            seen_revision: 0,
            _load: Task::ready(()),
            _search: Task::ready(()),
            _subs: subs,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let project = self.project;
        let task = self.ws.read(cx).db.run(cx, move |store| {
            store.kb_rollup(
                Owner::Project(project),
                &KbFilter::default(),
                PageReq::first(PageReq::MAX_LIMIT),
            )
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok(entries) = loaded {
                    page.entries = entries.items;
                }
                cx.notify();
            });
        });
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().trim().to_owned();
        if query.is_empty() {
            self.hits = None;
            cx.notify();
            return;
        }
        let project = self.project;
        let task = self.ws.read(cx).db.run(cx, move |store| {
            store.search(
                &SearchQuery {
                    text: query,
                    project: Some(project),
                    kinds: vec![SearchKind::Kb],
                    ..SearchQuery::default()
                },
                PageReq::first(PageReq::MAX_LIMIT),
            )
        });
        self._search = cx.spawn(async move |page, cx| {
            let hits = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok(hits) = hits {
                    page.hits = Some(
                        hits.items
                            .into_iter()
                            .filter_map(|h| match h.record {
                                Record::Kb(id) => Some((id, h.snippet)),
                                _ => None,
                            })
                            .collect(),
                    );
                }
                cx.notify();
            });
        });
    }

    fn filter_button(
        &self,
        which: FilterMenu,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (key, value, set) = match which {
            FilterMenu::Level => (
                Key::KbFilterLevel,
                match self.filters.level {
                    None => text(locale, Key::KbValueAll),
                    Some(Level::Project) => text(locale, Key::KbValueProject),
                    Some(Level::Epic) => text(locale, Key::KbValueEpics),
                    Some(Level::Topic) => text(locale, Key::KbValueTopics),
                },
                self.filters.level.is_some(),
            ),
            FilterMenu::Kind => (
                Key::KbFilterKind,
                match self.filters.kind {
                    None => text(locale, Key::KbValueAllKind),
                    Some(KbKind::Document) => text(locale, Key::KbValueDocuments),
                    Some(KbKind::Memory) => text(locale, Key::KbValueMemories),
                },
                self.filters.kind.is_some(),
            ),
            FilterMenu::Author => (
                Key::KbFilterAuthor,
                match self.filters.author {
                    None => text(locale, Key::KbValueAll),
                    Some(author) => when::author(locale, author),
                },
                self.filters.author.is_some(),
            ),
        };
        let label = tr(locale, key, &[("value", Arg::Text(&value))]);
        let button = ui::button(
            match which {
                FilterMenu::Level => "kb-filter-level",
                FilterMenu::Kind => "kb-filter-kind",
                FilterMenu::Author => "kb-filter-author",
            },
            label,
            Variant::Secondary,
            false,
            t,
        )
        .when(set, |this| this.font_weight(gpui_kit::FontWeight::SEMIBOLD))
        .child(icon(Icon::ChevronDown, 12., t.text_tertiary))
        .on_click(cx.listener(move |page, _, _, cx| {
            page.menu = if page.menu == Some(which) {
                None
            } else {
                Some(which)
            };
            cx.notify();
        }));
        let menu = (self.menu == Some(which)).then(|| {
            type Choice = (String, Box<dyn Fn(&mut Filters)>);
            let choices: Vec<Choice> = match which {
                FilterMenu::Level => vec![
                    (
                        text(locale, Key::KbValueAll),
                        Box::new(|f: &mut Filters| f.level = None),
                    ),
                    (
                        text(locale, Key::KbValueProject),
                        Box::new(|f: &mut Filters| f.level = Some(Level::Project)),
                    ),
                    (
                        text(locale, Key::KbValueEpics),
                        Box::new(|f: &mut Filters| f.level = Some(Level::Epic)),
                    ),
                    (
                        text(locale, Key::KbValueTopics),
                        Box::new(|f: &mut Filters| f.level = Some(Level::Topic)),
                    ),
                ],
                FilterMenu::Kind => vec![
                    (
                        text(locale, Key::KbValueAllKind),
                        Box::new(|f: &mut Filters| f.kind = None),
                    ),
                    (
                        text(locale, Key::KbValueDocuments),
                        Box::new(|f: &mut Filters| f.kind = Some(KbKind::Document)),
                    ),
                    (
                        text(locale, Key::KbValueMemories),
                        Box::new(|f: &mut Filters| f.kind = Some(KbKind::Memory)),
                    ),
                ],
                FilterMenu::Author => vec![
                    (
                        text(locale, Key::KbValueAll),
                        Box::new(|f: &mut Filters| f.author = None),
                    ),
                    (
                        when::human_name(locale),
                        Box::new(|f: &mut Filters| f.author = Some(Author::Human)),
                    ),
                    (
                        when::author(locale, Author::Claude),
                        Box::new(|f: &mut Filters| f.author = Some(Author::Claude)),
                    ),
                ],
            };
            let items = choices.into_iter().enumerate().map(|(i, (label, apply))| {
                menu::item(("kb-filter-item", i), None, label, None, false, t).on_click(
                    cx.listener(move |page, _, _, cx| {
                        apply(&mut page.filters);
                        page.menu = None;
                        cx.notify();
                    }),
                )
            });
            deferred(
                anchored().offset(point(px(0.), px(32.))).child(
                    menu::panel("kb-filter-menu", t)
                        .on_mouse_down_out(cx.listener(|page, _, _, cx| {
                            page.menu = None;
                            cx.notify();
                        }))
                        .children(items),
                ),
            )
        });
        div().child(button).children(menu).into_any_element()
    }
}

impl Render for KnowledgePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let ws = self.ws.read(cx);
        let project_title = owner_name(ws, Owner::Project(self.project));
        let documents = self
            .entries
            .iter()
            .filter(|e| e.kind == KbKind::Document)
            .count();
        let memories = self.entries.len() - documents;
        let scope = tr(
            locale,
            Key::KbScope,
            &[
                ("project", Arg::Text(&project_title)),
                (
                    "documents",
                    Arg::Text(&tr(
                        locale,
                        Key::CountDocuments,
                        &[("n", Arg::Count(documents as u64))],
                    )),
                ),
                (
                    "memories",
                    Arg::Text(&tr(
                        locale,
                        Key::CountMemories,
                        &[("n", Arg::Count(memories as u64))],
                    )),
                ),
            ],
        );
        let shown: Vec<(KbSummary, Option<String>)> = match &self.hits {
            Some(hits) => hits
                .iter()
                .filter_map(|(id, snippet)| {
                    self.entries
                        .iter()
                        .find(|e| e.id == *id)
                        .map(|e| (e.clone(), Some(snippet.clone())))
                })
                .filter(|(e, _)| self.filters.keeps(e))
                .collect(),
            None => self
                .entries
                .iter()
                .filter(|e| self.filters.keeps(e))
                .map(|e| (e.clone(), None))
                .collect(),
        };
        let names: Vec<(Owner, String)> = {
            let ws = self.ws.read(cx);
            shown
                .iter()
                .map(|(e, _)| (e.owner, owner_name(ws, e.owner)))
                .collect()
        };
        let query = self.search.read(cx).value().to_string();
        let mut column = ui::column()
            .child(
                type_scale::TITLE
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, Key::TreeKnowledge)),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .mt(px(8.))
                    .text_color(t.text_tertiary)
                    .child(scope),
            )
            .child(
                div()
                    .mt(px(24.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(px(RADIUS_SM))
                    .bg(t.bg_raised)
                    .border_1()
                    .border_color(t.border_strong)
                    .child(icon(Icon::Search, 16., t.text_tertiary))
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.search).appearance(false)),
                    ),
            )
            .child(
                div()
                    .mt(px(12.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(self.filter_button(FilterMenu::Level, locale, &t, cx))
                    .child(self.filter_button(FilterMenu::Kind, locale, &t, cx))
                    .child(self.filter_button(FilterMenu::Author, locale, &t, cx))
                    .when(!self.filters.is_default(), |this| {
                        this.child(
                            ui::button(
                                "kb-clear",
                                text(locale, Key::CommonClear),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|page, _, _, cx| {
                                page.filters = Filters::default();
                                cx.notify();
                            })),
                        )
                    }),
            );
        if self.hits.is_some() {
            let line = if shown.is_empty() {
                tr(
                    locale,
                    Key::KbNoResults,
                    &[("query", Arg::Text(query.trim()))],
                )
            } else {
                tr(
                    locale,
                    Key::KbResults,
                    &[
                        ("n", Arg::Count(shown.len() as u64)),
                        ("query", Arg::Text(query.trim())),
                    ],
                )
            };
            column = column.child(
                type_scale::SMALL
                    .el()
                    .mt(px(24.))
                    .text_color(t.text_tertiary)
                    .child(line),
            );
        }
        type Group = (Owner, Vec<(KbSummary, Option<String>)>);
        let mut groups: Vec<Group> = Vec::new();
        for item in shown {
            match groups.iter_mut().find(|(o, _)| *o == item.0.owner) {
                Some((_, g)) => g.push(item),
                None => groups.push((item.0.owner, vec![item])),
            }
        }
        groups.sort_by_key(|(o, _)| match o.level() {
            Level::Project => 0,
            Level::Epic => 1,
            Level::Topic => 2,
        });
        let mut index = 0;
        for (owner, entries) in groups {
            let name = names
                .iter()
                .find(|(o, _)| *o == owner)
                .map(|(_, n)| n.clone())
                .unwrap_or_default();
            let route = owner_route(owner);
            column = column.child(
                div()
                    .id(("kb-group", index))
                    .mt(px(16.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(28.))
                    .px(px(8.))
                    .child(
                        type_scale::SMALL
                            .el()
                            .id(("kb-group-link", index))
                            .text_color(t.text_tertiary)
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(
                                cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(route))),
                            )
                            .child(level_label(locale, owner, &name)),
                    )
                    .child(
                        type_scale::SMALL
                            .el()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_tertiary)
                            .child(number(locale, entries.len() as u64)),
                    ),
            );
            for (entry, excerpt) in entries {
                let id = entry.id;
                let kind = entry.kind;
                let open = self.open_memory == Some(id);
                column = column.child(
                    kb_row(&entry, excerpt.as_deref(), index, locale, &t).on_click(cx.listener(
                        move |page, _, _, cx| match kind {
                            KbKind::Document => cx.emit(PageEvent::Open(Route::Kb(id))),
                            KbKind::Memory => {
                                page.open_memory = if page.open_memory == Some(id) {
                                    None
                                } else {
                                    Some(id)
                                };
                                cx.notify();
                            }
                        },
                    )),
                );
                if open {
                    column = column.child(
                        div()
                            .pl(px(32.))
                            .pb(px(8.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                type_scale::READING
                                    .el()
                                    .text_color(t.text_primary)
                                    .child(entry.summary.clone()),
                            )
                            .child(
                                div().ml(px(-12.)).child(
                                    ui::button(
                                        ("kb-open", index),
                                        text(locale, Key::CommonOpen),
                                        Variant::Ghost,
                                        false,
                                        &t,
                                    )
                                    .on_click(cx.listener(
                                        move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Kb(id))),
                                    )),
                                ),
                            ),
                    );
                }
                index += 1;
            }
        }
        ui::page_scroll("knowledge-page").child(column)
    }
}

// ---- document reading view, editor and history ----

/// What the document page is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Read,
    Edit,
    History,
}

/// A document or memory not saved yet: the editor creates it on save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draft {
    pub owner: Owner,
    pub kind: KbKind,
}

pub struct DocPage {
    ws: Entity<Workspace>,
    pub id: Option<KbId>,
    draft: Option<Draft>,
    entry: Option<KbEntry>,
    mode: Mode,
    title: Option<Entity<InputState>>,
    summary: Option<Entity<InputState>>,
    body: Option<Entity<TextareaState>>,
    history: Vec<KbRevisionSummary>,
    /// The selected revision, with its text and the text before it.
    selected: Option<(i64, String, String)>,
    show_changes: bool,
    /// The project of the entry (for its undo stack), once known.
    project: Option<acta_core::model::ProjectId>,
    saving: bool,
    /// Someone else's revision that a save ran into, and whether it is
    /// shown beside the editor.
    conflict: Option<KbEntry>,
    show_theirs: bool,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PageEvent> for DocPage {}

impl DocPage {
    pub fn open(ws: Entity<Workspace>, id: KbId, cx: &mut Context<Self>) -> Self {
        let mut page = Self::blank(ws, Some(id), None, cx);
        page.reload(cx);
        page
    }

    /// "New document" / "New memory": the editor, empty, for `draft.owner`.
    pub fn create(
        ws: Entity<Workspace>,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut page = Self::blank(ws, None, Some(draft), cx);
        page.start_edit(window, cx);
        page
    }

    fn blank(
        ws: Entity<Workspace>,
        id: Option<KbId>,
        draft: Option<Draft>,
        cx: &mut Context<Self>,
    ) -> Self {
        let project = draft.and_then(|d| crate::workspace::project_of(&ws, d.owner, cx));
        let sub = cx.observe(&ws, |page, ws, cx| {
            let revision = ws.read(cx).revision;
            if revision != page.seen_revision {
                page.seen_revision = revision;
                // Never under an open editor (interaction.md 6.5 rule 4).
                if page.mode != Mode::Edit {
                    page.reload(cx);
                }
            }
        });
        Self {
            ws,
            id,
            draft,
            entry: None,
            mode: Mode::Read,
            title: None,
            summary: None,
            body: None,
            history: Vec::new(),
            selected: None,
            show_changes: false,
            project,
            saving: false,
            conflict: None,
            show_theirs: false,
            seen_revision: 0,
            _load: Task::ready(()),
            _subs: vec![sub],
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.id else { return };
        let task = self.ws.read(cx).db.run(cx, move |store| {
            Ok((
                store.kb_entry(id)?,
                store
                    .kb_history(id, PageReq::first(PageReq::MAX_LIMIT))?
                    .items,
            ))
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok((entry, history)) = loaded {
                    cx.emit(PageEvent::Located(entry.project));
                    page.project = Some(entry.project);
                    page.entry = Some(entry);
                    page.history = history;
                }
                cx.notify();
            });
        });
    }

    pub fn start_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = settings::locale(cx);
        let (title, summary, body) = match &self.entry {
            Some(e) => (e.title.clone(), e.summary.clone(), e.body.clone()),
            None => Default::default(),
        };
        let kind = self
            .entry
            .as_ref()
            .map(|e| e.kind)
            .or(self.draft.map(|d| d.kind));
        let summary_placeholder = match kind {
            Some(KbKind::Memory) => Key::KbSummaryMemory,
            _ => Key::KbSummaryDocument,
        };
        let title_state = cx.new(|cx| InputState::new(window, cx).default_value(title));
        let summary_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(text(locale, summary_placeholder))
                .default_value(summary)
        });
        let body_state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(12, 400)
                .default_value(body)
        });
        title_state.update(cx, |s, cx| s.focus(window, cx));
        for state in [&title_state, &summary_state] {
            self._subs.push(cx.subscribe_in(
                state,
                window,
                |page, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter {
                        secondary: true, ..
                    } => page.save(window, cx),
                    InputEvent::Change => cx.notify(),
                    _ => {}
                },
            ));
        }
        self._subs.push(cx.subscribe_in(
            &body_state,
            window,
            |page, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    page.save(window, cx);
                }
            },
        ));
        self.title = Some(title_state);
        self.summary = Some(summary_state);
        self.body = Some(body_state);
        self.mode = Mode::Edit;
        cx.notify();
    }

    fn can_save(&self, cx: &Context<Self>) -> bool {
        let filled = |s: &Option<Entity<InputState>>| {
            s.as_ref()
                .is_some_and(|s| !s.read(cx).value().trim().is_empty())
        };
        filled(&self.title) && filled(&self.summary)
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Edit && self.id.is_none() {
            // A draft never saved: back to where it was started from.
            if let Some(draft) = self.draft {
                cx.emit(PageEvent::Open(owner_route(draft.owner)));
            }
        }
        self.mode = Mode::Read;
        self.title = None;
        self.summary = None;
        self.body = None;
        cx.notify();
    }

    /// Save the editor. It stays open with the typed text until the write
    /// succeeds. A revision written meanwhile by someone else (Claude Code)
    /// stops the save and shows the conflict notice (interaction.md 6.5
    /// rule 4): the human's text stays in the editor, theirs can be shown,
    /// and saving again ("Keep mine") writes over it as a new revision, so
    /// both stay in the history.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save(cx) || self.saving {
            return;
        }
        let read = |s: &Option<Entity<InputState>>, cx: &Context<Self>| {
            s.as_ref()
                .map(|s| s.read(cx).value().trim().to_owned())
                .unwrap_or_default()
        };
        let title = read(&self.title, cx);
        let summary = read(&self.summary, cx);
        let body = self
            .body
            .as_ref()
            .map(|b| b.read(cx).value().to_string())
            .unwrap_or_default();
        self.saving = true;
        cx.notify();
        let project = self.project;
        if let Some(id) = self.id {
            let rev = self
                .conflict
                .as_ref()
                .map(|theirs| theirs.rev)
                .or(self.entry.as_ref().map(|e| e.rev));
            let task = self.ws.update(cx, |ws, cx| {
                ws.write_undoable(cx, project, None, move |store| {
                    let back = store.kb_entry(id)?.rev;
                    let expect = store.kb_update(
                        id,
                        KbUpdate {
                            expected_rev: rev,
                            title: Some(title),
                            summary: Some(summary),
                            edit: KbEdit::Body(body),
                        },
                        Author::Human,
                    )?;
                    Ok((Undo::KbRevision { id, back, expect }, ()))
                })
            });
            cx.spawn_in(window, async move |page, cx| {
                let result = task.await;
                let _ = page.update_in(cx, |page, _, cx| {
                    page.saving = false;
                    match result {
                        Ok(()) => page.close_editor(cx),
                        Err(WriteError::Stale) => page.load_conflict(cx),
                        Err(WriteError::Failed) => {}
                    }
                    cx.notify();
                });
            })
            .detach();
        } else if let Some(draft) = self.draft {
            let task = self.ws.update(cx, |ws, cx| {
                ws.write_undoable(cx, project, None, move |store| {
                    let id = store.kb_create(
                        NewKb {
                            owner: draft.owner,
                            kind: draft.kind,
                            title,
                            summary,
                            body,
                            from_file: false,
                        },
                        Author::Human,
                    )?;
                    Ok((Undo::Delete(Record::Kb(id)), id))
                })
            });
            cx.spawn_in(window, async move |page, cx| {
                let result = task.await;
                let _ = page.update_in(cx, |page, _, cx| {
                    page.saving = false;
                    if let Ok(id) = result {
                        page.id = Some(id);
                        page.draft = None;
                        page.close_editor(cx);
                        page.reload(cx);
                        cx.emit(PageEvent::Replace(Route::Kb(id)));
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Read;
        self.title = None;
        self.summary = None;
        self.body = None;
        self.conflict = None;
        self.show_theirs = false;
        self.reload(cx);
    }

    /// Someone else saved first: keep the editor, fetch their version.
    fn load_conflict(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.id else { return };
        let task = self.ws.read(cx).db.run(cx, move |store| store.kb_entry(id));
        cx.spawn(async move |page, cx| {
            if let Ok(theirs) = task.await {
                let _ = page.update(cx, |page, cx| {
                    page.conflict = Some(theirs);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn select_revision(&mut self, rev: i64, cx: &mut Context<Self>) {
        let Some(id) = self.id else { return };
        let task = self.ws.read(cx).db.run(cx, move |store| {
            let this = store.kb_revision(id, rev)?.body;
            let before = if rev > 1 {
                store
                    .kb_revision(id, rev - 1)
                    .map(|r| r.body)
                    .unwrap_or_default()
            } else {
                String::new()
            };
            Ok((this, before))
        });
        cx.spawn(async move |page, cx| {
            if let Ok((this, before)) = task.await {
                let _ = page.update(cx, |page, cx| {
                    page.selected = Some((rev, this, before));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn restore_selected(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some((rev, _, _))) = (self.id, &self.selected) else {
            return;
        };
        let rev = *rev;
        let locale = settings::locale(cx);
        let at = self
            .history
            .iter()
            .find(|h| h.rev == rev)
            .map(|h| h.created_at)
            .unwrap_or(0);
        let message = tr(
            locale,
            Key::HistoryRestored,
            &[("datetime", Arg::Text(&when::datetime(locale, at)))],
        );
        self.mode = Mode::Read;
        let project = self.project;
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, Some(message), move |store| {
                let back = store.kb_entry(id)?.rev;
                let expect = store.kb_restore_revision(id, rev, Author::Human)?;
                Ok((Undo::KbRevision { id, back, expect }, ()))
            });
        });
        cx.notify();
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(entry)) = (self.id, &self.entry) else {
            return;
        };
        let owner = entry.owner;
        let project = self.project;
        self.ws
            .update(cx, |ws, cx| ws.delete(Record::Kb(id), project, cx));
        cx.emit(PageEvent::Open(owner_route(owner)));
    }

    fn change_label(
        locale: Locale,
        change: ChangeKind,
        section: Option<&str>,
        kind: KbKind,
    ) -> String {
        match change {
            ChangeKind::ReplacedSection | ChangeKind::AppendedSection => tr(
                locale,
                Key::HistoryReplaced,
                &[("name", Arg::Text(section.unwrap_or_default()))],
            ),
            ChangeKind::CreatedFromFile => text(
                locale,
                match kind {
                    KbKind::Document => Key::HistoryFromFileDocument,
                    KbKind::Memory => Key::HistoryFromFileMemory,
                },
            ),
            _ => text(
                locale,
                match kind {
                    KbKind::Document => Key::HistoryEditedDocument,
                    KbKind::Memory => Key::HistoryEditedMemory,
                },
            ),
        }
    }

    fn render_history(
        &self,
        entry: &KbEntry,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let human = when::initial(&when::human_name(locale));
        let rows = self.history.iter().enumerate().map(|(i, h)| {
            let rev = h.rev;
            let selected = self.selected.as_ref().is_some_and(|s| s.0 == rev);
            div()
                .id(("history-row", i))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(32.))
                .px(px(8.))
                .rounded(px(RADIUS_SM))
                .when(selected, |this| this.bg(t.fill_selected))
                .hover({
                    let hover = t.fill_hover;
                    move |s| s.bg(hover)
                })
                .on_click(cx.listener(move |page, _, _, cx| page.select_revision(rev, cx)))
                .child(ui::author_mark(h.author, &human, t))
                .child(
                    type_scale::UI
                        .el()
                        .text_color(t.text_primary)
                        .child(when::author(locale, h.author)),
                )
                .child(
                    type_scale::UI
                        .el()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_secondary)
                        .child(Self::change_label(
                            locale,
                            h.change,
                            h.section.as_deref(),
                            entry.kind,
                        )),
                )
                .child(
                    type_scale::SMALL
                        .el()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(when::compact(locale, h.created_at)),
                )
        });
        let mut out = div().flex().flex_col().child(
            div()
                .id("history-list")
                .max_h(px(6. * 32.))
                .overflow_y_scroll()
                .children(rows),
        );
        if let Some((rev, body, before)) = &self.selected {
            let meta = self.history.iter().find(|h| h.rev == *rev);
            let version = meta
                .map(|h| {
                    tr(
                        locale,
                        Key::HistoryVersion,
                        &[
                            ("datetime", Arg::Text(&when::datetime(locale, h.created_at))),
                            ("author", Arg::Text(&when::author(locale, h.author))),
                        ],
                    )
                })
                .unwrap_or_default();
            let toggle = ui::segmented(t)
                .child(
                    ui::segment(
                        "history-text",
                        text(locale, Key::HistoryText),
                        !self.show_changes,
                        t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.show_changes = false;
                        cx.notify();
                    })),
                )
                .child(
                    ui::segment(
                        "history-changes",
                        text(locale, Key::HistoryChanges),
                        self.show_changes,
                        t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.show_changes = true;
                        cx.notify();
                    })),
                );
            out = out.child(
                div().mt(px(24.)).child(
                    ui::notice(None, version, t).child(toggle).child(
                        ui::button(
                            "history-restore",
                            text(locale, Key::HistoryRestore),
                            Variant::Secondary,
                            false,
                            t,
                        )
                        .on_click(cx.listener(|page, _, _, cx| page.restore_selected(cx))),
                    ),
                ),
            );
            let shown: AnyElement = if self.show_changes {
                div()
                    .flex()
                    .flex_col()
                    .children(diff::lines(before, body).into_iter().map(|(change, line)| {
                        let (mark, mark_color, text_color) = match change {
                            Change::Same => (" ", t.text_tertiary, t.text_primary),
                            Change::Added => ("+", t.success, t.text_primary),
                            Change::Removed => ("-", t.danger, t.text_secondary),
                        };
                        type_scale::CODE
                            .el()
                            .flex()
                            .flex_row()
                            .child(
                                div()
                                    .w(px(16.))
                                    .flex_shrink_0()
                                    .text_color(mark_color)
                                    .child(mark),
                            )
                            .child(div().text_color(text_color).child(line.to_owned()))
                    }))
                    .into_any_element()
            } else {
                markdown_view::document("history-body", body.clone(), t).into_any_element()
            };
            out = out.child(div().mt(px(16.)).child(shown));
        }
        out.into_any_element()
    }
}

impl Render for DocPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let entry = self.entry.clone();
        let mut column = ui::column();
        let editing = self.mode == Mode::Edit;
        if editing {
            let count = self
                .summary
                .as_ref()
                .map(|s| s.read(cx).value().chars().count())
                .unwrap_or(0);
            if let Some(theirs) = &self.conflict {
                let theirs_body = theirs.body.clone();
                column = column.child(
                    div().mb(px(16.)).child(
                        ui::notice(
                            Some((Icon::TriangleAlert, t.warning)),
                            text(locale, Key::ConflictNotice),
                            &t,
                        )
                        .child(
                            ui::button(
                                "conflict-theirs",
                                text(locale, Key::ConflictTheirs),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|page, _, _, cx| {
                                page.show_theirs = !page.show_theirs;
                                cx.notify();
                            })),
                        )
                        .child(
                            ui::button(
                                "conflict-mine",
                                text(locale, Key::ConflictMine),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|page, _, window, cx| page.save(window, cx))),
                        ),
                    ),
                );
                if self.show_theirs {
                    column = column.child(
                        div()
                            .mb(px(16.))
                            .p(px(12.))
                            .rounded(px(crate::theme::RADIUS_MD))
                            .border_1()
                            .border_color(t.border_subtle)
                            .child(markdown_view::document(
                                "conflict-theirs-body",
                                theirs_body,
                                &t,
                            )),
                    );
                }
            }
            if let (Some(title), Some(summary), Some(body)) =
                (&self.title, &self.summary, &self.body)
            {
                column = column
                    .child(
                        div()
                            .border_b_1()
                            .border_color(t.border_strong)
                            .text_size(px(24.))
                            .line_height(px(30.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(Input::new(title).appearance(false)),
                    )
                    .child(
                        div()
                            .mt(px(8.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(type_scale::READING.el().flex_1().child(Input::new(summary)))
                            .child(
                                type_scale::SMALL
                                    .el()
                                    .font_features(type_scale::tabular())
                                    .text_color(t.text_tertiary)
                                    .child(format!("{} / 140", number(locale, count as u64))),
                            ),
                    )
                    .child(div().mt(px(24.)).h(px(1.)).bg(t.border_subtle))
                    .child(
                        type_scale::DOCUMENT
                            .el()
                            .mt(px(24.))
                            .child(Textarea::new(body).appearance(false)),
                    );
            }
        } else if let Some(entry) = &entry {
            let owner_name = owner_name(self.ws.read(cx), entry.owner);
            let level = level_label(locale, entry.owner, &owner_name);
            let route = owner_route(entry.owner);
            let kind = text(
                locale,
                match entry.kind {
                    KbKind::Document => Key::KindDocument,
                    KbKind::Memory => Key::KindMemory,
                },
            );
            let updated = {
                let when = when::long_alone(locale, entry.stamp.updated_at);
                let author = when::author(locale, entry.stamp.updated_by);
                tr(
                    locale,
                    Key::CommonBy,
                    &[("when", Arg::Text(&when)), ("author", Arg::Text(&author))],
                )
            };
            let labels = [
                text(locale, Key::PropKind),
                text(locale, Key::PropLevel),
                text(locale, Key::PropId),
                text(locale, Key::PropUpdated),
            ];
            let width = ui::label_width(&labels.iter().map(String::as_str).collect::<Vec<_>>());
            let macos = cfg!(target_os = "macos");
            let _ = macos;
            column = column
                .child(
                    type_scale::TITLE
                        .el()
                        .text_color(t.text_primary)
                        .child(entry.title.clone()),
                )
                .child(
                    type_scale::READING
                        .el()
                        .mt(px(8.))
                        .text_color(t.text_secondary)
                        .child(entry.summary.clone()),
                )
                .child(
                    div()
                        .mt(px(12.))
                        .flex()
                        .flex_col()
                        .child(ui::property(
                            labels[0].clone(),
                            width,
                            div().child(kind),
                            &t,
                        ))
                        .child(ui::property(
                            labels[1].clone(),
                            width,
                            div()
                                .id("doc-level")
                                .cursor_pointer()
                                .text_color(t.accent)
                                .hover(|s| s.underline())
                                .on_click(
                                    cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(route))),
                                )
                                .child(level),
                            &t,
                        ))
                        .child(ui::property(
                            labels[2].clone(),
                            width,
                            type_scale::CODE
                                .el()
                                .text_color(t.text_secondary)
                                .child(entry.id.to_string()),
                            &t,
                        ))
                        .child(ui::property(
                            labels[3].clone(),
                            width,
                            div().child(updated),
                            &t,
                        )),
                )
                .child(
                    div()
                        .mt(px(12.))
                        .ml(px(-12.))
                        .flex()
                        .flex_row()
                        .gap(px(4.))
                        .child(
                            ui::button(
                                "doc-edit",
                                text(locale, Key::CommonEdit),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(
                                cx.listener(|page, _, window, cx| page.start_edit(window, cx)),
                            ),
                        )
                        .child(
                            ui::button(
                                "doc-history",
                                text(locale, Key::CommonHistory),
                                Variant::Ghost,
                                self.history.is_empty(),
                                &t,
                            )
                            .on_click(cx.listener(|page, _, _, cx| {
                                page.mode = if page.mode == Mode::History {
                                    Mode::Read
                                } else {
                                    Mode::History
                                };
                                if page.selected.is_none()
                                    && let Some(rev) = page.history.first().map(|h| h.rev)
                                {
                                    page.select_revision(rev, cx);
                                }
                                cx.notify();
                            })),
                        )
                        .child(
                            ui::button(
                                "doc-delete",
                                text(locale, Key::CommonDelete),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|page, _, _, cx| page.delete(cx))),
                        ),
                )
                .child(div().mt(px(24.)).h(px(1.)).bg(t.border_subtle))
                .child(div().mt(px(24.)).child(if self.mode == Mode::History {
                    self.render_history(entry, locale, &t, cx)
                } else {
                    markdown_view::document("doc-body", entry.body.clone(), &t).into_any_element()
                }));
        }
        let save_bar = editing.then(|| {
            let can_save = self.can_save(cx);
            let macos = cfg!(target_os = "macos");
            let keys = format!(
                "{}+{}",
                KeyName::Mod.label(locale, macos),
                KeyName::Enter.label(locale, macos)
            );
            div()
                .flex()
                .flex_row()
                .flex_shrink_0()
                .items_center()
                .gap(px(8.))
                .h(px(44.))
                .px(px(16.))
                .bg(t.bg_base)
                .border_t_1()
                .border_color(t.border_subtle)
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::DocEditing)),
                )
                .child(div().flex_1())
                .when(!can_save, |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .text_color(t.text_tertiary)
                            .child(text(locale, Key::DocNeeds)),
                    )
                })
                .child(
                    ui::button(
                        "doc-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Ghost,
                        false,
                        &t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| page.cancel(cx))),
                )
                .child(
                    ui::button(
                        "doc-save",
                        text(locale, Key::CommonSave),
                        Variant::Primary,
                        !can_save,
                        &t,
                    )
                    .child(type_scale::SMALL.el().child(keys))
                    .on_click(cx.listener(|page, _, window, cx| page.save(window, cx))),
                )
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_action(cx.listener(|page, _: &Escape, _, cx| {
                if page.mode == Mode::History {
                    page.mode = Mode::Read;
                    cx.notify();
                } else {
                    page.cancel(cx);
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(ui::page_scroll("doc-page").child(column)),
            )
            .children(save_bar)
    }
}
