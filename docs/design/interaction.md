# Acta design: interaction

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 6. Interaction

### 6.1 Keyboard map

| Action | Windows / Linux | macOS |
|---|---|---|
| Command palette (everything) | `Ctrl+K`, also `Ctrl+Shift+K` | `Cmd+K`, also `Cmd+Shift+K` |
| Go to topic (palette scoped to topics) | `Ctrl+P` | `Cmd+P` |
| Switch project (palette scoped to projects; opens the project page) | `Ctrl+O` | `Cmd+O` |
| Capture a note (from anywhere) | `Ctrl+N` | `Cmd+N` |
| New topic (in the current epic) | `Ctrl+T` | `Cmd+T` |
| Go to inbox | `Ctrl+I` | `Cmd+I` |
| Open knowledge (palette scoped to documents and memories in scope, nearest level first) | `Ctrl+D` | `Cmd+D` |
| Go to the project's knowledge base | `Ctrl+L` | `Cmd+L` |
| Go to heading (in a document) | `Ctrl+Shift+O` | `Cmd+Shift+O` |
| Dictate: press to start and again to stop, or hold to talk (only in the multi-line editors; components-voice.md 5) | `Ctrl+Shift+Space` | `Cmd+Shift+Space` |
| Attach files (only in quick capture, the inbox capture, the note editor, an entry being edited and the description editor; components-attachments.md 5) | `Ctrl+Shift+A` | `Cmd+Shift+A` |
| Back / forward in the active tab's history | `Alt+Left` / `Alt+Right` | `Cmd+[` / `Cmd+]` |
| Open in a new tab (on a row or a palette result) | `Ctrl+Enter` | `Cmd+Enter` |
| Close tab | `Ctrl+W` | `Cmd+W` |
| Next / previous tab | `Ctrl+Tab` / `Ctrl+Shift+Tab` | `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Toggle terminal (show and focus; if focused, hide) | ``Ctrl+` `` or `Ctrl+Shift+J` | ``Ctrl+` `` or `Cmd+Shift+J` |
| Toggle chat (same rule; on Linux, opens claude.ai in the browser) | `Ctrl+Shift+H` | `Cmd+Shift+H` |
| Maximize the focused area or group over the body, and back (4.8) | `Ctrl+Shift+Enter` | `Cmd+Shift+Enter` |
| Toggle left sidebar | `Ctrl+B` | `Cmd+B` |
| Toggle right sidebar | `Ctrl+Shift+B` | `Cmd+Shift+B` |
| Toggle bottom panel | `Ctrl+J` | `Cmd+J` |
| Split right (move the active tab into a new group) | `Ctrl+\` | `Cmd+\` |
| Focus Projects / the page area / Terminal / Chat, wherever each is (shows it if hidden) | `Ctrl+1` / `2` / `3` / `4` | `Cmd+1` / `2` / `3` / `4` |
| Focus next / previous area (6.2) | `F6` / `Shift+F6` | `F6` / `Shift+F6` |
| Save the editor in focus | `Ctrl+Enter` | `Cmd+Enter` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Shift+Z` or `Ctrl+Y` | `Cmd+Z` / `Cmd+Shift+Z` |
| Zoom in / out / reset to 100% (layout.md 4.9; the whole window) | `Ctrl+=` or `Ctrl++` / `Ctrl+-` / `Ctrl+0` (and the numpad keys) | `Cmd+=` / `Cmd+-` / `Cmd+0` (and the numpad keys) |
| Collapse all / expand all sections of the open page (components-records.md 5, Section fold) | `Ctrl+Shift+[` / `Ctrl+Shift+]` | `Cmd+Shift+[` / `Cmd+Shift+]` |
| Settings | `Ctrl+,` | `Cmd+,` |
| Keyboard shortcuts list | `Ctrl+/` | `Cmd+/` |

Inside lists and the tree (no modifier, only when a text field is not focused): `Up` / `Down` or `K` / `J` move; `Home` / `End`; `Left` / `Right` collapse / expand or go to parent / first child; `Enter` open; `Mod+Enter` open in a new tab; `E` or `F2` rename or edit; `S` set status, then `1` to `6`; `D` set due date (the picker of components-records.md 5, on project, epic and topic rows); `R` set Report to (the person popover of components-questions.md 5, on epic and topic rows and on their pages); on a focused question row `A` marks it answered and opens quick capture for the answer (`Q` was removed on 2026-11-22: questions come from notes) (components-questions.md 5); on a knowledge row, an unfolded memory or an entry page `V` marks the entry as still true (components-knowledge.md 5, Memory quality); `Alt+Up` / `Alt+Down` reorder; `M` move to epic; `Delete` (or `Cmd+Backspace`) delete with undo; `Shift+F10` or the menu key opens the context menu. In the tree (4.2.1): the cursor stops on every row, ghost rows included ("N closed": `Enter`, `Space` or `Right` unfolds, `Left` folds; "New topic": `Enter` or `Space` creates there); `Left` on a topic or a folded row goes to its parent, on an unfolded project, epic or "N closed" row folds it; `Right` on a folded row unfolds it, on an unfolded one goes to its first child; `Home` and `End` go to the first and last row; the sticky project row is not a stop; type-ahead matches project, epic and topic titles only; `Mod+1` focuses the tree with the cursor on the open page's row, revealing it (4.2.1). "Projects: Collapse all" is in the palette and the Projects view menu, with no key. On a topic page, `Up` / `Down` step through blocks (title, current state, description, each entry) and `Enter` edits the focused block. Type-ahead in the tree jumps to a title.

In the working folders list (the folder row is one of the blocks `Up` / `Down` step through): `Enter` or `Right` on the row unfolds it and moves to the active folder, `Left` or `Esc` folds it. The switcher is a stop of its own inside the row (`Tab` reaches it): `Space`, `Enter` or `Alt+Down` opens its menu. On any folder in the list, own or inherited: `Space` makes it active here; `Enter` reveals it in the file manager, selected in its parent (where the Linux desktop cannot reveal, it opens the folder itself); on a folder that was not found `Enter` is Locate; `Mod+C` copies its full path; `Shift+F10` or the menu key opens its menu. On an own folder only: `Alt+Up` / `Alt+Down` reorder (order never changes which folder is active), `F2` opens its folder settings with the name selected, `E` is "Choose another folder...", `Delete` (or `Cmd+Backspace`) removes it from this level with undo. On an inherited folder those keys do nothing. Adding has no key of its own: the "Add folder..." button is the last stop of the list, and "Set working folder..." is in the palette. Folders are set once per piece of work, so they take no global shortcut.

**The terminal and the chat own the keyboard when focused.** A shell and a web page both have their own use for almost every key, so while one of them has focus Acta keeps only the keys below and passes everything else through. `Mod+Shift+K` is the palette key that works everywhere, which is why it exists on every platform: `Ctrl+K` is a readline key in the shell, and `Mod+K` is claude.ai's own shortcut in the chat.

| Focus | Platform | Acta keeps | Goes through |
|---|---|---|---|
| Terminal | Windows, Linux | `Ctrl+Shift+K` (palette); ``Ctrl+` `` and `Ctrl+Shift+J` (terminal); `Ctrl+Shift+H` (chat); `Ctrl+Shift+B` (right sidebar); `Ctrl+1` to `Ctrl+4`; `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6`; `Ctrl+Shift+Enter`; `Ctrl+Shift+C`, `Ctrl+Shift+V` (copy, paste); `Ctrl+Alt+=`, `Ctrl+Alt+-` (text size); `Ctrl+=`, `Ctrl+Shift+=`, `Ctrl+0`, `Ctrl+Numpad+`, `Ctrl+Numpad0` (zoom, layout.md 4.9) | `Ctrl+-`, `Ctrl+Numpad-` and `Ctrl+Shift+-` (all reach Claude Code as its undo; zoom out from the terminal is the palette or `Ctrl+0`), `Ctrl+Shift+[` and `Ctrl+Shift+]`, `Ctrl+J` (Claude Code's newline; the bottom panel is toggled from the titlebar or the palette there), `Ctrl+\` (the shell's quit signal), every `Ctrl+letter` (`Ctrl+K`, `Ctrl+P`, `Ctrl+N`, `Ctrl+E`, `Ctrl+B`, `Ctrl+W` are readline keys), `Ctrl+Enter`, `Ctrl+,`, `Ctrl+/`, `Alt` combinations, function keys other than `F6` |
| Terminal | macOS | Every `Cmd` shortcut of the table above except undo and redo, `Cmd+C` and `Cmd+V`, `Cmd+Shift+=` and `Cmd+Shift+-`; and from the `Ctrl` side only ``Ctrl+` ``, `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6` | `Cmd+Z` and `Cmd+Shift+Z` (as `Ctrl+Z` does on Windows and Linux: Acta's undo never fires from inside the terminal), every other `Ctrl` and `Option` combination |
| Chat web view | Windows | `Ctrl+Shift+K`; `Ctrl+Shift+H`; `Ctrl+Shift+B`; ``Ctrl+` `` and `Ctrl+Shift+J`; `Ctrl+1` to `Ctrl+4`; `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6`; `Ctrl+Shift+Enter`; in the agent browser only, `F12` (developer tools); the zoom keys `Ctrl+=`, `Ctrl+Shift+=`, `Ctrl+-`, `Ctrl+0` and their numpad forms (the web view's own zoom is off, layout.md 4.9). The same set holds in the agent browser | Everything else, `Ctrl+K`, `Ctrl+W`, `Ctrl+N`, `Ctrl+Enter` and plain typing included |
| Chat web view | macOS | `Cmd+Shift+K`; `Cmd+Shift+H`; ``Ctrl+` `` and `Cmd+Shift+J`; `Cmd+1` to `Cmd+4`; `Ctrl+Tab`, `Ctrl+Shift+Tab`; `F6`, `Shift+F6`; `Cmd+Shift+Enter`; `Cmd+=`, `Cmd+-`, `Cmd+0`; the system's own `Cmd+Q`, `Cmd+H`, `Cmd+M` | Everything else, `Cmd+K`, `Cmd+W`, `Cmd+N`, `Cmd+,` included |

On macOS the app must have a standard Edit menu whose items go to the first responder; without it `Cmd+C`, `Cmd+V`, `Cmd+X`, `Cmd+A` and `Cmd+Z` do nothing inside the web view. On Linux, and on macOS for now (ADR 0009, changelog 2026-11-21), there is no chat web view: `Mod+Shift+H` opens claude.ai in the browser and `Mod+4` is not bound; the macOS row above applies once the panel comes to macOS. The web view does not forward keys by itself; the kept keys are routed from it (ADR 0009), and that route working is a condition of shipping the panel.

**Keyboard layouts.** Shortcuts on letters, and the zoom keys on `=`, `+` and `-`, match the character the layout produces. Shortcuts on digits (`Mod+0` included), on the backtick, on the backslash (`Mod+\`) and on the brackets (`Mod+Shift+[`, `Mod+Shift+]`) match the physical key (the US position), because on a French AZERTY keyboard the digit row yields symbols and the backtick needs AltGr; `Ctrl+Shift+J` exists so that the terminal toggle never depends on that key. If the pinned GPUI cannot match by position, the digit shortcuts are dropped on those layouts rather than made unreliable: `F6` and the palette cover them.

**Two meanings of `Mod+Enter`, never at once.** In an editor it saves; on a row, a link or a palette result it opens in a new tab. An editor has no rows and a list has no draft, so the context always decides.

**The palette is the universal entry point.** Every command above, every topic, epic and project, every status change ("Set status: Blocked"), layout command and setting is reachable from `Mod+K`. Each row shows its direct shortcut, so the palette teaches the keyboard. With a topic open, typing a status name or "entry" ranks actions on that topic first.

Palette commands for knowledge and data (exact labels): "Open document or memory...", "Go to knowledge base", "Search knowledge...", "Review memories written by Claude Code", "New document from file...", "Edit", "Show history", "Restore this version", "Go to heading...", "Save as file...", "Move to...", "Change kind to memory" / "Change kind to document", "Export...", "Import...", "Open trash", "Restore from snapshot...". Memory quality (2026-11-22, components-knowledge.md 5), on the focused knowledge row, else the open entry: "Mark as still true" (`V`), "Mark as no longer true", "Make live again" (superseded or retired only), "Merge into..." (memories), "Keep both {a} and {b}" and "Replace {id} with this" (only while the entry has that open pair); on the active project: "Show knowledge to check", "Show superseded and retired", "Find duplicate memories". A trailing ellipsis means the command asks for something before acting.

Palette commands for the workbench (exact labels): "Open settings", "Theme: System", "Theme: Light", "Theme: Dark", "Language: English", "Language: Français", "New project...", "Close tab", "Close other tabs", "Reopen closed tab", "Open in new tab", "Toggle left sidebar", "Toggle right sidebar", "Toggle bottom panel", "Toggle terminal", "Toggle chat", for each view "{view}: Show", "{view}: Hide", "{view}: Move to left sidebar", "{view}: Move to right sidebar", "{view}: Move to bottom panel", "{view}: Move to page area", "{view}: Split from group" (4.8.7), "Split right", "Split down", "Move tab to next group", "Move tab to previous group", "Join groups", "Focus next area", "Focus previous area", "Maximize focused area", "Grow focused area", "Shrink focused area", "Undo layout change", "Reset layout", "Restart terminal here", "Open claude.ai in your browser", "Sign out and clear chat data...", "Set up Claude Code", "Keyboard shortcuts", "Zoom in", "Zoom out", "Reset zoom", "Zoom: 150%" for each step (layout.md 4.9; each listed only when it would change something), "Collapse all sections" and "Expand all sections" (listed only on a page that has a foldable section or group, and only when one is unfolded, or folded, respectively; components-records.md 5, Section fold).

Palette commands for working folders (exact labels): "Switch active folder..." (the palette turns into the list of folders the page sees, filtered by name as typed, each row showing its origin as its path, "Active" on the current one; `Enter` makes the chosen one active), "Set working folder...", "Add working folder...", "Show working folders", "Set commit mode..." (the palette lists "Manual", "Commit automatically", "Commit and push automatically" for the page's active folder; present only when that folder is the page's own and in a git repository), and, with a folder focused, "Make folder active", "Folder settings...", "Move folder up", "Move folder down", "Choose another folder...", "Locate folder...", "Copy folder path", "Show folder in Explorer" (macOS: "Reveal folder in Finder"; Linux: "Show folder in file manager"), "Remove folder from this topic" (or epic, or project), "Apply the same change to 7 folders" (while that offer is open), "Restart terminal here". They act on the open project, epic or topic, and on the active project when another page is open; each row's path, in the format of every palette row, names the target ("Acta / Terminal / Embed the terminal" for a topic, "Acta / Terminal" for an epic, "Acta" for the project), so the level is never a guess. While no level has a folder, "Set working folder..." shows "Acta" from any page, because that is where it will attach.

Palette commands for questions and people (exact labels, 2026-11-21; components-questions.md 5): "Go to questions" (the questions page of the active project), "Go to contact..." (the palette turns into the contacts visible from the active project, nearest level first, each row with its details; `Enter` opens the contact page; replaced "Go to person..." on 2026-11-22), "Set Report to..." (`R`) and "Clear Report to", "Mark reported" / "Mark not reported" (the open or focused topic or epic, when it has a person), and with a question focused "Mark question answered" (`A`), "Edit question" (`E`), "Set answer date..." (`D`), "Clear answer date", "Drop question", "Reopen question", "Move question to..." (`M`), "Delete question", and "Show questions past their answer date" (the Overview's line, components-records.md 5). Targets follow the rule of due dates below; questions are not palette search results (the Questions page lists them, and they are few).

Palette commands for contacts (exact labels, 2026-11-22; components-contacts.md 5): "Go to contacts", "Go to contact...", "New contact" (from anywhere: the name in the palette's input, created at the active project's level; on the Contacts page, in the focused group); with a contact page open or a contact row focused, "Rename contact" (`F2`), "Set job title...", "Set company...", "Edit how to address them", "Move contact to..." (`M`), "Merge contact into...", "Delete contact"; and "Set your name", "Set your job title" (Settings, focused on the input). Contacts are palette search results only through "Go to contact...": the palette's default list is pages and commands.

Palette commands for due dates (exact labels, 2026-11-21): "Set due date..." opens the due date picker (components-records.md 5) for the target; "Clear due date" is listed only when the target has one. The target is the focused row's project, epic or topic when a tree or table row has the keyboard cursor, else the open project, epic or topic page; from any other page the commands are not listed. The row's path names the target as for working folders ("Acta / Terminal / Embed the terminal"). No global shortcut: `D` is the direct key on a page and on rows, like `S`.

Palette commands for dictation (exact labels, 2026-11-22; components-voice.md 5): "Dictate" (listed while a multi-line editor has focus or was the last focused element of the active page; it acts on that editor), "Stop dictating" (while a take runs), "Voice settings".

**Quick capture** (`Mod+N`): a palette-sized overlay (640 wide, same position and surface) containing one multi-line editor with the placeholder "Note for Acta" and the project name as a chip, and under it the optional Report to row with the person field (components-questions.md 5): `Tab` moves from the editor to it, `Shift+Tab` back, and the field starts empty each time. Files can be pasted (`Mod+V` with files or an image and no text on the clipboard), dropped on the overlay, or chosen with the `paperclip` button (`Mod+Shift+A`); they show in the attachment strip under the editor and are saved with the note (components-attachments.md 5). `Mod+Enter` saves to that project's inbox and closes, with no toast (the inbox count increments, which is feedback enough); `Esc` closes and keeps the draft. Works while the terminal is focused via the palette. Capture never navigates away from what the user was reading.

**Editing an entry** (the timeline has no composer: only Claude Code adds entries, components-records.md 5). An entry opened for editing (`Enter` or Edit) scrolls the page by the least amount that shows its editor whole, or brings its top into view when it is taller than the page, so an entry near the bottom never opens its editor below the fold.

### 6.2 Focus

- **`F6` / `Shift+F6`** move focus to the next / previous visible area, in reading order: left sidebar, each shown page group (left to right, or top to bottom), bottom panel, right sidebar, then around. In an area with two groups each group is a stop. Focus lands on the element that last had it there, else the first focusable element of the active view or tab. Hidden areas and folded groups are skipped; the titlebar and status bar are not stops (they are reached by pointer and by the palette).
- **During a view drag** keyboard focus does not move; `Esc` cancels the drag (4.8.7) before it means anything else.
- Exactly one pane is active. The active pane's selection uses `fill.selected`; in inactive panes the same fill stays and the keyboard ring is hidden. Nothing else marks the active pane.
- Focus is always visible when moved by keyboard (2px `focus.ring`). Pointer clicks do not draw rings on buttons and rows but always do on text fields.
- Overlays (palette, capture, menu, dialog, due date picker) take focus on open and return it to the exact previous element on close, including the caret position in an editor.
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
| Tree: fold, unfold, reveal, scroll a frame, a remote change landing | next frame, over 10,000 topics (only the rows in view are laid out, 4.2.1) |
| Switch project | 100 ms |
| Switch tab, theme or language; change the zoom; fold or unfold a section, or all of them | 50 ms, no intermediate state |
| Launch to interactive window with the last pages open, when no update is applied | 400 ms target, 800 ms at most |
| Launch, when the main window is not ready (an update check or download is running) | the updater window (5) by 300 ms; the main window within the bounds of ADR 0004 |
| Terminal key echo | next frame |

Due dates: relative forms and due marks are recomputed at local midnight, on resume and on a clock or time zone change (10.2.1) as one pass over the records in memory, within the next frame with 10,000 topics; the change never animates and never moves a row.

Nothing blocks the UI thread on disk, decryption or the network. If something exceeds its budget the old content stays in place until the new content is ready; a loading indicator (one line of `type.small` `text.tertiary` text, "Opening..." ) may appear only after 400 ms.

**Never animates**: hover, selection, focus ring, navigation between pages, tree expand and collapse, the tree revealing a row, the sticky project row, pane show, hide and resize, snap to hide, zoom, a page section folding or unfolding, the drag chip, the drop preview and its jumps between zones, a view landing after a drop or snapping back after a cancel, Reset layout, typing, scrolling to a keyboard target, theme switch, language switch, tab switch, status change.

**Animates** (the complete list):

| What | Motion | Duration, easing |
|---|---|---|
| Palette, capture, menu, tooltip, dialog, due date picker in | opacity 0 to 1, plus 4px upward translate for palette and dialog | 100 ms, ease-out |
| Same, out | opacity only | 60 ms, linear (input is already live underneath) |
| Toast in / out | opacity plus 8px vertical translate | 140 ms ease-out / 100 ms linear |
| Remote-change highlight | `accent.subtle` background to transparent | holds 600 ms, fades 900 ms, linear |
| Drag reorder | neighbouring rows shift to open the gap | 100 ms, ease-out |

Typing into an overlay that is still fading in works from the first frame. With the OS "reduce motion" setting, everything in the table becomes instant except the remote-change highlight, which becomes a 1.5 s static tint.

### 6.4 Writes, undo, confirmation

- **Optimistic, always.** The UI updates in the same frame as the input and persists in the background. A failed write restores the previous value in place and shows an error toast with "Retry"; the user's text is never lost (it goes back into the editor).
- **Autosave** on commit (`Mod+Enter`, `Enter` for single-line fields, or focus leaving the field). There is no Save button for whole pages and no dirty indicator. Drafts of uncommitted editors survive navigation and restarts; they are stored in the encrypted database (11), never in the preferences file.
- **Undo** (`Mod+Z`) covers, per project and across navigation, the last 100 of: create, edit, delete, move, reorder, status change, due date set, changed or cleared, note filing, every question change (create, edit, answer with or without its decision entry, drop, reopen, reorder, move, answer date), Report to set, changed, marked reported or cleared, adding files (one paste, drop or dialog is one step) and removing one, every working-folder change (set, add, remove, reorder, locate, active folder, name, icon, commit mode and its options), and every knowledge state change (mark as still true, mark as no longer true, make live again, replace, keep both, merge; components-knowledge.md 5, Memory quality). Deleting a project, epic, topic, note, entry, document or memory removes it immediately and shows a toast "Topic deleted" with Undo; deleted things stay in the trash page for 30 days. Saving, moving or restoring a document or memory is undoable too, since every save keeps the previous version. Layout changes (4.8.11, rule 3) are undo steps of the window, not of a project: `Mod+Z` takes whichever of the two is most recent, and "Undo layout change" reaches the layout alone. Undo applies to the human's own actions only; it never reverts something Claude Code wrote (those are edited or deleted explicitly). **Redo** (`Mod+Shift+Z`, also `Ctrl+Y` on Windows and Linux; palette "Redo") mirrors it: it puts back the newest undone step, a write of the active project or a layout change, whichever was undone last, with the same check, so a redo never lands over a write made since the undo (it is then dropped with the toast "Not redone: it was changed since."). A redone step can be undone again. A new action clears what is left to redo of its own kind: a write clears its project's redo, a layout change the window's.
- **Confirmation dialogs** exist only for what `Mod+Z` cannot take back: closing or restarting a terminal, or quitting, while a process is running in it, signing out of the chat panel (its profile is deleted), removing Acta from Claude Code (Settings: "Remove Acta from Claude Code?", Cancel, Remove; setup can be run again but the plugin's registration is gone), importing (the preview is the confirmation), restoring a snapshot, and deleting from the trash for good ("Delete permanently...", "Empty trash..."; components-app.md 5, Trash page). Deleting a project is not one of them: it goes to the trash like everything else. Export is a dialog because it has options and a warning, not because it is dangerous. The dialog names the object and the consequence, and its confirming button repeats the verb ("Restore and restart").

### 6.5 Changes written by Claude Code (or any other process)

The database is watched; changes land in the UI within 100 ms of the write. The rules, in priority order:

1. **Focus and caret never move.** No pane activates, no page or tab switches, even if the changed topic is not the open one.
2. **The viewport never shifts.** Lists and the timeline anchor on the first visible item: when entries are inserted above it, the scroll offset is compensated in the same frame. If the timeline is scrolled to the top (within 8px), new entries appear in place at the top; otherwise a ghost button "2 new entries" (`type.small`, `bg.overlay`, `border.default`, `radius.sm`) is pinned at the top of the timeline viewport; activating it, or pressing `Home`, scrolls there.
3. **What changed is marked, quietly.** A new or changed block visible on screen gets the remote-change highlight once. A topic changed while not open gets the 6 `accent` unread dot on its tree row until opened; a folded epic, project or "N closed" row holding it shows the dot, so folding never hides it (4.2.1). The inbox count increments. No toast, no sound, no badge on the window.
4. **A field being edited is never overwritten.** If the human is editing the current state and Claude Code updates it, the editor keeps the human's text and a notice line appears under it: `triangle-alert` 14 in `warning`, "Claude Code changed this while you were editing." with ghost actions "Show theirs" (side by side, read-only) and "Keep mine". Saving without choosing keeps the human's version and records the other as a Discussion entry by Claude Code, so nothing either side wrote is lost.
5. **Structure changes do not reorder under the pointer.** Rows created, moved or closed by another process appear in their place immediately, but if the pointer or keyboard cursor is in that list, the row under the cursor keeps its screen position (same anchoring as rule 2).
6. **Deleted while open.** If the open topic is deleted by another process, the page stays readable with a notice line at the top ("This topic was deleted by Claude Code.") and a "Restore" button.
