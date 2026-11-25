//! The Contacts page (docs/design/components-contacts.md 5; screens.md
//! 8.28): the address book, grouped by level in tree order, with search,
//! "This project" / "All", inline creation, and on each row rename, move,
//! merge and delete. The actions shared with the contact page (moving,
//! merging, deleting, renaming) are here too.

use std::collections::HashSet;

use crate::zoom::px;
use acta_core::model::{Author, Contact, ContactId, ContactLevel, Record};
use acta_core::store::{ContactPatch, ContactSummary};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Task, Window, div,
};

use crate::contacts_model::{self as cm, PageFilter};
use crate::i18n::{Arg, Key, Locale, number, text, tr};
use crate::icons::{Icon, icon};
use crate::menu::{Align, AnchorBounds};
use crate::palette::{Choice, Lead, OnPick, Picker};
use crate::record_page::PageEvent;
use crate::route::Route;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{Undo, Workspace, contact_fields};

// ---- actions shared with the contact page ----

/// "Move to...": the palette lists the levels; a narrower one where the
/// contact is named outside reads "Used outside" and refuses with the
/// places (components-contacts.md 5, Moving a contact).
pub fn pick_level<P: EventEmitter<PageEvent>>(
    ws: &Entity<Workspace>,
    id: ContactId,
    cx: &mut Context<P>,
) {
    let task = ws.read(cx).db.run(cx, move |store| {
        Ok((store.contact(id)?, store.contact_uses(id)?))
    });
    let ws = ws.clone();
    cx.spawn(async move |page, cx| {
        let Ok((contact, uses)) = task.await else {
            return;
        };
        let _ = page.update(cx, |_, cx| {
            let locale = settings::locale(cx);
            let projects = ws.read(cx).projects.clone();
            let choices = cm::move_choices(locale, &contact, &uses, &projects);
            let rows = choices
                .iter()
                .map(|c| {
                    let name = cm::level_name(locale, c.level, &projects);
                    Choice {
                        title: c.title.clone(),
                        detail: String::new(),
                        lead: Lead::Icon(match c.level {
                            ContactLevel::All => Icon::Users,
                            ContactLevel::Project(_) => Icon::Folder,
                            ContactLevel::Epic(..) => Icon::Layers,
                        }),
                        trailing: if c.current {
                            text(locale, Key::ContactLevelCurrent)
                        } else if !c.outside.is_empty() {
                            text(locale, Key::ContactMoveBlocked)
                        } else {
                            String::new()
                        },
                        refusal: (!c.outside.is_empty())
                            .then(|| cm::refusal(locale, &name, &c.outside)),
                    }
                })
                .collect();
            let levels: Vec<ContactLevel> = choices.iter().map(|c| c.level).collect();
            let ws = ws.clone();
            let on_pick = OnPick::new(move |i, _, cx| {
                if let Some(level) = levels.get(i).copied() {
                    move_contact(&ws, id, level, cx);
                }
            });
            cx.emit(PageEvent::Pick(
                Picker {
                    label: text(locale, Key::CmdContactMove),
                    choices: rows,
                },
                on_pick,
            ));
        });
    })
    .detach();
}

/// Move a contact: one undo step, no toast (the page shows the level).
pub fn move_contact(ws: &Entity<Workspace>, id: ContactId, level: ContactLevel, cx: &mut App) {
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, None, None, move |store| {
            let back = store.contact(id)?.level;
            store.move_contact(id, level, Author::Human)?;
            Ok((
                Undo::ContactLevel {
                    id,
                    back,
                    expect: level,
                },
                (),
            ))
        });
    });
}

/// "Merge into...": the palette lists the other contacts visible to or
/// from this one; `Enter` merges this one into the chosen one.
pub fn pick_merge<P: EventEmitter<PageEvent>>(
    ws: &Entity<Workspace>,
    id: ContactId,
    cx: &mut Context<P>,
) {
    let task = ws.read(cx).db.run(cx, move |store| {
        Ok((store.contact(id)?, store.contacts_all()?))
    });
    let ws = ws.clone();
    cx.spawn(async move |page, cx| {
        let Ok((contact, all)) = task.await else {
            return;
        };
        let _ = page.update(cx, |_, cx| {
            let locale = settings::locale(cx);
            let projects = ws.read(cx).projects.clone();
            let others: Vec<Contact> = all
                .into_iter()
                .map(|c| c.contact)
                .filter(|c| c.id != id && cm::overlaps(c.level, contact.level))
                .collect();
            open_merge_picker(&ws, id, others, locale, &projects, cx);
        });
    })
    .detach();
}

/// The merge picker over `others`.
pub fn open_merge_picker<P: EventEmitter<PageEvent>>(
    ws: &Entity<Workspace>,
    id: ContactId,
    others: Vec<Contact>,
    locale: Locale,
    projects: &[acta_core::model::ProjectTree],
    cx: &mut Context<P>,
) {
    let rows = others
        .iter()
        .map(|c| Choice {
            title: c.name.clone(),
            detail: [cm::details(c), cm::level_name(locale, c.level, projects)]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" \u{b7} "),
            lead: Lead::Icon(Icon::User),
            trailing: String::new(),
            refusal: None,
        })
        .collect();
    let targets: Vec<(ContactId, String)> = others.iter().map(|c| (c.id, c.name.clone())).collect();
    let ws = ws.clone();
    let on_pick = OnPick::new(move |i, _, cx| {
        if let Some((into, name)) = targets.get(i).cloned() {
            merge(&ws, id, into, &name, cx);
        }
    });
    cx.emit(PageEvent::Pick(
        Picker {
            label: text(locale, Key::CmdContactMerge),
            choices: rows,
        },
        on_pick,
    ));
}

/// Merge `from` into `into`: one step, with "Merged into Marie Dubois."
/// and Undo, since references moved on pages not on screen.
pub fn merge(ws: &Entity<Workspace>, from: ContactId, into: ContactId, name: &str, cx: &mut App) {
    let locale = settings::locale(cx);
    let message = tr(locale, Key::ContactMerged, &[("name", Arg::Text(name))]);
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, None, Some(message), move |store| {
            let before = store.contact(into)?;
            let uses: Vec<Record> = store
                .contact_uses(from)?
                .into_iter()
                .map(|u| u.record)
                .collect();
            let trace = store.merge_contacts(from, into, Author::Human)?;
            Ok((
                Undo::Unmerge {
                    from,
                    before: Box::new(before),
                    after: Box::new(store.contact(into)?),
                    uses,
                    trace,
                },
                (),
            ))
        });
    });
}

/// Delete: to the trash with "Contact deleted." and Undo; references keep
/// naming it (components-contacts.md 5, Deleting).
pub fn delete(ws: &Entity<Workspace>, id: ContactId, cx: &mut App) {
    ws.update(cx, |ws, cx| ws.delete(Record::Contact(id), None, cx));
}

/// Rename or change a detail: one undo step.
pub fn patch(ws: &Entity<Workspace>, id: ContactId, patch: ContactPatch, cx: &mut App) {
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, None, None, move |store| {
            let back = contact_fields(store, id)?;
            store.update_contact(id, patch, Author::Human)?;
            Ok((
                Undo::ContactFields {
                    id,
                    back,
                    expect: contact_fields(store, id)?,
                },
                (),
            ))
        });
    });
}

/// "New contact" with the name only, at `level`: one undo step.
pub fn create(ws: &Entity<Workspace>, name: String, level: ContactLevel, cx: &mut App) {
    ws.update(cx, |ws, cx| {
        ws.undoable(cx, None, None, move |store| {
            let id = store.create_contact(&name, level, Author::Human)?;
            Ok((Undo::Delete(Record::Contact(id)), ()))
        });
    });
}

// ---- the page ----

/// The items of a row's menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Open,
    Rename,
    Move,
    Merge,
    Delete,
}

const ITEMS: [Item; 5] = [
    Item::Open,
    Item::Rename,
    Item::Move,
    Item::Merge,
    Item::Delete,
];

pub struct ContactsPage {
    ws: Entity<Workspace>,
    /// The active project: "This project" lists what it sees.
    active: Option<acta_core::model::ProjectId>,
    search: Entity<InputState>,
    filter: PageFilter,
    all: Vec<ContactSummary>,
    unread: HashSet<ContactId>,
    loaded: bool,
    /// The row being added (`New contact`), with its level, or renamed.
    new_row: Option<(ContactLevel, Entity<InputState>)>,
    rename: Option<(ContactId, Entity<InputState>)>,
    menu: Option<(ContactId, usize)>,
    menu_anchor: AnchorBounds,
    row_focus: Vec<FocusHandle>,
    row_ids: std::cell::RefCell<Vec<ContactId>>,
    /// Scroll and focus across launches (interaction.md 6.6).
    keep: Option<crate::page_keep::Keep>,
    focus: FocusHandle,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PageEvent> for ContactsPage {}

impl Focusable for ContactsPage {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }
}

impl ContactsPage {
    /// What Resume keeps of this page (interaction.md 6.6): its route
    /// key and the focused row.
    pub fn kept_state(
        &self,
        window: &Window,
        _: &gpui_kit::App,
    ) -> Option<(String, Option<String>)> {
        let keep = self.keep.as_ref()?;
        let focus = self
            .row_focus
            .iter()
            .position(|h| h.is_focused(window))
            .map(crate::resume::row_key);
        Some((keep.route().to_owned(), focus))
    }

    /// Once the content is there: the kept scroll, and the kept row when
    /// the page has the keyboard, on the next frame (after layout).
    fn restore_kept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !(self.loaded) {
            return;
        }
        let Some(kept) = self.keep.as_mut().and_then(crate::page_keep::Keep::take) else {
            return;
        };
        cx.on_next_frame(window, move |page, window, cx| {
            if let Some(keep) = &page.keep {
                keep.apply_scroll(&kept);
            }
            let row = kept.focus.as_deref().and_then(crate::resume::row_of);
            if let Some(i) = row
                && Focusable::focus_handle(page, cx).contains_focused(window, cx)
                && let Some(h) = page.row_focus.get(i).cloned()
            {
                window.focus(&h, cx);
            }
        });
    }

    pub fn new(
        ws: Entity<Workspace>,
        active: Option<acta_core::model::ProjectId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let locale = settings::locale(cx);
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(text(locale, Key::ContactPlaceholder))
        });
        // The search is focused when the page opens.
        search.update(cx, |s, cx| s.focus(window, cx));
        let subs = vec![
            cx.observe(&ws, |page, ws, cx| {
                let revision = ws.read(cx).revision;
                if revision != page.seen_revision {
                    page.seen_revision = revision;
                    page.reload(cx);
                }
            }),
            cx.subscribe_in(&search, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        let mut page = Self {
            ws,
            active,
            search,
            filter: PageFilter::ThisProject,
            all: Vec::new(),
            unread: HashSet::new(),
            loaded: false,
            new_row: None,
            rename: None,
            menu: None,
            menu_anchor: AnchorBounds::default(),
            row_focus: Vec::new(),
            row_ids: Default::default(),
            keep: Some(crate::page_keep::Keep::new("contacts".into(), cx)),
            focus: cx.focus_handle(),
            seen_revision: 0,
            _load: Task::ready(()),
            _subs: subs,
        };
        page.reload(cx);
        page
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let task = self.ws.read(cx).db.run(cx, |store| {
            Ok((store.contacts_all()?, store.unread_contacts()?))
        });
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let _ = page.update(cx, |page, cx| {
                if let Ok((all, unread)) = loaded {
                    page.all = all;
                    // A contact shown here loses its dot next time.
                    let fresh: Vec<ContactId> = unread
                        .iter()
                        .copied()
                        .filter(|c| !page.unread.contains(c))
                        .collect();
                    page.unread.extend(unread);
                    if !fresh.is_empty() {
                        let task = page.ws.read(cx).db.run(cx, move |store| {
                            for c in fresh {
                                store.mark_seen(Record::Contact(c))?;
                            }
                            Ok(())
                        });
                        task.detach();
                    }
                }
                page.loaded = true;
                cx.notify();
            });
        });
    }

    fn active_project(&self, cx: &App) -> Option<acta_core::model::ProjectId> {
        self.active
            .or_else(|| self.ws.read(cx).projects.first().map(|t| t.project.id))
    }

    /// "New contact": an editing row at the top of the target group.
    fn start_new(&mut self, level: ContactLevel, window: &mut Window, cx: &mut Context<Self>) {
        let locale = settings::locale(cx);
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(text(locale, Key::ContactNamePlaceholder))
        });
        input.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.subscribe_in(
            &input,
            window,
            move |page, input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let name = input.read(cx).value().trim().to_owned();
                    if !name.is_empty() {
                        let ws = page.ws.clone();
                        create(&ws, name.chars().take(cm::NAME_MAX).collect(), level, cx);
                    }
                    page.new_row = None;
                    window.focus(&page.focus, cx);
                    cx.notify();
                }
            },
        );
        self._subs.push(sub);
        self.new_row = Some((level, input));
        cx.notify();
    }

    fn start_rename(&mut self, id: ContactId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.all.iter().find(|c| c.contact.id == id) else {
            return;
        };
        let name = c.contact.name.clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe_in(
            &input,
            window,
            move |page, input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let name = input.read(cx).value().trim().to_owned();
                    if !name.is_empty() {
                        let ws = page.ws.clone();
                        patch(
                            &ws,
                            id,
                            ContactPatch {
                                name: Some(name),
                                ..ContactPatch::default()
                            },
                            cx,
                        );
                    }
                    page.rename = None;
                    window.focus(&page.focus, cx);
                    cx.notify();
                }
            },
        );
        self._subs.push(sub);
        self.rename = Some((id, input));
        cx.notify();
    }

    fn choose(&mut self, id: ContactId, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        let ws = self.ws.clone();
        match item {
            Item::Open => cx.emit(PageEvent::Open(Route::Contact(id))),
            Item::Rename => self.start_rename(id, window, cx),
            Item::Move => pick_level(&ws, id, cx),
            Item::Merge => pick_merge(&ws, id, cx),
            Item::Delete => delete(&ws, id, cx),
        }
        cx.notify();
    }

    fn on_row_key(
        &mut self,
        id: ContactId,
        index: usize,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &event.keystroke;
        if let Some((mid, hl)) = self.menu {
            match k.key.as_str() {
                "up" => self.menu = Some((mid, hl.saturating_sub(1))),
                "down" => self.menu = Some((mid, (hl + 1).min(ITEMS.len() - 1))),
                "enter" | "space" => self.choose(mid, ITEMS[hl], window, cx),
                "escape" => self.menu = None,
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let ws = self.ws.clone();
        match k.key.as_str() {
            "enter" if !k.modifiers.modified() => cx.emit(PageEvent::Open(Route::Contact(id))),
            "f2" => self.start_rename(id, window, cx),
            "m" if !k.modifiers.modified() => pick_level(&ws, id, cx),
            "delete" if !k.modifiers.modified() => delete(&ws, id, cx),
            "f10" if k.modifiers.shift => self.menu = Some((id, 0)),
            "menu" => self.menu = Some((id, 0)),
            "up" | "down" if !k.modifiers.modified() => {
                let to = if k.key == "up" {
                    index.checked_sub(1)
                } else {
                    Some(index + 1)
                };
                match to.and_then(|i| self.row_focus.get(i)) {
                    Some(h) => window.focus(h, cx),
                    None if k.key == "up" => self.search.update(cx, |s, cx| s.focus(window, cx)),
                    None => {}
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn render_row(
        &self,
        c: &ContactSummary,
        index: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = c.contact.id;
        if let Some((rid, input)) = &self.rename
            && *rid == id
        {
            return div()
                .h(px(32.))
                .flex()
                .items_center()
                .child(div().flex_1().child(Input::new(input)))
                .into_any_element();
        }
        let details = cm::details(&c.contact);
        let hover = t.fill_hover;
        let mut row = ui::focusable(div().id(("contact-row", index)), t, false)
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(8.))
            .mx(px(-8.))
            .rounded(px(RADIUS_SM))
            .hover(move |s| s.bg(hover))
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Contact(id)))))
            .on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(move |page, _, _, cx| {
                    page.menu = Some((id, 0));
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, window, cx| {
                page.on_row_key(id, index, event, window, cx)
            }))
            .child(
                type_scale::UI
                    .el()
                    .flex_shrink_0()
                    .max_w(gpui_kit::relative(0.6))
                    .truncate()
                    .text_color(t.text_primary)
                    .child(c.contact.name.clone()),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text_tertiary)
                    .child(details),
            )
            .when(c.open > 0, |this| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .flex_shrink_0()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(tr(
                            locale,
                            Key::ContactOpenCount,
                            &[("n", Arg::Count(c.open as u64))],
                        )),
                )
            })
            .when(self.unread.contains(&id), |this| {
                this.child(ui::unread_dot(t))
            })
            // Only the row whose menu is open measures (one anchor).
            .when(self.menu.is_some_and(|(m, _)| m == id), |this| {
                this.child(self.menu_anchor.measure_keyed(id.0 as u64))
            });
        if let Some(h) = self.row_focus.get(index) {
            row = row.track_focus(h);
        }
        let menu = self.menu.filter(|(m, _)| *m == id).map(|(_, hl)| {
            let mut panel = crate::menu::panel("contact-menu", t).on_mouse_down_out(cx.listener(
                |page, _, _, cx| {
                    page.menu = None;
                    cx.notify();
                },
            ));
            for (i, item) in ITEMS.iter().enumerate() {
                let (label, hint) = match item {
                    Item::Open => (text(locale, Key::CommonOpen), None),
                    Item::Rename => (text(locale, Key::ContactRename), Some("F2")),
                    Item::Move => (text(locale, Key::ContactMove), Some("M")),
                    Item::Merge => (text(locale, Key::ContactMerge), None),
                    Item::Delete => (text(locale, Key::CommonDelete), Some("Delete")),
                };
                if *item == Item::Delete {
                    panel = panel.child(crate::menu::separator(t));
                }
                let item = *item;
                panel = panel.child(
                    crate::menu::item(
                        ("contact-item", i),
                        None,
                        label,
                        hint.map(|h| crate::menu::hint(h, t)),
                        i == hl,
                        t,
                    )
                    .on_click(
                        cx.listener(move |page, _, window, cx| page.choose(id, item, window, cx)),
                    ),
                );
            }
            self.menu_anchor
                .below_keyed(id.0 as u64, panel, Align::End, 2)
                .unwrap_or_else(|| div().into_any_element())
        });
        div().child(row).children(menu).into_any_element()
    }
}

impl Render for ContactsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let projects = self.ws.read(cx).projects.clone();
        let active = self.active_project(cx);
        let query = self.search.read(cx).value().to_string();
        let groups = cm::page_groups(&self.all, &projects, self.filter, active, &query);
        let shown: usize = groups.iter().map(|g| g.contacts.len()).sum();
        while self.row_focus.len() < shown {
            self.row_focus.push(cx.focus_handle());
        }
        *self.row_ids.borrow_mut() = groups
            .iter()
            .flat_map(|g| g.contacts.iter().map(|c| c.contact.id))
            .collect();
        let default_level = match self.filter {
            PageFilter::ThisProject => active
                .map(ContactLevel::Project)
                .unwrap_or(ContactLevel::All),
            PageFilter::All => ContactLevel::All,
        };
        let mut seg = ui::segmented(&t);
        for (f, key) in [
            (PageFilter::ThisProject, Key::ContactsFilterProject),
            (PageFilter::All, Key::ContactsFilterAll),
        ] {
            seg = seg.child(
                ui::segment(
                    ("contacts-seg", f as usize),
                    text(locale, key),
                    self.filter == f,
                    &t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.filter = f;
                    cx.notify();
                })),
            );
        }
        let search = div()
            .flex_1()
            .min_w(px(160.))
            .h(px(28.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(RADIUS_SM))
            .bg(t.bg_raised)
            .border(px(1.))
            .border_color(t.border_default)
            .child(
                Input::new(&self.search)
                    .appearance(false)
                    .px(px(0.))
                    .text_size(px(type_scale::UI.size())),
            );
        let mut column = ui::column()
            .child(
                type_scale::TITLE
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, Key::ContactsTitle)),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .mt(px(8.))
                    .text_color(t.text_tertiary)
                    .font_features(type_scale::tabular())
                    .child(tr(
                        locale,
                        Key::ContactsCount,
                        &[("n", Arg::Count(self.all.len() as u64))],
                    )),
            )
            .child(
                div()
                    .mt(px(24.))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(search)
                    .child(seg)
                    .child(
                        ui::button(
                            "contacts-new",
                            text(locale, Key::ContactsNew),
                            Variant::Secondary,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(
                            move |page, _, window, cx| page.start_new(default_level, window, cx),
                        )),
                    ),
            );
        if self.loaded && self.all.is_empty() && self.new_row.is_none() {
            column = column.child(
                type_scale::UI
                    .el()
                    .mt(px(16.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::ContactsEmpty)),
            );
        } else if self.loaded && groups.is_empty() && self.new_row.is_none() {
            let empty = if !query.trim().is_empty() {
                tr(
                    locale,
                    Key::ContactsNoMatch,
                    &[("query", Arg::Text(query.trim()))],
                )
            } else {
                let project = active
                    .map(|p| cm::project_title(&projects, p))
                    .unwrap_or_default();
                tr(
                    locale,
                    Key::ContactsNoneHere,
                    &[("project", Arg::Text(&project))],
                )
            };
            let show_all = query.trim().is_empty() && self.filter == PageFilter::ThisProject;
            column = column.child(
                div()
                    .mt(px(16.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(type_scale::UI.el().text_color(t.text_tertiary).child(empty))
                    .when(show_all, |this| {
                        this.child(
                            ui::button(
                                "contacts-show-all",
                                text(locale, Key::ContactsFilterAll),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|page, _, _, cx| {
                                page.filter = PageFilter::All;
                                cx.notify();
                            })),
                        )
                    }),
            );
        }
        // A new row whose group has no contact yet still shows its header.
        let mut levels: Vec<ContactLevel> = groups.iter().map(|g| g.level).collect();
        if let Some((level, _)) = &self.new_row
            && !levels.contains(level)
        {
            levels.insert(0, *level);
        }
        let mut index = 0;
        for level in levels {
            let contacts: Vec<ContactSummary> = groups
                .iter()
                .find(|g| g.level == level)
                .map(|g| g.contacts.clone())
                .unwrap_or_default();
            let link = match level {
                ContactLevel::All => None,
                ContactLevel::Project(p) => Some(Route::Project(p)),
                ContactLevel::Epic(_, e) => Some(Route::Epic(e)),
            };
            let group_id: gpui_kit::SharedString = format!("contacts-group-{level}").into();
            let hover = t.fill_hover;
            column = column.child(
                div()
                    .id(gpui_kit::ElementId::Name(group_id.clone()))
                    .group(group_id.clone())
                    .mt(px(16.))
                    .h(px(28.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id(gpui_kit::ElementId::Name(format!("{group_id}-name").into()))
                            .px(px(4.))
                            .mx(px(-4.))
                            .rounded(px(RADIUS_SM))
                            .when(link.is_some(), |this| {
                                this.cursor_pointer().hover(move |s| s.bg(hover))
                            })
                            .when_some(link, |this, route| {
                                this.on_click(
                                    cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(route))),
                                )
                            })
                            .child(
                                type_scale::LABEL
                                    .el()
                                    .text_color(t.text_secondary)
                                    .child(cm::level_name(locale, level, &projects)),
                            ),
                    )
                    .child(
                        div()
                            .invisible()
                            .group_hover(group_id.clone(), |s| s.visible())
                            .child(
                                ui::ghost_icon_button(
                                    format!("{group_id}-plus"),
                                    Icon::Plus,
                                    24.,
                                    false,
                                    false,
                                    true,
                                    &t,
                                )
                                .tooltip(ui::tip(text(locale, Key::ContactsNewHere)))
                                .on_click(cx.listener(
                                    move |page, _, window, cx| page.start_new(level, window, cx),
                                )),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        type_scale::SMALL
                            .el()
                            .text_color(t.text_tertiary)
                            .font_features(type_scale::tabular())
                            .child(number(locale, contacts.len() as u64)),
                    ),
            );
            if let Some((l, input)) = &self.new_row
                && *l == level
            {
                column = column.child(
                    div()
                        .h(px(32.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(icon(Icon::User, 16., t.text_secondary))
                        .child(div().flex_1().child(Input::new(input))),
                );
            }
            for c in &contacts {
                column = column.child(self.render_row(c, index, locale, &t, cx));
                index += 1;
            }
        }
        self.restore_kept(window, cx);
        ui::page_scroll("contacts-page")
            .when_some(
                self.keep.as_ref().map(|k| k.scroll().clone()),
                |el, scroll| {
                    el.track_scroll(&scroll)
                        .on_scroll(cx.listener(|page, _, _, cx| {
                            if let Some(keep) = page.keep.as_mut() {
                                keep.scrolled(cx);
                            }
                        }))
                },
            )
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, window, cx| {
                // Down from the search goes to the first row; Esc cancels a
                // new or renamed row.
                let k = &event.keystroke;
                if k.key == "escape" && (page.new_row.is_some() || page.rename.is_some()) {
                    page.new_row = None;
                    page.rename = None;
                    window.focus(&page.focus, cx);
                    cx.stop_propagation();
                    cx.notify();
                } else if k.key == "down"
                    && page.search.read(cx).focus_handle(cx).is_focused(window)
                    && let Some(h) = page.row_focus.first()
                {
                    window.focus(h, cx);
                    cx.stop_propagation();
                }
            }))
            .child(column)
    }
}
