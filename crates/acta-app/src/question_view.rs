//! The question row (docs/design/components-questions.md 5, Question row),
//! drawn the same on a record page's Questions section, the Questions page
//! and the contact page: the state glyph, the text that wraps, the answer,
//! the meta line, then the author mark and the unread dot. Each host adds
//! its own listeners, keys and hover buttons.

use acta_core::date::{Date, due_state};
use acta_core::model::{ContactId, Question, QuestionStatus};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, InteractiveElement, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::due::{self, Tone};
use crate::i18n::{Arg, Key, Locale, tr};
use crate::icons::{Icon, icon};
use crate::markdown_view;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;
use crate::when;

/// How a row is drawn on its host.
pub struct RowLook {
    pub index: usize,
    pub unread: bool,
    /// "in Topic Embed the terminal": the owner, on lists that mix owners.
    pub owner: Option<String>,
    /// The contact page names no contact: it is the page's.
    pub show_asked: bool,
    pub today: Date,
}

/// The leading glyph: Open `circle-question-mark` secondary, Answered
/// `circle-check` and Dropped `circle-slash` tertiary. Shape carries state.
pub fn glyph(status: QuestionStatus, t: &Tokens) -> gpui_kit::Svg {
    match status {
        QuestionStatus::Open => icon(Icon::CircleQuestionMark, 14., t.text_secondary),
        QuestionStatus::Answered => icon(Icon::CircleCheck, 14., t.text_tertiary),
        QuestionStatus::Dropped => icon(Icon::CircleSlash, 14., t.text_tertiary),
    }
}

/// A sentence of the catalogue cut where `{name}` goes, so the name can be
/// its own link in any language.
pub fn around_name(locale: Locale, key: Key, name: &str) -> (String, String) {
    const MARK: &str = "\u{1}";
    let line = tr(locale, key, &[("name", Arg::Text(MARK))]);
    match line.split_once(MARK) {
        Some((a, b)) => (a.to_owned(), b.to_owned()),
        None => (line.replace(MARK, name), String::new()),
    }
}

/// A contact's name in a meta line: a link to the contact page, plain with
/// the tooltip "In the trash" when the contact is in the trash.
pub fn name_link(
    id: impl Into<gpui_kit::ElementId>,
    name: &str,
    in_trash: bool,
    locale: Locale,
    t: &Tokens,
) -> Stateful<Div> {
    let el = div().id(id.into()).child(name.to_owned());
    if in_trash {
        el.tooltip(ui::tip(crate::i18n::text(locale, Key::ContactInTrash)))
    } else {
        el.text_color(t.accent)
            .cursor_pointer()
            .hover(|s| s.underline())
    }
}

/// The row: leading glyph, text, answer, meta; `actions` (the hover
/// buttons) and the author mark at the right. `on_contact` opens the
/// contact page from the Asked link; `on_logged` the decision entry.
#[allow(clippy::too_many_arguments)]
pub fn row(
    q: &Question,
    look: &RowLook,
    actions: Option<AnyElement>,
    on_contact: impl Fn(ContactId, &mut Window, &mut App) + 'static,
    on_logged: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    locale: Locale,
    t: &Tokens,
) -> Stateful<Div> {
    let open = q.is_open();
    let i = look.index;
    let text = type_scale::UI
        .el()
        .text_color(if open {
            t.text_primary
        } else {
            t.text_secondary
        })
        .child(q.text.clone());
    let answer = q
        .answer
        .as_ref()
        .filter(|_| q.status == QuestionStatus::Answered)
        .map(|a| {
            div()
                .mt(px(4.))
                .max_h(px(26. * 6.))
                .overflow_hidden()
                .child(markdown_view::short(("q-answer", i), a.clone(), t))
        });
    let mut meta: Vec<AnyElement> = Vec::new();
    let dot = || div().flex_shrink_0().child(" \u{b7} ").into_any_element();
    if let Some(owner) = &look.owner {
        meta.push(div().child(owner.clone()).into_any_element());
    }
    if look.show_asked
        && let Some(c) = &q.asked_to
    {
        if !meta.is_empty() {
            meta.push(dot());
        }
        let (before, after) = around_name(locale, Key::QuestionAsked, &c.name);
        let id = c.id;
        let mut link = name_link(("q-asked", i), &c.name, c.in_trash, locale, t);
        if !c.in_trash {
            link = link.on_click(move |_, window, cx| on_contact(id, window, cx));
        }
        meta.push(
            div()
                .flex()
                .flex_row()
                .child(before)
                .child(link)
                .child(after)
                .into_any_element(),
        );
    }
    match q.status {
        QuestionStatus::Open => {
            if let Some(day) = q.expected_on {
                if !meta.is_empty() {
                    meta.push(dot());
                }
                let date = due::date(locale, day);
                let state = due_state(Some(day), look.today, true);
                let phrase = due::phrase(locale, state).map(|(words, tone)| {
                    div()
                        .text_color(match tone {
                            Tone::Warning => t.warning,
                            Tone::Secondary => t.text_secondary,
                            Tone::Tertiary => t.text_tertiary,
                        })
                        .child(words)
                });
                meta.push(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .child(tr(locale, Key::QuestionBy, &[("date", Arg::Text(&date))]))
                        .children(phrase)
                        .into_any_element(),
                );
            }
        }
        status => {
            if !meta.is_empty() {
                meta.push(dot());
            }
            let at = q.resolved_at.unwrap_or(q.stamp.updated_at);
            let date = when::date(locale, at);
            let author = when::author(locale, q.resolved_by.unwrap_or(q.stamp.updated_by));
            let key = if status == QuestionStatus::Answered {
                Key::QuestionAnsweredBy
            } else {
                Key::QuestionDroppedBy
            };
            meta.push(
                div()
                    .child(tr(
                        locale,
                        key,
                        &[("date", Arg::Text(&date)), ("author", Arg::Text(&author))],
                    ))
                    .into_any_element(),
            );
            if q.decision_entry.is_some() && status == QuestionStatus::Answered {
                meta.push(dot());
                meta.push(
                    div()
                        .id(("q-logged", i))
                        .text_color(t.accent)
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .on_click(on_logged)
                        .child(crate::i18n::text(locale, Key::QuestionLogged))
                        .into_any_element(),
                );
            }
        }
    }
    let meta = (!meta.is_empty()).then(|| {
        type_scale::SMALL
            .el()
            .mt(px(if answer.is_some() { 4. } else { 2. }))
            .flex()
            .flex_row()
            .flex_wrap()
            .text_color(t.text_tertiary)
            .children(meta)
    });
    let human = when::initial(&when::human_name(locale));
    let group: gpui_kit::SharedString = format!("q-row-{i}").into();
    let hover = t.fill_hover;
    ui::focusable(div().id(("q-row", i)), t, false)
        .group(group.clone())
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.))
        .min_h(px(32.))
        .px(px(8.))
        .py(px(6.))
        .mx(px(-8.))
        .rounded(px(RADIUS_SM))
        .hover(move |s| s.bg(hover))
        .child(
            div()
                .w(px(16.))
                .h(px(20.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(glyph(q.status, t)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(text)
                .children(answer)
                .children(meta),
        )
        .child(
            div()
                .flex_shrink_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .children(actions.map(|a| {
                    div()
                        .invisible()
                        .group_hover(group.clone(), |s| s.visible())
                        .child(a)
                }))
                .when(look.unread, |this| this.child(ui::unread_dot(t)))
                .child(ui::author_mark(q.created_by, &human, t)),
        )
}
