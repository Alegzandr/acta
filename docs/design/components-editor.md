# Acta design: Markdown rendering and the editor

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 5. Components (continued): Markdown and the editor

Asked by the user on 2026-11-22: "Claude Code will write Markdown; it must be interpreted perfectly, with nothing displayed wrongly. And to make editing easy when needed, a good rich text editor: practical, well made, well integrated." This part is the contract for both. It replaces the "Rendered in v1" list and the raw-Markdown editing of typography.md 3.3, the raw editor of components.md 5 (Multi-line editor) and the "Markdown editing aids" of the document editor (components-knowledge.md 5). The setting of every element (sizes, gaps, headings per surface) stays in typography.md 3.3; this part says what is rendered, how it behaves, and how it is edited.

Three rules hold everything below:
1. **The stored text is the Markdown, byte for byte.** Nothing in Acta re-serialises it. Rendering reads it; editing changes the bytes the human changed and no others.
2. **What is shown is what was written.** An element Acta does not render is shown as typed, never dropped, never half-rendered, never fetched.
3. **Nothing in Claude Code's text reaches outside Acta by itself.** No image or link loads from the network, no HTML runs, no scheme other than the web and e-mail opens.

### Audit of the code (2026-11-22, read, not run)

Rendering today: `crates/acta-app/src/markdown_view.rs` (`short` for current state, description, entries, notes and answers; `document` for documents and memories opened in full) hands the text to gpui-kit 0.10's `TextView::markdown`, which parses with markdown-rs 1.0 (`ParseOptions::gfm()` plus math, gpui-base `text/markdown_ext.rs:472`) and converts the tree in gpui-base `text/format/markdown.rs`. Every editor is gpui-kit's plain `Textarea` (`record_page.rs:677` state and description, `:934` entry; `kb_page.rs:1135` document and memory body; `inbox_page.rs:309`, `:613` and `capture.rs:71` notes; `record_page/questions.rs:226`, `:421`). None of the Markdown editing aids, the slash menu or `Mod+B` / `Mod+I` exist in code yet (the `slash.*` keys exist in the catalogue only). Deviations, most severe first:

| # | What happens | Where | Severity | Fixed by |
|---|---|---|---|---|
| 1 | **Raw HTML is rendered.** Acta never calls `MarkdownExtensions::raw_html(false)`, so block and inline HTML go through html5ever and draw as formatted content, inline `<span style="color">` and `<mark>` included. typography.md 3.3 says HTML is not rendered. | gpui-base `format/markdown.rs:1026`, `:1346`, `:134-240` | Security and fidelity | HTML, below |
| 2 | **Images load from any URL.** `ImageNode::source` hands every non-`data:` URL to GPUI's loader: an `https` image in Claude Code's text is a network request from a local-first app (an address leak, a tracking pixel), `data:` URLs are decoded in full, `file:` and bare paths are URI-backed. 3.3 says images are not rendered. | gpui-base `text/node.rs:933-946` | Security, privacy | Images, below |
| 3 | **Links open any scheme.** No `on_link_click` is set, so the default handler calls `open_url` on left or middle click for `file:`, `ms-settings:`, `vscode:` or any registered protocol, with no check. | gpui-base `text/text_view.rs:197-214`; no handler in `crates/acta-app` | Security | Links, below |
| 4 | **Nested numbered lists change their numbers.** Depth 1 renders "A.", depth 2 "a.", whatever Claude Code wrote. | gpui-base `text/utils.rs:16-37` | Displayed wrongly | Lists, below |
| 5 | **YAML front matter becomes a heading.** Front matter is off, so `title: Storage` followed by `---` parses as a setext h2 and the opening `---` as a rule. | `markdown_ext.rs:474` | Displayed wrongly | Front matter, below |
| 6 | Footnotes: a reference shows as italic "[note]" in place, the definition as an italic paragraph where it was written; no numbering, no link. | `format/markdown.rs:1046`, `:1394` | Displayed wrongly | Footnotes, below |
| 7 | Code blocks are not highlighted (gpui-kit's `syntect` and `tree-sitter` features are off); a `$$` block and a `mermaid` fence show as unlabelled code. | `crates/acta-app/Cargo.toml:24` | Reading comfort | Code, below |
| 8 | Task checkboxes cannot be toggled (no `on_task_toggle`). This was the v1 rule; it changes here. | `markdown_view.rs` | Feature | Task lists, below |
| 9 | Inline code takes only a background; size and family come from the gpui-kit default, not `type.code`. | `markdown_view.rs:28-33` | Token drift | Element table, below |

Not yet verified (the conformance suite, Build steps, settles each): table column widths against 3.3's 320, code blocks scrolling rather than wrapping, hard breaks, setext headings, entity references, link reference definitions, and GFM autolink edge cases.

### Rendering contract

**Dialect.** CommonMark 0.31.2 plus the GFM extensions: tables, task list items, strikethrough, extended autolinks, footnotes. Parsed by markdown-rs (the parser gpui-base already uses, which passes the CommonMark and GFM spec suites) with these constructs: GFM on; front matter **on**; math text and math flow **on** (to recognise them, not to render them); raw HTML parsed (to recognise it) and drawn as text (below); MDX off. One parse per text; a text that fails to parse (markdown-rs fails only on MDX, which is off) draws as plain paragraphs split on blank lines, never as nothing.

**Per element.** Sizes in the "short" column are for current state, description, entries, notes, memories and answers (`type.reading`, 16 / 26 at the Default text size); "document" is the document setting of 3.3 (`type.document`, 17 / 28). Every value scales with Text size and zoom as 3.2 says.

| Element | Source | Rendered |
|---|---|---|
| Paragraph | text, blank line between | Body role in `text.primary`; gap 12 (short) / 16 (document). Left-aligned, ragged right, no hyphenation. A word wider than the line (a URL, a path, a hash) breaks at any character rather than overflowing. |
| Soft line break | a single newline | A space (CommonMark). Claude Code's wrapped lines read as one paragraph. |
| Hard line break | `\` or two spaces at the end of a line; `<br>` | A new line in the same paragraph, no extra gap. |
| Emphasis, strong | `*a*` `_a_`, `**a**` `__a__` | Italic of the platform sans; weight 600. Both: italic 600. |
| Strikethrough | `~~a~~` (also `~a~`) | 1px line through the middle of the x-height, text in `text.secondary`. |
| Inline code | `` `a` `` | `type.code` (13 at Default; 12 / 13 / 14 for Small / Default / Large; in a heading, the heading size minus 3), `text.primary` on `bg.sunken`, padding 0 x 4, radius 4 (`radius.sm`). Never wraps inside a word unless the span alone is wider than the line. Never linkified. |
| Heading | `#` to `######`, setext `===` / `---` | 3.3: short surfaces h1/h2 body + 2, h3 to h6 body size, all 600, 24 above and 8 below, h4 to h6 in `text.secondary`; documents h1 24, h2 body + 3 (max 22), h3 body + 1, h4 to h6 as h3 in `text.secondary`. Each heading has an anchor (Heading anchors, below). |
| Bulleted list | `-`, `*`, `+` | Marker `•` at depth 1, `◦` at depth 2, `▪` at depth 3 and deeper, in `text.tertiary`, in a 20 (short) / 24 (document) indent column per level; item gap 4 / 6. Any depth: the indent stops growing at depth 6 so text keeps 50% of the column; deeper items keep depth 6's indent and marker. Loose lists (blank lines between items) take the paragraph gap between items. |
| Numbered list | `1.` or `1)` | The number as written for the first item, then +1 per item (CommonMark: the start number counts, later numbers do not), at **every** depth: never letters, never roman. Right-aligned in the marker column with `tnum`, `text.tertiary`, followed by the delimiter as written (`.` or `)`). A start number wider than the column widens the column for that list only. |
| Task list item | `- [ ]`, `- [x]`, `- [X]` | The Markdown checkbox (Task lists, below) in the marker column; checked item text in `text.secondary`, no strikethrough (strikethrough is Markdown's own). |
| Blockquote | `>` | Indented 16 per level, text `text.secondary`, no bar, no tint (3.3). Its own lists, code and headings render inside it. |
| GitHub alert | `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]` on the quote's first line | A blockquote whose first line is the alert's word in `type.ui-strong` `text.secondary` ("Note", "Tip", "Important", "Warning", "Caution": `md.alert.*`); Warning and Caution put `triangle-alert` 14 in `warning` before the word, 6 apart. No coloured bar, no fill. An unknown `[!WORD]` is an ordinary quote, shown as typed. |
| Code block | fenced (backticks or tildes, any length) or indented | Code, below. |
| Thematic break | `---`, `***`, `___` | 1px `border.subtle`, 16 above and below (3.3), full column width. |
| Link | `[a](url)`, `[a][ref]`, `<url>` | Links, below. |
| Autolink | `https://…`, `www.…`, `name@host` in text | A link to that address (`www.` gets `https://`), same look. Trailing punctuation is excluded as GFM says. Never inside code. |
| Image | `![alt](src)` | Images, below. |
| Table | GFM pipe table | 3.3: `type.ui`, cells padded 6 x 12, header 600, 1px `border.subtle` under each row, no vertical lines, alignment from the delimiter row, digits `tnum`, a cell wraps at 320; the table scrolls horizontally inside its own block (Long content, below). Inline code, links, emphasis and `<br>` work in cells. A row with fewer cells is padded with empty cells; extra cells are dropped as GFM says. |
| Footnote | `[^1]`, `[^1]: text` | Footnotes, below. |
| HTML | block or inline tags, comments | HTML, below. |
| Math | `$…$`, `$$…$$` | Not rendered in v1. Inline `$…$` shows as typed, dollars included ("costs $5 and $10" stays prose). A `$$` block is a code block labelled "math", unhighlighted. |
| Front matter | `---` YAML `---` (or `+++` TOML) at the very top | A code block labelled "yaml" (or "toml"), highlighted, at the top. |
| Entity, escape | `&amp;`, `&#8212;`, `\*` | The character (CommonMark). Unknown entities as typed. |
| Anything else | definition lists, `^sup^`, `==mark==`, `:emoji:`, wiki links `[[a]]` | As typed, in the body role. |

**Code.** `type.code` 13 / 20 (JetBrains Mono NL, its bundled Italic where Monokai italicises, never bold) on `syntax.monokai.bg`, padding 12 x 16, `radius.md` 8, no border, in every setting (3.3: 80 columns fit the measure). Tabs are 4 columns. Lines **never wrap**: a wide line scrolls the block horizontally (Long content). Highlighting: fenced blocks whose info string names a language syntect knows (two-face's syntax set: Rust, TypeScript, JavaScript, JSON, TOML, YAML, Python, Go, C, C++, C#, Java, Kotlin, Swift, SQL, shell, HTML, CSS, Markdown, diff and the rest of that set, plus PowerShell from the MIT-licensed SublimeText/PowerShell package) are coloured in **Monokai**, the user's choice of 2026-11-25. In the dark theme the block is classic Monokai on its own dark background (`#272822`). In the light theme code is never written in night mode (the user, 2026-11-25): the block is `bg.sunken`, like inline code, and each role takes Monokai's hue darkened until it reaches 4.5:1 there (comments 3:1). Both columns of `syntax.monokai.*` are in color.md 2.1. Highlighting runs on one worker thread, off the UI thread: a block draws plain until its colours land (a moment), and a block the parser has not finished in 300 ms stays plain.

| Role | Scopes (the innermost scope that names a role decides) | Colour |
|---|---|---|
| Default | everything not below: identifiers, punctuation, `variable.language` | `syntax.monokai.fg` |
| Keyword | `keyword` (operators included), `storage` (modifiers), `entity.name.tag` | `syntax.monokai.keyword` |
| String | `string` | `syntax.monokai.string` |
| Constant | `constant` (numbers, `true`, `null`, characters), `constant.character.escape` | `syntax.monokai.constant` |
| Function | `entity.name` (functions, classes, types being defined), `entity.other.inherited-class`, `entity.other.attribute-name` | `syntax.monokai.function` |
| Type | `storage.type`, `support` (built-in types, functions, constants) | `syntax.monokai.type`, italic |
| Parameter | `variable.parameter` | `syntax.monokai.parameter`, italic |
| Comment | `comment` | `syntax.monokai.comment` |

A `diff` block (and a fence labelled `patch`) colours whole lines instead: lines starting `+` in `syntax.monokai.function`, `-` in `syntax.monokai.keyword`, `@@` hunk headers in `syntax.monokai.comment`, others `syntax.monokai.fg`, no background tints. No language, an unknown language, `mermaid`, `math`, `text`, `plaintext`: plain `syntax.monokai.fg`, no guessing from content. A block over 2,000 lines or 200 KB draws unhighlighted. Inline code is not a code block: it stays `type.code` on `bg.sunken`.

Contrast on `syntax.monokai.bg` (WCAG 2.x, color.md 2.2): in light every role reaches 4.5 (comments 4.42, over their 3.0); in dark, classic Monokai's keyword (3.93) and comment (3.03) stay below 4.5 as the user chose, every other role reaches it. The theme test checks each documented ratio in both themes.

Code block chrome: hovering the block, or keyboard focus inside it, shows in its top-right corner, over the code, on a `syntax.monokai.bg` patch with 4 padding: the info string as written (`type.small`, at most 160 wide, truncated) and a compact ghost icon button 24; in dark both in Monokai's text colours (`syntax.monokai.line-number` at rest, `syntax.monokai.fg` on hover, the scrollbar thumb tinted the same), in light in the theme's own (`text.tertiary` label, ghost icon button colours, the theme's scrollbar) (`copy` 14, tooltip "Copy code" `md.copy_code`), which copies the block's text without the fences and reads "Copied" (`common.copied`) for 1.5 s. Nothing at rest.

**Long content.** A code block or a table wider than the column scrolls horizontally **inside its own block**; the page never scrolls sideways and nothing is clipped without a way to reach it. While a block overflows, its horizontal scrollbar (5, Scrollbar) shows at rest at the bottom edge of the block, inside its padding, so the hidden part is announced without a fade or a gradient. Shift + wheel, a horizontal trackpad gesture and dragging the thumb scroll it; with keyboard focus in the block (its Copy button, or the caret while editing), `Left` / `Right` at the visible edge scroll it by 48. A table's header row stays with its columns. Paragraph text never scrolls: it wraps, breaking inside a word only when the word alone is wider than the line.

**Heading anchors.** Each heading gets the GitHub slug of its text (lower case, punctuation other than `-` and `_` removed, spaces to `-`, a repeat gets `-1`, `-2`). A link whose destination is only `#slug` scrolls that heading to the top of the page less 24, as "Go to heading" does, and is not a navigation history step. A `#slug` that matches no heading is drawn as a link whose tooltip reads "No heading “slug” on this page" (`md.anchor_missing`); activating it does nothing.

**Links.** `accent`, underlined at rest (1px, 2 under the baseline), `accent.hover` on hover, focus ring on keyboard focus (3.3). Which destinations open:

| Destination | Shown as | Activating it (click, `Enter` on focus) |
|---|---|---|
| `https:`, `http:` | A link | Opens in the system browser. No confirmation (Deceptive links, below, is the one exception). |
| `mailto:` | A link | Opens the system mail client. |
| `#slug` | A link | Heading anchors, above. |
| Relative paths and bare file names (`src/main.rs`, `../docs/a.md`, `main.rs#L10`): Claude Code links files constantly | The link text in `text.primary`, no underline, not focusable: it goes nowhere in Acta | Nothing. Tooltip: the destination as written in `type.code` at 12 (the path tooltip of 5). |
| `file:`, `javascript:`, `data:`, `vbscript:`, any other scheme (`ms-settings:`, `vscode:`, `slack:`…) | The link text in `text.primary`, no underline | Nothing. Tooltip "Not opened: Acta opens only web and e-mail links." (`md.link_blocked`) with the destination under it in `type.code` at 12. |
| A reference `[a][ref]` with no definition | As typed, brackets included (CommonMark) | n/a |

Only a left click or `Enter` opens; a middle click does nothing. The hover tooltip of an opening link is its full destination (`type.code` at 12, middle-truncated at 480), with an internationalised host shown in its ASCII (`xn--`) form, so the place a click goes is always visible before the click. Context menu on a link: "Open link" (`md.open_link`), "Copy link address" (`md.copy_link`). In the editor a plain click places the caret; `Mod`+click opens, and the tooltip adds "Ctrl+click to open" (`md.mod_click`, `Cmd` on macOS).

*Deceptive links.* When a link's visible text is itself a web address and its host differs from the destination's host (text "github.com/acta", destination `https://github.example.net/…`), activating it does not open: a popover anchored to the link (the context menu surface, 320 wide, padding 12) says "This link goes to {host}, not to the address it shows." (`md.link_mismatch`, `{host}` in `type.code`), with "Open {host}" (secondary, `md.link_open_host`) and Cancel (ghost, `Esc`). It is a popover, not a dialog, and appears only in that case: Claude Code copies text from web pages, and a page can lie about where a link goes.

**Images.** Only files attached in Acta are drawn; nothing is fetched and nothing is decoded from the text. An image whose destination is an attachment id, `![Whiteboard](A-5)`, of a file attached anywhere in the same project and not in the trash, draws that file from the encrypted store (ADR 0012), decrypted in memory, exactly as the attachment viewer does:
- **Alone in its paragraph**: a block image, as wide as its natural width up to the column, at most 480 tall, aspect kept, `radius.md`, no border; while it decodes, a `bg.sunken` box of its final size (the stored pixel size is known) so nothing reflows when it lands; no shimmer, no spinner. A click, or `Enter` on focus, opens the attachment viewer on it (components-attachments.md 5). Its alt text is its tooltip and accessible name.
- **Inside a line of text**: an inline chip, the attachment chip of components-attachments.md 5 at 24 high, showing `image` 14 and the alt text (or the file name when the alt is empty); it opens the viewer.
- **Anything else** (`https:`, `data:`, `file:`, a path, an id not found or in another project): a quiet line in place of the image, `image-off` 14 then the alt text in `text.secondary` (or "Image" `md.image` when empty), `type.small`; tooltip "Not shown: only files attached in Acta are displayed." (`md.image_blocked`), or for an id that is gone "File A-5 is not in this project." (`md.image_missing`), each with the destination under it in `type.code` at 12. For an `https:` image the line is also a link to its address (Links rules), so the person can still look at it in a browser by choice.

Claude Code learns the `![alt](A-5)` form from its skill (Build steps, integration). The human cannot type an image path that loads anything either: the rule is the renderer's, not the editor's.

**HTML.** Never rendered, never parsed as markup, never run: raw HTML is shown as the text it is, so what Claude Code wrote stays visible and nothing it copied from a web page can draw, load or style anything.
- **Inline HTML** (`<kbd>Ctrl</kbd>`, `<span style="…">`, `<!-- note -->`): the tags show as typed, in the body role, with their content between them.
- **An HTML block** (a line starting with `<div>`, `<details>`, `<table>`, `<!--`, `<script>`…, up to the blank line that ends it): a code block labelled "html", highlighted as HTML.
- **The one exception, `<br>`** (also `<br/>`, `<br />`, any case): a hard line break. Claude Code uses it in table cells, where Markdown has no other way to break a line; it can load nothing and style nothing.

**Footnotes.** A reference `[^x]` draws as its number in order of first reference ("1", "2"; the label as written is never shown), in `type.small` `accent`, `tnum`, raised 4 above the baseline, focusable; activating it scrolls its note into view (as an anchor link). Definitions are taken out of the flow and drawn together at the end of the text: a thematic break (above), then a numbered list one step under the body (`type.ui` 14 / 20 on short surfaces; 15 / 24, the Small reading size, in documents), in `text.secondary`, the numbers `tnum` in `text.tertiary`, each note ending with a `corner-up-left` 12 link back to its first reference (tooltip "Back to the text" `md.footnote_back`). A reference with no definition shows as typed. A definition never referenced stays at the end, numbered after the others.

**Task lists.** The **Markdown checkbox** is a quieter variant of the Checkbox component (5): 16 box, `radius.xs`, 1px `border.strong`; checked: fill `text.tertiary`, `check` 12 in `bg.base`, no accent, because a page of done items must not light up like a page of links (principle 2), and done work recedes. Centred on the first line of the item (top at (line height - 16) / 2). Hover: `fill.hover` inside the box, pointer cursor; keyboard focus: focus ring; pressed: `fill.pressed`. Ratios: box border on `bg.raised` / `bg.base` 3.88 / 4.30 dark, 3.55 / 3.40 light (3.0 needed); checked fill against `bg.raised` / `bg.base` 5.22 / 5.78 dark, 5.92 / 5.67 light; the check glyph `bg.base` on the `text.tertiary` fill 5.78 dark, 5.67 light (3.0 needed).

**Toggling a task.** A click on the box, or `Space` with the box focused, toggles it **wherever the text is editable** (current state, description, entries, notes, memories, documents, answers): it rewrites only the marker's one character (`[ ]` to `[x]`, `[x]` or `[X]` to `[ ]`), saves at once as an edit by the human (the meta line then reads "edited by Alexandre"; a document or memory gets a revision "Checked “Ship the installer”" / "Unchecked …", `kb.rev_checked` / `kb.rev_unchecked`), is one `Mod+Z` step, and shows no toast (the box is the feedback). This is a correction of Claude Code's text, allowed by principle 8. The toggle is applied to the revision on screen: if Claude Code changed the text since, Acta finds the item again by its line; if that line is gone or changed, nothing is toggled and the toast "Not changed: Claude Code rewrote this list a moment ago." (`md.toggle_stale`) appears, the new text being already on screen. History (a past revision) draws the boxes without hover, cursor or focus: it is read-only. A checkbox is a `Tab` stop in reading as links are; text is never a stop.

**Copying from rendered text.** `Mod+C` on a selection in rendered Markdown copies the Markdown source of the selected range (the selection maps to source through the parser's positions), so pasting into Claude Code, a note or a file keeps the formatting. The context menu adds "Copy as plain text" (`md.copy_plain`): the visible characters, list markers and numbers included.

### The editor

**Choice: live preview on the Markdown source.** The editor shows the text rendered, as in reading, except the block that holds the caret (or the blocks a selection touches), which shows its Markdown, styled. Obsidian's "live preview", at block granularity. Considered and refused:
- **WYSIWYG with a Markdown round trip** (a block or rich-text model, serialised back to Markdown on save, as Notion does): cannot keep Claude Code's bytes. Every save would rewrite `_a_` as `*a*`, renumber lists, re-pad tables, change line endings and drop what the model cannot hold (HTML, footnote placement, link reference definitions). Rule 1 forbids it, and it needs a model, a serialiser and table editing UI, the largest and riskiest piece of all three.
- **Raw Markdown only** (today's `Textarea`, with syntax decorations): safe, but it is not the "rich" editor asked for; it stays as **Source view**, the fallback and the tool for fine work (below).

Why live preview is the lowest-risk rich editor in GPUI: the source string is the only model, so the round trip is exact by construction and every command is a text edit testable without a window; rendering reuses the renderer of reading (one code path, one conformance suite); and gpui-base ships the seam it needs, `text::editor_host` (`parse_markdown` into the same block tree `TextView` draws, host-chosen leaf keys, `collect_leaf_geometry` for hit testing, caret placement and the IME's text bounds), which exists for "an editor that lives outside gpui-kit and keeps its own document model". What Acta owns is the caret, the selection, vertical movement and the platform input handler; Build steps puts those behind a model with tests before any UI.

**Where it is used.** Every field whose text is Markdown, in its existing place and box:

| Field | Setting (3.3) | Container | Files, dictation |
|---|---|---|---|
| Quick capture, inbox capture, a note being edited | short | Their own boxes (6.1 Quick capture; the inbox) | Yes |
| Description, current state, a timeline entry | short | The multi-line editor box (5) | Description and entry: files; all three: dictation |
| Memory body (edited in place) | short | The multi-line editor box | Dictation |
| Answer to a question (correction) | short | The multi-line editor box | Dictation |
| Document body | document | Borderless, the page is the editor (components-knowledge.md 5) | Dictation |

Not Markdown, unchanged: titles, summaries, the question text (one sentence), names and other single-line values, a contact's "How to address them" and address, the text-file viewer, the terminal and the chat. Principle 8 is untouched: the editor opens on text that exists, except in a note.

**Entering and leaving.** As today (5, Multi-line editor): `Enter` or `E` on the focused block, or a click on its text, opens the editor in place without moving the block; the click puts the caret at the clicked character (rendered position mapped to source), and that block shows its source. `Mod+Enter` saves, `Esc` cancels and keeps the draft (6.4), focus leaving saves where 6.4 says so. Opening and saving with no change writes nothing: no revision, no "edited by".

**What the caret's block looks like.** The **active block** is the top-level block that holds the caret, narrowed to the list item (for a list, only the item with the caret, not the whole list) or to the paragraph inside a blockquote; a table, a code block, a front matter block and an HTML block are active as a whole; a selection makes active every block it touches. The active block shows its source in the **same face and size as rendered**, so entering it barely moves anything:
- Markdown syntax characters (`#` and its space, `**`, `*`, `_`, `~~`, backticks, `>`, list markers, task brackets, the `[`, `](`, `)` of a link, a link's destination, a trailing `\`, escapes' backslashes, fences and their info string, table pipes and the delimiter row) in `text.tertiary`; the content keeps its rendered style: bold stays 600, a heading stays at its size, inline code keeps its `bg.sunken`, link text stays `accent` without underline.
- A list item keeps its rendered indent; its own leading spaces are not drawn (the indent shows them) and the caret cannot enter them; `Tab` / `Shift+Tab` change them.
- A table shows its source in `type.code` 13 / 20, so its pipes line up as Claude Code padded them; a code block shows its fences and keeps its highlighting.
- No tint, bar or outline marks the active block: the visible syntax is the mark.

Every other block is drawn exactly as in reading, inside the editor: links are not followed on a plain click, task boxes toggle (an edit of the draft, one undo step), attachment images show.

**Moving between blocks.** Arrows, `Home` / `End`, `PageUp` / `PageDown`, `Mod+Home` / `Mod+End` and clicks move through the source; the caret entering a block makes it active and the block it left renders. Vertical movement keeps the caret's horizontal pixel position (goal column) across blocks of different layout. **The viewport does not jump**: when a block above the caret changes height because it stopped (or started) showing its source, the scroll offset absorbs the difference in the same frame, so the line with the caret keeps its screen position. Selection extends across rendered and active blocks alike and is painted in `selection`; `Mod+A` selects the whole field (every block then shows its source; that is the cost of seeing exactly what is selected).

**Caret and composition.** Caret 2 wide, the line's height, `accent` (it marks "here", principle 2), blinking at the operating system's rate and solid for 500 ms after each key; solid always with reduce motion or when the OS turns blinking off. IME composition (Japanese, Chinese, Korean), dead keys (the French `^` then `e`) and macOS press-and-hold accents go through the platform input handler of GPUI with the active block's text and caret bounds from `editor_host`: the composing text is drawn inline in the active block's style with a 1px `text.primary` underline, the candidate window sits under the caret, and nothing commits to the source until the IME commits. A composition is one undo step.

**Keys inside the editor.** These win over the global map while an editor has focus (6.1); everything else keeps its global meaning (`Mod+Shift+K` is the palette everywhere, `Mod+K` is taken here).

| Key | Action |
|---|---|
| `Enter` | New paragraph (inserts a blank line). In a list item: a new item with the same marker (numbered: previous + 1; task: `- [ ] `); on an empty item: ends the list (removes the empty marker) or, when nested, outdents one level. In a blockquote: continues `> `; on an empty quote line: leaves the quote. In a code block or a front matter block: a newline with the current line's indentation. In a table row (source): a newline. |
| `Shift+Enter` | Line break in the same paragraph or item: writes a trailing `\` and a newline (the visible CommonMark form; two trailing spaces are invisible and editors strip them). |
| `Tab` / `Shift+Tab` | In a list item: indent / outdent the item (and its children) by the parent item's content offset (2 for `- `, 3 for `1. `). In a code block: insert / remove 4 spaces at the start of each selected line. Elsewhere: move focus to the next / previous control (5, Multi-line editor). |
| `Mod+B`, `Mod+I` | Bold, italic: toggle `**` / `*` around the selection, or around the word at the caret, or insert the pair with the caret between. Inside an existing span, removes it. |
| `Mod+E` | Inline code: same rule with backticks (a backtick run in the selection gets a longer fence). |
| `Mod+K` | Link: opens the link popover (below). |
| `Mod+Shift+1` / `2` / `3` | Heading 1 / 2 / 3 on the caret's block; the same key on a heading of that level makes it a paragraph. |
| `Mod+Shift+7` / `8` / `9` | Numbered list / bulleted list / task list on the selected blocks; the same key on that kind of list removes the markers. |
| `Mod+Shift+V` | Paste as plain text (below). |
| `Mod+Shift+M` | Show or hide the Markdown source of the whole field (Source view, below). |
| `Mod+Z`, `Mod+Shift+Z` / `Ctrl+Y` | Undo, redo inside the draft (below). |
| `*` `_` `` ` `` `~` typed over a selection | Wraps the selection in that character (one for `*`, `_` and `` ` ``; `~~` for `~`) instead of replacing it. With no selection they type themselves; nothing is ever auto-paired. |
| `/` | Slash menu (5), unchanged; now also lists "Text" first on a heading line. |
| `Shift+F10`, menu key | The editor's context menu: Cut, Copy, Paste, Paste as plain text, then Bold, Italic, Inline code, Link..., then Turn into: Text, Heading 1 to 3, Bulleted list, Numbered list, Task list, Quote. |

Digit keys and `.` match the physical key as 6.1 says for digits, so `Mod+Shift+7` is the same key on AZERTY. Quote and strikethrough have no direct key: the slash menu, the bubble, the context menu and the palette reach them. Every action is a palette command while an editor has focus (catalogue 10.4.21): "Bold", "Italic", "Inline code", "Strikethrough", "Link...", "Heading 1", "Heading 2", "Heading 3", "Text", "Bulleted list", "Numbered list", "Task list", "Quote", "Paste as plain text", "Show Markdown source" / "Show formatted text".

**Selection bubble.** After a selection made with the pointer is released (non-empty, inside one field), a bar appears centred above the selection's first line, 8 above it, or 8 below its last line when fewer than 44 remain above inside the page area: `bg.overlay`, 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4, height 36. Compact ghost icon buttons 28 x 28 with icons 16, 2 apart, in two groups split by a 1px `border.default` rule 16 tall with 4 either side: **Bold** (`bold`), **Italic** (`italic`), **Inline code** (`code`), **Link** (`link`) | **Turn into** (`heading`, with `chevron-down` 12: a dropdown of Text, Heading 1, 2, 3 with their keys), **Bulleted list** (`list`), **Task list** (`list-checks`), **Quote** (`text-quote`). Each has a tooltip with its name and key ("Bold  Ctrl+B"). A format already applied to the whole selection shows its button selected (`fill.selected`, icon `text.primary`); pressing it removes the format. Icons at rest `text.secondary` (6.15 dark, 7.57 light on `bg.overlay`); hover `fill.hover`; focus ring. It fades in as an overlay does (6.3) and disappears at once when the selection collapses, when a key is typed, on `Esc` (which closes only the bubble), or when the selection scrolls out of view; it follows the selection while scrolling. A keyboard selection does not raise it (shortcuts, the context menu and the palette serve the keyboard); no element of it is focusable by `Tab` from the text, so it never steals the caret.

New Lucide icons for `icons.rs`, each used only where this part says: `bold`, `italic`, `link`, `heading`, `list`, `list-checks`, `text-quote` (the bubble), `image` and `image-off` (images), `corner-up-left` (footnotes). `code`, `copy`, `chevron-down` and `triangle-alert` exist.

**Link popover.** `Mod+K` or the bubble's Link: a popover under the selection (or the caret), the context menu surface, 360 wide, padding 8. With a selection: one text input (`type.ui`, 32 high), placeholder "Paste or type a link" (`md.link_placeholder`), focused; `Enter` writes `[selection](url)`, `Esc` closes and changes nothing. With no selection: two inputs, "Text" (`md.link_text`) then the address, `Tab` between them. On an existing link: the input holds its destination, with two ghost buttons after it, "Open" (`md.link_open`) and "Remove link" (`md.link_remove`, which keeps the text). The address is checked on `Enter`: a bare host or `www.` gets `https://`; `mailto:` and `#slug` are kept; anything else (`file:`, other schemes, spaces) keeps the popover open with "Only web and e-mail links open from Acta." (`md.link_invalid`) in `type.small` `danger` under the input. A pasted URL over a selection in the text (no popover) makes the same link directly.

**Paste.** In this order:
1. `Mod+Shift+V` (or "Paste as plain text"): the clipboard's plain text, as is.
2. Files or an image and no text: attached, where the field takes files (components-attachments.md 5); elsewhere nothing happens.
3. A URL alone with a non-empty selection: a link (above).
4. Plain text only: as is. Markdown pasted from anywhere stays Markdown.
5. HTML whose text is all preformatted or monospaced (copied from a code editor or a terminal: VS Code, JetBrains IDEs, a `<pre>`): the plain text, as is.
6. Other HTML (a browser, Word, Google Docs, Notion, an e-mail): converted to Markdown, then inserted as text:

| HTML | Markdown written |
|---|---|
| `h1` to `h6` | `#` to `######` |
| `p`, `div` blocks | paragraphs |
| `br` | `\` and a newline (in a table cell: `<br>`) |
| `strong`, `b`, a style with `font-weight` 600 or more (Google Docs) | `**…**` |
| `em`, `i`, `font-style: italic` | `*…*` |
| `s`, `del`, `strike` | `~~…~~` |
| `code` | backticks, fenced longer than any backtick run inside |
| `pre` | a fenced code block; language from a `language-x` or `lang-x` class |
| `a` with `http`, `https` or `mailto` | `[text](url)`; any other `href`: the text alone |
| `ul`, `ol`, `li` (nested), `li` holding a checkbox | `- `, `1. ` (the `start` kept), indented by the parent's content offset; `- [ ] ` / `- [x] ` |
| `blockquote` | `> ` |
| `table` | a GFM table, first row as header when there is no `thead`; `|` in cells escaped `\|`, line breaks as `<br>` |
| `hr` | `---` |
| `img` | dropped; once per paste, the toast "Images in the pasted text were left out. Paste or drop them as files." (`md.paste_images_dropped`) |
| `script`, `style`, `head`, comments, forms, `iframe` | dropped with their content |
| everything else (`span`, `font`, `section`…) | its text |

Text is escaped only where it would otherwise turn into Markdown (`*`, `_`, `` ` ``, `[`, `]`, `<`, and `#`, `-`, `+`, `1.`, `>` at a line start). Whitespace collapses as HTML does. HTML over 1 MB, or a conversion that yields nothing, pastes the plain text instead. Inside a code block, inline code, or the source of a table, every paste is plain text (in a table cell its newlines become spaces). A paste is one undo step; the caret ends after the pasted text.

**Undo.** Inside the editor, `Mod+Z` undoes the draft's own history: a run of typing is one step until 1 s without a key, a caret jump, or a change of kind (typing, deleting); every command (format, list key, slash insertion, paste, a task toggle, a dictation take, a file attached) is a step of its own. 500 steps per editor session. When the draft's history is empty `Mod+Z` does nothing (it never reaches past the editor to undo a project write behind the person's back). After saving, the whole edit is one project step of 6.4, as today. Redo mirrors it; a new edit clears the redo of the draft.

**Round-trip guarantee.** What Claude Code wrote is preserved byte for byte unless the human edits it:
- The draft is the stored string; each edit replaces one byte range. Bytes outside the edited ranges are never touched: emphasis characters, list markers and numbering, table padding, trailing whitespace, tabs, blank lines, line endings, HTML, link reference definitions and unknown syntax stay as written.
- Nothing is normalised on open, on save, or on render. No renumbering of later list items (the rendered numbers follow CommonMark anyway), no reflow, no trimming.
- New lines use the text's dominant line ending (LF unless the text is mostly CRLF). Commands write the canonical forms (`**`, `*`, backticks, `- `, `1. `, `- [ ] `, `> `, `# `), except that continuing a list copies the marker and delimiter of the item above (`*`, `+`, `1)`).
- Save with no change writes nothing. A task toggle changes one byte.

**Source view.** `Mod+Shift+M` (or "Show Markdown source") shows the whole field as its source with the active block's styling everywhere (syntax characters in `text.tertiary`, content styled, tables and code in their mono): the old raw editor, kept for fine work on a big table or nested lists, and the fallback while live preview is not built. The choice is remembered for all editors (`editor_source` in `preferences.json`, a boolean, no content; layout.md 4.7). Every key above works the same in both.

**Spellcheck.** Not in v1. GPUI exposes no spell checker; Windows (`ISpellChecker`) and macOS (`NSSpellChecker`) have one each and Linux none, so it is a separate piece of platform work, to be specified with the user when asked for. Nothing in this part prevents it: it would decorate source ranges as the IME underline does.

**Large texts.** Budgets on top of 6.3, measured in a release build on a mid-range laptop with a 100 KB document (about 2,000 blocks):

| Interaction | Budget |
|---|---|
| A key typed to the frame showing it | next frame (16 ms); at most 4 ms of it in the editor's own work |
| Caret entering another block (it renders, the new one shows its source) | next frame |
| Open a 1 MB document in reading or in the editor | 100 ms to the first screen; only blocks in view, plus one screen above and below, are laid out |
| Paste converting 200 KB of HTML | 50 ms; over that the conversion runs off the UI thread and the text lands when ready (the caret is held) |
| Highlighting a code block | off the UI thread, cached by language and text; the block draws plain until it lands, then once in colour (not an animation) |

To meet them: an edit reparses only the active block and its neighbours (a block-level edit can merge or split blocks; the parse of a window of blocks says so); the whole text is reparsed off the UI thread 150 ms after the last key, and its result replaces the block list only if the draft has not changed meanwhile. Layout is cached per block by its text and width.

**Empty and error states.**
- An empty note editor shows its placeholder ("Note for Acta") in `text.tertiary`; a correction editor is never empty when it opens (principle 8); emptied by the person, it shows no placeholder.
- Claude Code changing the text while the person edits: 6.5, rule 4, unchanged (the notice, Show theirs, Keep mine).
- A save that fails: 6.4 (the text goes back into the editor).
- An attachment image that cannot be decoded: the "Image" line above with the tooltip "This file cannot be shown as an image." (`md.image_undecodable`).
- A highlighter that fails on a block: the block draws plain; nothing is reported.

### Wireframes

A document being edited: the caret is in the second paragraph, which shows its source; everything else is rendered. `·` marks `text.tertiary` syntax.

```
   Storage                                                              h1 24, rendered
   ---------------------------------------------------------------
   The database is one SQLCipher file in the data directory.           rendered paragraph
   ·**·Keys never touch the disk·**·: they live in the keychain,       active block: source,
   see ·[·ADR 0003·](·https://example.org/adr-3·)·.|                    bold kept 600, caret |
   • Snapshots before every migration                                   rendered list
   • Daily snapshot, 7 kept
   [x] Export round-trip test                                           Markdown checkbox, text.secondary
   [ ] Import preview
   +-------------------------------------------------------------+
   | fn open(path: &Path) -> Result<Db> {                        |      code block, bg.sunken
   |     let key = keychain::get("acta")?;   // never on disk    |      keyword/string/comment roles
   +-------------------------------------------------------------+
  +------------------------------------------------------------------+
  | Editing · draft saved                          Cancel  [ Save ]  |  44 bar (components-knowledge.md 5)
  +------------------------------------------------------------------+
```

The selection bubble over a pointer selection in an entry being edited:

```
              +---------------------------------------+
              | [B] [I] [<>] [ln] | [H v] [:=] [v=] ["] |     36 high, bg.overlay, border.default
              +---------------------------------------+
                          8
   Moved the watcher to [a background thread] so the UI            selection token
   never waits on disk.
```

The link popover on an existing link, and a refused address:

```
   see [ADR 0003] for the key handling.
       +--------------------------------------------------+
       | https://example.org/adr-3       Open  Remove link |   input 32, two ghost buttons
       +--------------------------------------------------+

       +--------------------------------------------------+
       | file:///C:/Users/Alexandre/key.txt               |   outline danger
       | Only web and e-mail links open from Acta.        |   type.small danger
       +--------------------------------------------------+
```

A wide table on a short surface, scrolled, and an image Acta does not load:

```
   | Step      | Owner     | Due         | Notes                  |→    scrolls inside the block
   |-----------|-----------|-------------|------------------------|
   | Installer | Alexandre | 15 Dec 2026 | Silent, per-user       |
   ====================                                                  scrollbar shown while it overflows

   [image-off] Architecture diagram                                    type.small text.secondary
               (tooltip: Not shown: only files attached in Acta are displayed.
                         https://example.org/diagram.png)
```

A code block hovered:

```
   +-------------------------------------------------------------+
   | let total = items.len();                         rust  [cp] |   label + Copy over the code,
   | println!("{total} items");                                  |   on a bg.sunken patch
   +-------------------------------------------------------------+
```
