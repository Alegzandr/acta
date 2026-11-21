# Acta design: typography

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 3. Typography

### 3.1 Families

| Role | Family | Files bundled | License |
|---|---|---|---|
| UI and reading | **Inter** 4.1 (Rasmus Andersson) | `Inter-Regular.ttf`, `Inter-Italic.ttf`, `Inter-SemiBold.ttf`, `Inter-SemiBoldItalic.ttf`: the static, hinted TTFs from `extras/ttf/` of the 4.1 release (412, 417, 420 and 424 KB; 1.67 MB in all) | SIL OFL 1.1 |
| Code and terminal | **JetBrains Mono NL** (no-ligature build of JetBrains Mono) | `JetBrainsMonoNL-Regular.ttf`, `-Italic.ttf`, `-Bold.ttf` (already bundled) | SIL OFL 1.1 |

Why Inter. It is drawn for interface text at 11 to 16px: its x-height is 0.546 em against 0.486 for Source Sans 3 (read from the font files), which is what lets the base size drop to 13px and the density tighten without losing legibility. It is neutral enough to disappear, which is the point of a workbench, and it is the face the reference interfaces lean on, so Acta feels native to that family of tools without borrowing anything they own. It replaces Source Sans 3, whose four files leave the bundle; one sans, not two.

Checked in the four Inter files with fontTools on 2026-11-20: every French character (é è ê ë à â ç î ï ô ù û ü œ Œ and their capitals, « » ’ … – — €), the no-break space U+00A0 and the narrow no-break space U+202F that French punctuation needs, the arrows, and the macOS key glyphs (⌘ ⇧ ⌥ ⌃ ↵) are present; the features `tnum`, `case` and `zero` exist in all four; the files carry hinting. Default figures are proportional (1 is 833 units wide, 0 is 1292), so `tnum` must be set explicitly wherever digits align. JetBrains Mono NL lacks U+202F, U+2009 and ↵, which is acceptable because interface sentences and shortcut hints are never set in mono (see kbd in 5).

Still conditional, as before, on the M1 rendering check in GPUI on Windows (grayscale antialiasing at 13px). The named fallback is Source Sans 3 with `type.ui` at 14 / 20 and `type.small` at 13 / 18, the rest of the scale unchanged.

Static files only (no variable fonts), two weights of the sans: 400 and 600. There is no 500 and no 700 in the UI. OpenType features: `kern`, `liga` and `calt` at defaults; enable `tnum` explicitly wherever digits align or tick: timestamps, counts, ids, version numbers, table columns, shortcut hints. Proportional figures in prose. GPUI's pinned version has no letter-spacing, so no role asks for any; that is also why the largest size is 24px, the largest at which Inter's default spacing still reads as intended. Fallback order for missing glyphs: bundled family, then the platform UI font (Segoe UI, SF Pro, system sans), then the platform symbol font; the terminal falls back to the platform mono (Cascadia Mono, Menlo, DejaVu Sans Mono). Bold 700 of the mono exists only for terminal bold.

### 3.2 Scale

| Role token | Family | Size / line | Weight | Used for |
|---|---|---|---|---|
| `type.title` | Sans | 24 / 30 | 600 | Page title (topic, epic, project, document, settings), first-run heading |
| `type.subtitle` | Sans | 20 / 28 | 600 | Document h2, settings section heading |
| `type.heading` | Sans | 16 / 24 | 600 | Dialog title, empty-state heading, Markdown h1/h2 on short surfaces, document h3 |
| `type.reading` | Sans | 15 / 24 | 400 | Current state, description, timeline bodies, notes |
| `type.document` | Sans | 16 / 26 | 400 | Document body, in the reading view and in its editor |
| `type.ui` | Sans | 13 / 20 | 400 | Everything else: rows, buttons, inputs, menus, property values |
| `type.ui-strong` | Sans | 13 / 20 | 600 | Author names, selected segment, primary button, project rows, Markdown h3 |
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
