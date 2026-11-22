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
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, div, px,
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

/// A knowledge row: kind icon, title, summary (or excerpt), the unread
/// dot, compact time (docs/design/components-knowledge.md 5). Under a column of
/// 520 the summary takes a second line and the row is 52.
pub(crate) fn kb_row(
    entry: &KbSummary,
    excerpt: Option<&str>,
    unread: bool,
    index: usize,
    locale: Locale,
    t: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
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
    let two_line = ui::two_line_rows();
    let kind = icon(
        match entry.kind {
            KbKind::Document => Icon::FileText,
            KbKind::Memory => Icon::Pin,
        },
        16.,
        t.text_secondary,
    );
    let title = type_scale::UI
        .el()
        .truncate()
        .text_color(t.text_primary)
        .child(entry.title.clone());
    let title = if two_line {
        title.flex_1().min_w_0()
    } else {
        title.flex_shrink_0().max_w(gpui_kit::relative(0.4))
    };
    // The unread dot, then the time; who wrote it last is the time's
    // tooltip (clarity review 2026-11-22: no author mark on rows).
    let tip = {
        let when = when::long(locale, entry.updated_at);
        let author = when::author(locale, entry.updated_by);
        tr(
            locale,
            Key::MetaUpdated,
            &[("when", Arg::Text(&when)), ("author", Arg::Text(&author))],
        )
    };
    let trailing = || -> Vec<AnyElement> {
        let mut out = Vec::new();
        if unread {
            out.push(ui::unread_dot(t).into_any_element());
        }
        out.push(
            type_scale::SMALL
                .el()
                .id(("kb-row-time", index))
                .w(px(72.))
                .flex_shrink_0()
                .text_right()
                .font_features(type_scale::tabular())
                .text_color(t.text_tertiary)
                .tooltip(ui::tip(tip.clone()))
                .child(when::compact(locale, entry.updated_at))
                .into_any_element(),
        );
        out
    };
    let row = ui::focusable(div().id(("kb-row", index)), t, false)
        .px(px(8.))
        .mx(px(-8.))
        .rounded(px(RADIUS_SM))
        .hover({
            let hover = t.fill_hover;
            move |s| s.bg(hover)
        });
    if two_line {
        // The summary under the title, aligned with it (16 + 8).
        row.flex()
            .flex_col()
            .justify_center()
            .h(px(52.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(24.))
                    .child(kind)
                    .child(title)
                    .children(trailing()),
            )
            .child(div().flex().h(px(20.)).pl(px(24.)).min_w_0().child(summary))
    } else {
        row.flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .child(kind)
            .child(title)
            // 12 between title and summary: the row's gap of 8 plus 4.
            .child(div().ml(px(4.)).flex_1().min_w_0().flex().child(summary))
            .children(trailing())
    }
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
    /// Where the open filter menu hangs.
    menu_anchor: menu::AnchorBounds,
    open_memory: Option<KbId>,
    /// Shown as the Knowledge view (workbench.md 4.8.3): no title, no
    /// meta line, following the active project.
    as_view: bool,
    /// The Overview's chip "Not updated for 90 days" (components-records.md
    /// 5): only these memories, oldest first; `None` without the chip.
    stale: Option<Vec<KbId>>,
    /// One focus handle per row in the order drawn: `Up` / `Down` move
    /// between them; opened from the Overview, the first takes the focus
    /// once the list is loaded (as the topic table does).
    row_focus: Vec<FocusHandle>,
    focus_first_row: bool,
    seen_revision: u64,
    _load: Task<()>,
    _search: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PageEvent> for KnowledgePage {}

impl KnowledgePage {
    #[cfg(windows)]
    /// A filter menu is open (the chat's web view hides under it).
    pub fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    pub fn new(
        ws: Entity<Workspace>,
        project: ProjectId,
        filters: Filters,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(ws, project, filters, false, window, cx)
    }

    /// The Overview's memory line: Kind Memories with the chip "Not
    /// updated for 90 days", oldest first. The chip is not saved (4.7).
    pub fn new_stale(
        ws: Entity<Workspace>,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filters = Filters {
            kind: Some(KbKind::Memory),
            ..Filters::default()
        };
        let mut page = Self::build(ws, project, filters, false, window, cx);
        page.stale = Some(Vec::new());
        page.focus_first_row = true;
        page.reload(cx);
        page
    }

    /// Knowledge as a view of a side area or the bottom panel.
    pub fn new_view(
        ws: Entity<Workspace>,
        project: ProjectId,
        filters: Filters,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(ws, project, filters, true, window, cx)
    }

    fn build(
        ws: Entity<Workspace>,
        project: ProjectId,
        filters: Filters,
        as_view: bool,
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
            menu_anchor: Default::default(),
            open_memory: None,
            as_view,
            stale: None,
            row_focus: Vec::new(),
            focus_first_row: false,
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
        let stale = self.stale.is_some();
        let today = crate::due::today(cx);
        let task = self.ws.read(cx).db.run(cx, move |store| {
            let entries = store.kb_rollup(
                Owner::Project(project),
                &KbFilter::default(),
                PageReq::first(PageReq::MAX_LIMIT),
            )?;
            // The chip's list is the Overview's own query.
            let stale = if stale {
                let items = store.overview_list(
                    acta_core::store::Scope::Project(project),
                    acta_core::store::OverviewFilter::StaleMemories,
                    today,
                    acta_core::date::Zone::system(),
                )?;
                Some(
                    items
                        .into_iter()
                        .filter_map(|i| match i.record {
                            Record::Kb(id) => Some(id),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            Ok((entries, stale))
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok((entries, stale)) = loaded {
                    page.entries = entries.items;
                    if page.stale.is_some() {
                        page.stale = stale;
                    }
                }
                cx.notify();
            });
        });
    }

    /// The filter chip "Not updated for 90 days" (components-records.md 5,
    /// Filter chip): after the dropdowns; its `x` or `Backspace` clears it.
    fn render_stale_chip(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        ui::focusable(div().id("kb-chip"), t, false)
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(24.))
            .pl(px(8.))
            .pr(px(4.))
            .rounded(px(RADIUS_SM))
            .bg(t.fill_selected)
            .on_key_down(cx.listener(|page, event: &gpui_kit::KeyDownEvent, _, cx| {
                if event.keystroke.key == "backspace" {
                    page.stale = None;
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                type_scale::UI
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, Key::FilterStaleMemories)),
            )
            .child(
                ui::ghost_icon_button("kb-chip-clear", Icon::X, 16., false, false, true, t)
                    .tooltip(ui::tip(text(locale, Key::FilterClear)))
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.stale = None;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// Keys on a row: `Up` / `Down` move between rows, `Esc` clears the
    /// chip "Not updated for 90 days" (components-records.md 5, Filter chip).
    fn row_key(
        &mut self,
        index: usize,
        rows: usize,
        event: &gpui_kit::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let to = match event.keystroke.key.as_str() {
            "down" => (index + 1 < rows).then_some(index + 1),
            "up" => index.checked_sub(1),
            "escape" if self.stale.is_some() => {
                self.stale = None;
                cx.stop_propagation();
                cx.notify();
                return;
            }
            _ => return,
        };
        if let Some(handle) = to.and_then(|i| self.row_focus.get(i)) {
            window.focus(handle, cx);
        }
        cx.stop_propagation();
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
            self.menu_anchor.below_keyed(
                which as u64,
                menu::panel("kb-filter-menu", t)
                    .on_mouse_down_out(cx.listener(|page, _, _, cx| {
                        page.menu = None;
                        cx.notify();
                    }))
                    .children(items),
                menu::Align::Start,
                1,
            )
        });
        div()
            .relative()
            .child(button)
            .children(
                (self.menu == Some(which)).then(|| self.menu_anchor.measure_keyed(which as u64)),
            )
            .children(menu.flatten())
            .into_any_element()
    }
}

impl Render for KnowledgePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            None => match &self.stale {
                Some(ids) => ids
                    .iter()
                    .filter_map(|id| self.entries.iter().find(|e| e.id == *id))
                    .filter(|e| self.filters.keeps(e))
                    .map(|e| (e.clone(), None))
                    .collect(),
                None => self
                    .entries
                    .iter()
                    .filter(|e| self.filters.keeps(e))
                    .map(|e| (e.clone(), None))
                    .collect(),
            },
        };
        let names: Vec<(Owner, String)> = {
            let ws = self.ws.read(cx);
            shown
                .iter()
                .map(|(e, _)| (e.owner, owner_name(ws, e.owner)))
                .collect()
        };
        let query = self.search.read(cx).value().to_string();
        let as_view = self.as_view;
        let mut column = if as_view {
            ui::view_column()
        } else {
            ui::column()
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
        }
        .child(
            div()
                .mt(px(if as_view { 0. } else { 24. }))
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
                    div().flex_1().child(
                        Input::new(&self.search)
                            .appearance(false)
                            .px(px(0.))
                            .text_size(px(type_scale::UI.size)),
                    ),
                ),
        )
        .child(
            div()
                .mt(px(12.))
                .flex()
                .flex_row()
                // The chip wraps under the dropdowns in a narrow column.
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .child(self.filter_button(FilterMenu::Level, locale, &t, cx))
                .child(self.filter_button(FilterMenu::Kind, locale, &t, cx))
                .child(self.filter_button(FilterMenu::Author, locale, &t, cx))
                .when(self.stale.is_some(), |this| {
                    this.child(self.render_stale_chip(locale, &t, cx))
                })
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
                            page.stale = None;
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
        let rows = groups.iter().map(|(_, e)| e.len()).sum::<usize>();
        while self.row_focus.len() < rows {
            self.row_focus.push(cx.focus_handle());
        }
        // The Overview's figure was opened: the first row takes the focus
        // once the chip's list has arrived.
        let loaded = self.stale.as_ref().is_some_and(|ids| !ids.is_empty());
        if self.focus_first_row && loaded && rows > 0 {
            self.focus_first_row = false;
            window.focus(&self.row_focus[0], cx);
        }
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
                    .mx(px(-8.))
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
                let unread = self.ws.read(cx).is_unread(acta_core::model::Record::Kb(id));
                let handle = self.row_focus[index].clone();
                column = column.child(
                    kb_row(&entry, excerpt.as_deref(), unread, index, locale, &t)
                        .track_focus(&handle)
                        .on_key_down(cx.listener(
                            move |page, event: &gpui_kit::KeyDownEvent, window, cx| {
                                page.row_key(index, rows, event, window, cx)
                            },
                        ))
                        .on_click(cx.listener(move |page, _, _, cx| match kind {
                            KbKind::Document => cx.emit(PageEvent::Open(Route::Kb(id))),
                            KbKind::Memory => {
                                page.open_memory = if page.open_memory == Some(id) {
                                    None
                                } else {
                                    // Unfolded is shown: the dot goes.
                                    page.ws.update(cx, |ws, cx| {
                                        ws.mark_seen(acta_core::model::Record::Kb(id), cx)
                                    });
                                    Some(id)
                                };
                                cx.notify();
                            }
                        })),
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
        if as_view {
            ui::view_scroll("knowledge-view")
                .child(column)
                .into_any_element()
        } else {
            ui::page_scroll("knowledge-page")
                .child(column)
                .into_any_element()
        }
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
    /// The `ellipsis` menu of the reading view is open.
    doc_menu: bool,
    doc_menu_anchor: menu::AnchorBounds,
    /// The page's scroll, and where each heading of the body was laid out
    /// (by its index in the outline), for "Go to heading".
    scroll: gpui_kit::ScrollHandle,
    headings: std::rc::Rc<std::cell::RefCell<Vec<Option<gpui_kit::Bounds<gpui_kit::Pixels>>>>>,
    /// The reading view's focus: where the keyboard goes when the editor
    /// closes, so `Mod+Z` reaches the window (interaction.md 6.4).
    focus: FocusHandle,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PageEvent> for DocPage {}

impl Focusable for DocPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DocPage {
    #[cfg(windows)]
    /// The ellipsis menu is open (the chat's web view hides under it).
    pub fn menu_open(&self) -> bool {
        self.doc_menu
    }

    /// The editor is open.
    pub fn editing(&self) -> bool {
        self.mode == Mode::Edit
    }

    /// The kind of a document or memory not saved yet.
    pub fn draft_kind(&self) -> Option<KbKind> {
        self.draft.filter(|_| self.id.is_none()).map(|d| d.kind)
    }

    /// The entry's owner and title, once loaded (the breadcrumb, and the
    /// tree row it selects). A draft has its owner and no title yet.
    pub fn located(&self) -> Option<(Owner, Option<String>)> {
        match (&self.entry, &self.draft) {
            (Some(entry), _) => Some((entry.owner, Some(entry.title.clone()))),
            (None, Some(draft)) => Some((draft.owner, None)),
            _ => None,
        }
    }

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
            doc_menu: false,
            doc_menu_anchor: Default::default(),
            scroll: gpui_kit::ScrollHandle::new(),
            headings: Default::default(),
            focus: cx.focus_handle(),
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
        crate::voice::attach(&body_state, window, cx);
        // Editing an entry starts in its body; a new one needs a title first.
        if self.entry.is_some() {
            body_state.update(cx, |s, cx| s.focus(window, cx));
        } else {
            title_state.update(cx, |s, cx| s.focus(window, cx));
        }
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

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        window.focus(&self.focus, cx);
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
                let _ = page.update_in(cx, |page, window, cx| {
                    page.saving = false;
                    match result {
                        Ok(()) => {
                            page.close_editor(cx);
                            window.focus(&page.focus, cx);
                        }
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
                let _ = page.update_in(cx, |page, window, cx| {
                    page.saving = false;
                    if let Ok(id) = result {
                        page.id = Some(id);
                        page.draft = None;
                        page.close_editor(cx);
                        window.focus(&page.focus, cx);
                        cx.emit(PageEvent::Saved(Route::Kb(id)));
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

    /// "Go to heading..." (`Mod+Shift+O`): the palette as a picker of the
    /// body's headings, each with its size; the chosen one scrolls to the
    /// top of the page.
    pub fn go_to_heading(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = &self.entry else { return };
        if self.mode != Mode::Read {
            return;
        }
        let locale = settings::locale(cx);
        let sections = acta_core::markdown::outline(&entry.body);
        if sections.is_empty() {
            return;
        }
        let choices = sections
            .iter()
            .map(|s| crate::palette::Choice {
                title: format!("{}{}", "  ".repeat(s.level.saturating_sub(1)), s.title),
                detail: format!(
                    "{} \u{b7} {}",
                    s.number,
                    tr(
                        locale,
                        Key::DocSectionLines,
                        &[("n", Arg::Count((s.end - s.start) as u64))],
                    )
                ),
                lead: crate::palette::Lead::Icon(Icon::FileText),
                trailing: String::new(),
                refusal: None,
            })
            .collect();
        let page = cx.entity().downgrade();
        let on_pick = crate::palette::OnPick::new(move |i, _, cx| {
            let _ = page.update(cx, |page, cx| page.scroll_to_heading(i, cx));
        });
        cx.emit(PageEvent::Pick(
            crate::palette::Picker {
                label: text(locale, Key::DocGoHeading),
                choices,
            },
            on_pick,
        ));
    }

    fn scroll_to_heading(&mut self, index: usize, cx: &mut Context<Self>) {
        let bounds = self.headings.borrow().get(index).copied().flatten();
        if let Some(bounds) = bounds {
            ui::scroll_into_view(&self.scroll, bounds, ui::Reveal::Top);
            cx.notify();
        }
    }

    /// "Save as file...": the platform's save dialog, then the Markdown is
    /// written off the UI thread; the toast says the file is not encrypted.
    pub fn save_as_file(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = &self.entry else { return };
        let name = kb_model::file_name(&entry.title);
        let content = kb_model::file_text(&entry.title, &entry.body);
        let start = crate::paths_view::home()
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let chosen = cx.prompt_for_new_path(&start, Some(&name));
        let ws = self.ws.clone();
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let target = path.clone();
            let written = cx
                .background_executor()
                .spawn(async move {
                    acta_core::fsutil::write_private_atomic(&target, content.as_bytes())
                })
                .await;
            ws.update(cx, |ws, cx| {
                let locale = settings::locale(cx);
                match written {
                    Ok(()) => {
                        let shown = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let message = tr(locale, Key::KbSavedFile, &[("name", Arg::Text(&shown))]);
                        ws.toast(message, crate::workspace::ToastKind::Done, cx);
                    }
                    Err(err) => {
                        crate::log::line(&format!("save as file failed: {:?}", err.kind()));
                        ws.toast_error(Key::ErrorSave, cx);
                    }
                }
            });
        })
        .detach();
    }

    /// "Move to...": the palette as a picker of the other levels of the
    /// project; the move is one undo step, toasted with Undo.
    pub fn move_to(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(entry)) = (self.id, &self.entry) else {
            return;
        };
        let locale = settings::locale(cx);
        let kind = entry.kind;
        let Some(tree) = self
            .ws
            .read(cx)
            .projects
            .iter()
            .find(|t| t.project.id == entry.project)
            .cloned()
        else {
            return;
        };
        let targets = kb_model::move_targets(&tree, entry.owner);
        let choices = targets
            .iter()
            .map(|(owner, name, path)| crate::palette::Choice {
                title: name.clone(),
                detail: path.clone(),
                lead: match owner {
                    Owner::Project(_) => crate::palette::Lead::Icon(Icon::Folder),
                    Owner::Epic(_) => crate::palette::Lead::Icon(Icon::Layers),
                    Owner::Topic(t) => crate::palette::Lead::Status(
                        tree.loose_topics
                            .iter()
                            .chain(tree.epics.iter().flat_map(|(_, ts)| ts))
                            .find(|x| x.id == *t)
                            .map_or(acta_core::model::Status::Draft, |x| x.status),
                    ),
                },
                trailing: String::new(),
                refusal: None,
            })
            .collect();
        let ws = self.ws.clone();
        let project = self.project;
        let on_pick = crate::palette::OnPick::new(move |i, _, cx| {
            let Some((owner, name, _)) = targets.get(i).cloned() else {
                return;
            };
            let key = match (owner, kind) {
                (Owner::Project(_), KbKind::Document) => Key::KbMovedProjectDocument,
                (Owner::Project(_), KbKind::Memory) => Key::KbMovedProjectMemory,
                (Owner::Epic(_), KbKind::Document) => Key::KbMovedEpicDocument,
                (Owner::Epic(_), KbKind::Memory) => Key::KbMovedEpicMemory,
                (Owner::Topic(_), KbKind::Document) => Key::KbMovedTopicDocument,
                (Owner::Topic(_), KbKind::Memory) => Key::KbMovedTopicMemory,
            };
            let message = tr(locale, key, &[("name", Arg::Text(&name))]);
            ws.update(cx, |ws, cx| {
                ws.undoable(cx, project, Some(message), move |store| {
                    let back = store.kb_entry(id)?.owner;
                    store.kb_move(id, owner, Author::Human)?;
                    Ok((
                        Undo::KbOwner {
                            id,
                            back,
                            expect: owner,
                        },
                        (),
                    ))
                });
            });
        });
        cx.emit(PageEvent::Pick(
            crate::palette::Picker {
                label: text(locale, Key::KbMoveTo),
                choices,
            },
            on_pick,
        ));
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
            ChangeKind::Created => text(
                locale,
                match kind {
                    KbKind::Document => Key::HistoryCreatedDocument,
                    KbKind::Memory => Key::HistoryCreatedMemory,
                },
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
                .mx(px(-8.))
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
                    // Narrow windows wrap the controls under the version
                    // line instead of clipping it.
                    ui::notice(None, version, t)
                        .flex_wrap()
                        .child(toggle)
                        .child(
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

impl DocPage {
    /// The body in parts cut at its headings, each heading's place kept
    /// for "Go to heading"; 16 between parts, as between paragraphs.
    fn render_body(&self, body: &str, t: &Tokens) -> AnyElement {
        let parts = kb_model::body_parts(body);
        let headings = std::rc::Rc::clone(&self.headings);
        let count = parts.iter().filter(|(h, _)| h.is_some()).count();
        headings.borrow_mut().resize(count, None);
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(parts.into_iter().enumerate().map(|(i, (heading, text))| {
                let headings = std::rc::Rc::clone(&headings);
                div()
                    .relative()
                    .children(heading.map(|h| {
                        ui::measure(move |bounds, _| {
                            if let Some(slot) = headings.borrow_mut().get_mut(h) {
                                *slot = Some(bounds);
                            }
                        })
                    }))
                    .child(markdown_view::document(("doc-body", i), text, t))
            }))
            .into_any_element()
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
                            .rounded(px(crate::theme::RADIUS_LG))
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
                column =
                    column
                        .child(
                            type_scale::TITLE
                                .el()
                                .border_b_1()
                                .border_color(t.border_strong)
                                .child(ui::role_input(Input::new(title), type_scale::TITLE)),
                        )
                        .child(
                            div()
                                .mt(px(8.))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .child(type_scale::READING.el().flex_1().child(ui::role_input(
                                    Input::new(summary),
                                    type_scale::READING,
                                )))
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
                            // The editor's own padding (10 x 8) is taken back, so
                            // the text sits where the reading view puts it.
                            type_scale::DOCUMENT
                                .el()
                                .mt(px(16.))
                                .mx(px(-10.))
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
            // The meta line (components-records.md 5): the id, then who
            // changed it last and when, the full date as its tooltip.
            let (before, after) = crate::record_page::meta_parts(
                locale,
                true,
                entry.stamp.updated_at,
                entry.stamp.updated_by,
            );
            let full = when::datetime(locale, entry.stamp.updated_at);
            let id = entry.id.to_string();
            let copy = id.clone();
            let hover = t.fill_hover;
            let meta = type_scale::SMALL
                .el()
                .flex()
                .flex_row()
                .items_center()
                .min_h(px(20.))
                .mt(px(4.))
                .text_color(t.text_tertiary)
                .font_features(type_scale::tabular())
                .when(!before.is_empty(), |this| this.child(before))
                .child(
                    ui::focusable(type_scale::CODE_SMALL.el().id("doc-meta-id"), &t, false)
                        .px(px(4.))
                        .mx(px(-4.))
                        .rounded(px(crate::theme::RADIUS_XS))
                        .hover(move |s| s.bg(hover))
                        .tooltip(ui::tip(text(locale, Key::MetaCopyId)))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(copy.clone()))
                        })
                        .child(id),
                )
                .child(
                    div()
                        .id("doc-meta-updated")
                        .min_w_0()
                        .truncate()
                        .tooltip(ui::tip(full))
                        .child(after),
                );
            let labels = [text(locale, Key::PropKind), text(locale, Key::PropLevel)];
            let width = ui::label_width(&labels.iter().map(String::as_str).collect::<Vec<_>>(), cx);
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
                        .child(meta),
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
                            // components-knowledge.md 5: Go to heading, Save
                            // as file, Move, then Delete, last and in danger.
                            div()
                                .relative()
                                .child(
                                    ui::ghost_icon_button(
                                        "doc-more",
                                        Icon::Ellipsis,
                                        28.,
                                        self.doc_menu,
                                        false,
                                        true,
                                        &t,
                                    )
                                    .on_click(cx.listener(
                                        |page, _, _, cx| {
                                            page.doc_menu = !page.doc_menu;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(self.doc_menu_anchor.measure())
                                .when(self.doc_menu, |this| {
                                    this.children(
                                        self.doc_menu_anchor.below(
                                            menu::panel("doc-menu", &t)
                                                .on_mouse_down_out(cx.listener(|page, _, _, cx| {
                                                    page.doc_menu = false;
                                                    cx.notify();
                                                }))
                                                .child(
                                                    menu::item(
                                                        "doc-heading",
                                                        None,
                                                        text(locale, Key::DocGoHeading),
                                                        Some(
                                                            ui::kbd(
                                                                &[
                                                                    KeyName::Mod.label(
                                                                        locale,
                                                                        cfg!(target_os = "macos"),
                                                                    ),
                                                                    KeyName::Shift.label(
                                                                        locale,
                                                                        cfg!(target_os = "macos"),
                                                                    ),
                                                                    "O",
                                                                ],
                                                                &t,
                                                            )
                                                            .into_any_element(),
                                                        ),
                                                        false,
                                                        &t,
                                                    )
                                                    .on_click(cx.listener(|page, _, _, cx| {
                                                        page.doc_menu = false;
                                                        page.go_to_heading(cx)
                                                    })),
                                                )
                                                .child(
                                                    menu::item(
                                                        "doc-save-file",
                                                        None,
                                                        text(locale, Key::KbSaveFile),
                                                        None,
                                                        false,
                                                        &t,
                                                    )
                                                    .on_click(cx.listener(|page, _, _, cx| {
                                                        page.doc_menu = false;
                                                        page.save_as_file(cx)
                                                    })),
                                                )
                                                .child(
                                                    menu::item(
                                                        "doc-move",
                                                        None,
                                                        text(locale, Key::KbMoveTo),
                                                        None,
                                                        false,
                                                        &t,
                                                    )
                                                    .on_click(cx.listener(|page, _, _, cx| {
                                                        page.doc_menu = false;
                                                        page.move_to(cx)
                                                    })),
                                                )
                                                .child(menu::separator(&t))
                                                .child(
                                                    menu::item(
                                                        "doc-delete",
                                                        None,
                                                        text(locale, Key::CommonDelete),
                                                        None,
                                                        false,
                                                        &t,
                                                    )
                                                    .text_color(t.danger)
                                                    .on_click(cx.listener(|page, _, _, cx| {
                                                        page.doc_menu = false;
                                                        page.delete(cx)
                                                    })),
                                                ),
                                            menu::Align::End,
                                        ),
                                    )
                                }),
                        ),
                )
                .child(div().mt(px(24.)).h(px(1.)).bg(t.border_subtle))
                .child(div().mt(px(24.)).child(if self.mode == Mode::History {
                    self.render_history(entry, locale, &t, cx)
                } else {
                    self.render_body(&entry.body, &t)
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
                // Dictation (components-voice.md 5): after "Editing", with
                // its line in the bar.
                .children(
                    self.body
                        .as_ref()
                        .and_then(|b| crate::voice::ui::mic_for(b, cx)),
                )
                .children(
                    self.body
                        .as_ref()
                        .and_then(|b| crate::voice::ui::line_in_bar_for(b, cx)),
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
                    .on_click(cx.listener(|page, _, window, cx| page.cancel(window, cx))),
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
            .track_focus(&self.focus)
            .on_action(cx.listener(|page, _: &Escape, window, cx| {
                if page.mode == Mode::History {
                    page.mode = Mode::Read;
                    cx.notify();
                } else {
                    page.cancel(window, cx);
                }
            }))
            .child(
                div().flex_1().min_h_0().child(
                    ui::page_scroll("doc-page")
                        .track_scroll(&self.scroll)
                        .child(column),
                ),
            )
            .children(save_bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_version_reads_created() {
        let label = |change, kind| DocPage::change_label(Locale::En, change, None, kind);
        assert_eq!(label(ChangeKind::Created, KbKind::Document), "Created");
        assert_eq!(
            DocPage::change_label(Locale::Fr, ChangeKind::Created, None, KbKind::Memory),
            "Créée"
        );
        assert_eq!(label(ChangeKind::Edited, KbKind::Document), "Edited");
        assert_eq!(
            label(ChangeKind::CreatedFromFile, KbKind::Memory),
            "Created from file"
        );
    }
}
