# Acta design: spacing, sizing and the app shell

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 4. Spacing, sizing and layout

### 4.1 Scales

- **Spacing** (`space.*`): 2, 4, 8, 12, 16, 24, 32, 48. Nothing else. 4 inside a control, 8 between related items, 12/16 inside containers, 24 between sections, 32/48 around the content column.
- **Control heights**: 28 default (buttons, inputs, segmented control, tree rows); 24 compact (icon buttons inside rows, titlebar controls on macOS); 32 for palette rows, 28 for menu rows (5, Context menu); 44 for the palette input. Minimum pointer target is 24 x 24.
- **Density**: one density. Tree row 28, menu row 28, palette row 32, timeline entries separated by 32 (space, never a divider), note cards by 8.
- **Radii** (softened 2026-11-21 at the user's request): `radius.xs` 4 (kbd, inline code, checkbox, Claude Code mark, progress bars); `radius.sm` 6 (buttons, inputs, segmented controls, chips, tree and list rows, menu and palette rows); `radius.md` 8 (tooltips, code blocks); `radius.lg` 10 (panels, note cards, editors, notices, the decision entry box, menus, toasts); `radius.xl` 12 (palette, dialogs, quick capture, updater window). A row inside a padded container takes the container's radius minus the padding (a menu's 10 minus its 4), so the corners stay concentric. Hover and selection fills are inset pills, never full-bleed: tree rows sit 8 inside the sidebar, menu rows 4 inside the menu, and list rows in the content column hang 8 into its gutter so their text stays on the column's edge. Docked panes (titlebar, sidebar, tool windows, status bar) stay square: they share edges with the window and with each other. Circles only for the human author mark, unread dot, scrollbar thumb and the nodes of the commit graph (components-repo.md 5.R.6).
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
| 260           | content column max 520, centred    | (view min to 60% window)            |
|               +------------------------------------+                                     |
|               | Bottom panel (hidden), header 28   |                                     |
|               | 320 (view min to 70% of the body)  |                                     |
+---------------+------------------------------------+-------------------------------------+
| Status bar 24: project, Claude Code                                   update, version    |
+------------------------------------------------------------------------------------------+
```

Five regions, each with one job: the tree finds, the page reads and writes, tool windows run things beside it, the titlebar says where you are, the status bar says what Acta is connected to. This is the **default** arrangement. The four areas (left sidebar, page area, right sidebar, bottom panel) and the views in them can be shown, hidden, resized and moved, and nothing moved is ever hard to put back: the model, the drag and drop, persistence and recovery are in 4.8 ([workbench.md](workbench.md)), which wins where this section and it disagree.

- **Sidebar: one tree, every project.** Top-level rows are projects, each collapsible. Under an expanded project: its Inbox and Knowledge rows, then the topics that belong to no epic, then the epics with their topics; closed topics fold under one "3 closed" row per group. Opening a page reveals its row. Pinned at the bottom, outside the scroll, above a 1px `border.subtle` line: the Contacts row (`users`; only once a contact exists, components-contacts.md 5), the Trash row (`trash-2`) and the Settings row (`settings`). The tree is the Projects view (4.8.3); in the left sidebar it sits under the area header "Projects" (4.8.4), which is how it is moved. Width 260, resizable from 200 to 60% of the window, hidden with `Mod+B`. Structure, geometry, signals, long trees and empty states are in 4.2.1; the row is in 5 (Tree row).
- **Page area: one page per tab.** A page is a topic, an epic, a project, an inbox, a knowledge base, a document or memory, a questions page, the contacts page, a contact page, the trash, settings or the shortcuts list. Every record page has the same skeleton: title, properties block, then its sections. Section order in a topic is fixed: title, properties (status, due and report to when set, epic, folders, then the meta line with id and updated), current state, description, questions, knowledge, timeline; the timeline is last because it is the only unbounded section. An epic page: title, properties (progress, due and report to when set, folders, the meta line), the collapsed "Needs attention" section when something applies, description, questions, knowledge, topic table. A project page: title, properties (progress, due when set, folders, the meta line), Overview, description, questions, knowledge, topic table grouped by epic. The Overview (components-records.md 5) says what needs the human and nothing else; it is never in the tree, the status bar or a view of its own. Questions sit between description and knowledge on all three (components-questions.md 5). The content column is centred, at most the Reading width (typography.md 3.3: 440, 520 or 680), with at least 32 of side padding and 32 above the title. Sections of a page are 32 apart, each opened by a 28 header row with its `type.ui-strong` `text.secondary` label vertically centred (typography.md 3.2); space separates them, never a line.
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

Content, left to right after the platform inset: on Windows and Linux the app icon, 16, vertically centred, 8 before the next control (the hand-rendered `hicolor` bitmap of the scale: 16 at 1x, 24 at 1.5x, 32 at 2x, never a downscaled large one; part of the drag region, no hover state, at 60% opacity when the window is inactive; macOS shows none, its traffic lights sit there); back and forward (`arrow-left`, `arrow-right`; compact ghost buttons, disabled at the ends of the active tab's history); the breadcrumb of the active page; drag region; "Search" button showing the palette shortcut (icon only, `search`, under 1040 of window width); 8; the **view toggles**, one per view (`list-tree`, `inbox`, `library`, `square-terminal`, `message-circle` and `globe` on Windows only, `bot`, `terminal` for Claude Code (no project), 4.5.1), gap 4; 8; the **area toggles** (`panel-left`, `panel-bottom`, `panel-right`), gap 4; the **Layout menu** (`panels-top-left`). All are 28 x 28 ghost icon buttons; they are always shown, whatever is hidden, because they are the way back (4.8.5, 4.8.11). A toggle whose view or area is visible shows `fill.selected` and its icon in `text.primary`. The whole bar is a drag region except the controls; double-click follows the platform (zoom on macOS, maximize elsewhere).

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
- **Tab** (height 32, padding 0 x 12, gap 8, min width 120, max 220): page icon 14 in `text.secondary` (the status glyph for a topic, `layers` for an epic, `folder` for a project, `file-text` or `pin` for knowledge, `inbox`, `library`, `trash-2`, `settings`, `users` for the contacts page, `user` for a contact page), title in `type.ui` `text.secondary`, truncated; trailing slot 16: a ghost `x` on hover and on the active tab, or the 6 `accent` unread dot when another process changed the page while the tab was in the background. Active: fill `bg.base`, so the tab joins its page, no bottom border under it, title `text.primary`. No accent line: tone and text weight carry the state. Hover: `fill.hover`. Keyboard focus ring inset. Dragging reorders, as tree rows do.
- **Overflow**: tabs shrink to 120, then the bar scrolls horizontally and a ghost `chevron-down` at its right end opens a menu of all tabs.
- **Keys**: `Mod+W` closes the active tab (the last remaining tab cannot be closed; the key then does nothing), `Ctrl+Tab` / `Ctrl+Shift+Tab` go to the next and previous tab on every platform. Palette: "Close tab", "Close other tabs", "Reopen closed tab".
- Tabs belong to the window, not to a project: two tabs may show pages of different projects. They are restored at launch.
- **Groups** (4.8.8): the page area can be split into up to three groups, each with its own tab bar, by dragging a tab to an edge or with "Split right" (`Mod+\`). Everything above holds per group. Views moved into the page area are tabs too (view tabs, 4.8.8).

### 4.5 Tool windows

A tool window is a view (4.8.3) that runs beside the page and keeps running when hidden: the **terminal** and the **chat** (ADR 0009), with the tree described above. By default both are tabs of the right sidebar (648 wide, 400 to 60% of the window); like every view they can be moved to any area, share an area as tabs, or sit in two groups to be seen together (4.8).

- **Toggle**: the shortcut shows and focuses the view where it was last; pressed while it has focus, it hides it and returns focus to the page (4.8.5). `Mod+Shift+Enter` maximizes the focused area or group over the body and back.
- **Header**: the area header of 4.8.4 (28 high): the view's name or the view tabs, a context text in `type.small` `text.tertiary`, the view's own buttons, the view menu and `minus`, which hides the area. The header is what a view is dragged by.
- **Terminal header** (the switcher and the narrow priorities below stand; the cross-page mismatch and "Restart here" are replaced by 4.5.1, where a session is shown only on its own page): the context names what the session is on, which is the page it was started from: "on topic Embed the terminal" (the name is a link), then the **folder switcher** (5, Working folders) naming the folder the session runs in. Button: Restart here (`rotate-cw`). Sessions belong to the page they were started from and the view shows the current page's own (4.5.1, which replaced "one session per project" on 2026-11-22). Navigation and folder switches never restart or move a session.
  - *Switching from the header.* The header switcher lists the folders of the session's page (not of the active page) and sets that page's active folder, exactly as the switcher on the page does. The session keeps running where it is; the switcher shows the newly chosen folder followed by "(running in embed-terminal)" in `type.small` `text.tertiary`, so the header never claims a folder the session is not in, and the mismatch below appears.
  - *Mismatch.* When the session was started from another topic, epic or project page than the active one, or its page's active folder is no longer the one it runs in, or the commit mode of the folder it runs in changed since it started (ADR 0011: its instruction is out of date), the context text turns `text.primary` and a ghost text button appears next to it: "Restart in acta-docs" when only the session page's active folder changed, "Restart here" otherwise. Nothing else marks the mismatch.
  - "Restart terminal here" (the "Restart here" button, the icon button, and the palette) ends the session and starts it with the active page's active folder, other folders and focus; "Restart in ..." does the same for the session's own page. With a process running both are a confirmed case of 6.4 ("Restart the terminal? Claude Code is still running in it.", Cancel, Restart). A restart is never automatic.
- **Chat header and states**: see Chat panel in 5.
- Dividers between areas, and between groups, are resize handles (5, 4.8.6).

### 4.5.1 Tools belong to the page they were started from

Decided by the user on 2026-11-22, replacing "one session per project" (4.5 above, components-app.md 5): "Where we are when we start it determines that it starts in the right folder with what it must know. When we move to other topics, that Claude Code is not shown there, only from the place it was launched. That is what must be clearly visualised and understood." And: "the same for claude.ai, the browsers, everything of that kind." One rule for every tool, so one thing to learn:

**A Claude Code session, a chat and an agent browser each belong to one page, the page it was started from. The Terminal, Chat and Agent browser views show the current page's own and nothing else.** Everything else keeps running, out of sight, and is reached by going back to its page.

*Which page owns.* Only a project, an epic or a topic owns tools (they are what has folders and a briefing). Pages that belong to one stand for it: the inbox, the knowledge base and the questions page for their scope (the project, or the epic or topic they are scoped to), a document or memory for its owner. Settings, Trash, Keyboard shortcuts, the Contacts page and a contact page own nothing.

*The current page* is the active tab of the focused page group (4.8.8), or of the last focused one while the focus is in a side area or the bottom panel. When it changes, on the next frame each of the three views shows that page's instances, or its empty state. Nothing is stopped, restarted or moved by navigation: a session's process, a chat's page and a browser's page keep running where they are. Two page groups showing two topics side by side: the views follow the group with the focus.

*Several per page.* A page can have several of each, numbered in the order they were opened, the lowest free number reused: "Claude 1", "Claude 2" (`tool.claude_n`); "Chat 1"; "Browser 1". With one instance the view looks as it does today; from two, an **instance strip** (28, `bg.sunken`, bottom border `border.subtle`, padding 0 x 8, gap 4) under the area header lists them as compact tabs (24, padding 0 x 8, `radius.sm`, `type.small`; the session's `bot` 14 in its state colour before a Claude name; active `fill.selected` `text.primary`, others `text.secondary`, hover `fill.hover`; a 16 trailing slot reserved on every tab for the `x` shown on hover and on the active one, components.md 5). `Ctrl+PageDown` / `Ctrl+PageUp` move between instances while the view has focus (passed through nowhere else). Limits, because each is a process or a web view: 4 Claude Code sessions per page, 3 chats and 3 browsers per page; at the limit the "New" commands are disabled with the tooltip "4 is the most on one page." (`tool.limit`).

*Ownership is always named.* Every view's header context begins with the owner: "Topic T-42 Embed the terminal" (`tool.owner`, the level word of `level.*`, the id, the title truncated first), then what is specific to the tool (below). It is the one place the view says whose it is, so it is never dropped: under the narrow priorities of 4.5 the title truncates to "T-42" before anything else in the header gives way.

**Claude Code sessions (the Terminal view).**
- *Starting* is one obvious act from any owning page: the terminal toggle (``Ctrl+` ``, `Mod+Shift+J`) shows the view; on a page with no session its empty state has the primary button "Start Claude Code here" (`tool.claude_start`), focused, so `Enter` starts it. Also: ``Ctrl+Shift+` `` (start a session on the current page, showing the view; on every platform, the backtick matched by position, 6.1), the view header's `plus` (tooltip "New Claude Code session here", `tool.claude_new`), the palette ("Start Claude Code here"), and the page line's "Show terminal" (below). A session starts in the page's **active folder** with its other folders, its focus and briefing, and Acta's plugin (components-app.md 5, Starting Claude Code); the first session of a page resumes the page's last conversation when "Continue the last conversation of each page" is on, every further one starts a new conversation.
- *Empty state* (an owning page with no session): heading "No Claude Code on this topic" (`tool.claude_none`, with the level word), sentence "Starts in embed-terminal with what this topic knows." (`tool.claude_none_body`, the active folder's name, its path the tooltip), the primary button. With no folder up the chain, the "Where should Claude Code start?" state of components-folders.md 5, unchanged. On a page that owns nothing: "Claude Code belongs to a project, an epic or a topic. Open one to start it or see its sessions." (`tool.no_owner`) and a ghost button "Claude Code (no project)" that shows that view (below).
- *Header*: the owner, then "in embed-terminal" (the folder the session runs in, its path the tooltip), then the folder switcher of 4.5, then " · waiting for you" in `warning` while it waits. Buttons: `plus`, Restart (`rotate-cw`), `ellipsis` (Restart, End session, then the view menu).
- *Restart* replaces "Restart here" and "Restart in ...": a session can only be on its own page now, so the only mismatch left is the folder. When the page's active folder, or the commit mode of the folder the session runs in, changed since it started, the context turns `text.primary` and the ghost text button "Restart in acta-docs" appears (`tool.restart_in`; the page's active folder's name); otherwise Restart restarts in the same folder. With a process running both are the confirmed case of 6.4, unchanged. "Restart terminal here" in the palette becomes "Restart Claude Code" (the session in view).
- *Out of sight.* A session on another page shows where it always did: the `bot` glyph on its page's tree row, the status bar's Claude Code item, the Sessions view and the page line. "Show terminal" there **opens the owning page** in the active tab and shows its session; the view never shows a session of another page.
- *Ending*: the instance's `x`, End session, or `exit` in the shell. Closing the last session of a page shows the page's empty state.

**Chats (the Chat view).** Page-bound like sessions, which keeps one thing to learn: a conversation started about a topic is found again on that topic. Every chat uses **one shared claude.ai profile** (`<data dir>/webview`, ADR 0009), so signing in once signs in every chat on every page (the fewest logins, Settled 2026-11-21). A new chat opens claude.ai's new-conversation page; each chat remembers the address it was last on, as an encrypted value of its page (data.md 11, request 27), so coming back to the page, even after a restart, brings it back where it was. Header: the owner, then "claude.ai". Empty state on an owning page: the Not started state of components-app.md 5 with "Open claude.ai here" creating Chat 1 of this page. "Sign out and clear chat data..." clears the shared profile: every chat of every page goes back to Not started (the confirmation says "every chat").

**Agent browsers (the Agent browser view).** Page-bound, on **one shared profile** (`<data dir>/agent-browser`, ADR 0010), so a site signed into once is signed in for every page. A browser is opened by the human ("New browser here", the view's `plus`) or by a Claude Code session of the page on its first `browser` call. **Each session drives its own browser by default**: the one it opened, named after it in the strip ("Browser for Claude 2", `tool.browser_for`). A browser the human opened is "Browser 1" and any session of the same page may drive it by number (the existing tools' optional `browser` argument; their replies' first line lists the page's browsers, "browsers: 2 (yours), 1"; data.md 11, request 27). A session never reaches a browser of another page. One controlling connection per browser (ADR 0010, 3, per instance). Header: the owner, then "driven by Claude 2" (`tool.driven_by`) or nothing. Stop, Resume, the handoff and the activity line are per browser.

**Resources.** Off-screen instances keep running, with two limits so a day of pages does not fill the memory: a web view (chat or browser) that has been off screen and unused for 10 minutes is closed and its address kept, and when its page comes back it reopens there ("Opening claude.ai..." or "Starting the browser..." after 400 ms, as on first open); at most 6 web views are open at once, beyond which the one off screen for the longest closes first (a browser with a call in flight or a handoff waiting is never closed). An agent browser off screen is parked, as before, so the agent's screenshots keep working while its page is elsewhere. Claude Code sessions are never closed by Acta: they are the person's work; their count per page is the only limit.

**Claude Code (no project)** (the user's request of 2026-11-22: "a basic Claude Code with nothing by default, in my home folder, for when I want it to work without context"). A separate **view**, not a page's tool, because it belongs to no page: key `free_claude`, name "Claude Code (no project)" (`tool.free`; « Claude Code (sans projet) »), icon `terminal` (the prompt without the frame, never the page terminal's `square-terminal`), default place the bottom panel, hidden; a titlebar toggle after the Sessions view's; key `Ctrl+Alt+J` (`Cmd+Option+J` on macOS; the AltGr rule of 6.1 applies); palette "Claude Code (no project): Show" and "New Claude Code (no project)". Why a view of its own and not the terminal's empty state or a page: it must look the same from every page, which is exactly what a page-bound view does not do, and it must never be mistaken for a page's session, which a shared view and icon would invite.
- *What it is*: `claude` started in the **home folder**, with no `--add-dir`, no focus, no briefing and **no `--plugin-dir`**: Acta passes nothing, so it is plain Claude Code. (If the person installed Acta's plugin in Claude Code globally, its hooks find no attached folder at the home folder and stay silent, as for any session outside Acta's folders, ADR 0005; to verify, PLAN.md.) No presence row, so it is not in the Sessions view, the tree or the status bar's count; it has no agent browser (the `browser` tools come with Acta's plugin).
- *Header*: name "Claude Code (no project)", context "In your home folder, without Acta's context" (`tool.free_context`, the home path as the tooltip). Several are allowed, up to 4, in the instance strip as "Claude 1", "Claude 2"; `plus` "New Claude Code (no project)".
- *Empty state*: heading "Claude Code without a project" (`tool.free_title`), sentence "For work that needs no project: it starts in your home folder and knows nothing about Acta." (`tool.free_body`), primary button "Start" (`tool.free_start`).
- Conversations are not resumed: each start is a new conversation (`claude` with no arguments). Closing, restarting and quitting with a process running follow 6.4, as every terminal.

*Migration from one session per project.* Nothing is migrated at runtime: sessions do not survive a restart today. After the update, the first time the Terminal view is shown a page with no session shows its empty state; a page's "last conversation" (already stored per page) is what its first session resumes, so work picked up on a topic continues there. The project session that used to follow every topic simply is the project page's session now. The `preferences.json` layout keeps its view keys; `free_claude` is new, placed by the defaults (4.8.12: an unknown key on an older Acta is dropped, a missing one placed at its default).

### 4.6 Status bar

24 high, `bg.sunken`, top border `border.subtle`, full window width, `type.small`, `text.tertiary`. Items are ghost controls as tall as the bar with padding 0 x 8 (hover `fill.hover` and `text.secondary`, keyboard focus ring inset). It answers two questions that had no home, is Claude Code wired to this work and which Acta is this, and it takes the update indicator out of the titlebar. It never shows messages, counts, a clock or progress; those would make it a second place to look.

- **Left**: the active project (`folder` 14 and its name; activating it is "Switch project", `Mod+O`), then the **Claude Code status**, then the **plan usage** meters (components-app.md 5, Plan usage).
- **The active project** is the project of the active page. Pages that belong to no project (Settings, Trash, Keyboard shortcuts) keep the project of the last page in this window that had one. It decides what the status bar names, where quick capture files a note and what `Mod+L` and `Mod+T` act on. On first run, with no project at all, there is none: the project item is absent, the Claude Code status reports setup only, and whatever needs a project shows its empty state (the first-run page, 8.5).
- **Right**: the zoom item while the zoom is not 100% (4.9), the update indicator (5), then the version ("Acta 0.4.1", `tnum`; opens Settings at About).
- **Claude Code status**: a glyph 14 and words, never colour alone. It reports what `acta setup --check` (ADR 0005) reports, checked at launch, after setup, and when the window regains focus, plus one fact about the active project.

| State | Glyph | Text | Activating it |
|---|---|---|---|
| Setup incomplete | `circle-dashed` in `text.tertiary` | "Claude Code: not set up" | Opens Settings at Claude Code |
| The `claude` command was not found | `circle-slash` in `text.tertiary` | "Claude Code: not found" | Opens Settings at Claude Code |
| Set up, but the active project has no working folder, so sessions started outside Acta cannot find it | `circle` in `text.secondary` | "Claude Code: no folder for this project" | "Set working folder..." |
| Set up and the project has a folder | `circle-check` in `text.secondary` | "Claude Code: ready" | Opens Settings at Claude Code |

The tooltip of the ready state gives the last write: "Last wrote 2 min ago in Embed the terminal". While Claude Code sessions run, the item names only the most urgent state ("Claude Code: 1 waiting for you"; with none waiting "Claude Code: 2 working"; else "Claude Code: 1 idle"), `bot` in that state's colour; the tooltip lists every state with its count, one per line. Activating it shows the Sessions view (components-app.md 5, Claude Code sessions). Clarity review 2026-11-22: the full list made the status bar a sentence; the bar answers "does something need me", the view answers the rest.

### 4.7 Preferences file

Theme, language, the layout of 4.8.12 (areas, sizes, where each view is, page groups), the open tabs (each a typed reference: a record id, or a fixed page key such as `inbox` or `knowledge` with its project id, `trash`, `settings`, `shortcuts`, `contacts`; never a title), the tree's expansion state (folded projects and epics, unfolded "N closed" groups), the Overview's unfolded "More" and "Needs attention" (record ids), the terminal font size, the reading text size and content width (3.2, 3.3), the zoom (`zoom`, an integer percent, 4.9), the folded page sections (`folds`, a list of fixed keys such as `topic.timeline`, never a record id; components-records.md 5, Section fold), the Claude Code settings (new conversation every time, flash when waiting, hide the usage meters), the version that last ran (for "Updated to 0.4.1") and the one-time flags (claude.ai opened in the panel, sign-in help dismissed, chat data clearing still pending; an export suggestion shown, once that suggestion exists) live in `preferences.json` in Acta's data directory, not in the encrypted database: they must be readable before the database opens, so that first run and the key-loss screen come up in the user's theme and language. The file holds no user content and no key material: drafts of unsaved editors are user content and live in the encrypted database, never here; if it is missing or unreadable, Acta starts with the defaults (System theme, English) and says nothing. Questions and people (components-questions.md 5) add three things and no content: the fold of each page's "N closed" questions (a record id, kept with the tree's "N closed" groups as `questions:<id>`), the "Also log it as a decision" checkbox (a boolean, stored only when unchecked), and the questions page as a fixed page key `questions` with its scope's record id. Contacts (components-contacts.md 5) add only the fixed key `contacts` and contact ids as tab references; a contact's name, details and the profile live in the encrypted database, never here. The Markdown editor adds one boolean, `editor_source` (Source view, components-editor.md 5). Dictation (components-voice.md 5) adds four settings and no content: the engine, the chosen model's id, the language and the graphics-card checkbox. The chosen microphone is a device id that can carry a person's name ("Alexandre's headset"), so it lives in the database's local values instead, with the model's recorded size and modification time (ADR 0014, 4).

### 4.9 Zoom

Requested by the user on 2026-11-22: the whole interface must grow and shrink with one key, for screen sharing above all, and a shared screen must be able to show only what the call is about (section folds, components-records.md 5, Section fold).

**What zoom is.** One factor that scales everything Acta draws, as if the display's scale factor were multiplied by it: type, spacing, control heights, icons, borders, radii, the terminal grid, the chat and agent browser web views, overlays, toasts, the updater window. Nothing is laid out differently at a given zoom than in a window of the same *effective size* at 100%: the **effective size** of the window is its size in logical px divided by the zoom, and every rule of 4.2, 4.8.10 and 10.3 reads effective px. Sizes stored in `preferences.json` (area sizes, shares) are effective px too, so a sidebar keeps its proportion to its content when the zoom changes. Colour and contrast do not change (2.2 holds at every zoom).

| | Value |
|---|---|
| Steps | **90%, 100%, 110%, 125%, 150%, 175%, 200%**. The browser ladder, not 10% steps: screen sharing wants 150% in three presses, not five. |
| Default | 100% |
| Floor | 90%. Zoom out exists to win back room on a small screen, and one step does it; at 90% the smallest role (`type.small`, 12) renders at 10.8, and any lower step would break the 12 floor of 3.2 by more than a pixel. |
| Ceiling | 200%, and never more than fits (below). |
| Stored | `zoom` in `preferences.json` (4.7): an integer percent, one per installation (every window, the updater included). A value that is not a step is read as the nearest step; missing or unreadable is 100. |

**Applied zoom and chosen zoom.** A zoomed window needs at least **760 x 480 effective** (the titlebar's toggles and window controls fit at 760; a body of 420 still holds a page of 200 and a bottom panel of 160, 4.8.10). So while the zoom is above 100%, the window's minimum size is max(880, 760 x zoom) by max(560, 480 x zoom) logical px. Zooming in on a window smaller than that grows the window to it, keeping its top-left corner and moving it up or left only as far as needed to stay inside its monitor's work area. A step whose minimum the work area cannot hold is skipped: `Mod+=` at the largest step that fits does nothing. When the window is made smaller by something other than the user's own drag (moved to a smaller monitor, snapped by the system, restored by the OS at a smaller size), the **applied** zoom drops to the largest step that fits and the **chosen** zoom stays stored; the applied zoom returns to the chosen one as soon as the window has room. As in 4.8.10, what the window allows is never saved. Examples, maximised: 1920 x 1040 logical takes 200%; 1536 x 790 (a 1920 x 1080 laptop at 125%, taskbar out) takes 150%; a window snapped to half height on a 1080p screen stays at 100%.

**Rendering.** Text is shaped at the zoomed size, never a scaled bitmap, so hinting and whole-pixel baselines hold (3.1). Lucide icons are rasterised at their zoomed size from the SVG. 1px borders, rules, the 2px focus ring and the scrollbar snap to whole device pixels and are never under one device pixel. Image thumbnails scale from their stored 256 px source (components-attachments.md 5), sharp up to 200%. A zoom change is one frame, no intermediate state, within the 50 ms budget of a theme switch (6.3), never animated; the scroll position of every list and page is kept by anchoring on its first visible item (6.5, rule 2).

**What it multiplies.** Text size (3.2) and Reading width (3.3) keep their meaning in effective px and multiply with the zoom: at 150% with Text size Large, timeline bodies render at 27. The terminal's own text size (11 to 18, 3.2) multiplies too: its cell is the terminal size times the zoom, so zooming reflows the grid like a resize (the session keeps running; fewer columns fit). The terminal size keys stay separate (`Ctrl+Alt+=` / `Ctrl+Alt+-`, `Cmd+Shift+=` / `Cmd+Shift+-`): one sizes the terminal alone, the other everything. The chat and the agent browser get the zoom as their web view's zoom factor (WebView2 `ZoomFactor`), so a page in them grows with the window; their own zoom is turned off (`IsZoomControlEnabled` and `IsPinchZoomEnabled` false) so that `Ctrl+wheel` or a pinch inside them cannot give one view a zoom of its own. The agent browser's page then lays out at a narrower CSS width, exactly as when its view is resized; Claude Code's screenshots and clicks are in the page's own coordinates and must keep matching (a test, PLAN.md).

**Keys** (6.1). Zoom in: `Mod+=`, also `Mod++` (`Ctrl+Shift+=` on Windows and Linux; on macOS `Cmd+Shift+=` stays the terminal's text size, so there zoom in is `Cmd+=`, and `Cmd++` only on layouts that type `+` without Shift) and `Mod+Numpad+`. Zoom out: `Mod+-` and `Mod+Numpad-`. Reset to 100%: `Mod+0` and `Mod+Numpad0`. `=`, `+` and `-` match the character the layout types; `0` matches the physical key, as every digit shortcut does (6.1, Keyboard layouts). No wheel or pinch zoom: a pinch or a `Ctrl+wheel` meant for something else would change the whole window by accident, and the keys and the palette are enough for a deliberate act. In the terminal on Windows and Linux, `Ctrl+-` goes through to Claude Code (its undo, 6.1), so there zoom out is the palette or `Ctrl+0`; `Ctrl+=`, `Ctrl+Shift+=` and `Ctrl+0` are kept by Acta (none is a shell or Claude Code key). The chat and agent browser web views keep all of them for Acta.

**Palette** (6.1): "Zoom in", "Zoom out", "Reset zoom", each with its key, and "Zoom: 150%" for each step (`cmd.zoom`, the pattern of "Text size: Large"). At the floor "Zoom out" is not listed, at the largest step that fits "Zoom in" is not, at 100% "Reset zoom" is not.

**Indicator: the status bar, not a toast.** The change is seen on the whole window, so a toast would say what is already visible (DESIGN 1, never-list). What is not visible, days later, is that the window is *not* at 100%; so while the applied zoom is not 100% the status bar carries one item at the right, before the update indicator: "Zoom 150%" (`zoom.status`, `type.small` `text.tertiary` `tnum`, a ghost control like the other items: at rest 6.18 / 5.33 on `bg.sunken`, hover `text.secondary` on `fill.hover` 6.96 / 6.14). Activating it resets to 100%; tooltip "Reset to 100%" with `Mod+0` (`zoom.status.tip`). When the applied zoom is below the chosen one, the item shows the applied value and the tooltip adds the line "You chose 175%. 150% is the most this window can show." (`zoom.limited`). At 100% the item is absent. When the status bar's items no longer fit (an effective width near 760), they give way in this order: the version, then the plan usage meters, then the Claude Code item's words (its glyph stays, the words move into its tooltip); the active project's name truncates last; the zoom item never goes, since it is the way back.

**Settings > Appearance** (components-app.md 5): a row "Zoom" (`settings.zoom`), description "Everything in the window." (`settings.zoom.hint`: it is what tells it apart from Text size, "Timelines, notes and documents."), control the stepper of the Terminal text size row, "[- 150% +]", minus disabled at 90%, plus disabled at the largest step that fits. Placed after Language and before Text size: the general before the particular.

**Screen sharing** (the user's use). Showing only what the call is about is ordinary Acta, not a mode: `Mod+=` to zoom; `Mod+Shift+[` to collapse the open page's sections, then open only the one being discussed (components-records.md 5, Section fold); `Mod+B` hides the tree, which names every project, or "Projects: Collapse all" folds it; `Mod+Shift+B` hides the terminal and the chat; Settings > Claude Code, "Show plan usage in the status bar", hides the meters. All of it is remembered, so a person who shares often finds the window as they left it.

*Considered and refused: a "Presenting" mode* (one switch hiding the usage meters, sessions, unread dots, the tree, contact notes). (a) What is private changes with each call: the tree that must hide from a client is what a teammate came to see, so a fixed list gets the call wrong either way. (b) A mode that hides things silently is a second state to forget: whoever ends the call without turning it off finds meters and dots gone days later with nothing saying why, the failure the zoom indicator exists to prevent. (c) Each thing it would hide already has its own visible, remembered control (above), and unread dots are not private: they say that Claude Code wrote, never what. Revisit only if the user, having used folds and zoom, still asks for one switch.
