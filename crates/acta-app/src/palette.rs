//! The command palette, Acta's search everywhere (docs/design/components.md
//! 5; screens.md 8.3): 640 wide, 96 below the window top, no scrim. Topics,
//! knowledge, commands and projects in a fixed group order; `Tab` narrows
//! to the selected row's group; `Enter` runs; with nothing found, `Enter`
//! creates a topic with the typed title.

use acta_core::model::{KbKind, PageReq, ProjectTree, Record, TopicId};
use acta_core::store::{SearchKind, SearchQuery};
use gpui_kit::component::input::{
    Escape, IndentInline, Input, InputEvent, InputState, MoveDown, MoveUp,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Task, Window, div, px,
};

use crate::commands::{self, Command};
use crate::db::Db;
use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::kb_model;
use crate::palette_model::{self, Group, Item, Target};
use crate::settings;
use crate::theme::{RADIUS_SM, RADIUS_XL, Tokens};
use crate::type_scale;
use crate::ui;

/// docs/design/components.md 5: up to 10 rows.
const ROWS: usize = 10;

/// What Enter makes when nothing matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateKind {
    Topic,
    Epic,
}

pub enum PaletteEvent {
    /// Open or run the target; `true`: in a new tab (`Mod+Enter`).
    Run(Target, bool),
    Create(CreateKind, String),
    Dismiss,
}

pub struct Palette {
    input: Entity<InputState>,
    scope: Option<Group>,
    /// Asked for by "New epic...": the typed text is the epic's title.
    create: Option<CreateKind>,
    projects: Vec<ProjectTree>,
    recent: Vec<TopicId>,
    context: commands::Context,
    db: Db,
    items: Vec<Item>,
    knowledge: Vec<Item>,
    selected: usize,
    _search: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PaletteEvent> for Palette {}

pub struct Setup {
    pub scope: Option<Group>,
    pub create: Option<CreateKind>,
    pub projects: Vec<ProjectTree>,
    pub recent: Vec<TopicId>,
    pub context: commands::Context,
    pub db: Db,
}

impl Palette {
    pub fn new(setup: Setup, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let locale = settings::locale(cx);
        let placeholder = match setup.create {
            Some(CreateKind::Epic) => Command::NewEpic.label(locale),
            Some(CreateKind::Topic) => text(locale, Key::TopicNew),
            None => text(locale, Key::CommonSearch),
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        input.update(cx, |state, cx| state.focus(window, cx));
        let subs = vec![cx.subscribe_in(
            &input,
            window,
            |palette, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => palette.refresh(cx),
                InputEvent::PressEnter { secondary, .. } => palette.run(*secondary, cx),
                _ => {}
            },
        )];
        let mut palette = Self {
            input,
            scope: setup.scope,
            create: setup.create,
            projects: setup.projects,
            recent: setup.recent,
            context: setup.context,
            db: setup.db,
            items: Vec::new(),
            knowledge: Vec::new(),
            selected: 0,
            _search: Task::ready(()),
            _subs: subs,
        };
        palette.refresh(cx);
        palette
    }

    fn query(&self, cx: &Context<Self>) -> String {
        self.input.read(cx).value().trim().to_owned()
    }

    /// Recompute the in-memory groups now, and start the knowledge search.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let locale = settings::locale(cx);
        let query = self.query(cx);
        self.items = self.local_items(&query, locale);
        self.selected = 0;
        if self.create.is_none()
            && !query.is_empty()
            && self.scope.is_none_or(|s| s == Group::Knowledge)
        {
            let db = self.db.clone();
            let task = db.run(cx, move |store| {
                store.search(
                    &SearchQuery {
                        text: query,
                        project: None,
                        kinds: vec![SearchKind::Kb],
                        ..SearchQuery::default()
                    },
                    PageReq::first(5),
                )
            });
            self._search = cx.spawn(async move |palette, cx| {
                let hits = task.await;
                let _ = palette.update(cx, |palette, cx| {
                    palette.knowledge = hits
                        .map(|page| {
                            page.items
                                .into_iter()
                                .filter_map(|hit| match hit.record {
                                    Record::Kb(id) => Some(Item {
                                        group: Group::Knowledge,
                                        target: Target::Kb(id, KbKind::Document),
                                        title: hit.title,
                                        detail: kb_model::excerpt_plain(&hit.snippet),
                                        matched: vec![],
                                        score: 0,
                                    }),
                                    _ => None,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    cx.notify();
                });
            });
        } else {
            self.knowledge.clear();
        }
        cx.notify();
    }

    fn local_items(&self, query: &str, locale: Locale) -> Vec<Item> {
        if self.create.is_some() {
            return Vec::new();
        }
        let mut items = Vec::new();
        for tree in &self.projects {
            let project = &tree.project;
            if let Some((score, matched)) = palette_model::fuzzy(query, &project.title) {
                items.push(Item {
                    group: Group::Projects,
                    target: Target::Project(project.id),
                    title: project.title.clone(),
                    detail: String::new(),
                    matched,
                    score,
                });
            }
            let loose = tree.loose_topics.iter().map(|t| (t, project.title.clone()));
            let in_epics = tree.epics.iter().flat_map(|(e, topics)| {
                topics
                    .iter()
                    .map(move |t| (t, format!("{} / {}", project.title, e.title)))
            });
            for (topic, path) in loose.chain(in_epics) {
                if query.is_empty() {
                    let rank = self.recent.iter().position(|id| *id == topic.id);
                    if let Some((group, score)) =
                        palette_model::idle_topic(rank, self.scope, topic.updated_at)
                    {
                        items.push(Item {
                            group,
                            target: Target::Topic(topic.id, topic.status),
                            title: topic.title.clone(),
                            detail: path,
                            matched: vec![],
                            score,
                        });
                    }
                    continue;
                }
                if let Some((score, matched)) = palette_model::fuzzy(query, &topic.title) {
                    items.push(Item {
                        group: Group::Topics,
                        target: Target::Topic(topic.id, topic.status),
                        title: topic.title.clone(),
                        detail: path,
                        matched,
                        // Recency breaks ties: newer writes first.
                        score: score * 1_000_000 + (topic.updated_at / 1000) % 1_000_000,
                    });
                }
            }
        }
        for command in Command::available(self.context) {
            let label = command.label(locale);
            if let Some((score, matched)) = palette_model::fuzzy(query, &label) {
                items.push(Item {
                    group: Group::Commands,
                    target: Target::Command(command),
                    title: label,
                    detail: String::new(),
                    matched,
                    score,
                });
            }
        }
        items
    }

    fn shown(&self) -> Vec<Item> {
        let mut all = self.items.clone();
        all.extend(self.knowledge.iter().cloned());
        let limit = if self.scope.is_some() { ROWS } else { 5 };
        let mut out = palette_model::arrange(all, self.scope, limit);
        out.truncate(ROWS);
        out
    }

    fn run(&mut self, new_tab: bool, cx: &mut Context<Self>) {
        let query = self.query(cx);
        if let Some(kind) = self.create {
            if !query.is_empty() {
                cx.emit(PaletteEvent::Create(kind, query));
            }
            return;
        }
        let shown = self.shown();
        match shown.get(self.selected) {
            Some(item) => cx.emit(PaletteEvent::Run(item.target.clone(), new_tab)),
            None if !query.is_empty() => cx.emit(PaletteEvent::Create(CreateKind::Topic, query)),
            None => {}
        }
    }

    fn step(&mut self, down: bool, cx: &mut Context<Self>) {
        let len = self.shown().len();
        self.selected = palette_model::step(self.selected, len, down);
        cx.notify();
    }

    /// `Tab` narrows to the selected row's group.
    fn narrow(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = self.shown().get(self.selected) {
            self.scope = Some(match item.group {
                Group::Recent => Group::Topics,
                other => other,
            });
            self.refresh(cx);
        }
    }

    fn row(
        &self,
        item: &Item,
        index: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = index == self.selected;
        let leading: AnyElement = match &item.target {
            Target::Topic(_, status) => ui::status_glyph(*status, 16., t).into_any_element(),
            Target::Project(_) => icon(Icon::Folder, 16., t.text_secondary).into_any_element(),
            Target::Kb(_, KbKind::Memory) => {
                icon(Icon::Pin, 16., t.text_secondary).into_any_element()
            }
            Target::Kb(..) => icon(Icon::FileText, 16., t.text_secondary).into_any_element(),
            // No glyph for commands: chevron-right is Claude Code's author
            // mark, and commands have no kind to show; the slot keeps the
            // titles aligned.
            Target::Command(_) => div().size(px(16.)).flex_shrink_0().into_any_element(),
        };
        // One text run per matched stretch, so the title truncates with an
        // ellipsis like any other text (components.md 5, Command palette):
        // matched characters in text.primary at 600 on a text.secondary
        // title.
        let rest = if item.matched.is_empty() {
            t.text_primary
        } else {
            t.text_secondary
        };
        let strong = gpui_kit::HighlightStyle {
            color: Some(t.text_primary),
            font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
            ..Default::default()
        };
        let highlights: Vec<(std::ops::Range<usize>, gpui_kit::HighlightStyle)> = item
            .title
            .char_indices()
            .enumerate()
            .filter(|(i, _)| item.matched.contains(i))
            .map(|(_, (at, c))| (at..at + c.len_utf8(), strong))
            .collect();
        let title = type_scale::UI
            .el()
            .flex_shrink(1.)
            .min_w_0()
            .truncate()
            .text_color(rest)
            .child(gpui_kit::StyledText::new(item.title.clone()).with_highlights(highlights));
        let shortcut = match &item.target {
            Target::Command(command) => command.shortcut(locale, cfg!(target_os = "macos")),
            _ => vec![],
        };
        let hover = t.fill_hover;
        div()
            .id(("palette-row", index))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(12.))
            .rounded(px(RADIUS_SM))
            .when(selected, |this| this.bg(t.fill_selected))
            .when(!selected, |this| this.hover(move |s| s.bg(hover)))
            .on_click(
                cx.listener(move |palette, event: &gpui_kit::ClickEvent, _, cx| {
                    palette.selected = index;
                    let new_tab = event.modifiers().secondary(cx);
                    palette.run(new_tab, cx);
                }),
            )
            .child(leading)
            .child(title)
            .child(
                type_scale::SMALL
                    .el()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    // On the selected overlay row tertiary is too faint
                    // (2.2): the path goes up to text.secondary.
                    .text_color(if selected {
                        t.text_secondary
                    } else {
                        t.text_tertiary
                    })
                    .child(item.detail.clone()),
            )
            .when(!shortcut.is_empty(), |this| {
                this.child(ui::kbd(&shortcut, t))
            })
            .into_any_element()
    }
}

fn group_label(locale: Locale, group: Group) -> String {
    text(
        locale,
        match group {
            Group::Recent => Key::PaletteRecent,
            Group::Topics => Key::PaletteTopics,
            Group::Knowledge => Key::SectionKnowledge,
            Group::Commands => Key::PaletteCommands,
            Group::Projects => Key::PaletteProjects,
        },
    )
}

impl Render for Palette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let width = (window.viewport_size().width - px(48.)).min(px(640.));
        let shown = self.shown();
        let query = self.query(cx);
        let mut results = div().flex().flex_col().p(px(4.));
        let mut last: Option<Group> = None;
        for (i, item) in shown.iter().enumerate() {
            if last != Some(item.group) {
                last = Some(item.group);
                results = results.child(
                    type_scale::LABEL
                        .el()
                        .h(px(28.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .text_color(t.text_tertiary)
                        .child(group_label(locale, item.group)),
                );
            }
            results = results.child(self.row(item, i, locale, &t, cx));
        }
        if shown.is_empty() && !query.is_empty() && self.create.is_none() {
            let keys = KeyName::Enter.label(locale, macos);
            results = results.child(
                type_scale::UI
                    .el()
                    .px(px(12.))
                    .py(px(8.))
                    .text_color(t.text_secondary)
                    .child(tr(
                        locale,
                        Key::PaletteNone,
                        &[("query", Arg::Text(&query)), ("keys", Arg::Text(keys))],
                    )),
            );
        }
        let chip = self.scope.map(|scope| {
            type_scale::SMALL
                .el()
                .flex_shrink_0()
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(RADIUS_SM))
                .bg(t.accent_subtle)
                .text_color(t.text_primary)
                .child(group_label(locale, scope))
        });
        let m = KeyName::Mod.label(locale, macos);
        let enter = KeyName::Enter.label(locale, macos);
        let esc = KeyName::Esc.label(locale, macos);
        let hint = |key: Key, keys: &[&str]| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(ui::kbd(keys, &t))
                .child(
                    type_scale::SMALL.el().text_color(t.text_tertiary).child(
                        tr(locale, key, &[("keys", Arg::Text(""))])
                            .trim()
                            .to_owned(),
                    ),
                )
        };
        div()
            .id("palette")
            .key_context("Palette")
            .occlude()
            .w(width)
            .flex()
            .flex_col()
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_XL))
            .shadow(vec![t.shadow_overlay()])
            .capture_action(cx.listener(|palette, _: &MoveUp, _, cx| {
                palette.step(false, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|palette, _: &MoveDown, _, cx| {
                palette.step(true, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|palette, _: &IndentInline, _, cx| {
                palette.narrow(cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|_, _: &Escape, _, cx| {
                cx.emit(PaletteEvent::Dismiss);
                cx.stop_propagation();
            }))
            .capture_key_down(
                cx.listener(|palette, event: &gpui_kit::KeyDownEvent, _, cx| {
                    // Backspace on an empty input removes the chip (8.3).
                    if event.keystroke.key == "backspace"
                        && palette.scope.is_some()
                        && palette.query(cx).is_empty()
                    {
                        palette.scope = None;
                        palette.refresh(cx);
                    }
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(44.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(t.border_default)
                    .child(icon(Icon::Search, 16., t.text_tertiary))
                    .children(chip)
                    .child(
                        type_scale::READING.el().flex_1().child(
                            Input::new(&self.input)
                                .appearance(false)
                                .px(px(0.))
                                .text_size(px(type_scale::READING.size()))
                                .line_height(px(type_scale::READING.line())),
                        ),
                    ),
            )
            .when(!shown.is_empty() || !query.is_empty(), |this| {
                this.child(results)
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(16.))
                    .h(px(28.))
                    .px(px(12.))
                    .border_t_1()
                    .border_color(t.border_default)
                    // Naming a new topic or epic: Enter creates it, and
                    // there is nothing to open in a tab.
                    .map(|this| {
                        if self.create.is_some() {
                            this.child(hint(Key::PaletteHintCreate, &[enter]))
                        } else {
                            this.child(hint(Key::PaletteHintOpen, &[enter]))
                                .child(hint(Key::PaletteHintTab, &[m, enter]))
                        }
                    })
                    .child(div().flex_1())
                    .child(hint(Key::PaletteHintClose, &[esc])),
            )
    }
}
