//! Small pieces of docs/design/components.md 5 shared by several views.

use std::sync::atomic::{AtomicU8, Ordering};

use acta_core::model::{Author, Status};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    BoxShadow, Div, ElementId, FocusHandle, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Svg, div, px,
    transparent_black,
};

use crate::i18n::Key;
use crate::icons::{Icon, icon};
use crate::theme::{RADIUS_LG, RADIUS_MD, RADIUS_SM, RADIUS_XS, Tokens};
use crate::type_scale;

/// The keyboard focus ring of docs/design/components.md 5, drawn inside
/// the element's edge: a 2px `focus.ring` band, so it takes no layout
/// space and is never clipped by a parent. `gap` adds a 1px band of
/// `bg.base` inside it, for a control whose own fill is the accent.
pub fn ring_shadow(t: &Tokens, gap: bool) -> Vec<BoxShadow> {
    let ring = gpui_kit::BoxShadow::new(px(0.), px(0.), t.focus_ring)
        .spread_radius(px(2.))
        .inset();
    if gap {
        let under = gpui_kit::BoxShadow::new(px(0.), px(0.), t.bg_base)
            .spread_radius(px(3.))
            .inset();
        vec![under, ring]
    } else {
        vec![ring]
    }
}

/// Make a control a tab stop that shows the focus ring for keyboard focus
/// only (not after a click). `Enter` and `Space` activate it like a click.
pub fn focusable<E: StatefulInteractiveElement + Styled>(el: E, t: &Tokens, gap: bool) -> E {
    let ring = ring_shadow(t, gap);
    el.tab_index(0).focus_visible(move |s| s.shadow(ring))
}

/// The focus ring around a control that tracks its own focus handle (the
/// settings controls): 2px `focus.ring`, 1px outside the control's edge,
/// radius plus 1 outside it, shown for keyboard focus only. It takes no
/// layout space.
pub fn focus_ring(
    id: &'static str,
    handle: &FocusHandle,
    radius: f32,
    t: &Tokens,
) -> Stateful<Div> {
    let ring = t.focus_ring;
    div()
        .id(id)
        .track_focus(handle)
        .tab_index(0)
        .m(px(-3.))
        .p(px(1.))
        .border_2()
        .border_color(transparent_black())
        .rounded(px(radius + 3.))
        .focus_visible(move |style| style.border_color(ring))
}

/// A one-line tooltip (docs/design/components.md 5, Tooltip): `bg.overlay`,
/// 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4 x 8,
/// `type.small` `text.primary`. Built with [`tip`].
pub struct Tip(SharedString);

impl gpui_kit::Render for Tip {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let t = cx.global::<Tokens>();
        type_scale::SMALL
            .el()
            .px(px(8.))
            .py(px(4.))
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_MD))
            .shadow(vec![t.shadow_overlay()])
            .text_color(t.text_primary)
            .child(self.0.clone())
    }
}

/// The builder `.tooltip()` takes, for a tooltip reading `text`.
pub fn tip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut gpui_kit::Window, &mut gpui_kit::App) -> gpui_kit::AnyView + 'static {
    let text = text.into();
    move |_, cx| {
        use gpui_kit::AppContext as _;
        let text = text.clone();
        cx.new(|_| Tip(text)).into()
    }
}

/// A tooltip with its shortcut as kbd hints after the label
/// (docs/design/components.md 5, Tooltip).
pub struct TipKeys(SharedString, Vec<&'static str>);

impl gpui_kit::Render for TipKeys {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        let t = cx.global::<Tokens>();
        type_scale::SMALL
            .el()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .py(px(4.))
            .whitespace_nowrap()
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_MD))
            .shadow(vec![t.shadow_overlay()])
            .text_color(t.text_primary)
            .child(self.0.clone())
            .child(kbd(&self.1, t))
    }
}

/// The builder `.tooltip()` takes, for `text` and its `keys`.
pub fn tip_keys(
    text: impl Into<SharedString>,
    keys: Vec<&'static str>,
) -> impl Fn(&mut gpui_kit::Window, &mut gpui_kit::App) -> gpui_kit::AnyView + 'static {
    let text = text.into();
    move |_, cx| {
        use gpui_kit::AppContext as _;
        let (text, keys) = (text.clone(), keys.clone());
        cx.new(|_| TipKeys(text, keys)).into()
    }
}

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
                .rounded(px(RADIUS_XS))
                .border_1()
                .border_color(tokens.border_default)
                .text_color(tokens.text_tertiary)
                .child(key.to_string())
        }))
}

/// The kbd hint inside a primary button (components.md 5, kbd): no border,
/// `text.on-accent` at weight 400.
pub fn kbd_on_accent(keys: &[&str], tokens: &Tokens) -> Div {
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
                .text_color(tokens.text_on_accent)
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
    let el = div()
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
        .child(glyph);
    if disabled {
        el
    } else {
        focusable(el, tokens, false)
    }
}

/// A ghost control as tall as its bar, padding 0 x 8 (status bar items).
/// `id` is also the hover group, for an icon that follows the label.
pub fn bar_item(id: &'static str, tokens: &Tokens) -> Stateful<Div> {
    let (hover, text) = (tokens.fill_hover, tokens.text_secondary);
    let el = div()
        .id(id)
        .group(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h_full()
        .px(px(8.))
        .hover(move |style| style.bg(hover).text_color(text));
    focusable(el, tokens, false)
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
    let base = if disabled {
        base
    } else {
        focusable(base, t, variant == Variant::Primary)
    };
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
/// The unread dot: 6, `accent` (components.md 5, list / tree row).
pub fn unread_dot(t: &Tokens) -> Div {
    div()
        .flex_shrink_0()
        .size(px(6.))
        .rounded(px(3.))
        .bg(t.accent)
}

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
            .rounded(px(RADIUS_XS))
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
        .items_start()
        .gap(px(8.))
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .rounded(px(RADIUS_LG))
        .bg(tokens.bg_raised)
        .border_1()
        .border_color(tokens.border_subtle)
        // Icon, message and actions share a 28 first line, so a message
        // that wraps keeps its icon on the first line and a one-line
        // message stays level with the ghost actions.
        .children(glyph.map(|(glyph, color)| {
            div()
                .flex_shrink_0()
                .h(px(28.))
                .flex()
                .items_center()
                .child(icon(glyph, 16., color))
        }))
        .child(
            type_scale::UI
                .el()
                .flex_1()
                .min_w_0()
                .py(px(4.))
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
    let el = role.el().id(id.into());
    focusable(el, tokens, false)
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
    // Sized to its segments, never stretched by a column (M7 S15).
    div()
        .flex()
        .flex_row()
        .flex_none()
        .self_start()
        .h(px(28.))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(tokens.border_strong)
        .overflow_hidden()
}

/// The Reading width setting (Settings > Appearance): the widest the
/// content column gets (docs/design/typography.md 3.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ReadingWidth {
    Narrow,
    #[default]
    Default,
    Wide,
}

impl ReadingWidth {
    pub const ALL: [ReadingWidth; 3] = [
        ReadingWidth::Narrow,
        ReadingWidth::Default,
        ReadingWidth::Wide,
    ];

    /// The column's maximum width: about 65, 75 and 100 characters of
    /// reading text per line at the Default text size.
    pub fn column_max(self) -> f32 {
        match self {
            ReadingWidth::Narrow => 480.,
            ReadingWidth::Default => 560.,
            ReadingWidth::Wide => 720.,
        }
    }
}

/// The setting in force, written by the settings module (see
/// `type_scale::set_text_size` for why it is not a GPUI global).
static READING_WIDTH: AtomicU8 = AtomicU8::new(1);

pub fn set_reading_width(width: ReadingWidth) {
    let n = ReadingWidth::ALL
        .iter()
        .position(|w| *w == width)
        .unwrap_or(1);
    READING_WIDTH.store(n as u8, Ordering::Relaxed);
}

pub fn reading_width() -> ReadingWidth {
    ReadingWidth::ALL[(READING_WIDTH.load(Ordering::Relaxed) as usize).min(2)]
}

/// The content column's maximum width under the Reading width setting.
pub fn column_max() -> f32 {
    reading_width().column_max()
}

/// The content column: at most [`column_max`] wide, 32 above the title.
pub fn column() -> Div {
    div()
        .w_full()
        .max_w(px(column_max()))
        .pt(px(32.))
        .pb(px(48.))
        .flex()
        .flex_col()
}

thread_local! {
    /// The side padding of the page being laid out: 32, or 16 in a narrow
    /// page group (workbench.md 4.8.8). Set around a group's page by
    /// [`NarrowPage`].
    static PAGE_PADDING: std::cell::Cell<f32> = const { std::cell::Cell::new(32.) };
}

/// The side padding of pages (docs/design/layout.md 4.2; 16 in a page
/// group under 452 wide, workbench.md 4.8.8).
pub fn page_padding() -> f32 {
    PAGE_PADDING.with(|p| p.get())
}

/// The page of a group, laid out with the side padding its group's width
/// allows: pages render while they are laid out, so the padding is set
/// around that and put back after.
pub struct NarrowPage {
    child: gpui_kit::AnyElement,
    narrow: bool,
}

impl NarrowPage {
    pub fn new(child: gpui_kit::AnyElement, narrow: bool) -> Self {
        Self { child, narrow }
    }
}

impl gpui_kit::IntoElement for NarrowPage {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl gpui_kit::Element for NarrowPage {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui_kit::GlobalElementId>,
        _: Option<&gpui_kit::InspectorElementId>,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) -> (gpui_kit::LayoutId, ()) {
        let before = PAGE_PADDING.with(|p| p.replace(if self.narrow { 16. } else { 32. }));
        let id = self.child.request_layout(window, cx);
        PAGE_PADDING.with(|p| p.set(before));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui_kit::GlobalElementId>,
        _: Option<&gpui_kit::InspectorElementId>,
        _: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut (),
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&gpui_kit::GlobalElementId>,
        _: Option<&gpui_kit::InspectorElementId>,
        _: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) {
        self.child.paint(window, cx);
    }
}

/// The scrolling page area that centres the column, with 32 of side
/// padding (docs/design/layout.md 4.2), 16 in a narrow page group.
pub fn page_scroll(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id.into())
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .px(px(page_padding()))
}

/// A view's scrolling content in a side area or the bottom panel
/// (workbench.md 4.8.3): it starts 12 below the area header, with side
/// padding 12, and takes the full width.
pub fn view_scroll(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id.into())
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .p(px(12.))
}

/// The column of a view: the full width, no page title.
pub fn view_column() -> Div {
    div().w_full().flex().flex_col()
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
                .max_w(gpui_kit::relative(0.4))
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
/// the current language, measured in `type.ui` with the font's own
/// advances. The 40% cap is applied against the actual column by
/// [`property`].
pub fn label_width(labels: &[&str], cx: &gpui_kit::App) -> f32 {
    let system = cx.text_system();
    let font = system.resolve_font(&gpui_kit::font(type_scale::UI.family()));
    let size = px(type_scale::UI.size);
    let width = |label: &str| -> f32 {
        label
            .chars()
            .map(|ch| {
                system
                    .advance(font, size, ch)
                    .map(|a| f32::from(a.width))
                    .unwrap_or(type_scale::UI.size * 0.6)
            })
            .sum()
    };
    labels.iter().map(|l| width(l)).fold(0., f32::max).ceil() + 1.
}

/// A ghost value in the properties block: padding 0 x 8, shifted left by 8
/// so its text aligns with plain values.
pub fn ghost_value(id: impl Into<ElementId>, tokens: &Tokens) -> Stateful<Div> {
    let hover = tokens.fill_hover;
    let el = type_scale::UI.el().id(id.into());
    focusable(el, tokens, false)
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
