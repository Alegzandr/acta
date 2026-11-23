//! The instance strip of the Terminal, Chat and Agent browser views
//! (docs/design/layout.md 4.5.1): from two instances on a page, one compact
//! tab each under the area header, the active one selected, a 16 trailing
//! slot on every tab for the `x` shown on hover and on the active one.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Div, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful,
    Styled, div, px,
};

use crate::icons::Icon;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;

/// The strip: 28 high, `bg.sunken`, bottom border `border.subtle`,
/// padding 0 x 8, gap 4.
pub fn strip(tabs: Vec<AnyElement>, t: &Tokens) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .gap(px(4.))
        .h(px(28.))
        .px(px(8.))
        .bg(t.bg_sunken)
        .border_b_1()
        .border_color(t.border_subtle)
        .children(tabs)
        .into_any_element()
}

/// One tab: 24 high, padding 0 x 8, `radius.sm`, `type.small`; active
/// `fill.selected` `text.primary`, others `text.secondary`, hover
/// `fill.hover`. `lead` goes before the label (a session's `bot`).
pub fn tab(
    id: impl Into<SharedString>,
    lead: Option<AnyElement>,
    label: String,
    active: bool,
    t: &Tokens,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let hover = t.fill_hover;
    div()
        .id(id.clone())
        .group(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .h(px(24.))
        .pl(px(8.))
        .pr(px(4.))
        .rounded(px(RADIUS_SM))
        .when(active, |this| this.bg(t.fill_selected))
        .when(!active, |this| this.hover(move |s| s.bg(hover)))
        .children(lead)
        .child(
            type_scale::SMALL
                .el()
                .text_color(if active {
                    t.text_primary
                } else {
                    t.text_secondary
                })
                .child(label),
        )
}

/// The tab's trailing `x` (16): shown on hover of its tab (`group`) and on
/// the active one; the slot is kept either way, so nothing moves.
pub fn close(
    id: impl Into<SharedString>,
    group: impl Into<SharedString>,
    active: bool,
    t: &Tokens,
) -> Stateful<Div> {
    let group: SharedString = group.into();
    ui::ghost_icon_button(id, Icon::X, 16., false, false, true, t)
        .when(!active, |this| {
            this.invisible().group_hover(group, |s| s.visible())
        })
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
}
