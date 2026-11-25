//! What rendered Markdown may do (docs/design/components-editor.md 5,
//! Rendering contract): which link destinations open, which images draw,
//! which HTML is a line break, heading anchors, and the per-text facts the
//! renderer needs before it draws (footnote numbers, link definitions).
//! Pure logic, no GPUI: the view in `markdown_view.rs` asks, this answers.

use std::collections::HashMap;

use markdown::mdast::Node;

/// Where activating a link goes (components-editor.md 5, Links).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// `http:` or `https:` with a host: the system browser. `host` is in its
    /// ASCII form (`xn--` for an internationalised name), `url` normalised.
    Web { url: String, host: String },
    /// `mailto:`: the system mail client.
    Mail(String),
    /// `#slug`: a heading of the same text.
    Anchor(String),
    /// A relative path or bare file name: goes nowhere in Acta.
    Inert,
    /// Any other scheme (`file:`, `javascript:`, `data:`, `ms-settings:`...),
    /// or a web address without a host: never opened.
    Blocked,
}

impl LinkTarget {
    /// Whether the link is drawn as a link (underlined, focusable).
    pub fn is_link(&self) -> bool {
        matches!(
            self,
            LinkTarget::Web { .. } | LinkTarget::Mail(_) | LinkTarget::Anchor(_)
        )
    }
}

/// The scheme of `dest` in lower case, when it has one. A single letter
/// before the colon is a Windows drive (`C:\a`), not a scheme.
fn scheme(dest: &str) -> Option<String> {
    let colon = dest.find(':')?;
    let name = &dest[..colon];
    let mut chars = name.chars();
    let first = chars.next()?;
    let valid = first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    (valid && name.len() > 1).then(|| name.to_ascii_lowercase())
}

/// Classifies a link destination as written in the Markdown.
pub fn classify_link(dest: &str) -> LinkTarget {
    let dest = dest.trim();
    if let Some(slug) = dest.strip_prefix('#') {
        return LinkTarget::Anchor(slug.to_string());
    }
    match scheme(dest).as_deref() {
        None => LinkTarget::Inert,
        Some("http" | "https") => match url::Url::parse(dest) {
            Ok(url) => match url.host_str() {
                Some(host) if !host.is_empty() => LinkTarget::Web {
                    host: host.to_string(),
                    url: url.to_string(),
                },
                _ => LinkTarget::Blocked,
            },
            Err(_) => LinkTarget::Blocked,
        },
        Some("mailto") => LinkTarget::Mail(dest.to_string()),
        Some(_) => LinkTarget::Blocked,
    }
}

/// Endings of file names that are also top-level domains (`README.md`,
/// `main.rs`, `setup.sh`): a bare name ending in one is a file, not a host.
const FILE_EXTENSIONS: [&str; 40] = [
    "md", "rs", "sh", "py", "js", "ts", "go", "rb", "pl", "cs", "fs", "kt", "so", "ps", "cc",
    "cpp", "hpp", "toml", "yaml", "yml", "json", "lock", "txt", "log", "csv", "xml", "html", "css",
    "sql", "zip", "png", "jpg", "gif", "svg", "pdf", "exe", "dll", "bin", "ini", "bat",
];

/// The host a visible link text names, when the text is itself a web
/// address (`https://github.com/a`, `www.github.com`, `github.com/acta`,
/// `example.org`). Trailing punctuation is not part of it; a bare name
/// ending in a file extension is a file.
fn host_in_text(text: &str) -> Option<String> {
    let text = text
        .trim()
        .trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '\'', '"']);
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = text.to_lowercase();
    let schemed = lower.starts_with("https://") || lower.starts_with("http://");
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let has_path = host_end < rest.len();
    let authority = &rest[..host_end];
    // `github.com@evil.example` goes to evil.example.
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host);
    let labels: Vec<&str> = host.split('.').collect();
    let tld = labels.last()?;
    let looks_like_host = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || !c.is_ascii())
        })
        && tld.chars().count() >= 2
        && tld.chars().all(|c| c.is_alphabetic() || !c.is_ascii());
    let a_file =
        !schemed && !has_path && !host.starts_with("www.") && FILE_EXTENSIONS.contains(tld);
    if !looks_like_host || a_file {
        return None;
    }
    // The ASCII form, so "gіthub.com" with a Cyrillic i does not match.
    let ascii = url::Url::parse(&format!("https://{host}/"))
        .ok()?
        .host_str()?
        .to_string();
    Some(ascii)
}

fn without_www(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

/// Deceptive links (components-editor.md 5): the destination host to warn
/// about when a web link's visible text is itself an address on another
/// host, or when the destination hides its host behind a user name
/// (`https://github.com@evil.example/`).
pub fn deceptive_host(visible: &str, dest: &str) -> Option<String> {
    let LinkTarget::Web { host, .. } = classify_link(dest) else {
        return None;
    };
    let userinfo = url::Url::parse(dest.trim())
        .is_ok_and(|url| !url.username().is_empty() || url.password().is_some());
    if userinfo {
        return Some(host);
    }
    let shown = host_in_text(visible)?;
    (without_www(&shown) != without_www(&host)).then_some(host)
}

/// The tooltip of an opening link: its destination, middle-truncated to
/// `max` characters.
pub fn middle_truncate(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max || max < 3 {
        return text.to_string();
    }
    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    format!("{start}\u{2026}{end}")
}

/// What an image destination draws (components-editor.md 5, Images).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSource {
    /// `A-5`: a file attached in Acta.
    Attachment(i64),
    /// `http:` / `https:`: never fetched; the quiet line links to it.
    Web(String),
    /// Anything else: a path, `data:`, `file:`...
    Other,
}

pub fn classify_image(dest: &str) -> ImageSource {
    let dest = dest.trim();
    if let Some(n) = dest
        .strip_prefix("A-")
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && !n.starts_with('0'))
        .and_then(|n| n.parse::<i64>().ok())
    {
        return ImageSource::Attachment(n);
    }
    match classify_link(dest) {
        LinkTarget::Web { url, .. } => ImageSource::Web(url),
        _ => ImageSource::Other,
    }
}

/// Whether raw inline HTML is the one tag drawn as what it means: `<br>`,
/// `<br/>`, `<br />`, any case (components-editor.md 5, HTML).
pub fn is_br(tag: &str) -> bool {
    let Some(inner) = tag
        .trim()
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
    else {
        return false;
    };
    let inner = inner.trim_end();
    let inner = inner.strip_suffix('/').unwrap_or(inner).trim_end();
    inner.eq_ignore_ascii_case("br")
}

/// The deepest nesting of containers (quotes, list items) drawn as
/// Markdown; a deeper text draws as plain text, so neither the parser nor
/// the renderer recurses without bound.
pub const MAX_DEPTH: usize = 64;

/// Whether some line of `text` opens more nested containers than
/// [`MAX_DEPTH`]: each `>` and list marker counts one, and so do two
/// columns of indent before them.
pub fn too_deep(text: &str) -> bool {
    text.lines().any(|line| {
        let bytes = line.as_bytes();
        let (mut depth, mut i, mut spaces) = (0usize, 0usize, 0usize);
        while i < bytes.len() {
            match bytes[i] {
                b' ' => spaces += 1,
                b'\t' => spaces += 4,
                b'>' => depth += 1,
                b'-' | b'*' | b'+' if bytes.get(i + 1).is_none_or(|b| *b == b' ') => depth += 1,
                b'0'..=b'9' => {
                    let digits = bytes[i..].iter().take_while(|b| b.is_ascii_digit()).count();
                    match bytes.get(i + digits) {
                        Some(b'.' | b')') if digits <= 9 => {
                            depth += 1;
                            i += digits;
                        }
                        _ => break,
                    }
                }
                _ => break,
            }
            i += 1;
        }
        // Indent alone nests nothing; before a marker it stands for the
        // levels above it.
        depth > 0 && depth + spaces / 2 > MAX_DEPTH
    })
}

/// The parse options of the rendering contract: GFM, front matter, math
/// recognised; the same as gpui-base's with `frontmatter()`.
pub fn parse_options() -> markdown::ParseOptions {
    let mut options = markdown::ParseOptions::gfm();
    options.constructs.frontmatter = true;
    options.constructs.math_text = true;
    options.constructs.math_flow = true;
    options
}

/// One footnote definition, as drawn at the end of the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub number: u32,
    /// Its Markdown, the `[^x]:` label and continuation indent removed.
    pub markdown: String,
    /// The facts of its own Markdown (its links), prepared with the text.
    pub prepared: Box<Prepared>,
}

/// What the renderer needs to know about a whole text before it draws a
/// part of it: footnote numbers (in order of first reference, then the
/// definitions never referenced) and the link reference definitions, which
/// a nested list parsed on its own must still see.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prepared {
    /// Footnote identifier (as markdown-rs normalises it) to its number.
    pub footnotes: HashMap<String, u32>,
    /// The notes, by number.
    pub notes: Vec<Note>,
    /// The source of every link reference definition and footnote
    /// definition, which a nested list parsed on its own must still see.
    pub definitions: String,
    /// Link reference identifier (normalised) to its destination, for
    /// image references.
    pub links: HashMap<String, String>,
    /// Destinations some link of the text disguises (its visible text is an
    /// address on another host), to the host they really go to.
    pub deceptive: HashMap<String, String>,
}

/// Reads the facts of [`Prepared`] from `text`. Cheap when the text has no
/// `[` (no footnote, no definition).
pub fn prepare(text: &str) -> Prepared {
    prepare_at(text, 0)
}

fn prepare_at(text: &str, depth: usize) -> Prepared {
    let mut prepared = Prepared::default();
    if !text.contains('[') || too_deep(text) {
        return prepared;
    }
    let Ok(root) = markdown::to_mdast(text, &parse_options()) else {
        return prepared;
    };
    let mut references = Vec::new();
    let mut definitions = Vec::new();
    let mut shown: Vec<(String, String)> = Vec::new();
    let mut shown_refs: Vec<(String, String)> = Vec::new();
    walk(&root, &mut |node| match node {
        Node::FootnoteReference(r) => references.push(r.identifier.clone()),
        Node::FootnoteDefinition(d) => {
            definitions.push((d.identifier.clone(), footnote_markdown(text, d)));
            push_source(&mut prepared.definitions, text, node);
        }
        Node::Image(i) => shown.push((i.alt.clone(), i.url.clone())),
        Node::Link(l) => shown.push((plain_text(node), l.url.clone())),
        Node::LinkReference(l) => shown_refs.push((plain_text(node), l.identifier.clone())),
        Node::Definition(d) => {
            prepared
                .links
                .entry(d.identifier.clone())
                .or_insert_with(|| d.url.clone());
            push_source(&mut prepared.definitions, text, node);
        }
        _ => {}
    });
    for (text, id) in shown_refs {
        if let Some(url) = prepared.links.get(&id) {
            shown.push((text, url.clone()));
        }
    }
    for (text, url) in shown {
        if let Some(host) = deceptive_host(&text, &url) {
            prepared.deceptive.insert(url, host);
        }
    }
    let mut next = 1;
    for id in references {
        if definitions.iter().any(|(d, _)| *d == id) && !prepared.footnotes.contains_key(&id) {
            prepared.footnotes.insert(id, next);
            next += 1;
        }
    }
    let mut seen = std::collections::HashSet::new();
    for (id, markdown) in definitions {
        if !seen.insert(id.clone()) {
            continue; // a repeated definition: the first one counts
        }
        let number = *prepared.footnotes.entry(id).or_insert_with(|| {
            next += 1;
            next - 1
        });
        // A note's own notes are not drawn: one level is prepared.
        let note = if depth == 0 {
            prepare_at(&markdown, 1)
        } else {
            Prepared::default()
        };
        prepared.notes.push(Note {
            number,
            markdown,
            prepared: Box::new(note),
        });
    }
    prepared.notes.sort_by_key(|n| n.number);
    prepared
}

/// Appends a definition's source, then a blank line.
fn push_source(out: &mut String, text: &str, node: &Node) {
    if let Some(source) = node
        .position()
        .and_then(|p| text.get(p.start.offset..p.end.offset))
    {
        out.push_str(source.trim_end());
        out.push_str("\n\n");
    }
}

/// The text a node shows, without its markup.
pub fn plain_text(node: &Node) -> String {
    let mut out = String::new();
    walk(node, &mut |n| match n {
        Node::Text(t) => out.push_str(&t.value),
        Node::InlineCode(c) => out.push_str(&c.value),
        _ => {}
    });
    out
}

/// Visits `node` and its descendants in document order, without
/// recursion: a deeply nested text cannot exhaust the stack.
fn walk(node: &Node, f: &mut impl FnMut(&Node)) {
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        f(node);
        if let Some(children) = node.children() {
            stack.extend(children.iter().rev());
        }
    }
}

/// A definition's content: its children's source, continuation lines
/// dedented by the four columns GFM gives a footnote.
fn footnote_markdown(text: &str, def: &markdown::mdast::FootnoteDefinition) -> String {
    let (Some(first), Some(last)) = (def.children.first(), def.children.last()) else {
        return String::new();
    };
    let (Some(start), Some(end)) = (first.position(), last.position()) else {
        return String::new();
    };
    let Some(source) = text.get(start.start.offset..end.end.offset) else {
        return String::new();
    };
    dedent(source, 4).unwrap_or_else(|| source.to_string())
}

/// Removes up to `columns` leading spaces from every line but the first.
/// `None` when a continuation line has a tab or a `>` inside that indent,
/// whose columns a plain cut would get wrong.
pub fn dedent(source: &str, columns: usize) -> Option<String> {
    let mut out = String::with_capacity(source.len());
    for (i, line) in source.split_inclusive('\n').enumerate() {
        if i == 0 {
            out.push_str(line);
            continue;
        }
        let spaces = line
            .bytes()
            .take(columns)
            .take_while(|b| *b == b' ')
            .count();
        let rest = &line[spaces..];
        if spaces < columns && (rest.starts_with('\t') || rest.starts_with('>')) {
            return None;
        }
        out.push_str(rest);
    }
    Some(out)
}

/// For an ordered list inside another block, the source to parse it alone
/// with: the list's own lines dedented by its column, then the text's link
/// definitions. `None` when the list does not stand on an indented line of
/// its own (inside a blockquote, after a tab), where it keeps the stock
/// rendering.
pub fn nested_list_source(
    full: &str,
    start: usize,
    end: usize,
    definitions: &str,
) -> Option<String> {
    let line_start = full[..start].rfind('\n').map_or(0, |i| i + 1);
    let prefix = &full[line_start..start];
    if prefix.is_empty() || !prefix.bytes().all(|b| b == b' ') {
        return None;
    }
    let mut source = dedent(full.get(start..end)?, prefix.len())?;
    if !definitions.is_empty() {
        source.push_str("\n\n");
        source.push_str(definitions);
    }
    Some(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_and_mail_open_everything_else_does_not() {
        assert_eq!(
            classify_link("https://example.com/a?b#c"),
            LinkTarget::Web {
                url: "https://example.com/a?b#c".into(),
                host: "example.com".into()
            }
        );
        assert!(matches!(
            classify_link("HTTP://Example.com"),
            LinkTarget::Web { .. }
        ));
        assert_eq!(
            classify_link("mailto:alexandre@example.com"),
            LinkTarget::Mail("mailto:alexandre@example.com".into())
        );
        assert_eq!(
            classify_link("#storage-layer"),
            LinkTarget::Anchor("storage-layer".into())
        );
        for inert in [
            "src/main.rs",
            "../docs/a.md",
            "main.rs#L10",
            "C:\\Users\\a.txt",
            "//host/x",
        ] {
            assert_eq!(classify_link(inert), LinkTarget::Inert, "{inert}");
        }
        for blocked in [
            "file:///C:/Windows/system32",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<b>x</b>",
            "vbscript:x",
            "ms-settings:privacy",
            "vscode://file/a",
            "slack://open",
            "https://",
        ] {
            assert_eq!(classify_link(blocked), LinkTarget::Blocked, "{blocked}");
        }
    }

    #[test]
    fn internationalised_hosts_show_in_ascii() {
        let LinkTarget::Web { host, .. } = classify_link("https://bücher.example/x") else {
            panic!("a web link");
        };
        assert_eq!(host, "xn--bcher-kva.example");
    }

    #[test]
    fn a_link_lying_about_its_host_is_caught() {
        assert_eq!(
            deceptive_host("github.com/acta", "https://github.example.net/acta"),
            Some("github.example.net".into())
        );
        assert_eq!(
            deceptive_host("https://github.com", "https://evil.example/"),
            Some("evil.example".into())
        );
        // The same host, with or without www, or text that is no address.
        assert_eq!(
            deceptive_host("github.com/acta", "https://github.com/acta"),
            None
        );
        assert_eq!(
            deceptive_host("www.github.com", "https://github.com/"),
            None
        );
        assert_eq!(deceptive_host("the docs", "https://evil.example/"), None);
        assert_eq!(deceptive_host("README.md", "https://evil.example/"), None);
        assert_eq!(deceptive_host("v1.2", "https://evil.example/"), None);
        // Trailing punctuation, any real ending, a user name in front.
        assert_eq!(
            deceptive_host("example.org.", "https://evil.example/"),
            Some("evil.example".into())
        );
        assert_eq!(
            deceptive_host("acta.dev)", "https://evil.example/"),
            Some("evil.example".into())
        );
        assert_eq!(deceptive_host("main.rs", "https://evil.example/"), None);
        assert_eq!(
            deceptive_host("docs", "https://github.com@evil.example/"),
            Some("evil.example".into())
        );
        assert_eq!(
            deceptive_host("github.com@evil.example", "https://github.com/"),
            Some("github.com".into())
        );
        // A lookalike host in the text never equals the real one.
        assert!(deceptive_host("gіthub.com", "https://github.com/").is_some());
        // Only web destinations are checked.
        assert_eq!(deceptive_host("github.com", "mailto:a@b.c"), None);
    }

    #[test]
    fn long_destinations_are_cut_in_the_middle() {
        assert_eq!(middle_truncate("abcdef", 10), "abcdef");
        assert_eq!(middle_truncate("abcdefghij", 5), "ab\u{2026}ij");
    }

    #[test]
    fn only_attachment_ids_are_images() {
        assert_eq!(classify_image("A-5"), ImageSource::Attachment(5));
        assert_eq!(classify_image(" A-12 "), ImageSource::Attachment(12));
        assert_eq!(
            classify_image("https://example.com/p.png"),
            ImageSource::Web("https://example.com/p.png".into())
        );
        for other in [
            "A-0",
            "A-",
            "a-5",
            "A-5x",
            "data:image/png;base64,AA==",
            "file:///p.png",
            "p.png",
        ] {
            assert_eq!(classify_image(other), ImageSource::Other, "{other}");
        }
    }

    #[test]
    fn br_in_every_spelling_and_nothing_else() {
        for br in ["<br>", "<BR>", "<br/>", "<br />", "<Br  />", " <br> "] {
            assert!(is_br(br), "{br}");
        }
        for not in ["<b>", "<br class=\"x\">", "</br>", "<brx>", "br", "<img>"] {
            assert!(!is_br(not), "{not}");
        }
    }

    #[test]
    fn footnotes_number_by_first_reference() {
        let text = "B[^b] then A[^a] and B again[^b].\n\n[^a]: Note A.\n[^b]: Note B\n    continued.\n[^c]: Never cited.\n";
        let p = prepare(text);
        assert_eq!(p.footnotes["b"], 1);
        assert_eq!(p.footnotes["a"], 2);
        assert_eq!(p.footnotes["c"], 3);
        let notes: Vec<_> = p
            .notes
            .iter()
            .map(|n| (n.number, n.markdown.as_str()))
            .collect();
        assert_eq!(
            notes,
            [
                (1, "Note B\ncontinued."),
                (2, "Note A."),
                (3, "Never cited.")
            ]
        );
    }

    #[test]
    fn definitions_are_collected_for_nested_lists() {
        let p = prepare("See [the plan][plan].\n\n[plan]: https://example.com/plan \"Plan\"\n");
        assert_eq!(
            p.definitions,
            "[plan]: https://example.com/plan \"Plan\"\n\n"
        );
        // Footnote definitions too, for a nested list's own parse.
        let p = prepare("Text[^n].\n\n[^n]: The note.\n");
        assert_eq!(p.definitions, "[^n]: The note.\n\n");
        assert_eq!(prepare("no brackets").definitions, "");
    }

    #[test]
    fn disguised_links_are_found_before_drawing() {
        let p = prepare(
            "[github.com/acta](https://github.example.net/acta) and [docs](https://docs.example.com) \
             and [ref][r]\n\n[r]: https://evil.example/\n",
        );
        assert_eq!(
            p.deceptive
                .get("https://github.example.net/acta")
                .map(String::as_str),
            Some("github.example.net")
        );
        assert!(!p.deceptive.contains_key("https://docs.example.com"));
        assert_eq!(p.links["r"], "https://evil.example/");
        let p = prepare("[www.example.com][r]\n\n[r]: https://evil.example/\n");
        assert!(p.deceptive.contains_key("https://evil.example/"));
    }

    #[test]
    fn deep_texts_are_caught_before_parsing() {
        assert!(!too_deep("- a\n  - b\n    - c\n> > quoted\n"));
        assert!(too_deep(&"- ".repeat(1500)));
        assert!(too_deep(&"> ".repeat(5000)));
        assert!(too_deep(&format!("{}- x", " ".repeat(200))));
        assert!(!too_deep(&format!("{}x", " ".repeat(200))));
        assert!(!too_deep("1. 2. 3. a list of three"));
    }

    /// Deep texts on a 1 MB stack (the UI thread's on Windows): preparing
    /// them neither recurses nor overflows.
    #[test]
    fn deep_texts_prepare_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                for text in [
                    "- ".repeat(1500) + "[a](b)",
                    "> ".repeat(5000),
                    "[x](https://example.com) ".repeat(20_000),
                ] {
                    let _ = prepare(&text);
                    let mut count = 0;
                    if let Ok(root) = markdown::to_mdast("[a](b)", &parse_options()) {
                        walk(&root, &mut |_| count += 1);
                    }
                    assert!(count > 0);
                }
            })
            .expect("a thread")
            .join()
            .expect("no overflow");
    }

    #[test]
    fn nested_list_source_is_dedented() {
        let full = "1. One\n   1. Inner\n      lazy\n   2. Two\n";
        let start = full.find("1. Inner").unwrap();
        let end = full.len() - 1;
        assert_eq!(
            nested_list_source(full, start, end, "").unwrap(),
            "1. Inner\n   lazy\n2. Two"
        );
        // A top-level list or one inside a blockquote keeps the stock path.
        assert_eq!(nested_list_source(full, 0, 6, ""), None);
        let quoted = "> 1. One\n>    1. Inner\n";
        let start = quoted.find("1. Inner").unwrap();
        assert_eq!(nested_list_source(quoted, start, quoted.len(), ""), None);
    }
}
