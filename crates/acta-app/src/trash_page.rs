//! The trash (docs/design/components-app.md 5, Trash page): what was
//! deleted, grouped by day, restorable for 30 days. Rows are a list with
//! multi-selection; the toolbar restores the selection in one undo step,
//! or deletes it (or everything) permanently after a confirmation
//! (data.md 11, request 25; ADR 0006 amendment). The human's only: the
//! purge is never offered to Claude Code.

use acta_core::model::{PageReq, ProjectId, Record};
use acta_core::store::TrashItem;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Task,
    Window, div,
};

use crate::commands::Command;
use crate::dialogs::Confirm;
use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::palette::OnPick;
use crate::record_page::PageEvent;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;
use crate::when;
use crate::workspace::Workspace;
use crate::zoom::px;

/// What a contact's trash row says instead of the days left
/// (components-contacts.md 5): merged into another, or kept while named.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ContactNote {
    MergedInto(String),
    KeptNamed(usize),
    /// A removed file: the record it belonged to (its place).
    Owner(String),
}

/// The selection of the trash list (components-app.md 5, Trash page;
/// interaction.md 6.2): records, so a reload keeps it. `rows` is always
/// the selectable rows in display order (a contact still named is not).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub selected: Vec<Record>,
    /// Where `Shift`+click and `Shift+Up` / `Down` extend from.
    pub anchor: Option<Record>,
    /// The keyboard's row.
    pub cursor: Option<Record>,
}

impl Selection {
    pub fn contains(&self, r: Record) -> bool {
        self.selected.contains(&r)
    }

    /// A click: that row alone.
    pub fn click(&mut self, r: Record) {
        self.selected = vec![r];
        self.anchor = Some(r);
        self.cursor = Some(r);
    }

    /// `Mod`+click or `Space`: add or remove one row.
    pub fn toggle(&mut self, r: Record) {
        if let Some(i) = self.selected.iter().position(|s| *s == r) {
            self.selected.remove(i);
        } else {
            self.selected.push(r);
        }
        self.anchor = Some(r);
        self.cursor = Some(r);
    }

    /// `Shift`+click: the range from the anchor (the last clicked row).
    pub fn extend_to(&mut self, rows: &[Record], r: Record) {
        let Some(to) = rows.iter().position(|x| *x == r) else {
            return;
        };
        let from = self
            .anchor
            .and_then(|a| rows.iter().position(|x| *x == a))
            .unwrap_or(to);
        let (a, b) = (from.min(to), from.max(to));
        self.selected = rows[a..=b].to_vec();
        self.anchor.get_or_insert(r);
        self.cursor = Some(r);
    }

    /// `Up` / `Down` move the cursor; with `Shift` the selection follows
    /// from the anchor.
    pub fn step(&mut self, rows: &[Record], down: bool, extend: bool) {
        if rows.is_empty() {
            return;
        }
        let at = self.cursor.and_then(|c| rows.iter().position(|x| *x == c));
        let next = match (at, down) {
            (None, true) => 0,
            (None, false) => rows.len() - 1,
            (Some(i), true) => (i + 1).min(rows.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
        };
        let r = rows[next];
        if extend {
            if self.anchor.is_none() {
                self.anchor = at.map(|i| rows[i]).or(Some(r));
            }
            self.extend_to(rows, r);
        } else {
            self.cursor = Some(r);
        }
    }

    /// `Mod+A`.
    pub fn all(&mut self, rows: &[Record]) {
        self.selected = rows.to_vec();
    }

    /// `Esc`.
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// After a reload: forget what is no longer listed.
    pub fn retain(&mut self, rows: &[Record]) {
        self.selected.retain(|r| rows.contains(r));
        if self.anchor.is_some_and(|a| !rows.contains(&a)) {
            self.anchor = None;
        }
        if self.cursor.is_some_and(|c| !rows.contains(&c)) {
            self.cursor = None;
        }
    }
}

pub struct TrashPage {
    ws: Entity<Workspace>,
    items: Vec<TrashItem>,
    contacts: std::collections::HashMap<Record, ContactNote>,
    /// A project's trashed content, (topics, notes).
    content: std::collections::HashMap<Record, (u64, u64)>,
    selection: Selection,
    focus: FocusHandle,
    loaded: bool,
    seen_revision: u64,
    _load: Task<()>,
    _purge: Task<()>,
    _sub: Subscription,
}

impl EventEmitter<PageEvent> for TrashPage {}

/// Group items by the local day they were deleted, newest day first.
pub fn by_day(items: &[TrashItem]) -> Vec<(i64, Vec<&TrashItem>)> {
    let mut groups: Vec<(i64, Vec<&TrashItem>)> = Vec::new();
    for item in items {
        let day = when::local(item.deleted_at).day_number();
        match groups.iter_mut().find(|(d, _)| *d == day) {
            Some((_, group)) => group.push(item),
            None => groups.push((day, vec![item])),
        }
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.0));
    groups
}

/// Whole days left before the 30 days are over (never below zero).
pub fn days_left(item: &TrashItem, now: i64) -> u64 {
    let ms = (item.expires_at - now).max(0);
    (ms as u64).div_ceil(acta_core::time::MS_PER_DAY as u64)
}

fn kind_icon(record: Record) -> Icon {
    match record {
        Record::Project(_) => Icon::Folder,
        Record::Epic(_) => Icon::Layers,
        Record::Topic(_) => Icon::Circle,
        Record::Entry(_) => Icon::MessageSquare,
        Record::Note(_) => Icon::StickyNote,
        Record::Kb(_) => Icon::FileText,
        Record::Folder(_) => Icon::FolderOpen,
        Record::Question(_) => Icon::CircleQuestionMark,
        Record::Contact(_) => Icon::User,
        Record::Attachment(_) => Icon::Paperclip,
    }
}

/// The title of the confirmation: the count, or "Empty the trash?".
pub fn confirm_title(locale: Locale, n: usize, all: bool) -> String {
    if all {
        text(locale, Key::TrashConfirmEmptyTitle)
    } else {
        tr(
            locale,
            Key::TrashConfirmTitle,
            &[("n", Arg::Count(n as u64))],
        )
    }
}

/// "31 topics, 12 notes": a trashed project's content.
fn content_text(locale: Locale, topics: u64, notes: u64) -> String {
    let topics = tr(locale, Key::CountTopics, &[("n", Arg::Count(topics))]);
    let notes = tr(locale, Key::CountNotes, &[("n", Arg::Count(notes))]);
    tr(
        locale,
        Key::TrashProjectContent,
        &[("topics", Arg::Text(&topics)), ("notes", Arg::Text(&notes))],
    )
}

impl TrashPage {
    pub fn new(ws: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let sub = cx.observe(&ws, |page, ws, cx| {
            let revision = ws.read(cx).revision;
            if revision != page.seen_revision {
                page.seen_revision = revision;
                page.reload(cx);
            }
        });
        let mut page = Self {
            ws,
            items: Vec::new(),
            contacts: std::collections::HashMap::new(),
            content: std::collections::HashMap::new(),
            selection: Selection::default(),
            focus: cx.focus_handle(),
            loaded: false,
            seen_revision: 0,
            _load: Task::ready(()),
            _purge: Task::ready(()),
            _sub: sub,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = self.ws.read(cx).db.run(cx, |store| {
            let page = store.trash(PageReq::first(PageReq::MAX_LIMIT))?;
            let mut notes = std::collections::HashMap::new();
            let mut content = std::collections::HashMap::new();
            for item in &page.items {
                if let Record::Project(id) = item.record {
                    content.insert(item.record, store.trash_project_content(id)?);
                }
                // A removed file's place is its owner (components-attachments.md 5).
                if let Record::Attachment(id) = item.record {
                    let owner = store.attachment_owner(id)?;
                    notes.insert(item.record, ContactNote::Owner(owner.to_string()));
                }
                if let Record::Contact(id) = item.record {
                    let c = store.contact_any(id)?;
                    let note = match c.merged_into {
                        Some(into) => store
                            .contact_any(into)
                            .ok()
                            .map(|i| ContactNote::MergedInto(i.name)),
                        None => {
                            let n = store.contact_uses(id)?.len();
                            (n > 0).then_some(ContactNote::KeptNamed(n))
                        }
                    };
                    if let Some(note) = note {
                        notes.insert(item.record, note);
                    }
                }
            }
            Ok((page, notes, content))
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok((items, contacts, content)) = loaded {
                    page.items = items.items;
                    page.contacts = contacts;
                    page.content = content;
                    let rows = page.rows();
                    page.selection.retain(&rows);
                }
                page.loaded = true;
                cx.notify();
            });
        });
    }

    /// The selectable rows in display order: a contact still named is
    /// never deleted, so it is not selectable.
    fn rows(&self) -> Vec<Record> {
        by_day(&self.items)
            .into_iter()
            .flat_map(|(_, group)| group.into_iter().map(|i| i.record))
            .filter(|r| self.selectable(*r))
            .collect()
    }

    fn selectable(&self, r: Record) -> bool {
        !matches!(self.contacts.get(&r), Some(ContactNote::KeptNamed(_)))
    }

    /// For the palette: (anything selected, anything in the trash).
    pub fn command_state(&self) -> (bool, bool) {
        (!self.selection.selected.is_empty(), !self.items.is_empty())
    }

    /// A palette command of the toolbar.
    pub fn command(&mut self, command: Command, _: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::TrashRestore => self.restore_selected(cx),
            Command::TrashDelete => self.confirm_purge(false, cx),
            Command::TrashEmpty => self.confirm_purge(true, cx),
            _ => {}
        }
    }

    fn restore_selected(&mut self, cx: &mut Context<Self>) {
        let records = self.selection.selected.clone();
        let Some(first) = records.first().copied() else {
            return;
        };
        let locale = settings::locale(cx);
        let project = self
            .items
            .iter()
            .find(|i| i.record == first)
            .map(|i| i.project);
        let one = records.len() == 1;
        let key = match first {
            Record::Topic(_) => Key::TopicRestored,
            Record::Attachment(_) => Key::AttachRestored,
            _ => Key::TrashRestoredItem,
        };
        self.items.retain(|i| !records.contains(&i.record));
        self.selection.clear();
        cx.notify();
        self.ws.update(cx, |ws, cx| {
            ws.restore_many(
                records,
                project,
                move |n| {
                    if one && n == 1 {
                        text(locale, key)
                    } else {
                        tr(locale, Key::TrashRestoredN, &[("n", Arg::Count(n as u64))])
                    }
                },
                cx,
            )
            .detach()
        });
    }

    /// "Delete permanently..." on the selection, or "Empty trash...":
    /// the dialog names the count; nothing happens until confirmed.
    fn confirm_purge(&mut self, all: bool, cx: &mut Context<Self>) {
        let records: Vec<Record> = if all {
            self.items
                .iter()
                .map(|i| i.record)
                .filter(|r| self.selectable(*r))
                .collect()
        } else {
            self.selection.selected.clone()
        };
        if records.is_empty() {
            return;
        }
        let locale = settings::locale(cx);
        let mut body = String::new();
        if all {
            body.push_str(&tr(
                locale,
                Key::CountItems,
                &[("n", Arg::Count(records.len() as u64))],
            ));
            body.push_str(". ");
        }
        let projects: Vec<(u64, u64)> = records
            .iter()
            .filter_map(|r| self.content.get(r).copied())
            .collect();
        if !projects.is_empty() {
            let (topics, notes) = projects
                .iter()
                .fold((0, 0), |(t, n), (a, b)| (t + a, n + b));
            let content = content_text(locale, topics, notes);
            body.push_str(&tr(
                locale,
                Key::TrashConfirmProject,
                &[
                    ("n", Arg::Count(projects.len() as u64)),
                    ("content", Arg::Text(&content)),
                ],
            ));
            body.push_str(". ");
        }
        body.push_str(&text(locale, Key::TrashConfirmBody));
        let page = cx.entity().downgrade();
        cx.emit(PageEvent::Confirm(Confirm {
            title: confirm_title(locale, records.len(), all),
            body,
            confirm: text(locale, Key::TrashConfirmDelete),
            on_confirm: OnPick::new(move |_, _, cx| {
                let records = records.clone();
                let _ = page.update(cx, |page, cx| page.purge(records, all, cx));
            }),
        }));
    }

    /// The purge itself, off the UI thread. No undo and no toast: the rows
    /// disappear; a failure keeps them and says so.
    fn purge(&mut self, records: Vec<Record>, all: bool, cx: &mut Context<Self>) {
        let task = self.ws.read(cx).db.run(cx, move |store| {
            if all {
                store.trash_purge_all()
            } else {
                store.trash_purge(&records)
            }
        });
        self._purge = cx.spawn(async move |page, cx| {
            let done = task.await;
            let _ = page.update(cx, |page, cx| {
                page.selection.clear();
                let ws = page.ws.clone();
                ws.update(cx, |ws, cx| {
                    if let Err(err) = &done {
                        // The blocking tables' names, never values.
                        crate::log::line(&format!("trash: purge failed: {err}"));
                        ws.toast_error(Key::TrashDeleteFailed, cx);
                    }
                    ws.reload(cx);
                });
                page.reload(cx);
            });
        });
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        let m = &k.modifiers;
        let rows = self.rows();
        let plain = !m.control && !m.alt && !m.platform;
        let handled = match k.key.as_str() {
            "up" | "down" if plain => {
                self.selection.step(&rows, k.key == "down", m.shift);
                true
            }
            "space" if plain && !m.shift => {
                if let Some(c) = self.selection.cursor {
                    self.selection.toggle(c);
                }
                true
            }
            "a" if m.secondary(cx) && !m.shift && !m.alt => {
                self.selection.all(&rows);
                true
            }
            "escape" if !self.selection.selected.is_empty() => {
                self.selection.clear();
                true
            }
            "enter" | "r" if plain && !m.shift => {
                self.restore_selected(cx);
                true
            }
            "delete" if plain && !m.shift => {
                self.confirm_purge(false, cx);
                true
            }
            "backspace" if m.platform && cfg!(target_os = "macos") => {
                self.confirm_purge(false, cx);
                true
            }
            _ => false,
        };
        if handled {
            window.focus(&self.focus, cx);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn project_title(&self, id: ProjectId, cx: &Context<Self>) -> String {
        self.ws
            .read(cx)
            .projects
            .iter()
            .find(|t| t.project.id == id)
            .map(|t| t.project.title.clone())
            .unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        item: &TrashItem,
        index: usize,
        locale: Locale,
        now: i64,
        focused: bool,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let record = item.record;
        let selectable = self.selectable(record);
        let selected = self.selection.contains(record);
        let cursor = focused && self.selection.cursor == Some(record);
        let by = tr(
            locale,
            Key::TrashBy,
            &[("author", Arg::Text(&when::author(locale, item.deleted_by)))],
        );
        let left = match self.contacts.get(&record) {
            Some(ContactNote::MergedInto(name)) => {
                tr(locale, Key::TrashMergedInto, &[("name", Arg::Text(name))])
            }
            Some(ContactNote::KeptNamed(n)) => {
                tr(locale, Key::TrashKeptNamed, &[("n", Arg::Count(*n as u64))])
            }
            Some(ContactNote::Owner(_)) | None => tr(
                locale,
                Key::TrashDays,
                &[("n", Arg::Count(days_left(item, now)))],
            ),
        };
        let place = match (self.contacts.get(&record), self.content.get(&record)) {
            // A project row counts its content (components-app.md 5).
            (_, Some((topics, notes))) => content_text(locale, *topics, *notes),
            (Some(ContactNote::Owner(owner)), _) => {
                format!("{} \u{b7} {owner}", self.project_title(item.project, cx))
            }
            _ => self.project_title(item.project, cx),
        };
        // Selected rows switch their metadata to text.secondary (2.2).
        let meta = if selected {
            t.text_secondary
        } else {
            t.text_tertiary
        };
        let ring = ui::ring_shadow(t, false);
        let (hover, selected_bg) = (t.fill_hover, t.fill_selected);
        div()
            .id(("trash-row", index))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(8.))
            .mx(px(-8.))
            .rounded(px(RADIUS_SM))
            .when(selected, |this| this.bg(selected_bg))
            .when(cursor, |this| this.shadow(ring))
            .when(!selected && selectable, |this| {
                this.hover(move |s| s.bg(hover))
            })
            .when(!selectable, |this| {
                this.tooltip(ui::tip(text(locale, Key::TrashKeptTip)))
            })
            .when(selectable, |this| {
                this.on_click(
                    cx.listener(move |page, e: &gpui_kit::ClickEvent, window, cx| {
                        let m = e.modifiers();
                        if m.shift {
                            let rows = page.rows();
                            page.selection.extend_to(&rows, record);
                        } else if m.secondary(cx) {
                            page.selection.toggle(record);
                        } else {
                            page.selection.click(record);
                        }
                        window.focus(&page.focus, cx);
                        cx.notify();
                    }),
                )
            })
            .child(icon(
                kind_icon(record),
                16.,
                if selectable {
                    t.text_secondary
                } else {
                    t.text_disabled
                },
            ))
            .child(
                type_scale::UI
                    .el()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text_primary)
                    .child(item.label.lines().next().unwrap_or_default().to_owned()),
            )
            .child(type_scale::SMALL.el().text_color(meta).child(place))
            .child(type_scale::SMALL.el().text_color(meta).child(by))
            .child(
                type_scale::SMALL
                    .el()
                    .font_features(type_scale::tabular())
                    .text_color(meta)
                    .child(left),
            )
            .into_any_element()
    }

    /// The toolbar (28, always there so selecting moves nothing):
    /// "{n} selected", Restore and Delete permanently... at the left,
    /// Empty trash... at the right.
    fn toolbar(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let n = self.selection.selected.len();
        let none = n == 0;
        let (danger, hover) = (t.danger, t.fill_hover);
        let delete = ui::button(
            "trash-delete",
            text(locale, Key::TrashDeleteSelected),
            ui::Variant::Ghost,
            none,
            t,
        )
        .when(!none, |this| {
            this.text_color(danger)
                .hover(move |s| s.bg(hover).text_color(danger))
                .on_click(cx.listener(|page, _, _, cx| page.confirm_purge(false, cx)))
        });
        let restore = ui::button(
            "trash-restore",
            text(locale, Key::CommonRestore),
            ui::Variant::Secondary,
            none,
            t,
        )
        .when(!none, |this| {
            this.on_click(cx.listener(|page, _, _, cx| page.restore_selected(cx)))
        });
        let empty = self.items.is_empty();
        let empty_all = ui::button(
            "trash-empty",
            text(locale, Key::TrashEmptyAll),
            ui::Variant::Ghost,
            empty,
            t,
        )
        .when(!empty, |this| {
            this.on_click(cx.listener(|page, _, _, cx| page.confirm_purge(true, cx)))
        });
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .mt(px(24.))
            .when(!none, |this| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(tr(
                            locale,
                            Key::TrashSelected,
                            &[("n", Arg::Count(n as u64))],
                        )),
                )
            })
            .child(restore)
            .child(delete)
            .child(div().flex_1())
            .child(empty_all)
            .into_any_element()
    }
}

impl Render for TrashPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let now = acta_core::time::now_ms();
        let focused = self.focus.is_focused(window);
        let toolbar = self.toolbar(locale, &t, cx);
        let mut column = ui::column()
            .child(
                type_scale::TITLE
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, Key::TreeTrash)),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .mt(px(8.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::TrashHint)),
            )
            .child(toolbar);
        if self.loaded && self.items.is_empty() {
            // Empty states are never in the reading role (typography.md 3.2).
            column = column.child(
                type_scale::UI
                    .el()
                    .mt(px(24.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::TrashEmpty)),
            );
        }
        let items = self.items.clone();
        let mut index = 0;
        for (_, group) in by_day(&items) {
            let first = group[0].deleted_at;
            column = column.child(
                type_scale::SMALL
                    .el()
                    .mt(px(24.))
                    .mb(px(4.))
                    .text_color(t.text_tertiary)
                    .child(when::date(locale, first)),
            );
            for item in group {
                column = column.child(self.row(item, index, locale, now, focused, &t, cx));
                index += 1;
            }
        }
        ui::page_scroll("trash-page")
            .track_focus(&self.focus)
            .key_context("Trash")
            .on_key_down(cx.listener(Self::on_key))
            .child(column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{Author, TopicId};

    fn item(id: i64, deleted_at: i64) -> TrashItem {
        TrashItem {
            record: Record::Topic(TopicId(id)),
            project: ProjectId(1),
            label: format!("T{id}"),
            deleted_at,
            deleted_by: Author::Human,
            expires_at: deleted_at + acta_core::store::TRASH_RETENTION_MS,
        }
    }

    #[test]
    fn items_group_by_day_newest_first() {
        let day = acta_core::time::MS_PER_DAY;
        let base = 1_763_600_000_000;
        let items = vec![
            item(1, base),
            item(2, base + 3 * day),
            item(3, base + 60_000),
        ];
        let groups = by_day(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].1[0].record, Record::Topic(TopicId(2)));
        assert_eq!(groups[1].1.len(), 2);
    }

    #[test]
    fn days_left_round_up_and_stop_at_zero() {
        let day = acta_core::time::MS_PER_DAY;
        let i = item(1, 0);
        assert_eq!(days_left(&i, 0), 30);
        assert_eq!(days_left(&i, 2 * day + 1), 28);
        assert_eq!(days_left(&i, 40 * day), 0);
    }

    fn rows() -> Vec<Record> {
        (1..=5).map(|i| Record::Topic(TopicId(i))).collect()
    }

    fn t(i: i64) -> Record {
        Record::Topic(TopicId(i))
    }

    #[test]
    fn a_click_selects_one_and_mod_click_adds_or_removes() {
        let mut s = Selection::default();
        s.click(t(2));
        assert_eq!(s.selected, [t(2)]);
        s.toggle(t(4));
        assert_eq!(s.selected, [t(2), t(4)]);
        s.toggle(t(2));
        assert_eq!(s.selected, [t(4)]);
        s.click(t(1));
        assert_eq!(s.selected, [t(1)]);
    }

    #[test]
    fn shift_click_selects_the_range_from_the_last_clicked_row() {
        let rows = rows();
        let mut s = Selection::default();
        s.click(t(2));
        s.extend_to(&rows, t(4));
        assert_eq!(s.selected, [t(2), t(3), t(4)]);
        // The anchor stays: a second Shift+click re-ranges from it.
        s.extend_to(&rows, t(1));
        assert_eq!(s.selected, [t(1), t(2)]);
    }

    #[test]
    fn keys_move_the_cursor_toggle_extend_select_all_and_clear() {
        let rows = rows();
        let mut s = Selection::default();
        s.step(&rows, true, false);
        assert_eq!(s.cursor, Some(t(1)));
        assert!(s.selected.is_empty(), "moving selects nothing");
        s.step(&rows, true, false);
        s.toggle(s.cursor.unwrap());
        assert_eq!(s.selected, [t(2)]);
        s.step(&rows, true, true);
        s.step(&rows, true, true);
        assert_eq!(s.selected, [t(2), t(3), t(4)]);
        s.step(&rows, false, false);
        assert_eq!(s.cursor, Some(t(3)));
        // The cursor stops at the ends.
        for _ in 0..10 {
            s.step(&rows, true, false);
        }
        assert_eq!(s.cursor, Some(t(5)));
        s.all(&rows);
        assert_eq!(s.selected.len(), 5);
        s.clear();
        assert!(s.selected.is_empty());
    }

    #[test]
    fn a_reload_forgets_rows_no_longer_listed() {
        let mut s = Selection::default();
        s.click(t(3));
        s.toggle(t(4));
        s.retain(&[t(1), t(4)]);
        assert_eq!(s.selected, [t(4)]);
        assert_eq!(s.cursor, Some(t(4)));
        s.retain(&[t(1)]);
        assert!(s.selected.is_empty());
        assert_eq!(s.cursor, None);
    }

    #[test]
    fn the_confirmation_names_the_count() {
        assert_eq!(
            confirm_title(Locale::En, 3, false),
            "Delete 3 items permanently?"
        );
        assert_eq!(
            confirm_title(Locale::En, 1, false),
            "Delete 1 item permanently?"
        );
        assert_eq!(confirm_title(Locale::En, 9, true), "Empty the trash?");
        assert_eq!(
            confirm_title(Locale::Fr, 2, false),
            "Supprimer définitivement 2 éléments\u{202f}?"
        );
        assert_eq!(content_text(Locale::En, 31, 12), "31 topics, 12 notes");
    }
}
