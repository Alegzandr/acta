//! Small pieces of docs/design/components.md 5 shared by several views.

use acta_core::model::{Author, Status};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Div, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Svg, div, px,
};

use crate::i18n::Key;
use crate::icons::{Icon, icon};
use crate::theme::{RADIUS_MD, RADIUS_SM, Tokens};
use crate::type_scale;

/// Keyboard shortcut hint: each key in its own 18-high box, `type.small`
/// with `tnum` in `text.tertiary`, 2 apart (docs/design/components.md 5, kbd).
pub fn kbd(keys: &[&str], tokens: &Tokens) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .gap(px(2.))
        .children(keys.iter().map(|key| {
            type_scale::SMALL
                .el()
                .font_features(type_scale::tabular())
                .h(px(18.))
                .min_w(px(18.))
                .px(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(RADIUS_SM))
                .border_1()
                .border_color(tokens.border_default)
                .text_color(tokens.text_tertiary)
                .child(key.to_string())
        }))
}

/// A ghost icon button (docs/design/components.md 5, Button): `size` is 28, or 24 compact.
/// `selected` is the active state of a tool-window toggle. In an inactive
/// window its icon drops to `text.tertiary` (docs/design/layout.md 4.3).
pub fn ghost_icon_button(
    id: impl Into<SharedString>,
    glyph: Icon,
    size: f32,
    selected: bool,
    disabled: bool,
    window_active: bool,
    tokens: &Tokens,
) -> Stateful<Div> {
    let color = if disabled {
        tokens.text_disabled
    } else if !window_active {
        tokens.text_tertiary
    } else if selected {
        tokens.text_primary
    } else {
        tokens.text_secondary
    };
    let id: SharedString = id.into();
    let (hover, pressed, primary) = (tokens.fill_hover, tokens.fill_pressed, tokens.text_primary);
    let glyph = icon(glyph, 16., color).when(!disabled, |this| {
        this.group_hover(id.clone(), move |style| style.text_color(primary))
    });
    div()
        .id(id.clone())
        .group(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(RADIUS_SM))
        .when(selected, |this| this.bg(tokens.fill_selected))
        .when(!disabled, |this| {
            this.hover(move |style| style.bg(hover))
                .active(move |style| style.bg(pressed))
        })
        .child(glyph)
}

/// A ghost control as tall as its bar, padding 0 x 8 (status bar items).
/// `id` is also the hover group, for an icon that follows the label.
pub fn bar_item(id: &'static str, tokens: &Tokens) -> Stateful<Div> {
    let (hover, text) = (tokens.fill_hover, tokens.text_secondary);
    div()
        .id(id)
        .group(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h_full()
        .px(px(8.))
        .hover(move |style| style.bg(hover).text_color(text))
}

/// The glyph of a topic status (docs/design/color.md 2.3): a distinct
/// interior per status, so it never relies on colour.
pub fn status_icon(status: Status) -> Icon {
    match status {
        Status::Draft => Icon::CircleDashed,
        Status::Planned => Icon::Circle,
        Status::Active => Icon::CircleDot,
        Status::Blocked => Icon::CirclePause,
        Status::Done => Icon::CircleCheck,
        Status::Dropped => Icon::CircleSlash,
    }
}

pub fn status_color(status: Status, tokens: &Tokens) -> Hsla {
    match status {
        Status::Draft | Status::Dropped => tokens.text_tertiary,
        Status::Planned => tokens.text_secondary,
        Status::Active => tokens.accent,
        Status::Blocked => tokens.warning,
        Status::Done => tokens.success,
    }
}

pub fn status_key(status: Status) -> Key {
    match status {
        Status::Draft => Key::StatusDraft,
        Status::Planned => Key::StatusPlanned,
        Status::Active => Key::StatusActive,
        Status::Blocked => Key::StatusBlocked,
        Status::Done => Key::StatusDone,
        Status::Dropped => Key::StatusDropped,
    }
}

pub fn status_glyph(status: Status, size: f32, tokens: &Tokens) -> Svg {
    icon(status_icon(status), size, status_color(status, tokens))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
}

/// A text button (docs/design/components.md 5, Button): 28 high, padding
/// 0 x 12, `radius.sm`. Never changes size between states.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Variant,
    disabled: bool,
    tokens: &Tokens,
) -> Stateful<Div> {
    let role = if variant == Variant::Primary {
        type_scale::UI_STRONG
    } else {
        type_scale::UI
    };
    let t = tokens;
    let base = role
        .el()
        .id(id.into())
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(12.))
        .rounded(px(RADIUS_SM))
        .child(label.into());
    match (variant, disabled) {
        (Variant::Primary, false) => {
            let (hover, pressed) = (t.accent_hover, t.accent_pressed);
            base.bg(t.accent)
                .text_color(t.text_on_accent)
                .hover(move |s| s.bg(hover))
                .active(move |s| s.bg(pressed))
        }
        (Variant::Primary, true) => base.bg(t.fill_selected).text_color(t.text_disabled),
        (Variant::Secondary, false) => {
            let (hover, pressed) = (t.fill_hover, t.fill_pressed);
            base.border_1()
                .border_color(t.border_strong)
                .text_color(t.text_primary)
                .hover(move |s| s.bg(hover))
                .active(move |s| s.bg(pressed))
        }
        (Variant::Secondary, true) => base
            .border_1()
            .border_color(t.border_default)
            .text_color(t.text_disabled),
        (Variant::Ghost, false) => {
            let (hover, pressed, primary) = (t.fill_hover, t.fill_pressed, t.text_primary);
            base.text_color(t.text_secondary)
                .hover(move |s| s.bg(hover).text_color(primary))
                .active(move |s| s.bg(pressed))
        }
        (Variant::Ghost, true) => base.text_color(t.text_disabled),
    }
}

/// The author mark of a timeline entry or knowledge row (docs/design/
/// components-records.md 5): a circle with the initial for the human, a
/// square with a prompt chevron for Claude Code. Shape, not colour.
pub fn author_mark(author: Author, human_initial: &str, tokens: &Tokens) -> Div {
    let mark = div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(20.));
    match author {
        Author::Human => mark
            .rounded_full()
            .bg(tokens.fill_selected)
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(tokens.text_primary)
            .child(human_initial.to_owned()),
        Author::Claude => mark
            .rounded(px(RADIUS_SM))
            .border_1()
            .border_color(tokens.border_strong)
            .child(icon(Icon::ChevronRight, 12., tokens.text_secondary)),
    }
}

/// A Notice (docs/design/components.md 5): the one inline message.
pub fn notice(
    glyph: Option<(Icon, Hsla)>,
    message: impl Into<SharedString>,
    tokens: &Tokens,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .w_full()
        .px(px(12.))
        .py(px(8.))
        .rounded(px(RADIUS_MD))
        .bg(tokens.bg_raised)
        .border_1()
        .border_color(tokens.border_subtle)
        .children(glyph.map(|(glyph, color)| icon(glyph, 16., color)))
        .child(
            type_scale::UI
                .el()
                .flex_1()
                .min_w_0()
                .text_color(tokens.text_primary)
                .child(message.into()),
        )
}

/// A section label (`type.label`, `text.secondary`): "Current state",
/// "Knowledge", "Timeline".
pub fn label(text: impl Into<SharedString>, tokens: &Tokens) -> Div {
    type_scale::LABEL
        .el()
        .text_color(tokens.text_secondary)
        .child(text.into())
}

/// One segment of a segmented control (docs/design/components.md 5).
pub fn segment(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    tokens: &Tokens,
) -> Stateful<Div> {
    let role = if selected {
        type_scale::UI_STRONG
    } else {
        type_scale::UI
    };
    let hover = tokens.fill_hover;
    role.el()
        .id(id.into())
        .flex()
        .items_center()
        .h_full()
        .px(px(12.))
        .text_color(if selected {
            tokens.text_primary
        } else {
            tokens.text_secondary
        })
        .when(selected, |this| this.bg(tokens.fill_selected))
        .when(!selected, |this| this.hover(move |style| style.bg(hover)))
        .child(label.into())
}

/// The outline of a segmented control, holding its segments.
pub fn segmented(tokens: &Tokens) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .h(px(28.))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(tokens.border_strong)
        .overflow_hidden()
}

/// docs/design/typography.md 3.3: the content column is at most 680 wide.
pub const COLUMN_MAX: f32 = 680.;

/// The content column: at most 680 wide, 32 above the title.
pub fn column() -> Div {
    div()
        .w_full()
        .max_w(px(COLUMN_MAX))
        .pt(px(32.))
        .pb(px(48.))
        .flex()
        .flex_col()
}

/// The scrolling page area that centres the column, with 32 of side
/// padding (docs/design/layout.md 4.2).
pub fn page_scroll(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id.into())
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .px(px(32.))
}

/// A row of the properties block (docs/design/components-records.md 5):
/// 28 high, the label in a column as wide as `label_width`, gap 16.
pub fn property(
    label: impl Into<SharedString>,
    label_width: f32,
    value: impl IntoElement,
    tokens: &Tokens,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(16.))
        .min_h(px(28.))
        .child(
            type_scale::UI
                .el()
                .flex_shrink_0()
                .w(px(label_width))
                .h(px(28.))
                .flex()
                .items_center()
                .text_color(tokens.text_tertiary)
                .truncate()
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .min_h(px(28.))
                .flex()
                .items_center()
                .child(value),
        )
}

/// The label column of a properties block: as wide as its longest label in
/// the current language (Inter at 13 averages about 0.53 em per character
/// in labels), capped at 40% of the column.
pub fn label_width(labels: &[&str]) -> f32 {
    let longest = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
    (longest * 13. * 0.56).ceil().min(COLUMN_MAX * 0.4)
}

/// A ghost value in the properties block: padding 0 x 8, shifted left by 8
/// so its text aligns with plain values.
pub fn ghost_value(id: impl Into<ElementId>, tokens: &Tokens) -> Stateful<Div> {
    let hover = tokens.fill_hover;
    type_scale::UI
        .el()
        .id(id.into())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(8.))
        .ml(px(-8.))
        .rounded(px(RADIUS_SM))
        .text_color(tokens.text_primary)
        .hover(move |s| s.bg(hover))
}
