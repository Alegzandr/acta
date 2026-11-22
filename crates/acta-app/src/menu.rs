//! Context and dropdown menus (docs/design/components.md 5): `bg.overlay`,
//! 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4, items of
//! 32. Hover and keyboard highlight are the same `fill.selected` row.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, AnyElement, Bounds, Div, ElementId, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, SharedString, Stateful, Styled, anchored, deferred, div, point, px,
};

use crate::icons::{Icon, icon};
use crate::theme::{RADIUS_LG, RADIUS_SM, Tokens};
use crate::type_scale;

/// The floating panel. The caller hangs it from its trigger
/// ([`AnchorBounds::below`]) and closes it on `on_mouse_down_out`.
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

/// A disabled item: `text.disabled`, no hover, no action.
pub fn disabled_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    trailing: Option<AnyElement>,
    tokens: &Tokens,
) -> Stateful<Div> {
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
        .text_color(tokens.text_disabled)
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

/// Which edge of its trigger a floating layer lines up with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// Left edges aligned: dropdowns, value menus, pickers.
    Start,
    /// Right edges aligned: the `ellipsis` and view menus at a right edge.
    End,
}

/// The gap between a trigger and the layer it opens (components.md 5,
/// menus: 4 below the anchor).
pub const GAP: f32 = 4.;

/// The trigger's corner the layer hangs from, the layer's corner put on
/// it, and the offset between them, for `align`. GPUI's `attach` moves the
/// layer above the trigger (offset mirrored) when there is more room there,
/// and slides it back inside the window along the trigger's edge.
pub fn attach_spec(align: Align) -> (Anchor, Anchor, Point<Pixels>) {
    match align {
        Align::Start => (Anchor::BottomLeft, Anchor::TopLeft, point(px(0.), px(GAP))),
        Align::End => (
            Anchor::BottomRight,
            Anchor::TopRight,
            point(px(0.), px(GAP)),
        ),
    }
}

/// Where a layer of `size` lands under `trigger` before any flip or slide
/// (both are GPUI's, tested there): what [`attach_spec`] asks for.
#[cfg(test)]
pub fn layer_bounds(
    trigger: Bounds<Pixels>,
    size: gpui_kit::Size<Pixels>,
    align: Align,
) -> Bounds<Pixels> {
    let (corner, anchor, offset) = attach_spec(align);
    Bounds::from_anchor_and_size(anchor, trigger.corner(corner) + offset, size)
}

/// Where a floating layer hangs: the bounds of its trigger in window
/// coordinates, recorded each time the trigger is laid out. A layer placed
/// from them follows the trigger wherever the page or a pane scrolled it.
///
/// One anchor can serve a family of triggers (the `ellipsis` of every
/// entry): each measures under its own key, and a layer only hangs from
/// the bounds of the key it asks for, never from another trigger's.
#[derive(Clone, Default)]
pub struct AnchorBounds(Rc<Cell<Option<(u64, Bounds<Pixels>)>>>);

impl AnchorBounds {
    /// An element over the trigger that records its bounds; a change
    /// redraws, so a layer opened before the trigger was laid out appears
    /// on the next frame. The trigger is its parent.
    pub fn measure(&self) -> AnyElement {
        self.measure_keyed(0)
    }

    /// As [`Self::measure`], for the trigger `key` of a family.
    pub fn measure_keyed(&self, key: u64) -> AnyElement {
        let cell = self.0.clone();
        crate::ui::measure(move |bounds, window| {
            if cell.get() != Some((key, bounds)) {
                cell.set(Some((key, bounds)));
                window.refresh();
            }
        })
    }

    /// `layer` 4 below the trigger, lined up with its start or end edge,
    /// flipped above when there is more room there (components.md 5).
    pub fn below(&self, layer: impl IntoElement, align: Align) -> Option<AnyElement> {
        self.below_keyed(0, layer, align, 1)
    }

    /// As [`Self::below`], drawn at `priority` among deferred layers.
    pub fn below_at(
        &self,
        layer: impl IntoElement,
        align: Align,
        priority: usize,
    ) -> Option<AnyElement> {
        self.below_keyed(0, layer, align, priority)
    }

    /// As [`Self::below_at`], under the trigger `key` of a family.
    pub fn below_keyed(
        &self,
        key: u64,
        layer: impl IntoElement,
        align: Align,
        priority: usize,
    ) -> Option<AnyElement> {
        let (measured, bounds) = self.0.get()?;
        if measured != key {
            return None;
        }
        let (corner, anchor, offset) = attach_spec(align);
        Some(
            deferred(
                anchored()
                    .attach(bounds, corner)
                    .anchor(anchor)
                    .offset(offset)
                    .snap_to_window_with_margin(px(8.))
                    .child(layer),
            )
            .with_priority(priority)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::size;

    fn trigger() -> Bounds<Pixels> {
        Bounds::new(point(px(300.), px(331.)), size(px(120.), px(28.)))
    }

    #[test]
    fn a_dropdown_hangs_4_under_its_trigger_left_edges_aligned() {
        let layer = layer_bounds(trigger(), size(px(200.), px(80.)), Align::Start);
        assert_eq!(layer.origin, point(px(300.), px(331. + 28. + 4.)));
    }

    #[test]
    fn an_end_menu_hangs_4_under_its_trigger_right_edges_aligned() {
        let layer = layer_bounds(trigger(), size(px(200.), px(80.)), Align::End);
        assert_eq!(layer.origin, point(px(300. + 120. - 200.), px(363.)));
        assert_eq!(layer.right(), trigger().right());
    }

    #[test]
    fn the_layer_follows_the_trigger_not_the_page_scroll() {
        // The same trigger scrolled 150 up: the layer moves with it.
        let mut scrolled = trigger();
        scrolled.origin.y -= px(150.);
        let before = layer_bounds(trigger(), size(px(200.), px(80.)), Align::Start);
        let after = layer_bounds(scrolled, size(px(200.), px(80.)), Align::Start);
        assert_eq!(before.origin.y - after.origin.y, px(150.));
    }

    #[test]
    fn the_layer_hangs_from_the_bottom_corner_and_only_flips_vertically() {
        // `attach` flips an axis only when the layer is outside the
        // trigger along it: below, never beside.
        for align in [Align::Start, Align::End] {
            let (corner, anchor, offset) = attach_spec(align);
            assert!(matches!(corner, Anchor::BottomLeft | Anchor::BottomRight));
            assert!(matches!(anchor, Anchor::TopLeft | Anchor::TopRight));
            assert_eq!(offset, point(px(0.), px(GAP)));
        }
    }
}
