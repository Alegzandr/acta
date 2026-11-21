# Acta design: typography

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

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
| `type.reading` | Sans | 15 / 24 | 400 | Current state, description, timeline bodies, notes |
| `type.document` | Sans | 16 / 26 | 400 | Document body, in the reading view and in its editor |
| `type.ui` | Sans | 14 / 20 | 400 | Everything else: rows, buttons, inputs, menus, property values |
| `type.ui-strong` | Sans | 14 / 20 | 600 | Author names, selected segment, primary button, project rows, Markdown h3 |
| `type.small` | Sans | 12 / 16 | 400 | Timestamps, counts, breadcrumb, tooltips, hints, status bar, kbd hints |
| `type.label` | Sans | 12 / 16 | 600 | Section labels ("Current state", "Timeline"), epic rows, table and tool window headers |
| `type.code` | Mono | 13 / 20 | 400 | Inline code, code blocks, paths, ids, "Claude Code" author name |
| `type.terminal` | Mono | 13 / 17 | 400 / 700 | Terminal grid (cell 7.8 x 17); size adjustable 11 to 18 with `Ctrl+Alt+=` / `Ctrl+Alt+-` (`Cmd+Shift+=` / `Cmd+Shift+-` on macOS) and from the palette |

Sentence case everywhere, in both languages. No uppercase, no italics for UI (italics are for Markdown emphasis only). Minimum text size is 12px; nothing smaller.

### 3.3 Long-form text

Descriptions, current state, timeline entries and notes are stored as **Markdown (CommonMark)** and rendered when not being edited. This is the format Claude Code writes natively and the human can type without a toolbar. Editing shows the raw Markdown in the same face and size (`type.reading`), so entering edit mode does not reflow the paragraph.

Rendered in v1: paragraphs, `*emphasis*`, `**strong**` (600), inline code, fenced code blocks (no syntax highlighting), links, ordered and unordered lists (two nesting levels), task lists (read-only checkboxes), blockquotes, headings, horizontal rule. Tables are rendered too (Claude Code writes them constantly in specs). Not rendered in v1: images, HTML, footnotes. Unknown syntax shows as typed.

Setting: `type.reading` in `text.primary`; paragraph gap 12; list item gap 4, indent 20, marker in `text.tertiary`; headings h1/h2 as `type.heading` and h3 as `type.ui-strong`, 24 above and 8 below (a timeline entry rarely needs them, so they stay small); blockquote indented 16 in `text.secondary` with no bar; inline code `type.code` on `bg.sunken`, padding 0 x 4, radius 4; code block `type.code` on `bg.sunken`, padding 12, radius 6, horizontal scroll, never wraps; links `accent`, underlined at rest (1px, 2 under the baseline), `accent.hover` on hover, focus ring on keyboard focus, open in the system browser; rule 1px `border.subtle` with 16 above and below.

Tables: `type.ui`, cells padded 6 x 12, header row weight 600, 1px `border.subtle` under each row and no vertical lines, column alignment follows the Markdown alignment row (left by default), numeric columns right-aligned with `tnum`, cell text wraps at a 320 maximum column width; a table wider than the column scrolls horizontally on its own, like a code block.

**Documents are the long-form surface** (ADR 0007): specs and architecture notes that are read for minutes, not glanced at. They get their own setting, used only in the document reading view and its editor:
- Body `type.document` 16 / 26 in `text.primary`; paragraph gap 16; list item gap 6, indent 24.
- Headings carry real structure there, because the agent addresses sections by heading: h1 `type.title` (32 above, 12 below); h2 `type.subtitle` (32 above, 8 below); h3 `type.heading` (24 above, 4 below); h4 and deeper as h3 in `text.secondary`. A heading is never the last line before a scroll boundary without its first paragraph line (keep-with-next when paginating by keyboard).
- Code blocks, inline code, tables, blockquotes, links and rules as above; code stays 13 / 20 so 80 columns fit the measure.
- Short surfaces (current state, description, entries, notes) keep `type.reading` 15 / 24 and the compact heading sizes: they sit among UI and must not out-shout the page title.

Measure: the content column is at most **680px** and never narrower than 360px. With Inter (average advance 0.47 em on English text, measured from the font) that is about 95 characters at 15px and 90 at 16px: a generous measure, the one the page-like references use, and the upper bound; nothing may widen it. Text is left-aligned, ragged right, no hyphenation. Single-line titles truncate with an ellipsis in lists and wrap in the topic header.
