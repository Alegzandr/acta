//! The renderer conformance suite (PLAN.md, Markdown rendering and the
//! editor): every example of the CommonMark 0.31.2 spec and the GFM
//! extension examples, parsed with Acta's extensions and reduced to a
//! block / inline tree, compared with the tree of the spec's expected HTML.
//! Each difference Acta accepts is listed with its reason in
//! `testdata/accepted.md`; a listed example that starts to pass fails the
//! suite too, so the list stays true. Then a corpus of Claude Code output
//! (`testdata/corpus`) as tree snapshots: `ACTA_BLESS=1` rewrites them.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use gpui_kit::base::text::editor_host::{BlockNode, InlineNode, Paragraph, TextMark};

use super::highlight;
use super::parse::{self, CodeData, FnRefData, ImageData, ListData, PlainLinkData};

/// A block of the normalised tree.
#[derive(Clone, Debug, PartialEq)]
enum B {
    Para(Vec<I>),
    Heading(u8, Vec<I>),
    Quote(Vec<B>),
    List {
        ordered: bool,
        start: u32,
        /// Loose: blank lines between items or inside one (paragraph gaps).
        loose: bool,
        items: Vec<(Option<bool>, Vec<B>)>,
    },
    Code(Option<String>, String),
    Rule,
    /// Column alignments (`none`, `left`, `center`, `right`), then rows.
    Table(Vec<String>, Vec<Vec<Vec<I>>>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Marks {
    em: bool,
    strong: bool,
    del: bool,
    code: bool,
    /// Destination and title.
    link: Option<(String, Option<String>)>,
}

/// An inline run.
#[derive(Clone, Debug, PartialEq)]
enum I {
    Text(String, Marks),
    Break,
    Image(String, String),
}

// ---- the tree of the spec's HTML ----

#[derive(Clone, Debug)]
enum H {
    El(String, Vec<(String, String)>, Vec<H>),
    Text(String),
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|e| *e < 12).and_then(|e| {
            let name = &rest[1..e];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut rest = s.trim();
    while !rest.is_empty() {
        let name_end = rest.find(['=', ' ', '/']).unwrap_or(rest.len());
        let name = rest[..name_end].to_string();
        rest = &rest[name_end..];
        let mut value = String::new();
        if let Some(r) = rest.strip_prefix("=\"") {
            let e = r.find('"').unwrap_or(r.len());
            value = decode_entities(&r[..e]);
            rest = &r[(e + 1).min(r.len())..];
        }
        rest = rest.trim_start_matches([' ', '/']);
        if name.is_empty() {
            break;
        }
        attrs.push((name, value));
    }
    attrs
}

const KNOWN: [&str; 26] = [
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "ul",
    "ol",
    "li",
    "pre",
    "code",
    "hr",
    "table",
    "thead",
    "tbody",
    "tr",
    "th",
    "td",
    "em",
    "strong",
    "del",
    "a",
    "img",
    "br",
];

/// The spec's HTML as a tree. Its output is regular: lower-case tags,
/// quoted attributes, `/>` on void elements. Raw HTML the spec passes
/// through (any other tag) stays text, which is how Acta shows it.
fn html_tree(html: &str) -> Vec<H> {
    type Open = (String, Vec<(String, String)>, Vec<H>);
    let mut stack: Vec<Open> = vec![("root".into(), Vec::new(), Vec::new())];
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('<')
            && let Some(end) = r.find('>')
        {
            let tag = &r[..end];
            let closing = tag.strip_prefix('/');
            let body = closing.unwrap_or(tag);
            let name_end = body.find([' ', '/']).unwrap_or(body.len());
            let name = &body[..name_end];
            let known = KNOWN.contains(&name) || name == "input";
            if known {
                rest = &r[end + 1..];
                if closing.is_some() {
                    if stack.iter().skip(1).any(|(n, _, _)| n == name) {
                        loop {
                            let (n, a, c) = stack.pop().unwrap();
                            let done = n == name;
                            stack.last_mut().unwrap().2.push(H::El(n, a, c));
                            if done {
                                break;
                            }
                        }
                    }
                    continue;
                }
                let attrs = parse_attrs(&body[name_end..]);
                if matches!(name, "br" | "hr" | "img" | "input") {
                    stack
                        .last_mut()
                        .unwrap()
                        .2
                        .push(H::El(name.to_string(), attrs, Vec::new()));
                } else {
                    stack.push((name.to_string(), attrs, Vec::new()));
                }
                continue;
            }
        }
        let first = rest.chars().next().map_or(1, char::len_utf8);
        let next = rest[first..].find('<').map_or(rest.len(), |n| n + first);
        stack
            .last_mut()
            .unwrap()
            .2
            .push(H::Text(decode_entities(&rest[..next])));
        rest = &rest[next..];
    }
    while stack.len() > 1 {
        let (n, a, c) = stack.pop().unwrap();
        stack.last_mut().unwrap().2.push(H::El(n, a, c));
    }
    stack.pop().unwrap().2
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "p" | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "ul"
            | "ol"
            | "pre"
            | "hr"
            | "table"
    )
}

fn html_inlines(nodes: &[H], marks: &Marks, out: &mut Vec<I>) {
    for node in nodes {
        match node {
            H::Text(t) => out.push(I::Text(t.clone(), marks.clone())),
            H::El(name, attrs, children) => {
                let mut m = marks.clone();
                match name.as_str() {
                    "br" => {
                        out.push(I::Break);
                        continue;
                    }
                    "img" => {
                        out.push(I::Image(
                            attr(attrs, "src").unwrap_or_default().to_string(),
                            attr(attrs, "alt").unwrap_or_default().to_string(),
                        ));
                        continue;
                    }
                    "input" => continue,
                    "em" => m.em = true,
                    "strong" => m.strong = true,
                    "del" => m.del = true,
                    "code" => m.code = true,
                    "a" => {
                        m.link = Some((
                            attr(attrs, "href").unwrap_or_default().to_string(),
                            attr(attrs, "title").map(str::to_string),
                        ))
                    }
                    _ => {}
                }
                html_inlines(children, &m, out);
            }
        }
    }
}

fn html_blocks(nodes: &[H]) -> Vec<B> {
    let mut out = Vec::new();
    let mut loose: Vec<H> = Vec::new();
    let flush = |loose: &mut Vec<H>, out: &mut Vec<B>| {
        if loose.is_empty() {
            return;
        }
        let inl = inlines_of(loose);
        loose.clear();
        // An empty paragraph draws nothing.
        if !inl.is_empty() {
            out.push(B::Para(inl));
        }
    };
    for node in nodes {
        let H::El(name, attrs, children) = node else {
            loose.push(node.clone());
            continue;
        };
        if !is_block_tag(name) {
            loose.push(node.clone());
            continue;
        }
        flush(&mut loose, &mut out);
        out.push(match name.as_str() {
            "p" => {
                let inl = inlines_of(children);
                if inl.is_empty() {
                    continue;
                }
                B::Para(inl)
            }
            "blockquote" => B::Quote(html_blocks(children)),
            "hr" => B::Rule,
            "pre" => {
                let (lang, code) = match children.as_slice() {
                    [H::El(c, a, inner)] if c == "code" => (
                        attr(a, "class")
                            .and_then(|c| c.strip_prefix("language-"))
                            .map(str::to_string),
                        text_of(inner),
                    ),
                    other => (None, text_of(other)),
                };
                let code = highlight::expand_tabs(code.strip_suffix('\n').unwrap_or(&code));
                B::Code(lang, code)
            }
            "ul" | "ol" => B::List {
                ordered: name == "ol",
                start: attr(attrs, "start")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1),
                // cmark wraps the items of a loose list in paragraphs.
                loose: children.iter().any(|li| match li {
                    H::El(n, _, c) if n == "li" => {
                        c.iter().any(|x| matches!(x, H::El(p, _, _) if p == "p"))
                    }
                    _ => false,
                }),
                items: children
                    .iter()
                    .filter_map(|li| match li {
                        H::El(n, _, c) if n == "li" => {
                            let checked = c.iter().find_map(|x| match x {
                                H::El(n, a, _) if n == "input" => {
                                    Some(attr(a, "checked").is_some())
                                }
                                _ => None,
                            });
                            Some((checked, html_blocks(c)))
                        }
                        _ => None,
                    })
                    .collect(),
            },
            "table" => {
                let mut rows = Vec::new();
                let mut aligns = Vec::new();
                collect_rows(children, &mut rows, &mut aligns);
                B::Table(aligns, rows)
            }
            h => B::Heading(h[1..].parse().unwrap_or(1), inlines_of(children)),
        });
    }
    flush(&mut loose, &mut out);
    out
}

fn collect_rows(nodes: &[H], rows: &mut Vec<Vec<Vec<I>>>, aligns: &mut Vec<String>) {
    for node in nodes {
        if let H::El(name, _, children) = node {
            if name == "tr" {
                if rows.is_empty() {
                    *aligns = children
                        .iter()
                        .filter_map(|c| match c {
                            H::El(n, a, _) if n == "td" || n == "th" => {
                                Some(attr(a, "align").unwrap_or("none").to_string())
                            }
                            _ => None,
                        })
                        .collect();
                }
                rows.push(
                    children
                        .iter()
                        .filter_map(|c| match c {
                            H::El(n, _, cell) if n == "td" || n == "th" => Some(inlines_of(cell)),
                            _ => None,
                        })
                        .collect(),
                );
            } else {
                collect_rows(children, rows, aligns);
            }
        }
    }
}

fn inlines_of(nodes: &[H]) -> Vec<I> {
    let mut out = Vec::new();
    html_inlines(nodes, &Marks::default(), &mut out);
    normalise(out)
}

fn text_of(nodes: &[H]) -> String {
    let mut out = String::new();
    for n in nodes {
        match n {
            H::Text(t) => out.push_str(t),
            H::El(name, _, c) => {
                if name == "br" {
                    out.push('\n');
                }
                out.push_str(&text_of(c));
            }
        }
    }
    out
}

// ---- the tree Acta draws ----

fn ours(blocks: &[BlockNode]) -> Vec<B> {
    let mut out = Vec::new();
    for block in blocks {
        our_block(block, &mut out);
    }
    out
}

fn our_block(block: &BlockNode, out: &mut Vec<B>) {
    match block {
        BlockNode::Root { children, .. } => out.extend(ours(children)),
        BlockNode::Paragraph(p) => {
            let inl = paragraph(p);
            if !inl.is_empty() {
                out.push(B::Para(inl));
            }
        }
        BlockNode::Heading {
            level, children, ..
        } => out.push(B::Heading(*level, paragraph(children))),
        BlockNode::Blockquote { children, .. } | BlockNode::Surface { children, .. } => {
            out.push(B::Quote(ours(children)))
        }
        BlockNode::List {
            children,
            ordered,
            start,
            ..
        } => out.push(B::List {
            ordered: *ordered,
            start: start.unwrap_or(1),
            // gpui-base keeps each item's spread, not the list's.
            loose: children
                .iter()
                .any(|item| matches!(item, BlockNode::ListItem { spread: true, .. })),
            items: children
                .iter()
                .filter_map(|item| match item {
                    BlockNode::ListItem {
                        children, checked, ..
                    } => Some((*checked, ours(children))),
                    _ => None,
                })
                .collect(),
        }),
        BlockNode::CodeBlock(c) => out.push(B::Code(
            c.lang().map(|l| l.to_string()),
            c.code().to_string(),
        )),
        BlockNode::Custom { node, .. } => match node.name() {
            parse::CODE => {
                let data = node.data::<CodeData>().expect("code data");
                let lang = data
                    .label
                    .as_deref()
                    .and_then(|l| l.split_whitespace().next())
                    .map(str::to_string);
                out.push(B::Code(lang, data.code.clone()));
            }
            parse::LIST => our_block(&node.data::<ListData>().expect("list").block, out),
            parse::HIDDEN => {}
            other => panic!("unknown block {other}"),
        },
        BlockNode::Table(t) => {
            let mut rows: Vec<Vec<Vec<I>>> =
                t.rows().map(|row| row.map(paragraph).collect()).collect();
            // A short row leaves its missing cells as empty space under
            // their columns, which is what GFM's padding draws.
            let width = rows.first().map_or(0, Vec::len);
            for row in &mut rows {
                row.resize(width.max(row.len()), Vec::new());
            }
            let aligns = t
                .column_aligns()
                .iter()
                .map(|a| format!("{a:?}").to_lowercase())
                .collect();
            out.push(B::Table(aligns, rows))
        }
        BlockNode::HorizontalRule { .. } => out.push(B::Rule),
        // Drawn as nothing: a link definition, a stray break.
        BlockNode::Definition { .. } | BlockNode::Break { .. } => {}
        other => panic!("a block the suite does not know: {other:?}"),
    }
}

fn paragraph(p: &Paragraph) -> Vec<I> {
    let mut out = Vec::new();
    for node in p.children() {
        inline(node, &mut out);
    }
    normalise(out)
}

fn inline(node: &InlineNode, out: &mut Vec<I>) {
    if let Some(image) = node.as_image() {
        out.push(I::Image(
            image.url.to_string(),
            image.alt.clone().unwrap_or_default().to_string(),
        ));
        return;
    }
    let text = node.text();
    if let Some(custom) = node.as_custom() {
        let base = marks_at(node.mark_runs(), 0..text.len());
        match custom.name() {
            parse::IMAGE => {
                let d = custom.data::<ImageData>().expect("image");
                out.push(I::Image(d.dest.clone(), d.alt.clone()));
            }
            parse::CODE_SPAN => {
                let mut code = base.clone();
                code.code = true;
                out.push(I::Text(custom.as_text().to_string(), code));
            }
            parse::FNREF => {
                let n = custom.data::<FnRefData>().expect("fnref").number;
                out.push(I::Text(format!("[{n}]"), base));
            }
            // A link that does not open, drawn plain on purpose (nested
            // lists): still the link the text wrote.
            parse::PLAIN_LINK => {
                let d = custom.data::<PlainLinkData>().expect("plain link");
                let mut link = base.clone();
                link.link = Some((d.dest.clone(), None));
                out.push(I::Text(custom.as_text().to_string(), link));
            }
            parse::HTML => out.push(I::Text(custom.as_text().to_string(), base)),
            other => panic!("an inline node the suite does not know: {other}"),
        }
        return;
    }
    // Split at every mark boundary.
    let mut cuts: Vec<usize> = vec![0, text.len()];
    for (r, _) in node.mark_runs() {
        cuts.push(r.start.min(text.len()));
        cuts.push(r.end.min(text.len()));
    }
    cuts.sort_unstable();
    cuts.dedup();
    for pair in cuts.windows(2) {
        let Some(piece) = text.get(pair[0]..pair[1]) else {
            continue;
        };
        let marks = marks_at(node.mark_runs(), pair[0]..pair[1]);
        for (i, line) in piece.split('\n').enumerate() {
            if i > 0 {
                out.push(I::Break);
            }
            if !line.is_empty() {
                out.push(I::Text(line.to_string(), marks.clone()));
            }
        }
    }
}

fn marks_at(runs: &[(std::ops::Range<usize>, TextMark)], at: std::ops::Range<usize>) -> Marks {
    let mut m = Marks::default();
    for (r, mark) in runs {
        if r.start <= at.start && at.end <= r.end && r.start < r.end {
            m.em |= mark.italic;
            m.strong |= mark.bold;
            m.del |= mark.strikethrough;
            m.code |= mark.code;
            if let Some(link) = &mark.link {
                // A reference link carries its identifier; the view
                // resolves it from the text's definitions when drawing.
                m.link = Some(match (&link.identifier, link.url.is_empty()) {
                    (Some(id), true) => LINKS
                        .with(|l| l.borrow().get(id.as_ref()).cloned())
                        .unwrap_or_default(),
                    _ => (
                        link.url.to_string(),
                        link.title.as_ref().map(|t| t.to_string()),
                    ),
                });
            }
        }
    }
    m
}

thread_local! {
    /// The definitions of the text being converted: destination, title.
    static LINKS: RefCell<HashMap<String, (String, Option<String>)>> =
        RefCell::new(HashMap::new());
}

/// The tree Acta draws for `text`.
fn ours_of(text: &str) -> Vec<B> {
    let mut links = HashMap::new();
    if let Ok(root) = markdown::to_mdast(text, &super::policy::parse_options()) {
        let mut stack = vec![&root];
        while let Some(node) = stack.pop() {
            if let markdown::mdast::Node::Definition(d) = node {
                links
                    .entry(d.identifier.clone())
                    .or_insert_with(|| (d.url.clone(), d.title.clone()));
            }
            if let Some(children) = node.children() {
                stack.extend(children.iter().rev());
            }
        }
    }
    LINKS.with(|l| *l.borrow_mut() = links);
    ours(&parse::parse_tree(text))
}

// ---- normalisation of both sides ----

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whitespace as it is drawn (runs folded to one space, none at the
/// block's ends or around a break), adjacent equal runs merged, link and
/// image addresses compared percent-decoded.
fn normalise(runs: Vec<I>) -> Vec<I> {
    let mut merged: Vec<I> = Vec::new();
    for run in runs {
        match run {
            I::Text(t, mut m) => {
                m.link = m.link.map(|(url, title)| (percent_decode(&url), title));
                if m.code {
                    // A code span is drawn with its spaces as they are.
                    if let Some(I::Text(prev, pm)) = merged.last_mut()
                        && *pm == m
                    {
                        prev.push_str(&t);
                        continue;
                    }
                    merged.push(I::Text(t, m));
                    continue;
                }
                let mut folded = String::new();
                let mut space = false;
                for c in t.chars() {
                    if matches!(c, ' ' | '\n' | '\t' | '\r') {
                        space = true;
                    } else {
                        if space {
                            folded.push(' ');
                        }
                        space = false;
                        folded.push(c);
                    }
                }
                if space {
                    folded.push(' ');
                }
                if let Some(I::Text(prev, pm)) = merged.last_mut()
                    && *pm == m
                {
                    prev.push_str(&folded);
                    continue;
                }
                merged.push(I::Text(folded, m));
            }
            I::Image(src, alt) => merged.push(I::Image(percent_decode(&src), alt)),
            other => merged.push(other),
        }
    }
    let mut result: Vec<I> = Vec::new();
    for run in merged {
        if let I::Text(mut t, m) = run {
            let after_space = match result.last() {
                None | Some(I::Break) => true,
                Some(I::Text(p, pm)) => p.ends_with(' ') && !pm.code,
                _ => false,
            };
            if after_space && !m.code {
                t = t.trim_start_matches(' ').to_string();
            }
            if !t.is_empty() {
                result.push(I::Text(t, m));
            }
        } else {
            if let I::Break = run
                && let Some(I::Text(p, pm)) = result.last_mut()
                && !pm.code
            {
                let trimmed = p.trim_end_matches(' ').len();
                p.truncate(trimmed);
            }
            result.push(run);
        }
    }
    while let Some(I::Break) = result.last() {
        result.pop();
    }
    if let Some(I::Text(p, pm)) = result.last_mut()
        && !pm.code
    {
        let trimmed = p.trim_end_matches(' ').len();
        p.truncate(trimmed);
    }
    result.retain(|r| !matches!(r, I::Text(t, _) if t.is_empty()));
    result
}

// ---- the suites ----

#[derive(serde::Deserialize)]
struct Example {
    markdown: String,
    html: String,
    example: u32,
    section: String,
}

fn render(tree: &[B]) -> String {
    let mut out = String::new();
    for b in tree {
        let _ = writeln!(out, "{b:?}");
    }
    out
}

/// `testdata/accepted.md`: `- <suite> <n>: <reason>` lines.
fn accepted() -> BTreeMap<String, String> {
    include_str!("testdata/accepted.md")
        .lines()
        .filter_map(|l| l.strip_prefix("- "))
        .filter_map(|l| {
            let (key, reason) = l.split_once(": ")?;
            Some((key.trim().to_string(), reason.trim().to_string()))
        })
        .collect()
}

fn check_suite(name: &str, json: &str) {
    let examples: Vec<Example> = serde_json::from_str(json).expect("spec examples");
    let mut failing = Vec::new();
    for e in &examples {
        let expected = html_blocks(&html_tree(&e.html));
        let got = ours_of(&e.markdown);
        if expected != got {
            failing.push(format!("{name} {}", e.example));
            if std::env::var_os("ACTA_MD_DIFF").is_some() {
                eprintln!(
                    "== {name} {} ({})\n{:?}\n-- expected\n{}-- got\n{}",
                    e.example,
                    e.section,
                    e.markdown,
                    render(&expected),
                    render(&got)
                );
            }
        }
    }
    let accepted = accepted();
    let unexpected: Vec<&String> = failing
        .iter()
        .filter(|f| !accepted.contains_key(*f))
        .collect();
    let stale: Vec<&String> = accepted
        .keys()
        .filter(|k| k.starts_with(&format!("{name} ")) && !failing.contains(k))
        .collect();
    assert!(
        unexpected.is_empty() && stale.is_empty(),
        "{name}: {} of {} differ; not accepted: {unexpected:?}; accepted but now passing: {stale:?} (ACTA_MD_DIFF=1 prints the trees)",
        failing.len(),
        examples.len()
    );
}

#[test]
fn commonmark_0_31_2() {
    check_suite(
        "commonmark",
        include_str!("testdata/commonmark-0.31.2.json"),
    );
}

#[test]
fn gfm_extensions() {
    check_suite("gfm", include_str!("testdata/gfm-0.29-extensions.json"));
}

#[test]
fn every_accepted_difference_has_a_reason() {
    for (key, reason) in accepted() {
        assert!(reason.len() > 8, "{key} needs a reason");
    }
}

/// The Acta corpus: Claude Code output as tree snapshots.
#[test]
fn corpus_snapshots() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/markdown_view/testdata/corpus");
    let bless = std::env::var_os("ACTA_BLESS").is_some();
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("the corpus") {
        let path = entry.expect("an entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        seen += 1;
        let text = std::fs::read_to_string(&path)
            .expect("a text")
            .replace("\r\n", "\n");
        let got = render(&ours_of(&text));
        let snapshot = path.with_extension("tree");
        if bless {
            std::fs::write(&snapshot, &got).expect("written");
            continue;
        }
        let want = std::fs::read_to_string(&snapshot)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(got, want, "{} (ACTA_BLESS=1 rewrites it)", path.display());
    }
    assert!(seen >= 5, "the corpus has {seen} texts");
}

fn visit(blocks: &[BlockNode], f: &mut dyn FnMut(&InlineNode), codes: &mut Vec<String>) {
    for b in blocks {
        match b {
            BlockNode::Paragraph(p) => p.children().iter().for_each(&mut *f),
            BlockNode::Custom { node, .. } if node.name() == parse::CODE => {
                codes.push(node.data::<CodeData>().expect("code").code.clone())
            }
            _ => {}
        }
    }
}

/// Safety on a hostile text: nothing it holds becomes a fetched image, a
/// rendered tag or a link that opens anything but the web or e-mail.
#[test]
fn hostile_text_loads_nothing_and_opens_nothing() {
    let text = "![pixel](https://tracker.example/p.png) ![local](file:///C:/a.png) \
        ![data](data:image/png;base64,AAAA) [settings](ms-settings:privacy) \
        [files](file:///C:/Windows) [js](javascript:alert(1)) [ok](https://example.com) \
        <img src=\"https://tracker.example/q.png\"> <span style=\"color:red\">red</span> \
        a<br>b\n\n<script>alert(1)</script>\n\n<iframe src=\"https://example.com\"></iframe>\n";
    let blocks = parse::parse_tree(text);
    let (mut stock_images, mut quiet_images) = (0, 0);
    let mut links = Vec::new();
    let mut html_text = Vec::new();
    let mut breaks = 0;
    let mut codes = Vec::new();
    visit(
        &blocks,
        &mut |node| {
            if node.as_image().is_some() {
                stock_images += 1;
            }
            if node.text().contains('\n') {
                breaks += 1;
            }
            if let Some(c) = node.as_custom() {
                match c.name() {
                    parse::IMAGE => quiet_images += 1,
                    parse::HTML => html_text.push(c.as_text().to_string()),
                    _ => {}
                }
            }
            for (_, mark) in node.mark_runs() {
                if let Some(link) = &mark.link {
                    links.push(link.url.to_string());
                }
            }
        },
        &mut codes,
    );
    assert_eq!(stock_images, 0, "no image reaches the loader");
    assert_eq!(quiet_images, 3);
    assert!(
        html_text.iter().any(|t| t.starts_with("<img")),
        "{html_text:?}"
    );
    assert!(
        html_text.iter().any(|t| t.starts_with("<span")),
        "{html_text:?}"
    );
    assert!(
        !html_text.iter().any(|t| t.contains("br")),
        "<br> is a break"
    );
    assert_eq!(breaks, 1, "<br> is a break");
    assert!(codes.iter().any(|c| c.contains("<script>")), "{codes:?}");
    assert!(codes.iter().any(|c| c.contains("<iframe")), "{codes:?}");
    let opening: Vec<&String> = links
        .iter()
        .filter(|l| super::policy::classify_link(l).is_link())
        .collect();
    assert_eq!(opening, ["https://example.com"]);
}

/// Nested numbered lists count in numbers at every depth, from the start
/// number written (components-editor.md 5, audit row 4).
#[test]
fn nested_numbered_lists_keep_their_numbers() {
    let text =
        "1. One\n   1. Inner\n   2. Inner two\n\n      3. Deep\n2. Two\n\n- Bullet\n\n  4. Four\n";
    let tree = ours_of(text);
    let printed = render(&tree);
    assert!(
        printed.contains("List { ordered: true, start: 1"),
        "{printed}"
    );
    assert!(printed.contains("start: 3"), "{printed}");
    assert!(printed.contains("start: 4"), "{printed}");
    // Each nested ordered list is drawn by Acta, at depth 0 of its own.
    let blocks = parse::parse_tree(text);
    let BlockNode::List { children, .. } = &blocks[0] else {
        panic!("a list");
    };
    let BlockNode::ListItem { children, .. } = &children[0] else {
        panic!("an item");
    };
    assert!(
        children
            .iter()
            .any(|b| matches!(b, BlockNode::Custom { node, .. } if node.name() == parse::LIST))
    );
}

/// Front matter is code at the top, never a heading.
#[test]
fn front_matter_is_code() {
    let tree = ours_of("---\ntitle: Storage\nauthor: Alexandre\n---\n\n# Storage\n");
    assert_eq!(
        tree[0],
        B::Code(
            Some("yaml".into()),
            "title: Storage\nauthor: Alexandre".into()
        )
    );
    assert!(matches!(tree[1], B::Heading(1, _)));
}

/// Footnotes are numbered where they are cited and leave the flow.
#[test]
fn footnotes_are_numbered() {
    let tree = ours_of("Alexandre chose SQLCipher[^why].\n\n[^why]: It encrypts pages.\n");
    assert_eq!(tree.len(), 1, "{tree:?}");
    assert_eq!(
        tree[0],
        B::Para(vec![I::Text(
            "Alexandre chose SQLCipher[1].".into(),
            Marks::default()
        )])
    );
}

/// On a 1 MB stack (the UI thread's on Windows): texts nested past the cap
/// are caught before any parse, and a text nested well within it, every
/// level a nested numbered list parsed on its own, parses whole.
#[test]
fn nesting_stays_within_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            for deep in ["- ".repeat(1500) + "[a](b)", "> ".repeat(5000)] {
                assert!(super::policy::too_deep(&deep));
            }
            let mut text = String::new();
            for level in 0..10 {
                text.push_str(&" ".repeat(level * 3));
                text.push_str("1. level\n");
            }
            assert!(!super::policy::too_deep(&text));
            let printed = render(&ours_of(&text));
            assert_eq!(printed.matches("ordered: true").count(), 10, "{printed}");
            // Exactly as deep as is drawn, through gpui-base's own parse,
            // which the text view runs on the UI thread.
            let max = super::policy::MAX_DEPTH;
            for (deep, kind) in [
                ("- ".repeat(max) + "x", "list"),
                ("> ".repeat(max) + "x", "quote"),
                ("1. ".repeat(max) + "x", "numbered list"),
            ] {
                assert!(!super::policy::too_deep(&deep), "{kind}");
                assert!(!parse::parse_tree(&deep).is_empty(), "{kind}");
            }
        })
        .expect("a thread")
        .join()
        .expect("no overflow");
}
