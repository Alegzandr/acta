//! The due date picker (docs/design/components-records.md 5, Due date
//! picker): a floating layer under its anchor with the input, the preview
//! line, the month and its grid, and Clear when the record has a date. The
//! rules live in [`crate::due::Picker`]; this is the view and its keys.
//!
//! The host (a record page or the tree) opens it, writes what it is told
//! to on [`DuePickerEvent::Done`], and returns focus to the anchor.

use acta_core::date::Date;
use gpui_kit::component::input::{
    Escape, IndentInline, Input, InputEvent, InputState, MoveDown, OutdentInline,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, div, px,
};

use crate::due::{self, Area, GridKey, Outcome, Picker, Preview, Tone};
use crate::i18n::{Key, Locale, number, text};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{RADIUS_LG, RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};

gpui_kit::actions!(
    acta_due,
    [
        GridLeft,
        GridRight,
        GridUp,
        GridDown,
        GridHome,
        GridEnd,
        GridPageUp,
        GridPageDown,
        GridYearUp,
        GridYearDown,
        GridToday,
        GridChoose,
        GridClear,
        /// `Esc` anywhere in the picker: close, change nothing.
        PickerClose,
        PickerTab,
        PickerTabBack,
    ]
);

/// The picker's keys. The grid's live in its own context, so a letter
/// typed in the input is never a grid key; contexts deeper than the tree's
/// and the page's win over their bindings of the same keys.
pub fn bindings() -> Vec<KeyBinding> {
    let grid = Some("DuePickerGrid");
    let picker = Some("DuePicker");
    vec![
        KeyBinding::new("left", GridLeft, grid),
        KeyBinding::new("right", GridRight, grid),
        KeyBinding::new("up", GridUp, grid),
        KeyBinding::new("down", GridDown, grid),
        KeyBinding::new("home", GridHome, grid),
        KeyBinding::new("end", GridEnd, grid),
        KeyBinding::new("pageup", GridPageUp, grid),
        KeyBinding::new("pagedown", GridPageDown, grid),
        KeyBinding::new("shift-pageup", GridYearUp, grid),
        KeyBinding::new("shift-pagedown", GridYearDown, grid),
        KeyBinding::new("t", GridToday, grid),
        KeyBinding::new("enter", GridChoose, grid),
        KeyBinding::new("space", GridChoose, grid),
        KeyBinding::new("delete", GridClear, grid),
        KeyBinding::new("backspace", GridClear, grid),
        KeyBinding::new("escape", PickerClose, picker),
        KeyBinding::new("tab", PickerTab, picker),
        KeyBinding::new("shift-tab", PickerTabBack, picker),
    ]
}

/// What the picker asks of its host: write a date (or clear it), or just
/// close. Either way the host closes it and puts focus back on the anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuePickerEvent {
    Done(Outcome),
}

pub struct DuePicker {
    pub model: Picker,
    input: Entity<InputState>,
    grid_focus: FocusHandle,
    clear_focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<DuePickerEvent> for DuePicker {}

impl DuePicker {
    pub fn new(
        today: Date,
        current: Option<Date>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let locale = settings::locale(cx);
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(text(locale, Key::DuePickerPlaceholder))
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        let subs = vec![cx.subscribe_in(
            &input,
            window,
            |picker: &mut Self, input, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    let value = input.read(cx).value().to_string();
                    picker.model.input(&value);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    let outcome = picker.model.enter();
                    picker.finish(outcome, cx);
                }
                _ => {}
            },
        )];
        Self {
            model: Picker::new(today, current),
            input,
            grid_focus: cx.focus_handle(),
            clear_focus: cx.focus_handle(),
            _subs: subs,
        }
    }

    #[cfg(debug_assertions)]
    /// Debug builds' captures: `text` as if typed, then `Enter` if asked.
    pub fn dev_type(
        &mut self,
        text: &str,
        enter: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = text.to_owned();
        self.input
            .update(cx, |state, cx| state.set_value(text.clone(), window, cx));
        self.model.input(&text);
        if enter {
            let outcome = self.model.enter();
            self.finish(outcome, cx);
        }
        cx.notify();
    }

    fn finish(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        match outcome {
            Outcome::Stay => cx.notify(),
            done => cx.emit(DuePickerEvent::Done(done)),
        }
    }

    /// Keyboard focus follows the model's area.
    fn focus_area(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.model.area {
            Area::Input => self.input.update(cx, |s, cx| s.focus(window, cx)),
            Area::Grid => window.focus(&self.grid_focus, cx),
            Area::Clear => window.focus(&self.clear_focus, cx),
        }
        cx.notify();
    }

    fn tab(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.model.tab(back);
        self.focus_area(window, cx);
    }

    fn grid(&mut self, key: GridKey, window: &mut Window, cx: &mut Context<Self>) {
        let outcome = self.model.grid_key(key);
        if outcome == Outcome::Stay {
            // `Up` from the first row goes back to the input.
            self.focus_area(window, cx);
        }
        self.finish(outcome, cx);
    }

    fn render_preview(&self, locale: Locale, t: &Tokens) -> AnyElement {
        // A fixed two lines of 16, so the month under it never moves while
        // typing (QA 2026-11-22): a date and its phrase on one line, the
        // date giving way first; a sentence about the input on up to two.
        let line = type_scale::SMALL
            .el()
            .h(px(PREVIEW_HEIGHT))
            .mt(px(4.))
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.));
        let tone = |tone: Tone| match tone {
            Tone::Warning => t.warning,
            Tone::Secondary => t.text_secondary,
            Tone::Tertiary => t.text_tertiary,
        };
        let day = |line: gpui_kit::Div, day: Date| {
            let state = acta_core::date::due_state(Some(day), self.model.today, true);
            line.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text_primary)
                    .child(due::full(locale, day)),
            )
            .children(due::phrase(locale, state).map(|(words, tn)| {
                div()
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .text_color(tone(tn))
                    .child(words)
            }))
        };
        let problem = |line: gpui_kit::Div, key: Key| {
            if self.model.error {
                line.text_color(t.danger)
                    .gap(px(4.))
                    .child(div().h(px(16.)).flex().items_center().child(icon(
                        Icon::TriangleAlert,
                        14.,
                        t.danger,
                    )))
                    .child(div().flex_1().min_w_0().child(text(locale, key)))
            } else {
                line.text_color(t.text_tertiary)
                    .child(div().flex_1().min_w_0().child(text(locale, key)))
            }
        };
        match self.model.preview {
            Preview::Day(d) | Preview::Empty(Some(d)) => day(line, d),
            Preview::Empty(None) => line
                .text_color(t.text_tertiary)
                .child(text(locale, Key::DuePickerNone)),
            Preview::Invalid => problem(line, Key::DuePickerInvalid),
            Preview::Range => problem(line, Key::DuePickerRange),
        }
        .into_any_element()
    }

    fn render_grid(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.model.selected();
        let keyboard = self.model.area == Area::Grid;
        let mut rows = div().flex().flex_col().mt(px(4.));
        let days = self.model.grid();
        for week in days.chunks(7) {
            let mut row = div().flex().flex_row();
            for &d in week {
                let is_selected = selected == Some(d);
                let is_today = d == self.model.today;
                let is_cursor = keyboard && d == self.model.cursor;
                let role = if is_selected {
                    type_scale::UI_STRONG
                } else {
                    type_scale::UI
                };
                let color = if self.model.outside(d) && !is_selected {
                    t.text_tertiary
                } else {
                    t.text_primary
                };
                let hover = t.fill_hover;
                row = row.child(
                    role.el()
                        .id(("due-day", d.days() as usize))
                        .w(px(32.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(RADIUS_SM))
                        .font_features(type_scale::tabular())
                        .text_color(color)
                        .when(is_selected, |this| this.bg(t.fill_selected))
                        .when(!is_selected, |this| this.hover(move |s| s.bg(hover)))
                        .when(is_today, |this| {
                            this.border_1().border_color(t.border_strong)
                        })
                        .when(is_cursor, |this| this.shadow(ui::ring_shadow(t, false)))
                        .on_click(cx.listener(move |picker, _, _, cx| {
                            let outcome = picker.model.pick(d);
                            picker.finish(outcome, cx);
                        }))
                        .child(number(locale, u64::from(d.ymd().2))),
                );
            }
            rows = rows.child(row);
        }
        div()
            .id("due-grid")
            .key_context("DuePickerGrid")
            .track_focus(&self.grid_focus)
            .on_action(cx.listener(|p, _: &GridLeft, w, cx| p.grid(GridKey::Left, w, cx)))
            .on_action(cx.listener(|p, _: &GridRight, w, cx| p.grid(GridKey::Right, w, cx)))
            .on_action(cx.listener(|p, _: &GridUp, w, cx| p.grid(GridKey::Up, w, cx)))
            .on_action(cx.listener(|p, _: &GridDown, w, cx| p.grid(GridKey::Down, w, cx)))
            .on_action(cx.listener(|p, _: &GridHome, w, cx| p.grid(GridKey::Home, w, cx)))
            .on_action(cx.listener(|p, _: &GridEnd, w, cx| p.grid(GridKey::End, w, cx)))
            .on_action(cx.listener(|p, _: &GridPageUp, w, cx| p.grid(GridKey::PageUp, w, cx)))
            .on_action(cx.listener(|p, _: &GridPageDown, w, cx| p.grid(GridKey::PageDown, w, cx)))
            .on_action(cx.listener(|p, _: &GridYearUp, w, cx| p.grid(GridKey::YearUp, w, cx)))
            .on_action(cx.listener(|p, _: &GridYearDown, w, cx| p.grid(GridKey::YearDown, w, cx)))
            .on_action(cx.listener(|p, _: &GridToday, w, cx| p.grid(GridKey::Today, w, cx)))
            .on_action(cx.listener(|p, _: &GridChoose, w, cx| p.grid(GridKey::Enter, w, cx)))
            .on_action(cx.listener(|p, _: &GridClear, w, cx| p.grid(GridKey::Clear, w, cx)))
            .child(rows)
            .into_any_element()
    }
}

impl Render for DuePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let input_focused = self.model.area == Area::Input;
        let input_border = if self.model.error {
            t.danger
        } else {
            t.border_strong
        };
        let input = div()
            .h(px(28.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(RADIUS_SM))
            .bg(t.bg_raised)
            .border_1()
            .border_color(input_border)
            .when(input_focused && !self.model.error, |this| {
                this.shadow(ui::ring_shadow(&t, false))
            })
            .child(
                div().flex_1().child(
                    Input::new(&self.input)
                        .appearance(false)
                        .px(px(0.))
                        .text_size(px(type_scale::UI.size)),
                ),
            );
        let header = div()
            .mt(px(8.))
            .h(px(28.))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                type_scale::LABEL
                    .el()
                    .text_color(t.text_primary)
                    .child(due::month_header(locale, self.model.month)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .child(
                        ui::ghost_icon_button(
                            "due-prev",
                            Icon::ChevronLeft,
                            24.,
                            false,
                            false,
                            true,
                            &t,
                        )
                        .tooltip(ui::tip_keys(
                            text(locale, Key::DuePickerPrev),
                            vec!["PageUp"],
                        ))
                        .on_click(cx.listener(|p, _, _, cx| {
                            p.model.turn(-1);
                            cx.notify();
                        })),
                    )
                    .child(
                        ui::ghost_icon_button(
                            "due-next",
                            Icon::ChevronRight,
                            24.,
                            false,
                            false,
                            true,
                            &t,
                        )
                        .tooltip(ui::tip_keys(
                            text(locale, Key::DuePickerNext),
                            vec!["PageDown"],
                        ))
                        .on_click(cx.listener(|p, _, _, cx| {
                            p.model.turn(1);
                            cx.notify();
                        })),
                    ),
            );
        let weekdays = type_scale::SMALL
            .el()
            .mt(px(4.))
            .h(px(16.))
            .flex()
            .flex_row()
            .text_color(t.text_tertiary)
            .children(
                crate::i18n::time::weekdays_short(locale)
                    .into_iter()
                    .map(|w| div().w(px(32.)).flex().justify_center().child(w)),
            );
        let footer = self.model.has_clear().then(|| {
            div().mt(px(8.)).h(px(28.)).flex().flex_row().child(
                ui::button(
                    "due-clear",
                    text(locale, Key::CommonClear),
                    Variant::Ghost,
                    false,
                    &t,
                )
                .ml(px(-12.))
                .track_focus(&self.clear_focus)
                .on_click(cx.listener(|p, _, _, cx| {
                    let outcome = p.model.clear();
                    p.finish(outcome, cx);
                })),
            )
        });
        div()
            .id("due-picker")
            .key_context("DuePicker")
            .occlude()
            .w(px(248.))
            .p(px(12.))
            .flex()
            .flex_col()
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_LG))
            .shadow(vec![t.shadow_overlay()])
            .on_mouse_down_out(cx.listener(|p, _, _, cx| p.finish(Outcome::Close, cx)))
            .capture_action(cx.listener(|p, _: &MoveDown, window, cx| {
                p.model.down();
                p.focus_area(window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|p, _: &Escape, _, cx| {
                p.finish(Outcome::Close, cx);
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
            .on_action(cx.listener(|p, _: &PickerClose, _, cx| p.finish(Outcome::Close, cx)))
            .on_action(cx.listener(|p, _: &PickerTab, w, cx| p.tab(false, w, cx)))
            .on_action(cx.listener(|p, _: &PickerTabBack, w, cx| p.tab(true, w, cx)))
            .child(input)
            .child(self.render_preview(locale, &t))
            .child(header)
            .child(weekdays)
            .child(self.render_grid(locale, &t, cx))
            .children(footer)
    }
}

pub use crate::menu::AnchorBounds;

/// The preview's height: two lines of `type.small`, whatever it shows.
const PREVIEW_HEIGHT: f32 = 32.;
