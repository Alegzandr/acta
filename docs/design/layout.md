# Acta design: spacing, sizing and the app shell

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 4. Spacing, sizing and layout

### 4.1 Scales

- **Spacing** (`space.*`): 2, 4, 8, 12, 16, 24, 32, 48. Nothing else. 4 inside a control, 8 between related items, 12/16 inside containers, 24 between sections, 32/48 around the content column.
- **Control heights**: 28 default (buttons, inputs, segmented control, tree rows); 24 compact (icon buttons inside rows, titlebar controls on macOS); 32 for palette and menu rows; 44 for the palette input. Minimum pointer target is 24 x 24.
- **Density**: one density. Tree row 28, menu and palette row 32, timeline entries separated by 32 (space, never a divider), note cards by 8.
- **Radii** (softened 2026-11-21 at the user's request): `radius.xs` 4 (kbd, inline code, checkbox, Claude Code mark, progress bars); `radius.sm` 6 (buttons, inputs, segmented controls, chips, tree and list rows, menu and palette rows); `radius.md` 8 (tooltips, code blocks); `radius.lg` 10 (panels, note cards, editors, notices, the decision entry box, menus, toasts); `radius.xl` 12 (palette, dialogs, quick capture, updater window). A row inside a padded container takes the container's radius minus the padding (a menu's 10 minus its 4), so the corners stay concentric. Hover and selection fills are inset pills, never full-bleed: tree rows sit 8 inside the sidebar, menu rows 4 inside the menu, and list rows in the content column hang 8 into its gutter so their text stays on the column's edge. Docked panes (titlebar, sidebar, tool windows, status bar) stay square: they share edges with the window and with each other. Circles only for the human author mark, unread dot and scrollbar thumb.
- **Borders vs shadows**: panes are separated by a 1px `border.subtle` line and a surface step, never a shadow. Containers on `bg.base` use `bg.raised` plus a 1px `border.subtle` outline. Only floating layers (palette, menu, dialog, tooltip, toast) get `shadow.overlay`, always together with a 1px `border.default` outline, because a shadow alone disappears on a dark surface.
- **Icons**: **Lucide** (ISC license; a subset derived from Feather is MIT), embedded as SVG, only the icons listed in the design docs. Rendered at 16 (default) or 14 (status glyphs in the tree, entry type icons), exported with `stroke-width="1.5"` on the 24 grid so strokes land at 1px at 16. Colour is `text.secondary` at rest and `text.primary` on hover or selection, unless a token above says otherwise. Icons are never filled, never inside coloured circles, and an icon-only button always has a tooltip with its shortcut.

### 4.2 App shell

```
+------------------------------------------------------------------------------------------+
| Titlebar 36: back, forward, breadcrumb ... search, view toggles, area toggles, layout    |
+---------------+------------------------------------+-------------------------------------+
| Left sidebar  | Tab bar 32 (only with 2+ tabs)     | Right sidebar (hidden by default)   |
| header 28     +------------------------------------+ header 28: Terminal | Chat          |
| Projects      | Page area, bg.base                 | the active view                     |
| bg.sunken     | flexible, min 420                  | 648 = 80 columns                    |
| 260           | content column max 560, centred    | (view min to 60% window)            |
|               +------------------------------------+                                     |
|               | Bottom panel (hidden), header 28   |                                     |
|               | 320 (view min to 70% of the body)  |                                     |
+---------------+------------------------------------+-------------------------------------+
| Status bar 24: project, Claude Code                                   update, version    |
+------------------------------------------------------------------------------------------+
```

Five regions, each with one job: the tree finds, the page reads and writes, tool windows run things beside it, the titlebar says where you are, the status bar says what Acta is connected to. This is the **default** arrangement. The four areas (left sidebar, page area, right sidebar, bottom panel) and the views in them can be shown, hidden, resized and moved, and nothing moved is ever hard to put back: the model, the drag and drop, persistence and recovery are in 4.8 ([workbench.md](workbench.md)), which wins where this section and it disagree.

- **Sidebar: one tree, every project.** Top-level rows are projects, each collapsible. Under an expanded project, in this order: the Inbox row (`inbox` icon, count of open notes), the Knowledge row (`library` icon, count of entries in the project, unread dot when Claude Code has added some), the topics that belong to no epic, then the epics, each collapsible, with their topics. The project of the active page is expanded when that page opens; every other node stays as the user left it. Pinned at the bottom, outside the scroll, above a 1px `border.subtle` line: the Trash row (`trash-2`) and the Settings row (`settings`). The tree is the Projects view (4.8.3); in the left sidebar it sits under the area header "Projects" (4.8.4), which is how it is moved. Width 260, resizable from 200 to 60% of the window, hidden with `Mod+B`. Padding 8, rows inset 8, tree indent 16 per level (project 0, epic and loose topic 16, topic in an epic 32).
- **Page area: one page per tab.** A page is a topic, an epic, a project, an inbox, a knowledge base, a document or memory, the trash, settings or the shortcuts list. Every record page has the same skeleton: title, properties block, then its sections. Section order in a topic is fixed: title, properties (status, epic, folders, id, updated), current state, description, knowledge, timeline; the timeline is last because it is the only unbounded section. An epic page: title, properties (progress, folders, id, updated), description, knowledge, topic table. A project page: title, properties (progress, folders, updated), description, knowledge, topic table grouped by epic. The content column is centred, at most the Reading width (typography.md 3.3: 480, 560 or 720), with at least 32 of side padding and 32 above the title. Sections of a page are 32 apart, each opened by a 28 header row with its `type.label` vertically centred; space separates them, never a line.
- **Tabs** (4.4), **tool windows** (4.5) and the **status bar** (4.6) are specified below.
- **Window**: default 1440 x 900, minimum **880 x 560**. Widths, dock positions and visibility of panes, open tabs and the tree's expansion are remembered per window in the preferences file (4.7).
- **Narrow behaviour**: areas give way in a fixed order (4.8.10): the left sidebar first, then the page area's split, never a view's minimum. If left sidebar + 420 + right sidebar no longer fit, the left sidebar auto-hides (it returns when space returns; `Mod+B` still shows it as an overlay on top of the page, at its width, with `shadow.overlay`, dismissed by `Esc` or by opening something). At 880 with the right sidebar open the split is page 420 / right sidebar 460. Nothing ever scrolls horizontally except code blocks, wide tables and an overfull tab bar.

### 4.3 Titlebar (client-side, 36 high, `bg.sunken`, bottom border `border.subtle`)

Content, left to right after the platform inset: on Windows and Linux the app icon, 16, vertically centred, 8 before the next control (the hand-rendered `hicolor` bitmap of the scale: 16 at 1x, 24 at 1.5x, 32 at 2x, never a downscaled large one; part of the drag region, no hover state, at 60% opacity when the window is inactive; macOS shows none, its traffic lights sit there); back and forward (`arrow-left`, `arrow-right`; compact ghost buttons, disabled at the ends of the active tab's history); the breadcrumb of the active page; drag region; "Search" button showing the palette shortcut (icon only, `search`, under 1040 of window width); 8; the **view toggles**, one per view (`list-tree`, `inbox`, `library`, `square-terminal`, `message-circle` on Windows only, `globe` once ADR 0010 ships), gap 4; 8; the **area toggles** (`panel-left`, `panel-bottom`, `panel-right`), gap 4; the **Layout menu** (`panels-top-left`). All are 28 x 28 ghost icon buttons; they are always shown, whatever is hidden, because they are the way back (4.8.5, 4.8.11). A toggle whose view or area is visible shows `fill.selected` and its icon in `text.primary`. (Until 4.8 ships: sidebar toggle `panel-left` after the app icon, then chat and terminal toggles at the right.) The whole bar is a drag region except the controls; double-click follows the platform (zoom on macOS, maximize elsewhere).

**Breadcrumb**: `type.small`, parts separated by " / " in `text.tertiary`: "Acta / Terminal / Embed the terminal". Every part but the last is a link (`text.tertiary`, `text.primary` and underlined on hover and focus) to that project or epic page; the last part is the page itself, in `text.secondary`, not a link. A page's location (its project and epic) appears here and only here, so pages carry no breadcrumb of their own; the page's title is on the page. When it does not fit, the last part truncates at its end; under 240 of available width the middle parts collapse into one "…" part that opens a menu of them.

| Platform | Window controls | Rules |
|---|---|---|
| macOS | Native traffic lights, left, inset 12 from the left edge, vertically centred | Content starts at x = 76. In full screen the lights disappear and content starts at x = 12. Never redraw the lights. |
| Windows | Minimize, maximize/restore, close on the right, each 46 x 36, drawn by the app with 10px 1px-stroke glyphs | Hover `fill.hover`, pressed `fill.pressed`; close hover `caption.close.hover` with a white glyph. Hit-test regions must report as native caption buttons so Snap Layouts appears on maximize hover. 1px `border.subtle` window outline when not maximized. |
| Linux | Client-side controls on the right in the same style as Windows, 36 x 36 each | Honour the desktop's button layout and side when the platform reports it. When the compositor provides server-side decorations (tiling window managers), keep the system titlebar and render this bar as a plain 36 toolbar without controls. |

When the window is inactive, titlebar text and icons drop to `text.tertiary` and the app icon to 60% opacity. Nothing else changes. First run, key loss and open failed show only the app icon (Windows and Linux), "Acta" and the window controls.

### 4.4 Tabs

Tabs earn their place only when a second page is open, so that is when they appear. With one page there is no tab bar and nothing to manage; navigating (tree, palette, links, back and forward) replaces the page in the active tab, and each tab keeps its own history.

- **Opening a tab**: `Mod+Enter` on any row or palette result, `Mod`+click, middle click, or "Open in new tab" in a row menu. A page that is already open in a tab is focused there instead of being opened twice.
- **Tab bar**: 32 high, `bg.sunken`, bottom border `border.subtle`, across the page area only (not over the docks). Shown when two or more tabs exist.
- **Tab** (height 32, padding 0 x 12, gap 8, min width 120, max 220): page icon 14 in `text.secondary` (the status glyph for a topic, `layers` for an epic, `folder` for a project, `file-text` or `pin` for knowledge, `inbox`, `library`, `trash-2`, `settings`), title in `type.ui` `text.secondary`, truncated; trailing slot 16: a ghost `x` on hover and on the active tab, or the 6 `accent` unread dot when another process changed the page while the tab was in the background. Active: fill `bg.base`, so the tab joins its page, no bottom border under it, title `text.primary`. No accent line: tone and text weight carry the state. Hover: `fill.hover`. Keyboard focus ring inset. Dragging reorders, as tree rows do.
- **Overflow**: tabs shrink to 120, then the bar scrolls horizontally and a ghost `chevron-down` at its right end opens a menu of all tabs.
- **Keys**: `Mod+W` closes the active tab (the last remaining tab cannot be closed; the key then does nothing), `Ctrl+Tab` / `Ctrl+Shift+Tab` go to the next and previous tab on every platform. Palette: "Close tab", "Close other tabs", "Reopen closed tab".
- Tabs belong to the window, not to a project: two tabs may show pages of different projects. They are restored at launch.
- **Groups** (4.8.8): the page area can be split into up to three groups, each with its own tab bar, by dragging a tab to an edge or with "Split right" (`Mod+\`). Everything above holds per group. Views moved into the page area are tabs too (view tabs, 4.8.8).

### 4.5 Tool windows

4.8 generalises this section: the terminal and the chat are views, which share an area as tabs instead of hiding each other, can go to any area, and are moved by dragging as well as by command. What follows stays true of their content, keys and headers; "dock" below reads "area", and "one tool window per dock" and "the header is a drag handle for nothing" are superseded by 4.8.2 and 4.8.4.

A tool window is something that runs beside the page and keeps running when hidden. There are three: the tree (left, described above, no header), the **terminal** and the **chat** (ADR 0009). The terminal and the chat live in a dock: **right** (default for both; width 648, range 400 to 60% of the window) or **bottom** (under the page area; height 320, range 160 to 70% of the window height).

- **One tool window per dock at a time.** Showing the chat in the dock where the terminal is visible hides the terminal, and the reverse; both go on running. To see both, move one to the bottom ("Move terminal to bottom", "Move chat to bottom", and the reverse commands). Size and dock are remembered per tool window.
- **Toggle**: the shortcut shows and focuses the tool window; pressed while it has focus, it hides it and returns focus to the page. `Mod+Shift+Enter` zooms the focused tool window over the page area and back.
- **Header** (28 high, `bg.sunken`, bottom border `border.subtle`, padding 0 x 8, gap 8): the name in `type.label` `text.secondary` ("Terminal", "Chat"), a context text in `type.small` `text.tertiary`, then right-aligned compact ghost icon buttons (24), the last of which always hides the tool window (`minus`). The header is a drag handle for nothing; docks change by command.
- **Terminal header**: the context names what the session is on, which is the page it was started from: "on topic Embed the terminal" (the name is a link). Buttons: Restart here (`rotate-cw`), hide. One session per project: the dock shows the session of the active project (4.6), and moving to a page of another project shows that project's session and leaves the first running. Navigation never restarts or moves a session. When the session was started from another topic, epic or project page than the active one, or when the page's primary effective folder is no longer the one the session started in, the context text turns `text.primary` and a ghost text button "Restart here" appears next to it; nothing else marks the mismatch. "Restart terminal here" (that button, the icon button, and the palette) ends the session and starts it with the active page's folders and focus; with a process running it is a confirmed case of 6.4 ("Restart the terminal? Claude Code is still running in it.", Cancel, Restart).
- **Chat header and states**: see Chat panel in 5.
- Dividers between the page and a dock are resize handles (5).

### 4.6 Status bar

24 high, `bg.sunken`, top border `border.subtle`, full window width, `type.small`, `text.tertiary`. Items are ghost controls as tall as the bar with padding 0 x 8 (hover `fill.hover` and `text.secondary`, keyboard focus ring inset). It answers two questions that had no home, is Claude Code wired to this work and which Acta is this, and it takes the update indicator out of the titlebar. It never shows messages, counts, a clock or progress; those would make it a second place to look.

- **Left**: the active project (`folder` 14 and its name; activating it is "Switch project", `Mod+O`), then the **Claude Code status**.
- **The active project** is the project of the active page. Pages that belong to no project (Settings, Trash, Keyboard shortcuts) keep the project of the last page in this window that had one. It decides what the status bar names, which terminal session the dock shows, where quick capture files a note and what `Mod+L` and `Mod+T` act on. On first run, with no project at all, there is none: the project item is absent, the Claude Code status reports setup only, and whatever needs a project shows its empty state (the first-run page, 8.5).
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

Theme, language, the layout of 4.8.12 (areas, sizes, where each view is, page groups), the open tabs (each a typed reference: a record id, or a fixed page key such as `inbox` or `knowledge` with its project id, `trash`, `settings`, `shortcuts`; never a title), the tree's expansion state, the terminal font size, the version that last ran (for "Updated to 0.4.1") and the one-time flags (sign-in help dismissed, export suggestion shown) live in `preferences.json` in Acta's data directory, not in the encrypted database: they must be readable before the database opens, so that first run and the key-loss screen come up in the user's theme and language. The file holds no user content and no key material: drafts of unsaved editors are user content and live in the encrypted database, never here; if it is missing or unreadable, Acta starts with the defaults (System theme, English) and says nothing.
