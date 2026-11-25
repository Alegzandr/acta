//! The contact picker (docs/design/components-contacts.md 5; screens.md
//! 8.29): the one control that chooses a person. A search input over the
//! contacts a record sees, grouped nearest level first, with "New contact"
//! as its last row. Its value is a contact, a new contact's name (created
//! by the host when it saves), or nothing; never free text: leaving it
//! with text that was not taken puts the previous value back.
//!
//! The rules (groups, ranking, matched characters) are in
//! [`crate::contacts_model`]; this is the view and its keys. Inline (the
//! question composer, quick capture) the list floats under the input; in
//! a popover (Report to) it is part of the popover.

use crate::zoom::px;
use acta_core::model::{ContactId, Place};
use acta_core::store::ContactSummary;
use gpui_kit::component::input::{
    Escape, IndentInline, Input, InputEvent, InputState, MoveDown, MoveUp, OutdentInline,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription, Task, Window,
    div,
};

use crate::contacts_model::{self, NAME_MAX, Pick, PickerList};
use crate::i18n::{Arg, Key, text, tr};
use crate::icons::{Icon, icon};
use crate::menu::{Align, AnchorBounds};
use crate::settings;
use crate::theme::{RADIUS_LG, RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;
use crate::workspace::Workspace;

/// What the field holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Value {
    #[default]
    None,
    Contact(ContactId, String),
    /// "New contact" taken: created when the host saves.
    New(String),
}

impl Value {
    pub fn name(&self) -> &str {
        match self {
            Value::None => "",
            Value::Contact(_, n) | Value::New(n) => n,
        }
    }
}

/// What the picker tells its host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerEvent {
    /// A row was taken, or the value cleared. `tab`: `Some(false)` Tab,
    /// `Some(true)` Shift+Tab: move to the next or previous field.
    Taken(Value, Option<bool>),
    /// Tab or Shift+Tab with nothing to take: just move.
    Tab(bool),
    /// `Esc` with the list closed: the host cancels its editor.
    Cancel,
    /// `Mod+Enter`: the host saves.
    Submit,
}

/// Where the list goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A floating layer under the input (composer, capture).
    Inline,
    /// Part of a popover, under the input.
    Popover,
}

pub struct ContactPicker {
    ws: Entity<Workspace>,
    input: Entity<InputState>,
    mode: Mode,
    /// The contacts the record sees, nearest level first.
    visible: Vec<ContactSummary>,
    list: PickerList,
    highlight: usize,
    open: bool,
    value: Value,
    /// "in Project Acta": where a new contact goes.
    new_in: String,
    /// A row was just taken by the pointer: the blur that follows keeps it.
    taking: bool,
    /// A store refusal shown under the field (components-contacts.md 5).
    error: Option<String>,
    anchor: AnchorBounds,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<PickerEvent> for ContactPicker {}

impl Focusable for ContactPicker {
    fn focus_handle(&self, cx: &gpui_kit::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl ContactPicker {
    pub fn new(
        ws: Entity<Workspace>,
        place: Place,
        value: Value,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let locale = settings::locale(cx);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(text(locale, Key::ContactPlaceholder))
                .default_value(value.name().to_owned())
        });
        let subs = vec![cx.subscribe_in(
            &input,
            window,
            |picker: &mut Self, input, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    let typed = input.read(cx).value().to_string();
                    if typed.chars().count() > NAME_MAX {
                        let cut: String = typed.chars().take(NAME_MAX).collect();
                        input.update(cx, |s, cx| s.set_value(cut, window, cx));
                        return;
                    }
                    if typed == picker.value.name() && !picker.open {
                        return;
                    }
                    picker.error = None;
                    picker.refresh(&typed);
                    picker.open = !typed.trim().is_empty();
                    cx.notify();
                }
                InputEvent::Focus => {
                    // Focusing selects the name, so typing replaces it.
                    input.update(cx, |s, cx| s.select_all(window, cx));
                }
                InputEvent::PressEnter { secondary, .. } => {
                    if *secondary {
                        if picker.open {
                            picker.take(None, window, cx);
                        }
                        cx.emit(PickerEvent::Submit);
                    } else {
                        picker.enter(window, cx);
                    }
                }
                InputEvent::Blur => {
                    if std::mem::take(&mut picker.taking) {
                        return;
                    }
                    picker.restore(window, cx);
                }
            },
        )];
        let mut picker = Self {
            ws: ws.clone(),
            input,
            mode,
            visible: Vec::new(),
            list: PickerList::default(),
            highlight: 0,
            open: false,
            value,
            new_in: String::new(),
            taking: false,
            error: None,
            anchor: AnchorBounds::default(),
            _load: Task::ready(()),
            _subs: subs,
        };
        picker.new_in = tr(
            locale,
            Key::ContactNewIn,
            &[(
                "level",
                Arg::Text(&contacts_model::level_name(
                    locale,
                    contacts_model::default_level(place.project),
                    &ws.read(cx).projects,
                )),
            )],
        );
        let task = ws
            .read(cx)
            .db
            .run(cx, move |store| store.contacts_visible(place));
        picker._load = cx.spawn(async move |picker, cx| {
            if let Ok(visible) = task.await {
                let _ = picker.update(cx, |picker, cx| {
                    picker.visible = visible;
                    let typed = picker.typed(cx);
                    if picker.open {
                        picker.refresh(&typed);
                    }
                    cx.notify();
                });
            }
        });
        picker
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |s, cx| s.focus(window, cx));
    }

    /// Set the value (the host cleared or restored it).
    pub fn set_value(&mut self, value: Value, window: &mut Window, cx: &mut Context<Self>) {
        self.value = value;
        self.open = false;
        let name = self.value.name().to_owned();
        self.input.update(cx, |s, cx| s.set_value(name, window, cx));
        cx.notify();
    }

    /// A store refusal (the contact `name` moved out of the record's sight
    /// meanwhile): the field is restored to no contact and the inline
    /// error says so under it, until the next keystroke
    /// (components-contacts.md 5, Errors). The list loads again, so the
    /// contact no longer shows.
    pub fn refuse(
        &mut self,
        name: &str,
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = settings::locale(cx);
        self.set_value(Value::None, window, cx);
        self.error = Some(refusal(locale, name));
        let task = self
            .ws
            .read(cx)
            .db
            .run(cx, move |store| store.contacts_visible(place));
        self._load = cx.spawn(async move |picker, cx| {
            if let Ok(visible) = task.await {
                let _ = picker.update(cx, |picker, cx| {
                    picker.visible = visible;
                    cx.notify();
                });
            }
        });
        cx.notify();
    }

    #[cfg(debug_assertions)]
    /// Debug builds' captures: `text` as if typed, the list open.
    pub fn dev_type(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.to_owned();
        self.input
            .update(cx, |s, cx| s.set_value(text.clone(), window, cx));
        self.refresh(&text);
        self.open = true;
        cx.notify();
    }

    fn typed(&self, cx: &Context<Self>) -> String {
        self.input.read(cx).value().to_string()
    }

    fn refresh(&mut self, typed: &str) {
        self.list = contacts_model::picker_list(&self.visible, typed);
        // The first row is highlighted as soon as there is text, so Enter
        // takes what is highlighted.
        self.highlight = 0;
    }

    /// `Down`: open the list (with no text, the usual contacts), or move.
    fn down(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            let typed = self.typed(cx);
            let shown = if typed == self.value.name() {
                ""
            } else {
                &typed
            };
            self.refresh(shown);
            self.open = !self.list.is_empty();
        } else {
            let len = self.list.flat().len();
            if len > 0 {
                self.highlight = (self.highlight + 1) % len;
            }
        }
        cx.notify();
    }

    /// `Up`: from the first row to "New contact", else one up.
    fn up(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        let len = self.list.flat().len();
        if len > 0 {
            self.highlight = if self.highlight == 0 {
                len - 1
            } else {
                self.highlight - 1
            };
        }
        cx.notify();
    }

    /// Take the highlighted row (or `at`), set it and close the list.
    fn take(&mut self, at: Option<usize>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let rows = self.list.flat();
        let Some(pick) = rows.get(at.unwrap_or(self.highlight)).cloned() else {
            return false;
        };
        self.value = match pick {
            Pick::Contact(id, name) => Value::Contact(id, name),
            Pick::New(name) => Value::New(name),
        };
        self.open = false;
        let name = self.value.name().to_owned();
        self.input.update(cx, |s, cx| s.set_value(name, window, cx));
        cx.notify();
        true
    }

    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.typed(cx);
        if self.open && self.take(None, window, cx) {
            cx.emit(PickerEvent::Taken(self.value.clone(), None));
            return;
        }
        if typed.trim().is_empty() {
            // An emptied field clears the value.
            self.value = Value::None;
            self.open = false;
            cx.emit(PickerEvent::Taken(Value::None, None));
        } else {
            cx.emit(PickerEvent::Taken(self.value.clone(), None));
        }
        cx.notify();
    }

    fn tab(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.typed(cx);
        if !back && self.open && !typed.trim().is_empty() && self.take(None, window, cx) {
            cx.emit(PickerEvent::Taken(self.value.clone(), Some(false)));
            return;
        }
        // Shift+Tab with unconfirmed text restores the previous value.
        self.restore(window, cx);
        cx.emit(PickerEvent::Tab(back));
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open || self.typed(cx) != self.value.name() {
            self.restore(window, cx);
        } else {
            cx.emit(PickerEvent::Cancel);
        }
    }

    /// Close the list and put the value's name back in the input.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        let name = self.value.name().to_owned();
        if self.typed(cx) != name {
            self.input.update(cx, |s, cx| s.set_value(name, window, cx));
        }
        cx.notify();
    }

    fn render_list(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let locale = settings::locale(cx);
        let projects = self.ws.read(cx).projects.clone();
        let mut rows = div().flex().flex_col();
        let mut index = 0;
        let headers = self.list.headers();
        for group in &self.list.groups {
            if headers {
                rows = rows.child(
                    type_scale::SMALL
                        .el()
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .text_color(t.text_tertiary)
                        .child(contacts_model::level_name(locale, group.level, &projects)),
                );
            }
            for row in &group.rows {
                let at = index;
                let lit = at == self.highlight;
                let strong = gpui_kit::HighlightStyle {
                    font_weight: Some(gpui_kit::FontWeight::SEMIBOLD),
                    ..Default::default()
                };
                let highlights: Vec<(std::ops::Range<usize>, gpui_kit::HighlightStyle)> = row
                    .name
                    .char_indices()
                    .enumerate()
                    .filter(|(i, _)| row.matched.contains(i))
                    .map(|(_, (b, c))| (b..b + c.len_utf8(), strong))
                    .collect();
                let hover = t.fill_hover;
                let group: gpui_kit::SharedString = format!("contact-row-{at}").into();
                let secondary = t.text_secondary;
                rows = rows.child(
                    div()
                        .id(("contact-row", at))
                        .group(group.clone())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .h(px(32.))
                        .px(px(8.))
                        .rounded(px(RADIUS_SM))
                        .when(lit, |this| this.bg(t.fill_selected))
                        .when(!lit, |this| this.hover(move |s| s.bg(hover)))
                        .on_mouse_down(
                            gpui_kit::MouseButton::Left,
                            cx.listener(move |picker, _, window, cx| {
                                picker.taking = true;
                                if picker.take(Some(at), window, cx) {
                                    cx.emit(PickerEvent::Taken(picker.value.clone(), None));
                                }
                                cx.stop_propagation();
                            }),
                        )
                        .child(
                            type_scale::UI
                                .el()
                                .flex_shrink_0()
                                .max_w_full()
                                .truncate()
                                .text_color(t.text_primary)
                                .child(
                                    gpui_kit::StyledText::new(row.name.clone())
                                        .with_highlights(highlights),
                                ),
                        )
                        .child(
                            type_scale::SMALL
                                .el()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(if lit {
                                    t.text_secondary
                                } else {
                                    t.text_tertiary
                                })
                                .group_hover(group, move |s| s.text_color(secondary))
                                .child(row.details.clone()),
                        ),
                );
                index += 1;
            }
        }
        if let Some(name) = &self.list.new_row {
            let at = index;
            let lit = at == self.highlight;
            let hover = t.fill_hover;
            let level = if lit {
                t.text_secondary
            } else {
                t.text_tertiary
            };
            rows = rows.child(
                div()
                    .id("contact-new")
                    .when(!self.list.groups.is_empty(), |this| this.mt(px(4.)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(32.))
                    .px(px(8.))
                    .rounded(px(RADIUS_SM))
                    .when(lit, |this| this.bg(t.fill_selected))
                    .when(!lit, |this| this.hover(move |s| s.bg(hover)))
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(move |picker, _, window, cx| {
                            picker.taking = true;
                            if picker.take(Some(at), window, cx) {
                                cx.emit(PickerEvent::Taken(picker.value.clone(), None));
                            }
                            cx.stop_propagation();
                        }),
                    )
                    .child(icon(Icon::Plus, 14., t.text_secondary))
                    .child(
                        // The typed name stays whole; "in Project Acta"
                        // gives way first.
                        type_scale::UI
                            .el()
                            .flex_shrink_0()
                            .max_w(gpui_kit::relative(0.7))
                            .truncate()
                            .text_color(t.text_primary)
                            .child(tr(
                                locale,
                                Key::ContactNewNamed,
                                &[("name", Arg::Text(name))],
                            )),
                    )
                    .child(
                        type_scale::SMALL
                            .el()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_right()
                            .text_color(level)
                            .child(self.new_in.clone()),
                    ),
            );
        }
        let list = div()
            .id("contact-list")
            .occlude()
            .flex()
            .flex_col()
            .p(px(4.));
        match self.mode {
            Mode::Inline => list
                .min_w(px(240.))
                .w(px(self.anchor_width().max(300.)))
                .bg(t.bg_overlay)
                .border(px(1.))
                .border_color(t.border_default)
                .rounded(px(RADIUS_LG))
                .shadow(vec![t.shadow_overlay()])
                .child(rows)
                .into_any_element(),
            Mode::Popover => list.mt(px(4.)).mx(px(-4.)).child(rows).into_any_element(),
        }
    }

    fn anchor_width(&self) -> f32 {
        self.anchor.width().unwrap_or(240.)
    }
}

impl Render for ContactPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let new_tag = matches!(self.value, Value::New(_)) && !self.open;
        let field = div()
            .relative()
            .h(px(28.))
            .px(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .rounded(px(RADIUS_SM))
            .bg(t.bg_raised)
            .border(px(1.))
            .border_color(if self.error.is_some() {
                t.danger
            } else {
                t.border_default
            })
            .when(focused && self.error.is_none(), |this| {
                this.shadow(ui::ring_shadow(&t, false))
            })
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.input)
                        .appearance(false)
                        .px(px(0.))
                        .text_size(px(type_scale::UI.size())),
                ),
            )
            .when(new_tag, |this| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .flex_shrink_0()
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::ContactNewTag)),
                )
            })
            .child(self.anchor.measure());
        let show = self.open && !self.list.is_empty();
        let list = show.then(|| self.render_list(&t, cx));
        let error = self.error.clone().map(|message| {
            type_scale::SMALL
                .el()
                .mt(px(4.))
                .flex()
                .flex_row()
                .gap(px(4.))
                .text_color(t.danger)
                .child(icon(Icon::TriangleAlert, 14., t.danger))
                .child(div().flex_1().min_w_0().child(message))
        });
        let mut root = div()
            .id("contact-picker")
            .key_context("ContactPicker")
            .flex()
            .flex_col()
            .w_full()
            .capture_action(cx.listener(|p, _: &MoveDown, _, cx| {
                p.down(cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|p, _: &MoveUp, _, cx| {
                p.up(cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|p, _: &IndentInline, window, cx| {
                p.tab(false, window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|p, _: &OutdentInline, window, cx| {
                p.tab(true, window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|p, _: &Escape, window, cx| {
                p.escape(window, cx);
                cx.stop_propagation();
            }))
            .child(field)
            .children(error);
        match self.mode {
            Mode::Inline => {
                if let Some(list) = list {
                    root = root.children(self.anchor.below_at(list, Align::Start, 3));
                }
            }
            Mode::Popover => root = root.children(list),
        }
        root
    }
}

/// The inline error of a store refusal: "Marie is no longer visible here.
/// Choose again." (`contact.out_of_scope`).
pub fn refusal(locale: crate::i18n::Locale, name: &str) -> String {
    tr(locale, Key::ContactOutOfScope, &[("name", Arg::Text(name))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;

    #[test]
    fn a_refusal_names_the_contact_in_both_languages() {
        assert_eq!(
            refusal(Locale::En, "Marie"),
            "Marie is no longer visible here. Choose again."
        );
        assert!(refusal(Locale::Fr, "Marie").contains("Marie"));
    }
}
