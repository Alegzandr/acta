# Acta design system

Single source of truth for every pixel and every key press. UI code conforms to this file; if a screen needs something that is not here, change this file first, in the same change, and say why.

Identity: **a lamp-lit desk, paper and ink.** Warm near-black surfaces, warm off-white text, one verdigris accent (oxidised copper), a humanist sans for reading and a plain mono for the machine. Acta is a record: it should feel like a well-kept ledger, not a dashboard.

Units are logical pixels (GPUI `px`). `Mod` means `Cmd` on macOS and `Ctrl` on Windows and Linux.

## 1. Principles

1. **The record is the interface.** Topic title, current state and timeline are the product; chrome exists to reach them. If a pixel is neither content nor a control the user will use this week, remove it. A surface with nothing on it is correct, not unfinished.
2. **One accent, spent on "here" and "go".** Verdigris marks only: keyboard focus, the current selection marker, the primary action, links, the Active status and unread changes. Never decoration, never a second accent. If two accent things compete on one screen, one of them is wrong.
3. **Hierarchy comes from type and space, then tone, then lines.** Reach for size/weight/spacing first, a surface step second, a 1px border third. Shadows only on things that float above the window. Boxes inside boxes are a defect.
4. **Keyboard first, pointer complete.** Every action has a key path through the command palette at minimum; every key path has a visible pointer equivalent. A feature without a palette command is not shipped.
5. **Instant means the next frame.** Selection, hover, focus, navigation and typing never animate and never wait on disk. Motion is reserved for things that appear from nowhere (overlays, toasts, remote changes) and exists to explain, not to please.
6. **The agent writes; the human is never moved.** Changes made by Claude Code appear live but never take focus, never scroll the viewport, never overwrite a field being edited. The reader's place is sacred.
7. **Undo over asking.** Anything reversible happens immediately with an undo. A dialog is an admission that we could not make the action reversible.

**Never:** gradients; glow or coloured shadows; blur; emoji anywhere (UI, sample data, empty states); illustrations or mascots; all-caps labels; coloured left-stripe borders on cards; pill-shaped buttons; more than two font weights; a spinner before 400 ms; skeleton shimmer; badges with counts above 99; toasts for things the user can already see changed; modal dialogs for reversible actions; a robot or sparkle icon for Claude Code; pure black `#000` or pure white `#FFF` as a surface or text colour; centred body text; tooltips as the only way to learn what a control does when a label fits.

## 2. Color

All values are sRGB hex. Alpha tokens are composited by the renderer (GPUI `hsla` alpha), so they work on every surface. Token names are the Rust theme field names with `.` replaced by `_`.

### 2.1 Tokens

| Token | Dark | Light | Used for |
|---|---|---|---|
| `bg.sunken` | `#141210` | `#ECE7DD` | Sidebar, titlebar, terminal (dark), code blocks |
| `bg.base` | `#1A1816` | `#F6F2EA` | Main content area |
| `bg.raised` | `#211E1B` | `#FCFAF5` | Current-state panel, note cards, inputs, terminal (light) |
| `bg.overlay` | `#292622` | `#FFFDF9` | Command palette, menus, dialogs, tooltips, toasts |
| `fill.hover` | `text.primary` @ 6% | same rule | Hovered row or ghost control |
| `fill.selected` | `text.primary` @ 9% | same rule | Selected row, active segment, human author mark |
| `fill.pressed` | `text.primary` @ 10% | same rule | Pressed row or control |
| `border.subtle` | `#2B2824` | `#E6E0D5` | Pane dividers, hairlines between rows, panel outlines |
| `border.default` | `#38342F` | `#D6CFC2` | Overlay outlines, decision entry box, kbd hint |
| `border.strong` | `#7A7468` | `#8A8273` | Input and secondary-button outline, Claude Code author mark, scrollbar thumb |
| `text.primary` | `#E9E3D8` | `#26221D` | Titles, body, values |
| `text.secondary` | `#B5AD9F` | `#575045` | Labels, tree rows at rest, entry type, icons at rest |
| `text.tertiary` | `#9A9285` | `#6A6357` | Timestamps, counts, placeholders, breadcrumb, hints |
| `text.disabled` | `#625C52` | `#A69F92` | Disabled controls only (exempt from AA by definition) |
| `accent` | `#72B5A2` | `#2B6F61` | Primary button fill, link text, focus ring, selection marker, Active glyph, unread dot, terminal cursor |
| `accent.hover` | `#86C4B2` | `#235E52` | Primary button hover, link hover |
| `accent.pressed` | `#5E9F8D` | `#1C4E44` | Primary button pressed |
| `accent.subtle` | `accent` @ 16% | same rule | Remote-change highlight, palette scope chip, matched-drop target |
| `text.on-accent` | `#10201C` | `#FBF8F2` | Text and icons on `accent` and on `danger` fills |
| `danger` | `#DE7B70` | `#AD4337` | Destructive button, error text and icon, invalid input outline |
| `warning` | `#D4A24C` | `#85590A` | Blocked glyph, edit-conflict and update-failed icon |
| `success` | `#9BB26B` | `#4C6A24` | Done glyph, confirmation toast icon |
| `selection` | `accent` @ 30% | same rule | Text selection in editors, rendered Markdown and terminal |
| `focus.ring` | = `accent` | = `accent` | 2px keyboard focus ring |
| `scrim` | `#000000` @ 45% | `#26221D` @ 25% | Behind dialogs only |
| `shadow.overlay` | `0 8px 24px #000000` @ 40% | `0 8px 24px #26221D` @ 14% | Palette, menus, dialogs, tooltips, toasts |
| `caption.close.hover` | `#C42B1C` | `#C42B1C` | Windows/Linux close button hover (platform convention), glyph `#FFFFFF` |

Elevation is tone, lightest on top in dark and in light: `sunken < base < raised < overlay`. A surface never sits on a surface more than one step lighter than itself except overlays.

Why verdigris: the semantic colours (red, amber, green) are all warm, so a warm accent (clay, ochre) would collide with danger or warning on a focus ring next to an error. A grey-green copper patina is the only hue that stays calm on warm neutrals, separates from all three semantics, and is not the default developer-tool blue.

### 2.2 Measured contrast (WCAG 2.x, computed from the hex values)

| Pair | Dark | Light | Requirement |
|---|---|---|---|
| `text.primary` on `bg.sunken` / `base` / `raised` / `overlay` | 14.63 / 13.86 / 12.99 / 11.79 | 12.82 / 14.15 / 15.15 / 15.55 | 4.5 |
| `text.secondary` on same four | 8.40 / 7.96 / 7.46 / 6.77 | 6.46 / 7.13 / 7.63 / 7.83 | 4.5 |
| `text.tertiary` on same four | 6.07 / 5.75 / 5.39 / 4.89 | 4.82 / 5.32 / 5.69 / 5.85 | 4.5 |
| `accent` (link, glyph) on same four | 7.87 / 7.46 / 6.99 / 6.34 | 4.80 / 5.30 / 5.67 / 5.82 | 4.5 |
| `danger` on same four | 6.38 / 6.05 / 5.67 / 5.14 | 4.69 / 5.17 / 5.54 / 5.69 | 4.5 |
| `warning` on same four | 8.07 / 7.65 / 7.16 / 6.50 | 4.97 / 5.48 / 5.87 / 6.03 | 3.0 (glyph) |
| `success` on same four | 7.99 / 7.57 / 7.09 / 6.43 | 5.02 / 5.54 / 5.93 / 6.09 | 3.0 (glyph) |
| `text.on-accent` on `accent` / `.hover` / `.pressed` | 7.10 / 8.47 / 5.48 | 5.58 / 7.09 / 8.91 | 4.5 |
| `text.on-accent` on `danger` | 5.76 | 5.45 | 4.5 |
| `text.primary` on `fill.selected` over `sunken` / `base` / `overlay` | 11.98 / 11.15 / 9.26 | 10.84 / 11.92 / 13.07 | 4.5 |
| `text.secondary` on `fill.hover` over `sunken` / `base` / `overlay` | 7.42 / 6.94 / 5.79 | 5.78 / 6.37 / 6.99 | 4.5 |
| `text.primary` on `accent.subtle` over `sunken` / `base` / `overlay` | 11.24 / 10.48 / 8.80 | 10.42 / 11.39 / 12.42 | 4.5 |
| `text.primary` on `selection` over `base` | 7.73 | 9.29 | 4.5 |
| `border.strong` vs `sunken` / `base` / `raised` / `overlay` | 4.03 / 3.82 / 3.57 / 3.25 | 3.09 / 3.41 / 3.65 / 3.74 | 3.0 (control boundary) |
| `focus.ring` vs `sunken` / `base` / `overlay` | 7.87 / 7.46 / 6.34 | 4.80 / 5.30 / 5.82 | 3.0 |
| `#FFFFFF` on `caption.close.hover` | 5.66 | 5.66 | 4.5 |

Rules that follow from the numbers:
- `text.tertiary` is not allowed on `fill.selected` over `bg.overlay` (3.84 dark). In a selected palette or menu row, metadata switches to `text.secondary`.
- Text on `accent.subtle` is always `text.primary`. Coloured text on a tinted fill is not allowed (it measures 3.8 to 4.8).
- `border.subtle` and `border.default` are separators (1.2 to 1.5 against their surface), never the only boundary of an interactive control.
- `text.disabled` (2.3 to 2.8) is used only with a disabled control, never for information.

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

No "In review", no priorities, no custom statuses in v1: each addition makes the agent's writes less predictable. Status never relies on colour: every status has a distinct glyph interior (dashes, empty, dot, bars, check, slash), the topic header always shows glyph plus word, and the glyph has a tooltip and an accessible name with the word. Closed topics render their title in `text.tertiary` (no strikethrough) and collapse under a "3 closed" row at the end of their epic.

### 2.4 Terminal palette

Terminal background is `bg.sunken` in dark and `bg.raised` in light. Cursor: block, `accent`, character under it `bg.sunken` (7.87); unfocused cursor is a 1px `accent` outline. Selection: `selection`, text colours unchanged (foreground on it: 7.29 dark, 9.83 light). Ratios below are against the terminal background.

| Slot | Dark | Ratio | Light | Ratio |
|---|---|---|---|---|
| foreground | `#DCD6CA` | 12.92 | `#26221D` | 15.15 |
| 0 black | `#2B2824` | 1.27 (background use) | `#26221D` | 15.15 |
| 1 red | `#DE7B70` | 6.38 | `#AD4337` | 5.54 |
| 2 green | `#9BB26B` | 7.99 | `#4C6A24` | 5.93 |
| 3 yellow | `#D4A24C` | 8.07 | `#85590A` | 5.87 |
| 4 blue | `#7FA6CC` | 7.32 | `#2F5F8F` | 6.38 |
| 5 magenta | `#BC96BC` | 7.32 | `#844C86` | 5.99 |
| 6 cyan | `#72B5A2` | 7.87 | `#2B6F61` | 5.67 |
| 7 white | `#B5AD9F` | 8.40 | `#6A6357` | 5.69 |
| 8 bright black | `#9A9285` | 6.07 | `#6A6357` | 5.69 |
| 9 bright red | `#EC9A90` | 8.54 | `#963529` | 7.09 |
| 10 bright green | `#B4C888` | 10.29 | `#3E591B` | 7.60 |
| 11 bright yellow | `#E4BB70` | 10.36 | `#6F4A06` | 7.57 |
| 12 bright blue | `#9DBDDD` | 9.57 | `#244D76` | 8.39 |
| 13 bright magenta | `#D0B0D2` | 9.63 | `#6E3D70` | 7.85 |
| 14 bright cyan | `#93CCBB` | 10.32 | `#235E52` | 7.20 |
| 15 bright white | `#F2EDE3` | 16.02 | `#575045` | 7.63 |

Red, green, yellow and cyan are the app's `danger`, `success`, `warning` and `accent`, so the terminal and the tracker read as one object. Bright black is `text.tertiary` because Claude Code sets a lot of secondary text in it; it must stay readable. Blue and magenta exist only here.

## 3. Typography

### 3.1 Families

| Role | Family | Files bundled | License |
|---|---|---|---|
| UI and reading | **Source Sans 3** (Adobe, Paul D. Hunt) | Regular 400, Italic 400, Semibold 600, Semibold Italic 600 (static TTF) | SIL OFL 1.1 |
| Code and terminal | **JetBrains Mono NL** (no-ligature build of JetBrains Mono) | Regular 400, Italic 400, Bold 700 (static TTF) | SIL OFL 1.1 |

Why Source Sans 3: a humanist sans with open apertures and a calligraphic italic, drawn for long UI and document reading; it gives the paper-and-ink warmth that a neo-grotesque cannot, and its simple, open shapes hold up under grayscale antialiasing without hinting. This is conditional on the M1 rendering check in GPUI on Windows; the named fallback is **IBM Plex Sans** (SIL OFL 1.1; Regular 400, Italic 400, SemiBold 600, SemiBold Italic 600), with the scale unchanged. Its x-height is modest, so the UI base is 14px, not 13px. Why JetBrains Mono NL: tall x-height and unambiguous shapes at 13px, full box-drawing coverage for Claude Code's TUI, and the NL build removes ligatures at the file level so the terminal never depends on a feature flag. Bold 700 exists only for terminal bold.

Static files only (no variable fonts), two weights of the sans: 400 and 600. There is no 500 and no 700 in the UI. OpenType features: `kern` and `liga` at defaults; enable `tnum` + `lnum` (tabular lining figures) explicitly wherever digits align or tick: timestamps, counts, version numbers, shortcut hints. Proportional figures in prose. Fallback order for missing glyphs: bundled family, then the platform UI font (Segoe UI, SF Pro, system sans), then the platform symbol font; the terminal falls back to the platform mono (Cascadia Mono, Menlo, DejaVu Sans Mono).

### 3.2 Scale

| Role token | Family | Size / line | Weight | Tracking | Used for |
|---|---|---|---|---|---|
| `type.title` | Sans | 22 / 28 | 600 | -0.2px | Topic title, epic title, first-run heading |
| `type.heading` | Sans | 17 / 24 | 600 | 0 | Dialog title, empty-state heading, Markdown h1/h2 |
| `type.reading` | Sans | 15 / 24 | 400 | 0 | Current state, description, timeline bodies, notes |
| `type.document` | Sans | 16 / 26 | 400 | 0 | Document body, in the reading view and in its editor |
| `type.ui` | Sans | 14 / 20 | 400 | 0 | Everything else: rows, buttons, inputs, menus |
| `type.ui-strong` | Sans | 14 / 20 | 600 | 0 | Author names, selected segment, primary button, Markdown h3 |
| `type.small` | Sans | 13 / 18 | 400 | 0 | Timestamps, counts, breadcrumb, tooltips, hints |
| `type.label` | Sans | 13 / 18 | 600 | +0.1px | Section labels ("Current state", "Timeline"), sidebar group names |
| `type.code` | Mono | 13 / 20 | 400 | 0 | Inline code, code blocks, "Claude Code" author name, kbd hints (12 / 16) |
| `type.terminal` | Mono | 13 / 17 | 400 / 700 | 0 | Terminal grid (cell 7.8 x 17); size adjustable 11 to 18 with `Mod+=` / `Mod+-` |

Sentence case everywhere. No uppercase, no letter-spaced labels beyond `type.label`, no italics for UI (italics are for Markdown emphasis only). Minimum text size is 12px (kbd hints); nothing smaller.

### 3.3 Long-form text

Descriptions, current state, timeline entries and notes are stored as **Markdown (CommonMark)** and rendered when not being edited. This is the format Claude Code writes natively and the human can type without a toolbar. Editing shows the raw Markdown in the same face and size (`type.reading`), so entering edit mode does not reflow the paragraph.

Rendered in v1: paragraphs, `*emphasis*`, `**strong**` (600), inline code, fenced code blocks (no syntax highlighting), links, ordered and unordered lists (two nesting levels), task lists (read-only checkboxes), blockquotes, headings, horizontal rule. Tables are rendered too (Claude Code writes them constantly in specs). Not rendered in v1: images, HTML, footnotes. Unknown syntax shows as typed.

Setting: `type.reading` in `text.primary`; paragraph gap 12; list item gap 4, indent 20, marker in `text.tertiary`; headings h1/h2 as `type.heading` and h3 as `type.ui-strong`, 24 above and 8 below (a timeline entry rarely needs them, so they stay small); blockquote indented 16 in `text.secondary` with no bar; inline code `type.code` on `bg.sunken`, padding 0 x 4, radius 4; code block `type.code` on `bg.sunken`, padding 12, radius 6, horizontal scroll, never wraps; links `accent`, underlined on hover and focus, open in the system browser; rule 1px `border.subtle` with 16 above and below.

Tables: `type.ui`, cells padded 6 x 12, header row weight 600, 1px `border.subtle` under each row and no vertical lines, column alignment follows the Markdown alignment row (left by default), numeric columns right-aligned with `tnum`, cell text wraps at a 320 maximum column width; a table wider than the column scrolls horizontally on its own, like a code block.

**Documents are the long-form surface** (ADR 0007): specs and architecture notes that are read for minutes, not glanced at. They get their own setting, used only in the document reading view and its editor:
- Body `type.document` 16 / 26 in `text.primary`; paragraph gap 16; list item gap 6, indent 24.
- Headings carry real structure there, because the agent addresses sections by heading: h1 `type.title` (32 above, 12 below); h2 `type.heading` (32 above, 8 below); h3 16 / 26 weight 600 (24 above, 4 below); h4 and deeper as h3 in `text.secondary`. A heading is never the last line before a scroll boundary without its first paragraph line (keep-with-next when paginating by keyboard).
- Code blocks, inline code, tables, blockquotes, links and rules as above; code stays 13 / 20 so 80 columns fit the measure.
- Short surfaces (current state, description, entries, notes) keep `type.reading` 15 / 24 and the compact heading sizes: they sit among UI and must not out-shout the topic title.

Measure: the content column is at most **680px** (about 78 characters of Source Sans 3 at 15px, about 73 at 16px) and never narrower than 360px. Text is left-aligned, ragged right, no hyphenation. Single-line titles truncate with an ellipsis in lists and wrap in the topic header.

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
+--------------------------------------------------------------------------+
| Titlebar 36                                                              |
+---------------+------------------------------------+---------------------+
| Sidebar       | Main                               | Terminal (optional) |
| bg.sunken     | bg.base                            | bg.sunken           |
| 260           | flexible, min 420                  | 648 = 80 columns    |
| (200 to 400)  | content column max 680, centred    | (400 to 60% window) |
+---------------+------------------------------------+---------------------+
```

- **No status bar.** Everything that would live there (update, terminal toggle) is in the titlebar; a second chrome strip costs 24px of every reading session.
- **Sidebar**: project switcher row (36), Inbox row with count, Knowledge row (`library` icon, count of entries in the project, unread dot when Claude Code has added some), then the epic/topic tree, each epic collapsible; topics outside any epic are listed first under no heading. Width 260, resizable 200 to 400, hidden with `Mod+B`. Padding 8, rows inset 8, tree indent 16 per level.
- **Main**: one view at a time: topic, epic (title, description, knowledge, topic list), project (title, description, knowledge, epics), inbox, knowledge base, document, trash, settings. Section order in a topic is fixed: title and status, current state, description, knowledge, timeline; the timeline is last because it is the only unbounded section. The content column is centred with at least 32 of side padding and 32 above the title. No tabs; switching is so cheap (`Mod+P`, `Alt+Left`) that tabs would only add state to manage.
- **Terminal**: docked right by default, because Claude Code is a tall conversation and a topic is a narrow column; they sit side by side like a page and its margin. Default width fits exactly 80 columns (80 x 7.8 + 2 x 12 padding = 648). Can be moved to the bottom (palette: "Move terminal to bottom"; default height 320, range 160 to 70% of the window) and zoomed to fill the main area (`Mod+Shift+Enter`). One terminal per project; switching project switches the session and leaves the other running. Hidden terminals keep running.
- **Window**: default 1440 x 900, minimum **880 x 560**. Widths and visibility of panes are remembered per project.
- **Narrow behaviour**: panes give way in a fixed order, sidebar first, then main, never the terminal's 400 minimum. If sidebar + 420 + terminal no longer fit, the sidebar auto-hides (it returns when space returns; `Mod+B` still shows it as an overlay on top of main, 260 wide, with `shadow.overlay`, dismissed by `Esc` or by opening something). At 880 with the terminal open the split is main 420 / terminal 460. Nothing ever scrolls horizontally except code blocks.

### 4.3 Titlebar (client-side, 36 high, `bg.sunken`, bottom border `border.subtle`)

Content, left to right after the platform inset: sidebar toggle (`panel-left`), project switcher (project name in `type.ui-strong` plus `chevron-down`), drag region, update indicator (only when relevant), "Search" button showing the palette shortcut, terminal toggle (`square-terminal`). The whole bar is a drag region except the controls; double-click follows the platform (zoom on macOS, maximize elsewhere).

| Platform | Window controls | Rules |
|---|---|---|
| macOS | Native traffic lights, left, inset 12 from the left edge, vertically centred | Content starts at x = 76. In full screen the lights disappear and content starts at x = 12. Never redraw the lights. |
| Windows | Minimize, maximize/restore, close on the right, each 46 x 36, drawn by the app with 10px 1px-stroke glyphs | Hover `fill.hover`, pressed `fill.pressed`; close hover `caption.close.hover` with a white glyph. Hit-test regions must report as native caption buttons so Snap Layouts appears on maximize hover. 1px `border.subtle` window outline when not maximized. |
| Linux | Client-side controls on the right in the same style as Windows, 36 x 36 each | Honour the desktop's button layout and side when the platform reports it. When the compositor provides server-side decorations (tiling window managers), keep the system titlebar and render this bar as a plain 36 toolbar without controls. |

When the window is inactive, titlebar text and icons drop to `text.tertiary`. Nothing else changes.

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

**List / tree row** (height 28, padding 0 x 8, `radius.sm`, gap 8): disclosure chevron 16 (epics only; `chevron-right` / `chevron-down`), status glyph 14 (topics), title `type.ui` `text.secondary` truncated, trailing slot (unread dot 6 `accent`, or count in `type.small` `text.tertiary` with `tnum`). Hover: `fill.hover`, title `text.primary`. Selected (the item open in main): `fill.selected`, title `text.primary` at weight 600. Keyboard cursor: focus ring; cursor and selection are separate until `Enter`. Dragging: row at 60% opacity, 2px `accent` insertion line between rows, `accent.subtle` fill on an epic that would receive the drop. Renaming: the title becomes an inline edit. Epic rows use `type.label`. The "3 closed" row is a ghost row in `type.small` `text.tertiary`.

**Status indicator**: glyph only (14, tree and lists) or glyph 16 plus word in `type.ui` (topic header, menus). In the topic header it is a ghost button (height 28, padding 0 x 8) opening the status menu; shortcut `S`. The menu lists the six statuses in the table order with number hints 1 to 6; the current one has a trailing `check`.

**Segmented control** (height 28, 1px `border.default`, `radius.sm`, segments padding 0 x 12, `type.ui` `text.secondary`): selected segment `fill.selected`, `type.ui-strong`, `text.primary`; hover `fill.hover`; arrows move between segments when focused. Used for the timeline filter (All / Progress / Decisions / Discussion), the composer entry type and the inbox filter (Open / Filed). There are no tabs in v1.

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

**Knowledge base view** (a main view; the destination for "what does this project know"). Reached from the sidebar row "Knowledge" under Inbox (`Mod+L`), from the roll-up rows above, and scoped to an epic from that epic's section. Top to bottom in the 680 column: title "Knowledge" (`type.title`) with the scope under it in `type.small` `text.tertiary` ("Acta · 12 documents, 43 memories"); a search input (28, full width, `search` icon, placeholder "Search knowledge", focused with `/`); a filter row of three compact secondary dropdown buttons, "Level: All", "Kind: All", "Author: All" (menus: All / Project / Epics / Topics; All / Documents / Memories; All / Alexandre / Claude Code), a non-default filter showing its value in `text.primary` weight 600, and a ghost "Clear" when any is set; then the list.
- The list is grouped by owner, widest first: "Project Acta", "Epic Terminal", "Topic Embed the terminal" (group header 28: `type.small` `text.tertiary`, the name a link to the owner, count at the right). Rows are knowledge rows and behave identically (documents open, memories unfold, same menu). `J` / `K` cross groups; `Left` on a row jumps to its group header; `Enter` on a header folds the group.
- Searching is full text and instant (budget as the palette): groups with no match disappear, the summary is replaced by the matching excerpt with the matched words in `text.primary` weight 600, and the header line reads "7 results for "resize"". No results: "Nothing in this project mentions "resize"." Filters and search combine; both persist while the app runs and reset on restart.
- This view is also where the human reviews the agent's memory: "Author: Claude Code" plus "Kind: Memories" is one saved habit, so the palette has the command "Review memories written by Claude Code", which opens the view with those two filters, newest first, ungrouped.

**Document reading view** (a main view, part of navigation history; the tree keeps the owner selected; a memory opened in full uses it unchanged). Top to bottom in the 680 column: breadcrumb (`type.small` `text.tertiary`: "Acta / Terminal / Embed the terminal", each part a link); title (`type.title`); summary (`type.reading` `text.secondary`); meta row (author mark 20, "Updated 2 h ago by Claude Code", then ghost buttons Edit `E`, History `H`, and `ellipsis`: Go to heading, Save as file, Move, Delete); 24; a 1px `border.subtle` rule; 24; the body in the document setting of 3.3. No outline column: "Go to heading" (`Mod+Shift+O`) opens the palette scoped to this document's headings, with each section's size, which is the same outline the agent sees. `Space` / `Shift+Space` page, `Home` / `End` jump. Remote changes follow 6.5: the changed section gets the highlight, the viewport is anchored on the first visible block.

**Document editor**: the reading view switches in place, without moving the scroll position. Title becomes an inline title edit; summary becomes a text input with the placeholder "One line: what this is and when to read it" and a `tnum` counter in `type.small` `text.tertiary` ("64 / 140"); the body becomes a borderless raw-Markdown editor in `type.document` filling the column (no box: the page is the editor). A 44 bar is pinned to the bottom of the main pane (`bg.base`, top border `border.subtle`): left, "Editing" plus the draft state in `type.small` `text.tertiary`; right, Cancel (ghost, `Esc`) and Save (primary, `Mod+Enter`). Save is disabled with the hint "A document needs a title and a summary." until both exist. Every save keeps the previous version, so there is no confirmation on save and none on cancel (the draft is kept). Markdown editing aids: `Enter` continues lists, `Tab` / `Shift+Tab` indent, `Mod+B` / `Mod+I` wrap the selection in `**` / `*` (inside an editor these win over the global shortcuts); nothing else, no toolbar.

**History** (documents and memories alike; for a memory, `H` opens it in the reading view first): `H` replaces the body with the history, same view, same header. A list of revisions, newest first (rows of 32: author mark, "Claude Code" or name, what changed in `text.secondary` ("Replaced section Storage", "Edited", "Created from file"), lines added and removed with `tnum`, time); at most 6 rows visible, then scroll. Below the list, the selected revision rendered read-only, headed by a notice "Version from Tue 14:05 by Claude Code" with a segmented control (Text / Changes) and a secondary button "Restore this version". Changes shows the raw Markdown with a 16 gutter: `+` in `success` for added lines, `-` in `danger` for removed lines whose text is `text.secondary`; no background tints. Restoring writes the old text as a new revision on top, so it is undoable, needs no dialog, and shows the toast "Restored the version from Tue 14:05. Undo". `Esc` or `H` returns to reading.

**Notice** (the one inline message component; used for edit conflicts, deleted-while-open, version headers, plaintext warnings, migration reports): full column width, `bg.raised`, 1px `border.subtle`, `radius.md`, padding 8 x 12, optional icon 16 (`info` in `text.secondary`, `triangle-alert` in `warning` or `danger`), message in `type.ui` `text.primary`, ghost actions right-aligned. Never a coloured fill, never dismisses itself, never more than one per view.

**Checkbox** (export options only): 16 box, `radius.sm`, 1px `border.strong`, label `type.ui` 8 to its right; checked: fill `accent`, `check` 12 in `text.on-accent`; hover `fill.hover` inside the box; focus ring; disabled `border.default` and `text.disabled`.

**Export dialog** (palette: "Export...", also in settings). Dialog of 440. Title "Export". Scope: segmented control (Everything / This project). Then a notice with `triangle-alert` in `warning`: "The file is not encrypted. Anyone who can open it can read these projects. That is what lets you read it without Acta or its key." Then a checkbox "Protect with a passphrase"; when checked, two inputs (passphrase, repeat) and the line "Acta cannot recover a lost passphrase." in `type.small` `text.secondary`. Buttons: Cancel, "Choose location..." (primary; opens the system save dialog, default name `acta-2026-11-20.acta.json`). Result: toast "Exported 4 projects to acta-2026-11-20.acta.json. Show in folder". The same one-line warning appears on "Save document as file". Settings shows "Last export: 12 days ago" in `type.small`; once, after the tenth topic exists, a notice at the top of the main view suggests a first export ("Everything is stored on this computer only. An export is your copy if the system keychain is ever lost.", actions Export..., Dismiss) and never returns.

**Import** (palette: "Import..."): system open dialog, then, if the file is protected, a passphrase input; then the **import preview**, a dialog of 520. Title: Import "acta-2026-11-20.acta.json". Meta line in `type.small` `text.tertiary`: "Exported 20 Nov 2026 · format 1 · 4 projects". A table (see 8.8): one row per kind (Projects, Epics, Topics, Entries, Notes, Documents, Memories), three `tnum` columns Created / Updated / Skipped, zeros in `text.tertiary`. Below: "Nothing here will be deleted. Skipped records are the same as, or older than, what Acta already has." and, when Updated is not zero, a ghost "Show what changes" expanding the titles of updated records (max 240 high, scrolls). Buttons: Cancel (focused), Import. Nothing is written before Import; Acta takes a snapshot first, so the result line reads "Imported: 12 created, 3 updated. A snapshot from before the import is kept." Refusals replace the table with one notice and a single Close button: "This file was made by a newer Acta (format 3). Update Acta, then import it." / "This is not an Acta export." / "Wrong passphrase."

**Trash view** (palette: "Open trash"; a main view). Title "Trash", line "Deleted things stay here for 30 days." in `type.small` `text.tertiary`. Rows of 32 grouped by day of deletion (group headers `type.small` `text.tertiary`): kind icon 16 (`folder`, `layers`, status glyph, `message-square`, `sticky-note`, `file-text`, `pin`), title `type.ui`, original place in `type.small` `text.tertiary`, "by Claude Code" or the name, days left ("28 days left", `tnum`). `Enter` or `R` restores to the original place and shows "Topic restored. Undo"; if its epic is also in the trash, both come back and the toast says so. A project row counts its content ("31 topics, 12 notes"). No "empty trash" and no "delete now" in v1: the trash exists so that nothing is ever lost by a key press, and a purge button is exactly such a key press. Empty: "Trash is empty."

**Snapshot restore** (palette: "Restore from snapshot..."): dialog of 520 listing snapshots, newest first, rows of 32: date and time (`tnum`), kind ("Daily", "Weekly", "Before update to 0.4.1", "Before import", "Before restore"), size. Selecting one enables the destructive-styled button "Restore and restart" and shows: "Acta will restart with its data as of 18 Nov 2026, 09:12. Everything as it is now is saved as a snapshot first, so you can come back." This is one of the confirmed cases in 6.4. After restart a notice reads "Restored the snapshot from 18 Nov 2026, 09:12." If a migration fails after an update, Acta restores by itself and the next launch shows the notice "The update to 0.4.1 could not convert the database. Acta put back the snapshot from just before; nothing was lost." with the action "Details".

**Key-loss recovery screen** (replaces the whole window content, like first run; see 8.9). Shown when the database file exists but its key is not in the system keychain. Calm and exact: no red, no icon larger than 16, no "error" in the heading. It states what is true (the file is intact, unreadable, untouched), why it usually happens, and offers three actions in order of safety: "Import an export..." (primary), "Try again" (secondary; re-reads the keychain, for a locked or slow keychain), "Start empty" (ghost; the unreadable file is renamed and kept, the screen says its new name). Acta never creates a database over the old one and never deletes the kept file.

**Command palette**: width 640 (or window width minus 48), top edge 96 below the window top, `bg.overlay`, 1px `border.default`, `radius.lg`, `shadow.overlay`, no scrim (the user keeps seeing what they are acting on). Input row 44: `search` icon 16, text `type.reading`, optional scope chip (height 24, `accent.subtle`, `radius.sm`, `type.small` `text.primary`: "Topics", "Projects", "Status"). Results: up to 10 rows of 32, grouped under `type.label` headers in `text.tertiary` (Topics, Knowledge, Commands, Projects; a knowledge row shows its kind icon, its title, and its summary in place of the path); row = icon or status glyph 16, title `type.ui`, path in `type.small` `text.tertiary` ("Acta / Terminal"), right-aligned shortcut hint. Matched characters are `text.primary` weight 600 on a `text.secondary` title. Selected row `fill.selected`; the first row is always selected; hover does not move the selection until the pointer actually moves. Footer 28, `border.subtle` top, hints in `type.small`: "Enter open", "Mod+Enter open beside terminal", "Esc close". No results: one line, "Nothing matches "oauth". Enter creates a topic with this title."

**Context menu / dropdown menu**: `bg.overlay`, 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4, min width 200, max 320. Item 32 (28 is too tight for a pointer menu), padding 0 x 8, icon 16 optional, label `type.ui`, shortcut hint right. Hover and keyboard highlight are the same single `fill.selected` row. Destructive item: `danger` text, last, after a 1px `border.subtle` separator with 4 above and below. Disabled: `text.disabled`. Type-ahead selects by first letters. Submenus only for "Move to epic".

**Tooltip**: `bg.overlay`, 1px `border.default`, `radius.md`, `shadow.overlay`, padding 4 x 8, `type.small` `text.primary`, optional kbd hint after the label. Appears after 500 ms of hover or immediately on keyboard focus, 6 away from the target, never over it; once one has shown, neighbours show instantly until the pointer rests elsewhere for 1 s. One line, no wrapping, no tooltip repeating a visible label.

**Toast**: bottom-left of the main pane, 16 from the edges (bottom-right is the terminal), width 360 max, `bg.overlay`, 1px `border.default`, `radius.md`, `shadow.overlay`, padding 8 x 12, `type.ui`. Optional leading icon 16 (`check` in `success`, `triangle-alert` in `danger` or `warning`), message, one ghost action ("Undo") with its kbd hint, then a ghost `x`. Stays 6 s (errors stay until dismissed), pauses while hovered or focused; at most one visible, a new one replaces the old. Toasts exist for undo and for failures only.

**Dialog**: centred, width 440, `bg.overlay`, 1px `border.default`, `radius.lg`, `shadow.overlay`, over `scrim`. Padding 24; title `type.heading`; body `type.ui` `text.secondary`, 8 below the title; buttons right-aligned, 24 below the body, gap 8, the confirming action last (first on Windows, following the platform). Focus starts on the safe button; `Esc` cancels; `Enter` activates the focused button only. Focus is trapped inside.

**Empty state**: left-aligned at the top of the content column, never centred in the pane, never illustrated. Heading `type.heading`, one sentence in `type.reading` `text.secondary` (max 480 wide), then the one action that fixes it as a button with its kbd hint. See 8.5.

**Scrollbar**: overlay, 8 wide track with no fill, thumb 4 wide (8 when hovered or dragged), fully rounded, `border.strong` at 50% opacity (100% hovered), minimum length 32. Appears while scrolling or when the pointer is within 12 of the edge, hides 800 ms after. Never takes layout space.

**Resize handle**: the 1px `border.subtle` divider between panes with an invisible 8 hit area centred on it. Hover for 150 ms or drag: the line becomes 2px `accent` and the cursor changes to the resize cursor. Dragging resizes live, every frame, clamped to the pane limits; double-click restores the default. Keyboard: palette commands "Widen sidebar", "Narrow sidebar", "Widen terminal", "Narrow terminal" (steps of 40).

**Keyboard shortcut hint (kbd)**: each key in its own box: height 18, min width 18, padding 0 x 4, `radius.sm`, 1px `border.default`, `type.code` at 12 / 16, `text.tertiary`, gap 2 between keys. macOS shows glyphs (`⌘ ⇧ ⌥ ⌃ ↵`), Windows and Linux show words (`Ctrl`, `Shift`, `Alt`, `Enter`). Inside a primary button the hint has no border and uses `text.on-accent` at weight 400.

**Update indicator** (titlebar, compact ghost button, height 24): hidden when up to date and while downloading; updates are fetched silently and applied at next launch. **Ready**: 6 dot in `accent` plus "Restart to update" in `type.small` `text.secondary`; click or palette command restarts into the new version and restores the window, project, topic and terminal session. **Just updated** (first launch on a new version): "Updated to 0.4.1" in `type.small` `text.tertiary` for that session, opening the release notes; no dot. **Failed** three launches in a row: `triangle-alert` 14 in `warning` plus "Update failed"; click shows the reason and a "Download manually" link. Never a dialog, never a toast, never a badge on anything else.

## 6. Interaction

### 6.1 Keyboard map

| Action | Windows / Linux | macOS |
|---|---|---|
| Command palette (everything) | `Ctrl+K`, also `Ctrl+Shift+K` | `Cmd+K` |
| Go to topic (palette scoped to topics) | `Ctrl+P` | `Cmd+P` |
| Switch project | `Ctrl+O` | `Cmd+O` |
| Capture a note (from anywhere) | `Ctrl+N` | `Cmd+N` |
| New topic (in the current epic) | `Ctrl+T` | `Cmd+T` |
| Add a timeline entry to the current topic | `Ctrl+E` | `Cmd+E` |
| Go to inbox | `Ctrl+I` | `Cmd+I` |
| Open knowledge (palette scoped to documents and memories in scope, nearest level first) | `Ctrl+D` | `Cmd+D` |
| Go to the project's knowledge base | `Ctrl+L` | `Cmd+L` |
| New document / new memory (owned by the open project, epic or topic) | `Ctrl+Shift+D` / `Ctrl+Shift+M` | `Cmd+Shift+D` / `Cmd+Shift+M` |
| Go to heading (in a document) | `Ctrl+Shift+O` | `Cmd+Shift+O` |
| Back / forward in navigation history | `Alt+Left` / `Alt+Right` | `Cmd+[` / `Cmd+]` |
| Toggle terminal (show and focus; if focused, hide) | ``Ctrl+` `` | ``Ctrl+` `` |
| Zoom terminal over the main pane | `Ctrl+Shift+Enter` | `Cmd+Shift+Enter` |
| Toggle sidebar | `Ctrl+B` | `Cmd+B` |
| Focus sidebar / main / terminal | `Ctrl+1` / `2` / `3` | `Cmd+1` / `2` / `3` |
| Cycle panes | `F6` / `Shift+F6` | `F6` / `Shift+F6` |
| Save the editor in focus | `Ctrl+Enter` | `Cmd+Enter` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Shift+Z` or `Ctrl+Y` | `Cmd+Z` / `Cmd+Shift+Z` |
| Settings | `Ctrl+,` | `Cmd+,` |
| Keyboard shortcuts list | `Ctrl+/` | `Cmd+/` |

Inside lists and the tree (no modifier, only when a text field is not focused): `Up` / `Down` or `K` / `J` move; `Home` / `End`; `Left` / `Right` collapse / expand or go to parent / first child; `Enter` open; `E` or `F2` rename or edit; `S` set status, then `1` to `6`; `Alt+Up` / `Alt+Down` reorder; `M` move to epic; `Delete` (or `Cmd+Backspace`) delete with undo; `Shift+F10` or the menu key opens the context menu. In the topic view, `Up` / `Down` step through blocks (title, current state, description, composer, each entry) and `Enter` edits the focused block. Type-ahead in the tree jumps to a title.

**The terminal owns the keyboard when focused.** On Windows and Linux every `Ctrl+letter` goes to the shell (`Ctrl+K`, `Ctrl+P`, `Ctrl+N`, `Ctrl+E`, `Ctrl+B` are readline keys). Reserved for the app even there: `Ctrl+Shift+K` (palette), ``Ctrl+` ``, `Ctrl+1/2/3`, `F6`, `Ctrl+Shift+Enter`, plus `Ctrl+Shift+C` / `Ctrl+Shift+V` for copy and paste. That is why `Ctrl+Shift+K` exists everywhere: one palette key that always works. On macOS all `Cmd` shortcuts keep working and `Ctrl` goes to the shell.

**The palette is the universal entry point.** Every command above, every topic, epic and project, every status change ("Set status: Blocked"), layout command and setting is reachable from `Mod+K`. Each row shows its direct shortcut, so the palette teaches the keyboard. With a topic open, typing a status name or "entry" ranks actions on that topic first.

Palette commands for knowledge and data (exact labels): "Open document or memory...", "Go to knowledge base", "Search knowledge...", "Review memories written by Claude Code", "New document", "New memory", "New document from file...", "Edit", "Show history", "Restore this version", "Go to heading...", "Save as file...", "Move to...", "Change kind to memory" / "Change kind to document", "Export...", "Import...", "Open trash", "Restore from snapshot...". A trailing ellipsis means the command asks for something before acting.

**Quick capture** (`Mod+N`): a palette-sized overlay (640 wide, same position and surface) containing one multi-line editor with the placeholder "Note for Acta" and the project name as a chip. `Mod+Enter` saves to that project's inbox and closes, with no toast (the inbox count increments, which is feedback enough); `Esc` closes and keeps the draft. Works while the terminal is focused via the palette. Capture never navigates away from what the user was reading.

**Quick entry** (`Mod+E`): focuses the composer at the top of the timeline, scrolling it into view if needed, with the type preset to Progress; `Alt+1/2/3` switches Progress / Decision / Discussion; `Mod+Enter` saves and returns focus to where it was.

### 6.2 Focus

- Exactly one pane is active. The active pane's selection uses `fill.selected`; in inactive panes the same fill stays and the keyboard ring is hidden. Nothing else marks the active pane.
- Focus is always visible when moved by keyboard (2px `focus.ring`). Pointer clicks do not draw rings on buttons and rows but always do on text fields.
- Overlays (palette, capture, menu, dialog) take focus on open and return it to the exact previous element on close, including the caret position in an editor.
- Opening a topic from the tree with `Enter` moves focus to main; with `Space` it previews (opens in main, focus stays in the tree).
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
| Launch to interactive window with the last topic open | 400 ms target, 800 ms at most |
| Terminal key echo | next frame |

Nothing blocks the UI thread on disk, decryption or the network. If something exceeds its budget the old content stays in place until the new content is ready; a loading indicator (one line of `type.small` `text.tertiary` text, "Opening..." ) may appear only after 400 ms.

**Never animates**: hover, selection, focus ring, navigation between views, tree expand and collapse, pane show, hide and resize, typing, scrolling to a keyboard target, theme switch, status change.

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
- **Undo** (`Mod+Z`) covers, per project and across navigation, the last 100 of: create, edit, delete, move, reorder, status change, note filing. Deleting a project, epic, topic, note, entry, document or memory removes it immediately and shows a toast "Topic deleted" with Undo; deleted things stay in the trash view for 30 days. Saving, moving or restoring a document or memory is undoable too, since every save keeps the previous version. Undo applies to the human's own actions only; it never reverts something Claude Code wrote (those are edited or deleted explicitly).
- **Confirmation dialogs** exist only for what `Mod+Z` cannot take back: closing a terminal or quitting while a process is running in it, importing (the preview is the confirmation), and restoring a snapshot. Deleting a project is not one of them: it goes to the trash like everything else. Export is a dialog because it has options and a warning, not because it is dangerous. The dialog names the object and the consequence, and its confirming button repeats the verb ("Delete project").

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
| Attribution | Updated by AI 2 hours ago | Updated 2 h ago by Claude Code |
| Placeholder | Enter description here... | What is this topic about, and what does done look like? |

## 8. Key screens

Widths are for the default 1440 x 900 window. `( )` is a status glyph, `[ ]` a control, `·` a separator rendered as 8 of space plus the next item, not a character.

### 8.1 Main window, topic view

```
+------------------------------------------------------------------------------------------------+
| [=] Acta v                                                        [Search  Ctrl K] [>_] - [] X | 36  titlebar, bg.sunken
+----------------------+-------------------------------------------------------------------------+
| 8                    |                               32                                        |
|  Inbox            4  |        Terminal                         <- breadcrumb: epic, type.small |
|                   12 |        Embed the terminal bound to the project   <- type.title 22/28    |
|  (o) Naming          |    12                                                                   |
|                    8 |        [(.) Active v]  Updated 2 h ago  <- status button 28, type.small |
|  v Core UI           |    24                                                                   |
|    (v) App shell     |        +--------------------------------------------------------+       |
|    (.) Sidebar tree  |        | Current state                Updated 2 h ago by Claude Code |  |
|    ( ) Palette     * |        | Rendering and input work on Windows. Resize still       |      |
|  v Terminal          |        | drops the last row; fix is in progress.                 |      |
|   [(.) Embed the..]  |        +--------------------------------------------------------+       |
|    (=) Scrollback    |    24   bg.raised, border.subtle, radius.md, padding 16, type.reading   |
|    (-) Links         |        Description                                  <- type.label       |
|    3 closed          |        Run Claude Code inside Acta, in the project directory, so        |
|  > Notes             |        the tracker and the agent share one window.                      |
|                      |    32                                                                   |
|                      |        Knowledge  3                                          [+]        |
|                      |    4   [d] Resize protocol   How the grid reflows; read b..  [>] 2 h ago|
|                      |        [m] ConPTY minimum    Needs Windows 10 1809 or later  [>] Mon    |
|                      |        [m] No tabs           One session per project, by d.. (A) Tue    |
|                      |        9 more from epic and project          <- ghost row, collapsed    |
|                      |    32   rows 32: kind icon, title, summary, author mark 20, time        |
|                      |        Timeline            [All|Progress|Decisions|Discussion]          |
|                      |    12  +--------------------------------------------------------+       |
|                      |        | Add an entry...                                        |       |
|                      |        +--------------------------------------------------------+       |
|                      |    24                                                                   |
|                      |        [>] Claude Code  Progress  2 h ago    <- square mark, mono name  |
|                      |            Resize now reflows the grid; the last row is still cut       |
|                      |            when the panel is under 12 rows.                             |
|                      |    24                                                                   |
|                      |        (A) Alexandre  Decision  Tue 14:05    <- circle mark, sans name  |
|                      |            +------------------------------------------------+           |
|                      |            | Dock the terminal on the right by default.     |           |
|                      |            +------------------------------------------------+           |
|                      |            (.) Alexandre set status to Active  Tue 13:58  <- system     |
+----------------------+-------------------------------------------------------------------------+
  260 sidebar, bg.sunken        1180 main, bg.base; content column 680 centred (250 each side)
  rows 28, indent 16            [..] around a tree row = fill.selected; * = unread dot (accent, 6)
```

Annotations: the sidebar has no header label and no "Projects" title; the project switcher in the titlebar is the header. Tree glyphs here: `(o)` Draft, `( )` Planned, `(.)` Active, `(=)` Blocked, `(v)` Done, `(-)` Dropped. The composer is one line (28 + padding) until focused, then becomes the multi-line editor. Scrolling: the title block scrolls with the content; nothing is sticky except the "new entries" button.

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
                    | Enter open   Ctrl Enter open beside terminal  Esc close | 28  footer, type.small
                    +---------------------------------------------------------+
                      bg.overlay, border.default, radius.lg, shadow.overlay, no scrim
```

Annotations: empty query shows "Recent" (the last 5 topics) then the 5 most relevant commands for the current view. Group order is fixed (Topics, Commands, Projects) so results do not jump between keystrokes; within a group, ranking is match quality then recency. `Up` / `Down` wrap; `Tab` narrows to the group of the selected row (adds the scope chip); `Backspace` on an empty input removes the chip.

### 8.4 Terminal open beside a topic

```
+------------------------------------------------------------------------------------------------+
| [=] Acta v                                           [Search  Ctrl K] [>_ on] - [] X           | 36
+----------------------+-------------------------------------+-----------------------------------+
|  Inbox            4  |  32                                 | 12                                |
|  ...                 |  Terminal                           |  > claude                         |
|   [(.) Embed the..]  |  Embed the terminal bound to        |                                   |
|                      |  the project                        |  * Fixing the resize path in      |
|                      |  [(.) Active v]  Updated just now   |    crates/terminal/src/grid.rs    |
|                      |  +-------------------------------+  |                                   |
|                      |  | Current state                 |  |  * acta entry add "Resize fixed"  |
|                      |  | Resize is fixed on Windows.   |  |                                   |
|                      |  +-------------------------------+  |  > _                              |
|                      |  ^ remote-change highlight, once    |                                   |
+----------------------+-------------------------------------+-----------------------------------+
  260                    532 main (min 420), column fills     648 terminal = 80 cols, bg.sunken
                         it with 32 side padding              padding 12, no header, no tabs
```

Annotations: the terminal pane has no title strip, tab bar or toolbar: the titlebar toggle shows its state (icon in `text.primary` with `fill.selected` when open) and everything else is in the palette. The divider is a resize handle. The terminal session is bound to the active project's directory; when the session ends, the pane shows one line, "Session ended. Enter to restart.", in `text.tertiary`. The current-state update written by Claude Code appears in the topic view with the highlight while focus stays in the terminal.

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

Annotations: the three steps are plain text rows in `text.secondary` (not a checklist to tick, not a wizard); they become available as soon as the project exists and disappear for good once the project has one topic. Choosing a folder creates the project named after the folder and opens its inbox with the capture editor focused. Other empty states follow the same pattern at the top of their column: empty project ("No topics yet. Capture notes and let Claude Code file them, or start one yourself." [New topic]), empty epic, empty timeline, no search results.

### 8.6 Topic view, knowledge section expanded

```
        Knowledge  3                                                           [+]     28  type.label + count
     4  [d] Resize protocol    How the grid reflows; read before touching it   [>]  2 h ago   32
        [m] ConPTY minimum     Needs Windows 10 1809 or later                  [>]  Mon       32  expanded:
               ConPTY is missing before Windows 10 1809. Acta refuses to start the      <- type.reading,
               terminal there and says why; do not add a winpty fallback.                  inset 32
               Written by Claude Code Mon 16:20 · edited by Alexandre 10 min ago
               [Edit] [History] [Move] [Delete]                                         <- ghost
        [m] No tabs            One session per project, by decision            (A)  Tue       32
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
|  v Terminal          |        Acta / Terminal / Embed the terminal      <- breadcrumb, links   |
|   [(.) Embed the..]  |     8  Resize protocol                           <- type.title 22/28    |
|    (=) Scrollback    |     8  How the grid reflows; read before touching it  <- reading, secondary |
|                      |    12  [>] Updated 2 h ago by Claude Code    [Edit] [History] [...]     |
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

## 9. Review checklist

Run against every screen, in both themes when light ships. One failed line blocks the change.

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

**States**
- [ ] Each interactive element has default, hover, pressed, keyboard focus and disabled, as specified in 5.
- [ ] Empty, loading (over 400 ms only), error and long-content states exist: a 200-character title, a 500-entry timeline, 0 and 1,000 topics.
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
- [ ] A write from another process during reading, scrolling and editing moves neither focus nor viewport, and never overwrites a field in edit.

**Copy**
- [ ] Sentence case, the vocabulary of section 7, no exclamation marks, no emoji, "Claude Code" by name.
- [ ] Errors state what failed and the next step; empty states state the purpose and the one action.
- [ ] Sample data uses "Alexandre".

**Last look**
- [ ] Remove one element. If the screen still works, it stays removed.
- [ ] Would this screen be mistaken for a template or for another tool? If yes, the answer is in type and spacing, not in adding decoration.
