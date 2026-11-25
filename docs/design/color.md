# Acta design: color

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

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
| `text.primary` | `#E4E3DF` | `#302F2B` | Titles, body, values |
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
| `syntax.keyword` | `#BC96BC` | `#844C86` | Keywords in highlighted code blocks (terminal slot 5) |
| `syntax.string` | `#9BB26B` | `#4C6A24` | Strings in highlighted code blocks (terminal slot 2, = `success`) |
| `syntax.constant` | `#7FA6CC` | `#2F5F8F` | Numbers and language constants in highlighted code blocks (terminal slot 4) |

Elevation is tone, lightest on top in dark and in light: `sunken < base < raised <= overlay`. In light, `raised` and `overlay` are both white; an overlay is told apart by its outline and shadow, which it always has. A surface never sits on a surface more than one step lighter than itself except overlays.

Why neutral greys: the surfaces are greys with a trace of warmth (blue one to three points under red), close to what the three references use and far from the earlier brown-blacks. Neutral chrome lets the only colours on screen, status glyphs, the accent and terminal output, read true; the trace of warmth keeps hours of reading from feeling cold. The light theme's page is one step off white (`#FBFAF8`) so that white inputs and panels can sit above it. Body text is comfortable rather than maximal (2026-11-21, for an app read for hours): `text.primary` measures about 12.5:1 on the page in both themes (12.55 dark, 12.85 light), not the 15 to 17 of near-black on white, which tires and, in dark, halates.

Why verdigris: the semantic colours (red, amber, green) are all warm, so a warm accent would collide with danger or warning on a focus ring next to an error. A grey-green copper patina stays calm on neutral greys, separates from all three semantics, and is neither the blue nor the purple of the three references, so nothing in Acta reads as borrowed.

The three syntax colours of code blocks (`syntax.keyword`, `syntax.string`, `syntax.constant`) take the terminal's slots 5, 2 and 4, so code in a page and code in the terminal read alike; their use is in components-editor.md 5 (Code).

### 2.2 Measured contrast (WCAG 2.x, computed from the hex values)

| Pair | Dark | Light | Requirement |
|---|---|---|---|
| `text.primary` on `bg.sunken` / `base` / `raised` / `overlay` | 13.42 / 12.55 / 11.34 / 10.30 | 12.08 / 12.85 / 13.40 / 13.40 | 4.5 |
| `text.secondary` on same four | 8.02 / 7.50 / 6.77 / 6.15 | 6.82 / 7.26 / 7.57 / 7.57 | 4.5 |
| `text.tertiary` on same four | 6.18 / 5.78 / 5.22 / 4.74 | 5.33 / 5.67 / 5.92 / 5.92 | 4.5 |
| `accent` (link, glyph) on same four | 7.26 / 6.79 / 6.13 / 5.57 | 5.33 / 5.67 / 5.92 / 5.92 | 4.5 |
| `danger` on same four | 5.89 / 5.50 / 4.97 / 4.52 | 5.21 / 5.54 / 5.78 / 5.78 | 4.5 |
| `warning` on same four | 7.45 / 6.96 / 6.29 / 5.71 | 5.52 / 5.87 / 6.12 / 6.12 | 3.0 (glyph) |
| `success` on same four | 7.37 / 6.89 / 6.22 / 5.65 | 5.58 / 5.93 / 6.19 / 6.19 | 3.0 (glyph) |
| `syntax.keyword` on `bg.sunken` / `base` / `raised` | 6.75 / 6.31 / 5.70 | 5.63 / 5.99 / 6.25 | 4.5 |
| `syntax.string` on same three | 7.37 / 6.89 / 6.22 | 5.58 / 5.93 / 6.19 | 4.5 |
| `syntax.constant` on same three | 6.75 / 6.31 / 5.71 | 6.00 / 6.38 / 6.66 | 4.5 |
| `text.on-accent` on `accent` / `.hover` / `.pressed` | 7.10 / 8.47 / 5.48 | 5.67 / 7.20 / 9.06 | 4.5 |
| `text.on-accent` on `danger` | 5.76 | 5.54 | 4.5 |
| `text.primary` on `fill.selected` over `sunken` / `base` / `overlay` | 10.73 / 9.93 / 8.08 | 10.28 / 10.91 / 11.37 | 4.5 |
| `text.secondary` on `fill.selected` over `sunken` / `base` / `overlay` | 6.41 / 5.93 / 4.83 | 5.81 / 6.17 / 6.42 | 4.5 |
| `text.secondary` on `fill.hover` over `sunken` / `base` / `overlay` | 6.96 / 6.45 / 5.24 | 6.14 / 6.52 / 6.79 | 4.5 |
| `text.tertiary` on `fill.hover` over `sunken` / `base` (tree counts, folder row label) | 5.36 / 4.97 | 4.79 / 5.09 | 4.5 |
| `warning` glyph on `fill.hover` over `base` (folder not found) | 5.99 | 5.27 | 3.0 (glyph) |
| `accent` glyph on `fill.selected` over `sunken` (Active topic selected in the tree) | 5.81 | 4.54 | 3.0 (glyph) |
| `accent` glyph on `fill.hover` over `sunken` (Active topic hovered in the tree) | 6.30 | 4.80 | 3.0 (glyph) |
| `warning` glyph on `fill.hover` / `fill.selected` over `sunken` (Blocked topic hovered, selected in the tree) | 6.46 / 5.96 | 4.96 / 4.70 | 3.0 (glyph) |
| `text.primary` on `accent.subtle` over `sunken` / `base` / `overlay` | 10.09 / 9.37 / 7.73 | 9.72 / 10.28 / 10.69 | 4.5 |
| `text.primary` on `accent.subtle` over `raised` (file drop target on a note card, components-attachments.md) | 8.46 | 10.69 | 4.5 |
| `text.secondary` on `fill.hover` over `raised` (file chip hovered on a note card) | 5.78 | 6.79 | 4.5 |
| `text.primary` on `selection` over `base` | 6.99 | 8.34 | 4.5 |
| `border.strong` vs `sunken` / `base` / `raised` / `overlay` | 4.60 / 4.30 / 3.88 / 3.53 | 3.20 / 3.40 / 3.55 / 3.55 | 3.0 (control boundary) |
| `focus.ring` vs `sunken` / `base` / `overlay` | 7.26 / 6.79 / 5.57 | 5.33 / 5.67 / 5.92 | 3.0 |
| `#FFFFFF` on `caption.close.hover` | 5.66 | 5.66 | 4.5 |

Rules that follow from the numbers:
- `text.tertiary` is not allowed on `fill.selected` (3.73 over `bg.overlay` in dark; 4.52 over `bg.sunken` in light, too close to the bar to rely on; the blended fill rounded to 8 bits as the renderer does). On any selected row, in the tree, a table, the palette or a menu, metadata switches to `text.secondary`.
- Text on `accent.subtle` is always `text.primary`. Coloured or tertiary text on a tinted fill is not allowed (it measures 4.1 to 5.1).
- `border.subtle` and `border.default` are separators (1.1 to 1.7 against their surface), never the only boundary of an interactive control. `border.subtle` nearly vanishes on `bg.overlay` (1.08 in dark), so hairlines inside overlays use `border.default`.
- `text.tertiary` is not allowed on `fill.hover` over `bg.raised` (4.46 in dark): a hovered row or chip inside a card switches its metadata to `text.secondary`.
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

No "In review", no priorities, no custom statuses in v1: each addition makes the agent's writes less predictable. Status never relies on colour: every status has a distinct glyph interior (dashes, empty, dot, bars, check, slash), the Status property of a topic page always shows glyph plus word, and the glyph has a tooltip and an accessible name with the word. Closed topics render their title in `text.tertiary` (no strikethrough) and collapse under a "3 closed" row at the end of their group. In lists (the tree, the topic table) a closed topic's glyph is `text.tertiary` too, whatever its status: a finished topic asks nothing of a reader scanning a list, so the colour of Done is kept for the places that state a status (the Status property, the status menu, tabs), and colour in a list is left to Active and Blocked (4.2.1). The shape still tells Done from Dropped.

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
