//! Markdown at rest (docs/design/components-editor.md 5, Rendering
//! contract; the settings of typography.md 3.3), drawn by gpui-base's text
//! view with Acta's extensions (`parse.rs`) and policy (`policy.rs`):
//! short surfaces (current state, description, entries, notes, answers) in
//! `type.reading` with compact headings, documents in `type.document` with
//! real heading sizes. Every length goes through `zoom::px` or rems, so the
//! text follows the window's zoom.
//!
//! Nothing in the text reaches outside Acta by itself: no image is fetched,
//! no HTML is rendered (`<br>` is a line break), only web and e-mail links
//! open, on a left click or `Enter`, and a link whose text shows another
//! host asks first. Every text view and nested list of a view is built by
//! [`text_view`] and [`render_context`], which route every activation
//! through [`activate`]. The per-text facts ([`Prepared`]) are read off
//! the UI thread; until they land, no web link opens.

mod highlight;
mod parse;
mod policy;

#[cfg(test)]
mod conformance;

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::text::editor_host::{FlowPosition, RenderContext};
use gpui_kit::base::text::{
    InlineElement, InlineRenderContext, MarkdownExtensions, MarkdownNode, TextView, TextViewStyle,
};
use gpui_kit::base::{ScrollableMask, Scrollbar, ScrollbarMode};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Axis, ClickEvent, ClipboardItem, Context, Div,
    ElementId, Entity, FocusHandle, FontStyle, FontWeight, HighlightStyle, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels, Point, Render, RenderOnce,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    StyledText, WeakEntity, Window, anchored, deferred, div, rems,
};

use crate::i18n::{Arg, Key, text, tr};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{FONT_MONO, RADIUS_MD, RADIUS_SM, RADIUS_XS, Tokens};
use crate::type_scale::{self, TextSize, TypeRole};
use crate::ui;
use crate::zoom::px;

use highlight::Role;
use parse::{CodeData, FnRefData, ImageData, ListData, PlainLinkData};
use policy::{ImageSource, LinkTarget, Prepared};

/// A short surface: `type.reading` at the Text size setting, paragraph gap
/// 12; headings stay close to the body (h1/h2 two px larger, h3 to h6 the
/// body size), so an entry never out-shouts the page title.
pub fn short(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    tokens: &Tokens,
) -> MarkdownView {
    MarkdownView {
        id: id.into(),
        text: text.into(),
        tokens: tokens.clone(),
        document: false,
    }
}

/// A document body: `type.document` at the Text size setting, paragraph gap
/// 16; h1 the page title's 24, h2 three px over the body (at most 22), h3
/// one px over it, so the steps hold at every Text size.
pub fn document(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    tokens: &Tokens,
) -> MarkdownView {
    MarkdownView {
        id: id.into(),
        text: text.into(),
        tokens: tokens.clone(),
        document: true,
    }
}

/// A Markdown text, drawn by [`short`] or [`document`].
#[derive(IntoElement)]
pub struct MarkdownView {
    id: ElementId,
    text: SharedString,
    tokens: Tokens,
    document: bool,
}

/// A view's state, kept while the view is drawn in consecutive frames
/// and dropped with it: the facts of its text, and the deceptive-link
/// popover it shows, if any.
#[derive(Default)]
struct ViewState {
    prepared: Option<(u64, Arc<Prepared>)>,
    preparing: Option<u64>,
    warning: Option<Warning>,
}

/// Where activation of a view's links goes: its state (for the popover)
/// and the facts of its text, `ready` once they are read.
#[derive(Clone)]
struct Links {
    state: WeakEntity<ViewState>,
    prep: Arc<Prepared>,
    ready: bool,
}

/// What the renderers need from the view.
struct Env {
    tokens: Tokens,
    style: TextViewStyle,
    document: bool,
    links: Links,
}

impl Env {
    fn role(&self) -> TypeRole {
        if self.document {
            type_scale::DOCUMENT
        } else {
            type_scale::READING
        }
    }

    fn dark(&self) -> bool {
        self.tokens.bg_base.l < 0.5
    }
}

/// `type.code` beside reading text: 12 / 13 / 14 for Small / Default /
/// Large (components-editor.md 5, element table).
fn inline_code_size() -> f32 {
    match type_scale::text_size() {
        TextSize::Small => 12.,
        TextSize::Default => 13.,
        TextSize::Large => 14.,
    }
}

fn text_style(tokens: &Tokens, document: bool) -> TextViewStyle {
    let body = if document {
        type_scale::DOCUMENT.size()
    } else {
        type_scale::READING.size()
    };
    let secondary = tokens.text_secondary;
    let heading = move |level: u8| {
        let size = match (document, level) {
            (false, 1 | 2) => body + 2.,
            (false, _) => body,
            (true, 1) => 24.,
            (true, 2) => (body + 3.).min(22.),
            (true, _) => body + 1.,
        };
        let refinement = StyleRefinement::default().text_size(px(size));
        if level >= 4 {
            refinement.text_color(secondary)
        } else {
            refinement
        }
    };
    // typography.md 3.3: no frame and no fill, a hairline under each row;
    // a table wider than the column scrolls inside its own block.
    let mut table = StyleRefinement::default()
        .border_0()
        .bg(tokens.bg_base.opacity(0.));
    table.overflow.x = Some(gpui_kit::Overflow::Scroll);
    TextViewStyle::default()
        .with_foreground(tokens.text_primary)
        .with_muted_foreground(tokens.text_secondary)
        .with_link(tokens.accent)
        .with_selection(tokens.selection)
        .with_code_background(tokens.bg_sunken)
        .with_border(tokens.border_subtle)
        .with_paragraph_gap(rems(if document { 1. } else { 0.75 }))
        .with_heading(heading)
        .with_inline_code_style(
            StyleRefinement::default()
                .font_family(FONT_MONO)
                .text_size(px(inline_code_size()))
                .text_color(tokens.text_primary)
                .bg(tokens.bg_sunken)
                .px(px(4.))
                .rounded(px(RADIUS_XS)),
        )
        .with_list_style(StyleRefinement::default().text_color(tokens.text_tertiary))
        .with_blockquote_style(
            StyleRefinement::default()
                .border_0()
                .pl(px(16.))
                .text_color(tokens.text_secondary),
        )
        .with_table(table)
        .with_table_head(
            StyleRefinement::default()
                .bg(tokens.bg_base.opacity(0.))
                .text_color(tokens.text_primary)
                .font_weight(FontWeight::SEMIBOLD),
        )
        .with_table_cell(StyleRefinement::default().px(px(12.)).py(px(6.)))
        .with_dark(tokens.bg_base.l < 0.5)
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

thread_local! {
    /// [`Prepared`] of texts read recently, so a text drawn again (a page
    /// opened again, its notes) does not wait for the background read.
    static PREPARED: RefCell<HashMap<u64, Arc<Prepared>>> = RefCell::new(HashMap::new());
    /// Whether the window is told when a code block's colours land.
    static LISTENING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn remember(key: u64, prep: Arc<Prepared>) {
    PREPARED.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 256 {
            cache.clear();
        }
        cache.insert(key, prep);
    });
}

/// The highlighting worker refreshes the windows when a block's colours
/// land, instead of the view polling for them.
fn listen_for_highlights(cx: &mut App) {
    if LISTENING.with(|l| l.replace(true)) {
        return;
    }
    let (landed, landings) = async_channel::unbounded::<()>();
    highlight::set_notifier(move || {
        let _ = landed.try_send(());
    });
    cx.spawn(async move |cx| {
        while landings.recv().await.is_ok() {
            while landings.try_recv().is_ok() {}
            cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
}

/// The facts of `text` for the view whose state is `state`: from the state
/// or the recent cache when already read, else read in the background
/// (a text with no `[` has none to read). `ready` is false until they are.
fn facts(text: &SharedString, state: &Entity<ViewState>, cx: &mut App) -> (Arc<Prepared>, bool) {
    let key = text_hash(text);
    if let Some((k, prep)) = &state.read(cx).prepared
        && *k == key
    {
        return (prep.clone(), true);
    }
    if !text.contains('[') {
        return (Arc::default(), true);
    }
    if let Some(prep) = PREPARED.with(|c| c.borrow().get(&key).cloned()) {
        state.update(cx, |s, _| s.prepared = Some((key, prep.clone())));
        return (prep, true);
    }
    if state.read(cx).preparing != Some(key) {
        state.update(cx, |s, _| s.preparing = Some(key));
        let owned = text.to_string();
        let task = cx.background_spawn(async move { policy::prepare(&owned) });
        let weak = state.downgrade();
        cx.spawn(async move |cx| {
            let prep = Arc::new(task.await);
            let _ = weak.update(cx, |s, cx| {
                remember(key, prep.clone());
                if s.preparing == Some(key) {
                    s.prepared = Some((key, prep));
                    s.preparing = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
    (Arc::default(), false)
}

/// A link that does not go where its text says (components-editor.md 5,
/// Deceptive links), waiting for the person's choice.
struct Warning {
    at: Point<Pixels>,
    url: String,
    message: String,
    open: String,
    cancel: String,
    focus: FocusHandle,
}

/// What activating a link does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    /// Open this address in the system browser or mail client.
    Open(String),
    /// Ask first: the link's text shows another host than `host`.
    Warn {
        url: String,
        host: String,
    },
    Nothing,
}

/// The decision behind [`activate`]: only a left click or `Enter`; web
/// and e-mail open, a disguised web link asks first, a web link whose
/// check has not run yet does nothing, the rest does nothing.
fn decide(url: &str, left_or_key: bool, prep: &Prepared, ready: bool) -> Action {
    if !left_or_key {
        return Action::Nothing;
    }
    match policy::classify_link(url) {
        LinkTarget::Web { .. } if !ready => Action::Nothing,
        LinkTarget::Web { url: target, host } => match prep.deceptive.get(url) {
            None => Action::Open(target),
            Some(_) => Action::Warn { url: target, host },
        },
        LinkTarget::Mail(mail) => Action::Open(mail),
        // `#slug` scrolling needs the page's scroll handle, which this
        // view is not given yet; paths and other schemes go nowhere.
        LinkTarget::Anchor(_) | LinkTarget::Inert | LinkTarget::Blocked => Action::Nothing,
    }
}

/// Activating a link of a view, from any of its text views, nested lists
/// or image lines.
fn activate(links: &Links, url: &str, event: &ClickEvent, window: &mut Window, cx: &mut App) {
    let left_or_key = match event {
        ClickEvent::Mouse(click) => click.up.button == MouseButton::Left,
        ClickEvent::Keyboard(_) => true,
        ClickEvent::Touch(tap) => !tap.long_press,
    };
    match decide(url, left_or_key, &links.prep, links.ready) {
        Action::Open(target) => cx.open_url(&target),
        Action::Warn { url, host } => {
            let locale = settings::locale(cx);
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            let warning = Warning {
                at: event.position(),
                url,
                message: tr(locale, Key::MdLinkMismatch, &[("host", Arg::Text(&host))]),
                open: tr(locale, Key::MdLinkOpenHost, &[("host", Arg::Text(&host))]),
                cancel: text(locale, Key::CommonCancel),
                focus,
            };
            let _ = links.state.update(cx, |s, cx| {
                s.warning = Some(warning);
                cx.notify();
            });
        }
        Action::Nothing => {}
    }
}

fn close_warning(state: &WeakEntity<ViewState>, cx: &mut App) {
    let _ = state.update(cx, |s, cx| {
        s.warning = None;
        cx.notify();
    });
}

fn warning_popover(state: &Entity<ViewState>, tokens: &Tokens, cx: &App) -> Option<AnyElement> {
    let w = state.read(cx).warning.as_ref()?;
    let url = w.url.clone();
    let (out, esc, cancel, open) = (
        state.downgrade(),
        state.downgrade(),
        state.downgrade(),
        state.downgrade(),
    );
    let panel = crate::menu::panel("md-link-warning", tokens)
        .w(px(320.))
        .p(px(12.))
        .gap(px(12.))
        .track_focus(&w.focus)
        .on_mouse_down_out(move |_, _, cx| close_warning(&out, cx))
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
                cx.stop_propagation();
                close_warning(&esc, cx);
            }
        })
        .child(
            type_scale::UI
                .el()
                .text_color(tokens.text_primary)
                .child(w.message.clone()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(8.))
                .child(
                    ui::button(
                        "md-link-cancel",
                        w.cancel.clone(),
                        ui::Variant::Ghost,
                        false,
                        tokens,
                    )
                    .on_click(move |_, _, cx| close_warning(&cancel, cx)),
                )
                .child(
                    ui::button(
                        "md-link-open",
                        w.open.clone(),
                        ui::Variant::Secondary,
                        false,
                        tokens,
                    )
                    .on_click(move |_, _, cx| {
                        cx.open_url(&url);
                        close_warning(&open, cx);
                    }),
                ),
        );
    Some(
        deferred(
            anchored()
                .position(w.at)
                .snap_to_window_with_margin(px(8.))
                .child(panel),
        )
        .with_priority(1)
        .into_any_element(),
    )
}

/// The widest a tooltip's destination line may be before it is cut in
/// the middle (components-editor.md 5, Links).
const TIP_WIDTH: f32 = 480.;

/// `text` cut in the middle until it fits [`TIP_WIDTH`] in `type.code` at
/// 12.
fn fit_tip(text: &str, cx: &App) -> String {
    let fits = |s: &str| ui::text_width(type_scale::CODE_SMALL, s, cx) <= TIP_WIDTH;
    if fits(text) {
        return text.to_string();
    }
    let (mut lo, mut hi) = (3, text.chars().count());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if fits(&policy::middle_truncate(text, mid)) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    policy::middle_truncate(text, lo)
}

/// A tooltip with an optional message over a destination in `type.code`
/// at 12 (components-editor.md 5, Links and Images).
struct DestinationTip {
    message: Option<SharedString>,
    destination: SharedString,
}

impl Render for DestinationTip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>();
        let code = type_scale::CODE_SMALL;
        type_scale::SMALL
            .el()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(8.))
            .py(px(4.))
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_MD))
            .shadow(vec![t.shadow_overlay()])
            .text_color(t.text_primary)
            .children(self.message.clone())
            .child(
                div()
                    .font_family(code.family())
                    .text_size(px(code.size))
                    .line_height(px(code.line))
                    .whitespace_nowrap()
                    .child(self.destination.clone()),
            )
    }
}

fn destination_tip(message: Option<String>, destination: &str, cx: &mut App) -> AnyView {
    let destination = fit_tip(destination, cx).into();
    let message = message.map(SharedString::from);
    cx.new(|_| DestinationTip {
        message,
        destination,
    })
    .into()
}

/// Every text view of a Markdown view: its text, Acta's extensions, and
/// the link hook, handler and tooltip that route through [`activate`].
fn text_view(id: ElementId, text: SharedString, prep: &Arc<Prepared>, env: &Arc<Env>) -> TextView {
    let revision = text_hash(&text) ^ u64::from(env.links.ready);
    let links = env.links.clone();
    TextView::markdown(id, text)
        .style(env.style.clone())
        .markdown_extensions(extensions(prep, env, false).parser_revision(revision))
        .link(|link, _| {
            policy::classify_link(&link.url)
                .is_link()
                .then(|| link.clone())
        })
        .on_link_click(move |url, event, window, cx| activate(&links, url, event, window, cx))
        .link_tooltip(|url, _, cx| destination_tip(None, &link_destination(url), cx))
}

/// Every nested list's rendering context: the same extensions and the
/// same [`activate`]. Links that do not open were claimed as plain text
/// when the list was parsed, since a context has no link hook.
fn render_context(prep: &Arc<Prepared>, env: &Arc<Env>) -> RenderContext {
    let links = env.links.clone();
    RenderContext::new(env.style.clone())
        .with_markdown_extensions(Arc::new(extensions(prep, env, true)))
        .with_body_font_size(px(env.role().size()))
        .on_link_click(move |url, event, window, cx| activate(&links, url, event, window, cx))
}

/// The destination a link's tooltip shows: a web address normalised, its
/// host in ASCII.
fn link_destination(url: &str) -> String {
    match policy::classify_link(url) {
        LinkTarget::Web { url, .. } => url,
        _ => url.to_string(),
    }
}

impl RenderOnce for MarkdownView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        listen_for_highlights(cx);
        let tokens = &self.tokens;
        let role = if self.document {
            type_scale::DOCUMENT
        } else {
            type_scale::READING
        };
        let root = div()
            .w_full()
            .font_family(role.family())
            .text_size(px(role.size()))
            .line_height(px(role.line()))
            .text_color(tokens.text_primary);
        // Too deeply nested to parse safely: the text as it is.
        if policy::too_deep(&self.text) {
            return root
                .flex()
                .flex_col()
                .children(self.text.lines().map(|line| div().child(line.to_string())));
        }
        let key = ElementId::NamedInteger("md-view".into(), text_hash(&format!("{:?}", self.id)));
        let state = window.use_keyed_state(key, cx, |_, _| ViewState::default());
        let (prep, ready) = facts(&self.text, &state, cx);
        let env = Arc::new(Env {
            tokens: tokens.clone(),
            style: text_style(tokens, self.document),
            document: self.document,
            links: Links {
                state: state.downgrade(),
                prep: prep.clone(),
                ready,
            },
        });
        let notes = notes(&self.id, &prep, &env);
        let popover = warning_popover(&state, tokens, cx);
        root.child(text_view(self.id, self.text, &prep, &env))
            .children(notes)
            .children(popover)
    }
}

/// The extensions with Acta's renderers (`nested`: see
/// [`parse::extensions`]).
fn extensions(prep: &Arc<Prepared>, env: &Arc<Env>, nested: bool) -> MarkdownExtensions {
    let image_env = env.clone();
    let fnref_env = env.clone();
    let fnref_prep = prep.clone();
    let span_env = env.clone();
    let mut ext = parse::extensions(
        prep,
        parse::Renderers {
            image: Some(Arc::new(move |node, context, window, cx| {
                render_image(node, context, &image_env, window, cx)
            })),
            fnref: Some(Arc::new(move |node, context, window, cx| {
                render_fnref(node, context, &fnref_prep, &fnref_env, window, cx)
            })),
            code_span: Some(Arc::new(move |node, _, _, _| {
                Some(InlineElement::new(code_span(
                    node.as_text(),
                    &span_env.tokens,
                )))
            })),
            plain_link: Some(Arc::new(|node, _, _, _| render_plain_link(node))),
        },
        nested,
    );
    let code_env = env.clone();
    let list_env = env.clone();
    let list_prep = prep.clone();
    ext = ext
        .block_renderer(parse::CODE, move |node, window, cx| {
            render_code(node, &code_env, window, cx)
        })
        .block_renderer(parse::HIDDEN, |_, _, _| div())
        .block_renderer(parse::LIST, move |node, window, cx| {
            render_list(node, &list_prep, &list_env, window, cx)
        });
    for (name, key) in parse::ALERTS.into_iter().zip([
        Key::MdAlertNote,
        Key::MdAlertTip,
        Key::MdAlertImportant,
        Key::MdAlertWarning,
        Key::MdAlertCaution,
    ]) {
        let tokens = env.tokens.clone();
        let warn = matches!(name, "warning" | "caution");
        ext = ext.block_surface(name, move |_, cx| {
            let word = text(settings::locale(cx), key);
            div()
                .flex()
                .flex_col()
                .pl(px(16.))
                .text_color(tokens.text_secondary)
                .child(
                    type_scale::UI_STRONG
                        .el()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .pb(px(4.))
                        .text_color(tokens.text_secondary)
                        .when(warn, |row| {
                            row.child(icon(Icon::TriangleAlert, 14., tokens.warning))
                        })
                        .child(word),
                )
        });
    }
    ext
}

/// The colours of a code block's chrome (label, Copy, scrollbar): Monokai's
/// own on its dark block, the theme's in light.
struct Chrome {
    label: gpui_kit::Hsla,
    icon: gpui_kit::Hsla,
    icon_hover: gpui_kit::Hsla,
    hover: gpui_kit::Hsla,
    pressed: gpui_kit::Hsla,
    thumb: Option<gpui_kit::Hsla>,
}

impl Chrome {
    fn of(env: &Env) -> Self {
        let t = &env.tokens;
        if env.dark() {
            let fg = t.syntax_monokai_fg;
            Chrome {
                label: t.syntax_monokai_line_number,
                icon: t.syntax_monokai_line_number,
                icon_hover: fg,
                hover: fg.opacity(0.10),
                pressed: fg.opacity(0.16),
                thumb: Some(fg),
            }
        } else {
            Chrome {
                label: t.text_tertiary,
                icon: t.text_secondary,
                icon_hover: t.text_primary,
                hover: t.fill_hover,
                pressed: t.fill_pressed,
                thumb: None,
            }
        }
    }
}

/// Code blocks: `type.code` 13 / 20 in Monokai (classic on its dark block
/// in the dark theme, darkened on `bg.sunken` in light), padding 12 x 16,
/// radius 8; never wraps, scrolls sideways inside the block; the label and
/// Copy show on hover or keyboard focus (components-editor.md 5, Code).
fn render_code(node: &MarkdownNode, env: &Env, window: &mut Window, cx: &mut App) -> AnyElement {
    let Some(data) = node.data::<CodeData>() else {
        return div().into_any_element();
    };
    let t = &env.tokens;
    let start = node.source_range().map_or(0, |r| r.start);
    let scroll = window
        .use_keyed_state(("md-code-scroll", start), cx, |_, _| ScrollHandle::new())
        .read(cx)
        .clone();
    let copied = window.use_keyed_state(("md-code-copied", start), cx, |_, _| false);
    let focus = window
        .use_keyed_state(("md-code-focus", start), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();
    let is_copied = *copied.read(cx);
    let shown = is_copied || focus.is_focused(window);
    // Highlighting runs on the worker: plain text until its runs land,
    // then the worker's notice redraws the window.
    let highlights: Vec<(Range<usize>, HighlightStyle)> = data
        .lang
        .as_deref()
        .and_then(|lang| highlight::runs(&data.code, lang))
        .unwrap_or_default()
        .iter()
        .map(|(range, role)| (range.clone(), role_style(*role, t)))
        .collect();
    let group: SharedString = format!("md-code-{start}").into();
    let locale = settings::locale(cx);
    let code = data.code.clone();
    let chrome_colors = Chrome::of(env);
    let (icon_hover, hover, pressed) = (
        chrome_colors.icon_hover,
        chrome_colors.hover,
        chrome_colors.pressed,
    );
    let copy_group: SharedString = format!("md-code-copy-{start}").into();
    let copy = ui::focusable(
        div()
            .id(copy_group.clone())
            .group(copy_group.clone())
            .track_focus(&focus)
            .flex()
            .items_center()
            .justify_center()
            .size(px(24.))
            .rounded(px(RADIUS_SM))
            .hover(move |s| s.bg(hover))
            .active(move |s| s.bg(pressed))
            .child(
                icon(
                    if is_copied { Icon::Check } else { Icon::Copy },
                    14.,
                    chrome_colors.icon,
                )
                .group_hover(copy_group, move |s| s.text_color(icon_hover)),
            ),
        t,
        false,
    )
    .tooltip(ui::tip(text(
        locale,
        if is_copied {
            Key::CommonCopied
        } else {
            Key::MdCopyCode
        },
    )))
    .on_click(move |_, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
        let copied = copied.clone();
        copied.update(cx, |c, cx| {
            *c = true;
            cx.notify();
        });
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            copied.update(cx, |c, cx| {
                *c = false;
                cx.notify();
            });
        })
        .detach();
    });
    let chrome = div()
        .absolute()
        .top(px(4.))
        .right(px(4.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .p(px(4.))
        .rounded(px(RADIUS_MD))
        .bg(t.syntax_monokai_bg)
        .when(!shown, |c| {
            c.invisible().group_hover(group.clone(), |s| s.visible())
        })
        .children(data.label.clone().map(|label| {
            type_scale::SMALL
                .el()
                .max_w(px(160.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_color(chrome_colors.label)
                .child(label)
        }))
        .child(copy);
    let thumb = chrome_colors.thumb;
    div()
        .id(("md-code", start))
        .group(group)
        .relative()
        .w_full()
        .rounded(px(RADIUS_MD))
        .bg(t.syntax_monokai_bg)
        .child(
            div()
                .id(("md-code-x", start))
                .w_full()
                .flex()
                .flex_row()
                .overflow_hidden()
                .track_scroll(&scroll)
                .child(
                    div()
                        .flex_none()
                        .px(px(16.))
                        .py(px(12.))
                        .font_family(FONT_MONO)
                        .text_size(px(type_scale::CODE.size))
                        .line_height(px(type_scale::CODE.line))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(t.syntax_monokai_fg)
                        .whitespace_nowrap()
                        .child(StyledText::new(data.code.clone()).with_highlights(highlights)),
                ),
        )
        .child(ScrollableMask::new(Axis::Horizontal, &scroll).id(("md-code-mask", start)))
        .child(
            div().absolute().inset_0().child(
                Scrollbar::horizontal(&scroll)
                    .id(("md-code-bar", start))
                    .mode(ScrollbarMode::Always)
                    .when_some(thumb, |bar, fg| {
                        bar.styles(|s| {
                            s.thumb(|th| th.bg(fg.opacity(0.30)))
                                .thumb_hover(|th| th.bg(fg.opacity(0.45)))
                        })
                    }),
            ),
        )
        .child(chrome)
        .into_any_element()
}

/// The Monokai colour of a role; storage and support types and
/// parameters in the bundled JetBrains Mono NL Italic, as classic Monokai
/// sets them; never bold (components-editor.md 5, Code).
fn role_style(role: Role, t: &Tokens) -> HighlightStyle {
    let color = match role {
        Role::Keyword | Role::Removed => t.syntax_monokai_keyword,
        Role::String => t.syntax_monokai_string,
        Role::Constant => t.syntax_monokai_constant,
        Role::Function | Role::Added => t.syntax_monokai_function,
        Role::Type => t.syntax_monokai_type,
        Role::Parameter => t.syntax_monokai_parameter,
        Role::Comment | Role::Hunk => t.syntax_monokai_comment,
    };
    HighlightStyle {
        color: Some(color),
        font_style: role.italic().then_some(FontStyle::Italic),
        ..HighlightStyle::default()
    }
}

/// A nested ordered list, drawn as a list of its own so it counts in
/// numbers at every depth.
fn render_list(
    node: &MarkdownNode,
    prep: &Arc<Prepared>,
    env: &Arc<Env>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Some(data) = node.data::<ListData>() else {
        return div().into_any_element();
    };
    let context = render_context(prep, env);
    let blocks = [&data.block];
    data.block
        .render(0, FlowPosition::at(&blocks, 0, true), &context, window, cx)
}

/// Inline code drawn by Acta (a span over a line ending): the look the
/// text view gives inline code.
fn code_span(code: &str, t: &Tokens) -> Div {
    div()
        .font_family(FONT_MONO)
        .text_size(px(inline_code_size()))
        .text_color(t.text_primary)
        .bg(t.bg_sunken)
        .px(px(4.))
        .rounded(px(RADIUS_XS))
        .child(code.to_string())
}

/// A link of a nested list that does not open: its text, with its
/// destination (and why it does not open) as tooltip.
fn render_plain_link(node: &MarkdownNode) -> Option<InlineElement> {
    let data = node.data::<PlainLinkData>()?;
    let start = node.source_range().map_or(0, |r| r.start);
    let blocked = matches!(policy::classify_link(&data.dest), LinkTarget::Blocked);
    let dest = data.dest.clone();
    Some(InlineElement::new(
        div()
            .id(("md-plain-link", start))
            .child(node.as_text().to_string())
            .tooltip(move |_, cx| {
                let message = blocked.then(|| text(settings::locale(cx), Key::MdLinkBlocked));
                destination_tip(message, &dest, cx)
            }),
    ))
}

/// An image: never fetched. Until attachments are drawn here, every image
/// is the quiet line of components-editor.md 5 (Images): `image-off` and
/// the alt text in `text.secondary`, `type.small`; a web image's line is a
/// link to its address, under the Links rules.
fn render_image(
    node: &MarkdownNode,
    _: &InlineRenderContext,
    env: &Env,
    _: &mut Window,
    cx: &mut App,
) -> Option<InlineElement> {
    let data = node.data::<ImageData>()?;
    let t = &env.tokens;
    let locale = settings::locale(cx);
    let alt = if data.alt.trim().is_empty() {
        text(locale, Key::MdImage)
    } else {
        data.alt.clone()
    };
    let start = node.source_range().map_or(0, |r| r.start);
    let web = match policy::classify_image(&data.dest) {
        ImageSource::Web(_) => Some(data.dest.clone()),
        ImageSource::Attachment(_) | ImageSource::Other => None,
    };
    let dest = data.dest.clone();
    let links = env.links.clone();
    let line = type_scale::SMALL
        .el()
        .id(("md-image", start))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .text_color(t.text_secondary)
        .child(icon(Icon::ImageOff, 14., t.text_secondary))
        .child(alt)
        .tooltip(move |_, cx| {
            let message = text(settings::locale(cx), Key::MdImageBlocked);
            destination_tip(Some(message), &dest, cx)
        })
        .when_some(web, |line, url| {
            line.cursor_pointer()
                .on_click(move |event, window, cx| activate(&links, &url, event, window, cx))
        });
    Some(InlineElement::new(line))
}

/// A footnote reference: its number in `type.small` `accent`, raised 4;
/// its note as the tooltip.
fn render_fnref(
    node: &MarkdownNode,
    _: &InlineRenderContext,
    prep: &Prepared,
    env: &Env,
    _: &mut Window,
    _: &mut App,
) -> Option<InlineElement> {
    let FnRefData { number } = *node.data::<FnRefData>()?;
    let note = prep
        .notes
        .iter()
        .find(|n| n.number == number)
        .map(|n| policy::middle_truncate(&n.markdown, 200))
        .unwrap_or_default();
    let start = node.source_range().map_or(0, |r| r.start);
    let small = type_scale::SMALL;
    let el = div()
        .id(("md-fnref", start))
        .px(px(1.))
        .pb(px(4.))
        .font_family(small.family())
        .text_size(px(small.size))
        .line_height(px(small.line))
        .font_features(type_scale::tabular())
        .text_color(env.tokens.accent)
        .child(number.to_string())
        .tooltip(ui::tip(note));
    // The box reaches 4 below the glyphs; claiming its baseline 4 lower
    // than theirs raises the number by 4 (a baseline past the box falls
    // back to plain text).
    Some(InlineElement::new(el).with_baseline(px(small.line * 0.78 + 4.)))
}

/// The footnotes, out of the flow at the end of the text: a rule, then a
/// numbered list one step under the body in `text.secondary`, each note
/// a text view of its own built by [`text_view`].
fn notes(owner: &ElementId, prep: &Arc<Prepared>, env: &Arc<Env>) -> Option<Div> {
    if prep.notes.is_empty() {
        return None;
    }
    let t = &env.tokens;
    let (size, line) = if env.document { (15., 24.) } else { (14., 20.) };
    let mut list = div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .text_size(px(size))
        .line_height(px(line))
        .text_color(t.text_secondary);
    for note in &prep.notes {
        let note_prep = Arc::new((*note.prepared).clone());
        let note_env = Arc::new(Env {
            tokens: env.tokens.clone(),
            style: env.style.clone(),
            document: env.document,
            links: Links {
                prep: note_prep.clone(),
                ..env.links.clone()
            },
        });
        let id = ElementId::NamedInteger(format!("{owner:?}/note").into(), u64::from(note.number));
        list = list.child(
            div()
                .flex()
                .flex_row()
                .child(
                    div()
                        .flex_none()
                        .min_w(px(24.))
                        .pr(px(6.))
                        .text_right()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(format!("{}.", note.number)),
                )
                .child(div().flex_1().min_w_0().child(text_view(
                    id,
                    note.markdown.clone().into(),
                    &note_prep,
                    &note_env,
                ))),
        );
    }
    Some(
        div()
            .w_full()
            .pt(px(16.))
            .child(div().h(px(1.)).bg(t.border_subtle))
            .child(div().pt(px(16.)).child(list)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mocked opener: what each activation would open.
    fn opened(url: &str, left: bool, prep: &Prepared, ready: bool) -> Vec<String> {
        let mut opener = Vec::new();
        if let Action::Open(target) = decide(url, left, prep, ready) {
            opener.push(target);
        }
        opener
    }

    #[test]
    fn only_web_and_mail_open_and_only_on_a_left_click_or_enter() {
        let prep = Prepared::default();
        assert_eq!(
            opened("https://example.com", true, &prep, true),
            ["https://example.com/"]
        );
        assert_eq!(
            opened("mailto:alexandre@example.com", true, &prep, true),
            ["mailto:alexandre@example.com"]
        );
        for url in [
            "file:///C:/Windows",
            "javascript:alert(1)",
            "ms-settings:privacy",
            "src/main.rs",
            "#storage",
        ] {
            assert!(opened(url, true, &prep, true).is_empty(), "{url}");
        }
        assert!(opened("https://example.com", false, &prep, true).is_empty());
        // Until the text's check has run, no web link opens.
        assert!(opened("https://example.com", true, &prep, false).is_empty());
    }

    #[test]
    fn a_disguised_link_asks_first() {
        let prep = policy::prepare("[github.com](https://evil.example/x)\n");
        assert_eq!(
            decide("https://evil.example/x", true, &prep, true),
            Action::Warn {
                url: "https://evil.example/x".into(),
                host: "evil.example".into()
            }
        );
    }

    /// Every text view and nested-list context of this module is built by
    /// the two helpers that route activation through `activate`: the
    /// main view, the notes and the nested lists alike.
    #[test]
    fn every_view_routes_links_through_activate() {
        let source = include_str!("markdown_view.rs");
        let body = &source[..source.find("#[cfg(test)]\nmod tests").expect("tests")];
        let count = |needle: &str| body.matches(needle).count();
        assert_eq!(count("TextView::markdown("), 1, "only text_view builds one");
        assert_eq!(
            count("RenderContext::new("),
            1,
            "only render_context builds one"
        );
        assert_eq!(count(".on_link_click("), 2);
        assert_eq!(
            count("activate(&links"),
            3,
            "text_view, render_context, image lines"
        );
        assert_eq!(count("cx.open_url("), 2, "activate, and the popover's Open");
        for caller in ["text_view(", "render_context("] {
            assert!(count(caller) >= 2, "{caller} is used");
        }
    }
}
