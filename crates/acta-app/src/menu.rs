//! Context and dropdown menus (docs/design/components.md 5): `bg.overlay`,
//! 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4, items of
//! 32. Hover and keyboard highlight are the same `fill.selected` row.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Div, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString,
    Stateful, Styled, div, px,
};

use crate::icons::{Icon, icon};
use crate::theme::{RADIUS_LG, RADIUS_SM, Tokens};
use crate::type_scale;

/// The floating panel. The caller anchors it (`deferred(anchored()...)`)
/// and closes it on `on_mouse_down_out`.
pub fn panel(id: impl Into<ElementId>, tokens: &Tokens) -> Stateful<Div> {
    div()
        .id(id.into())
        .occlude()
        .flex()
        .flex_col()
        .min_w(px(200.))
        .max_w(px(320.))
        .p(px(4.))
        .bg(tokens.bg_overlay)
        .border_1()
        .border_color(tokens.border_default)
        .rounded(px(RADIUS_LG))
        .shadow(vec![tokens.shadow_overlay()])
}

/// One item: optional leading glyph, the label, a trailing hint or check.
pub fn item(
    id: impl Into<ElementId>,
    leading: Option<AnyElement>,
    label: impl Into<SharedString>,
    trailing: Option<AnyElement>,
    highlighted: bool,
    tokens: &Tokens,
) -> Stateful<Div> {
    let selected = tokens.fill_selected;
    type_scale::UI
        .el()
        .id(id.into())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(32.))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .text_color(tokens.text_primary)
        .when(highlighted, |this| this.bg(selected))
        .hover(move |style| style.bg(selected))
        .children(leading)
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
        .children(trailing)
}

/// The 1px `border.default` line before a destructive item, 4 above and
/// below.
pub fn separator(tokens: &Tokens) -> Div {
    div().my(px(4.)).h(px(1.)).bg(tokens.border_default)
}

/// A check mark for the current value.
pub fn check(tokens: &Tokens) -> AnyElement {
    icon(Icon::Check, 16., tokens.text_secondary).into_any_element()
}

/// A number hint ("1" to "6" in the status menu), `type.small` tertiary.
pub fn hint(text: impl Into<SharedString>, tokens: &Tokens) -> AnyElement {
    type_scale::SMALL
        .el()
        .text_color(tokens.text_tertiary)
        .child(text.into())
        .into_any_element()
}
