//! The trash (docs/design/components-app.md 5, Trash page): what was
//! deleted, grouped by day, restorable for 30 days. No purge in v1.

use acta_core::model::{PageReq, ProjectId, Record};
use acta_core::store::TrashItem;
use gpui_kit::{
    AnyElement, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, Subscription, Task, Window, div, px,
};

use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::record_page::PageEvent;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;
use crate::when;
use crate::workspace::{Undo, Workspace};

pub struct TrashPage {
    ws: Entity<Workspace>,
    items: Vec<TrashItem>,
    loaded: bool,
    seen_revision: u64,
    _load: Task<()>,
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
    }
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
            loaded: false,
            seen_revision: 0,
            _load: Task::ready(()),
            _sub: sub,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = self
            .ws
            .read(cx)
            .db
            .run(cx, |store| store.trash(PageReq::first(PageReq::MAX_LIMIT)));
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok(items) = loaded {
                    page.items = items.items;
                }
                page.loaded = true;
                cx.notify();
            });
        });
    }

    fn restore(&mut self, record: Record, project: ProjectId, cx: &mut Context<Self>) {
        let locale = settings::locale(cx);
        let key = if matches!(record, Record::Topic(_)) {
            Key::TopicRestored
        } else {
            Key::TrashRestoredItem
        };
        let message = text(locale, key);
        self.items.retain(|item| item.record != record);
        cx.notify();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, Some(project), Some(message), move |store| {
                store.restore(record, acta_core::model::Author::Human)?;
                Ok((Undo::Delete(record), ()))
            });
        });
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

    fn row(
        &self,
        item: &TrashItem,
        index: usize,
        locale: Locale,
        now: i64,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let record = item.record;
        let project = item.project;
        let by = tr(
            locale,
            Key::TrashBy,
            &[("author", Arg::Text(&when::author(locale, item.deleted_by)))],
        );
        let left = tr(
            locale,
            Key::TrashDays,
            &[("n", Arg::Count(days_left(item, now)))],
        );
        let place = self.project_title(item.project, cx);
        ui::focusable(div().id(("trash-row", index)), t, false)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(8.))
            .mx(px(-8.))
            .rounded(px(RADIUS_SM))
            .hover({
                let hover = t.fill_hover;
                move |s| s.bg(hover)
            })
            .on_click(cx.listener(move |page, _, _, cx| page.restore(record, project, cx)))
            .child(icon(kind_icon(record), 16., t.text_secondary))
            .child(
                type_scale::UI
                    .el()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text_primary)
                    .child(item.label.lines().next().unwrap_or_default().to_owned()),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .text_color(t.text_tertiary)
                    .child(place),
            )
            .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(by))
            .child(
                type_scale::SMALL
                    .el()
                    .font_features(type_scale::tabular())
                    .text_color(t.text_tertiary)
                    .child(left),
            )
            .into_any_element()
    }
}

impl Render for TrashPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let now = acta_core::time::now_ms();
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
            );
        if self.loaded && self.items.is_empty() {
            column = column.child(
                type_scale::READING
                    .el()
                    .mt(px(24.))
                    .text_color(t.text_secondary)
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
                column = column.child(self.row(item, index, locale, now, &t, cx));
                index += 1;
            }
        }
        ui::page_scroll("trash-page").child(column)
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
}
