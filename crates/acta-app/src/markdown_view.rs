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
//! host asks first.

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
    AnyElement, App, Axis, ClickEvent, ClipboardItem, Div, ElementId, FocusHandle, FontStyle,
    FontWeight, HighlightStyle, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Pixels, Point, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, StyledText, Window, anchored, deferred, div, rems,
};

use crate::i18n::{Arg, Key, text, tr};
use crate::icons::{Icon, icon};
use crate::settings;
use crate::theme::{FONT_MONO, RADIUS_MD, RADIUS_XS, Tokens};
use crate::type_scale::{self, TextSize, TypeRole};
use crate::ui;
use crate::zoom::px;

use highlight::Role;
use parse::{CodeData, FnRefData, ImageData, ListData};
use policy::{ImageSource, LinkTarget, Prepared};

/// A short surface: `type.reading` at the Text size setting, paragraph gap
/// 12; headings stay close to the body (h1/h2 two px larger, h3 to h6 the
/// body size), so an entry never out-shouts the page title.
pub fn short(id: impl Into<ElementId>, text: impl Into<SharedString>, tokens: &Tokens) -> Div {
    view(id.into(), text.into(), tokens, false)
}

/// A document body: `type.document` at the Text size setting, paragraph gap
/// 16; h1 the page title's 24, h2 three px over the body (at most 22), h3
/// one px over it, so the steps hold at every Text size.
pub fn document(id: impl Into<ElementId>, text: impl Into<SharedString>, tokens: &Tokens) -> Div {
    view(id.into(), text.into(), tokens, true)
}

/// What the renderers need from the view.
struct Env {
    tokens: Tokens,
    style: TextViewStyle,
    document: bool,
}

impl Env {
    fn role(&self) -> TypeRole {
        if self.document {
            type_scale::DOCUMENT
        } else {
            type_scale::READING
        }
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

thread_local! {
    /// [`Prepared`] per text, so a frame does not parse a text twice.
    static PREPARED: RefCell<HashMap<u64, Arc<Prepared>>> = RefCell::new(HashMap::new());
    /// The deceptive-link popover on screen, if any.
    static WARNING: RefCell<Option<Warning>> = const { RefCell::new(None) };
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn prepared(text: &str) -> Arc<Prepared> {
    let key = text_hash(text);
    PREPARED.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(p) = cache.get(&key) {
            return p.clone();
        }
        if cache.len() > 512 {
            cache.clear();
        }
        let p = Arc::new(policy::prepare(text));
        cache.insert(key, p.clone());
        p
    })
}

/// A link that does not go where its text says (components-editor.md 5,
/// Deceptive links), waiting for the person's choice.
struct Warning {
    owner: SharedString,
    at: Point<Pixels>,
    url: String,
    message: String,
    open: String,
    cancel: String,
    focus: FocusHandle,
}

fn close_warning(window: &mut Window) {
    WARNING.with(|w| w.borrow_mut().take());
    window.refresh();
}

/// What activating a link does: only a left click or `Enter`; web and
/// e-mail open, a disguised web link asks first, the rest does nothing.
fn activate(
    owner: &SharedString,
    prep: &Prepared,
    url: &str,
    event: &ClickEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if let ClickEvent::Mouse(click) = event
        && click.up.button != MouseButton::Left
    {
        return;
    }
    match policy::classify_link(url) {
        LinkTarget::Web { url: target, host } => match prep.deceptive.get(url) {
            None => cx.open_url(&target),
            Some(_) => {
                let locale = settings::locale(cx);
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                let warning = Warning {
                    owner: owner.clone(),
                    at: event.position(),
                    url: target,
                    message: tr(locale, Key::MdLinkMismatch, &[("host", Arg::Text(&host))]),
                    open: tr(locale, Key::MdLinkOpenHost, &[("host", Arg::Text(&host))]),
                    cancel: text(locale, Key::CommonCancel),
                    focus,
                };
                WARNING.with(|w| *w.borrow_mut() = Some(warning));
                window.refresh();
            }
        },
        LinkTarget::Mail(mail) => cx.open_url(&mail),
        // `#slug` scrolling needs the page's scroll handle, which this
        // view is not given yet; paths and other schemes go nowhere.
        LinkTarget::Anchor(_) | LinkTarget::Inert | LinkTarget::Blocked => {}
    }
}

fn warning_popover(owner: &SharedString, tokens: &Tokens) -> Option<AnyElement> {
    WARNING.with(|w| {
        let w = w.borrow();
        let w = w.as_ref().filter(|w| w.owner == *owner)?;
        let url = w.url.clone();
        let panel = crate::menu::panel("md-link-warning", tokens)
            .w(px(320.))
            .p(px(12.))
            .gap(px(12.))
            .track_focus(&w.focus)
            .on_mouse_down_out(|_, window, _| close_warning(window))
            .on_key_down(|event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    close_warning(window);
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
                        .on_click(|_, window, _| close_warning(window)),
                    )
                    .child(
                        ui::button(
                            "md-link-open",
                            w.open.clone(),
                            ui::Variant::Secondary,
                            false,
                            tokens,
                        )
                        .on_click(move |_, window, cx| {
                            cx.open_url(&url);
                            close_warning(window);
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
    })
}

/// The tooltip of a link: its destination, so where a click goes is
/// visible before the click.
fn link_tooltip(url: &str) -> String {
    match policy::classify_link(url) {
        LinkTarget::Web { url, .. } => policy::middle_truncate(&url, 80),
        _ => policy::middle_truncate(url, 80),
    }
}

fn view(id: ElementId, text: SharedString, tokens: &Tokens, document: bool) -> Div {
    highlight::warm();
    let prep = prepared(&text);
    let env = Arc::new(Env {
        tokens: tokens.clone(),
        style: text_style(tokens, document),
        document,
    });
    let owner: SharedString = format!("{id:?}").into();
    let role = env.role();
    let notes = notes(&owner, &prep, &env);
    let popover = warning_popover(&owner, tokens);
    let click_prep = prep.clone();
    let click_owner = owner.clone();
    let markdown = TextView::markdown(id, text.clone())
        .style(env.style.clone())
        .markdown_extensions(extensions(&prep, &env).parser_revision(text_hash(&text)))
        .link(|link, _| {
            policy::classify_link(&link.url)
                .is_link()
                .then(|| link.clone())
        })
        .on_link_click(move |url, event, window, cx| {
            activate(&click_owner, &click_prep, url, event, window, cx)
        })
        .link_tooltip(|url, window, cx| ui::tip(link_tooltip(url))(window, cx));
    div()
        .w_full()
        .font_family(role.family())
        .text_size(px(role.size()))
        .line_height(px(role.line()))
        .text_color(tokens.text_primary)
        .child(markdown)
        .children(notes)
        .children(popover)
}

/// The extensions with Acta's renderers.
fn extensions(prep: &Arc<Prepared>, env: &Arc<Env>) -> MarkdownExtensions {
    let image_env = env.clone();
    let fnref_env = env.clone();
    let fnref_prep = prep.clone();
    let code_env = env.clone();
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
                    &code_env.tokens,
                )))
            })),
        },
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

/// Code blocks: `type.code` 13 / 20 in the Monokai palette, the same dark
/// block in both themes, padding 12 x 16, radius 8; never wraps, scrolls
/// sideways inside the block; the label and Copy show on hover
/// (components-editor.md 5, Code).
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
    let is_copied = *copied.read(cx);
    // Highlighting runs on a worker: plain text until it is done, then a
    // frame later in colour, without waiting for input.
    let runs = data
        .lang
        .as_deref()
        .map(|lang| highlight::runs(&data.code, lang));
    if matches!(runs, Some(None)) {
        window.request_animation_frame();
    }
    let highlights: Vec<(Range<usize>, HighlightStyle)> = runs
        .flatten()
        .unwrap_or_default()
        .iter()
        .map(|(range, role)| (range.clone(), role_style(*role, t)))
        .collect();
    let group: SharedString = format!("md-code-{start}").into();
    let locale = settings::locale(cx);
    let code = data.code.clone();
    // A compact ghost icon button in Monokai's own text colours, on the
    // block's background in both themes.
    let (fg, dim) = (t.syntax_monokai_fg, t.syntax_monokai_line_number);
    let copy_group: SharedString = format!("md-code-copy-{start}").into();
    let copy = ui::focusable(
        div()
            .id(copy_group.clone())
            .group(copy_group.clone())
            .flex()
            .items_center()
            .justify_center()
            .size(px(24.))
            .rounded(px(crate::theme::RADIUS_SM))
            .hover(move |s| s.bg(fg.opacity(0.10)))
            .active(move |s| s.bg(fg.opacity(0.16)))
            .child(
                icon(if is_copied { Icon::Check } else { Icon::Copy }, 14., dim)
                    .group_hover(copy_group, move |s| s.text_color(fg)),
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
        .when(!is_copied, |c| {
            c.invisible().group_hover(group.clone(), |s| s.visible())
        })
        .children(data.label.clone().map(|label| {
            type_scale::SMALL
                .el()
                .max_w(px(160.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_color(t.syntax_monokai_line_number)
                .child(label)
        }))
        .child(copy);
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
                    .styles(|s| {
                        s.thumb(|th| th.bg(fg.opacity(0.30)))
                            .thumb_hover(|th| th.bg(fg.opacity(0.45)))
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
    let click_prep = prep.clone();
    let context = RenderContext::new(env.style.clone())
        .with_markdown_extensions(Arc::new(extensions(prep, env)))
        .with_body_font_size(px(env.role().size()))
        .on_link_click(move |url, event, window, cx| {
            // A nested list has no popover of its own: a disguised link
            // there does nothing rather than open unchecked.
            if !click_prep.deceptive.contains_key(url.as_ref()) {
                activate(
                    &SharedString::default(),
                    &click_prep,
                    url,
                    event,
                    window,
                    cx,
                );
            }
        });
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

/// An image: never fetched. Until attachments are drawn here, every image
/// is the quiet line of components-editor.md 5 (Images): `image-off` and
/// the alt text in `text.secondary`, `type.small`; a web image's line is a
/// link to its address.
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
    let tip = format!(
        "{}\n{}",
        text(locale, Key::MdImageBlocked),
        policy::middle_truncate(&data.dest, 80)
    );
    let start = node.source_range().map_or(0, |r| r.start);
    let web = match policy::classify_image(&data.dest) {
        ImageSource::Web(url) => Some(url),
        ImageSource::Attachment(_) | ImageSource::Other => None,
    };
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
        .tooltip(ui::tip(tip))
        .when_some(web, |line, url| {
            line.cursor_pointer().on_click(move |event, _, cx| {
                if let ClickEvent::Mouse(click) = event
                    && click.up.button != MouseButton::Left
                {
                    return;
                }
                cx.open_url(&url);
            })
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
/// numbered list one step under the body in `text.secondary`.
fn notes(owner: &SharedString, prep: &Arc<Prepared>, env: &Arc<Env>) -> Option<Div> {
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
        let note_prep = prepared(&note.markdown);
        let id = ElementId::NamedInteger(format!("{owner}/note").into(), u64::from(note.number));
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
                .child(
                    div().flex_1().min_w_0().child(
                        TextView::markdown(id, note.markdown.clone())
                            .style(env.style.clone())
                            .markdown_extensions(extensions(&note_prep, env)),
                    ),
                ),
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
