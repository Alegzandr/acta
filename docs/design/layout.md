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

- **Sidebar: one tree, every project.** Top-level rows are projects, each collapsible. Under an expanded project: its Inbox and Knowledge rows, then the topics that belong to no epic, then the epics with their topics; closed topics fold under one "3 closed" row per group. Opening a page reveals its row. Pinned at the bottom, outside the scroll, above a 1px `border.subtle` line: the Trash row (`trash-2`) and the Settings row (`settings`). The tree is the Projects view (4.8.3); in the left sidebar it sits under the area header "Projects" (4.8.4), which is how it is moved. Width 260, resizable from 200 to 60% of the window, hidden with `Mod+B`. Structure, geometry, signals, long trees and empty states are in 4.2.1; the row is in 5 (Tree row).
- **Page area: one page per tab.** A page is a topic, an epic, a project, an inbox, a knowledge base, a document or memory, a questions page, a person page, the trash, settings or the shortcuts list. Every record page has the same skeleton: title, properties block, then its sections. Section order in a topic is fixed: title, properties (status, due and report to when set, epic, folders, then the meta line with id and updated), current state, description, questions, knowledge, timeline; the timeline is last because it is the only unbounded section. An epic page: title, properties (progress, due and report to when set, folders, the meta line), the collapsed "Needs attention" section when something applies, description, questions, knowledge, topic table. A project page: title, properties (progress, due when set, folders, the meta line), Overview, description, questions, knowledge, topic table grouped by epic. The Overview (components-records.md 5) says what needs the human and nothing else; it is never in the tree, the status bar or a view of its own. Questions sit between description and knowledge on all three (components-questions.md 5). The content column is centred, at most the Reading width (typography.md 3.3: 480, 560 or 720), with at least 32 of side padding and 32 above the title. Sections of a page are 32 apart, each opened by a 28 header row with its `type.ui-strong` `text.secondary` label vertically centred (typography.md 3.2); space separates them, never a line.
- **Tabs** (4.4), **tool windows** (4.5) and the **status bar** (4.6) are specified below.
- **Window**: default 1440 x 900, minimum **880 x 560**. The layout of 4.8 (areas, sizes, where each view is, page groups), open tabs and the tree's expansion are remembered per window in the preferences file (4.7).
- **Narrow behaviour**: areas give way in a fixed order (4.8.10): the left sidebar first, then the page area's split, never a view's minimum. If left sidebar + 420 + right sidebar no longer fit, the left sidebar auto-hides (it returns when space returns; `Mod+B` still shows it as an overlay on top of the page, at its width, with `shadow.overlay`, dismissed by `Esc` or by opening something). At 880 with the right sidebar open the split is page 420 / right sidebar 460. Nothing ever scrolls horizontally except code blocks, wide tables and an overfull tab bar.

### 4.2.1 The tree (Projects view)

Decided on 2026-11-21 at the user's request: the tree must be clear at a glance, without noise. A glance answers four questions, and each has one answer, never two:

| Question | What answers it | What does not, by decision |
|---|---|---|
| Where am I? | The open page's row: `fill.selected`, title `text.primary`. The project and epic rows on its path: title `text.primary`, while every other project and epic row is `text.secondary`. Opening a page reveals its row (below). | An accent bar, a bold title, an icon. |
| What is being worked on? | The Active glyph, `circle-dot` in `accent`. | Sorting Active first; a "Now" section. |
| What needs me? | The Blocked glyph, `circle-pause` in `warning` (leading); the unread dot (trailing); the due mark, overdue or due today (trailing, neutral, below); the Inbox count. | Badges on epics or projects; Blocked or due marks rolled up into folded parents; a "due soon" mark; colour on a due mark; a mark for open questions or for Report to (components-questions.md 5: they wait on someone else, and live on the page, the topic table and the briefing). |
| How is it organised? | Three row kinds, three treatments: project (depth 0, `type.ui-strong`, no icon), epic (depth 1, `type.ui-strong`, chevron), topic (`type.ui`, status glyph); a project's places (Inbox, Knowledge) above its work, set off by space; space between projects. | Guide lines, section headers, coloured icons. |

How the references do it, and what Acta takes: **VS Code's explorer** reveals the active file (expands its folders, scrolls to it), sticks the parent folder at the top while scrolling (sticky scroll), and needs indent guides because folders nest without limit and every level looks the same. **Obsidian** draws guides for the same reason and marks the active file with a fill and brighter text, nothing else. **Notion** marks the current page with a fill and darker text, reveals controls (`+`, `...`) on hover only, and writes "No pages inside" under an empty page instead of leaving a lone open chevron. **Linear** keeps colour for status and a dot for unread, shows counts only where something waits (Inbox), and lets the user order things by hand. Acta takes the reveal, the sticky parent, the fill-and-text selection, the hover-only control, the empty-container line, the single unread dot and the manual order. It leaves the guides: the tree has three levels at most and every level change is also a change of row kind (chevron and weight, or glyph), so the eye never has to count indentation.

**Order inside an expanded project**, top to bottom:

1. **Inbox** (`inbox`), with the count of open notes when it is above 0 ("99+" above 99). The count is the one number in the tree: notes waiting to be filed are work waiting.
2. **Knowledge** (`library`), no count; the unread dot when Claude Code created or changed an entry since it was last shown. A total of 55 entries asks nothing of the reader, so it is not shown (the knowledge page states it).
3. 4 of space.
4. **Topics in no epic**, open ones in stored order, then their "N closed" row.
5. **Epics** in stored order, each followed, when expanded, by its open topics in stored order, then its "N closed" row, or by the "New topic" ghost row when it has no topic at all.

Then 8 of space after the project's last visible row when another project follows. Folded projects stack with no space between them, so a list of folded projects reads as one list.

Topics in no epic come first because (a) the project page's topic table already puts "No epic" first (components-records.md 5), so both views of a project agree; (b) after them every step down the tree changes row kind (topic, then epic with chevron and weight), and every step back up is a project row with space above it, so there is never a depth-2 row followed by a depth-1 row of the same kind, which is the ambiguity guide lines exist to resolve; (c) they are usually the newest, still unfiled work and sit next to the inbox most of them come from.

**Status in the tree.** Colour is spent only on the two open states that say something to the reader: Active (`accent`, "here") and Blocked (`warning`, "needs you"). Draft is `text.tertiary`, Planned `text.secondary`, and closed topics (Done, Dropped) draw glyph and title in `text.tertiary`: in a list, a finished topic asks nothing, so it does not get the green of its Status property (2.3). Status still reads in greyscale: the glyph shapes differ (dashes, empty, dot, bars, check, slash). Topics keep their **stored order**, which the human sets (drag, `Alt+Up` / `Alt+Down`) and Claude Code sees; the tree never sorts by status, because a status written by Claude Code would then move rows under the reader (6.5, rule 5) and a hand-made order would mean nothing. Closed topics fold under the "N closed" row at the end of their group, folded by default, the fold remembered per group (4.7).

**Signals: one trailing slot, one signal at most.** Every row has a leading slot (what the row is, or its state) and a trailing slot (what is new for you, or how much waits). Nothing else is drawn in a row.

| Row | Leading (16 slot) | Trailing at rest | Trailing on hover |
|---|---|---|---|
| Project | `chevron-right` / `chevron-down` | Session mark; else the unread dot when folded and anything inside it is unread; else its own due mark | Ghost `plus`, "New topic" (in no epic) |
| Epic | Chevron | Session mark; else the unread dot when folded and one of its topics is unread; else its own due mark | Ghost `plus`, "New topic" (in this epic) |
| Inbox | `inbox` | Count of open notes, when above 0 | Unchanged |
| Knowledge | `library` | Unread dot | Unchanged |
| Topic | Status glyph 14 | Session mark; else the unread dot; else its due mark | Unchanged |
| "N closed" | Chevron 14 | Unread dot when folded and a closed topic in it is unread | Unchanged |
| "New topic" (ghost) | `plus` 14 | Nothing | Nothing |

Only unread rolls up into a folded parent, because it is transient and folding must never hide it; Active and Blocked are lasting states, and rolling them up would mark a folded epic for weeks until the reader stops seeing the mark. Epics and projects carry no counts and no progress in the tree: progress is on their pages (components-records.md 5). The unread dot means one thing everywhere: Claude Code (or any other process) wrote it since this installation last showed it (6.5, data.md 11 request 1); its tooltip says so.

**Due marks** (2026-11-21, with due dates; components-records.md 5). A row whose record is open and has a due date before today shows **overdue**, `clock-alert` 14 in `text.secondary`; one due today shows **due today**, `clock` 14 in `text.tertiary`. Nothing else about dates is drawn in the tree: no "due soon", no date text, no colour. The two marks differ by shape (the alert notch), then by tone, so they read in greyscale; colour in the tree stays with Active, Blocked and unread, as this section decided. They follow the row states of 5 (Tree row): `text.primary` on hover, and on a selected row the due-today mark becomes `text.secondary` (2.2). Measured on `bg.sunken`, dark / light (glyphs need 3.0): overdue 8.02 / 6.82, due today 6.18 / 5.33, either on a selected row 6.41 / 5.81. Tooltip, on the 16 hit box: the sentence form of 10.2.1 ("Overdue by 3 days, due 18 Nov 2026", "Due today").

**One signal, in this order.** The trailing slot shows the first that applies: on hover of a project or epic row, the `plus`; then the Claude Code session mark; then the unread dot; then overdue; then due today. Unread wins because it is transient and folding or a date must never hide it; opening the row clears it, and the due mark then shows. Blocked never competes: it is the leading glyph, so a blocked, overdue topic shows both, in two different slots, which is the one case where a row says two things, both lasting and both "needs you". The session mark (decided 2026-11-21, with session presence, data.md 11 request 2) is `bot` 14 in the colour of the session's state, on the row of the record a live Claude Code session focuses, never on its ancestors (components-app.md 5, Claude Code sessions); it ranks first after the `plus` because a record being worked on keeps changing, so its unread dot would say less. It is not rolled up into folded rows either. Due marks never roll up into a folded parent: like Active and Blocked they last for days, and an epic marked for a week stops being seen. An epic or a project shows a due mark only for its own date (components-records.md 5).

**Where am I: reveal.** Opening a page by any route (tree, palette, link, back and forward, switching tabs or groups) reveals its row: its project and its epic expand (and stay expanded, as if the user had unfolded them), revealing a closed topic also unfolds its "N closed" row, and the tree scrolls by the least amount that shows the row whole below the sticky row (below). One exception keeps the reader's place: while the pointer is over the tree, the tree expands but does not scroll, and the rows on screen keep their position (anchored as in 6.5, rule 2). A document or memory selects the row of its owner (components-knowledge.md 5). `Mod+1` focuses the tree and puts the keyboard cursor on the open page's row, revealing it the same way. If the user folds the open page's epic or project afterwards, the folded row keeps its `text.primary` title, so the path stays visible.

**Geometry** (x from the view's left edge; the view is `bg.sunken`):

| Part | Value |
|---|---|
| Tree viewport padding | 8 on every side |
| Row | height 28, a pill from x 8 to the right edge minus 8, `radius.sm`, padding 0 x 8 |
| Indent | 16 per depth: project 0, Inbox, Knowledge, epic and topic in no epic 1, topic in an epic 2. "N closed" and "New topic" take the depth of the topics of their group |
| Leading slot | 16 x 16 at x = 16 + 16 x depth (16, 32, 48); a 14 glyph is centred in it |
| Title | starts 8 after the leading slot: x = 40, 56, 72; fills, truncates at the end |
| Trailing slot | right-aligned at the row's content edge (view width minus 16), 8 after the title. Project and epic rows always reserve 24 (so the `plus` appears without moving the title); Inbox reserves the count's width; topic, Knowledge and "N closed" rows reserve 16 only while the dot or a due mark shows |
| Due mark | `clock-alert` or `clock` 14, centred in a 16 x 16 box at the right of the trailing slot, where the dot would be |
| Unread dot | 6, `accent`, centred in a 16 x 16 box at the right of the trailing slot |
| `plus` | compact ghost icon button 24, its centre on the dot's centre line (it overhangs the row padding by 4) |
| Space | 4 below the Knowledge row; 8 below the last visible row of an expanded project when another project follows |

Title space at the two widths that matter (projects and epics with their reserved 24; topics without / with a dot): at **260**, project 172, epic 156, topic in no epic 188 / 164, topic in an epic 172 / 148; at **200**, project 112, epic 96, topic in no epic 128 / 104, topic in an epic 112 / 88. At 200 a topic title in an epic shows about 14 characters; the tooltip gives the rest. Nothing hides or changes layout at narrow widths: the counts, the dots and the hover `plus` keep their place, because each is what the row is for.

**Long trees** (100 topics in a project, 10,000 in all; the budgets of 6.3 hold):
- Rows are virtualised: only the rows in the viewport, plus one screen above and below, are laid out (GPUI `list` with per-item heights, because of the 4 and 8 spaces). Building the row list is a pure function of the model and the folds (`tree::rows`), done once per model change, not per frame.
- **Sticky project row.** While the tree is scrolled so that an expanded project's row is above the viewport and any of its rows are still in it, a copy of that project row is drawn at the top of the viewport (5, Tree row). The next project's row pushes it up as it arrives. One row at most, so it never costs more than 28; epics do not stick. It answers "which project is this?" anywhere in a long tree, which the breadcrumb cannot (it names the open page, not the scrolled place).
- Titles truncate at the end with an ellipsis; a truncated title shows its full text in a tooltip (5, Tooltip, the title exception). Ids are never shown in the tree.
- **Collapse all**: the Projects view menu's first item, above the view menu items and a separator (as the chat's own items are, 4.8.4), and the palette command "Projects: Collapse all". It folds every project and every epic; the "N closed" folds are kept. The open page's project unfolds again the next time a page of it is opened.
- **No filter field.** Type-ahead (6.1) jumps to a visible title; the palette (`Mod+K`, scope chip Topics) finds any topic, folded or not, and opening it reveals it. A filter box at the top of the tree would be a second search for navigation, which 5 (Command palette) rules out, and would cost a permanent 36 of height for something the palette already does with one key. VS Code's explorer hides its find-in-tree behind a key for the same reason.

**Empty states.**
- **No project** (only reachable after deleting the last one; first run has no sidebar, 8.5): at the top of the tree, padding 8 then 8 inside, "No projects yet." in `type.ui` `text.secondary`, then 8 below it a secondary button "New project..." (`cmd.new_project`). Trash and Settings stay pinned.
- **A project with no topic and no epic**: its Inbox and Knowledge rows only. It does not look broken, so nothing is added; the project row's hover `plus` and the project page's empty state (8.5) offer the action.
- **An expanded epic with no topic**: one ghost row, `plus` 14 and "New topic", at the topics' depth. An open chevron with nothing under it reads as a fold that failed (Notion writes "No pages inside" for the same reason); the ghost row says it is empty and offers the one action.
- **First run, once the project exists**: the tree shows one expanded project, "Inbox" (no count), "Knowledge", nothing else, the project row in `text.primary` (it is the active project), and the Inbox row selected (choosing the folder opens the inbox, 8.5).
- No loading state: the tree is drawn from the model already in memory, on the window's first frame after the database opens.

**Wireframes.** Characters are 8 wide; `[ ... ]` marks the selected row (its fill spans the whole pill, x 8 to the width minus 8), `*` the unread dot, `[+]` the hover `plus`.

Default, 260 wide:

```
+--------------------------------+
| Projects                [.][-] |  28  area header (4.8.4)
|--------------------------------|
| v Acta                         |  project, on path: 600, text.primary
|     [i] Inbox                4 |  depth 1, count text.tertiary
|     [k] Knowledge            * |  no count; unread dot
|                                |  4
|     (o) Naming                 |  no epic: Draft glyph text.tertiary
|     ( ) Release checklist    ! |  Planned glyph text.secondary; ! overdue mark
|     > 1 closed                 |  ghost row, type.small text.tertiary
|   v Terminal                   |  epic on path: 600, text.primary
|     [(.) Embed the terminal  ] |  selected: fill.selected, text.primary
|       (=) Scrollback           |  Blocked: glyph in warning
|       ( ) Links in output    * |  unread: Claude Code wrote it
|       > 2 closed               |
|   > Core UI                  * |  folded epic, 600 text.secondary; dot
|   v Notes                      |
|       + New topic              |  empty epic: ghost row
|                                |  8
| > Atlas                    [+] |  other project, hovered: plus
| > Field notes                * |  folded project with unread inside
|                                |
|--------------------------------|  border.subtle
|  [t] Trash                     |  pinned
|  [s] Settings                  |
+--------------------------------+
  x: rows 8..252; leading 16 / 32 / 48; titles 40 / 56 / 72; trailing ends at 244
```

Long tree, scrolled into the middle of a project with 140 topics:

```
+--------------------------------+
| Projects                [.][-] |
|--------------------------------|
| v Acta                         |  sticky copy of the project row, bg.sunken
|--------------------------------|  border.subtle, only while stuck
|       ( ) Palette ranking      |  rows of epic "Core UI" pass under it
|       (o) Recent files in th.. |  truncated: full title in the tooltip
|       ( ) Shortcut cheat sh..  |
|       > 14 closed              |
|   v Storage                    |
|       (.) Encrypted backups    |
|       (=) Snapshot restore   * |
|       ( ) Export to Markdo..   |
|       > 31 closed              |
|   > Sync                       |
|   > Website                    |
|                                |  8
| v Atlas                        |  next project: pushes the sticky row up
|     [i] Inbox                  |
|     [k] Knowledge              |       scrollbar: overlay, 4 wide (5)
|--------------------------------|
|  [t] Trash                     |
|  [s] Settings                  |
+--------------------------------+
```

Narrow, 200 wide (the minimum):

```
+-------------------------+
| Projects         [.][-] |
|-------------------------|
| v Acta                  |
|     [i] Inbox         4 |
|     [k] Knowledge     * |
|                         |
|     (o) Naming          |
|   v Terminal            |
|     [(.) Embed the t.. ]|  depth 2: 112 of title (88 with a dot)
|       (=) Scrollback    |
|       ( ) Links in o.. *|
|       > 2 closed        |
|   > Core UI           * |
|                         |
| > Atlas                 |
|-------------------------|
|  [t] Trash              |
|  [s] Settings           |
+-------------------------+
```

No project (after the last one was deleted):

```
+--------------------------------+
| Projects                [.][-] |
|--------------------------------|
|  No projects yet.              |  type.ui text.secondary, at x 16
|  [New project...]              |  secondary button, 8 below
|                                |
|--------------------------------|
|  [t] Trash                     |
|  [s] Settings                  |
+--------------------------------+
```

Glyphs as in 8.1: `(o)` Draft, `( )` Planned, `(.)` Active, `(=)` Blocked, `(v)` Done, `(-)` Dropped; `v` and `>` are chevrons; `[i]` `inbox`, `[k]` `library`; `!` the overdue mark, `clock-alert`.

### 4.3 Titlebar (client-side, 36 high, `bg.sunken`, bottom border `border.subtle`)

Content, left to right after the platform inset: on Windows and Linux the app icon, 16, vertically centred, 8 before the next control (the hand-rendered `hicolor` bitmap of the scale: 16 at 1x, 24 at 1.5x, 32 at 2x, never a downscaled large one; part of the drag region, no hover state, at 60% opacity when the window is inactive; macOS shows none, its traffic lights sit there); back and forward (`arrow-left`, `arrow-right`; compact ghost buttons, disabled at the ends of the active tab's history); the breadcrumb of the active page; drag region; "Search" button showing the palette shortcut (icon only, `search`, under 1040 of window width); 8; the **view toggles**, one per view (`list-tree`, `inbox`, `library`, `square-terminal`, `message-circle` and `globe` on Windows only, `bot`), gap 4; 8; the **area toggles** (`panel-left`, `panel-bottom`, `panel-right`), gap 4; the **Layout menu** (`panels-top-left`). All are 28 x 28 ghost icon buttons; they are always shown, whatever is hidden, because they are the way back (4.8.5, 4.8.11). A toggle whose view or area is visible shows `fill.selected` and its icon in `text.primary`. The whole bar is a drag region except the controls; double-click follows the platform (zoom on macOS, maximize elsewhere).

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
- **Tab bar**: 32 high, `bg.sunken`, bottom border `border.subtle`, across its page group only (not over the side areas). Shown when two or more tabs exist.
- **Tab** (height 32, padding 0 x 12, gap 8, min width 120, max 220): page icon 14 in `text.secondary` (the status glyph for a topic, `layers` for an epic, `folder` for a project, `file-text` or `pin` for knowledge, `inbox`, `library`, `trash-2`, `settings`), title in `type.ui` `text.secondary`, truncated; trailing slot 16: a ghost `x` on hover and on the active tab, or the 6 `accent` unread dot when another process changed the page while the tab was in the background. Active: fill `bg.base`, so the tab joins its page, no bottom border under it, title `text.primary`. No accent line: tone and text weight carry the state. Hover: `fill.hover`. Keyboard focus ring inset. Dragging reorders, as tree rows do.
- **Overflow**: tabs shrink to 120, then the bar scrolls horizontally and a ghost `chevron-down` at its right end opens a menu of all tabs.
- **Keys**: `Mod+W` closes the active tab (the last remaining tab cannot be closed; the key then does nothing), `Ctrl+Tab` / `Ctrl+Shift+Tab` go to the next and previous tab on every platform. Palette: "Close tab", "Close other tabs", "Reopen closed tab".
- Tabs belong to the window, not to a project: two tabs may show pages of different projects. They are restored at launch.
- **Groups** (4.8.8): the page area can be split into up to three groups, each with its own tab bar, by dragging a tab to an edge or with "Split right" (`Mod+\`). Everything above holds per group. Views moved into the page area are tabs too (view tabs, 4.8.8).

### 4.5 Tool windows

A tool window is a view (4.8.3) that runs beside the page and keeps running when hidden: the **terminal** and the **chat** (ADR 0009), with the tree described above. By default both are tabs of the right sidebar (648 wide, 400 to 60% of the window); like every view they can be moved to any area, share an area as tabs, or sit in two groups to be seen together (4.8).

- **Toggle**: the shortcut shows and focuses the view where it was last; pressed while it has focus, it hides it and returns focus to the page (4.8.5). `Mod+Shift+Enter` maximizes the focused area or group over the body and back.
- **Header**: the area header of 4.8.4 (28 high): the view's name or the view tabs, a context text in `type.small` `text.tertiary`, the view's own buttons, the view menu and `minus`, which hides the area. The header is what a view is dragged by.
- **Terminal header**: the context names what the session is on, which is the page it was started from: "on topic Embed the terminal" (the name is a link), then the **folder switcher** (5, Working folders) naming the folder the session runs in. Button: Restart here (`rotate-cw`). One session per project: the terminal shows the session of the active project (4.6), and moving to a page of another project shows that project's session and leaves the first running. Navigation and folder switches never restart or move a session.
  - *Switching from the header.* The header switcher lists the folders of the session's page (not of the active page) and sets that page's active folder, exactly as the switcher on the page does. The session keeps running where it is; the switcher shows the newly chosen folder followed by "(running in embed-terminal)" in `type.small` `text.tertiary`, so the header never claims a folder the session is not in, and the mismatch below appears.
  - *Mismatch.* When the session was started from another topic, epic or project page than the active one, or its page's active folder is no longer the one it runs in, or the commit mode of the folder it runs in changed since it started (ADR 0011: its instruction is out of date), the context text turns `text.primary` and a ghost text button appears next to it: "Restart in acta-docs" when only the session page's active folder changed, "Restart here" otherwise. Nothing else marks the mismatch.
  - "Restart terminal here" (the "Restart here" button, the icon button, and the palette) ends the session and starts it with the active page's active folder, other folders and focus; "Restart in ..." does the same for the session's own page. With a process running both are a confirmed case of 6.4 ("Restart the terminal? Claude Code is still running in it.", Cancel, Restart). A restart is never automatic.
- **Chat header and states**: see Chat panel in 5.
- Dividers between areas, and between groups, are resize handles (5, 4.8.6).

### 4.6 Status bar

24 high, `bg.sunken`, top border `border.subtle`, full window width, `type.small`, `text.tertiary`. Items are ghost controls as tall as the bar with padding 0 x 8 (hover `fill.hover` and `text.secondary`, keyboard focus ring inset). It answers two questions that had no home, is Claude Code wired to this work and which Acta is this, and it takes the update indicator out of the titlebar. It never shows messages, counts, a clock or progress; those would make it a second place to look.

- **Left**: the active project (`folder` 14 and its name; activating it is "Switch project", `Mod+O`), then the **Claude Code status**, then the **plan usage** meters (components-app.md 5, Plan usage).
- **The active project** is the project of the active page. Pages that belong to no project (Settings, Trash, Keyboard shortcuts) keep the project of the last page in this window that had one. It decides what the status bar names, which terminal session the Terminal view shows, where quick capture files a note and what `Mod+L` and `Mod+T` act on. On first run, with no project at all, there is none: the project item is absent, the Claude Code status reports setup only, and whatever needs a project shows its empty state (the first-run page, 8.5).
- **Right**: the update indicator (5), then the version ("Acta 0.4.1", `tnum`; opens Settings at About).
- **Claude Code status**: a glyph 14 and words, never colour alone. It reports what `acta setup --check` (ADR 0005) reports, checked at launch, after setup, and when the window regains focus, plus one fact about the active project.

| State | Glyph | Text | Activating it |
|---|---|---|---|
| Setup incomplete | `circle-dashed` in `text.tertiary` | "Claude Code: not set up" | Opens Settings at Claude Code |
| The `claude` command was not found | `circle-slash` in `text.tertiary` | "Claude Code: not found" | Opens Settings at Claude Code |
| Set up, but the active project has no working folder, so sessions started outside Acta cannot find it | `circle` in `text.secondary` | "Claude Code: no folder for this project" | "Set working folder..." |
| Set up and the project has a folder | `circle-check` in `text.secondary` | "Claude Code: ready" | Opens Settings at Claude Code |

The tooltip of the ready state gives the last write: "Last wrote 2 min ago in Embed the terminal". While Claude Code sessions run, the item names only the most urgent state ("Claude Code: 1 waiting for you"; with none waiting "Claude Code: 2 working"; else "Claude Code: 1 idle"), `bot` in that state's colour; the tooltip lists every state with its count, one per line. Activating it shows the Sessions view (components-app.md 5, Claude Code sessions). Clarity review 2026-11-22: the full list made the status bar a sentence; the bar answers "does something need me", the view answers the rest.

### 4.7 Preferences file

Theme, language, the layout of 4.8.12 (areas, sizes, where each view is, page groups), the open tabs (each a typed reference: a record id, or a fixed page key such as `inbox` or `knowledge` with its project id, `trash`, `settings`, `shortcuts`; never a title), the tree's expansion state, the terminal font size, the version that last ran (for "Updated to 0.4.1") and the one-time flags (sign-in help dismissed, export suggestion shown) live in `preferences.json` in Acta's data directory, not in the encrypted database: they must be readable before the database opens, so that first run and the key-loss screen come up in the user's theme and language. The file holds no user content and no key material: drafts of unsaved editors are user content and live in the encrypted database, never here; if it is missing or unreadable, Acta starts with the defaults (System theme, English) and says nothing. Questions and people (components-questions.md 5) add three things and no content: the fold of each page's "N closed" questions (a record id), the "Also log it as a decision" checkbox (a boolean), and the questions page as a fixed page key `questions` with its scope's record id. A person page is never saved as a tab: its key is a name. Dictation (components-voice.md 5) adds four settings and no content: the engine, the chosen model's id, the language and the graphics-card checkbox. The chosen microphone is a device id that can carry a person's name ("Alexandre's headset"), so it lives in the database's local values instead, with the model's recorded size and modification time (ADR 0014, 4).
