# Acta design system

Single source of truth for every pixel and every key press. UI code conforms to this file; if a screen needs something that is not here, change this file first, in the same change, and say why.

Identity: **a workbench for long work.** A tree on the left, a page in the middle, tool windows at the side, one palette that reaches everything. Neutral, quiet surfaces in dark and in light, one verdigris accent, one sans for everything a person reads and one mono for the machine. The content belongs to the user and to Claude Code; the chrome recedes behind it.

Units are logical pixels (GPUI `px`). `Mod` means `Cmd` on macOS and `Ctrl` on Windows and Linux.

## Direction

Decided by the user on 2026-11-20. Acta takes its structure from three tools its users already live in, Obsidian, Notion and the JetBrains IDEs, and is none of them: what is borrowed is structure and behaviour, never branding, icons or palettes. Everything below in this file follows from this section; where a later section and this one disagree, this one wins and the later one is a defect.

| From | Acta takes | Acta leaves, and why |
|---|---|---|
| **Obsidian** | The tree sidebar with collapsible nodes. Chrome that recedes behind content the user owns. Markdown as the format people and the agent write, rendered at rest and raw while editing. A properties block at the top of a page. Tabs. The quick switcher. A neutral grey dark theme. | Split panes: the terminal and the chat are the companions that sit beside a page, and a second page column would push everything under its minimum at common widths. Backlinks: links between records are not in the model yet (11). An outline pane: "Go to heading" in the palette is the outline, the same one the agent sees. Graph, canvas, ribbon, plugins, community themes: nothing in Acta's data asks for them. |
| **Notion** | A page is a title, typed properties, then the body. A generous measure. The breadcrumb in a top bar. Controls revealed on hover. A slash menu in editors. Collections of records as quiet tables with filter and sort. A near-white light theme and unshowy type. | Blocks as the storage model: text stays Markdown, which Claude Code writes natively (ADR 0007). Boards: six statuses do not fit a page column, and `S` already changes a status in one key. User-defined properties, page icons, covers, emoji, comments: each makes the agent's writes less predictable or adds decoration. |
| **JetBrains IDEs (new UI)** | Tool windows docked right or bottom, each with a header, a toggle shortcut and a remembered size. The terminal as a tool window. Search everywhere: one palette with scopes. A slim status bar. Compact density. Unambiguous focus and selection. A top bar that stays out of the way. | Icon stripes: three tool windows do not pay for 80px of permanent chrome, so their toggles live in the titlebar. Menu bar, action toolbar, gutter icons, hover popups, double-Shift: an IDE's density of affordances is wrong for a record that is mostly read. |

The synthesis is one object, a workbench, not three apps side by side: one tree (projects, epics, topics, with each project's inbox and knowledge), pages that read like documents (a topic is a page with properties, a current state and a timeline; a knowledge entry is a Markdown document), tool windows for the terminal and the chat panel, tabs only when more than one page is open, one palette, one status bar.

Requirements that hold whatever the look: two complete themes, dark and light, both first-class, with a System / Light / Dark setting (2); the interface in English (default) and French, selectable and persisted, with user and agent content never translated (10); every element on screen backed by real data from the model (11).

Refused outright: serif or editorial display type, parchment or paper textures, illustrations and engravings, Roman numerals, burgundy, gradients, glass effects, dashboards made of cards. A "Project Ledger" direction (parchment, serif headings, engravings) was briefed and withdrawn by the user on 2026-11-20; do not revive it.

## 1. Principles

1. **The record is the interface.** Topic title, current state and timeline are the product; chrome exists to reach them. If a pixel is neither content nor a control the user will use this week, remove it. A surface with nothing on it is correct, not unfinished.
2. **One accent, spent on "here" and "go".** Verdigris marks only: keyboard focus, the current selection marker, the primary action, links, the Active status and unread changes. Never decoration, never a second accent. If two accent things compete on one screen, one of them is wrong.
3. **Hierarchy comes from type and space, then tone, then lines.** Reach for size/weight/spacing first, a surface step second, a 1px border third. Shadows only on things that float above the window. Boxes inside boxes are a defect.
4. **Keyboard first, pointer complete.** Every action has a key path through the command palette at minimum; every key path has a visible pointer equivalent. A feature without a palette command is not shipped.
5. **Instant means the next frame.** Selection, hover, focus, navigation and typing never animate and never wait on disk. Motion is reserved for things that appear from nowhere (overlays, toasts, remote changes) and exists to explain, not to please.
6. **The agent writes; the human is never moved.** Changes made by Claude Code appear live but never take focus, never scroll the viewport, never overwrite a field being edited. The reader's place is sacred.
7. **Undo over asking.** Anything reversible happens immediately with an undo. A dialog is an admission that we could not make the action reversible.

**Never:** gradients; glow or coloured shadows; blur; emoji anywhere (UI, sample data, empty states); illustrations or mascots; all-caps labels; coloured left-stripe borders on cards; pill-shaped buttons; more than two font weights; a spinner before 400 ms; skeleton shimmer; badges with counts above 99; toasts for things the user can already see changed; modal dialogs for reversible actions; a robot or sparkle icon for Claude Code; pure black `#000` anywhere, pure white `#FFF` as text or as a dark-theme colour; a second typeface for display; textures; centred body text; tooltips as the only way to learn what a control does when a label fits.

## 2. Color

All values are sRGB hex. Alpha tokens are composited by the renderer (GPUI `hsla` alpha), so they work on every surface. Token names are the Rust theme field names with `.` replaced by `_`.

**Themes.** Dark and light are both first-class: every token has both values, every screen is built and reviewed in both, and neither is derived from the other by inversion. The setting is System / Light / Dark (Settings, and palette commands "Theme: System", "Theme: Light", "Theme: Dark"). **System is the default on a new install**: the user has already told their OS what they want, and because both themes are held to the same standard neither needs to be the showcase. System follows the OS live; where the OS reports nothing (some Linux desktops) Acta uses dark, the theme its users' terminals already wear. Switching is instant and never animates (6.3). The choice is stored in the preferences file (4.7), so it applies before the database opens.

### 2.1 Tokens

| Token | Dark | Light | Used for |
|---|---|---|---|
| `bg.sunken` | `#1B1B1A` | `#F4F3F0` | Sidebar, titlebar, tab bar, status bar, tool window headers, terminal (dark), code blocks |
| `bg.base` | `#212120` | `#FBFAF8` | Pages, the active tab |
| `bg.raised` | `#292928` | `#FFFFFF` | Current-state panel, note cards, inputs, terminal (light) |
| `bg.overlay` | `#30302E` | `#FFFFFF` | Command palette, menus, dialogs, tooltips, toasts |
| `fill.hover` | `text.primary` @ 6% | same rule | Hovered row or ghost control |
| `fill.selected` | `text.primary` @ 9% | same rule | Selected row, active segment, active tool window toggle, human author mark |
| `fill.pressed` | `text.primary` @ 10% | same rule | Pressed row or control |
| `border.subtle` | `#353533` | `#E8E6E1` | Pane dividers, hairlines, panel outlines (on sunken, base and raised) |
| `border.default` | `#424240` | `#D9D7D1` | Overlay outlines and hairlines inside overlays, decision entry box, kbd hint |
| `border.strong` | `#85847E` | `#8A8881` | Input and secondary-button outline, Claude Code author mark, scrollbar thumb |
| `text.primary` | `#E4E3DF` | `#24231F` | Titles, body, values |
| `text.secondary` | `#B2B1AB` | `#56544E` | Tree rows at rest, entry type, icons at rest, metadata on selected rows |
| `text.tertiary` | `#9C9B94` | `#66645E` | Property labels, timestamps, counts, placeholders, breadcrumb, hints, status bar |
| `text.disabled` | `#62615C` | `#A7A59F` | Disabled controls only (exempt from AA by definition) |
| `accent` | `#72B5A2` | `#2B6F61` | Primary button fill, link text, focus ring, selection marker, Active glyph, unread dot, terminal cursor |
| `accent.hover` | `#86C4B2` | `#235E52` | Primary button hover, link hover |
| `accent.pressed` | `#5E9F8D` | `#1C4E44` | Primary button pressed |
| `accent.subtle` | `accent` @ 16% | same rule | Remote-change highlight, palette scope chip, matched-drop target |
| `text.on-accent` | `#10201C` | `#FBFAF8` | Text and icons on `accent` and on `danger` fills |
| `danger` | `#DE7B70` | `#AD4337` | Destructive button, error text and icon, invalid input outline |
| `warning` | `#D4A24C` | `#85590A` | Blocked glyph, edit-conflict and update-failed icon, folder-not-found icon |
| `success` | `#9BB26B` | `#4C6A24` | Done glyph, confirmation toast icon, completed setup step |
| `selection` | `accent` @ 30% | same rule | Text selection in editors, rendered Markdown and terminal |
| `focus.ring` | = `accent` | = `accent` | 2px keyboard focus ring |
| `scrim` | `#000000` @ 45% | `#24231F` @ 25% | Behind dialogs only |
| `shadow.overlay` | `0 8px 24px #000000` @ 40% | `0 8px 24px #24231F` @ 14% | Palette, menus, dialogs, tooltips, toasts |
| `caption.close.hover` | `#C42B1C` | `#C42B1C` | Windows/Linux close button hover (platform convention), glyph `#FFFFFF` |

Elevation is tone, lightest on top in dark and in light: `sunken < base < raised <= overlay`. In light, `raised` and `overlay` are both white; an overlay is told apart by its outline and shadow, which it always has. A surface never sits on a surface more than one step lighter than itself except overlays.

Why neutral greys: the surfaces are greys with a trace of warmth (blue one to three points under red), close to what the three references use and far from the earlier brown-blacks. Neutral chrome lets the only colours on screen, status glyphs, the accent and terminal output, read true; the trace of warmth keeps hours of reading from feeling cold. The light theme's page is one step off white (`#FBFAF8`) so that white inputs and panels can sit above it.

Why verdigris: the semantic colours (red, amber, green) are all warm, so a warm accent would collide with danger or warning on a focus ring next to an error. A grey-green copper patina stays calm on neutral greys, separates from all three semantics, and is neither the blue nor the purple of the three references, so nothing in Acta reads as borrowed.

### 2.2 Measured contrast (WCAG 2.x, computed from the hex values)

| Pair | Dark | Light | Requirement |
|---|---|---|---|
| `text.primary` on `bg.sunken` / `base` / `raised` / `overlay` | 13.42 / 12.55 / 11.34 / 10.30 | 14.17 / 15.08 / 15.73 / 15.73 | 4.5 |
| `text.secondary` on same four | 8.02 / 7.50 / 6.77 / 6.15 | 6.82 / 7.26 / 7.57 / 7.57 | 4.5 |
| `text.tertiary` on same four | 6.18 / 5.78 / 5.22 / 4.74 | 5.33 / 5.67 / 5.92 / 5.92 | 4.5 |
| `accent` (link, glyph) on same four | 7.26 / 6.79 / 6.13 / 5.57 | 5.33 / 5.67 / 5.92 / 5.92 | 4.5 |
| `danger` on same four | 5.89 / 5.50 / 4.97 / 4.52 | 5.21 / 5.54 / 5.78 / 5.78 | 4.5 |
| `warning` on same four | 7.45 / 6.96 / 6.29 / 5.71 | 5.52 / 5.87 / 6.12 / 6.12 | 3.0 (glyph) |
| `success` on same four | 7.37 / 6.89 / 6.22 / 5.65 | 5.58 / 5.93 / 6.19 / 6.19 | 3.0 (glyph) |
| `text.on-accent` on `accent` / `.hover` / `.pressed` | 7.10 / 8.47 / 5.48 | 5.67 / 7.20 / 9.06 | 4.5 |
| `text.on-accent` on `danger` | 5.76 | 5.54 | 4.5 |
| `text.primary` on `fill.selected` over `sunken` / `base` / `overlay` | 10.73 / 9.93 / 8.08 | 11.94 / 12.68 / 13.22 | 4.5 |
| `text.secondary` on `fill.selected` over `sunken` / `base` / `overlay` | 6.41 / 5.93 / 4.83 | 5.75 / 6.11 / 6.36 | 4.5 |
| `text.secondary` on `fill.hover` over `sunken` / `base` / `overlay` | 6.96 / 6.45 / 5.24 | 6.09 / 6.48 / 6.75 | 4.5 |
| `text.tertiary` on `fill.hover` over `sunken` / `base` (tree counts, folder row label) | 5.36 / 4.97 | 4.76 / 5.06 | 4.5 |
| `warning` glyph on `fill.hover` over `base` (folder not found) | 5.99 | 5.24 | 3.0 (glyph) |
| `accent` glyph on `fill.selected` over `sunken` (Active topic selected in the tree) | 5.81 | 4.49 | 3.0 (glyph) |
| `text.primary` on `accent.subtle` over `sunken` / `base` / `overlay` | 10.09 / 9.37 / 7.73 | 11.41 / 12.07 / 12.55 | 4.5 |
| `text.primary` on `selection` over `base` | 6.99 | 9.78 | 4.5 |
| `border.strong` vs `sunken` / `base` / `raised` / `overlay` | 4.60 / 4.30 / 3.88 / 3.53 | 3.20 / 3.40 / 3.55 / 3.55 | 3.0 (control boundary) |
| `focus.ring` vs `sunken` / `base` / `overlay` | 7.26 / 6.79 / 5.57 | 5.33 / 5.67 / 5.92 | 3.0 |
| `#FFFFFF` on `caption.close.hover` | 5.66 | 5.66 | 4.5 |

Rules that follow from the numbers:
- `text.tertiary` is not allowed on `fill.selected` (3.72 over `bg.overlay` in dark, 4.49 over `bg.sunken` in light). On any selected row, in the tree, a table, the palette or a menu, metadata switches to `text.secondary`.
- Text on `accent.subtle` is always `text.primary`. Coloured or tertiary text on a tinted fill is not allowed (it measures 4.1 to 5.1).
- `border.subtle` and `border.default` are separators (1.1 to 1.7 against their surface), never the only boundary of an interactive control. `border.subtle` nearly vanishes on `bg.overlay` (1.08 in dark), so hairlines inside overlays use `border.default`.
- `text.disabled` (2.1 to 2.8) is used only with a disabled control, never for information.

### 2.3 Topic statuses

Six statuses. Four are open, two are closed. The set answers exactly the questions both readers ask: "what is being worked on", "where am I needed", "what was decided not to happen".

| Status | Meaning | Glyph (Lucide) | Colour | Open |
|---|---|---|---|---|
| Draft | Captured, not yet shaped. Default for topics Claude Code creates from notes; the human's cue to refine. | `circle-dashed` | `text.tertiary` | yes |
| Planned | Scoped and ready to pick up. | `circle` | `text.secondary` | yes |
| Active | Being worked on now. | `circle-dot` | `accent` | yes |
| Blocked | Cannot proceed; the latest entry says on what. Usually waiting on the human. | `circle-pause` | `warning` | yes |
| Done | Finished. | `circle-check` | `success` | no |
| Dropped | Deliberately abandoned. Kept because "we decided not to" is memory the agent needs. | `circle-slash` | `text.tertiary` | no |

No "In review", no priorities, no custom statuses in v1: each addition makes the agent's writes less predictable. Status never relies on colour: every status has a distinct glyph interior (dashes, empty, dot, bars, check, slash), the Status property of a topic page always shows glyph plus word, and the glyph has a tooltip and an accessible name with the word. Closed topics render their title in `text.tertiary` (no strikethrough) and collapse under a "3 closed" row at the end of their epic.

### 2.4 Terminal palette

Terminal background is `bg.sunken` in dark and `bg.raised` in light. Cursor: block, `accent`, character under it the terminal background (7.26 dark, 5.92 light); unfocused cursor is a 1px `accent` outline. Selection: `selection`, text colours unchanged (foreground on it: 6.72 dark, 10.13 light). Ratios below are against the terminal background.

| Slot | Dark | Ratio | Light | Ratio |
|---|---|---|---|---|
| foreground | `#D9D8D3` | 12.08 | `#24231F` | 15.73 |
| 0 black | `#353533` | 1.40 (background use) | `#24231F` | 15.73 |
| 1 red | `#DE7B70` | 5.89 | `#AD4337` | 5.78 |
| 2 green | `#9BB26B` | 7.37 | `#4C6A24` | 6.19 |
| 3 yellow | `#D4A24C` | 7.45 | `#85590A` | 6.12 |
| 4 blue | `#7FA6CC` | 6.75 | `#2F5F8F` | 6.66 |
| 5 magenta | `#BC96BC` | 6.75 | `#844C86` | 6.25 |
| 6 cyan | `#72B5A2` | 7.26 | `#2B6F61` | 5.92 |
| 7 white | `#B2B1AB` | 8.02 | `#66645E` | 5.92 |
| 8 bright black | `#9C9B94` | 6.18 | `#66645E` | 5.92 |
| 9 bright red | `#EC9A90` | 7.87 | `#963529` | 7.40 |
| 10 bright green | `#B4C888` | 9.49 | `#3E591B` | 7.93 |
| 11 bright yellow | `#E4BB70` | 9.56 | `#6F4A06` | 7.90 |
| 12 bright blue | `#9DBDDD` | 8.83 | `#244D76` | 8.76 |
| 13 bright magenta | `#D0B0D2` | 8.88 | `#6E3D70` | 8.19 |
| 14 bright cyan | `#93CCBB` | 9.52 | `#235E52` | 7.52 |
| 15 bright white | `#F1F0EC` | 15.12 | `#56544E` | 7.57 |

Red, green, yellow and cyan are the app's `danger`, `success`, `warning` and `accent`, so the terminal and the tracker read as one object. Bright black is `text.tertiary` because Claude Code sets a lot of secondary text in it; it must stay readable. Blue and magenta exist only here.

## 3. Typography

### 3.1 Families

| Role | Family | Files bundled | License |
|---|---|---|---|
| UI and reading | **Inter** 4.1 (Rasmus Andersson) | `Inter-Regular.ttf`, `Inter-Italic.ttf`, `Inter-SemiBold.ttf`, `Inter-SemiBoldItalic.ttf`: the static, hinted TTFs from `extras/ttf/` of the 4.1 release (412, 417, 420 and 424 KB; 1.67 MB in all) | SIL OFL 1.1 |
| Code and terminal | **JetBrains Mono NL** (no-ligature build of JetBrains Mono) | `JetBrainsMonoNL-Regular.ttf`, `-Italic.ttf`, `-Bold.ttf` (already bundled) | SIL OFL 1.1 |

Why Inter. It is drawn for interface text at 11 to 16px: its x-height is 0.546 em against 0.486 for Source Sans 3 (read from the font files), which is what lets the base size drop to 13px and the density tighten without losing legibility. It is neutral enough to disappear, which is the point of a workbench, and it is the face the reference interfaces lean on, so Acta feels native to that family of tools without borrowing anything they own. It replaces Source Sans 3, whose four files leave the bundle; one sans, not two.

Checked in the four Inter files with fontTools on 2026-11-20: every French character (é è ê ë à â ç î ï ô ù û ü œ Œ and their capitals, « » ’ … – — €), the no-break space U+00A0 and the narrow no-break space U+202F that French punctuation needs, the arrows, and the macOS key glyphs (⌘ ⇧ ⌥ ⌃ ↵) are present; the features `tnum`, `case` and `zero` exist in all four; the files carry hinting. Default figures are proportional (1 is 833 units wide, 0 is 1292), so `tnum` must be set explicitly wherever digits align. The Source Sans 3 file bundled before has no `tnum` feature at all, a second reason for the change. JetBrains Mono NL lacks U+202F, U+2009 and ↵, which is acceptable because interface sentences and shortcut hints are never set in mono (see kbd in 5).

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
| `type.terminal` | Mono | 13 / 17 | 400 / 700 | Terminal grid (cell 7.8 x 17); size adjustable 11 to 18 with `Mod+=` / `Mod+-` |

Sentence case everywhere, in both languages. No uppercase, no italics for UI (italics are for Markdown emphasis only). Minimum text size is 12px; nothing smaller.

### 3.3 Long-form text

Descriptions, current state, timeline entries and notes are stored as **Markdown (CommonMark)** and rendered when not being edited. This is the format Claude Code writes natively and the human can type without a toolbar. Editing shows the raw Markdown in the same face and size (`type.reading`), so entering edit mode does not reflow the paragraph.

Rendered in v1: paragraphs, `*emphasis*`, `**strong**` (600), inline code, fenced code blocks (no syntax highlighting), links, ordered and unordered lists (two nesting levels), task lists (read-only checkboxes), blockquotes, headings, horizontal rule. Tables are rendered too (Claude Code writes them constantly in specs). Not rendered in v1: images, HTML, footnotes. Unknown syntax shows as typed.

Setting: `type.reading` in `text.primary`; paragraph gap 12; list item gap 4, indent 20, marker in `text.tertiary`; headings h1/h2 as `type.heading` and h3 as `type.ui-strong`, 24 above and 8 below (a timeline entry rarely needs them, so they stay small); blockquote indented 16 in `text.secondary` with no bar; inline code `type.code` on `bg.sunken`, padding 0 x 4, radius 4; code block `type.code` on `bg.sunken`, padding 12, radius 6, horizontal scroll, never wraps; links `accent`, underlined on hover and focus, open in the system browser; rule 1px `border.subtle` with 16 above and below.

Tables: `type.ui`, cells padded 6 x 12, header row weight 600, 1px `border.subtle` under each row and no vertical lines, column alignment follows the Markdown alignment row (left by default), numeric columns right-aligned with `tnum`, cell text wraps at a 320 maximum column width; a table wider than the column scrolls horizontally on its own, like a code block.

**Documents are the long-form surface** (ADR 0007): specs and architecture notes that are read for minutes, not glanced at. They get their own setting, used only in the document reading view and its editor:
- Body `type.document` 16 / 26 in `text.primary`; paragraph gap 16; list item gap 6, indent 24.
- Headings carry real structure there, because the agent addresses sections by heading: h1 `type.title` (32 above, 12 below); h2 `type.subtitle` (32 above, 8 below); h3 `type.heading` (24 above, 4 below); h4 and deeper as h3 in `text.secondary`. A heading is never the last line before a scroll boundary without its first paragraph line (keep-with-next when paginating by keyboard).
- Code blocks, inline code, tables, blockquotes, links and rules as above; code stays 13 / 20 so 80 columns fit the measure.
- Short surfaces (current state, description, entries, notes) keep `type.reading` 15 / 24 and the compact heading sizes: they sit among UI and must not out-shout the page title.

Measure: the content column is at most **680px** and never narrower than 360px. With Inter (average advance 0.47 em on English text, measured from the font) that is about 95 characters at 15px and 90 at 16px: a generous measure, the one the page-like references use, and the upper bound; nothing may widen it. Text is left-aligned, ragged right, no hyphenation. Single-line titles truncate with an ellipsis in lists and wrap in the topic header.

## 4. Spacing, sizing and layout

### 4.1 Scales

- **Spacing** (`space.*`): 2, 4, 8, 12, 16, 24, 32, 48. Nothing else. 4 inside a control, 8 between related items, 12/16 inside containers, 24 between sections, 32/48 around the content column.
- **Control heights**: 28 default (buttons, inputs, segmented control, tree rows); 24 compact (icon buttons inside rows, titlebar controls on macOS); 32 for palette and menu rows; 44 for the palette input. Minimum pointer target is 24 x 24.
- **Density**: one density. Tree row 28, menu and palette row 32, timeline entries separated by 24, note cards by 8.
- **Radii**: `radius.sm` 4 (buttons, inputs, rows, kbd, inline code, Claude Code mark); `radius.md` 6 (panels, note cards, code blocks, toasts, tooltips, menus); `radius.lg` 8 (palette, dialogs). Circles only for the human author mark, unread dot and scrollbar thumb.
- **Borders vs shadows**: panes are separated by a 1px `border.subtle` line and a surface step, never a shadow. Containers on `bg.base` use `bg.raised` plus a 1px `border.subtle` outline. Only floating layers (palette, menu, dialog, tooltip, toast) get `shadow.overlay`, always together with a 1px `border.default` outline, because a shadow alone disappears on a dark surface.
- **Icons**: **Lucide** (ISC license; a subset derived from Feather is MIT), embedded as SVG, only the icons listed in this file. Rendered at 16 (default) or 14 (status glyphs in the tree, entry type icons), exported with `stroke-width="1.5"` on the 24 grid so strokes land at 1px at 16. Colour is `text.secondary` at rest and `text.primary` on hover or selection, unless a token above says otherwise. Icons are never filled, never inside coloured circles, and an icon-only button always has a tooltip with its shortcut.

### 4.2 App shell

```
+------------------------------------------------------------------------------------------+
| Titlebar 36: sidebar toggle, back, forward, breadcrumb ... search, chat, terminal        |
+---------------+------------------------------------+-------------------------------------+
| Sidebar       | Tab bar 32 (only with 2+ tabs)     | Right dock: one tool window         |
| (tree)        +------------------------------------+ header 28                           |
| bg.sunken     | Page, bg.base                      | Terminal or Chat                    |
| 260           | flexible, min 420                  | 648 = 80 columns                    |
| (200 to 400)  | content column max 680, centred    | (400 to 60% window)                 |
|               +------------------------------------+                                     |
|               | Bottom dock (optional), header 28  |                                     |
|               | 320 (160 to 70% of the height)     |                                     |
+---------------+------------------------------------+-------------------------------------+
| Status bar 24: project, Claude Code                                   update, version    |
+------------------------------------------------------------------------------------------+
```

Five regions, each with one job: the tree finds, the page reads and writes, tool windows run things beside it, the titlebar says where you are, the status bar says what Acta is connected to.

- **Sidebar: one tree, every project.** Top-level rows are projects, each collapsible. Under an expanded project, in this order: the Inbox row (`inbox` icon, count of open notes), the Knowledge row (`library` icon, count of entries in the project, unread dot when Claude Code has added some), the topics that belong to no epic, then the epics, each collapsible, with their topics. The project of the active page is expanded when that page opens; every other node stays as the user left it. Pinned at the bottom, outside the scroll, above a 1px `border.subtle` line: the Trash row (`trash-2`) and the Settings row (`settings`). There is no sidebar header and no "Projects" title. Width 260, resizable 200 to 400, hidden with `Mod+B`. Padding 8, rows inset 8, tree indent 16 per level (project 0, epic and loose topic 16, topic in an epic 32).
- **Page area: one page per tab.** A page is a topic, an epic, a project, an inbox, a knowledge base, a document or memory, the trash, settings or the shortcuts list. Every record page has the same skeleton: title, properties block, then its sections. Section order in a topic is fixed: title, properties (status, epic, folders, id, updated), current state, description, knowledge, timeline; the timeline is last because it is the only unbounded section. An epic page: title, properties (progress, folders, id, updated), description, knowledge, topic table. A project page: title, properties (progress, folders, updated), description, knowledge, topic table grouped by epic. The content column is centred with at least 32 of side padding and 32 above the title.
- **Tabs** (4.4), **tool windows** (4.5) and the **status bar** (4.6) are specified below.
- **Window**: default 1440 x 900, minimum **880 x 560**. Widths, dock positions and visibility of panes, open tabs and the tree's expansion are remembered per window in the preferences file (4.7).
- **Narrow behaviour**: panes give way in a fixed order, sidebar first, then the page, never a tool window's 400 minimum. If sidebar + 420 + right dock no longer fit, the sidebar auto-hides (it returns when space returns; `Mod+B` still shows it as an overlay on top of the page, 260 wide, with `shadow.overlay`, dismissed by `Esc` or by opening something). At 880 with the right dock open the split is page 420 / dock 460. Nothing ever scrolls horizontally except code blocks, wide tables and an overfull tab bar.

### 4.3 Titlebar (client-side, 36 high, `bg.sunken`, bottom border `border.subtle`)

Content, left to right after the platform inset: sidebar toggle (`panel-left`); back and forward (`arrow-left`, `arrow-right`; compact ghost buttons, disabled at the ends of the active tab's history); the breadcrumb of the active page; drag region; "Search" button showing the palette shortcut; chat toggle (`message-circle`; Windows and macOS only); terminal toggle (`square-terminal`). A toggle whose tool window is visible shows `fill.selected` and its icon in `text.primary`. The whole bar is a drag region except the controls; double-click follows the platform (zoom on macOS, maximize elsewhere).

**Breadcrumb**: `type.small`, parts separated by " / " in `text.tertiary`: "Acta / Terminal / Embed the terminal". Every part but the last is a link (`text.tertiary`, `text.primary` and underlined on hover and focus) to that project or epic page; the last part is the page itself, in `text.secondary`, not a link. It names the page once, here, so pages carry no breadcrumb of their own. When it does not fit, the last part truncates at its end; under 240 of available width the middle parts collapse into one "…" part that opens a menu of them.

| Platform | Window controls | Rules |
|---|---|---|
| macOS | Native traffic lights, left, inset 12 from the left edge, vertically centred | Content starts at x = 76. In full screen the lights disappear and content starts at x = 12. Never redraw the lights. |
| Windows | Minimize, maximize/restore, close on the right, each 46 x 36, drawn by the app with 10px 1px-stroke glyphs | Hover `fill.hover`, pressed `fill.pressed`; close hover `caption.close.hover` with a white glyph. Hit-test regions must report as native caption buttons so Snap Layouts appears on maximize hover. 1px `border.subtle` window outline when not maximized. |
| Linux | Client-side controls on the right in the same style as Windows, 36 x 36 each | Honour the desktop's button layout and side when the platform reports it. When the compositor provides server-side decorations (tiling window managers), keep the system titlebar and render this bar as a plain 36 toolbar without controls. |

When the window is inactive, titlebar text and icons drop to `text.tertiary`. Nothing else changes.

### 4.4 Tabs

Tabs earn their place only when a second page is open, so that is when they appear. With one page there is no tab bar and nothing to manage; navigating (tree, palette, links, back and forward) replaces the page in the active tab, and each tab keeps its own history.

- **Opening a tab**: `Mod+Enter` on any row or palette result, `Mod`+click, middle click, or "Open in new tab" in a row menu. A page that is already open in a tab is focused there instead of being opened twice.
- **Tab bar**: 32 high, `bg.sunken`, bottom border `border.subtle`, across the page area only (not over the docks). Shown when two or more tabs exist.
- **Tab** (height 32, padding 0 x 12, gap 8, min width 120, max 220): page icon 14 in `text.secondary` (the status glyph for a topic, `layers` for an epic, `folder` for a project, `file-text` or `pin` for knowledge, `inbox`, `library`, `trash-2`, `settings`), title in `type.ui` `text.secondary`, truncated; trailing slot 16: a ghost `x` on hover and on the active tab, or the 6 `accent` unread dot when another process changed the page while the tab was in the background. Active: fill `bg.base`, so the tab joins its page, no bottom border under it, title `text.primary`. No accent line: tone and text weight carry the state. Hover: `fill.hover`. Keyboard focus ring inset. Dragging reorders, as tree rows do.
- **Overflow**: tabs shrink to 120, then the bar scrolls horizontally and a ghost `chevron-down` at its right end opens a menu of all tabs.
- **Keys**: `Mod+W` closes the active tab (the last remaining tab cannot be closed; the key then does nothing), `Ctrl+Tab` / `Ctrl+Shift+Tab` go to the next and previous tab on every platform. Palette: "Close tab", "Close other tabs", "Reopen closed tab".
- Tabs belong to the window, not to a project: two tabs may show pages of different projects. They are restored at launch.

### 4.5 Tool windows

A tool window is something that runs beside the page and keeps running when hidden. There are three: the tree (left, described above, no header), the **terminal** and the **chat** (ADR 0009). The terminal and the chat live in a dock: **right** (default for both; width 648, range 400 to 60% of the window) or **bottom** (under the page area; height 320, range 160 to 70% of the window height).

- **One tool window per dock at a time.** Showing the chat in the dock where the terminal is visible hides the terminal, and the reverse; both go on running. To see both, move one to the bottom ("Move terminal to bottom", "Move chat to bottom", and the reverse commands). Size and dock are remembered per tool window.
- **Toggle**: the shortcut shows and focuses the tool window; pressed while it has focus, it hides it and returns focus to the page. `Mod+Shift+Enter` zooms the focused tool window over the page area and back.
- **Header** (28 high, `bg.sunken`, bottom border `border.subtle`, padding 0 x 8, gap 8): the name in `type.label` `text.secondary` ("Terminal", "Chat"), a context text in `type.small` `text.tertiary`, then right-aligned compact ghost icon buttons (24), the last of which always hides the tool window (`minus`). The header is a drag handle for nothing; docks change by command.
- **Terminal header**: the context names what the session is on, which is the page it was started from: "on topic Embed the terminal" (the name is a link). Buttons: Restart here (`rotate-cw`), hide. One session per project: the dock shows the session of the active page's project, and moving to a page of another project shows that project's session and leaves the first running. Navigation never restarts or moves a session. When the session was started from another topic, epic or project page than the active one, or when the page's primary effective folder is no longer the one the session started in, the context text turns `text.primary` and a ghost text button "Restart here" appears next to it; nothing else marks the mismatch. "Restart terminal here" (that button, the icon button, and the palette) ends the session and starts it with the active page's folders and focus; with a process running it is a confirmed case of 6.4 ("Restart the terminal? Claude Code is still running in it.", Cancel, Restart).
- **Chat header and states**: see Chat panel in 5.
- Dividers between the page and a dock are resize handles (5).

### 4.6 Status bar

24 high, `bg.sunken`, top border `border.subtle`, full window width, `type.small`, `text.tertiary`. Items are ghost controls as tall as the bar with padding 0 x 8 (hover `fill.hover` and `text.secondary`, keyboard focus ring inset). It answers two questions that had no home, is Claude Code wired to this work and which Acta is this, and it takes the update indicator out of the titlebar. It never shows messages, counts, a clock or progress; those would make it a second place to look.

- **Left**: the project of the active page (`folder` 14 and its name; activating it is "Switch project", `Mod+O`), then the **Claude Code status**.
- **Right**: the update indicator (5), then the version ("Acta 0.4.1", `tnum`; opens Settings at About).
- **Claude Code status**: a glyph 14 and words, never colour alone. It reports what `acta setup --check` (ADR 0005) reports, checked at launch, after setup, and when the window regains focus, plus one fact about the active project.

| State | Glyph | Text | Activating it |
|---|---|---|---|
| Setup incomplete | `circle-dashed` in `text.tertiary` | "Claude Code: not set up" | Opens Settings at Claude Code |
| The `claude` command was not found | `circle-slash` in `text.tertiary` | "Claude Code: not found" | Opens Settings at Claude Code |
| Set up, but the active project has no working folder, so sessions started outside Acta cannot find it | `circle` in `text.secondary` | "Claude Code: no folder for this project" | "Set working folder..." |
| Set up and the project has a folder | `circle-check` in `text.secondary` | "Claude Code: ready" | Opens Settings at Claude Code |

The tooltip of the ready state gives the last write: "Last wrote 2 min ago in Embed the terminal". Whether a session is live right now is not in the model; see 11.

### 4.7 Preferences file

Theme, language, pane sizes and dock positions, the open tabs (record ids only, never titles), the tree's expansion state, the terminal font size and the one-time flags (sign-in help dismissed, export suggestion shown) live in `preferences.json` in Acta's data directory, not in the encrypted database: they must be readable before the database opens, so that first run and the key-loss screen come up in the user's theme and language. The file holds no user content and no key material; if it is missing or unreadable, Acta starts with the defaults (System theme, English) and says nothing.

## 5. Components

States are listed as default / hover / pressed / focus / disabled unless noted. **Focus** always means: a 2px `focus.ring` outline, offset 1px outside the control's edge, same radius plus 1, shown only for keyboard focus (not after a click). Rows show focus as the ring inset by 1px.

**Button** (height 28, padding 0 x 12, `radius.sm`, `type.ui`, optional 16 icon with gap 8; icon-only is 28 x 28, or 24 x 24 compact)

| Variant | Default | Hover | Pressed | Disabled | Use |
|---|---|---|---|---|---|
| Primary | fill `accent`, text `text.on-accent`, `type.ui-strong` | `accent.hover` | `accent.pressed` | fill `fill.selected`, text `text.disabled` | At most one per view or dialog |
| Secondary | no fill, 1px `border.strong`, text `text.primary` | `fill.hover` | `fill.pressed` | border `border.default`, text `text.disabled` | Alternative actions |
| Ghost | no fill, no border, text `text.secondary` | `fill.hover`, text `text.primary` | `fill.pressed` | text `text.disabled` | Toolbars, rows, icon buttons |
| Destructive | fill `danger`, text `text.on-accent` | fill at 90% opacity | fill at 80% opacity | as primary | Only inside a confirmation dialog |

Destructive actions outside a dialog are ghost buttons or menu items with `danger` text. Buttons never change size between states and never show a spinner; a slow action disables the button and reports through a toast.

**Text input** (height 28, padding 0 x 8, `radius.sm`, fill `bg.raised`, 1px `border.strong`, `type.ui`, placeholder `text.tertiary`, caret `accent` 1.5px, no blink animation easing: on 530 ms / off 530 ms, solid while typing). Hover: unchanged. Focus: border becomes `focus.ring` at 2px (drawn inside, no layout shift). Invalid: border `danger`, message below in `type.small` `danger` with `triangle-alert` 14. Disabled: border `border.default`, text `text.disabled`.

**Inline title edit**: titles (topic, epic, project) are edited in place in their own type role with no box; a 1px `border.strong` underline appears while editing. `Enter` commits, `Esc` reverts.

**Multi-line editor** (description, current state, timeline composer, note capture): raw Markdown in `type.reading`, fill `bg.raised`, 1px `border.strong`, `radius.md`, padding 12 x 16, minimum 3 lines (72), grows with content to 60% of the viewport height, then scrolls. Focus as text input. Footer row inside the box (28): left, context control (composer: entry type segmented control); right, hint `Mod+Enter to save` in `type.small` `text.tertiary` and the primary button. `Mod+Enter` commits, `Esc` cancels (if the draft is non-empty it is kept and restored next time; no "discard?" dialog). `Tab` indents when the caret is in a list item and moves focus to the next control otherwise. Read mode to edit mode: `Enter` or `E` on the focused block, or a click on its text; the block does not move.

**List / tree row** (height 28, padding 0 x 8, `radius.sm`, gap 8): disclosure chevron 16 (projects and epics; `chevron-right` / `chevron-down`), status glyph 14 (topics), title `type.ui` `text.secondary` truncated, trailing slot (unread dot 6 `accent`, or count in `type.small` `text.tertiary` with `tnum`). Hover: `fill.hover`, title `text.primary`. Selected (the item open in main): `fill.selected`, title `text.primary` at weight 600. Keyboard cursor: focus ring; cursor and selection are separate until `Enter`. Dragging: row at 60% opacity, 2px `accent` insertion line between rows, `accent.subtle` fill on an epic that would receive the drop. Renaming: the title becomes an inline edit. Epic rows use `type.label`. Project rows use `type.ui-strong` in `text.primary`, with a disclosure chevron and no icon. The Inbox and Knowledge rows of a project carry their icon and a count. Hover on a project or epic row reveals a compact ghost `plus` in the trailing slot ("New topic", in that epic or project); the count or dot moves left of it. `Mod+Enter`, `Mod`+click or middle click opens the row's page in a new tab. The "3 closed" row is a ghost row in `type.small` `text.tertiary`. On a selected row the count is `text.secondary` (2.2).

**Status indicator**: glyph only (14, tree, tabs and tables) or glyph 16 plus word in `type.ui` (the Status property, menus). In a topic's properties it is a ghost button (height 28, padding 0 x 8) opening the status menu; shortcut `S`. The menu lists the six statuses in the table order with number hints 1 to 6; the current one has a trailing `check`.

**Segmented control** (height 28, 1px `border.default`, `radius.sm`, segments padding 0 x 12, `type.ui` `text.secondary`): selected segment `fill.selected`, `type.ui-strong`, `text.primary`; hover `fill.hover`; arrows move between segments when focused. Used for the timeline filter (All / Progress / Decisions / Discussion), the composer entry type, the inbox filter (Open / Filed), the topic table filter (Open / All / Closed) and the theme setting. It never switches pages; page tabs are a different thing (4.4).

**Timeline entry**: a two-column block, gutter 20 + gap 12, then content.
- Gutter: the author mark, 20 x 20, top-aligned with the header line. **Human**: circle, `fill.selected`, initial in 12px weight 600 `text.primary` ("A" for Alexandre). **Claude Code**: square with `radius.sm`, no fill, 1px `border.strong`, Lucide `chevron-right` 12 in `text.secondary` (a prompt mark).
- Header (line 20): author name, then entry type, then time. Human name in `type.ui-strong`; "Claude Code" in `type.code` `text.primary`. Circle plus sans versus square plus mono is the at-a-glance distinction: shape and typeface, no colour, no badge. Type: icon 14 plus word in `type.small` `text.secondary`. Time: `type.small` `text.tertiary`, `tnum`, relative under 7 days ("2 h ago", "Tue 14:05"), absolute after ("12 Nov 2026"); tooltip gives full local date and time.
- Body: rendered Markdown, 4 below the header.
- By type: **Progress** (`git-commit-horizontal`): plain. **Decision** (`signpost`): body and header sit in a 1px `border.default` box, `radius.md`, padding 12 x 16, and the word "Decision" is `text.primary` weight 600; decisions are the entries people come back for, so they are the only ones with an outline. **Discussion** (`message-square`): plain.
- **System event** (status change, creation, moved): one line, no author mark, a status glyph 14 in the gutter, `type.small` `text.tertiary`: "Alexandre set status to Blocked - 2 h ago". Consecutive events within 5 minutes collapse into one line.
- States: hover reveals a ghost `ellipsis` button at the top right (also `Shift+F10` / context-menu key on the focused entry) with Edit, Copy as Markdown, Change type, Delete; focused: ring around the entry; editing: body becomes the multi-line editor in place; newly arrived from another process: `accent.subtle` background fading out (see 6.5).

**Current-state panel**: `bg.raised`, 1px `border.subtle`, `radius.md`, padding 16. Label row: "Current state" in `type.label` `text.secondary`, right-aligned meta "Updated 2 h ago by Claude Code" in `type.small` `text.tertiary`. Body `type.reading`, intended to stay under about 6 lines; over 12 lines it clamps with a "Show all" ghost button. Empty: placeholder text "Where does this stand? One or two sentences." in `text.tertiary`.

**Note card**: `bg.raised`, 1px `border.subtle`, `radius.md`, padding 12 x 16. Body `type.reading`, clamped to 6 lines when not focused. Meta row (`type.small` `text.tertiary`): time, and for filed notes "Filed as" plus topic links. Hover or focus reveals ghost icon buttons: Make topic (`corner-down-right`), Edit (`pencil`), Delete (`trash-2`). Focused: focus ring. Filed: body in `text.secondary`.

**Knowledge section** (project, epic and topic views; ADR 0007). Holds the two kinds of knowledge entry: **documents** (long-form reference) and **memories** (one durable fact in a few lines, mostly written by Claude Code, fully owned by the human). It is a section in the main column, not tabs and not a side column: tabs would hide either the timeline or the knowledge behind a click and add state to every topic; a side column would compete with the terminal for width and vanish at 880. A section keeps one scroll and one reading order, and costs 28px when empty.
- Header row (28): "Knowledge" in `type.label` `text.secondary`, count in `type.small` `text.tertiary`, and at the right a ghost `plus` button opening a two-item menu: New document (`Mod+Shift+D`), New memory (`Mod+Shift+M`).
- **Knowledge row** (height 32, padding 0 x 8, `radius.sm`, gap 8): kind icon 16 in `text.secondary` (`file-text` for a document, `pin` for a memory), title in `type.ui` `text.primary` (truncates, at most 40% of the row), 12 gap, summary in `type.ui` `text.secondary` (fills the rest, truncates), author mark 20 (the circle / square marks of the timeline, for the last author), updated time in `type.small` `text.tertiary` `tnum`, right-aligned in a fixed 72 column. Kind is the leading shape, author is the trailing shape: two columns, each answering one question, no colour and no badges. When the column is under 520 the summary moves to a second line and the row is 52. Hover `fill.hover`; keyboard focus ring; unread dot (6, `accent`) left of the author mark when another process created or changed the entry, which is how the human notices what Claude Code has just memorised. The summary is always shown and never optional: it is what both readers pick by.
- **A document opens; a memory unfolds.** `Enter` or a click on a document row opens the document reading view. `Enter`, `Right` or a click on a memory row expands it in place: under the row, aligned with the title (left inset 32), the body in `type.reading`, then a meta row in `type.small` `text.tertiary` ("Written by Claude Code 2 h ago · edited by Alexandre 10 min ago") with ghost buttons Edit `E`, History `H`, Move `M`, Delete. `Left` or `Esc` folds it. One memory is expanded at a time. A memory longer than 12 lines clamps with "Open", which shows it in the reading view like a document; the tooltip then suggests making it a document ("Change kind" in the row menu converts either way, keeping history).
- **Editing a memory** happens in place: title as an inline title edit, summary as a text input ("One line: the fact, in short"), body in the multi-line editor, `Mod+Enter` saves, `Esc` cancels. The human's edit is what the agent reads from then on; the meta row says who wrote and who last edited.
- **Row menu** (`Shift+F10`, right click, or the `ellipsis` shown on hover): Open, Edit, Show history, Move to epic "Terminal", Move to project "Acta" (the two nearest other levels, one key press each), Move to..., Change kind, Save as file, Delete. Dragging a row onto a tree row moves it too. Moving changes the owner only; the entry keeps its id and history, and the toast reads "Moved to project Acta. Undo".
- **Looking up, on a topic (and an epic)**: the view's own entries first, documents before memories, each by last update. Inherited entries follow in groups headed "From epic Terminal" and "From project Acta" (`type.small` `text.tertiary`, 8 above), nearest level first, collapsed by default into one ghost row "9 more from epic and project" whose expanded state is remembered per view. Inherited rows are the same entry, not a copy.
- **Looking down, on an epic or a project**: after the own entries, one ghost row "31 more in the topics of this epic" (or "in this project's epics and topics") opens the knowledge base view scoped to it. The section itself never lists descendants: a roll-up belongs in a view that can filter.
- More than 6 own entries: show 6, then a ghost row "Show all 14" (opens the knowledge base view scoped to this owner). Empty: one line in `text.tertiary`, "Nothing here yet. Documents and memories for this topic appear here.", with the inherited row still below.

**Knowledge base view** (a page; the destination for "what does this project know"). Reached from the Knowledge row of a project in the tree (`Mod+L` for the active page's project), from the roll-up rows above, and scoped to an epic from that epic's section. Top to bottom in the 680 column: title "Knowledge" (`type.title`) with the scope under it in `type.small` `text.tertiary` ("Acta · 12 documents, 43 memories"); a search input (28, full width, `search` icon, placeholder "Search knowledge", focused with `/`); a filter row of three compact secondary dropdown buttons, "Level: All", "Kind: All", "Author: All" (menus: All / Project / Epics / Topics; All / Documents / Memories; All / Alexandre / Claude Code), a non-default filter showing its value in `text.primary` weight 600, and a ghost "Clear" when any is set; then the list.
- The list is grouped by owner, widest first: "Project Acta", "Epic Terminal", "Topic Embed the terminal" (group header 28: `type.small` `text.tertiary`, the name a link to the owner, count at the right). Rows are knowledge rows and behave identically (documents open, memories unfold, same menu). `J` / `K` cross groups; `Left` on a row jumps to its group header; `Enter` on a header folds the group.
- Searching is full text and instant (budget as the palette): groups with no match disappear, the summary is replaced by the matching excerpt with the matched words in `text.primary` weight 600, and the header line reads "7 results for "resize"". No results: "Nothing in this project mentions "resize"." Filters and search combine; both persist while the app runs and reset on restart.
- This view is also where the human reviews the agent's memory: "Author: Claude Code" plus "Kind: Memories" is one saved habit, so the palette has the command "Review memories written by Claude Code", which opens the view with those two filters, newest first, ungrouped.

**Document reading view** (a page, part of navigation history; the tree keeps the owner selected; a memory opened in full uses it unchanged). Top to bottom in the 680 column: title (`type.title`); summary (`type.reading` `text.secondary`); the properties block (Kind, Level, Id, Updated; see Properties block); a row of ghost buttons, Edit `E`, History `H`, and `ellipsis`: Go to heading, Save as file, Move, Delete; 24; a 1px `border.subtle` rule; 24; the body in the document setting of 3.3. No outline column: "Go to heading" (`Mod+Shift+O`) opens the palette scoped to this document's headings, with each section's size, which is the same outline the agent sees. `Space` / `Shift+Space` page, `Home` / `End` jump. Remote changes follow 6.5: the changed section gets the highlight, the viewport is anchored on the first visible block.

**Document editor**: the reading view switches in place, without moving the scroll position. Title becomes an inline title edit; summary becomes a text input with the placeholder "One line: what this is and when to read it" and a `tnum` counter in `type.small` `text.tertiary` ("64 / 140"); the body becomes a borderless raw-Markdown editor in `type.document` filling the column (no box: the page is the editor). A 44 bar is pinned to the bottom of the main pane (`bg.base`, top border `border.subtle`): left, "Editing" plus the draft state in `type.small` `text.tertiary`; right, Cancel (ghost, `Esc`) and Save (primary, `Mod+Enter`). Save is disabled with the hint "A document needs a title and a summary." until both exist. Every save keeps the previous version, so there is no confirmation on save and none on cancel (the draft is kept). Markdown editing aids: `Enter` continues lists, `Tab` / `Shift+Tab` indent, `Mod+B` / `Mod+I` wrap the selection in `**` / `*` (inside an editor these win over the global shortcuts); nothing else, no toolbar.

**History** (documents and memories alike; for a memory, `H` opens it in the reading view first): `H` replaces the body with the history, same view, same header. A list of revisions, newest first (rows of 32: author mark, "Claude Code" or name, what changed in `text.secondary` ("Replaced section Storage", "Edited", "Created from file"), lines added and removed with `tnum`, time); at most 6 rows visible, then scroll. Below the list, the selected revision rendered read-only, headed by a notice "Version from Tue 14:05 by Claude Code" with a segmented control (Text / Changes) and a secondary button "Restore this version". Changes shows the raw Markdown with a 16 gutter: `+` in `success` for added lines, `-` in `danger` for removed lines whose text is `text.secondary`; no background tints. Restoring writes the old text as a new revision on top, so it is undoable, needs no dialog, and shows the toast "Restored the version from Tue 14:05. Undo". `Esc` or `H` returns to reading.

**Working folders** (project, epic and topic views; ADR 0008; wireframes in 8.11). The folders Claude Code works in: the first is the **primary** folder, where a session starts; the others are **additional** folders it may also read and edit. Each level has its own ordered list. The **effective** folders of a view are its own if it has any, otherwise those of the nearest level above that has some. The surface is one row at rest, because folders are set once and read rarely; it unfolds in place into the list, like a memory.
- **Folder row** (height 28, `radius.sm`, gap 8; it is the value of the Folders row in the properties block of a topic, epic or project page, and when unfolded its list opens under that row, pushing the properties below it down): disclosure chevron 16 (`chevron-right` / `chevron-down`), `folder-open` 16 in `text.secondary`, the primary effective path in `type.code` `text.primary`, then with a gap of 12 the count of additional folders ("+2", `type.small` `text.tertiary`, `tnum`, absent at zero), then with a gap of 12 the origin label in `type.small` `text.tertiary`: "from epic Terminal" or "from project Acta", the name a link in the breadcrumb style to that view. No label means the folders are this view's own. The order of elements is fixed: chevron, icon, path, count, origin label, then state (the not-found words and their button), so the origin is shown in every state. Only the path truncates; the name in the label truncates at 24 characters. When the column is under 520, whatever follows the path (label, or the not-found words and their button) moves to a second line under it and the row is 52, as a knowledge row does; the same holds for list rows. Hover `fill.hover`; keyboard focus ring; a click, `Enter` or `Right` unfolds; the unfolded state is remembered per view.
- **Unfolded**: the row becomes a header ("Working folders" in `type.label` `text.secondary`, the count of effective folders in `type.small` `text.tertiary`), and the list follows, inset 24 so its icons sit under the header's label. In order: the view's own folders; the checkbox "Also include inherited folders" (only when the view has own folders and a level above has some; off by default, so a topic in its own worktree does not also get the main checkout); the inherited folders under a group line "From epic Terminal" (`type.small` `text.tertiary`, as in the knowledge section); a ghost button with `plus` 16, "Add folder..."; and one line in `type.small` `text.tertiary`: "Claude Code starts in the primary folder and may also read and edit the others."
- **Folder list row** (height 28, padding 0 x 8, `radius.sm`, gap 8): `folder-open` 16, the path in `type.code` (`text.primary` for the primary folder, `text.secondary` for additional ones), and in the trailing slot the word "Primary" in `type.small` `text.tertiary` on the first effective row only. Order is the whole meaning: first is primary, the rest are passed in list order. Hover `fill.hover` and a ghost `ellipsis` button (24); keyboard focus ring; dragging as a tree row (2px `accent` insertion line), within the own folders only. Keys are in 6.1. Row menu: Show in Explorer (Reveal in Finder / Show in file manager), Copy path, Make primary, Move up, Move down, Choose another folder..., then after a separator "Remove from this topic" (or epic, or project). Remove is not styled as destructive: it detaches the folder, changes nothing on disk, and shows the toast "embed-terminal removed from this topic. Undo" (toasts name a folder by its last segment, never the full path).
- **Inheritance, shown not explained.** A view without own folders unfolds to the group line and the inherited rows, which are read-only here (menu: Show in Explorer, Copy path, "Edit in epic Terminal"), then the ghost button "Set folder for this topic..." and the line "This topic uses the folders of epic Terminal. A folder set here replaces them for this topic only." Once the view has an own folder, the inherited ones stop applying and the checkbox appears, off: under it one line, "Not used here: 2 folders from epic Terminal." Checked, the line is replaced by the group and its rows, appended after the own folders as additional folders in their own order (the level above's primary is additional here); a folder present in both lists is shown once, in its own position. "Inherited" always means the effective folders of the level directly above, so an epic that itself includes its project's passes both on. The checkbox saves on toggle.
- **Nothing attached anywhere up the chain**: the row has no chevron and reads, in `type.ui` `text.tertiary` after the icon, "No working folder. The terminal will ask where to start.", with a ghost button "Set working folder..." at the right. `Enter` on the row opens the picker. In this one case the folder is attached to the project, whichever view the row is on, exactly as the terminal's prompt saves it; a topic or epic row then reads "from project Acta", which is how the user sees where it went.
- **Folder not found** (the path is not a folder on this computer: deleted, moved, an unplugged drive, or imported from another machine per ADR 0006). The icon becomes `folder-x` in `warning`, the path drops to `text.secondary`, and the trailing slot reads "Not found on this computer" in `type.small` `text.secondary` followed by the one repair action, a ghost button "Locate..." that is always visible, not hover-only. Locate opens the picker and replaces the path in place, keeping position, owner and primary rank; because it repairs the folder itself, it is offered wherever the folder appears, inherited rows included. An inherited folder keeps its origin: on the folded row the label stays between the count and the not-found words ("D:\work\acta  from epic Terminal  Not found on this computer  Locate..."), and in the list the group line names it. Locating an inherited folder repairs the record of the level that owns it, for every view that inherits it, so the result says so in a toast: "acta located for epic Terminal and everything that inherits from it. Undo". Locating an own folder shows no toast; the row changes in place. The folded row tells the two severities apart by position. A missing primary, which stops a launch, changes the leading icon to `folder-x` in `warning`, drops the path to `text.secondary` and shows the words and the button. A missing additional folder, which does not, leaves the leading icon, the path and their colours untouched and adds, 8 after the count, a `folder-x` 14 in `warning` and "1 not found" in `type.small` `text.secondary` ("+2  1 not found"), with no button: unfolding shows which one. Icon shape and words carry the state; `warning` is never the only signal. Nothing else is blocked: the topic, its knowledge and its timeline work as before, and the only other consequence is at terminal launch (below). Existence is checked off the UI thread when the view opens and when the window regains focus; until the answer arrives the row renders as found, so nothing flickers. A check still pending after 400 ms (a dead network path) puts "Checking..." in the trailing slot in `type.small` `text.tertiary`, per the loading rule of 6.3; it blocks nothing.
- **Repairing the rest.** After a Locate, Acta compares the old and the new path, takes the part that changed as the old and new root, and looks for other folders, at any level of any project, that are not found, sit under the old root and exist under the new one. If there are any, a Notice at the top of the main view (`info` icon) offers the same change: "7 other folders under D:\work were not found either. Apply the same change to them?", the old root in `type.code`, with ghost actions "Apply to 7 folders" and "Dismiss". Applying is one undoable step and answers with the toast "7 folders located. Undo". The count includes only folders the change would actually resolve, so the promise is exact; with none, no notice. This is what makes an import on another computer one Locate and one confirmation instead of one per folder.
- **Same folder on two topics** (allowed; also two epics). Under the folder's row, inset to the path, one line in `type.small` `text.secondary` with `info` 14: "Also attached to topic Scrollback. A session started in this folder outside Acta opens on epic Terminal and is told about both topics." For two epics: "Also attached to epic Notes. A session started in this folder outside Acta opens on project Acta and is told about both epics." The other owner's name is a link; with three or more, "Also attached to topics Scrollback, Links and 2 more." The level named is the nearest one the candidates share (their epic, otherwise the project). It is a line, not a Notice, and not a warning: sessions opened from Acta's terminal are unaffected, since the view they are opened from names the topic.
- **Choosing a folder.** Every "..." action above opens the platform's native folder picker (single selection, titled "Choose a working folder"), starting in the parent of the current primary effective folder, or the home folder. Cancelling changes nothing. "Add folder..." appends to the own list (the first own folder is primary); "Set working folder..." and "Set folder for this topic..." set the primary of the open view, replacing its current own primary and keeping its additional folders; the exception is the empty case above, where no level has a folder and the command attaches to the project. Choosing a folder already in the own list adds nothing and moves the keyboard cursor to its row. There is no typed-path field and no confirmation; every change is undoable (6.4) and appears in place, unfolding the row if it was folded. Changes made by another process (`acta link`) follow 6.5.
- **Path display.** Paths are `type.code`: they are the machine's strings, and mono keeps `l`, `I`, `1`, `\` and `/` unambiguous. Stored in the normal form of ADR 0008 (absolute, canonical, links resolved, no trailing separator, upper-case drive letter) and shown exactly as stored, with the separators of the platform that wrote them (`\` on Windows, `/` on macOS and Linux, UNC prefixes kept) and no trailing separator; a path imported from another platform keeps its foreign separators, which is part of why it reads as not found. The home folder is shortened to `~` on all three platforms (`~\dev\acta`, `~/dev/acta`). A path that does not fit is truncated in the middle with a single `…` (U+2026, never three dots, which read as a parent folder), by whole segments: the root (`~`, `C:`, `/`, `\\server\share`) and the last segment always stay, and as many trailing segments as fit are kept, since the end of a path is what tells two worktrees apart: `~\…\acta-wt\embed-terminal`. If that does not fit, the root goes (`…\embed-terminal`); if the last segment alone does not fit, it is cut at its end. The path slot is never narrower than 160. The full absolute path, without `~`, is the row's accessible name, what "Copy path" copies, and the path tooltip (see Tooltip) shown only when the row displays a shortened or truncated form. Paths in rows are not selectable text. Two paths are the same folder when their stored forms are equal, ignoring case on Windows and macOS: the one rule, shared with the CLI and hook resolution, behind "already in the list", "shown once" and "Also attached to...". The UI never compares the displayed strings.
- **Terminal launch.** Opening the terminal from a view starts Claude Code in that view's primary effective folder with the additional folders passed along. Two cases replace the terminal grid with a message set as an empty state at the top left of the pane (padding 12, sans, on the terminal's surface), focused, with ordinary keys since no shell exists yet:
  - *No folder anywhere up the chain.* Heading "Where should Claude Code start?"; sentence "Embed the terminal has no working folder, and neither has its epic or its project." (an epic: "and neither has its project"; a project: "Acta has no working folder yet."); a checkbox, checked, "Save as the working folder of project Acta"; under it in `type.small` `text.tertiary`, "If not saved, it is used until Acta quits."; then the primary button "Choose a folder...". The picker's result starts the session at once. This is asked once: saved, the project has a folder from then on; not saved, the choice holds for this project's terminal until Acta quits.
  - *The primary folder was not found.* Heading "The working folder was not found"; sentence "Claude Code was not started. This is the primary folder of epic Terminal, and it is not on this computer:"; the full path in `type.code`, selectable, wrapping at separators; then "Locate folder..." (primary; the same repair as in the list, then the session starts) and "Try again" (secondary; for a drive that was reconnected). Acta never starts the session in another folder on its own.
  - A missing *additional* folder does not stop the launch: it is left out, and a toast with `triangle-alert` in `warning` says "acta-docs was not found and was left out of this session. Locate". Like an error toast it stays until dismissed. Locate repairs the record; the running session started without the folder and is not touched, so the toast that follows reads "acta-docs located. Restart the terminal to include it." with the action "Restart terminal here".
  - While "Try again" is checking, both buttons are disabled; after 400 ms the line "Checking..." (`type.small` `text.tertiary`) appears under them until the answer arrives.
  - *The session is on something else.* One session per project stays, and navigating never restarts or moves it. The terminal's header always names what the session is on, and marks a mismatch with the active page; the rules and the "Restart terminal here" command are in 4.5.

**Properties block** (under the title of every record page; the model's own fields, shown as a page's typed properties). Rows of 28, two columns, no box and no dividers; 12 under the title, 24 above the next section. Label: `type.ui` `text.tertiary`, in a column exactly as wide as the longest label of the block in the current language (never a fixed width; capped at 40% of the column, beyond which a label truncates with a tooltip). Gap 16. Value: `type.ui` `text.primary`. A value that can be changed is a ghost control (padding 0 x 8, shifted left by 8 so its text aligns with plain values; hover `fill.hover`); a value that cannot is plain text. The block is one stop when stepping through a page with `Up` / `Down`; `Enter` goes inside, `Up` / `Down` move between rows, `Enter` acts on the value, `Esc` leaves. The direct keys (`S` status, `M` move) work from anywhere on the page.

| Page | Rows, in order |
|---|---|
| Topic | **Status**: the status indicator button. **Epic**: the epic's name, a link; `M` or the row's menu moves the topic; "None" in `text.tertiary` for a topic outside any epic. **Folders**: the working folders row. **Id**: "T-42" in `type.code` `text.secondary`, the short id Claude Code uses; activating it copies it and the value reads "Copied" for 1.5 s. **Updated**: "2 h ago by Claude Code" (time rules of the timeline; tooltip with the full date). |
| Epic | **Progress**: "3 of 7 topics done" with a bar (120 x 4, `radius.sm`, track `fill.selected`, fill `text.secondary`): topics Done over all topics not Dropped. **Folders**. **Id**: "E-7". **Updated**. |
| Project | **Progress** (same rule over the whole project). **Folders**. **Updated**. |
| Document or memory | **Kind**: "Document" or "Memory" (menu: Change kind). **Level**: the owner, "Topic Embed the terminal", a link (`M` moves the entry). **Id**: "K-7". **Updated**: "2 h ago by Claude Code". |

There are no user-defined properties, no dates to fill in and no empty rows to hide: every row is a field the model has for every record of that kind, so the agent and the human always see the same set.

**Topic table** (the collection on epic and project pages; the last section, under Knowledge). A toolbar row (28): the filter as a segmented control, Open / All / Closed (default Open), the count in `type.small` `text.tertiary` ("7 topics"), and at the right a ghost `plus` button, "New topic" (`Mod+T`). A header row (28, `type.small` `text.tertiary`, 1px `border.subtle` under it): Status (24, no label), Title (fills), Entries (56, right-aligned), Updated (88, right-aligned). Rows of 32, padding 0 x 8, `radius.sm`: status glyph 14, title `type.ui` `text.primary` truncated, the unread dot after the title, entry count and updated time in `type.small` `text.tertiary` with `tnum`. No lines between rows. Hover `fill.hover`; keyboard focus ring; the same keys as the tree (`Enter` open, `Mod+Enter` new tab, `S` status, `E` rename, `Alt+Up` / `Alt+Down` reorder, `Delete`). Closed topics render as in the tree (title `text.tertiary`).
- Default order is the tree's order, the one the human arranged. Activating a column header sorts by it (`chevron-up` / `chevron-down` 12 after the label) and a ghost "Reset order" appears in the toolbar; while sorted, reordering by drag or key is off.
- On a project page the table is grouped by epic: group rows of 28 (`type.label`, the epic's name as a link, "3 of 7 done" in `type.small` `text.tertiary` at the right), topics outside any epic first under "No epic". `Enter` on a group row folds it.
- Under 520 of column width the Entries column is dropped. Empty: the existing empty states ("No topics yet. ..." with "New topic").
- It is a table and only a table. A board was considered and left out (Direction).

**Slash menu** (in every multi-line editor). Typing `/` at the start of a line or after a space opens a menu at the caret: the context menu component, at most 8 rows, filtered as typing continues ("/co" leaves Code block), `Enter` inserts, `Esc` closes and leaves the typed text alone. Every item inserts Markdown and nothing else: Heading 1, Heading 2, Heading 3, Bulleted list, Numbered list, Task list, Quote, Code block, Table (a 3 x 2 skeleton), Divider. In the timeline composer a first group sets the entry type instead: Progress, Decision, Discussion. The stored text is plain Markdown; there are no blocks, so what the menu writes is exactly what Claude Code would have written.

**Chat panel** (a tool window; ADR 0009; Windows and macOS). It shows claude.ai, the website, in the system web view. Acta never reads from the page, writes to it or styles it, so the page keeps claude.ai's own theme whatever Acta's is.
- **Header** (4.5): name "Chat", context "claude.ai" (plain text; no logo of Anthropic or Claude anywhere). Buttons: Open in browser (`external-link`), Reload (`rotate-cw`), `ellipsis` (Sign-in help, Move to bottom / Move to right, Sign out and clear chat data...), hide. Tooltips of these buttons always open upward, away from the web view, which would cover them.
- **Toggle**: `Mod+Shift+H`, the titlebar toggle, palette "Toggle chat". `Mod+4` focuses it. Showing the panel focuses the page inside it; nothing else ever gives it focus.
- **Placement**: the right dock by default, sharing it with the terminal one at a time (4.5); move either to the bottom dock to see both.
- **States.** All but the live one are drawn by Acta on `bg.sunken` as an empty state at the top left of the panel (padding 16, text at most 480 wide).
  - *Not started* (the web view is created only when the user asks). Heading "Chat with Claude beside your work". Sentence: "This panel shows claude.ai, the website, in your system's web view. You sign in with your own account, and Acta does not read or change anything on the page." Then, in `type.small` `text.tertiary`: "Once started, the web view uses about 400 MB of memory, also while the panel is hidden." Buttons: "Open claude.ai here" (primary), "Open in your browser" (ghost).
  - *Starting*: the empty surface; after 400 ms one line, "Opening claude.ai...", in `type.small` `text.tertiary`.
  - *Live*: the web view fills the panel under the header. Acta draws nothing over it.
  - *Sign-in help*: a Notice at the top of the panel, above the web view, the first time the panel is live and whenever "Sign-in help" is chosen: `info`, "Sign in with email: the link in the message opens in your browser and shows a code; type that code here. Signing in with Google or Apple may be refused in this panel.", action "Got it".
  - *Under an overlay.* The web view is a native view above everything Acta draws, so while the palette, quick capture, a dialog, a menu or the sidebar overlay is open it is hidden. Its place shows `bg.sunken` and one line at the top left, "The chat is hidden while this is open.", in `type.small` `text.tertiary`. It returns at the same bounds, without animation, when the overlay closes. Toasts and tooltips are placed so they never reach into the panel.
  - *Web view missing* (Windows without the WebView2 runtime). Heading "The chat panel needs WebView2". Sentence: "Microsoft's WebView2 runtime is not installed on this computer, so Acta cannot show claude.ai here. Everything else in Acta works." Buttons: "Open claude.ai in your browser" (primary), "Get WebView2" (ghost; Microsoft's download page in the system browser), "Try again" (ghost).
  - *Could not open*: heading "claude.ai could not be opened here", the reason when known ("There is no connection."), buttons "Try again" (primary) and "Open claude.ai in your browser" (ghost). The panel never fails to a blank surface.
- **Sign out and clear chat data...** is a confirmed case of 6.4, because the profile cannot be brought back: "Sign out and clear chat data?", "This deletes the panel's cookies and site data from this computer. Your conversations stay in your Claude account.", Cancel, "Sign out and clear". Afterwards the panel is back to *Not started*.
- **Keyboard.** While the page has focus, keys belong to the page. These stay with Acta, routed from the web view: `Ctrl+Shift+K`, `Mod+Shift+H`, ``Ctrl+` ``, `Ctrl+Shift+J`, `Mod+1` to `Mod+4`, `F6`. ADR 0009 makes this a condition of shipping the panel.
- **Linux**: there is no panel and no titlebar toggle. "Open claude.ai in your browser" is a palette command and a row in Settings, and `Mod+Shift+H` runs it.

**Settings page** (a page like any other: `Mod+,`, the Settings row of the sidebar, or the palette; it opens in the active tab). Title "Settings". Sections are headed in `type.subtitle`, 32 apart. A **setting row** is at least 44 high with 8 of vertical padding: at the left the label in `type.ui` `text.primary` and under it a description in `type.small` `text.tertiary` (at most 420 wide, wraps); at the right the control. No boxes, no lines. Every setting applies the moment it changes, has no Save button, and exists as a palette command.

| Section | Rows |
|---|---|
| **Appearance** | Theme: segmented control System / Light / Dark; description "System follows your computer's setting." Language: dropdown English / Français; description "Changes Acta's own text. What you and Claude Code write is never translated." Applies at once, without a restart. Terminal text size: a stepper 11 to 18. |
| **Working folders** | "Turn off Claude Code's own memory in attached folders": checkbox, on by default; description "When a folder is attached, Acta writes autoMemoryEnabled: false to its .claude/settings.local.json, so that what Claude Code learns is kept in Acta, where you can read it." "Folders not found": present only when some exist; the count, then one folder list row per folder (path, owning page as a link, "Locate..."), at most 6 with "Show all 14". This is the one place that lists them across projects. |
| **Claude Code** | The four checks of `acta setup --check`, one row each, a glyph 14 and words: "Claude Code found", "Plugin files written", "Plugin installed", "Permission rule added"; `check` in `success` and "Done", or `circle-dashed` in `text.tertiary` and "Missing". Then "Set up Claude Code" (primary while anything is missing; afterwards a secondary "Run setup again") and a ghost "Remove from Claude Code". Description: "Setup installs Acta's plugin in Claude Code and adds one permission line to ~/.claude/settings.json. Remove undoes both." The path of the `acta` command in `type.code`, with "Copy path". |
| **Data** | "Export...", with "Last export: 12 days ago" or "Never exported". "Import...". "Restore from snapshot...", with "11 snapshots kept; the newest is from today, 09:12". "Open trash", with "23 items". "Database": its path in `type.code`, with "Show in Explorer". |
| **Chat** | Windows and macOS: "Sign out and clear chat data...". Linux: "Open claude.ai in your browser". |
| **About** | "Acta 0.4.1", "Release notes", "Keyboard shortcuts" (`Mod+/`), "Licences" (Inter and JetBrains Mono under the SIL Open Font License, Lucide under ISC). |

**Notice** (the one inline message component; used for edit conflicts, deleted-while-open, version headers, plaintext warnings, migration reports): full column width, `bg.raised`, 1px `border.subtle`, `radius.md`, padding 8 x 12, optional icon 16 (`info` in `text.secondary`, `triangle-alert` in `warning` or `danger`), message in `type.ui` `text.primary`, ghost actions right-aligned. Never a coloured fill, never dismisses itself, never more than one per view.

**Checkbox** (export options, the inherited-folders option and the terminal's save-folder option): 16 box, `radius.sm`, 1px `border.strong`, label `type.ui` 8 to its right; checked: fill `accent`, `check` 12 in `text.on-accent`; hover `fill.hover` inside the box; focus ring; disabled `border.default` and `text.disabled`.

**Export dialog** (palette: "Export...", also in settings). Dialog of 440. Title "Export". Scope: segmented control (Everything / This project). Then a notice with `triangle-alert` in `warning`: "The file is not encrypted. Anyone who can open it can read these projects. That is what lets you read it without Acta or its key." Then a checkbox "Protect with a passphrase"; when checked, two inputs (passphrase, repeat) and the line "Acta cannot recover a lost passphrase." in `type.small` `text.secondary`. Buttons: Cancel, "Choose location..." (primary; opens the system save dialog, default name `acta-2026-11-20.acta.json`). Result: toast "Exported 4 projects to acta-2026-11-20.acta.json. Show in folder". The same one-line warning appears on "Save document as file". Settings shows "Last export: 12 days ago" in `type.small`; once, after the tenth topic exists, a notice at the top of the main view suggests a first export ("Everything is stored on this computer only. An export is your copy if the system keychain is ever lost.", actions Export..., Dismiss) and never returns.

**Import** (palette: "Import..."): system open dialog, then, if the file is protected, a passphrase input; then the **import preview**, a dialog of 520. Title: Import "acta-2026-11-20.acta.json". Meta line in `type.small` `text.tertiary`: "Exported 20 Nov 2026 · format 1 · 4 projects". A table (see 8.8): one row per kind (Projects, Epics, Topics, Entries, Notes, Documents, Memories), three `tnum` columns Created / Updated / Skipped, zeros in `text.tertiary`. Below: "Nothing here will be deleted. Skipped records are the same as, or older than, what Acta already has." and, when Updated is not zero, a ghost "Show what changes" expanding the titles of updated records (max 240 high, scrolls). Buttons: Cancel (focused), Import. Nothing is written before Import; Acta takes a snapshot first, so the result line reads "Imported: 12 created, 3 updated. A snapshot from before the import is kept." Working folders in the file belong to the computer that exported it; those that do not exist here are kept and add one sentence to the result: "3 working folders were not found on this computer. Locate each one from its project, epic or topic." Refusals replace the table with one notice and a single Close button: "This file was made by a newer Acta (format 3). Update Acta, then import it." / "This is not an Acta export." / "Wrong passphrase."

**Trash view** (palette: "Open trash"; a page). Title "Trash", line "Deleted things stay here for 30 days." in `type.small` `text.tertiary`. Rows of 32 grouped by day of deletion (group headers `type.small` `text.tertiary`): kind icon 16 (`folder`, `layers`, status glyph, `message-square`, `sticky-note`, `file-text`, `pin`), title `type.ui`, original place in `type.small` `text.tertiary`, "by Claude Code" or the name, days left ("28 days left", `tnum`). `Enter` or `R` restores to the original place and shows "Topic restored. Undo"; if its epic is also in the trash, both come back and the toast says so. A project row counts its content ("31 topics, 12 notes"). No "empty trash" and no "delete now" in v1: the trash exists so that nothing is ever lost by a key press, and a purge button is exactly such a key press. Empty: "Trash is empty."

**Snapshot restore** (palette: "Restore from snapshot..."): dialog of 520 listing snapshots, newest first, rows of 32: date and time (`tnum`), kind ("Daily", "Weekly", "Before update to 0.4.1", "Before import", "Before restore"), size. Selecting one enables the destructive-styled button "Restore and restart" and shows: "Acta will restart with its data as of 18 Nov 2026, 09:12. Everything as it is now is saved as a snapshot first, so you can come back." This is one of the confirmed cases in 6.4. After restart a notice reads "Restored the snapshot from 18 Nov 2026, 09:12." If a migration fails after an update, Acta restores by itself and the next launch shows the notice "The update to 0.4.1 could not convert the database. Acta put back the snapshot from just before; nothing was lost." with the action "Details".

**Key-loss recovery screen** (replaces the whole window content, like first run; see 8.9). Shown when the database file exists but its key is not in the system keychain. Calm and exact: no red, no icon larger than 16, no "error" in the heading. It states what is true (the file is intact, unreadable, untouched), why it usually happens, and offers three actions in order of safety: "Import an export..." (primary), "Try again" (secondary; re-reads the keychain, for a locked or slow keychain), "Start empty" (ghost; the unreadable file is renamed and kept, the screen says its new name). Acta never creates a database over the old one and never deletes the kept file.

**Command palette**: width 640 (or window width minus 48), top edge 96 below the window top, `bg.overlay`, 1px `border.default`, `radius.lg`, `shadow.overlay`, no scrim (the user keeps seeing what they are acting on). Input row 44: `search` icon 16, text `type.reading`, optional scope chip (height 24, `accent.subtle`, `radius.sm`, `type.small` `text.primary`: "Topics", "Projects", "Status"). Results: up to 10 rows of 32, grouped under `type.label` headers in `text.tertiary` (Topics, Knowledge, Commands, Projects; a knowledge row shows its kind icon, its title, and its summary in place of the path); row = icon or status glyph 16, title `type.ui`, path in `type.small` `text.tertiary` ("Acta / Terminal"), right-aligned shortcut hint. Matched characters are `text.primary` weight 600 on a `text.secondary` title. Selected row `fill.selected`; the first row is always selected; hover does not move the selection until the pointer actually moves. Footer 28, `border.default` top, hints in `type.small`: "Enter open", "Mod+Enter open in new tab", "Esc close". This is Acta's search everywhere: there is no second search box for navigation, and the scope chip (`Tab` on a row) is how it narrows to topics, knowledge, commands or projects. No results: one line, "Nothing matches "oauth". Enter creates a topic with this title."

**Context menu / dropdown menu**: `bg.overlay`, 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4, min width 200, max 320. Item 32 (28 is too tight for a pointer menu), padding 0 x 8, icon 16 optional, label `type.ui`, shortcut hint right. Hover and keyboard highlight are the same single `fill.selected` row. Destructive item: `danger` text, last, after a 1px `border.default` separator with 4 above and below. Disabled: `text.disabled`. Type-ahead selects by first letters. Submenus only for "Move to epic".

**Tooltip**: `bg.overlay`, 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4 x 8, `type.small` `text.primary`, optional kbd hint after the label. Appears after 500 ms of hover or immediately on keyboard focus, 6 away from the target, never over it; once one has shown, neighbours show instantly until the pointer rests elsewhere for 1 s. One line, no wrapping, no tooltip repeating a visible label. The one exception is the **path tooltip** (working folders): the full path in `type.code` at 12 / 16, at most 480 wide (or the window width minus 32), wrapping only after a separator.

**Toast**: bottom-left of the page area, 16 from the edges and above the bottom dock if one is open; never over a tool window (the chat's web view would cover it), width 360 max, `bg.overlay`, 1px `border.default`, `radius.md`, `shadow.overlay`, padding 8 x 12, `type.ui`. Optional leading icon 16 (`check` in `success`, `triangle-alert` in `danger` or `warning`), message, one ghost action ("Undo") with its kbd hint, then a ghost `x`. Stays 6 s (errors stay until dismissed), pauses while hovered or focused; at most one visible, a new one replaces the old. Toasts exist for undo and for failures only.

**Dialog**: centred, width 440, `bg.overlay`, 1px `border.default`, `radius.lg`, `shadow.overlay`, over `scrim`. Padding 24; title `type.heading`; body `type.ui` `text.secondary`, 8 below the title; buttons right-aligned, 24 below the body, gap 8, the confirming action last (first on Windows, following the platform). Focus starts on the safe button; `Esc` cancels; `Enter` activates the focused button only. Focus is trapped inside.

**Empty state**: left-aligned at the top of the content column, never centred in the pane, never illustrated. Heading `type.heading`, one sentence in `type.reading` `text.secondary` (max 480 wide), then the one action that fixes it as a button with its kbd hint. See 8.5.

**Scrollbar**: overlay, 8 wide track with no fill, thumb 4 wide (8 when hovered or dragged), fully rounded, `border.strong` at 50% opacity (100% hovered), minimum length 32. Appears while scrolling or when the pointer is within 12 of the edge, hides 800 ms after. Never takes layout space.

**Resize handle**: the 1px `border.subtle` divider between panes with an invisible 8 hit area centred on it. Hover for 150 ms or drag: the line becomes 2px `accent` and the cursor changes to the resize cursor. Dragging resizes live, every frame, clamped to the pane limits; double-click restores the default. Keyboard: palette commands "Widen sidebar", "Narrow sidebar", "Widen terminal", "Narrow terminal" (steps of 40).

**Keyboard shortcut hint (kbd)**: each key in its own box: height 18, min width 18, padding 0 x 4, `radius.sm`, 1px `border.default`, `type.small` with `tnum`, `text.tertiary`, gap 2 between keys (the sans, not the mono, which lacks the key glyphs). macOS shows glyphs (`⌘ ⇧ ⌥ ⌃ ↵`), Windows and Linux show words (`Ctrl`, `Shift`, `Alt`, `Enter`). Inside a primary button the hint has no border and uses `text.on-accent` at weight 400.

**Update indicator** (a status bar item, at the right, before the version): hidden when up to date and while downloading; updates are fetched silently and applied at next launch. **Ready**: 6 dot in `accent` plus "Restart to update" in `type.small` `text.secondary`; click or palette command restarts into the new version and restores the window, project, topic and terminal session. **Just updated** (first launch on a new version): "Updated to 0.4.1" in `type.small` `text.tertiary` for that session, opening the release notes; no dot. **Failed** three launches in a row: `triangle-alert` 14 in `warning` plus "Update failed"; click shows the reason and a "Download manually" link. Never a dialog, never a toast, never a badge on anything else.

## 6. Interaction

### 6.1 Keyboard map

| Action | Windows / Linux | macOS |
|---|---|---|
| Command palette (everything) | `Ctrl+K`, also `Ctrl+Shift+K` | `Cmd+K` |
| Go to topic (palette scoped to topics) | `Ctrl+P` | `Cmd+P` |
| Switch project (palette scoped to projects; opens the project page) | `Ctrl+O` | `Cmd+O` |
| Capture a note (from anywhere) | `Ctrl+N` | `Cmd+N` |
| New topic (in the current epic) | `Ctrl+T` | `Cmd+T` |
| Add a timeline entry to the current topic | `Ctrl+E` | `Cmd+E` |
| Go to inbox | `Ctrl+I` | `Cmd+I` |
| Open knowledge (palette scoped to documents and memories in scope, nearest level first) | `Ctrl+D` | `Cmd+D` |
| Go to the project's knowledge base | `Ctrl+L` | `Cmd+L` |
| New document / new memory (owned by the open project, epic or topic) | `Ctrl+Shift+D` / `Ctrl+Shift+M` | `Cmd+Shift+D` / `Cmd+Shift+M` |
| Go to heading (in a document) | `Ctrl+Shift+O` | `Cmd+Shift+O` |
| Back / forward in the active tab's history | `Alt+Left` / `Alt+Right` | `Cmd+[` / `Cmd+]` |
| Open in a new tab (on a row or a palette result) | `Ctrl+Enter` | `Cmd+Enter` |
| Close tab | `Ctrl+W` | `Cmd+W` |
| Next / previous tab | `Ctrl+Tab` / `Ctrl+Shift+Tab` | `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Toggle terminal (show and focus; if focused, hide) | ``Ctrl+` `` or `Ctrl+Shift+J` | ``Ctrl+` `` or `Cmd+Shift+J` |
| Toggle chat (same rule; on Linux, opens claude.ai in the browser) | `Ctrl+Shift+H` | `Cmd+Shift+H` |
| Zoom the focused tool window over the page area | `Ctrl+Shift+Enter` | `Cmd+Shift+Enter` |
| Toggle sidebar | `Ctrl+B` | `Cmd+B` |
| Focus sidebar / page / terminal / chat | `Ctrl+1` / `2` / `3` / `4` | `Cmd+1` / `2` / `3` / `4` |
| Cycle panes | `F6` / `Shift+F6` | `F6` / `Shift+F6` |
| Save the editor in focus | `Ctrl+Enter` | `Cmd+Enter` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Shift+Z` or `Ctrl+Y` | `Cmd+Z` / `Cmd+Shift+Z` |
| Settings | `Ctrl+,` | `Cmd+,` |
| Keyboard shortcuts list | `Ctrl+/` | `Cmd+/` |

Inside lists and the tree (no modifier, only when a text field is not focused): `Up` / `Down` or `K` / `J` move; `Home` / `End`; `Left` / `Right` collapse / expand or go to parent / first child; `Enter` open; `Mod+Enter` open in a new tab; `E` or `F2` rename or edit; `S` set status, then `1` to `6`; `Alt+Up` / `Alt+Down` reorder; `M` move to epic; `Delete` (or `Cmd+Backspace`) delete with undo; `Shift+F10` or the menu key opens the context menu. In the topic view, `Up` / `Down` step through blocks (title, current state, description, composer, each entry) and `Enter` edits the focused block. Type-ahead in the tree jumps to a title.

In the working folders list (the folder row is one of the blocks `Up` / `Down` step through): `Enter` or `Right` on the row unfolds it and moves to the first folder, `Left` or `Esc` folds it. On any folder: `Enter` reveals it in the file manager, selected in its parent (where the Linux desktop cannot reveal, it opens the folder itself); on a folder that was not found `Enter` is Locate; `Mod+C` copies its full path; `Shift+F10` or the menu key opens its menu. On an own folder only: `Alt+Up` / `Alt+Down` reorder (the folder that reaches the top becomes primary), `P` makes it primary, `E` or `F2` is "Choose another folder...", `Delete` (or `Cmd+Backspace`) removes it from this level with undo. On an inherited, read-only folder those keys do nothing. `Space` toggles the inherited-folders checkbox when the cursor is on it, and does nothing elsewhere in the list. Adding has no key of its own: the "Add folder..." button is the last stop of the list, and "Set working folder..." is in the palette. Folders are set once per piece of work, so they take no global shortcut.

**The terminal owns the keyboard when focused.** On Windows and Linux every `Ctrl+letter` goes to the shell (`Ctrl+K`, `Ctrl+P`, `Ctrl+N`, `Ctrl+E`, `Ctrl+B` are readline keys). Reserved for the app even there: `Ctrl+Shift+K` (palette), ``Ctrl+` `` and `Ctrl+Shift+J` (terminal), `Ctrl+Shift+H` (chat), `Ctrl+1/2/3/4`, `Ctrl+Tab`, `F6`, `Ctrl+Shift+Enter`, plus `Ctrl+Shift+C` / `Ctrl+Shift+V` for copy and paste. `Ctrl+W` goes to the shell there, as it should. That is why `Ctrl+Shift+K` exists everywhere: one palette key that always works. On macOS all `Cmd` shortcuts keep working and `Ctrl` goes to the shell.

**Keyboard layouts.** Shortcuts on letters match the character the layout produces. Shortcuts on digits and on the backtick match the physical key (the US position), because on a French AZERTY keyboard the digit row yields symbols and the backtick needs AltGr; `Ctrl+Shift+J` exists so that the terminal toggle never depends on that key. If the pinned GPUI cannot match by position, the digit shortcuts are dropped on those layouts rather than made unreliable: `F6` and the palette cover them.

**Two meanings of `Mod+Enter`, never at once.** In an editor it saves; on a row, a link or a palette result it opens in a new tab. An editor has no rows and a list has no draft, so the context always decides.

**The palette is the universal entry point.** Every command above, every topic, epic and project, every status change ("Set status: Blocked"), layout command and setting is reachable from `Mod+K`. Each row shows its direct shortcut, so the palette teaches the keyboard. With a topic open, typing a status name or "entry" ranks actions on that topic first.

Palette commands for knowledge and data (exact labels): "Open document or memory...", "Go to knowledge base", "Search knowledge...", "Review memories written by Claude Code", "New document", "New memory", "New document from file...", "Edit", "Show history", "Restore this version", "Go to heading...", "Save as file...", "Move to...", "Change kind to memory" / "Change kind to document", "Export...", "Import...", "Open trash", "Restore from snapshot...". A trailing ellipsis means the command asks for something before acting.

Palette commands for the workbench (exact labels): "Open settings", "Theme: System", "Theme: Light", "Theme: Dark", "Language: English", "Language: Français", "New project...", "Close tab", "Close other tabs", "Reopen closed tab", "Open in new tab", "Toggle sidebar", "Toggle terminal", "Toggle chat", "Move terminal to bottom" / "Move terminal to right", "Move chat to bottom" / "Move chat to right", "Restart terminal here", "Open claude.ai in your browser", "Sign out and clear chat data...", "Set up Claude Code", "Keyboard shortcuts".

Palette commands for working folders (exact labels): "Set working folder...", "Add working folder...", "Show working folders", and, with a folder focused, "Make primary folder", "Move folder up", "Move folder down", "Choose another folder...", "Locate folder...", "Copy folder path", "Show folder in Explorer" (macOS: "Reveal folder in Finder"; Linux: "Show folder in file manager"), "Remove folder from this topic" (or epic, or project), "Include inherited folders" / "Stop including inherited folders", "Apply the same change to 7 folders" (while that offer is open), "Restart terminal here". They act on the open project, epic or topic, and on the project when another view is open; each row's path, in the format of every palette row, names the target ("Acta / Terminal / Embed the terminal" for a topic, "Acta / Terminal" for an epic, "Acta" for the project), so the level is never a guess. While no level has a folder, "Set working folder..." shows "Acta" from any view, because that is where it will attach.

**Quick capture** (`Mod+N`): a palette-sized overlay (640 wide, same position and surface) containing one multi-line editor with the placeholder "Note for Acta" and the project name as a chip. `Mod+Enter` saves to that project's inbox and closes, with no toast (the inbox count increments, which is feedback enough); `Esc` closes and keeps the draft. Works while the terminal is focused via the palette. Capture never navigates away from what the user was reading.

**Quick entry** (`Mod+E`): focuses the composer at the top of the timeline, scrolling it into view if needed, with the type preset to Progress; `Alt+1/2/3` switches Progress / Decision / Discussion; `Mod+Enter` saves and returns focus to where it was.

### 6.2 Focus

- Exactly one pane is active. The active pane's selection uses `fill.selected`; in inactive panes the same fill stays and the keyboard ring is hidden. Nothing else marks the active pane.
- Focus is always visible when moved by keyboard (2px `focus.ring`). Pointer clicks do not draw rings on buttons and rows but always do on text fields.
- Overlays (palette, capture, menu, dialog) take focus on open and return it to the exact previous element on close, including the caret position in an editor.
- Opening a topic from the tree with `Enter` moves focus to the page; with `Space` it previews (opens in the active tab, focus stays in the tree).
- Nothing takes focus on its own: not a toast, not the update indicator, not content written by Claude Code, not the terminal producing output.
- `Esc` always goes one level out: close overlay, then cancel edit, then clear multi-selection, then nothing. It never navigates.

### 6.3 Motion and latency

Budget, measured from input event to presented frame on a mid-range laptop:

| Interaction | Budget |
|---|---|
| Key press or click to visual feedback (selection, hover, caret, typed character) | next frame, 16 ms at most |
| Open a topic, switch view, toggle a pane, open the palette | 50 ms, no intermediate state |
| Palette results after each key | 16 ms target, 30 ms at most, over 10,000 topics |
| Switch project | 100 ms |
| Switch tab, theme or language | 50 ms, no intermediate state |
| Launch to interactive window with the last topic open | 400 ms target, 800 ms at most |
| Terminal key echo | next frame |

Nothing blocks the UI thread on disk, decryption or the network. If something exceeds its budget the old content stays in place until the new content is ready; a loading indicator (one line of `type.small` `text.tertiary` text, "Opening..." ) may appear only after 400 ms.

**Never animates**: hover, selection, focus ring, navigation between views, tree expand and collapse, pane show, hide and resize, typing, scrolling to a keyboard target, theme switch, language switch, tab switch, status change.

**Animates** (the complete list):

| What | Motion | Duration, easing |
|---|---|---|
| Palette, capture, menu, tooltip, dialog in | opacity 0 to 1, plus 4px upward translate for palette and dialog | 100 ms, ease-out |
| Same, out | opacity only | 60 ms, linear (input is already live underneath) |
| Toast in / out | opacity plus 8px vertical translate | 140 ms ease-out / 100 ms linear |
| Remote-change highlight | `accent.subtle` background to transparent | holds 600 ms, fades 900 ms, linear |
| Drag reorder | neighbouring rows shift to open the gap | 100 ms, ease-out |

Typing into an overlay that is still fading in works from the first frame. With the OS "reduce motion" setting, everything in the table becomes instant except the remote-change highlight, which becomes a 1.5 s static tint.

### 6.4 Writes, undo, confirmation

- **Optimistic, always.** The UI updates in the same frame as the input and persists in the background. A failed write restores the previous value in place and shows an error toast with "Retry"; the user's text is never lost (it goes back into the editor).
- **Autosave** on commit (`Mod+Enter`, `Enter` for single-line fields, or focus leaving the field). There is no Save button for whole views and no dirty indicator. Drafts of uncommitted editors survive navigation and restarts.
- **Undo** (`Mod+Z`) covers, per project and across navigation, the last 100 of: create, edit, delete, move, reorder, status change, note filing, and every working-folder change (set, add, remove, reorder, locate, include inherited). Deleting a project, epic, topic, note, entry, document or memory removes it immediately and shows a toast "Topic deleted" with Undo; deleted things stay in the trash view for 30 days. Saving, moving or restoring a document or memory is undoable too, since every save keeps the previous version. Undo applies to the human's own actions only; it never reverts something Claude Code wrote (those are edited or deleted explicitly).
- **Confirmation dialogs** exist only for what `Mod+Z` cannot take back: closing or restarting a terminal, or quitting, while a process is running in it, signing out of the chat panel (its profile is deleted), importing (the preview is the confirmation), and restoring a snapshot. Deleting a project is not one of them: it goes to the trash like everything else. Export is a dialog because it has options and a warning, not because it is dangerous. The dialog names the object and the consequence, and its confirming button repeats the verb ("Delete project").

### 6.5 Changes written by Claude Code (or any other process)

The database is watched; changes land in the UI within 100 ms of the write. The rules, in priority order:

1. **Focus and caret never move.** No pane activates, no view switches, even if the changed topic is not the open one.
2. **The viewport never shifts.** Lists and the timeline anchor on the first visible item: when entries are inserted above it, the scroll offset is compensated in the same frame. If the timeline is scrolled to the top (within 8px), new entries appear in place at the top; otherwise a ghost button "2 new entries" (`type.small`, `bg.overlay`, `border.default`, `radius.sm`) is pinned at the top of the timeline viewport; activating it, or pressing `Home`, scrolls there.
3. **What changed is marked, quietly.** A new or changed block visible on screen gets the remote-change highlight once. A topic changed while not open gets the 6 `accent` unread dot on its tree row (and its collapsed epic shows the dot) until opened. The inbox count increments. No toast, no sound, no badge on the window.
4. **A field being edited is never overwritten.** If the human is editing the current state and Claude Code updates it, the editor keeps the human's text and a notice line appears under it: `triangle-alert` 14 in `warning`, "Claude Code changed this while you were editing." with ghost actions "Show theirs" (side by side, read-only) and "Keep mine". Saving without choosing keeps the human's version and records the other as a Discussion entry by Claude Code, so nothing either side wrote is lost.
5. **Structure changes do not reorder under the pointer.** Rows created, moved or closed by another process appear in their place immediately, but if the pointer or keyboard cursor is in that list, the row under the cursor keeps its screen position (same anchoring as rule 2).
6. **Deleted while open.** If the open topic is deleted by another process, the view stays readable with a notice line at the top ("This topic was deleted by Claude Code.") and a "Restore" button.

## 7. Voice

Plain, specific, short. Acta talks like a careful colleague's notes: it names the object, says what happened and what to do next, and stops.

- Sentence case. No exclamation marks, no "please", no "oops", "sorry", "successfully", "simply", no emoji.
- Verb plus object for actions ("New topic", "Delete entry"); the same word for the same thing everywhere (topic, epic, project, entry, note, inbox, current state; never "task", "ticket", "card", "item", "log").
- The agent is always "Claude Code", never "AI", "the assistant" or "the bot". The human is their name ("Alexandre"), never "You" in attributions and "you" only in instructions.
- Errors say what failed, why if known, and the next step. Never blame, never a bare code.
- Numbers and times are exact and short: "3 open", "2 h ago", "12 Nov 2026". Counts above 99 show "99+".
- Empty states say what the place is for and offer the one action that fills it.
- Every string exists in English and in French (10). The rules above hold in both; French adds its own typography and addresses the user as "vous". What the user and Claude Code write is shown as written, in whatever language they wrote it.

| Where | Before | After |
|---|---|---|
| Button | Submit | Add entry |
| Button | Create New Topic | New topic |
| Menu | Remove | Delete topic |
| Toast | Successfully deleted! | Topic deleted. Undo |
| Empty inbox | No notes yet! Start capturing your brilliant ideas. | Inbox is empty. Notes you capture land here until they become topics. |
| Empty timeline | Nothing to see here. | No entries yet. Add one, or let Claude Code log progress as it works. |
| Error | Error: write failed (code 5) | Could not save the entry. The database is locked by another process. Retry |
| Error | Invalid input | A topic needs a title. |
| Conflict | Conflict detected! | Claude Code changed this while you were editing. |
| Dialog title | Are you sure? | Restore the snapshot from 18 Nov 2026? |
| Dialog body | This action cannot be undone. | Acta will restart with its data as of 18 Nov 2026, 09:12. Everything as it is now is saved as a snapshot first. |
| Export warning | WARNING: unencrypted export! | The file is not encrypted. Anyone who can open it can read these projects. |
| Key loss | Fatal error: decryption failed | Acta cannot open its database. The file is intact, but its key is missing from the system keychain. |
| Import result | Import completed with 9 conflicts | Imported: 12 created, 3 updated. 9 skipped because Acta already has the same or a newer version. |
| Missing folder | Error: invalid path | Not found on this computer. Locate... |
| No folder | No directory configured! | No working folder. The terminal will ask where to start. |
| Attribution | Updated by AI 2 hours ago | Updated 2 h ago by Claude Code |
| Placeholder | Enter description here... | What is this topic about, and what does done look like? |

## 8. Key screens

Widths are for the default 1440 x 900 window. Every screen exists in both themes; the wireframes show structure, which is the same in both. Sidebars drawn in 8.2, 8.7 and 8.10 are excerpts from inside an expanded project node. `( )` is a status glyph, `[ ]` a control, `·` a separator rendered as 8 of space plus the next item, not a character.

### 8.1 Main window, topic page

```
+------------------------------------------------------------------------------------------------+
| [=] [<] [>]  Acta / Terminal / Embed the terminal             [Search  Ctrl K] [o] [>_] - [] X | 36  titlebar
+----------------------+-------------------------------------------------------------------------+
| v Acta               |                               32                                        |
|     [i] Inbox      4 |        Embed the terminal bound to the project   <- type.title 24/30    |
|     [k] Knowledge 55 |    12                                                                   |
|     (o) Naming       |        Status    [(.) Active v]            <- properties block: rows 28,|
|   v Core UI          |        Epic      Terminal                    label column as wide as its|
|       (v) App shell  |        Folders   > [f] ~\dev\acta-wt\embed-terminal  +1   longest label |
|       (.) Sidebar t..|        Id        T-42                      <- type.code, copies on Enter|
|       ( ) Palette   *|        Updated   2 h ago by Claude Code                                 |
|   v Terminal         |    24                                                                   |
|      [(.) Embed th..]|        +--------------------------------------------------------+       |
|       (=) Scrollback |        | Current state                                          |       |
|       (-) Links      |        | Rendering and input work on Windows. Resize still      |       |
|       3 closed       |        | drops the last row; fix is in progress.                |       |
|   > Notes            |        +--------------------------------------------------------+       |
| > Atlas              |    24   bg.raised, border.subtle, radius.md, padding 16, type.reading   |
| > Field notes        |        Description                                  <- type.label       |
|                      |        Run Claude Code inside Acta, in the project directory, so        |
|                      |        the tracker and the agent share one window.                      |
|                      |    32                                                                   |
|                      |        Knowledge  3                                          [+]        |
|                      |    4   [d] Resize protocol   How the grid reflows; read b..  [>] 2 h ago|
|                      |        [m] ConPTY minimum    Needs Windows 10 1809 or later  [>] Mon    |
|                      |        9 more from epic and project          <- ghost row, collapsed    |
|                      |    32                                                                   |
|                      |        Timeline            [All|Progress|Decisions|Discussion]          |
|                      |    12  +--------------------------------------------------------+       |
|                      |        | Add an entry...                                        |       |
|                      |        +--------------------------------------------------------+       |
|                      |    24                                                                   |
|                      |        [>] Claude Code  Progress  2 h ago    <- square mark, mono name  |
|                      |            Resize now reflows the grid; the last row is still cut       |
|                      |            when the panel is under 12 rows.                             |
|                      |    24                                                                   |
|----------------------|        (A) Alexandre  Decision  Tue 14:05    <- circle mark, sans name  |
|  [t] Trash           |            (.) Alexandre set status to Active  Tue 13:58  <- system     |
|  [s] Settings        |                                                                         |
+----------------------+-------------------------------------------------------------------------+
| [p] Acta   (v) Claude Code: ready                                                   Acta 0.4.1 | 24  status bar
+------------------------------------------------------------------------------------------------+
  260 sidebar, bg.sunken        1180 page area, bg.base; content column 680 centred (250 each side)
  rows 28, indent 16            [..] around a tree row = fill.selected; * = unread dot (accent, 6)
```

Annotations: one tab is open, so there is no tab bar. The tree lists every project; "Acta" is expanded because the open page belongs to it, "Atlas" and "Field notes" are folded. `[i]` is `inbox`, `[k]` is `library`, `[t]` `trash-2`, `[s]` `settings`, `[p]` `folder`; in the titlebar `[=]` is the sidebar toggle, `[o]` the chat toggle, `[>_]` the terminal toggle. Tree glyphs: `(o)` Draft, `( )` Planned, `(.)` Active, `(=)` Blocked, `(v)` Done, `(-)` Dropped. The page names itself in the titlebar breadcrumb and nowhere else. `> [f]` is the folded working-folders row (5, 8.11), here as the value of the Folders property. The composer is one line (28 + padding) until focused, then becomes the multi-line editor. Scrolling: title and properties scroll with the content; nothing is sticky except the "new entries" button. In the status bar, `(v)` is the `circle-check` glyph of the ready state.

### 8.2 Notes inbox

```
+----------------------+-------------------------------------------------------------------------+
|  [Inbox          4]  |                               32                                        |
|                      |        Inbox                                         <- type.title      |
|  (o) Naming          |    8   4 notes · Acta                                <- type.small      |
|  v Core UI           |    24                                                                   |
|    ...               |        +--------------------------------------------------------+       |
|                      |        | Note for Acta                                          | 72    |
|                      |        |                              Ctrl Enter to save  [Add] |       |
|                      |        +--------------------------------------------------------+       |
|                      |    24                                                                   |
|                      |        [Open|Filed]                      [File with Claude Code]        |
|                      |    12                                                                   |
|                      |        +--------------------------------------------------------+       |
|                      |        | Terminal should remember its width per project, not    |       |
|                      |        | globally.                                              |       |
|                      |        | 10 min ago                              [->] [e] [x]   |       |
|                      |        +--------------------------------------------------------+       |
|                      |    8                                                                    |
|                      |        +--------------------------------------------------------+       |
|                      |        | Idea: weekly digest of decisions across all topics.    |       |
|                      |        | Yesterday 18:42                                        |       |
|                      |        +--------------------------------------------------------+       |
+----------------------+-------------------------------------------------------------------------+
```

Annotations: the capture editor is focused when the inbox opens. Newest note first. Card actions appear on hover or focus only (shown on the first card): Make topic `T`, Edit `Enter`, Delete `Delete`. "File with Claude Code" is the single secondary button: it shows the terminal and runs the filing command there; resulting topics arrive as Draft with unread dots, and their notes move to Filed with links. With the Open filter empty the button is disabled. `J` / `K` move between cards.

### 8.3 Command palette

```
                    |<------------------------- 640 ------------------------->|
            96 from +---------------------------------------------------------+
            window  | (s)  term                                               | 44  input, type.reading
            top     +---------------------------------------------------------+
                    | Topics                                                  | 28  group label
                    |[(.) Embed the terminal        Acta / Terminal          ]| 32  selected: fill.selected
                    | (=) Terminal scrollback       Acta / Terminal           | 32
                    | (v) Terminal spike            Acta / Research           | 32
                    | Commands                                                | 28
                    |  >_ Toggle terminal                            Ctrl `   | 32
                    |  >_ Move terminal to bottom                             | 32
                    +---------------------------------------------------------+
                    | Enter open   Ctrl Enter open in new tab       Esc close | 28  footer, type.small
                    +---------------------------------------------------------+
                      bg.overlay, border.default, radius.lg, shadow.overlay, no scrim
```

Annotations: empty query shows "Recent" (the last 5 topics) then the 5 most relevant commands for the current view. Group order is fixed (Topics, Commands, Projects) so results do not jump between keystrokes; within a group, ranking is match quality then recency. `Up` / `Down` wrap; `Tab` narrows to the group of the selected row (adds the scope chip); `Backspace` on an empty input removes the chip.

### 8.4 Terminal tool window beside a topic

```
+------------------------------------------------------------------------------------------------+
| [=] [<] [>]  Acta / Terminal / Embed the terminal          [Search  Ctrl K] [o] [>_ on] - [] X | 36
+----------------------+-------------------------------------+-----------------------------------+
| v Acta               |  32                                 | Terminal  on topic Embed..[r] [-] |
|   v Terminal         |  Embed the terminal bound to        |-----------------------------------|
|      [(.) Embed th..]|  the project                        |  > claude                         |
|                      |  Status   [(.) Active v]            |                                   |
|                      |  Epic     Terminal                  |  * Fixing the resize path in      |
|                      |  Folders > [f] ~\…\embed-terminal +1|    crates/terminal/src/grid.rs    |
|                      |  Id       T-42                      |                                   |
|                      |  Updated  just now by Claude Code   |  * acta log "Resize fixed"        |
|                      |  +-------------------------------+  |                                   |
|                      |  | Current state                 |  |  > _                              |
|                      |  | Resize is fixed on Windows.   |  |                                   |
|                      |  +-------------------------------+  |                                   |
|                      |  ^ remote-change highlight, once    |                                   |
+----------------------+-------------------------------------+-----------------------------------+
| [p] Acta   (v) Claude Code: ready                                                   Acta 0.4.1 | 24
+------------------------------------------------------------------------------------------------+
  260                    532 page (min 420), column fills     648 right dock = 80 cols, bg.sunken
                         it with 32 side padding              header 28, then the grid, padding 12
```

Annotations: the terminal is a tool window in the right dock (4.5). Its header names what the session is on ("on topic Embed the terminal", truncated here) and holds Restart here `[r]` and hide `[-]`; the titlebar toggle shows `fill.selected` while it is visible. The divider is a resize handle. With the dock open the column is 468 wide, under the 520 threshold: the folder row's path is middle-truncated, and a row that carries a label or a not-found state takes its two-line form (8.11). The session starts in the primary working folder of the page it was opened from, with the additional folders passed along (8.11 shows what the tool window says when there is no folder or the folder is gone); when the session ends, the grid shows one line, "Session ended. Enter to restart.", in `text.tertiary`, and restarting uses the folders of the page open at that moment. The current-state update written by Claude Code appears in the page with the highlight while focus stays in the terminal.

### 8.5 First run

```
+------------------------------------------------------------------------------------------------+
| Acta                                                                              - [] X       | 36
+------------------------------------------------------------------------------------------------+
|                                        (no sidebar until a project exists)                     |
|                                                                                                |
|                96  Start with a project                              <- type.title             |
|                12  A project is a folder you work on with Claude Code. Acta keeps its          |
|                    topics, decisions and notes next to the work.     <- type.reading, 480 max  |
|                24  [Choose a folder  Ctrl O]                         <- primary, 28            |
|                32  Then                                              <- type.label             |
|                 8  1  Connect Claude Code        acta setup          <- type.ui + type.code    |
|                 4  2  Capture a first note       Ctrl N                                        |
|                 4  3  Open the terminal          Ctrl `                                        |
|                                                                                                |
+------------------------------------------------------------------------------------------------+
  content column 680 centred; everything left-aligned inside it; no logo, no illustration
```

Annotations: the three steps are plain text rows in `text.secondary` (not a checklist to tick, not a wizard); they become available as soon as the project exists and disappear for good once the project has one topic. Choosing a folder creates the project named after the folder, attaches the folder as the project's primary working folder, and opens its inbox with the capture editor focused. Other empty states follow the same pattern at the top of their column: empty project ("No topics yet. Capture notes and let Claude Code file them, or start one yourself." [New topic]), empty epic, empty timeline, no search results.

### 8.6 Topic view, knowledge section expanded

```
        Knowledge  3                                                           [+]     28  type.label + count
     4  [d] Resize protocol    How the grid reflows; read before touching it   [>]  2 h ago   32
        [m] ConPTY minimum     Needs Windows 10 1809 or later                  [>]  Mon       32  expanded:
               ConPTY is missing before Windows 10 1809. Acta refuses to start the      <- type.reading,
               terminal there and says why; do not add a winpty fallback.                  inset 32
               Written by Claude Code Mon 16:20 · edited by Alexandre 10 min ago
               [Edit] [History] [Move] [Delete]                                         <- ghost
        [m] One session        One terminal per project by decision            (A)  Tue       32
     8  From epic Terminal                                         <- type.small, tertiary
        [d] Terminal research  What alacritty_terminal gives us and what not   (A)  12 Nov    32
        [m] Shell detection  * Use COMSPEC on Windows, SHELL elsewhere         [>]  11 Nov    32
     8  From project Acta
        [d] Architecture       Crates, data flow, where things live            (A)  9 Nov     32
        [m] Sample data        Sample and test data uses the name Alexandre    [>]  3 Nov     32
    32
        Timeline               [All|Progress|Decisions|Discussion]
        |icon|<- title, max 40% ->|<- summary, fills, truncates ->|<- mark ->|<- time 72 ->|
```

Annotations: `[d]` is the `file-text` icon, `[m]` the `pin` icon; `[>]` last written by Claude Code, `(A)` by Alexandre; `*` is the unread dot. Own entries first, then nearest level. In the epic and project views the same section sits under the description and ends with the roll-up row ("31 more in the topics of this epic") instead of, or after, inherited groups; a project has nothing to inherit.

### 8.7 Document reading view

```
+----------------------+-------------------------------------------------------------------------+
|  Inbox            4  |                               32                                        |
|  v Terminal          |    32   (the breadcrumb is in the titlebar)                             |
|   [(.) Embed the..]  |        Resize protocol                           <- type.title 24/30    |
|    (=) Scrollback    |     8  How the grid reflows; read before touching it  <- reading, secondary |
|                      |    12  Kind      Document                        <- properties block    |
|                      |        Level     Topic Embed the terminal                               |
|                      |        Id        K-7                                                    |
|                      |        Updated   2 h ago by Claude Code                                 |
|                      |    12  [Edit] [History] [...]                    <- ghost               |
|                      |    24  ----------------------------------------------------------       |
|                      |    24                                                                   |
|                      |        Goals                                     <- h2, type.heading    |
|                      |     8  The grid must reflow on every resize without losing the          |
|                      |        scrollback position. Three cases matter:  <- type.document 16/26 |
|                      |    16                                                                   |
|                      |          -  Width shrinks: soft-wrapped lines rewrap.                   |
|                      |          -  Height shrinks: rows move into scrollback.                  |
|                      |    32                                                                   |
|                      |        Sequence                                                         |
|                      |     8  +--------------------------------------------------------+       |
|                      |        | resize(cols, rows) -> pty.resize -> grid.reflow        | code  |
|                      |        +--------------------------------------------------------+       |
|                      |    16  Step        Owner      Budget                                    |
|                      |        ---------------------------------------- <- border.subtle rows   |
|                      |        Measure     view       1 ms                                      |
|                      |        Reflow      grid       4 ms                                      |
+----------------------+-------------------------------------------------------------------------+
  260                           column 680 centred; nothing in the margins; no outline column
```

Annotations: the tree keeps the owning topic selected. Edit turns this same page into the editor (title, summary and body become editable in place; a 44 Save bar appears at the bottom of the pane). History replaces everything under the rule with the revision list and the selected version. `Alt+Left` returns to the topic at its previous scroll position.

### 8.8 Import preview

```
            +------------------------------------------------------------+
            | Import "acta-2026-11-20.acta.json"                          |  type.heading
            | Exported 20 Nov 2026 · format 1 · 4 projects                |  type.small
         16 |                                                            |
            |                    Created     Updated     Skipped         |  type.small, tertiary
            | Projects                 1           0           3         |  rows 28, tnum,
            | Epics                    2           1           9         |  zeros in tertiary
            | Topics                   7           2          22         |
            | Entries                 31           0         148         |
            | Notes                    4           0           8         |
            | Documents                2           1           6         |
            | Memories                 9           2          31         |
         16 |                                                            |
            | Nothing here will be deleted. Skipped records are the same |  type.ui, secondary
            | as, or older than, what Acta already has.                  |
            | Show what changes                                          |  ghost
         24 |                                        [Cancel]  [Import]  |
            +------------------------------------------------------------+
              520, bg.overlay, radius.lg, padding 24, over scrim; focus starts on Cancel
```

### 8.9 Key-loss recovery

```
+------------------------------------------------------------------------------------------------+
| Acta                                                                              - [] X       |
+------------------------------------------------------------------------------------------------+
|                96  Acta cannot open its database                     <- type.title             |
|                12  The database is on this computer and intact, but its key is missing         |
|                    from Windows Credential Manager. This usually follows a system              |
|                    reinstall, a new user profile or a move to another machine.                 |
|                12  Acta has not changed the file:                    <- type.reading           |
|                 4  C:\Users\Alexandre\AppData\Roaming\Acta\acta.db   <- type.code, selectable  |
|                24  [Import an export...]  [Try again]                <- primary, secondary     |
|                24  Without the key or an export, the content cannot be read. You can           |
|                    start empty; the file is kept as acta.db.unreadable-2026-11-20.             |
|                 8  Start empty                                       <- ghost                  |
+------------------------------------------------------------------------------------------------+
  column 680 centred, left-aligned; no sidebar, no icon, no red; keychain name per platform
```

Annotations: the keychain is named as the platform names it (Windows Credential Manager, macOS Keychain, the Secret Service keyring). "Import an export..." leads to the normal import preview, where every row is Created. The sentence about what cannot be done is stated once, plainly, and is not softened.

### 8.10 Knowledge base (project)

```
+----------------------+-------------------------------------------------------------------------+
|  Inbox            4  |                               32                                        |
| [Knowledge      55]  |        Knowledge                                 <- type.title          |
|                      |     8  Acta · 12 documents, 43 memories          <- type.small          |
|  (o) Naming          |    24  +--------------------------------------------------------+       |
|  v Core UI           |        | (s) Search knowledge                                   | 28    |
|    ...               |        +--------------------------------------------------------+       |
|  v Terminal          |    12  [Level: All v] [Kind: Memories v] [Author: Claude Code v]  Clear  |
|    ...               |    24                                                                   |
|                      |        Project Acta                                          14         |
|                      |        [m] Sample data       Sample and test data uses th..  [>] 3 Nov  |
|                      |        [m] Commit style      Imperative subject, no trailers [>] 2 Nov  |
|                      |    16  Epic Terminal                                          6         |
|                      |        [m] Shell detection * Use COMSPEC on Windows, SHEL..  [>] 11 Nov |
|                      |    16  Topic Embed the terminal                               2         |
|                      |        [m] ConPTY minimum    Needs Windows 10 1809 or later  [>] Mon    |
|                      |               ConPTY is missing before Windows 10 1809. Acta refuses    |
|                      |               to start the terminal there and says why.                 |
|                      |               Written by Claude Code Mon 16:20                          |
|                      |               [Edit] [History] [Move] [Delete]                          |
+----------------------+-------------------------------------------------------------------------+
  260; Knowledge row selected     column 680 centred; groups widest level first; rows as in 8.6
```

Annotations: the view with two filters set (kind and author). Set filters show their value in `text.primary` weight 600. Group names link to the project, epic or topic. A memory unfolds in place, so the human can read, correct, move or delete what the agent recorded without leaving the list; a document row opens 8.7.

### 8.11 Working folders

The folder row at rest. It is the value of the Folders property, so the label "Folders" stands to its left (not drawn here). The cases:

```
        > [f] ~\dev\acta-wt\embed-terminal  +1                               28  own folders: no label

        > [f] ~\dev\acta  +1  from epic Terminal                             28  inherited: label, name is a link

        > [x] D:\work\acta  Not found on this computer  [Locate...]          28  primary missing: folder-x, warning

        > [x] D:\work\acta  from epic Terminal  Not found on this computer  [Locate...]   inherited and missing

        > [f] ~\dev\acta-wt\embed-terminal  +2  [x] 1 not found              28  additional missing: launch not blocked

          [f] No working folder. The terminal will ask where to start.   [Set working folder...]
        |chev|icon|<- path, type.code, middle-truncated, min 160 ->|12|count|12|<- label ->|12|<- state ->|
```

The same rows when the column is under 520 (the normal case with the terminal docked: 468). What follows the path and its count moves to a second line aligned with the path, and the row is 52:

```
        > [f] ~\dev\acta  +1                                         52
              from epic Terminal

        > [x] D:\work\acta                                           52
              from epic Terminal  Not found on this computer  [Locate...]

        > [f] ~\…\embed-terminal  +2                                 52
              [x] 1 not found
        |<------------------------ 468 ------------------------>|
```

Unfolded on a topic that has its own folders and includes its epic's:

```
        v Working folders  4                                                 28  type.label + count
            [f] ~\dev\acta-wt\embed-terminal                        Primary  28  type.code, text.primary
            [f] ~\dev\acta-docs                                              28  additional: text.secondary
                (i) Also attached to topic Scrollback. A session started in this folder
                    outside Acta opens on epic Terminal and is told about both topics.
            [x] D:\work\design-assets    Not found on this computer  [Locate...]
            [v] Also include inherited folders                               28  checkbox, checked
         8  From epic Terminal                                 <- type.small, tertiary
            [f] ~\dev\acta                                                   28  read-only here
            [+ Add folder...]                                                28  ghost
         8  Claude Code starts in the primary folder and may also read and edit the others.
    24
        +--------------------------------------------------------+
        | Current state                                          |
```

Unfolded on a topic with nothing of its own:

```
        v Working folders  2
         8  From epic Terminal
            [f] ~\dev\acta                                          Primary
            [f] ~\…\clients\alexandre\acta-design-assets
            [Set folder for this topic...]
         8  This topic uses the folders of epic Terminal. A folder set here replaces them
            for this topic only.
```

The terminal pane when there is nothing to start in, and when the primary folder is gone:

```
+-----------------------------------+      +-----------------------------------+
| 12                                |      | 12                                |
|  Where should Claude Code start?  |      |  The working folder was not found |  type.heading
|  8                                |      |  8                                |
|  Embed the terminal has no        |      |  Claude Code was not started.     |  type.reading,
|  working folder, and neither has  |      |  This is the primary folder of    |  text.secondary
|  its epic or its project.         |      |  epic Terminal, and it is not on  |
| 16                                |      |  this computer:                   |
|  [v] Save as the working folder   |      |  8                                |
|      of project Acta              |      |  C:\Users\Alexandre\dev\acta      |  type.code,
|  4   If not saved, it is used     |      |                                   |  selectable
|      until Acta quits.            |      | 24                                |
| 24                                |      |  [Locate folder...]  [Try again]  |  primary, secondary
|  [Choose a folder...]             |      |                                   |
+-----------------------------------+      +-----------------------------------+
  terminal surface (bg.sunken dark, bg.raised light), padding 12, text max 480, focus on the primary button
```

Annotations: `[f]` is `folder-open`, `[x]` is `folder-x` in `warning`, `(i)` is `info` 14, `[v]` a checked checkbox. In the first two blocks the rows are alternatives, not a list. The folded row always shows the primary effective folder, so "where will Claude Code start" is answered without unfolding. Inherited rows are never editable from below; their menu leads to the level that owns them. The terminal messages are the empty-state pattern on the terminal's surface, not dialogs: nothing was lost and nothing needs confirming, and the main pane stays usable beside them.

### 8.12 Settings

```
+----------------------+-------------------------------------------------------------------------+
| v Acta               |                               32                                        |
|     [i] Inbox      4 |        Settings                                  <- type.title          |
|     ...              |    32                                                                   |
|                      |        Appearance                                <- type.subtitle 20/28 |
|                      |    8   Theme                              [System|Light|Dark]           |
|                      |        System follows your computer's setting.   <- type.small, tertiary|
|                      |        Language                                     [English v]         |
|                      |        Changes Acta's own text. What you and Claude Code write          |
|                      |        is never translated.                                             |
|                      |        Terminal text size                              [- 13 +]         |
|                      |    32                                                                   |
|                      |        Claude Code                                                      |
|                      |    8   (v) Claude Code found                               Done         |
|                      |        (v) Plugin files written                            Done         |
|                      |        (v) Plugin installed                                Done         |
|                      |        (o) Permission rule added                        Missing         |
|                      |        [Set up Claude Code]  Remove from Claude Code                    |
|                      |    32                                                                   |
|                      |        Data                                                             |
|                      |    8   Export                                       [Export...]         |
|                      |        Last export: 12 days ago                                         |
|                      |        Import                                       [Import...]         |
|----------------------|        Snapshots                    [Restore from snapshot...]          |
|  [t] Trash           |        11 snapshots kept; the newest is from today, 09:12               |
| [[s] Settings]       |        ...                                                              |
+----------------------+-------------------------------------------------------------------------+
```

Annotations: Settings is a page in the active tab; its sidebar row is selected. Sections in `type.subtitle`, setting rows of at least 44 with the description under the label and the control at the right. The fourth check is missing, so "Set up Claude Code" is the primary button. The Working folders, Chat and About sections follow below the fold. Nothing here has a Save button.

### 8.13 Chat panel states

```
+-------------------------------------+   +-------------------------------------+
| Chat  claude.ai      [^] [r] [.] [-] |   | Chat  claude.ai      [^] [r] [.] [-] |  header 28
+-------------------------------------+   +-------------------------------------+
| 16                                  |   | 16                                  |
|  Chat with Claude beside your work  |   |  The chat panel needs WebView2      |  type.heading
|  8                                  |   |  8                                  |
|  This panel shows claude.ai, the    |   |  Microsoft's WebView2 runtime is    |  type.reading,
|  website, in your system's web      |   |  not installed on this computer,    |  text.secondary
|  view. You sign in with your own    |   |  so Acta cannot show claude.ai      |
|  account, and Acta does not read    |   |  here. Everything else in Acta      |
|  or change anything on the page.    |   |  works.                             |
|  8                                  |   | 24                                  |
|  Once started, the web view uses    |   |  [Open claude.ai in your browser]   |  primary
|  about 400 MB of memory, also       |   |  Get WebView2   Try again           |  ghost
|  while the panel is hidden.         |   |                                     |
| 24                                  |   +-------------------------------------+
|  [Open claude.ai here]              |   | The chat is hidden while this is    |  under an overlay:
|  Open in your browser               |   | open.                               |  bg.sunken, one line
+-------------------------------------+   +-------------------------------------+
  bg.sunken, padding 16, text max 480; live state: the web view fills everything under the header
```

Annotations: left, the panel before the web view exists; right, Windows without the WebView2 runtime, and under it the placeholder shown while an overlay is open. `[^]` Open in browser, `[r]` Reload, `[.]` the menu, `[-]` hide; their tooltips open upward. On Linux none of this exists: the command opens the browser.

### 8.14 Epic page with its topic table

```
        Terminal                                                  <- type.title
    12  Progress   3 of 7 topics done  [=====-------]             <- properties block
        Folders    > [f] ~\dev\acta  +1  from project Acta
        Id         E-3
        Updated    2 h ago by Claude Code
    24  Description
        Everything that runs Claude Code inside Acta.
    32  Knowledge  2                                          [+]
        ...
    32  [Open|All|Closed]  4 topics                           [+]   28  toolbar
     8       Title                               Entries   Updated   28  header, type.small
        ------------------------------------------------------------    border.subtle
        (.)  Embed the terminal bound to the pr..     14   2 h ago   32
        (=)  Terminal scrollback                 *     6   Mon       32  * unread dot
        ( )  Links in terminal output                  0   12 Nov    32
        (o)  Session restore                           1   9 Nov     32
        |24 |<- title, fills, truncates ->|          |<- 56 ->|<- 88 ->|
```

Annotations: the epic's progress is computed (topics Done over topics not Dropped). The table shows open topics by default, in the tree's order; a project page shows the same table grouped by epic. Under 520 of column width the Entries column goes.

### 8.15 Tab bar

```
+----------------------+-------------------------------------------------------------------------+
|                      | [(.) Embed the terminal.. x]  [d] Resize protocol     (=) Scrollback  * | 32
|                      +-----------------------------+-------------------------------------------+
|                      |                               32                                        |
|                      |        Embed the terminal bound to the project                          |
+----------------------+-------------------------------------------------------------------------+
  tab bar only with 2+ tabs; bg.sunken; active tab bg.base, joined to the page, close button shown;
  tabs 120 to 220 wide: icon 14, title, then close on hover or the unread dot (*)
```

## 9. Review checklist

Run against every screen, in dark and in light, in English and in French. One failed line blocks the change.

**Tokens and type**
- [ ] Every colour is a token from 2.1; no literal hex, no ad hoc alpha, no opacity on text to fake a text level.
- [ ] `accent` appears only on: focus, selection marker, the single primary action, links, Active, unread, cursor. Count accent elements at rest: three or fewer.
- [ ] Every text style is a role from 3.2; only weights 400 and 600; no uppercase; nothing under 12px.
- [ ] Digits that align or update use `tnum`.
- [ ] Reading text is within the 680 column and left-aligned.
- [ ] Text/surface pairs used are in 2.2 and pass; `text.tertiary` is not on a selected overlay row; no coloured text on `accent.subtle`.

**Layout**
- [ ] Every gap is on the spacing scale; control heights are 24, 28, 32 or 44; radii are 4, 6 or 8.
- [ ] No shadow on anything that does not float; every floating layer has outline plus shadow.
- [ ] No box inside a box; no divider where spacing already separates.
- [ ] Works at 880 x 560 with the terminal open, and at 200% scale; no horizontal scroll outside code blocks.
- [ ] Titlebar follows 4.3 on the platform under review; window controls are where the platform puts them.
- [ ] Workbench: the page names itself only in the titlebar breadcrumb; no tab bar with a single tab; one tool window per dock, each with its 28 header; the status bar holds only project, Claude Code status, update and version.

**States**
- [ ] Each interactive element has default, hover, pressed, keyboard focus and disabled, as specified in 5.
- [ ] Empty, loading (over 400 ms only), error and long-content states exist: a 200-character title, a 500-entry timeline, 0 and 1,000 topics.
- [ ] Both themes: every colour comes from a token with a dark and a light value; the screen was looked at in both; switching theme moves nothing.
- [ ] Status is identifiable with the screen in greyscale.
- [ ] Human and Claude Code entries are distinguishable in greyscale and without reading the name.

**Keyboard and focus**
- [ ] Every action on the screen is reachable without a pointer and is in the palette with its shortcut shown.
- [ ] Focus is visible at every step, moves in reading order, and returns to its origin when an overlay closes.
- [ ] `Esc` goes one level out and never navigates; no shortcut steals a shell key while the terminal is focused.

**Behaviour**
- [ ] Feedback on the next frame; nothing in the "never animates" list animates; budgets in 6.3 hold with 10,000 topics.
- [ ] Writes are optimistic; failure restores state and keeps the user's text.
- [ ] Every destructive action is undoable, or is one of the confirmed cases in 6.4. No path deletes for good, overwrites a document without a revision, or writes plaintext to disk without the warning.
- [ ] Data-safety screens (export, import, trash, snapshots, key loss) state facts and the next step: no red fills, no alarm words, no blame, exact counts and dates.
- [ ] Knowledge: kind icon, summary and author mark visible wherever an entry is listed; inherited and rolled-up entries labelled with their level; every memory written by Claude Code can be read, edited, moved and deleted by the human from the list; document body uses `type.document`, short surfaces use `type.reading`.
- [ ] Working folders: the row shows the primary effective folder and, when it is inherited, where it comes from; paths are `type.code`, middle-truncated, with native separators; a folder that was not found is marked by icon and words (not colour alone), offers Locate, and blocks nothing; removing a folder never touches the disk and is undoable; the terminal never starts anywhere but the primary folder.
- [ ] A write from another process during reading, scrolling and editing moves neither focus nor viewport, and never overwrites a field in edit.

**Copy**
- [ ] Sentence case, the vocabulary of section 7, no exclamation marks, no emoji, "Claude Code" by name.
- [ ] Errors state what failed and the next step; empty states state the purpose and the one action.
- [ ] Sample data uses "Alexandre".
- [ ] Every string comes from the catalogue (10.4) by key, in both languages; no sentence is assembled from fragments; counts use the plural forms.
- [ ] In French nothing is clipped, overlapped or abbreviated by hand: labels size to their text, and what truncates is listed in 10.3. Dates, times and numbers follow 10.2.
- [ ] Nothing the user or Claude Code wrote is translated or reformatted.
- [ ] Every value on screen comes from the model as mapped in 11; nothing is a placeholder.
- [ ] Chat panel: Acta draws nothing over the web view and never reads or changes the page; under an overlay the placeholder shows; every failure ends in a way to open claude.ai in the browser.

**Last look**
- [ ] Remove one element. If the screen still works, it stays removed.
- [ ] Would this screen be mistaken for a template, or for Obsidian, Notion or a JetBrains IDE? If yes, the answer is in type and spacing, not in adding decoration.

## 10. Languages

The interface exists in English (the default on a new install) and in French. The language is chosen in Settings or with the palette ("Language: English", "Language: Français"), applies at once without a restart, and is stored in the preferences file (4.7). Acta does not follow the OS language on its own: the default is English until the user chooses.

**What is translated and what never is.** Translated: every string Acta itself puts on screen, the date, time and number formats, and the key names in shortcut hints. Never translated, reformatted or re-punctuated: anything the user or Claude Code wrote (titles, states, entries, notes, documents, memories, summaries), project, epic and topic names inside Acta's sentences, paths, ids, code, terminal content, the claude.ai page, and the proper names "Acta", "Claude Code", "claude.ai", "WebView2", "Markdown". Agent-facing output (MCP, CLI, hooks; ADR 0005) stays in English whatever the interface language: it is a protocol, not an interface.

### 10.1 French conventions

- Sentence case as in English: a capital on the first word and on proper names only ("Nouveau sujet", never "Nouveau Sujet").
- The user is addressed as "vous". Commands and buttons are infinitives ("Supprimer le sujet", "Choisir un dossier..."); states are past participles ("Sujet supprimé").
- A narrow no-break space (U+202F) before `;` `:` `!` `?` and `»` and after `«`; in the catalogue below an ordinary space is typed there and stands for U+202F. Quotation marks are « ». The apostrophe is the typographic one (’) in both languages; the table types `'` for legibility.
- Numbers: U+202F as thousands separator ("1 000"), a comma for decimals, U+202F before `%` and before units ("68 %", "400 Mo").
- A trailing ellipsis is the single character U+2026 in both languages, with no space before it; the table types three dots.
- Vocabulary, fixed like the English one: projet, epic (masculine, plural epics), sujet, entrée, note, boîte de réception, état actuel, journal (the timeline), connaissances, document, mémoire, dossier de travail, corbeille, instantané (snapshot), onglet, paramètres. Statuses: Brouillon, Prévu, En cours, Bloqué, Terminé, Abandonné.
- Key names on Windows and Linux: `Ctrl`, `Maj`, `Alt`, `Entrée`, `Échap`, `Suppr`, `Tab`. macOS glyphs do not change.

### 10.2 Dates, times and numbers

Both languages use the 24-hour clock and day-month-year order; neither follows the OS region, so a screenshot reads the same on every machine.

| What | English | French |
|---|---|---|
| Under a minute | just now | à l'instant |
| Under an hour | 12 min ago | il y a 12 min |
| Under a day | 2 h ago | il y a 2 h |
| The day before | Yesterday 18:42 | hier 18:42 |
| Under 7 days | Tue 14:05 | mar. 14:05 |
| Older, same or other year | 12 Nov 2026 | 12 nov. 2026 |
| Full (tooltips) | Tuesday 17 November 2026, 14:05 | mardi 17 novembre 2026 à 14:05 |
| Date and time (snapshots) | 18 Nov 2026, 09:12 | 18 nov. 2026 à 09:12 |
| Weekdays | Mon Tue Wed Thu Fri Sat Sun | lun. mar. mer. jeu. ven. sam. dim. |
| Months | Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec | janv. févr. mars avr. mai juin juil. août sept. oct. nov. déc. |
| Thousands, decimals | 1,000 and 0.5 | 1 000 and 0,5 |
| Percent, sizes | 68%, 400 MB | 68 %, 400 Mo |
| Counts over 99 | 99+ | 99+ |

A relative time is lower case inside a sentence and capitalised when it starts one or stands alone in a cell. File names Acta proposes (`acta-2026-11-20.acta.json`) are not localised.

### 10.3 Layout under longer text

French runs 15 to 25% longer than English on sentences and up to twice as long on one-word labels. The layout absorbs that by rule, not by case:

- **Nothing has a width fitted to its English text.** Buttons, segments, menu items, tabs, chips and property labels size to their content. The fixed numeric columns (72, 88, 56) hold numbers and short times only, and the longest French relative time ("il y a 12 min") fits in 88.
- **What may truncate, with an ellipsis and a tooltip giving the whole text**: tree rows, tab titles, table titles, the breadcrumb (4.3), a tool window header's context text, a property label beyond 40% of the column, a setting's label beyond half the row. **What may never truncate**: buttons, menu items, dialog titles and bodies, notices, toasts, empty states, error messages, status words. They wrap.
- **Segmented controls** size each segment to its label. If the whole control would exceed the width it has, it becomes a dropdown button showing the current value; this is the timeline filter's behaviour in a 356 column in French.
- **Dialogs**: buttons stay on one row while they fit the dialog's content width; otherwise they stack, full width, the confirming action on top. Dialog widths (440, 520) do not change with language.
- **Toasts** wrap to at most three lines inside their 360; the action stays on the last line.
- **The status bar and the palette footer** drop items from the least important end when they do not fit: the status bar truncates the project name to 24 characters, then hides the version label's "Acta" prefix; the palette footer drops its hints from the right.
- **Setting rows** let the description wrap under the label; the control never moves to another line.
- **Sentences are whole catalogue entries with named placeholders.** No sentence is built by joining fragments, because word order differs ("{time} par {author}"). Anything with a count has plural forms: English has one and other; French uses one for 0 and 1.
- **Review aid**: a pseudo-language that lengthens every string by 40% and wraps it in brackets must leave every screen usable at 880 x 560. It is a build switch, not a setting.
- Alphabetical order, where Acta sorts by name, follows the interface language's collation; the order the user arranged by hand is never touched.

### 10.4 Copy catalogue

The authoritative text of every string this file defines. Sections 4 to 8 quote the English; code refers to the key. `{name}` is a placeholder. A cell with two forms gives "one · other". Rows marked (W), (M), (L) apply to Windows, macOS, Linux.

| Key | English | French |
|---|---|---|
| `common.cancel` | Cancel | Annuler |
| `common.close` | Close | Fermer |
| `common.undo` | Undo | Annuler |
| `common.retry` | Retry | Réessayer |
| `common.try_again` | Try again | Réessayer |
| `common.edit` | Edit | Modifier |
| `common.delete` | Delete | Supprimer |
| `common.restore` | Restore | Restaurer |
| `common.dismiss` | Dismiss | Ignorer |
| `common.got_it` | Got it | Compris |
| `common.copied` | Copied | Copié |
| `common.save` | Save | Enregistrer |
| `common.open` | Open | Ouvrir |
| `common.move` | Move | Déplacer |
| `common.history` | History | Historique |
| `common.details` | Details | Détails |
| `common.none` | None | Aucun |
| `common.search` | Search | Rechercher |
| `common.clear` | Clear | Effacer |
| `common.opening` | Opening... | Ouverture... |
| `common.checking` | Checking... | Vérification... |
| `common.show_all` | Show all | Tout afficher |
| `common.show_all_n` | Show all {n} | Tout afficher ({n}) |
| `common.by` | {time} by {author} | {time} par {author} |
| `time.now` | just now | à l'instant |
| `time.minutes` | {n} min ago | il y a {n} min |
| `time.hours` | {n} h ago | il y a {n} h |
| `time.yesterday` | Yesterday {time} | hier {time} |
| `tree.inbox` | Inbox | Boîte de réception |
| `tree.knowledge` | Knowledge | Connaissances |
| `tree.trash` | Trash | Corbeille |
| `tree.settings` | Settings | Paramètres |
| `tree.closed` | {n} closed | {n} clos |
| `tree.no_epic` | No epic | Sans epic |
| `status.draft` | Draft | Brouillon |
| `status.planned` | Planned | Prévu |
| `status.active` | Active | En cours |
| `status.blocked` | Blocked | Bloqué |
| `status.done` | Done | Terminé |
| `status.dropped` | Dropped | Abandonné |
| `event.status` | {author} set status to {status} | {author} a passé le statut à {status} |
| `prop.status` | Status | Statut |
| `prop.epic` | Epic | Epic |
| `prop.folders` | Folders | Dossiers |
| `prop.id` | Id | Identifiant |
| `prop.updated` | Updated | Modifié |
| `prop.progress` | Progress | Avancement |
| `prop.kind` | Kind | Type |
| `prop.level` | Level | Niveau |
| `prop.progress.value` | {done} of {total} topics done | {done} sujet terminé sur {total} · {done} sujets terminés sur {total} |
| `kind.document` | Document | Document |
| `kind.memory` | Memory | Mémoire |
| `level.project` | Project {name} | Projet {name} |
| `level.epic` | Epic {name} | Epic {name} |
| `level.topic` | Topic {name} | Sujet {name} |
| `section.state` | Current state | État actuel |
| `section.description` | Description | Description |
| `section.knowledge` | Knowledge | Connaissances |
| `section.timeline` | Timeline | Journal |
| `state.updated` | Updated {time} by {author} | Modifié {time} par {author} |
| `state.placeholder` | Where does this stand? One or two sentences. | Où en est-on ? Une ou deux phrases. |
| `description.placeholder` | What is this topic about, and what does done look like? | De quoi s'agit-il, et à quoi verra-t-on que c'est terminé ? |
| `timeline.all` | All | Tout |
| `entry.progress` | Progress | Avancement |
| `entry.decision` | Decision | Décision |
| `entry.decisions` | Decisions | Décisions |
| `entry.discussion` | Discussion | Discussion |
| `composer.placeholder` | Add an entry... | Ajouter une entrée... |
| `composer.hint` | {keys} to save | {keys} pour enregistrer |
| `composer.add` | Add entry | Ajouter l'entrée |
| `timeline.new` | {n} new entry · {n} new entries | {n} nouvelle entrée · {n} nouvelles entrées |
| `timeline.empty` | No entries yet. Add one, or let Claude Code log progress as it works. | Aucune entrée pour l'instant. Ajoutez-en une, ou laissez Claude Code consigner l'avancement au fil du travail. |
| `entry.copy_markdown` | Copy as Markdown | Copier en Markdown |
| `entry.change_type` | Change type | Changer le type |
| `conflict.notice` | Claude Code changed this while you were editing. | Claude Code a modifié ceci pendant que vous écriviez. |
| `conflict.theirs` | Show theirs | Voir sa version |
| `conflict.mine` | Keep mine | Garder la mienne |
| `deleted.notice` | This topic was deleted by Claude Code. | Ce sujet a été supprimé par Claude Code. |
| `topic.new` | New topic | Nouveau sujet |
| `topic.needs_title` | A topic needs a title. | Un sujet doit avoir un titre. |
| `topic.deleted` | Topic deleted. | Sujet supprimé. |
| `topic.restored` | Topic restored. | Sujet restauré. |
| `topic.empty_project` | No topics yet. Capture notes and let Claude Code file them, or start one yourself. | Aucun sujet pour l'instant. Saisissez des notes et laissez Claude Code les classer, ou créez-en un vous-même. |
| `error.save_entry` | Could not save the entry. The database is locked by another process. | Impossible d'enregistrer l'entrée. La base de données est verrouillée par un autre processus. |
| `table.count` | {n} topic · {n} topics | {n} sujet · {n} sujets |
| `table.filter.open` | Open | Ouverts |
| `table.filter.all` | All | Tous |
| `table.filter.closed` | Closed | Clos |
| `table.title` | Title | Titre |
| `table.entries` | Entries | Entrées |
| `table.updated` | Updated | Modifié |
| `table.reset` | Reset order | Rétablir l'ordre |
| `table.group_progress` | {done} of {total} done | {done} sur {total} terminés |
| `inbox.count` | {n} note · {n} notes | {n} note · {n} notes |
| `inbox.placeholder` | Note for {project} | Note pour {project} |
| `inbox.add` | Add | Ajouter |
| `inbox.filter.open` | Open | À traiter |
| `inbox.filter.filed` | Filed | Classées |
| `inbox.file` | File with Claude Code | Classer avec Claude Code |
| `inbox.filed_as` | Filed as | Classée dans |
| `inbox.make_topic` | Make topic | Créer un sujet |
| `inbox.empty` | Inbox is empty. Notes you capture land here until they become topics. | La boîte de réception est vide. Les notes que vous saisissez arrivent ici jusqu'à devenir des sujets. |
| `kb.new_document` | New document | Nouveau document |
| `kb.new_memory` | New memory | Nouvelle mémoire |
| `kb.written` | Written by {author} {time} | Écrit par {author} {time} |
| `kb.edited` | edited by {author} {time} | modifié par {author} {time} |
| `kb.from_epic` | From epic {name} | De l'epic {name} |
| `kb.from_project` | From project {name} | Du projet {name} |
| `kb.more_inherited` | {n} more from epic and project | {n} autre de l'epic et du projet · {n} autres de l'epic et du projet |
| `kb.more_epic` | {n} more in the topics of this epic | {n} de plus dans les sujets de cet epic |
| `kb.more_project` | {n} more in this project's epics and topics | {n} de plus dans les epics et sujets de ce projet |
| `kb.empty` | Nothing here yet. Documents and memories for this topic appear here. | Rien ici pour l'instant. Les documents et mémoires de ce sujet apparaîtront ici. |
| `kb.summary.memory` | One line: the fact, in short | Une ligne : le fait, en bref |
| `kb.summary.document` | One line: what this is and when to read it | Une ligne : ce que c'est et quand le lire |
| `kb.show_history` | Show history | Afficher l'historique |
| `kb.move_epic` | Move to epic "{name}" | Déplacer vers l'epic « {name} » |
| `kb.move_project` | Move to project "{name}" | Déplacer vers le projet « {name} » |
| `kb.move_to` | Move to... | Déplacer vers... |
| `kb.change_kind` | Change kind | Changer de type |
| `kb.save_file` | Save as file | Enregistrer comme fichier |
| `kb.moved` | Moved to project {name}. | Déplacé vers le projet {name}. |
| `kb.scope` | {project} · {d} documents, {m} memories | {project} · {d} documents, {m} mémoires |
| `kb.search` | Search knowledge | Rechercher dans les connaissances |
| `kb.filter.level` | Level: {value} | Niveau : {value} |
| `kb.filter.kind` | Kind: {value} | Type : {value} |
| `kb.filter.author` | Author: {value} | Auteur : {value} |
| `kb.value.all` | All | Tous |
| `kb.value.project` | Project | Projet |
| `kb.value.epics` | Epics | Epics |
| `kb.value.topics` | Topics | Sujets |
| `kb.value.documents` | Documents | Documents |
| `kb.value.memories` | Memories | Mémoires |
| `kb.results` | {n} result for "{query}" · {n} results for "{query}" | {n} résultat pour « {query} » · {n} résultats pour « {query} » |
| `kb.no_results` | Nothing in this project mentions "{query}". | Rien dans ce projet ne mentionne « {query} ». |
| `doc.editing` | Editing | Modification en cours |
| `doc.needs` | A document needs a title and a summary. | Un document doit avoir un titre et un résumé. |
| `doc.go_heading` | Go to heading | Aller au titre |
| `history.version` | Version from {time} by {author} | Version du {time} par {author} |
| `history.text` | Text | Texte |
| `history.changes` | Changes | Modifications |
| `history.restore` | Restore this version | Restaurer cette version |
| `history.restored` | Restored the version from {time}. | Version du {time} restaurée. |
| `history.replaced` | Replaced section {name} | Section {name} remplacée |
| `history.edited` | Edited | Modifié |
| `history.from_file` | Created from file | Créé depuis un fichier |
| `folders.title` | Working folders | Dossiers de travail |
| `folders.from_epic` | from epic {name} | de l'epic {name} |
| `folders.from_project` | from project {name} | du projet {name} |
| `folders.primary` | Primary | Principal |
| `folders.include` | Also include inherited folders | Inclure aussi les dossiers hérités |
| `folders.not_used` | Not used here: {n} folder from epic {name}. · Not used here: {n} folders from epic {name}. | Non utilisé ici : {n} dossier de l'epic {name}. · Non utilisés ici : {n} dossiers de l'epic {name}. |
| `folders.add` | Add folder... | Ajouter un dossier... |
| `folders.hint` | Claude Code starts in the primary folder and may also read and edit the others. | Claude Code démarre dans le dossier principal et peut aussi lire et modifier les autres. |
| `folders.set_here` | Set folder for this topic... | Définir un dossier pour ce sujet... |
| `folders.uses` | This topic uses the folders of epic {name}. A folder set here replaces them for this topic only. | Ce sujet utilise les dossiers de l'epic {name}. Un dossier défini ici les remplace pour ce sujet seulement. |
| `folders.none` | No working folder. The terminal will ask where to start. | Aucun dossier de travail. Le terminal demandera où démarrer. |
| `folders.set` | Set working folder... | Définir le dossier de travail... |
| `folders.not_found` | Not found on this computer | Introuvable sur cet ordinateur |
| `folders.locate` | Locate... | Localiser... |
| `folders.n_not_found` | {n} not found | {n} introuvable · {n} introuvables |
| `folders.also_topic` | Also attached to topic {name}. A session started in this folder outside Acta opens on epic {epic} and is told about both topics. | Également rattaché au sujet {name}. Une session lancée dans ce dossier hors d'Acta s'ouvre sur l'epic {epic} et est informée des deux sujets. |
| `folders.also_epic` | Also attached to epic {name}. A session started in this folder outside Acta opens on project {project} and is told about both epics. | Également rattaché à l'epic {name}. Une session lancée dans ce dossier hors d'Acta s'ouvre sur le projet {project} et est informée des deux epics. |
| `folders.show.w` | Show in Explorer (W) | Afficher dans l'Explorateur |
| `folders.show.m` | Reveal in Finder (M) | Afficher dans le Finder |
| `folders.show.l` | Show in file manager (L) | Afficher dans le gestionnaire de fichiers |
| `folders.copy_path` | Copy path | Copier le chemin |
| `folders.make_primary` | Make primary | Définir comme principal |
| `folders.move_up` | Move up | Monter |
| `folders.move_down` | Move down | Descendre |
| `folders.choose_other` | Choose another folder... | Choisir un autre dossier... |
| `folders.remove` | Remove from this topic | Retirer de ce sujet |
| `folders.edit_in` | Edit in epic {name} | Modifier dans l'epic {name} |
| `folders.removed` | {folder} removed from this topic. | {folder} retiré de ce sujet. |
| `folders.located_for` | {folder} located for epic {name} and everything that inherits from it. | {folder} localisé pour l'epic {name} et tout ce qui en hérite. |
| `folders.bulk` | {n} other folders under {root} were not found either. Apply the same change to them? | {n} autres dossiers sous {root} sont également introuvables. Leur appliquer le même changement ? |
| `folders.bulk_apply` | Apply to {n} folders | Appliquer à {n} dossiers |
| `folders.bulk_done` | {n} folders located. | {n} dossiers localisés. |
| `folders.picker` | Choose a working folder | Choisir un dossier de travail |
| `terminal.title` | Terminal | Terminal |
| `terminal.on_topic` | on topic {name} | sur le sujet {name} |
| `terminal.on_epic` | on epic {name} | sur l'epic {name} |
| `terminal.on_project` | on project {name} | sur le projet {name} |
| `terminal.restart_here` | Restart here | Redémarrer ici |
| `terminal.ended` | Session ended. Enter to restart. | Session terminée. Entrée pour relancer. |
| `terminal.ask.title` | Where should Claude Code start? | Où Claude Code doit-il démarrer ? |
| `terminal.ask.topic` | {name} has no working folder, and neither has its epic or its project. | {name} n'a pas de dossier de travail, son epic et son projet non plus. |
| `terminal.ask.project` | {name} has no working folder yet. | {name} n'a pas encore de dossier de travail. |
| `terminal.ask.save` | Save as the working folder of project {name} | Enregistrer comme dossier de travail du projet {name} |
| `terminal.ask.unsaved` | If not saved, it is used until Acta quits. | Sinon, il sert jusqu'à la fermeture d'Acta. |
| `terminal.ask.choose` | Choose a folder... | Choisir un dossier... |
| `terminal.missing.title` | The working folder was not found | Le dossier de travail est introuvable |
| `terminal.missing.body` | Claude Code was not started. This is the primary folder of epic {name}, and it is not on this computer: | Claude Code n'a pas été lancé. C'est le dossier principal de l'epic {name}, et il n'est pas sur cet ordinateur : |
| `terminal.missing.locate` | Locate folder... | Localiser le dossier... |
| `terminal.left_out` | {folder} was not found and was left out of this session. | {folder} est introuvable et n'a pas été inclus dans cette session. |
| `terminal.located` | {folder} located. Restart the terminal to include it. | {folder} localisé. Redémarrez le terminal pour l'inclure. |
| `terminal.restart.title` | Restart the terminal? | Redémarrer le terminal ? |
| `terminal.restart.body` | Claude Code is still running in it. | Claude Code y est encore en cours d'exécution. |
| `terminal.restart.confirm` | Restart | Redémarrer |
| `chat.title` | Chat | Discussion |
| `chat.open_browser` | Open in browser | Ouvrir dans le navigateur |
| `chat.reload` | Reload | Recharger |
| `chat.help` | Sign-in help | Aide à la connexion |
| `chat.start.title` | Chat with Claude beside your work | Discutez avec Claude à côté de votre travail |
| `chat.start.body` | This panel shows claude.ai, the website, in your system's web view. You sign in with your own account, and Acta does not read or change anything on the page. | Ce panneau affiche claude.ai, le site web, dans la vue web de votre système. Vous vous connectez avec votre propre compte, et Acta ne lit ni ne modifie rien sur la page. |
| `chat.start.memory` | Once started, the web view uses about 400 MB of memory, also while the panel is hidden. | Une fois lancée, la vue web utilise environ 400 Mo de mémoire, même quand le panneau est masqué. |
| `chat.start.here` | Open claude.ai here | Ouvrir claude.ai ici |
| `chat.start.browser` | Open in your browser | Ouvrir dans votre navigateur |
| `chat.starting` | Opening claude.ai... | Ouverture de claude.ai... |
| `chat.signin` | Sign in with email: the link in the message opens in your browser and shows a code; type that code here. Signing in with Google or Apple may be refused in this panel. | Connectez-vous par e-mail : le lien du message s'ouvre dans votre navigateur et affiche un code ; saisissez ce code ici. La connexion avec Google ou Apple peut être refusée dans ce panneau. |
| `chat.hidden` | The chat is hidden while this is open. | La discussion est masquée tant que ceci est ouvert. |
| `chat.runtime.title` | The chat panel needs WebView2 | Le panneau de discussion a besoin de WebView2 |
| `chat.runtime.body` | Microsoft's WebView2 runtime is not installed on this computer, so Acta cannot show claude.ai here. Everything else in Acta works. | Le composant WebView2 de Microsoft n'est pas installé sur cet ordinateur ; Acta ne peut donc pas afficher claude.ai ici. Tout le reste d'Acta fonctionne. |
| `chat.runtime.get` | Get WebView2 | Obtenir WebView2 |
| `chat.browser` | Open claude.ai in your browser | Ouvrir claude.ai dans votre navigateur |
| `chat.failed.title` | claude.ai could not be opened here | claude.ai n'a pas pu être ouvert ici |
| `chat.failed.offline` | There is no connection. | Il n'y a pas de connexion. |
| `chat.signout` | Sign out and clear chat data... | Se déconnecter et effacer les données de discussion... |
| `chat.signout.title` | Sign out and clear chat data? | Se déconnecter et effacer les données de discussion ? |
| `chat.signout.body` | This deletes the panel's cookies and site data from this computer. Your conversations stay in your Claude account. | Cela supprime de cet ordinateur les cookies et les données de site du panneau. Vos conversations restent dans votre compte Claude. |
| `chat.signout.confirm` | Sign out and clear | Se déconnecter et effacer |
| `tab.open_new` | Open in new tab | Ouvrir dans un nouvel onglet |
| `tab.close` | Close tab | Fermer l'onglet |
| `tab.close_others` | Close other tabs | Fermer les autres onglets |
| `tab.reopen` | Reopen closed tab | Rouvrir l'onglet fermé |
| `palette.recent` | Recent | Récents |
| `palette.topics` | Topics | Sujets |
| `palette.commands` | Commands | Commandes |
| `palette.projects` | Projects | Projets |
| `palette.hint.open` | Enter open | Entrée ouvrir |
| `palette.hint.tab` | {keys} open in new tab | {keys} ouvrir dans un nouvel onglet |
| `palette.hint.close` | Esc close | Échap fermer |
| `palette.none` | Nothing matches "{query}". Enter creates a topic with this title. | Aucun résultat pour « {query} ». Entrée crée un sujet avec ce titre. |
| `cmd.set_status` | Set status: {status} | Définir le statut : {status} |
| `cmd.settings` | Open settings | Ouvrir les paramètres |
| `cmd.theme` | Theme: {value} | Thème : {value} |
| `cmd.language` | Language: {value} | Langue : {value} |
| `cmd.new_project` | New project... | Nouveau projet... |
| `cmd.switch_project` | Switch project | Changer de projet |
| `cmd.toggle_sidebar` | Toggle sidebar | Afficher ou masquer la barre latérale |
| `cmd.toggle_terminal` | Toggle terminal | Afficher ou masquer le terminal |
| `cmd.toggle_chat` | Toggle chat | Afficher ou masquer la discussion |
| `cmd.terminal_bottom` | Move terminal to bottom | Placer le terminal en bas |
| `cmd.terminal_right` | Move terminal to right | Placer le terminal à droite |
| `cmd.chat_bottom` | Move chat to bottom | Placer la discussion en bas |
| `cmd.chat_right` | Move chat to right | Placer la discussion à droite |
| `cmd.restart_terminal` | Restart terminal here | Redémarrer le terminal ici |
| `cmd.widen_sidebar` | Widen sidebar | Élargir la barre latérale |
| `cmd.narrow_sidebar` | Narrow sidebar | Rétrécir la barre latérale |
| `cmd.widen_terminal` | Widen terminal | Élargir le terminal |
| `cmd.narrow_terminal` | Narrow terminal | Rétrécir le terminal |
| `cmd.setup` | Set up Claude Code | Configurer Claude Code |
| `cmd.shortcuts` | Keyboard shortcuts | Raccourcis clavier |
| `cmd.kb_open` | Open document or memory... | Ouvrir un document ou une mémoire... |
| `cmd.kb_go` | Go to knowledge base | Aller aux connaissances |
| `cmd.kb_search` | Search knowledge... | Rechercher dans les connaissances... |
| `cmd.kb_review` | Review memories written by Claude Code | Revoir les mémoires écrites par Claude Code |
| `cmd.kb_from_file` | New document from file... | Nouveau document depuis un fichier... |
| `cmd.kb_to_memory` | Change kind to memory | Changer le type en mémoire |
| `cmd.kb_to_document` | Change kind to document | Changer le type en document |
| `cmd.go_heading` | Go to heading... | Aller au titre... |
| `cmd.save_file` | Save as file... | Enregistrer comme fichier... |
| `cmd.export` | Export... | Exporter... |
| `cmd.import` | Import... | Importer... |
| `cmd.trash` | Open trash | Ouvrir la corbeille |
| `cmd.snapshot` | Restore from snapshot... | Restaurer depuis un instantané... |
| `cmd.folders_add` | Add working folder... | Ajouter un dossier de travail... |
| `cmd.folders_show` | Show working folders | Afficher les dossiers de travail |
| `cmd.folder_primary` | Make primary folder | Définir comme dossier principal |
| `cmd.folder_up` | Move folder up | Monter le dossier |
| `cmd.folder_down` | Move folder down | Descendre le dossier |
| `cmd.folder_locate` | Locate folder... | Localiser le dossier... |
| `cmd.folder_copy` | Copy folder path | Copier le chemin du dossier |
| `cmd.folder_show.w` | Show folder in Explorer (W) | Afficher le dossier dans l'Explorateur |
| `cmd.folder_show.m` | Reveal folder in Finder (M) | Afficher le dossier dans le Finder |
| `cmd.folder_show.l` | Show folder in file manager (L) | Afficher le dossier dans le gestionnaire de fichiers |
| `cmd.folder_remove` | Remove folder from this topic | Retirer le dossier de ce sujet |
| `cmd.folders_include` | Include inherited folders | Inclure les dossiers hérités |
| `cmd.folders_exclude` | Stop including inherited folders | Ne plus inclure les dossiers hérités |
| `cmd.folders_bulk` | Apply the same change to {n} folders | Appliquer le même changement à {n} dossiers |
| `slash.h1` | Heading 1 | Titre 1 |
| `slash.h2` | Heading 2 | Titre 2 |
| `slash.h3` | Heading 3 | Titre 3 |
| `slash.bullets` | Bulleted list | Liste à puces |
| `slash.numbers` | Numbered list | Liste numérotée |
| `slash.tasks` | Task list | Liste de tâches |
| `slash.quote` | Quote | Citation |
| `slash.code` | Code block | Bloc de code |
| `slash.table` | Table | Tableau |
| `slash.divider` | Divider | Séparateur |
| `settings.appearance` | Appearance | Apparence |
| `settings.theme` | Theme | Thème |
| `settings.theme.system` | System | Système |
| `settings.theme.light` | Light | Clair |
| `settings.theme.dark` | Dark | Sombre |
| `settings.theme.hint` | System follows your computer's setting. | Système suit le réglage de votre ordinateur. |
| `settings.language` | Language | Langue |
| `settings.language.hint` | Changes Acta's own text. What you and Claude Code write is never translated. | Change les textes d'Acta. Ce que vous et Claude Code écrivez n'est jamais traduit. |
| `settings.terminal_size` | Terminal text size | Taille du texte du terminal |
| `settings.folders` | Working folders | Dossiers de travail |
| `settings.auto_memory` | Turn off Claude Code's own memory in attached folders | Désactiver la mémoire propre à Claude Code dans les dossiers rattachés |
| `settings.auto_memory.hint` | When a folder is attached, Acta writes autoMemoryEnabled: false to its .claude/settings.local.json, so that what Claude Code learns is kept in Acta, where you can read it. | Quand un dossier est rattaché, Acta écrit autoMemoryEnabled: false dans son .claude/settings.local.json, afin que ce que Claude Code apprend soit conservé dans Acta, où vous pouvez le lire. |
| `settings.not_found` | Folders not found | Dossiers introuvables |
| `settings.claude` | Claude Code | Claude Code |
| `settings.check.cli` | Claude Code found | Claude Code trouvé |
| `settings.check.files` | Plugin files written | Fichiers du plugin écrits |
| `settings.check.plugin` | Plugin installed | Plugin installé |
| `settings.check.rule` | Permission rule added | Règle d'autorisation ajoutée |
| `settings.check.done` | Done | Fait |
| `settings.check.missing` | Missing | Manquant |
| `settings.setup` | Set up Claude Code | Configurer Claude Code |
| `settings.setup_again` | Run setup again | Relancer la configuration |
| `settings.setup_remove` | Remove from Claude Code | Retirer de Claude Code |
| `settings.setup.hint` | Setup installs Acta's plugin in Claude Code and adds one permission line to ~/.claude/settings.json. Remove undoes both. | La configuration installe le plugin d'Acta dans Claude Code et ajoute une ligne d'autorisation à ~/.claude/settings.json. Retirer annule les deux. |
| `settings.data` | Data | Données |
| `settings.export.last` | Last export: {time} | Dernier export : {time} |
| `settings.export.never` | Never exported | Jamais exporté |
| `settings.snapshots` | {n} snapshots kept; the newest is from {time} | {n} instantanés conservés ; le plus récent date de {time} |
| `settings.trash_count` | {n} item · {n} items | {n} élément · {n} éléments |
| `settings.database` | Database | Base de données |
| `settings.chat` | Chat | Discussion |
| `settings.about` | About | À propos |
| `settings.release_notes` | Release notes | Notes de version |
| `settings.licences` | Licences | Licences |
| `statusbar.not_set_up` | Claude Code: not set up | Claude Code : non configuré |
| `statusbar.not_found` | Claude Code: not found | Claude Code : introuvable |
| `statusbar.no_folder` | Claude Code: no folder for this project | Claude Code : aucun dossier pour ce projet |
| `statusbar.ready` | Claude Code: ready | Claude Code : prêt |
| `statusbar.last_wrote` | Last wrote {time} in {topic} | Dernière écriture {time} dans {topic} |
| `update.ready` | Restart to update | Redémarrer pour mettre à jour |
| `update.done` | Updated to {version} | Mis à jour vers {version} |
| `update.failed` | Update failed | Échec de la mise à jour |
| `update.manual` | Download manually | Télécharger manuellement |
| `export.title` | Export | Exporter |
| `export.scope.all` | Everything | Tout |
| `export.scope.project` | This project | Ce projet |
| `export.warning` | The file is not encrypted. Anyone who can open it can read these projects. That is what lets you read it without Acta or its key. | Le fichier n'est pas chiffré. Toute personne pouvant l'ouvrir peut lire ces projets. C'est ce qui vous permet de le lire sans Acta ni sa clé. |
| `export.passphrase` | Protect with a passphrase | Protéger par une phrase secrète |
| `export.passphrase.lost` | Acta cannot recover a lost passphrase. | Acta ne peut pas retrouver une phrase secrète perdue. |
| `export.choose` | Choose location... | Choisir l'emplacement... |
| `export.done` | Exported {n} projects to {file}. | {n} projets exportés vers {file}. |
| `export.show` | Show in folder | Afficher dans le dossier |
| `export.suggest` | Everything is stored on this computer only. An export is your copy if the system keychain is ever lost. | Tout est stocké sur cet ordinateur uniquement. Un export est votre copie si le trousseau du système venait à être perdu. |
| `import.title` | Import "{file}" | Importer « {file} » |
| `import.meta` | Exported {date} · format {n} · {p} projects | Exporté le {date} · format {n} · {p} projets |
| `import.created` | Created | Créés |
| `import.updated` | Updated | Mis à jour |
| `import.skipped` | Skipped | Ignorés |
| `import.kind.projects` | Projects | Projets |
| `import.kind.epics` | Epics | Epics |
| `import.kind.topics` | Topics | Sujets |
| `import.kind.entries` | Entries | Entrées |
| `import.kind.notes` | Notes | Notes |
| `import.kind.documents` | Documents | Documents |
| `import.kind.memories` | Memories | Mémoires |
| `import.safe` | Nothing here will be deleted. Skipped records are the same as, or older than, what Acta already has. | Rien ne sera supprimé. Les éléments ignorés sont identiques à ceux qu'Acta possède déjà, ou plus anciens. |
| `import.show_changes` | Show what changes | Afficher ce qui change |
| `import.confirm` | Import | Importer |
| `import.done` | Imported: {c} created, {u} updated. A snapshot from before the import is kept. | Import terminé : {c} créés, {u} mis à jour. Un instantané d'avant l'import est conservé. |
| `import.folders` | {n} working folders were not found on this computer. Locate each one from its project, epic or topic. | {n} dossiers de travail sont introuvables sur cet ordinateur. Localisez chacun depuis son projet, son epic ou son sujet. |
| `import.newer` | This file was made by a newer Acta (format {n}). Update Acta, then import it. | Ce fichier a été créé par une version plus récente d'Acta (format {n}). Mettez Acta à jour, puis importez-le. |
| `import.not_acta` | This is not an Acta export. | Ce n'est pas un export Acta. |
| `import.wrong_passphrase` | Wrong passphrase. | Phrase secrète incorrecte. |
| `trash.hint` | Deleted things stay here for 30 days. | Les éléments supprimés restent ici 30 jours. |
| `trash.days` | {n} day left · {n} days left | {n} jour restant · {n} jours restants |
| `trash.empty` | Trash is empty. | La corbeille est vide. |
| `snapshot.daily` | Daily | Quotidien |
| `snapshot.weekly` | Weekly | Hebdomadaire |
| `snapshot.before_update` | Before update to {version} | Avant la mise à jour vers {version} |
| `snapshot.before_import` | Before import | Avant import |
| `snapshot.before_restore` | Before restore | Avant restauration |
| `snapshot.confirm` | Restore and restart | Restaurer et redémarrer |
| `snapshot.title` | Restore the snapshot from {date}? | Restaurer l'instantané du {date} ? |
| `snapshot.body` | Acta will restart with its data as of {datetime}. Everything as it is now is saved as a snapshot first, so you can come back. | Acta redémarrera avec ses données du {datetime}. Tout l'état actuel est d'abord enregistré dans un instantané, vous pourrez donc y revenir. |
| `snapshot.restored` | Restored the snapshot from {datetime}. | Instantané du {datetime} restauré. |
| `snapshot.migration` | The update to {version} could not convert the database. Acta put back the snapshot from just before; nothing was lost. | La mise à jour vers {version} n'a pas pu convertir la base de données. Acta a remis l'instantané pris juste avant ; rien n'a été perdu. |
| `keyloss.title` | Acta cannot open its database | Acta ne peut pas ouvrir sa base de données |
| `keyloss.body` | The database is on this computer and intact, but its key is missing from {keychain}. This usually follows a system reinstall, a new user profile or a move to another machine. | La base de données est sur cet ordinateur et intacte, mais sa clé est absente de {keychain}. Cela suit généralement une réinstallation du système, un nouveau profil utilisateur ou un changement de machine. |
| `keyloss.untouched` | Acta has not changed the file: | Acta n'a pas modifié le fichier : |
| `keyloss.import` | Import an export... | Importer un export... |
| `keyloss.limit` | Without the key or an export, the content cannot be read. You can start empty; the file is kept as {file}. | Sans la clé ni un export, le contenu ne peut pas être lu. Vous pouvez repartir de zéro ; le fichier est conservé sous le nom {file}. |
| `keyloss.start_empty` | Start empty | Repartir de zéro |
| `firstrun.title` | Start with a project | Commencez par un projet |
| `firstrun.body` | A project is a folder you work on with Claude Code. Acta keeps its topics, decisions and notes next to the work. | Un projet est un dossier sur lequel vous travaillez avec Claude Code. Acta garde ses sujets, ses décisions et ses notes à côté du travail. |
| `firstrun.choose` | Choose a folder | Choisir un dossier |
| `firstrun.then` | Then | Ensuite |
| `firstrun.step1` | Connect Claude Code | Connecter Claude Code |
| `firstrun.step2` | Capture a first note | Saisir une première note |
| `firstrun.step3` | Open the terminal | Ouvrir le terminal |

Strings that exist in the behaviour sections but are not in this table are a defect of the table: add the row in the same change that adds the string.

## 11. Data: what every element shows

Nothing on screen is illustrative. Every value comes from the model of ADRs 0005 to 0008 (Project > Epic > Topic, statuses, current state, timeline entries, notes, knowledge entries and revisions, working folders) or is computed from it by a rule stated here.

| Element | Source |
|---|---|
| Tree: projects, epics, topics, order | The records and their stored position. Collapsed state is a preference (4.7). |
| Inbox count | Notes of the project not yet filed. |
| Knowledge count | Knowledge entries of the project at every level. |
| Status glyph and word | The topic's status (2.3). Epics and projects have no status of their own. |
| Progress (epic, project, group row) | Topics Done divided by topics not Dropped, in that epic or project. Computed, never stored. |
| Id (T-42, E-7, K-7) | The per-installation short ids of ADR 0005 and 0006. |
| Updated, "by Claude Code" | `updated_at` and the author of the last write (human or Claude Code, ADR 0005). |
| Current state, description, title | The topic's fields. |
| Timeline entries and system events | Timeline entries (type, author, time, text) and the recorded status changes, creations and moves. |
| Entries column of the topic table | Count of timeline entries of the topic. |
| Knowledge rows, summaries, kinds, levels, history | `kb_entries` and `kb_revisions` (ADR 0007). |
| Folders, origin label, not-found state | The `folders` table and inheritance rule of ADR 0008; existence is checked on this machine. |
| Breadcrumb | The page's project and epic. |
| Tabs | A preference: the list of open record ids per window. |
| Terminal header context | The page the session was started from, held by the app while the session lives. |
| Status bar: Claude Code status | `acta setup --check` (ADR 0005) and whether the active project has a working folder. Tooltip: the latest write authored by Claude Code in the project. |
| Status bar: version, update | The running build and the updater's state (ADR 0004). |
| Settings: last export, snapshots, trash count, database path | Export history, the snapshot list and the trash of ADR 0006; the storage location of ADR 0003. |
| Settings: folders not found | Every folder record, across projects, whose path is not a folder on this machine. |
| Chat panel | Nothing from the model: it is the website. Acta holds only whether the sign-in help was dismissed (a preference). |

### Data model requests

For the lead. Each is something the design wants and the ADRs do not yet give; the design works without them today, in the way stated.

1. **Read markers.** The unread dot (tree, tabs, topic table, knowledge rows) needs, per record, when this installation last showed it: a `last_seen_at` per topic and per knowledge entry, local and not exported. Without it the dot can only cover changes that arrive while the app is running.
2. **Session presence.** To let the status bar say that a Claude Code session is live on a topic right now, `acta mcp` would have to keep a presence row (session, project, focus, heartbeat) while it runs. Until then the status bar reports setup and the last write, and makes no claim about live sessions.
3. **Links between records.** Backlinks and a "mentioned in" list, taken from Obsidian, need stable references in text (for example a link form that carries the UUID) or a relation table; the short ids are per-installation aliases and cannot serve after an import. Left out of v1.
4. **Setup check as data.** The four checks of `acta setup --check` and the path of the `acta` command must be callable from the app as structured results, not parsed from CLI text.
5. **Export history.** "Last export: 12 days ago" needs the time of the last export recorded (in the database or the preferences file; it must survive an update).
6. **Folders not found, across projects.** A query over all folder records for the Settings list and for bulk repair; no schema change, but the existence check must run off the UI thread for the whole set.
7. **Preferences outside the database.** `preferences.json` (4.7) is new: plaintext, no user content, holding record ids for tabs and tree expansion. The lead should confirm that ids in a plaintext file are acceptable under the rule on plaintext data in CLAUDE.md; if not, tabs and expansion move into the database and only theme and language stay in the file.
8. **Entry count per topic** for the topic table should come from one grouped query, not one per row, to hold the 50 ms page budget with 1,000 topics.

Not requested, by decision: due dates, priorities, assignees, custom properties, tags or categories on knowledge entries, file-change tracking. None is in the user's direction, and each would make the agent's writes less predictable.
