# Acta design: typography

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 3. Typography

### 3.1 Families

| Role | Family | Source | License |
|---|---|---|---|
| UI and reading, Windows | **Segoe UI Variable Text** (falls back to Segoe UI on Windows 10, which is metric-compatible) | The system font, not bundled | Ships with Windows |
| UI and reading, macOS | **SF Pro** (GPUI's `.SystemUIFont`) | The system font, not bundled | Ships with macOS |
| UI and reading, Linux | **Inter** 4.1 (Rasmus Andersson) | `Inter-Regular.ttf`, `Inter-Italic.ttf`, `Inter-SemiBold.ttf`, `Inter-SemiBoldItalic.ttf`: the static, hinted TTFs from `extras/ttf/` of the 4.1 release, bundled in the Linux build only | SIL OFL 1.1 |
| Code and terminal | **JetBrains Mono NL** (no-ligature build of JetBrains Mono) | `JetBrainsMonoNL-Regular.ttf`, `-Italic.ttf`, `-Bold.ttf`, bundled everywhere | SIL OFL 1.1 |

Why the platform's own sans (decided 2026-11-21, after the user found the type fuzzy on Windows). GPUI rasterises with DirectWrite on Windows (ClearType when the system uses it, natural symmetric rendering, 4 horizontal subpixel positions, whole-pixel baselines). Measured side by side in the app's own renderer at 100% (`--specimen` with the font comparison, PrintWindow captures zoomed 5x): Inter at 13 to 15px puts its stems and its x-height on fractional pixels, so every vertical and the top of every lowercase letter is a two-pixel grey band; Segoe UI Variable is hinted for exactly this rasteriser, lands stems and x-height on the pixel grid and reads sharp at the same sizes, in dark and in light. On macOS, Core Text renders every face well and SF Pro is what the platform's own apps use. On Linux there is no single system face, so Inter stays, bundled. One sans per platform, never two in one build; the identity lives in the layout, the scale and the colour, not in a typeface.

Checked with fontTools on 2026-11-21: Segoe UI Variable and Segoe UI carry every French character (é è ê ë à â ç î ï ô ù û ü œ Œ and their capitals, « » ’ … – — €), U+00A0 and U+202F, the arrows, and `tnum`; they lack the macOS key glyphs (⌘ ⇧ ⌥ ⌃ ↵), which only macOS shows (kbd in 5), where SF Pro has them. Segoe UI's default figures are already equal-width (tabular), Inter's are not, so `tnum` stays set explicitly wherever digits align. JetBrains Mono NL lacks U+202F, U+2009 and ↵, which is acceptable because interface sentences and shortcut hints are never set in mono.

Two weights of the sans: 400 and 600 (Segoe UI Variable's Regular and Semibold instances). There is no 500 and no 700 in the UI. OpenType features: `kern`, `liga` and `calt` at defaults; enable `tnum` explicitly wherever digits align or tick: timestamps, counts, ids, version numbers, table columns, shortcut hints. Proportional figures in prose. GPUI's pinned version has no letter-spacing, so no role asks for any; that is also why the largest size is 24px. Fallback order for missing glyphs: the platform sans, then the platform symbol font; the terminal falls back to the platform mono (Cascadia Mono, Menlo, DejaVu Sans Mono). Bold 700 of the mono exists only for terminal bold.

### 3.2 Scale

| Role token | Family | Size / line | Weight | Used for |
|---|---|---|---|---|
| `type.title` | Sans | 24 / 30 | 600 | Page title (topic, epic, project, document, settings), first-run heading |
| `type.subtitle` | Sans | 20 / 28 | 600 | Document h2, settings section heading |
| `type.heading` | Sans | 16 / 24 | 600 | Dialog title, empty-state heading, Markdown h1/h2 on short surfaces, document h3 |
| `type.reading` | Sans | 15 / 24 (Text size: 14 / 22, 15 / 24, 17 / 28) | 400 | Current state, description, timeline bodies, notes: content only, never an empty state or a placeholder (lead decision 2026-11-22: at 16 / 26 they read much bigger than the 14 around them) |
| `type.document` | Sans | 16 / 26 (Text size: 15 / 24, 16 / 26, 18 / 28) | 400 | Document body, in the reading view and in its editor |
| `type.ui` | Sans | 14 / 20 | 400 | Everything else: rows, buttons, inputs, menus, property values |
| `type.ui-strong` | Sans | 14 / 20 | 600 | Author names, selected segment, primary button, project and epic rows, Markdown h3, the section header of a page ("Current state", "Description", "Knowledge", "Timeline", "Overview"), in `text.secondary` |
| `type.menu` | Sans | 13 / 18 | 400 | Items of context menus, dropdown menus, the slash menu and the tab overflow menu (user feedback 2026-11-22: 14 read too big in a floating layer). Not the palette, which is a search surface and keeps `type.ui` |
| `type.small` | Sans | 12 / 16 | 400 | Timestamps, counts, breadcrumb, tooltips, hints, status bar, kbd hints |
| `type.label` | Sans | 12 / 16 | 600 | Table and tool window headers, view tabs, the tree's area header |
| `type.code` | Mono | 13 / 20 | 400 | Inline code, code blocks, paths, ids, "Claude Code" author name |
| `type.terminal` | Mono | 13 / 17 | 400 / 700 | Terminal grid (cell 7.8 x 17); size adjustable 11 to 18 with `Ctrl+Alt+=` / `Ctrl+Alt+-` (`Cmd+Shift+=` / `Cmd+Shift+-` on macOS) and from the palette |

**Text size** (Settings > Appearance, and the palette: "Text size: Small / Default / Large"; stored in `preferences.json`, applied live) scales `type.reading` and `type.document` only, the two roles people read for minutes; the interface chrome keeps its sizes so rows, controls and the tree do not reflow. Every step keeps a line height of about 1.6 em and an even line height, so baselines stay on whole pixels.

**Why page sections are 14, not 12** (clarity review 2026-11-22). A page reads title 24, property labels 14, section header, body 16. At 12 the section headers were smaller than the property labels above them and than every row under them, so the eye read "Description" as a caption and the page as one long run; at 14 / 600 in `text.secondary` they sit one clear step under the title and over the rows, with no new size. The rendered set of sizes on a record page is then 24, 15, 14 and 12, plus 13 for mono and for an open menu (`type.menu`), and no role is used out of its row of this table.

**Empty states are never in a reading role** (lead decision 2026-11-22, from a capture of a French topic page where "Aucune entrée pour l'instant..." under the Timeline header read larger than the headers). An empty state or the placeholder of an empty field ("No entries yet...", "Trash is empty.", "Add a description") is `type.ui` in `text.tertiary` (or `type.small` where the surface is small), like the Knowledge section's; `type.heading` stays for the heading of a whole empty page.

**Mono beside sans.** `type.code` is 13 / 20 and sits on a 14 / 20 line, so ids and paths in a sans row share its baseline grid; where mono is a value among sans metadata at 12 (the meta line's id), it is set at 12 / 16, the one place mono takes the small size.

Sentence case everywhere, in both languages. No uppercase, no italics for UI (italics are for Markdown emphasis only). Minimum text size is 12px; nothing smaller. Sizes here are at 100% zoom: zoom (layout.md 4.9, 90% to 200%) scales every role at once, Text size and the terminal size included, and is the only thing that takes a role under 12 (to 10.8, at the 90% floor).

**Zoom and Text size are different settings.** Text size changes the two reading roles and leaves the interface where it is, for a person who reads long text; zoom scales the whole window, chrome included, for a screen shared or seen from further away. They multiply.

### 3.3 Long-form text

Descriptions, current state, timeline entries and notes are stored as **Markdown (CommonMark)** and rendered when not being edited. This is the format Claude Code writes natively and the human can type without a toolbar. Editing shows the raw Markdown in the same face and size (`type.reading`), so entering edit mode does not reflow the paragraph.

Rendered in v1: paragraphs, `*emphasis*`, `**strong**` (600), inline code, fenced code blocks (no syntax highlighting), links, ordered and unordered lists (two nesting levels), task lists (read-only checkboxes), blockquotes, headings, horizontal rule. Tables are rendered too (Claude Code writes them constantly in specs). Not rendered in v1: images, HTML, footnotes. Unknown syntax shows as typed.

Setting (tuned on 2026-11-21 for an app that is read for hours: calm, untiring, few sizes): `type.reading` in `text.primary`; paragraph gap 0.75 em (12); list item gap 4, indent 20, marker in `text.tertiary`; headings stay close to the body: h1/h2 two px over the reading size, weight 600, h3 the reading size at 600, 24 above and 8 below (a timeline entry rarely needs them, and it must not out-shout the page title); blockquote indented 16 in `text.secondary` with no bar; inline code `type.code` on `bg.sunken`, padding 0 x 4, radius 4; code block `type.code` on `bg.sunken`, padding 12 x 16, radius 8, horizontal scroll, never wraps; links `accent`, underlined at rest (1px, 2 under the baseline), `accent.hover` on hover, focus ring on keyboard focus, open in the system browser; rule 1px `border.subtle` with 16 above and below.

Tables: `type.ui`, cells padded 6 x 12, header row weight 600, 1px `border.subtle` under each row and no vertical lines, column alignment follows the Markdown alignment row (left by default), numeric columns right-aligned with `tnum`, cell text wraps at a 320 maximum column width; a table wider than the column scrolls horizontally on its own, like a code block.

**Documents are the long-form surface** (ADR 0007): specs and architecture notes that are read for minutes, not glanced at. They get their own setting, used only in the document reading view and its editor:
- Body `type.document` 16 / 26 at the Default text size, in `text.primary`; paragraph gap 16; list item gap 6, indent 24.
- Headings carry real structure there, because the agent addresses sections by heading: h1 the page title's 24 (32 above, 12 below); h2 three px over the body, at most 22 (32 above, 8 below); h3 one px over the body (24 above, 4 below), all at 600, so the steps hold at every Text size; h4 and deeper as h3 in `text.secondary`. A heading is never the last line before a scroll boundary without its first paragraph line (keep-with-next when paginating by keyboard).
- Code blocks, inline code, tables, blockquotes, links and rules as above; code stays 13 / 20 so 80 columns fit the measure.
- Short surfaces (current state, description, entries, notes) keep `type.reading` and the compact heading sizes: they sit among UI and must not out-shout the page title.

Measure: the content column is at most **560** wide by default and never narrower than 360. **Reading width** (Settings > Appearance, and the palette: "Reading width: Narrow / Default / Wide"; stored in `preferences.json`, applied live) sets that maximum to 480, 560 or 720. Measured with Segoe UI (0.43 em per character of English prose: 6.5 px at 15, 6.95 at 16), a timeline entry, which sits 32 inside the column, holds about 69, 81 and 106 characters per line at the Default text size since the reading role became 15 (it held 65, 75 and 100 at 16), and a document at 16 about 69, 81 and 104. *Open (2026-11-22, for the product designer):* the Default is now above the comfortable 60 to 75; a 520 default (Narrow 440, Wide 680) would bring it back to about 75. Text never runs full-bleed across a wide window: the column stays centred and the margins grow. Text is left-aligned, ragged right, no hyphenation. Single-line titles truncate with an ellipsis in lists and wrap in the topic header.
