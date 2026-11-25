# Acta design: files and commits

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts. Inside this part, subsections are numbered 5.R.1 to 5.R.13 so they can be cited.

## 5. Components (continued): files and commits

Added 2026-11-22 at the user's request: "You must also be able to browse the contents of the associated folders if you wish, and the commit histories with the full graph, directly in Acta." Two views, **Files** and **Commits**, and one page-area tab, the **file viewer**, which shows a file or a diff. All three are read-only. Git is read as ADR 0011 reads it (2, and the live-files addendum): `--no-optional-locks`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`, a timeout on every call, never a lock, never a write. Copy is in catalogue 10.4.20 (pending); the build steps are in PLAN.md, Files and commits.

States, the focus ring and the controls used here (buttons, inputs, tree row, list row, menus, segmented control, notice, empty state, toast, scrollbar, resize handle) are in [components.md](components.md); areas, views, view strips and tabs in [workbench.md](workbench.md) (4.8).

### 5.R.1 Decisions

1. **Two views, page-bound.** Files and Commits are views (4.8.3), like the terminal: shown, hidden, moved and resized like every view. They show the folders of the page being read (the rule is in 5.R.2), never a folder of their own choosing, so what is browsed is always the work in front of the user, as for sessions.
2. **Read-only in v1.** Nothing in Files, Commits or the viewer writes to disk, to the index or to a ref: no edit, rename, delete, stage, commit, checkout, reset, revert or cherry-pick. Writing code belongs to Claude Code and to the user's editor; the two git actions Acta already offers (pull, switch branch) stay on the folder row (components-folders.md 5). A read-only view can be glanced at during a running session without any risk of a conflict with what Claude Code is writing.
3. **One viewer tab, reused.** Opening a file or a diff shows it in the focused group's **file viewer tab**, created on first use and reused after; its back and forward history walks the files viewed in it (4.4). Tabs therefore never pile up, and a second viewer exists only when the user moves the tab to another group (4.8.8). Viewer tabs are not saved in `preferences.json` (4.7 forbids paths there): they are gone after a restart.
4. **Plain text, no syntax colours** in v1, as code blocks are (3.3): a syntax palette would add some ten colour tokens, each to be measured in two themes against `bg.base`, for a viewer that is read for minutes, not edited. Markdown files render as documents, since the renderer exists and READMEs and specs are what people open most. Diffs get two tints, the only new tokens (5.R.9).
5. **A calm git state.** One trailing letter per changed file in `text.tertiary` (only a conflict takes `warning`), no coloured file names, nothing on folders except a fully untracked one. "Show changed files only" is the way to see where the changes are.
6. **One graph colour.** Lanes and nodes are `text.tertiary`; the first-parent chain of HEAD is `accent`, because it is "where you are" (principle 2). Shape, not colour, tells a merge (ring) from a commit (dot) and HEAD (ring with a dot).
7. **The user's own files leave Acta freely.** These are the repository's files, already plaintext on disk and not Acta's encrypted data, so "Open in default app" and "Show in Explorer" are offered (unlike attachments, ADR 0012, 7). Acta never runs a program: executables are only shown in their folder.
8. **Claude Code gets a mention, not a new flow.** "Mention in Claude Code" types `@path` at the prompt of the page's session, without sending it; the user finishes the sentence (5.R.8).

### 5.R.2 Where they live, and which folder they show

| View | Key | Icon (Lucide) | Default place | Min width x height | Shortcut |
|---|---|---|---|---|---|
| Files | `files` | `folder-tree` (new icon) | Left sidebar, second tab after Projects, hidden | 200 x 160 | `Mod+Shift+E` |
| Commits | `commits` | `git-graph` (new icon) | Bottom panel, hidden | 400 x 160 | `Mod+Shift+G` |

Both are hidden on a new install and after Reset layout. **They have no titlebar toggle** (lead decision of 2026-11-22: thirteen icons would be too many, clarity review finding 14): they are shown and hidden by their key, the palette ("Toggle files", "Files: Show"...), the Layout menu, which lists every view with a `check` when visible and is always in the titlebar, and the folder row menus' "Browse files" and "Show commits" (components-folders.md 5). That keeps rule 1 of 4.8.11: the Layout menu is their visible way back. A history is wide and short, so the bottom panel suits it; a file tree is narrow and tall, so the left sidebar does. Both can go anywhere (4.8.7). On `bg.sunken` in a side area or the bottom panel, on `bg.base` as a tab in the page area (with the view strip of 4.8.3).

**The page they follow.** The views show the folders of the **followed page**: the active tab of the focused group when it is a topic, epic or project page; a document or memory page follows its owner; any other page (settings, trash, shortcuts, the inbox, the knowledge base) leaves the followed page unchanged, so opening Settings does not empty the tree. At start, before any record page was shown, the followed page is the active project's page.

**The folder shown.** By default the page's **active folder** (components-folders.md 5), and it follows that choice when it changes. The **folder chooser** in the header context (or the view strip) picks another of the folders the page sees, for this view and this page only: it never changes the active folder, so it never changes where Claude Code starts. Choosing the page's active folder again returns to following it. The choice lasts until Acta quits (a folder record id per page, in memory; not saved). Files and Commits keep their own choice, so Files can show `acta-docs` while Commits shows `embed-terminal`.

- **Chooser** (the switcher's anatomy, components-folders.md 5: ghost button 24 high, padding 0 x 4, `radius.sm`, gap 4, the folder's icon 14, its name in `type.small` `text.secondary`, `chevron-down` 12 in `text.tertiary`; `type.small` and icon 14 because it sits in a header among `type.label`). Tooltip, one fact per line: the full path (`type.code`) and "Folders of topic Embed the terminal" (`files.of.*`). A click, `Space` or `Alt+Down` opens a dropdown menu (5, Context menu): the folders the page sees in list order, grouped as the switcher groups them ("This topic", "From epic Terminal", "From project Acta"), each with its icon and name, the word "Active" in the shortcut slot of the page's active folder and a trailing `check` on the one shown; a folder that was not found is listed with `folder-x` and cannot be chosen (`text.disabled`). After a separator, "Show working folders" unfolds the page's folder row. Type-ahead selects by name.
- **Header** (4.8.4): the view's name, then the chooser; buttons: `ellipsis` (the view menu, with the view's own items above the Move items, after a separator) and `minus`. No other buttons at rest: the filter and the toggles are keys and menu items.
- **Per folder, for the session:** the tree's expanded folders, scroll position and cursor, and the history's selection and filters, are kept per folder in memory, so going back and forth between two topics that share a folder finds the tree as it was. Nothing of it is saved (paths and names are not allowed in `preferences.json`).

**Nothing to show** (one line at the top left, padding 12, `type.small` `text.tertiary`, as an empty area, with the one action after it as a ghost button):
- No record page yet: "Open a project, epic or topic to see its files." (`files.no_page`); for Commits "... to see its commits." (`commits.no_page`).
- The page sees no folder: "No working folder here." (`files.no_folder`) and "Set working folder..." (`folders.set`), the same command as the palette's.
- The folder shown was not found: `folder-x` 14 in `warning`, "embed-terminal was not found on this computer." (`files.not_found`) and "Locate..." (`folders.locate`), the folder row's repair.

### 5.R.3 Files view

**Anatomy, top to bottom**: the header (or view strip); the **filter field** when open (below); the tree, virtualised, padding 8 as the Projects view (4.2.1). The root is the folder itself: it is not a row, the chooser names it, and its children start at depth 0.

**Tree row** (the tree row of components.md 5, 28 high, `radius.sm`, inset 8, `type.ui`): indent 16 per depth; leading slot 16 holding the chevron (`chevron-right` / `chevron-down`) on a folder, empty on a file; 4; the kind icon 16 in `text.secondary` (`folder` / `folder-open` when expanded, `file` for every file, `folder-symlink` / `file-symlink` for links (new icons)); 8; the name in `text.secondary` at rest, `text.primary` on hover and selection (as Projects); and the trailing slot: the git mark (below). Names truncate at the end with `…`; the full relative path is the tooltip (5, Tooltip). Sorting: folders first, then files, each in natural order ignoring case ("file2" before "file10"), as the system file managers sort. Dotfiles and dot-folders are listed (`.github`, `.claude` matter); `.git` never is.

**Indent guides.** Unlike Projects (4.2.1 leaves them out because the tree has three levels), a folder nests without limit: each expanded folder draws a 1px `border.subtle` vertical line at the centre of its chevron, from under its row to its last visible descendant. Guides are separators, not controls (1.1 to 1.7 against their surface, 2.2), and are never highlighted.

**Git mark** (only in a git repository, from `git status`, 5.R.10). One letter in `type.small`, `tnum`, in a 12 slot at the row's right end, `text.tertiary` (`text.secondary` on a selected row, 2.2): **M** modified, **A** added to the index, **U** untracked, **R** renamed, **C** conflict (the letter in `warning`). The letters are git's and VS Code's, the same in French; the tooltip says the word ("Modified", "Untracked", "Renamed from src/old.rs", "Conflict"; `files.status.*`) and the row's accessible name ends with it, so colour never carries the state (only C has one, and its letter carries it too). A folder gets a mark only when git reports it whole (an untracked folder: **U**); folders that merely contain changes get none, which is what keeps a large change set from lighting up every ancestor. Deleted files do not exist on disk and are not in the tree; they appear in "Show changed files only" and in Commits, Uncommitted changes. A file marked ignored is not in the tree unless "Show ignored files" is on, and then its name is `text.tertiary` with no mark.

**Hidden by default: ignored files.** In a git repository, what `.gitignore`, `.git/info/exclude` and the global excludes file ignore is not listed (`node_modules`, `target`): matched in Rust by the `ignore` crate, per directory as it is read, with no process. **Show ignored files** (view menu, palette; a `check` when on) lists them as above; saved as `files.show_ignored` in `preferences.json` (a setting, no content). Outside a git repository nothing is hidden and the item is absent. Known limit: a file git tracks although an ignore rule matches it (added before the rule, or with `add -f`) is hidden like an ignored one, since the rules are matched without asking git; "Show ignored files" lists it.

**Show changed files only** (view menu, palette): the tree keeps only changed files and the folders that hold them, all of them expanded, deleted files included (name in `text.tertiary`, mark **D**, `Enter` shows the diff since they have no content). With no change: "No changes in embed-terminal." (`files.no_changes`), `type.small` `text.tertiary`. Off again, the tree comes back as it was. Session only.

**Lazy loading.** A folder is read when it is expanded (one `read_dir`, off the UI thread, nothing held open, ADR 0008); its children appear in the next frame after the answer. Pending after 400 ms, one row at the children's depth reads "Loading..." (`files.loading`, `type.small` `text.tertiary`); an empty folder shows one row "Empty" (`files.empty_folder`, same style), as Projects writes under an empty container; a folder that cannot be read shows `triangle-alert` 14 in `danger` and "Could not read this folder." (`files.read_failed`). A folder of 50,000 entries is one virtualised list: nothing is capped while browsing. **Links** to folders (symlinks, Windows junctions) expand only when their target stays inside the repository's worktree (the folder itself outside a repository); one that leads out shows "Link to outside the folder." (`files.link_outside`, `type.small` `text.tertiary`) as its only child and is never read. A file reached through a link leading out is not opened in the viewer (the same line as its empty state), and anything that is not a regular file (a FIFO, a device) is never opened (`viewer.binary`).

**Staying current** without a watcher (ADR 0008 holds nothing open on a working folder): while the view is visible and the window focused, the expanded folders are read again every 2 s and when the window regains focus; a new file appears in place, a removed one goes, and the rows above the cursor never move it: the scroll is anchored on the row under the cursor (principle 6). The git marks are read again when the window regains focus, when the view is shown and 1 s after a re-read finds a change, at most once every 5 s. "Refresh" (view menu, `F5` in the view) does both at once.

**Filter by name** (the keyboard path to a file). Typing a printable character with the tree focused, or `Mod+F` in the view, opens the filter field at the top (an input 28 high, inset 8, `type.ui`, placeholder "Filter by name" (`files.filter_placeholder`), the typed character in it). While it holds text, the tree is replaced by a flat list of matches across the whole folder: a walk off the UI thread that respects the ignore rules and "Show ignored files", never follows a link to a folder, stops after 100,000 entries (then one line: "Searched the first 100,000 files.", `files.search_capped`) and is kept for the session, refreshed when a re-read finds a change. Matching is fuzzy on the relative path, name matches first, then shorter paths. Each result is a list row 28: `file` 16, the name with matched characters in `type.ui-strong`, and after 8 the parent path in `type.small` `text.tertiary`, middle-truncated by segments (components-folders.md 5, Path display); the git mark at the right. At most 200 rows, then one line "200 of 1,240 matches. Keep typing to narrow." (`files.matches_more`); none: "No file matches "rsize"." (`files.no_match`). `Down` moves from the field into the results, `Up` from the first result back to the field; `Enter` opens the result; `Esc` clears and closes the field and puts the cursor back on the tree row it was on (or, after a result was opened, reveals that file in the tree). "Go to file..." in the palette is the same search from anywhere (5.R.11).

**Opening.** A click on a file, or `Enter`, opens it in the viewer (5.R.4); a click on a folder or its chevron, `Enter`, `Right` or `Left` folds and unfolds it (`Right` on an open folder goes to its first child, `Left` on a child to its folder), as in the Projects tree. Moving the cursor opens nothing: arrowing through a folder never reads its files. A double click is the same as a click.

**Keys in the tree.** `Up` / `Down` (not `K` / `J`: a letter starts the filter here), `Home` / `End`, `PageUp` / `PageDown`, `Left` / `Right` as above, `Enter` open, `Mod+Enter` open in a new viewer tab beside the current one (the one case that adds a viewer tab), `Shift+Enter` "Mention in Claude Code", `Mod+C` copy the full path, `Mod+Shift+C` copy the path relative to the folder, `F5` refresh, `Mod+F` filter, `Shift+F10` or the menu key the row menu. `Delete`, `F2` and `E` do nothing (read-only).

**Row menu** (right click, `Shift+F10`): on a file, "Open" (`common.open`), "Open in default app" (`files.open_default`), "Show in Explorer" (Reveal in Finder, Show in file manager; `folders.show.*`, the file selected in its folder), separator, "Mention in Claude Code" (`files.mention`), "Show changes" (changed files only; `files.show_changes`), "Show file history" (in a repository; `files.history`), separator, "Copy path", "Copy relative path" (`files.copy_relative`). On a folder: "Show in Explorer", "Mention in Claude Code", "Show folder history", "Copy path", "Copy relative path".

**Never run a program.** "Open in default app" hands the file to the system's opener (ShellExecute `open`, `open`, `xdg-open`), which for a program means running it. It is disabled, with the tooltip "Acta does not run programs." (`files.no_run`), for: on Windows, the extensions of `PATHEXT` plus `.msi`, `.lnk`, `.scr`, `.ps1`, `.vbs`, `.wsf`, `.hta`, `.reg`, `.url`, `.jar`, `.cpl`, `.pif`; on macOS, `.app`, `.command`, `.pkg`, `.terminal`, `.tool` and any file with an execute bit; on Linux, `.desktop`, `.AppImage` and any file with an execute bit. "Show in Explorer" stays.

### 5.R.4 File viewer tab

A page-area tab (4.8.8) that is neither a record page nor a view: no breadcrumb part of its own (the breadcrumb keeps the last page), no unread dot, its own back and forward history of what it showed. Closing it (`Mod+W`, its `x`) closes it; it is created again on the next open.

- **Tab**: icon 14 (`file`, or `file-diff` for a diff, new icon) and the file name; a diff adds " · a1b2c3d" (the short hash in `type.code`) or " · changes" for uncommitted changes (`diff.tab_uncommitted`). Tooltip: the full path.
- **Where it opens**: in the focused group. If the view that asked is itself the active tab of the focused group (Files or Commits moved into the page area), the viewer goes to the next group; with only one group, it splits right when both groups keep 400 (4.8.8), otherwise it becomes the tab after the view in the same group. The viewer never covers the view it was opened from.
- **Strip** (the view strip of 4.8.3: 28, `bg.sunken`, bottom border `border.subtle`, padding 0 x 8, gap 8): the folder's name in `type.small` `text.secondary` and the relative path in `type.code` at 12 / 16 (mono among `type.small`, 3.2) in `text.tertiary`, middle-truncated by segments; then the facts in `type.small` `text.tertiary`, `tnum`: "214 lines · 7.9 KB" (a text file), "3024 x 4032 · 3.4 MB" (an image); then, for a changed file, the mark's word ("Modified") and a ghost button "Show changes"; at the right, for a Markdown file a segmented control "Rendered" / "Source" (compact, 24), the compact ghost icon button `history` ("Show file history"), and `ellipsis` (the file's row menu of 5.R.3, plus "Reveal in Files", `files.reveal`). Under 520 of group width the facts drop, then the folder name.
- **Text** (UTF-8 with no NUL byte in the first 8 KB, ADR 0012's test, a BOM dropped): on `bg.base`, `type.code` `text.primary`, line height 20, no wrapping, both scrollbars. A gutter of line numbers in `type.code` `text.tertiary`, `tnum`, right-aligned, as wide as the largest number, padding 0 12 0 16; the text starts 16 after it. Tabs are 4 columns wide; `\r\n` reads as one line break. Text is selectable: drag, `Shift` with arrows, `Mod+A`, `Mod+C`. The cursor is not shown: there is nothing to type.
- **Markdown** (`.md`, `.markdown`): rendered by default with the document setting of 3.3 (the content column, `type.document`), with its limits (no images, no HTML); a relative link to another file of the folder opens it in this viewer, a web link opens the browser. "Source" shows it as text (above). The choice is per tab, not saved.
- **Image** (PNG, JPEG, GIF first frame, WebP, BMP, decoded in memory as attachments are; at most 50 MB and 100 megapixels): the attachments viewer's body (components-attachments.md 5, Viewer, Image): centred on `bg.sunken`, fitted, never above 100%, `0` or a double click toggles "Fit" / "Actual size" (`attach.fit`, `attach.actual`), `Shift` with arrows and the wheel pan. SVG is text.
- **Anything else** (binary, not UTF-8, larger than the limits): an empty state (5) centred in the tab: `file` 16 in `text.tertiary`, the name in `type.ui-strong`, one line saying why in `type.small` `text.secondary` ("This file is not text Acta can show." `viewer.binary`; "This file is not UTF-8 text." `viewer.not_utf8`; "48 MB is too large to show here." `viewer.too_large`), then the secondary button "Open in default app" (disabled for a program, with its tooltip) and the ghost button "Show in Explorer".
- **Not downloaded** (a partial clone without the file's old version: git never fetches for a read): in a diff, the empty state's line reads "Not downloaded in this clone." (`diff.not_downloaded`).
- **Large text**: the first 1 MB is shown; a Notice (5) at the bottom of the text, `info` 14, "Showing the first 1 MB." (`viewer.truncated`) with the ghost action "Open in default app".
- **Find** (`Mod+F` in the viewer, or "Find in file" in the palette): an input 24 high (compact) takes the place of the strip's facts, placeholder "Find" (`viewer.find`), then "3 of 12" (`attach.position`, `type.small` `text.tertiary` `tnum`) or "No results" (`viewer.find_none`). Matches are case-insensitive plain text; every match takes `selection`, the current one `accent.subtle` (both under `text.primary` only, 2.2); `Enter` / `Shift+Enter` next and previous, scrolling by the least amount; `Esc` closes the field and keeps the scroll. In rendered Markdown, find works on the rendered text.
- **Staying current**: while the tab is visible and the window focused, the file's size and modification time are checked every 2 s and on focus; when they change, the text is read again in place, the first visible line kept at the top (principle 6), nothing highlighted, no toast. A file that disappears keeps its last content under a Notice at the top, `triangle-alert` 14 in `warning`: "This file was deleted or moved." (`viewer.gone`). A read that fails: the empty state with "Could not read embed.rs. {reason}" (`viewer.read_failed`) and "Try again" (`common.try_again`).
- **Keys**: `Alt+Left` / `Alt+Right` (`Cmd+[` / `Cmd+]`) back and forward in the tab's history; `Mod+F` find; `Mod+A`, `Mod+C`; `Up` / `Down`, `PageUp` / `PageDown`, `Home` / `End` (`Mod+Home` / `Mod+End` to the first and last line) scroll; `Shift+Enter` "Mention in Claude Code"; `Shift+F10` the strip's `ellipsis` menu.

### 5.R.5 Commits view

**Layout.** A filter row, then the commit list; and the **details pane** for the commit under the cursor:
- **Wide** (view 720 or more): the details pane on the right, 40% of the view, at least 300 and at most 520, separated by a resize handle (5) whose width is saved as `commits.details` (a number, 4.7). Double click on the handle: 40%.
- **Narrow** (under 720, e.g. in a sidebar): the list alone; `Enter` or a click on a commit replaces it with the details pane, headed by a 28 row with a ghost `chevron-left` button "Commits" (`view.commits`); `Esc`, `Backspace` or that button returns to the list with the cursor where it was.

**Filter row** (32 high: padding 4 x 8, gap 8, bottom border `border.subtle`):
- **Branch chooser**: a ghost button 24 high, `git-branch` 14, "All branches" (`commits.all_branches`, the default), "Current branch (main)" (`commits.current_branch`) or a branch's name, `chevron-down` 12. Its menu: "All branches", "Current branch (main)" (or "Current commit" when HEAD is detached), separator, the groups "Branches" and "Remote branches" (`folders.git.local`, `folders.git.remote`), type-ahead by name; a trailing `check` on the current choice. "All branches" means local branches, remote-tracking branches, tags and HEAD (`--branches --remotes --tags HEAD`), never the stash.
- **Search field** (input 24 high, flexible, at least 160, `type.ui`, placeholder "Filter by message, author or hash", `commits.filter_placeholder`). Typing filters after 250 ms of quiet; `Esc` clears it, a second `Esc` returns focus to the list. A hexadecimal query of 4 to 40 characters is also looked up as a commit: when it names one, that commit is listed first.
- **Path chip** (only with a path filter; height 24, padding 0 x 4 0 8, `radius.sm`, 1px `border.default`, `file` 14 or `folder` 14, the relative path in `type.code` at 12 / 16 in `text.secondary`, middle-truncated at 240, then a ghost `x` 20 "Clear file filter" (`commits.clear_path`)); tooltip "Only commits that change src/term.rs" (`commits.path_tip`). Set by "Show file history" and "Show folder history". When the folder shown is a subfolder of its repository, Commits opens with the chip set to that subfolder, so the history is the folder's; clearing it shows the whole repository's.

**Commit row** (28 high, the list row of components.md 5: inset 8 as the tree, `radius.sm`, its hover and selection fill under every column, the graph included). Columns, gap 8:
1. **Graph** (5.R.6), as wide as the lanes of the rows on screen need, at most 12 lanes.
2. **Refs**, then the **subject** in `type.ui` `text.primary` (flexible, at least 160, truncated at the end with `…`).
3. **Author** name, 128, `type.small` `text.tertiary`, truncated at the end.
4. **Date**, 96, `type.small` `text.tertiary`, `tnum`, the short relative form of 10.2 up to 7 days ("2 h ago", "yesterday"), then the date ("12 Nov 2026"); the tooltip has the full date and time.
5. **Hash**, 64, the first 7 characters in `type.code` at 12 / 16 (mono among `type.small`, 3.2) in `text.tertiary`, `tnum`.

On a selected row every `text.tertiary` becomes `text.secondary` (2.2). Under 640 of list width the hash drops; under 520 the author drops too; the date stays. No column headers: the contents name themselves.

**Refs** (before the subject, gap 4, together at most 240 wide; those that do not fit fold into a last chip "+3" whose tooltip lists them all). A ref chip: 20 high, padding 0 x 4, gap 4, `radius.xs`, 1px `border.default`, an icon 12 in `text.tertiary` and the name in `type.small` `text.secondary`: `git-branch` for a local branch, `cloud` for a remote-tracking branch (new icon), `tag` for a tag (new icon). The branch HEAD is on (or "HEAD" when detached) is a chip on `fill.selected` with its name in `text.primary`: tone, not accent. A local branch and its upstream at the same commit share one chip, "main" followed by `cloud` 12, tooltip "main and origin/main" (`commits.with_upstream`). Ref names are never translated.

**Uncommitted changes.** When the working tree has changes (5.R.10), a first row, above every commit, reads "Uncommitted changes" (`commits.uncommitted`) in `type.ui` `text.secondary` with "3" (`tnum`, `type.small` `text.tertiary`) in the author column's place; its graph node is `circle-dashed` 12 in `text.tertiary` on HEAD's lane, joined to HEAD's node by a `text.tertiary` line. It is absent under a branch filter that excludes HEAD and under a text filter.

**The cursor.** On open, the cursor is on HEAD's commit, scrolled into view, and the details pane shows it. `Up` / `Down` (or `K` / `J`) move it and the details follow at once from cache, otherwise after 150 ms of rest, so holding a key does not start a `git` per row. `Home` / `End`, `PageUp` / `PageDown`. `Enter` moves focus to the details pane's file list (narrow: opens the details). `Mod+C` copies the full hash. `Shift+Enter` "Mention in Claude Code" (inserts `commit a1b2c3d4e5f6 `, 12 characters). `Mod+F` or `/` focuses the search field. `Shift+F10` the row menu: "Copy hash" (`commits.copy_hash`), "Copy subject" (`commits.copy_subject`), "Mention in Claude Code", "Show only this branch" (on a row with a branch chip; `commits.only_branch`).

**Details pane** (padding 12, on the view's surface, scrolls as one):
- The subject in `type.ui-strong` `text.primary`, wrapping; 8 below, the body in `type.ui` `text.primary`, line breaks kept, wrapped at the pane, selectable, never rendered as Markdown (a commit message is plain text). Absent when the message has no body.
- 12 below, the facts, one per line in `type.small`: the author's name in `text.secondary` and " · 22 Nov 2026, 14:05" in `text.tertiary`; "Committed by Claude Code" (`commits.committed_by`) when the committer differs from the author; the full hash in `type.code` at 12 / 16 in `text.tertiary` with a compact ghost icon button `copy` 14 ("Copy hash"; `common.copied` as its tooltip for 1.5 s after); "Parents" (`commits.parents`) and each parent's short hash as a link (`accent`) that moves the cursor to that commit, loading pages until it is found; the refs as chips.
- 16 below, the files header: "3 files changed" (`commits.files_changed`) in `type.label` `text.secondary`, then "+42 −7" in `type.small` `text.tertiary` `tnum` (U+2212 for the minus). For a merge, one line under it: "Compared with the first parent, a1b2c3d." (`diff.first_parent`).
- The changed files, one list row 28 each (inset as the list): the mark letter (5.R.3: M, A, D, R; a type change reads M, a copy A) in a 12 slot, the file name in `type.ui` `text.primary`, 8, its folder path in `type.small` `text.tertiary` middle-truncated, and at the right "+12 −3" in `type.small` `text.tertiary` `tnum` ("binary" for a binary file, `diff.binary_short`). A rename's tooltip: "Renamed from src/old.rs". `Up` / `Down` move, `Enter` or a click opens its diff in the viewer (5.R.7), `Esc` returns to the commit list. All files are listed (virtualised).
- While the details load (after 400 ms): "Loading..." in `type.small` `text.tertiary` in place of the files. A failure: `triangle-alert` 14 in `danger` and the message (below), with Retry.

**Loading and paging.** Commits are read in pages of 1,000 (5.R.10). Before the first page, after 400 ms: one line "Loading commits..." (`commits.loading`). The next page is read when the list comes within 200 rows of its end; the list grows under the reader without moving the rows on screen. A page that fails ends the list with a row "Could not load more commits." (`commits.more_failed`) and Retry. At most 100,000 commits are held; past that, a last row reads "Showing the first 100,000 commits. Filter by branch or file to see older ones." (`commits.capped`).

**Staying current.** While the view is visible and the window focused, the refs and HEAD are read every 5 s and on focus (one `for-each-ref`, 5.R.10). When they changed (a commit by Claude Code, a pull, a branch switch), the first page is read again, new rows appear above, the rows on screen keep their position (the scroll is anchored on the row under the cursor, principle 6), the cursor stays on its commit while it exists, else moves to HEAD's. No toast, no highlight. The Uncommitted changes row follows the git state of the folder (5.R.10).

**Filters** combine: a branch, a text, a path. With a text filter the graph column is hidden (a filtered list cannot draw true lines) and the rows start with the refs; with a branch or a path filter the graph stays, drawn from the rewritten parents git gives (`--parents`), so it is the true graph of what is shown. Renames are not followed in v1 (git's `--follow` gives no graph). No match: "No commits match." (`commits.no_match`) with the ghost action "Clear filters" (`commits.clear_filters`).

**States** (one line, padding 12, `type.small` `text.secondary`, with its one action as a ghost button):
- Not a repository: "embed-terminal is not a git repository." (`commits.not_repo`); Files still works.
- No commits yet: "No commits yet." (`commits.empty`); Uncommitted changes still shows when there are changes.
- No git: "Git is not installed on this computer." (`folders.git.err.no_git`) and "Open settings" (`cmd.settings`, at Settings > Claude Code, Ready for Claude Code).
- Timed out: `triangle-alert` 14 in `danger`, "Git took longer than 10 s to read the history." (`commits.timed_out`) and Retry (`common.retry`).
- Other failure: "Could not read the history. {message}" (`commits.failed`, `{message}` git's last line, untranslated) and Retry.
- Shallow clone: the oldest commits' lines end at the row's lower edge in a 4 stub; nothing is written about it.

### 5.R.6 The commit graph

Computed in Rust (acta-core, 5.R.10) from commits in topological order and their parents; drawn per row, so a row needs only its own lane data.

- **Geometry**: lanes 12 apart, the first lane's centre 10 from the row's left edge; the column is `8 + 12 x lanes` wide, at most 12 lanes (152). Lines are 1.5 wide (the Lucide stroke, 4.1), round caps. A node sits at the row's vertical centre (14).
- **Nodes**: a commit is a filled circle 8 in its lane's colour; a merge (two parents or more) a ring 8, stroke 1.5, its inside the row's surface; HEAD's commit a ring 10 with a filled centre 4, in `accent` (the shape of `circle-dot`, the active mark elsewhere). Uncommitted changes: `circle-dashed` 12 (5.R.5). The graph is the fourth place circles are allowed (layout.md 4.1).
- **Lines**: a lane that passes a row is a vertical line through it. A line that joins a node from another lane bends in the half of the row that touches the node: from the top edge at its own lane to the node (a merge parent's line coming in, a branch's first commit), or from the node to the bottom edge at its new lane (a merge commit's second parent going out). A straight segment, never a curve; lanes never shift sideways between nodes, so the eye follows a vertical.
- **Colour**: `text.tertiary` for every lane and node, except HEAD's first-parent chain (HEAD and each first parent down from it) and the segments between those commits, in `accent`. On a selected row the colours stay (both measure 3.0 or more on `fill.selected`, 5.R.9). One accent line, at most, on screen: where the user is.
- **Beyond 12 lanes** the column is clipped at its edge; a node on a clipped lane is drawn on the twelfth, as a ring.
- **Accessible name** of a row: kind ("Merge commit" `commits.merge`, or "Commit"), short hash, subject, author, date, then its refs: "Merge commit, a1b2c3d, Merge branch resize, Alexandre, 2 h ago, main, origin/main".

### 5.R.7 Diff in the viewer

A diff opens in the file viewer tab (5.R.4), from a changed file in the details pane, from "Show changes" on a changed file (working tree against HEAD), or from a deleted file in Files.

- **Strip**: the path (as 5.R.4), then the change in `type.small` `text.tertiary`: "Modified", "Added", "Deleted", "Renamed from src/old.rs" (`files.status.*`) and "+12 −3" (`tnum`); at the right, the compact ghost icon buttons `chevron-up` / `chevron-down` "Previous change" / "Next change" (`diff.prev_change`, `diff.next_change`; `Shift+F7` / `F7`), `arrow-up` / `arrow-down` "Previous file" / "Next file" (`attach.prev`, `attach.next`; `Alt+Up` / `Alt+Down`, through the commit's changed files), "Open current version" (a ghost button, `diff.open_current`, when the file exists in the working tree; it opens the file in this tab, so Back returns to the diff), `ellipsis`.
- **Body**, unified (one column), on `bg.base`, `type.code`, no wrapping: two line-number gutters (old, new; `type.code` `text.tertiary` `tnum`, padding 0 8), then a 16 sign column, then the text. An added line: its sign `+` in `success` and its sign column and text on `diff.added`; a removed line: `−` in `danger` on `diff.removed`; a context line: no sign, no tint. The gutters are never tinted (`text.tertiary` is not allowed on a tint, 2.2). The sign carries the kind; the tint only helps the eye.
- **Hunks**: three lines of context; each hunk starts with a header row 28 on `bg.sunken`: git's `@@ -12,7 +12,9 @@` and the function line after it, in `type.code` `text.tertiary`. Between hunks, nothing else. `F7` / `Shift+F7` move to the next / previous hunk, scrolling it to the top.
- **Binary**: "Binary file, 12 KB to 14 KB." (`diff.binary`; added or deleted: "Binary file, 14 KB." `diff.binary_one`). **Images** (the image kinds of 5.R.4): "Before" and "After" (`diff.before`, `diff.after`, `type.label` `text.secondary`) side by side, each fitted on `bg.sunken`, read from git into memory (`git cat-file`), never written to disk; stacked under 560 of group width.
- **Mode only** (a permission change): "No changes in content." (`diff.no_content`) and the old and new mode in `type.code`.
- **Large**: at most 5,000 lines of diff per file (and 2 MB of git output); then a Notice: "Showing the first 5,000 lines of this diff." (`diff.truncated`).
- **Not in v1**: side by side, word-level highlight, expanding the unchanged lines between hunks, whitespace options.

### 5.R.8 Mention in Claude Code

On a file or folder (Files, the viewer) or a commit (Commits): `Shift+Enter`, the row menu, the palette.

- **The session**: the Claude Code session the terminal shows for the followed page (4.5; with page-bound sessions, the session bound to that page). The text inserted: `@` and the path relative to the session's working folder when the file is inside it, else the absolute path; a path with a space in it is wrapped in double quotes; a commit inserts `commit ` and 12 characters of its hash. A space follows; nothing is sent: no `Enter`, so a busy session queues nothing.
- **Running**: the text goes into the terminal as a bracketed paste (as `Ctrl+Shift+V` pastes), the terminal view is shown and focused (the toggle rule, 4.8.5), the cursor at the end of the text.
- **Not running**: the session starts as "Toggle terminal" starts it (components-folders.md 5, Terminal launch), and the text is pasted when the session first reports its prompt ready through presence (ADR 0005, data.md request 2: the first Idle). If that has not happened within 20 s, the text is put on the clipboard instead and a toast says "Claude Code was not ready in time. The mention is copied." (`mention.timeout`, `info`).
- The path reaches Claude Code only as terminal input, never on a command line (ADR 0011 addendum), and only when the user asks.

### 5.R.9 Tokens and contrast

Two new colour tokens, pending: they join 2.1 and `theme.rs` in the commit that builds the diff (2.1 is parsed by tests; adding the rows before the code would fail them).

| Token | Dark | Light | Used for |
|---|---|---|---|
| `diff.added` | `success` @ 12% | same rule | Added lines in a diff (sign column and text) |
| `diff.removed` | `danger` @ 12% | same rule | Removed lines in a diff (sign column and text) |

Measured (WCAG 2.x, the tint composited over `bg.base`, rounded to 8 bits as the renderer does), dark / light:

| Pair | Dark | Light | Requirement |
|---|---|---|---|
| `text.primary` on `diff.added` | 10.13 | 10.90 | 4.5 |
| `text.primary` on `diff.removed` | 10.47 | 10.82 | 4.5 |
| `success` sign `+` on `diff.added` | 5.56 | 5.03 | 4.5 (a character) |
| `danger` sign `−` on `diff.removed` | 4.59 | 4.67 | 4.5 (a character) |
| `text.tertiary` on `diff.added` (not used: gutters stay untinted) | 4.67 | 4.81 | not allowed |
| `text.primary` on `selection` (find matches) over `bg.base` | 7.03 | 8.33 | 4.5 |
| `text.primary` on `accent.subtle` (current match) over `bg.base` | 9.33 | 10.31 | 4.5 |
| `text.tertiary` (git mark, graph) on `bg.sunken` / `bg.base` | 6.18 / 5.78 | 5.33 / 5.67 | 4.5 |
| `text.secondary` (mark on a selected row) on `fill.selected` over `bg.sunken` / `bg.base` | 6.41 / 5.93 | 5.81 / 6.17 | 4.5 |
| `warning` (C mark) on `bg.sunken` / on `fill.selected` over `bg.sunken` | 7.45 / 5.96 | 5.52 / 4.70 | 4.5 (a character) |
| `accent` graph line on `fill.selected` over `bg.sunken` / `bg.base` | 5.81 / 5.39 | 4.54 / 4.83 | 3.0 (graphic) |
| `text.tertiary` graph line on `fill.selected` over `bg.sunken` / `bg.base` | 4.94 / 4.59 | 4.52 / 4.83 | 3.0 (graphic) |
| `text.primary` on `fill.selected` (HEAD ref chip) over `bg.sunken` | 10.73 | 10.28 | 4.5 |

At 16% the danger sign would fall to 4.33 in dark and the light tints under 4.5 for the sign too; 12% is the most tint that keeps every sign at AA.

New icons (Lucide, 4.1): `folder-tree`, `git-graph`, `folder-symlink`, `file-symlink`, `file-diff`, `cloud`, `tag`, `history`, `copy`, `arrow-up`, `arrow-down` (those already in the set are reused as they are).

### 5.R.10 Reading the disk and git (contract for acta-core)

Everything runs off the UI thread; every git call is `git --no-optional-locks -C <folder> ...` with `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`, `GIT_PAGER=cat`, `LC_ALL=C`, and `-c core.fsmonitor=false -c core.quotepath=off`, `GIT_NO_LAZY_FETCH=1` (a partial clone never downloads for a read) and `GIT_LITERAL_PATHSPECS=1` (a path is never a glob or pathspec magic), no stdin, killed at its timeout. Nothing writes a lock, the index or a ref. Diffs pass `--no-ext-diff --no-textconv --no-color` so no configured program runs.

| Need | Source | Timeout |
|---|---|---|
| A folder's entries | `std::fs::read_dir`, with the `ignore` crate's matcher for that directory (no process) | none (no process); a pending read shows "Loading..." after 400 ms |
| Search by name | `ignore::WalkBuilder` (hidden files on, follow links off, standard filters unless "Show ignored files"), at most 100,000 entries | 3 s, then the results so far and the capped line |
| Git marks, Uncommitted changes | `status --porcelain=v2 -z --untracked-files=normal` (ADR 0011, 2; the same call feeds the folder row's "3 changed") | 2 s; past it, marks are not shown |
| A page of commits | `log --topo-order --parents -z --format=%H%x1f%P%x1f%an%x1f%cn%x1f%at%x1f%D%x1f%s` with the branch selection, `--max-count=1000 --skip=N`, and for a text filter `-i -F --grep=<q> --author=<q>` (OR); `-- <path>` for a path | 10 s per page |
| Refs and HEAD, for staying current | `for-each-ref --format=%(objectname) %(refname)` and HEAD from the files (`agent::git::inspect`) | 2 s |
| A commit's message and files | `show -s --format=%B` and `diff-tree -r -M --root --numstat -z` (a merge: against its first parent) | 5 s |
| A file's diff | `diff -M -U3 <parent> <commit> -- <path>` (working tree: `diff -M -U3 HEAD -- <path>`, untracked read as added) | 5 s, at most 2 MB read |
| An image's two versions | `cat-file blob <commit>:<path>`, in memory, at most 50 MB | 5 s |

- **Lanes**, streaming, so each page continues the last: keep `lanes: Vec<Option<Oid>>` (the commit each lane waits for). For each commit, its lane is the first that waits for it, else the first free slot, else a new one; every other lane that waits for it ends at its node (an incoming bend) and is freed. Its first parent takes over its lane, unless another lane already waits for that parent (then the line bends out to that lane and this lane is freed); each other parent takes the lane already waiting for it or the first free slot to the right (an outgoing bend). Free slots are reused; lanes never move. Each row stores its node lane, its pass-through lanes and its bends, plus a flag for HEAD's first-parent chain. A parent never reached (shallow clone, filtered out) ends in a stub.
- **When refs change** between pages, the pages already read are dropped and read again from the top (cheap: the first page is what is on screen).
- **Tests** (temporary repositories built by the tests with `git init`, author and committer "Alexandre", fixed dates; never a network): linear history; a branch and a merge; an octopus merge; criss-cross merges; two roots; a detached HEAD; tags and remote-tracking refs on one commit; a page boundary inside a merge (lanes continue across pages exactly as in one page); a path filter on a file changed on two branches; a shallow clone (stub); a commit whose subject holds the separator characters and non-ASCII text; a rename; a binary file; a 6,000-line diff (truncated at 5,000); untracked, modified, added, renamed and conflicted files in `status`; a timeout (a `git` stand-in that sleeps) killed and reported; no `index.lock` and an unchanged index mtime after every call.

### 5.R.11 Palette and keys

Global keys (added to 6.1): `Mod+Shift+E` toggle Files, `Mod+Shift+G` toggle Commits, both with the toggle rule of 4.8.5 (show and focus, hide when focused). With no titlebar toggle (5.R.2), the key, the palette and the Layout menu are the three ways to them. In the terminal and the chat they pass through, like every `Ctrl+Shift+letter` not kept there; the palette reaches both.

Palette commands (exact labels; with a view focused, its commands rank first; each row's path names the followed page and the folder, "Acta / Terminal / Embed the terminal · embed-terminal"):
- Views: "Files: Show", "Files: Hide", "Commits: Show", "Commits: Hide" and the Move commands, from 4.8.7; "Toggle files", "Toggle commits".
- Files: "Go to file..." (the palette turns into the search of 5.R.3 over the folder Files shows, results with their parent path; `Enter` opens in the viewer; works with the view hidden), "Show files of folder..." (the palette lists the chooser's folders), "Show ignored files" / "Hide ignored files", "Show changed files only" / "Show all files", "Collapse all folders", "Refresh files"; on a focused file or the viewer's file: "Open in default app", "Show in Explorer" (per platform), "Copy path", "Copy relative path", "Mention in Claude Code", "Show changes", "Show file history", "Reveal in Files".
- Commits: "Filter commits by branch...", "Show all branches", "Go to current commit", "Show commits of folder...", "Copy commit hash", "Clear file filter".
- Viewer: "Find in file", "Show source" / "Show rendered" (Markdown), "Next change" / "Previous change", "Next changed file" / "Previous changed file" (in a diff).

### 5.R.12 Wireframes

#### 8.40 Files in the left sidebar, a file in the viewer

```
+------------------------------+---------------------------------------------------------+
|[Projects] [Files] embed-t.. v [.][-]| [f] term.rs                                   x | 28 / 32
|------------------------------+---------------------------------------------------------|
|  > .claude                   | embed-terminal  src/term.rs   214 lines · 7.9 KB       |
|  > .github                   |   Modified [Show changes]          [hist] [...]        | 28 strip
|  v src                       |---------------------------------------------------------|
|  |  > pty                    |   1  use std::io;                                        |
|  |    lib.rs                 |   2                                                      |
|  |    term.rs            M   |   3  pub struct Term {                                   |
|  |    resize.rs          U   |   4      cols: u16,                                      |
|    Cargo.toml                |   5      rows: u16,                                      |
|    README.md                 |   6  }                                                   |
|                              |  ...                                                     |
+------------------------------+---------------------------------------------------------+
  260 left sidebar, two view     the file viewer tab, reused; Back returns to the
  tabs; the chooser names the    previous file; text in type.code, gutter text.tertiary
  folder shown (the page's
  active folder by default)
```

Annotations: the guide under `src` is `border.subtle`; `M` and `U` are `text.tertiary` letters in a 12 slot; `target` and `node_modules` are hidden (ignored). The strip's facts drop under 520 of group width. The tab bar shows because the page area now has two tabs (the topic and the viewer).

#### 8.41 Filtering by name

```
+------------------------------+
| Files  embed-terminal v [.][-]|
|------------------------------|
| [rsz                       ] |  28 input, inset 8
|  [f] resize.rs   src       U |  name match first, parent path type.small text.tertiary
|  [f] resize_test.rs  tests   |
|  [f] README.md   docs/rsz    |  path match after
|                              |
+------------------------------+
```

`Esc` clears the field and returns to the tree, revealing the file last opened.

#### 8.42 Commits in the bottom panel, wide, with details

```
+----------------------------------------------------------------------------------------------+
| Commits  embed-terminal v                                                            [.] [-] | 28
|----------------------------------------------------------------------------------------------|
| [All branches v]  [Filter by message, author or hash          ]                              | 32
|------------------------------------------------------------+---------------------------------|
| o     Uncommitted changes          3                        | Fix resize on DPI change        |
| @     [main~] Fix resize on DPI..  Alexandre   2 h ago  a1b2c3d | The grid used logical    |
| |\    Merge branch scrollback      Claude Code 5 h ago  9f8e7d6 | pixels when the window   |
| | *   [scrollback] Keep 10k lines  Claude Code 6 h ago  4c3b2a1 | moved to another screen. |
| * |   Add PTY reader               Alexandre   yesterday 77aa001|                          |
| |/                                                          | Alexandre · 22 Nov 2026, 14:05  |
| *     [v0.5.0] Release 0.5.0       Alexandre  12 Nov 2026 5e6f7a8| a1b2c3d4e5f6...  [copy] |
|                                                            | Parents 9f8e7d6                 |
|                                                            |                                 |
|                                                            | 3 files changed  +42 −7         |
|                                                            |  M term.rs     src     +30 −5   |
|                                                            |  A resize.rs   src     +12      |
|                                                            |  M Cargo.toml          +0 −2    |
+------------------------------------------------------------+---------------------------------+
  list: graph, refs + subject, author 128, date 96, hash 64      details: 40%, 300 to 520,
  (hash then author drop under 640 and 520)                      resize handle between
```

Annotations: `@` is HEAD's node (a ring with a dot) and the vertical line under it, HEAD's first-parent chain, is `accent`; every other lane and node is `text.tertiary`; the merge's node is a ring. `[main~]` is HEAD's branch chip on `fill.selected`, its upstream folded in (the `cloud` icon after the name). `Enter` on a changed file opens its diff in the viewer (8.44). With a path filter, a chip "src/term.rs x" follows the search field.

#### 8.43 Commits in a narrow sidebar, details opened

```
+------------------------------+
| Commits  embed-terminal v [.][-]|
|------------------------------|
| [<] Commits                  |  28 back row; Esc returns
|------------------------------|
| Fix resize on DPI change     |
| The grid used logical...     |
| Alexandre · 22 Nov, 14:05    |
| a1b2c3d4e5f6...      [copy]  |
| 3 files changed  +42 −7      |
|  M term.rs   src    +30 −5   |
|  A resize.rs src    +12      |
+------------------------------+
```

#### 8.44 A diff in the viewer

```
+------------------------------------------------------------------------------------+
| [d] term.rs · a1b2c3d  x |                                                         | 32 tab bar
|------------------------------------------------------------------------------------|
| embed-terminal  src/term.rs   Modified  +30 −5    [^][v] [up][dn] [Open current version] [...] |
|------------------------------------------------------------------------------------|
| @@ -40,7 +40,9 @@ fn resize(&mut self, size: Size)                     (bg.sunken)  |
|  40  40     let cols = size.width / self.cell.width;                               |
|  41      −  let rows = size.height / self.cell.height;           (diff.removed)    |
|      41  +  let scale = self.window.scale_factor();               (diff.added)      |
|      42  +  let rows = (size.height * scale) / self.cell.height;   (diff.added)      |
|  42  43     self.pty.resize(cols, rows)?;                                          |
+------------------------------------------------------------------------------------+
```

The gutters stay on `bg.base`; only the sign column and the text take the tint; `+` is `success`, `−` is `danger`.

### 5.R.13 Not in v1, and why

- **Editing, creating, renaming or deleting files; staging, committing, checking out, resetting, reverting, cherry-picking, stashing.** Acta is the record and Claude Code is the hand; a second place that writes to the working tree is a second source of conflicts with a running session. Pull and branch switching stay on the folder row.
- **Syntax highlighting** (5.R.1, 4). Revisit with evidence that people read long files here; it would take a syntax palette built from the terminal palette's slots (2.4), measured like them.
- **Following renames in a file's history**, side-by-side diffs, blame, word-level diff, a commit search inside file contents (`-S`), a file watcher. Each is a known cost with no request behind it yet.
- **Files in the Projects tree.** The tree holds records; files are another hierarchy with their own view.
