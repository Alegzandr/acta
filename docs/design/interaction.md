# Acta design: interaction

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 6. Interaction

### 6.1 Keyboard map

| Action | Windows / Linux | macOS |
|---|---|---|
| Command palette (everything) | `Ctrl+K`, also `Ctrl+Shift+K` | `Cmd+K`, also `Cmd+Shift+K` |
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

Inside lists and the tree (no modifier, only when a text field is not focused): `Up` / `Down` or `K` / `J` move; `Home` / `End`; `Left` / `Right` collapse / expand or go to parent / first child; `Enter` open; `Mod+Enter` open in a new tab; `E` or `F2` rename or edit; `S` set status, then `1` to `6`; `Alt+Up` / `Alt+Down` reorder; `M` move to epic; `Delete` (or `Cmd+Backspace`) delete with undo; `Shift+F10` or the menu key opens the context menu. On a topic page, `Up` / `Down` step through blocks (title, current state, description, composer, each entry) and `Enter` edits the focused block. Type-ahead in the tree jumps to a title.

In the working folders list (the folder row is one of the blocks `Up` / `Down` step through): `Enter` or `Right` on the row unfolds it and moves to the first folder, `Left` or `Esc` folds it. On any folder: `Enter` reveals it in the file manager, selected in its parent (where the Linux desktop cannot reveal, it opens the folder itself); on a folder that was not found `Enter` is Locate; `Mod+C` copies its full path; `Shift+F10` or the menu key opens its menu. On an own folder only: `Alt+Up` / `Alt+Down` reorder (the folder that reaches the top becomes primary), `P` makes it primary, `E` or `F2` is "Choose another folder...", `Delete` (or `Cmd+Backspace`) removes it from this level with undo. On an inherited, read-only folder those keys do nothing. `Space` toggles the inherited-folders checkbox when the cursor is on it, and does nothing elsewhere in the list. Adding has no key of its own: the "Add folder..." button is the last stop of the list, and "Set working folder..." is in the palette. Folders are set once per piece of work, so they take no global shortcut.

**The terminal and the chat own the keyboard when focused.** A shell and a web page both have their own use for almost every key, so while one of them has focus Acta keeps only the keys below and passes everything else through. `Mod+Shift+K` is the palette key that works everywhere, which is why it exists on every platform: `Ctrl+K` is a readline key in the shell, and `Mod+K` is claude.ai's own shortcut in the chat.

| Focus | Platform | Acta keeps | Goes through |
|---|---|---|---|
| Terminal | Windows, Linux | `Ctrl+Shift+K` (palette); ``Ctrl+` `` and `Ctrl+Shift+J` (terminal); `Ctrl+Shift+H` (chat); `Ctrl+1` to `Ctrl+4`; `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6`; `Ctrl+Shift+Enter`; `Ctrl+Shift+C`, `Ctrl+Shift+V` (copy, paste); `Ctrl+Alt+=`, `Ctrl+Alt+-` (text size) | `Ctrl+-` and `Ctrl+Shift+-` (both reach Claude Code as its undo), every `Ctrl+letter` (`Ctrl+K`, `Ctrl+P`, `Ctrl+N`, `Ctrl+E`, `Ctrl+B`, `Ctrl+W` are readline keys), `Ctrl+Enter`, `Ctrl+,`, `Ctrl+/`, `Alt` combinations, function keys other than `F6` |
| Terminal | macOS | Every `Cmd` shortcut of the table above except undo and redo, `Cmd+C` and `Cmd+V`, `Cmd+Shift+=` and `Cmd+Shift+-`; and from the `Ctrl` side only ``Ctrl+` ``, `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6` | `Cmd+Z` and `Cmd+Shift+Z` (as `Ctrl+Z` does on Windows and Linux: Acta's undo never fires from inside the terminal), every other `Ctrl` and `Option` combination |
| Chat web view | Windows | `Ctrl+Shift+K`; `Ctrl+Shift+H`; ``Ctrl+` `` and `Ctrl+Shift+J`; `Ctrl+1` to `Ctrl+4`; `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6`; `Ctrl+Shift+Enter` | Everything else, `Ctrl+K`, `Ctrl+W`, `Ctrl+N`, `Ctrl+Enter` and plain typing included |
| Chat web view | macOS | `Cmd+Shift+K`; `Cmd+Shift+H`; ``Ctrl+` `` and `Cmd+Shift+J`; `Cmd+1` to `Cmd+4`; `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6`; `Cmd+Shift+Enter`; the system's own `Cmd+Q`, `Cmd+H`, `Cmd+M` | Everything else, `Cmd+K`, `Cmd+W`, `Cmd+N`, `Cmd+,` included |

On macOS the app must have a standard Edit menu whose items go to the first responder; without it `Cmd+C`, `Cmd+V`, `Cmd+X`, `Cmd+A` and `Cmd+Z` do nothing inside the web view. On Linux, and on macOS for now (ADR 0009, changelog 2026-11-21), there is no chat web view: `Mod+Shift+H` opens claude.ai in the browser and `Mod+4` is not bound; the macOS row above applies once the panel comes to macOS. The web view does not forward keys by itself; the kept keys are routed from it (ADR 0009), and that route working is a condition of shipping the panel.

**Keyboard layouts.** Shortcuts on letters match the character the layout produces. Shortcuts on digits and on the backtick match the physical key (the US position), because on a French AZERTY keyboard the digit row yields symbols and the backtick needs AltGr; `Ctrl+Shift+J` exists so that the terminal toggle never depends on that key. If the pinned GPUI cannot match by position, the digit shortcuts are dropped on those layouts rather than made unreliable: `F6` and the palette cover them.

**Two meanings of `Mod+Enter`, never at once.** In an editor it saves; on a row, a link or a palette result it opens in a new tab. An editor has no rows and a list has no draft, so the context always decides.

**The palette is the universal entry point.** Every command above, every topic, epic and project, every status change ("Set status: Blocked"), layout command and setting is reachable from `Mod+K`. Each row shows its direct shortcut, so the palette teaches the keyboard. With a topic open, typing a status name or "entry" ranks actions on that topic first.

Palette commands for knowledge and data (exact labels): "Open document or memory...", "Go to knowledge base", "Search knowledge...", "Review memories written by Claude Code", "New document", "New memory", "New document from file...", "Edit", "Show history", "Restore this version", "Go to heading...", "Save as file...", "Move to...", "Change kind to memory" / "Change kind to document", "Export...", "Import...", "Open trash", "Restore from snapshot...". A trailing ellipsis means the command asks for something before acting.

Palette commands for the workbench (exact labels): "Open settings", "Theme: System", "Theme: Light", "Theme: Dark", "Language: English", "Language: Français", "New project...", "Close tab", "Close other tabs", "Reopen closed tab", "Open in new tab", "Toggle sidebar", "Toggle terminal", "Toggle chat", "Move terminal to bottom" / "Move terminal to right", "Move chat to bottom" / "Move chat to right", "Restart terminal here", "Open claude.ai in your browser", "Sign out and clear chat data...", "Set up Claude Code", "Keyboard shortcuts".

Palette commands for working folders (exact labels): "Set working folder...", "Add working folder...", "Show working folders", and, with a folder focused, "Make primary folder", "Move folder up", "Move folder down", "Choose another folder...", "Locate folder...", "Copy folder path", "Show folder in Explorer" (macOS: "Reveal folder in Finder"; Linux: "Show folder in file manager"), "Remove folder from this topic" (or epic, or project), "Include inherited folders" / "Stop including inherited folders", "Apply the same change to 7 folders" (while that offer is open), "Restart terminal here". They act on the open project, epic or topic, and on the active project when another page is open; each row's path, in the format of every palette row, names the target ("Acta / Terminal / Embed the terminal" for a topic, "Acta / Terminal" for an epic, "Acta" for the project), so the level is never a guess. While no level has a folder, "Set working folder..." shows "Acta" from any page, because that is where it will attach.

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
| Launch to interactive window with the last pages open, when no update is applied | 400 ms target, 800 ms at most |
| Launch, when the main window is not ready (an update check or download is running) | the updater window (5) by 300 ms; the main window within the bounds of ADR 0004 |
| Terminal key echo | next frame |

Nothing blocks the UI thread on disk, decryption or the network. If something exceeds its budget the old content stays in place until the new content is ready; a loading indicator (one line of `type.small` `text.tertiary` text, "Opening..." ) may appear only after 400 ms.

**Never animates**: hover, selection, focus ring, navigation between pages, tree expand and collapse, pane show, hide and resize, typing, scrolling to a keyboard target, theme switch, language switch, tab switch, status change.

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
- **Autosave** on commit (`Mod+Enter`, `Enter` for single-line fields, or focus leaving the field). There is no Save button for whole pages and no dirty indicator. Drafts of uncommitted editors survive navigation and restarts; they are stored in the encrypted database (11), never in the preferences file.
- **Undo** (`Mod+Z`) covers, per project and across navigation, the last 100 of: create, edit, delete, move, reorder, status change, note filing, and every working-folder change (set, add, remove, reorder, locate, include inherited). Deleting a project, epic, topic, note, entry, document or memory removes it immediately and shows a toast "Topic deleted" with Undo; deleted things stay in the trash page for 30 days. Saving, moving or restoring a document or memory is undoable too, since every save keeps the previous version. Undo applies to the human's own actions only; it never reverts something Claude Code wrote (those are edited or deleted explicitly).
- **Confirmation dialogs** exist only for what `Mod+Z` cannot take back: closing or restarting a terminal, or quitting, while a process is running in it, signing out of the chat panel (its profile is deleted), importing (the preview is the confirmation), and restoring a snapshot. Deleting a project is not one of them: it goes to the trash like everything else. Export is a dialog because it has options and a warning, not because it is dangerous. The dialog names the object and the consequence, and its confirming button repeats the verb ("Restore and restart").

### 6.5 Changes written by Claude Code (or any other process)

The database is watched; changes land in the UI within 100 ms of the write. The rules, in priority order:

1. **Focus and caret never move.** No pane activates, no page or tab switches, even if the changed topic is not the open one.
2. **The viewport never shifts.** Lists and the timeline anchor on the first visible item: when entries are inserted above it, the scroll offset is compensated in the same frame. If the timeline is scrolled to the top (within 8px), new entries appear in place at the top; otherwise a ghost button "2 new entries" (`type.small`, `bg.overlay`, `border.default`, `radius.sm`) is pinned at the top of the timeline viewport; activating it, or pressing `Home`, scrolls there.
3. **What changed is marked, quietly.** A new or changed block visible on screen gets the remote-change highlight once. A topic changed while not open gets the 6 `accent` unread dot on its tree row (and its collapsed epic shows the dot) until opened. The inbox count increments. No toast, no sound, no badge on the window.
4. **A field being edited is never overwritten.** If the human is editing the current state and Claude Code updates it, the editor keeps the human's text and a notice line appears under it: `triangle-alert` 14 in `warning`, "Claude Code changed this while you were editing." with ghost actions "Show theirs" (side by side, read-only) and "Keep mine". Saving without choosing keeps the human's version and records the other as a Discussion entry by Claude Code, so nothing either side wrote is lost.
5. **Structure changes do not reorder under the pointer.** Rows created, moved or closed by another process appear in their place immediately, but if the pointer or keyboard cursor is in that list, the row under the cursor keeps its screen position (same anchoring as rule 2).
6. **Deleted while open.** If the open topic is deleted by another process, the page stays readable with a notice line at the top ("This topic was deleted by Claude Code.") and a "Restore" button.
