//! Acta's Markdown extensions for gpui-base's parser (docs/design/
//! components-editor.md 5, Rendering contract): what is claimed from the
//! stock conversion and turned into Acta's own nodes, before anything is
//! drawn. Parsing only; `markdown_view.rs` draws the nodes.
//!
//! - inline HTML other than `<br>` stays the text it is (`acta.html`);
//!   `<br>` goes through the stock path, a line break;
//! - HTML blocks, code blocks, math blocks and front matter become one kind
//!   of code block (`acta.code`) that scrolls instead of wrapping;
//! - images become a quiet line or an attachment (`acta.image`), never a
//!   fetch;
//! - footnote references show their number (`acta.fnref`) and definitions
//!   leave the flow (`acta.hidden`), drawn together at the end;
//! - a code span that runs over a line ending keeps it as a space, as
//!   CommonMark says, where the stock path breaks the line (`acta.code-span`);
//! - a link that does not open (a path, another scheme) is its text, with
//!   its destination as tooltip (`acta.plain-link`), so no click handler
//!   ever sees it;
//! - an ordered list nested in another block is parsed on its own
//!   (`acta.list`), so it counts in numbers at every depth.

use std::cell::Cell;
use std::sync::Arc;

use gpui_kit::base::text::editor_host::{self, BlockNode};
use gpui_kit::base::text::{
    InlineElement, InlineRenderContext, MarkdownExtensions, MarkdownNode, MarkdownParseContext,
    MarkdownPlugin,
};
use gpui_kit::{App, Window};
use markdown::mdast::Node;

use super::highlight;
use super::policy::{self, Prepared};

pub const CODE: &str = "acta.code";
pub const HIDDEN: &str = "acta.hidden";
pub const LIST: &str = "acta.list";
pub const HTML: &str = "acta.html";
pub const IMAGE: &str = "acta.image";
pub const FNREF: &str = "acta.fnref";
pub const CODE_SPAN: &str = "acta.code-span";
pub const PLAIN_LINK: &str = "acta.plain-link";

/// The five GitHub alert kinds (components-editor.md 5, GitHub alert).
pub const ALERTS: [&str; 5] = ["note", "tip", "important", "warning", "caution"];

/// A block drawn as code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeData {
    /// The code, tabs expanded, without its last line ending.
    pub code: String,
    /// The info string as written, or `yaml`, `toml`, `math`, `html`.
    pub label: Option<String>,
    /// The language to highlight in, when any.
    pub lang: Option<String>,
}

/// An ordered list parsed on its own.
pub struct ListData {
    pub block: BlockNode,
}

/// An image as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageData {
    pub alt: String,
    pub dest: String,
}

/// A link drawn as plain text: what it shows and where it pointed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlainLinkData {
    pub dest: String,
}

/// A footnote reference's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FnRefData {
    pub number: u32,
}

fn code(code: &str, label: Option<String>, lang: Option<String>) -> MarkdownNode {
    let code = highlight::expand_tabs(code.strip_suffix('\n').unwrap_or(code));
    MarkdownNode::new(
        CODE,
        CodeData {
            code: code.clone(),
            label,
            lang,
        },
    )
    .text(code)
}

thread_local! {
    /// How many nested-list parses enclose the current one.
    static NESTING: Cell<usize> = const { Cell::new(0) };
}

fn block(node: &Node, cx: &MarkdownParseContext<'_>, prep: &Arc<Prepared>) -> Option<MarkdownNode> {
    let source = || cx.node_source(node).unwrap_or_default().to_string();
    let claimed = match node {
        Node::Code(c) => {
            let info = match (&c.lang, &c.meta) {
                (Some(lang), Some(meta)) => Some(format!("{lang} {meta}")),
                (Some(lang), None) => Some(lang.clone()),
                _ => None,
            };
            let lang = highlight::language(info.as_deref());
            code(&c.value, info, lang)
        }
        Node::Math(m) => code(&m.value, Some("math".into()), None),
        Node::Yaml(y) => code(&y.value, Some("yaml".into()), Some("yaml".into())),
        Node::Toml(t) => code(&t.value, Some("toml".into()), Some("toml".into())),
        Node::Html(h) => code(&h.value, Some("html".into()), Some("html".into())),
        Node::FootnoteDefinition(_) => MarkdownNode::new(HIDDEN, ()).text(""),
        Node::List(list) if list.ordered => {
            let position = node.position()?;
            let source_text = policy::nested_list_source(
                cx.source(),
                position.start.offset,
                position.end.offset,
                &prep.definitions,
            )?;
            // Each nested list parses its own nested lists: a bounded number
            // of levels, past which the stock path draws them.
            let depth = NESTING.with(Cell::get);
            if depth >= policy::MAX_DEPTH {
                return None;
            }
            NESTING.with(|n| n.set(depth + 1));
            let parsed = editor_host::parse_markdown(&source_text, Arc::new(parsers(prep, true)));
            NESTING.with(|n| n.set(depth));
            let blocks = parsed.ok()?.into_iter().next()?;
            if !matches!(blocks, BlockNode::List { .. }) {
                return None;
            }
            MarkdownNode::new(LIST, ListData { block: blocks }).text(source())
        }
        _ => return None,
    };
    Some(claimed.markdown(source()))
}

/// Inline raw HTML other than `<br>`: shown as typed.
struct HtmlText;

impl MarkdownPlugin for HtmlText {
    fn name(&self) -> &str {
        HTML
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        match node {
            Node::Html(h) if !policy::is_br(&h.value) => {
                Some(MarkdownNode::new(HTML, ()).text(h.value.clone()))
            }
            _ => None,
        }
    }

    fn render_inline(
        &self,
        _: &MarkdownNode,
        _: &InlineRenderContext,
        _: &mut Window,
        _: &mut App,
    ) -> Option<InlineElement> {
        // The atomic text fallback: the tag drawn as the text it is.
        None
    }
}

/// The inline renderer of an Acta node, set by the view.
pub type InlineRender = Arc<
    dyn Fn(&MarkdownNode, &InlineRenderContext, &mut Window, &mut App) -> Option<InlineElement>
        + Send
        + Sync,
>;

struct Image {
    prep: Arc<Prepared>,
    render: Option<InlineRender>,
}

impl MarkdownPlugin for Image {
    fn name(&self) -> &str {
        IMAGE
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let (alt, dest) = match node {
            Node::Image(i) => (i.alt.clone(), i.url.clone()),
            Node::ImageReference(i) => (
                i.alt.clone(),
                self.prep
                    .links
                    .get(&i.identifier)
                    .cloned()
                    .unwrap_or_default(),
            ),
            _ => return None,
        };
        Some(
            MarkdownNode::new(
                IMAGE,
                ImageData {
                    alt: alt.clone(),
                    dest,
                },
            )
            .text(alt.clone())
            .accessibility_label(alt),
        )
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        self.render.as_ref()?(node, context, window, cx)
    }
}

const LINE_ENDS: [char; 2] = ['\n', '\r'];

/// A code span holding a line ending: one line, the ending a space.
struct CodeSpan {
    render: Option<InlineRender>,
}

impl MarkdownPlugin for CodeSpan {
    fn name(&self) -> &str {
        CODE_SPAN
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        match node {
            Node::InlineCode(c) if c.value.contains(LINE_ENDS) => {
                let text = c.value.replace("\r\n", " ").replace(LINE_ENDS, " ");
                // CommonMark: with the line endings now spaces, one space
                // is stripped from each end when both ends have one and the
                // span is not only spaces.
                let text = match text.strip_prefix(' ').and_then(|t| t.strip_suffix(' ')) {
                    Some(inner) if !text.bytes().all(|b| b == b' ') => inner.to_string(),
                    _ => text,
                };
                Some(MarkdownNode::new(CODE_SPAN, ()).text(text))
            }
            _ => None,
        }
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        self.render.as_ref()?(node, context, window, cx)
    }
}

/// A link whose destination does not open in Acta: its text, plain.
struct PlainLink {
    prep: Arc<Prepared>,
    render: Option<InlineRender>,
}

impl MarkdownPlugin for PlainLink {
    fn name(&self) -> &str {
        PLAIN_LINK
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let dest = match node {
            Node::Link(l) => l.url.clone(),
            Node::LinkReference(l) => self.prep.links.get(&l.identifier)?.clone(),
            _ => return None,
        };
        if policy::classify_link(&dest).is_link() {
            return None;
        }
        Some(MarkdownNode::new(PLAIN_LINK, PlainLinkData { dest }).text(policy::plain_text(node)))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        self.render.as_ref()?(node, context, window, cx)
    }
}

struct FootnoteRef {
    prep: Arc<Prepared>,
    render: Option<InlineRender>,
}

impl MarkdownPlugin for FootnoteRef {
    fn name(&self) -> &str {
        FNREF
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::FootnoteReference(r) = node else {
            return None;
        };
        let number = *self.prep.footnotes.get(&r.identifier)?;
        Some(MarkdownNode::new(FNREF, FnRefData { number }).text(number.to_string()))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        self.render.as_ref()?(node, context, window, cx)
    }
}

/// The inline renderers the view gives, `None` when only parsing.
#[derive(Clone, Default)]
pub struct Renderers {
    pub image: Option<InlineRender>,
    pub fnref: Option<InlineRender>,
    pub code_span: Option<InlineRender>,
    pub plain_link: Option<InlineRender>,
}

/// The extensions of the rendering contract with the given inline
/// renderers; block renderers and alert surfaces are the view's to add.
/// `nested`: for a nested list, drawn without the text view's link hook,
/// so links that do not open are claimed as plain text at parse time.
pub fn extensions(prep: &Arc<Prepared>, renderers: Renderers, nested: bool) -> MarkdownExtensions {
    let block_prep = prep.clone();
    let ext = MarkdownExtensions::default()
        .frontmatter()
        .plugin(HtmlText)
        .plugin(Image {
            prep: prep.clone(),
            render: renderers.image,
        })
        .plugin(CodeSpan {
            render: renderers.code_span,
        })
        .plugin(FootnoteRef {
            prep: prep.clone(),
            render: renderers.fnref,
        })
        .block_parser(move |node, cx| block(node, cx, &block_prep));
    if nested {
        ext.plugin(PlainLink {
            prep: prep.clone(),
            render: renderers.plain_link,
        })
    } else {
        ext
    }
}

/// The extensions for parsing only: a nested list (`nested`), the tests.
pub fn parsers(prep: &Arc<Prepared>, nested: bool) -> MarkdownExtensions {
    with_alert_markers(extensions(prep, Renderers::default(), nested))
}

/// Registers the alert surfaces with an empty container, for a parse whose
/// tree must match the view's.
fn with_alert_markers(mut ext: MarkdownExtensions) -> MarkdownExtensions {
    for name in ALERTS {
        ext = ext.block_surface(name, |_, _| gpui_kit::div());
    }
    ext
}

/// The block tree a text parses into with Acta's extensions.
#[cfg(test)]
pub fn parse_tree(text: &str) -> Vec<BlockNode> {
    let prep = Arc::new(policy::prepare(text));
    editor_host::parse_markdown(text, Arc::new(parsers(&prep, false))).unwrap_or_default()
}
