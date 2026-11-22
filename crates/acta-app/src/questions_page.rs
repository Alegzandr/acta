//! The Questions page (docs/design/components-questions.md 5; screens.md
//! 8.22): every question of a project, an epic or a topic, grouped by
//! owner widest first, with the Open / Overdue / Closed / All segments.
//! `Enter` on a row opens its owner's page; the section there is where a
//! question is edited.

use std::collections::{HashMap, HashSet};

use acta_core::date::Date;
use acta_core::model::{EpicId, Owner, ProjectId, Question, QuestionId, TopicId};
use acta_core::store::QuestionFilter;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, div, px,
};

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
use crate::workspace::Workspace;

/// What the page shows, read in one go.
#[derive(Clone, Debug)]
struct Loaded {
    project: (ProjectId, String),
    questions: Vec<Question>,
    order: HashMap<Owner, usize>,
    names: HashMap<Owner, String>,
    unread: HashSet<QuestionId>,
}

pub struct QuestionsPage {
    ws: Entity<Workspace>,
    pub scope: Owner,
    filter: Filter,
    data: Option<Loaded>,
    focus: FocusHandle,
    row_focus: Vec<FocusHandle>,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
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
        let page = cx.entity().downgrade();
        let page2 = page.clone();
        let logged = q.decision_entry;
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
    use acta_core::model::{EntryId, EpicId, TopicId};

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
