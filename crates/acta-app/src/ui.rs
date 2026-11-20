//! Small pieces of DESIGN.md section 5 shared by several views.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Div, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement, Styled, div, px,
};

use crate::icons::{Icon, icon};
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;

/// Keyboard shortcut hint: each key in its own 18-high box, `type.small`
/// with `tnum` in `text.tertiary`, 2 apart (DESIGN.md 5, kbd).
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

/// A ghost icon button (DESIGN.md 5, Button): `size` is 28, or 24 compact.
/// `selected` is the active state of a tool-window toggle. In an inactive
/// window its icon drops to `text.tertiary` (DESIGN.md 4.3).
pub fn ghost_icon_button(
    id: &'static str,
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
    let (hover, pressed, primary) = (tokens.fill_hover, tokens.fill_pressed, tokens.text_primary);
    let glyph = icon(glyph, 16., color).when(!disabled, |this| {
        this.group_hover(id, move |style| style.text_color(primary))
    });
    div()
        .id(id)
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
