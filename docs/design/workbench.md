# Acta design: workbench layout

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 4. Spacing, sizing and layout (continued)

### 4.8 Workbench layout: areas, views, moving and recovering

Decided by the user on 2026-11-21 (Direction, amendment): everything around the page can be shown, hidden, resized and moved, the way it can in VS Code, and nothing the user moves or closes is ever hard to put back. This section is the model; 4.2 to 4.5 describe the default arrangement it produces. Where they disagree, this section wins.

Implemented in M8; the catalogue rows of this section are in 10.4.

#### 4.8.1 Vocabulary

- An **area** is a region of the window that holds things. There are four: the **left sidebar**, the **right sidebar**, the **bottom panel** and the **page area**. Areas never move; what is in them does. (VS Code's "primary side bar", "secondary side bar", "panel" and "editor area".)
- A **view** is a movable unit that keeps running when hidden: Projects (the tree), Inbox, Knowledge, Terminal, Chat and Agent browser. A view is in exactly one place at a time, or hidden; it is never duplicated.
- A **group** is a set of tabs inside an area, one of them active. A side area or the bottom panel holds one or two groups; the page area holds one to three.
- A **page** is what 4.2 already calls a page (topic, epic, project, document, settings, trash, shortcuts). Pages live only in the page area. Inbox and Knowledge are both: a page by default, a view once moved out of the page area (4.8.3).

User-facing words, in both languages, are exactly the catalogue's: "left sidebar", "right sidebar", "bottom panel", "page area", "view", "group", "layout". Never "dock", "pane", "panel" alone or "editor" in copy.

#### 4.8.2 Areas

| Area | Key | Position | Default size | Size range | Default content | Default visibility |
|---|---|---|---|---|---|---|
| Left sidebar | `left` | Full body height, left | 260 wide | active view's min width to 60% of the window | Projects | shown |
| Right sidebar | `right` | Full body height, right | 648 wide (80 terminal columns) | active view's min width to 60% of the window | Terminal, Chat (tabs, Terminal active) | hidden |
| Bottom panel | `bottom` | Under the page area only, between the sidebars | 320 high | active view's min height to 70% of the body height | nothing | hidden |
| Page area | `page` | The rest | flexible | min 420 wide with one group, 400 per group when split; min 200 high | one group, the start page | always shown |

"Body" is the window between the titlebar and the status bar. The page area is never hidden and never under its minimum: the other areas clamp to leave it room (4.8.10).

- **Size is per area**, remembered across hiding, moving views and restarts. When the active view needs more than the area's remembered size (the Terminal moved into a 260 left sidebar), the area renders at that view's minimum while the view is active and keeps its remembered size for the others; nothing is rewritten.
- **An area with no view hides itself** when its last view leaves. Shown on purpose while empty (its toggle, its key), it shows one line at the top left, padding 12, `type.small` `text.tertiary`: "Drag a view here, or move one from the palette." It is a drop target like any area.
- **Two groups at most in a side area or the bottom panel**, split along its long axis: stacked top and bottom in a sidebar, side by side in the bottom panel. Each group keeps the share of the area it was given (default half), each at least its active view's minimum. That is how the Terminal and the Chat are both visible on the right.

#### 4.8.3 Views

| View | Key | Icon (Lucide) | Default place | Min width x height | Notes |
|---|---|---|---|---|---|
| Projects | `projects` | `list-tree` | Left sidebar, alone | 200 x 160 | The tree of 4.2, with Trash and Settings pinned at its bottom. |
| Inbox | `inbox` | `inbox` | Page area, as the inbox page | 280 x 200 | Follows the active project when it is a view. |
| Knowledge | `knowledge` | `library` | Page area, as the knowledge base page | 280 x 200 | Same. |
| Terminal | `terminal` | `square-terminal` | Right sidebar, first tab | 400 x 160 | One session per project, unchanged (4.5). Moving never restarts it; the grid reflows to the new size. |
| Chat | `chat` | `message-circle` | Right sidebar, second tab | 400 x 240 | Windows only (ADR 0009). A native web view (4.8.9). Moving never reloads it. |
| Agent browser | `agent_browser` | `globe` | Right sidebar, third tab | 400 x 240 | Windows only (ADR 0010). A native web view (4.8.9), parked rather than hidden when off screen (components-app.md 5, Agent browser). Moving never reloads it. |
| Claude Code sessions | `sessions` | `bot` | Bottom panel, hidden | 280 x 120 | The live Claude Code sessions (components-app.md 5, Claude Code sessions). Hidden on a new install; the status bar's Claude Code item shows it. |

**What a view looks like where it lands.**
- In a side area or the bottom panel, the area header (4.8.4) names the view, so a view drops its page title and its meta line; its content starts 12 below the header with side padding 12. Inbox and Knowledge take the narrow forms their components already define (note cards full width; knowledge rows in their two-line 52 form under 520; the filter row wraps). Projects is the tree, padding 8 as in 4.2.
- In the page area, a view is a tab (4.8.8) with its icon and its name. Under the tab bar, a 28 **view strip** (`bg.sunken`, bottom border `border.subtle`, padding 0 x 8) carries what the area header would have carried except the name and the area buttons: the context text and the view's own controls ("on topic Embed the terminal", the folder switcher, Restart here). Projects, Inbox and Knowledge have no view strip: Projects has nothing to put there; Inbox and Knowledge in the page area are their pages.
- **Inbox and Knowledge, page and view.** By default they are pages: they belong to a project, navigate in the active tab and keep history, exactly as 4.2 says. Moved to a side area or the bottom panel (by dragging their tab, the tab menu or the palette), they become a view that follows the active project (4.6), so the inbox of whatever is being read stays beside it. Moved back into the page area, the view becomes the page of the active project, as a new tab where it was dropped. While Inbox or Knowledge is a view, `Mod+I` or `Mod+L` and its titlebar toggle show and focus the view; the tree's Inbox and Knowledge rows still open the page in the active tab, so both can be open at once.
- A page that is not Inbox or Knowledge (topic, epic, project, document, settings, trash, shortcuts) cannot leave the page area: while one is dragged, only the page area shows drop zones.

#### 4.8.4 Area header and view tabs

Every side area and the bottom panel has one header per group: 28 high, `bg.sunken`, bottom border `border.subtle`, padding 0 x 8, gap 8. It replaces the tool window header of 4.5 and is new for the tree, which had none: the header is the handle a view is dragged by and the home of its menu, so the 28 it costs is the price of being movable.

Left to right:
- **One view in the group**: its name in `type.label` `text.secondary` ("Projects", "Terminal"), then its context text in `type.small` `text.tertiary` (the terminal's "on topic ..." followed by its folder switcher, 4.5; the chat's "claude.ai"; Projects, Inbox and Knowledge have none, Inbox and Knowledge show the active project's name).
- **Two or more views**: one **view tab** per view in place of the name, then the active view's context text.
- Right-aligned, compact ghost icon buttons (24): the active view's own buttons as defined for it (terminal: Restart here; chat: Open in browser, Reload), then `ellipsis` (the **view menu**), then `minus`, which hides the area (not the view). The `minus` tooltip names the area: "Hide left sidebar". On the chat, the `ellipsis` keeps its own items (Sign-in help, Sign out and clear chat data...) above the view menu items, after a separator.

**View tab** (height 24, padding 0 x 8, `radius.sm`, `type.label`, gap 4 between tabs, vertically centred in the header). No icon: the name is short and the header is narrow.

| State | Fill | Text |
|---|---|---|
| Default (inactive) | none | `text.tertiary` (6.18 dark, 5.33 light on `bg.sunken`) |
| Hover | `fill.hover` | `text.secondary` (6.96 / 6.09) |
| Pressed | `fill.pressed` | `text.secondary` |
| Active | `fill.selected` | `text.primary` (10.73 / 11.94) |
| Keyboard focus | focus ring inside, as rows | unchanged |
| Being dragged | the tab at 60% opacity | unchanged |

`Left` / `Right` move between view tabs when one has keyboard focus, `Enter` or `Space` activates. No accent: tone carries the active tab, as on page tabs (4.4). When the tabs do not fit, the ones that do not are replaced by a ghost `chevron-down` at the end of the tab row opening a menu of all the group's views (the active one with a trailing `check`). Which fit is decided from the group's width in the frame before, less the header's padding and buttons: tabs are kept in order from the first while they fit, the active one always, with room left for the 24 chevron.

**View menu** (`ellipsis`, right click or `Shift+F10` on a header, a view tab or a page-area view tab; the context menu component): "Move to left sidebar", "Move to right sidebar", "Move to bottom panel", "Move to page area" (the current area's item disabled, with a trailing `check`), "Split from group" (disabled when the area already has two groups or the view is alone), separator, "Hide". Every item exists in the palette (4.8.7). The tab menu of an Inbox or Knowledge page tab carries the four Move items too.

#### 4.8.5 Showing and hiding

Two kinds of toggle, both in the titlebar (4.3) and both always visible, whatever is hidden.

**View toggles**: one ghost icon button (28 x 28, icon 16; 24 x 24 on macOS, like every titlebar control there) per view, in the table order of 4.8.3, gap 4. Visible view: `fill.selected`, icon `text.primary`. Hidden view: no fill, icon `text.secondary`. Hover `fill.hover`, pressed `fill.pressed`, keyboard focus ring inside. Tooltip: the view's name and its shortcut. Right click opens the view menu. The chat toggle exists on Windows only (ADR 0009); the agent browser's too (ADR 0010). For Inbox and Knowledge while they are pages, the toggle is selected when the active tab of the focused group shows that page.

**Area toggles**: `panel-left`, `panel-bottom`, `panel-right`, same size and states; selected when the area is shown. They replace the sidebar toggle at the left of the titlebar. Right click opens the Layout menu.

**Layout menu**: a ghost icon button `panels-top-left` after the area toggles (tooltip "Layout"), the context menu component, min width 240. Items, with shortcut hints at the right:
1. The views, each with a trailing `check` when visible; choosing one toggles it.
2. Separator. "Left sidebar", "Bottom panel", "Right sidebar", same `check` rule.
3. Separator. "Split right", "Split down", "Join groups" (disabled unless the page area is split).
4. Separator. "Undo layout change" (disabled when there is nothing to undo), "Reset layout".

**The toggle rule** (keys, view toggles, palette "{view}: Show"): a hidden view is shown where it was last (4.8.11) and focused; a visible view without focus is focused; a focused view is hidden and focus returns to the active page. This is 4.5's rule, kept. Hiding a view removes it from its group and keeps it running; its area hides if it was the last view there. A view's toggle shows the area it lives in when that area is hidden. Area toggles show or hide the area with everything in it; showing an area focuses nothing.

#### 4.8.6 Resizing

Every boundary between areas, and between groups in an area, is a resize handle (5): the 1px `border.subtle` line, an invisible 8 hit area lying over the line and the area or group before it, so a native web view after it never covers it; hover and drag show a 2px `accent` line and the resize cursor; dragging resizes live, every frame, clamped to the limits of 4.8.2.

- **Snap to hide.** Dragging a side area or the bottom panel to less than half its active view's minimum hides the area at once, live; dragging back past that point shows it again at its minimum. Released while hidden, the area stays hidden and remembers the size it had before the drag began, so showing it again brings back that size, not the sliver. The page area and groups never snap: a handle between groups stops at the groups' minimums.
- **Double-click** a handle: the default size of the area (4.8.2), or equal shares between groups.
- **Keyboard**: "Grow focused area" and "Shrink focused area" in the palette, steps of 40 (width for a sidebar, height for the bottom panel, share for a group). They replace the four "Widen ..." / "Narrow ..." commands of 5.
- Sizes are saved when the drag ends, never during it (4.8.12).

#### 4.8.7 Moving views

**By dragging.**
- **What drags**: a view tab or the header's name in a side area or the bottom panel; a tab in the page area (page or view). Pressing and moving 4 or more starts the drag; less is a click. A header whose group has one view drags that view from anywhere on the header except its buttons.
- **While dragging**: the source stays where it is at 60% opacity; nothing reflows until the drop. A **drag chip** follows the pointer, 12 right and 12 below it: height 24, padding 0 x 8, gap 4, `bg.overlay`, 1px `border.default`, `radius.sm`, `shadow.overlay`, icon 14 and name in `type.small` `text.primary`. Every native web view is hidden for the whole drag (4.8.9). The cursor is the closed hand; over a place that accepts nothing it is the not-allowed cursor.
- **Drop zones** (computed against the group under the pointer):

| Pointer over | Zone | Result |
|---|---|---|
| A tab row (view tabs in a header, page tabs in a tab bar) | between two tabs | inserted there; preview is a 2px `accent` insertion line, 20 high, between the tabs |
| A header with one view | anywhere | added as the last tab of that group |
| A group's content, centre | the content minus the edge zones | added as the last tab of that group, made active |
| A group's content, edge | the outer 25% along an edge that may split | a new group on that side, holding the view, sized half of the old group |
| A hidden area's edge strip | 24 deep along the body edge where the area would appear (left, right, or above the status bar under the page area) | the area is shown with the view in it, at its remembered size |

An edge may split only if: in the page area, fewer than three groups exist and the split is along the existing direction (the first split chooses rows or columns); in a side area, it is the area's long axis and it has one group; and both resulting groups would keep their minimum width and height. Edges that may not split are not zones; their part of the content is centre. Corners belong to the nearer edge.

- **The drop preview shows exactly where the view will land**, before release, in the same frame as the pointer move, with no animation: a rectangle with the bounds the view will have (the whole group for a centre drop, the new group's half for an edge, the area at its remembered size for a hidden area's strip), inset 4, `radius.md`, 2px `accent` outline and `fill.hover` fill. It never uses `accent.subtle`, which 2.2 forbids under anything but `text.primary` (`text.tertiary` on it measures 4.62 dark, 4.30 light over `bg.sunken`): the preview adds only the tint a hovered row already has, so the text under it keeps the hover contrasts of 2.2 (`text.tertiary` 5.36 / 4.76 over `bg.sunken`, 4.97 / 5.06 over `bg.base`), and its outline measures as the focus ring (7.26 / 5.33 against `bg.sunken`, 6.79 / 5.67 against `bg.base`). The source's own place is not a zone: dropping a view where it already is does nothing.
- **Dropping** commits the move in one frame, focuses the moved view, saves the layout and shows the move toast (4.8.11).
- **Cancelling**: `Esc`, releasing outside every zone (titlebar, status bar, outside the window, a page dragged over a side area), or the window losing focus. The source returns to full opacity where it was; nothing changes; no toast. There are no floating windows and no tear-off in v1: a drop outside the window is a cancel.

**By menu and palette** (the keyboard path; principle 4). The view menu (4.8.4). The palette has, for each view, "{view}: Show", "{view}: Hide", "{view}: Move to left sidebar", "{view}: Move to right sidebar", "{view}: Move to bottom panel", "{view}: Move to page area" and "{view}: Split from group"; with a view focused, its commands rank first. A move by command puts the view as the last tab of the target area's last group, shows that area, focuses the view, and toasts like a drop.

**A move or a drop never lands out of sight.** Only the left sidebar can lack room (4.8.10, step 1): when what lands there would make it auto-hide, the right sidebar narrows to make room, down to its active view's minimum, in the same undo step. With no room even so, a drop there is not a zone (rule 5 of 4.8.11), and a move by command changes nothing and toasts "Not enough room: the window is too narrow for both sidebars." with one ghost action, "Hide the right sidebar and move", which does both as one undo step; being a failure, that toast has `triangle-alert` 16 in `warning` and stays until dismissed (components.md 5, Toast). The last page of the window moved out as Inbox or Knowledge (its tab menu, a drag) stays open, since the page area keeps a page (4.8.8): the view lands beside it, as the palette's move does.

#### 4.8.8 Page area: groups and tabs

Everything in 4.4 holds, per group: each group has its own tabs, its own active tab and each tab its own history. Additions:

- **Tab bar**: shown when the page area holds two or more tabs in all or two or more groups (a lone page in a lone group still has none). In a split, every group has its bar, even with one tab, because it is the group's handle.
- **Reordering**: dragging a tab along its bar reorders, with the insertion line; dragging it onto another group's bar or centre moves it there; onto an edge splits (4.8.7). A tab moved out of a group that it leaves empty closes that group, and its space goes to its neighbour.
- **Split right** (`Mod+\`) and **Split down** (palette) move the active tab into a new group to the right of or under the focused group. They need two tabs in that group (a page is open once, 4.4, so a split never duplicates) and room for both groups at their minimums; otherwise they are disabled in the palette and the Layout menu. At most three groups, in one direction.
- **Move tab to next group** / **Move tab to previous group** (palette) move the active tab; to an edge group's outside they do nothing. **Join groups** (palette, Layout menu) gathers every tab into the focused group, in group order.
- **The focused group** is the one with keyboard focus or last clicked; it is the active group. Navigation from the tree, the palette and links goes into its active tab. Its active tab sets the breadcrumb, the back and forward buttons and the active project (4.6). Nothing else marks it: the active tab's `bg.base` in the other groups stays, as selection stays in inactive panes (6.2).
- **Closing**: `Mod+W` closes the active tab of the focused group; the last tab of a group closes the group. The page area always keeps at least one page: the last page tab of the window cannot be closed, whatever view tabs exist.
- **View tabs** (a view moved into the page area): icon 14 and the view's name, no unread dot; the close `x` hides the view (it keeps running), and so does `Mod+W`. They are not pages: they have no history, no breadcrumb part of their own (the breadcrumb keeps the last page) and never receive navigation.
- **Page padding in a narrow group**: under 452 of group width, a page's side padding drops from 32 to 16, so its column stays at least 368 (3.3 keeps 360).

#### 4.8.9 Native web views while things move

The chat and the agent browser are native child windows drawn above everything Acta draws (ADR 0009). Their rule under overlays (5, Chat panel) extends to layout:

- **During any view or tab drag**, from the 4 px threshold to the drop or the cancel, every visible web view is hidden. Its place shows `bg.sunken` and one line at the top left, padding 16, `type.small` `text.tertiary`: "Hidden while you rearrange the layout." Drop zones and the preview are drawn over that placeholder like over any surface.
- After the drop or cancel, each web view returns at its new bounds in the frame the layout settles, without animation. A moved web view is the same instance: it is repositioned, never recreated or reloaded, and keeps its page, scroll and sign-in.
- **During a resize drag**, web views stay visible and follow their bounds every frame, as today; the handle's hit area lies over the area before the line so the web view cannot swallow it.
- A web view in a page-area tab that is not active, or in a folded group (4.8.10), is hidden, as when its area is hidden.

#### 4.8.10 Narrow windows and minimum sizes

The window minimum stays 880 x 560. Every width and height in this section is in effective px (4.9): a zoomed window is laid out as a window of its size divided by the zoom, down to 760 x 480. When the areas no longer fit, they give way in this order, each step undone in reverse as space returns, and none of it is saved (the remembered layout is what the user chose, not what the window allowed):

1. **The left sidebar auto-hides** (4.2's rule, unchanged): its toggle and `Mod+B` show it as an overlay on top of the page area at its remembered width (at most the window width minus 48), with `shadow.overlay` and a 1px `border.default` edge, dismissed by `Esc` (wherever the keyboard is, except in a terminal, a field, a menu or a dialog), by a click outside or by opening something. A web view in it is shown in it, the overlay being its host; a web view under it is hidden (4.8.9).
2. **The page area folds its groups** when it cannot give every group 400: only the focused group is shown, its tab bar stays, and the bar's `chevron-down` menu lists every group's tabs under `type.label` headings "Group 1", "Group 2"; choosing a tab of another group shows that group in place of the first. `F6` reaches only the shown group. The split returns when the width does.
3. **The right sidebar** clamps between its active view's minimum and the room the page area leaves (at 880 it gets 460 against the page's 420, as today). It auto-hides only when the page area's 420 and the active view's minimum no longer fit side by side, which happens only in a zoomed window (4.9: effective widths from 760 to 820). It then behaves exactly as the auto-hidden left sidebar of step 1, mirrored: its toggle, `Mod+Shift+B` and the keys of its views (``Ctrl+` ``, `Mod+Shift+H`) show it as an overlay at the right edge at its remembered width (at most the window width minus 48), and the same things dismiss it.
4. **The bottom panel** clamps so the page area keeps 200 of height; at the 560 minimum the body is 500, so the panel always keeps at least 160.

Two groups in a sidebar stack vertically and need only height: at the minimum body of 500, two groups of 240 fit; when they do not, the second group folds behind the first the same way as step 2, its views listed in the header's `chevron-down` menu.

#### 4.8.11 Recovering your layout

Rearranging must never be a way to lose something. These rules are absolute: a change that breaks one is a defect, whatever else it fixes.

1. **Every view always has a visible way back.** Its titlebar toggle is shown whether the view is visible, hidden, in a hidden area or in a folded group, and activating it brings the view back where it was last. The Layout menu lists every view by name, the palette has "{view}: Show" for each, and each view keeps its key.
2. **Reset layout is one click away** from the Layout menu, the palette, Settings (Appearance) and the move toast. It is undoable, so it asks nothing.
3. **Every layout change is one undo step**: moving a view, hiding or showing a view or an area, resizing (one step per drag, keyboard steps within one second coalesced), splitting, joining, moving or reordering a tab, resetting. `Mod+Z` undoes the most recent undoable thing, whether a layout change or a write (6.4); "Undo layout change" in the palette and the Layout menu undoes the last layout change even when focus is in an editor, a terminal or the chat, where `Mod+Z` belongs to them. The layout keeps the last 50 steps per window, for the session.
4. **A closed view reopens in its last place and size**: the same area, the same group (or the group nearest it if that group is gone), the same tab position (or the end), the area at its remembered size. A view never reappears in an area it was not in, except after Reset layout.
5. **An invalid drop changes nothing**: the view snaps back to where it was, at full opacity, in one frame, and no toast appears.
6. **The preview always shows the landing place** before release, exactly the bounds the view will take (4.8.7). No zone without a preview; no drop that lands elsewhere than previewed.
7. **No view can be lost.** Hiding an area hides its views with it and keeps their toggles in the titlebar; hiding a view never hides another; an area's last view leaving hides the area instead of leaving an empty frame; preferences that name a view twice, or a place that no longer exists, are repaired at load by putting the view at its default place (4.8.12).

**The move toast.** After a view is moved (drop, menu or palette), or hidden from its view menu or tab, a toast (5) at the bottom left of the page area: "View moved to the bottom panel: Terminal" (or "View hidden: Terminal"), with two ghost actions, "Undo" (`Mod+Z`) and "Reset layout". Because a French message with two actions does not fit 360 on one line, this toast sets the message on its own line (wrapping to at most two) and the actions with the `x` right-aligned on a line under it, 4 apart. It follows the toast rules otherwise: 6 s, paused on hover or focus, replaced by the next one. Hiding by a toggle, a key or `minus` toasts nothing: the toggle that did it is the way back. Resizes, tab moves and splits toast nothing: they are seen and undone where they happened. "Reset layout" itself toasts "Layout reset." with "Undo".

#### 4.8.12 Persistence

The layout is saved per window in `preferences.json` (4.7) under one `layout` key, written when a change ends (drop, release of a resize handle, a toggle) and never during a drag. It holds only fixed keys, numbers and the typed tab references 4.7 already allows (a record id, or a fixed page key with its project id); never a title, a path or anything a view displays.

```json
"layout": {
  "v": 1,
  "areas": {
    "left":   { "shown": true,  "size": 260, "groups": [ { "views": ["projects"], "active": 0 } ] },
    "right":  { "shown": false, "size": 648, "groups": [ { "views": ["terminal", "chat"], "active": 0 } ] },
    "bottom": { "shown": false, "size": 320, "groups": [] }
  },
  "page": {
    "direction": "row",
    "shares": [0.5, 0.5],
    "focused": 0,
    "groups": [
      { "tabs": ["topic:01JD...", "view:terminal"], "active": 0 },
      { "tabs": ["inbox:01JC..."], "active": 0 }
    ]
  },
  "last": { "chat": { "area": "right", "group": 0, "index": 1 } }
}
```

- `last` holds the last place of every hidden view (rule 4). Inbox and Knowledge as views are the keys `inbox` and `knowledge` in a side area or the bottom panel (and `view:inbox` never appears in the page area, where they are pages); as pages they are ordinary tab references. A side area with two groups adds `shares`, as the page area does.
- Loading is forgiving, never failing: an unknown view key (from a newer Acta, or the agent browser on a build without it) is dropped; a view named twice keeps its first place; a size out of range is clamped; a group with nothing valid is removed; a `layout` that cannot be read at all gives the defaults, without a message, like the rest of the file (4.7). The chat on macOS and Linux is ignored, not removed, so a shared preferences file does not lose it.
- **Migration** from the fields M7 writes: `sidebar_width` becomes `left.size`; `terminal_width` (or `chat_width` when only the chat was used) becomes `right.size`; `chat_bottom: true` puts the chat alone in the bottom panel; `chat_height` becomes `bottom.size`; `sidebar_hidden` becomes `left.shown: false`; `tabs` and `active_tab` become the one page group. The old fields are then no longer written.

#### 4.8.13 Defaults and Reset layout

The defaults are 4.2's workbench, unchanged for someone who never moves anything:
- Left sidebar shown, 260, Projects alone.
- Right sidebar hidden, 648, Terminal, Chat and Agent browser as tabs (the last two on Windows), Terminal active. It is hidden on a new install because showing the terminal starts a shell and showing the chat creates a web view; both wait for the user (4.5, 5).
- Bottom panel hidden, 320, empty.
- Page area: one group. Inbox and Knowledge are pages.

**Reset layout** puts every area, view, size and split back to these defaults in one step, in one frame. It keeps the open tabs (gathered into one group, in group order, the focused group's active tab staying active), every terminal session, the chat's page and sign-in, the theme and everything else in the file. It is undoable and toasts "Layout reset." with Undo. Reached from the Layout menu, the palette, the move toast, and Settings > Appearance: a row "Layout", description "Puts every view and panel back where it was on a new install. You can undo it right after.", control a secondary button "Reset layout".
