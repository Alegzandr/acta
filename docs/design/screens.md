# Acta design: key screens

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 8. Key screens

Widths are for the default 1440 x 900 window. Every screen exists in both themes; the wireframes show structure, which is the same in both. Sidebars drawn in 8.2, 8.7 and 8.10 are excerpts from inside an expanded project node. `( )` is a status glyph, `[ ]` a control, `·` a separator rendered as 8 of space plus the next item, not a character.

### 8.1 Main window, topic page

```
+------------------------------------------------------------------------------------------------+
| [=] [<] [>]  Acta / Terminal / Embed the terminal             [Search  Ctrl K] [o] [>_] - [] X | 36  titlebar
+----------------------+-------------------------------------------------------------------------+
| v Acta               |                               32                                        |
|     [i] Inbox      4 |        Embed the terminal bound to the project   <- type.title 24/30    |
|     [k] Knowledge    |    12                                                                   |
|     (o) Naming       |        Status    [(.) Active v]            <- properties block: rows 28,|
|   v Core UI          |        Due       15 Dec 2026  In 24 days      optional; "None" if unset  |
|       (v) App shell  |        Epic      Terminal                    label column as wide as its|
|       (.) Sidebar t..|        Folders   > [f] ~\dev\acta-wt\embed-terminal  +1   longest label |
|       ( ) Palette   *|        Id        T-42                      <- type.code, copies on Enter|
|   v Terminal         |        Updated   2 h ago by Claude Code                                 |
|      [(.) Embed th..]|    24                                                                   |
|       (=) Scrollback |        +--------------------------------------------------------+       |
|       (-) Links      |        | Current state                                          |       |
|       > 3 closed     |        | Rendering and input work on Windows. Resize still      |       |
|   > Notes            |        | drops the last row; fix is in progress.                |       |
| > Atlas              |        +--------------------------------------------------------+       |
| > Field notes        |    24   bg.raised, border.subtle, radius.lg, padding 16, type.reading   |
|                      |        Description                                  <- type.label       |
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
  260 sidebar, bg.sunken        1180 page area, bg.base; content column 560 centred (310 each side)
  rows 28, indent 16            [..] around a tree row = fill.selected; * = unread dot (accent, 6)
```

Annotations: one tab is open, so there is no tab bar. The tree follows 4.2.1: it lists every project; "Acta" is expanded because the open page belongs to it, "Atlas" and "Field notes" are folded. `[i]` is `inbox`, `[k]` is `library`, `[t]` `trash-2`, `[s]` `settings`, `[p]` `folder`; in the titlebar `[=]` is the sidebar toggle, `[o]` the chat toggle, `[>_]` the terminal toggle. Tree glyphs: `(o)` Draft, `( )` Planned, `(.)` Active, `(=)` Blocked, `(v)` Done, `(-)` Dropped. The page's location is in the titlebar breadcrumb and nowhere else; its title heads the page. `> [f]` is the folded working-folders row (5, 8.11), here as the value of the Folders property. The composer is one line (28 + padding) until focused, then becomes the multi-line editor. Scrolling: title and properties scroll with the content; nothing is sticky except the "new entries" button. In the status bar, `(v)` is the `circle-check` glyph of the ready state.

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

Annotations: the capture editor is focused when the inbox opens. Newest note first. Card actions appear on hover or focus only (shown on the first card): Make topic `T`, Edit `Enter`, Delete `Delete`. "File with Claude Code" is the single secondary button: it shows the terminal and runs the filing command (`/acta:triage`) in the project's session. The command is typed into that session only when Claude Code sits idle at an empty prompt (nothing printed for 1.5 s, no "esc to interrupt" on screen, the cursor on an empty `>` line); with no live session, one starts on the project's folders with the command as its first prompt; with Claude Code mid-task (or anything that does not read as idle), nothing is typed and the restart confirmation of 6.4 asks before a new session starts on the command; resulting topics arrive as Draft with unread dots, and their notes move to Filed with links. With the Open filter empty the button is disabled. `J` / `K` move between cards.

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
                    |  >_ Terminal: Move to bottom panel                      | 32
                    +---------------------------------------------------------+
                    | Enter open   Ctrl Enter open in new tab       Esc close | 28  footer, type.small
                    +---------------------------------------------------------+
                      bg.overlay, border.default, radius.xl, shadow.overlay, no scrim
```

Annotations: empty query shows "Recent" (the last 5 topics) then the 5 most relevant commands for the current page. Group order is fixed (Topics, Knowledge, Commands, Projects) so results do not jump between keystrokes; within a group, ranking is match quality then recency. `Up` / `Down` wrap; `Tab` narrows to the group of the selected row (adds the scope chip); `Backspace` on an empty input removes the chip.

### 8.4 Terminal tool window beside a topic

```
+------------------------------------------------------------------------------------------------+
| [=] [<] [>]  Acta / Terminal / Embed the terminal          [Search  Ctrl K] [o] [>_ on] - [] X | 36
+----------------------+-------------------------------------+-----------------------------------+
| v Acta               |  32                                 | Terminal on Em.. [g]em..v [r] [-] |
|   v Terminal         |  Embed the terminal bound to        |-----------------------------------|
|      [(.) Embed th..]|  the project                        |  > claude                         |
|                      |  Status   [(.) Active v]            |                                   |
|                      |  Epic     Terminal                  |  * Fixing the resize path in      |
|                      |  Folders > [g] embed-terminal v  +2 |    crates/terminal/src/grid.rs    |
|                      |            [b] embed · 3 changed    |                                   |
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

Annotations: the terminal is a tool window in the right dock (4.5). Its header names what the session is on ("on topic Embed the terminal", truncated here), then the folder switcher with the folder the session runs in (`[g]` is `git-branch`, the automatic icon of a worktree), and holds Restart here `[r]` and hide `[-]`; the titlebar toggle shows `fill.selected` while it is visible. The divider is a resize handle. With the dock open the column is 468 wide, under the 520 threshold: the folder row takes its two-line form, the git state (`[b]` `git-branch` 12, branch, changed count) moving under the switcher (8.11). The session starts in the active folder of the page it was opened from, with the other folders it sees passed along (8.11 shows what the tool window says when there is no folder or the folder is gone); when the session ends, the grid shows one line, "Session ended. Enter to restart.", in `text.tertiary`, and restarting uses the folders of the page open at that moment. The current-state update written by Claude Code appears in the page with the highlight while focus stays in the terminal.

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
  content column 560 centred; everything left-aligned inside it; no logo, no illustration
```

Annotations: the three steps are plain text rows in `text.secondary` (not a checklist to tick, not a wizard); they become available as soon as the project exists and disappear for good once the project has one topic. Choosing a folder creates the project named after the folder, attaches the folder to the project as its working folder (active by default), and opens its inbox with the capture editor focused. Other empty states follow the same pattern at the top of their column: empty project ("No topics yet. Capture notes and let Claude Code file them, or start one yourself." [New topic]), empty epic, empty timeline, no search results.

### 8.6 Topic page, knowledge section expanded

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

Annotations: `[d]` is the `file-text` icon, `[m]` the `pin` icon; `[>]` last written by Claude Code, `(A)` by Alexandre; `*` is the unread dot. Own entries first, then nearest level. On epic and project pages the same section sits under the description and ends with the roll-up row ("31 more in the topics of this epic") instead of, or after, inherited groups; a project has nothing to inherit.

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
|                      |        Goals                                     <- h2, type.subtitle   |
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
  260                           column 560 centred; nothing in the margins; no outline column
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
              520, bg.overlay, radius.xl, padding 24, over scrim; focus starts on Cancel
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
|                    start empty; the file is kept as acta.unreadable-2026-11-20-091200.db       |
|                 8  Start empty                                       <- ghost                  |
+------------------------------------------------------------------------------------------------+
  column 560 centred, left-aligned; no sidebar, no icon, no red; keychain name per platform
```

Annotations: the keychain is named as the platform names it (Windows Credential Manager, macOS Keychain, the Secret Service keyring; the `keychain.*` strings, whose French values carry their article). "Import an export..." leads to the normal import preview, where every row is Created. The sentence about what cannot be done is stated once, plainly, and is not softened.

### 8.10 Knowledge base (project)

```
+----------------------+-------------------------------------------------------------------------+
|  Inbox            4  |                               32                                        |
| [Knowledge        ]  |        Knowledge                                 <- type.title          |
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
  260; Knowledge row selected     column 560 centred; groups widest level first; rows as in 8.6
```

Annotations: the view with two filters set (kind and author). Set filters show their value in `text.primary` weight 600. Group names link to the project, epic or topic. A memory unfolds in place, so the human can read, correct, move or delete what the agent recorded without leaving the list; a document row opens 8.7.

### 8.11 Working folders

The folder row at rest. It is the value of the Folders property, so the label "Folders" stands to its left (not drawn here). The cases:

```
        > [g] embed-terminal v  [b] topic/embed · 3 changed  +2              28  own active folder: no label

        > [r] Acta v  [b] main  +1  from project Acta                        28  inherited active folder: label, a link

        > [x] Acta v  from project Acta  Not found on this computer  [Locate...]   active folder missing

        > [g] embed-terminal v  [b] topic/embed  +2  [x] 1 not found         28  additional missing: launch not blocked

          [f] No working folder. The terminal will ask where to start.   [Set working folder...]
        |chev|<- switcher: icon, name, v ->|12|<- git ->|12|count|12|<- origin ->|12|<- state ->|
```

The same rows when the column is under 520 (the normal case with the terminal docked: 468). The switcher and the count stay on the first line; the rest moves to a second line aligned with the switcher's icon, and the row is 52:

```
        > [g] embed-terminal v  +2                                   52
              [b] topic/embed · 3 changed

        > [x] Acta v                                                 52
              from project Acta  Not found on this computer  [Locate...]
        |<------------------------ 468 ------------------------>|
```

The switcher open (on the folder row or in the terminal header; the same menu):

```
        > [g] embed-terminal v
          +--------------------------------------------+
          | This topic                                 |  group labels: type.small, tertiary
          | [g]  embed-terminal                 Active |  32 per item
          | [k]  Design notes                          |
          | From epic Terminal                         |
          | [p]  alacritty fork                        |
          | From project Acta                          |
          | [r]  Acta                                  |
          | [d]  acta-docs                   Not found |  text.disabled, cannot be chosen
          |--------------------------------------------|
          | Edit folders...                            |
          | Add folder...                              |
          +--------------------------------------------+
            dropdown menu (5): min 200, max 320; type-ahead by name
```

Unfolded on a topic worked in its own git worktree, with a folder of its own for notes, inheriting its epic's crate folder and its project's checkout and docs:

```
        v [g] embed-terminal v   Working folders  5                          28  header
         8  This topic                                         <- type.small, tertiary
            (o) [g] embed-terminal  [b] topic/embed · 3 changed  [c] Active  52  circle-dot, accent; name text.primary
                    ~\dev\acta-wt\embed-terminal                                 path: type.code, tertiary
            ( ) [k] Design notes                                             52  circle on hover/focus only
                    ~\notes\acta\terminal
         8  From epic Terminal
                [p] alacritty fork  [b] acta-patches                         52  another repository: passed
                    ~\dev\alacritty-fork
         8  From project Acta
                [r] Acta  [b] main                                           52
                    ~\dev\acta
                    (i) Another checkout of the same repository as embed-terminal.
                        Not passed to Claude Code here.
                [x] acta-docs   Not found on this computer  [Locate...]      52
                    D:\work\acta-docs
            [+ Add folder...]                                                28  ghost
         8  Claude Code starts in the active folder and may also read and edit the others.
    24
        +--------------------------------------------------------+
        | Current state                                          |
```

Folder settings, unfolded in place under an own folder (here a git worktree in Commit and push mode):

```
            (o) [g] embed-terminal  [b] topic/embed · 3 changed  [c] Active
                    ~\dev\acta-wt\embed-terminal
        12
                    Name   [embed-terminal                ]                  text input 240, placeholder = last segment
         8
                    Icon   [A][f][r][g][<>][b][k][s][w][m][d][t][a]          13 toggles 28, "A" = Automatic, selected
         8
                    Commits by Claude Code                                   type.label, secondary
                    [ Manual | Commit | Commit and push ]                    segmented control, "Commit and push" selected
                    After each commit, Claude Code also pushes to the branch's upstream.
                    Never forced; it stops and tells you if a push is rejected or needs a sign-in.
         8
                    [v] Commit in my name, without Co-Authored-By Claude     checkbox
                    [ ] Also on main                                         checkbox, off
                    Commits use your git identity: Alexandre.                type.small, tertiary
         8
                    Acta writes these rules to .claude/settings.local.json in this folder,
                    kept out of git.                                         type.small, tertiary
```

Unfolded on a topic with nothing of its own:

```
        v [r] Acta v   Working folders  2
         8  This topic
                No folders of its own.                                       type.small, tertiary
         8  From project Acta
            (o) [r] Acta  [b] main                                [c] Active
                    ~\dev\acta
            ( ) [d] acta-docs
                    ~\dev\acta-docs
            [+ Add folder...]
         8  Claude Code starts in the active folder and may also read and edit the others.
```

The terminal pane when there is nothing to start in, and when the active folder is gone:

```
+-----------------------------------+      +-----------------------------------+
| 12                                |      | 12                                |
|  Where should Claude Code start?  |      |  The working folder was not found |  type.heading
|  8                                |      |  8                                |
|  Embed the terminal has no        |      |  Claude Code was not started.     |  type.reading,
|  working folder, and neither has  |      |  This is the active folder of     |  text.secondary
|  its epic or its project.         |      |  topic Embed the terminal, and it |
| 16                                |      |  is not on this computer:         |
|  [v] Save as the working folder   |      |  8                                |
|      of project Acta              |      |  C:\Users\Alexandre\dev\acta      |  type.code,
|  4   If not saved, it is used     |      |                                   |  selectable
|      until Acta quits.            |      | 24                                |
| 24                                |      |  [Locate folder...]  [Try again]  |  primary, secondary
|  [Choose a folder...]             |      | 16                                |
|                                   |      |  Or start in another folder:      |  type.small, secondary
|                                   |      |  [d] acta-docs v                  |  the switcher
+-----------------------------------+      +-----------------------------------+
  terminal surface (bg.sunken dark, bg.raised light), padding 12, text max 480, focus on the primary button
```

Annotations: icons are the folders' own: `[g]` `git-branch` (automatic, a worktree), `[r]` `folder-git-2` (automatic, a repository), `[f]` `folder-open`, `[k]` `book-open`, `[p]` `package`, `[d]` `folder-open` (a plain folder); `[x]` is `folder-x` in `warning`, `[b]` `git-branch` 12 of the git state, `[c]` `git-commit-horizontal` 12 (automatic commits), `(i)` `info` 14, `(o)` `circle-dot` (active), `( )` `circle` (hover or focus only), `[v]` a checked checkbox, `v` after a name the switcher's `chevron-down`. In the first two blocks the rows are alternatives, not a list. The folded row always shows the active folder by name, so "where will Claude Code start" is answered without unfolding, and the switcher changes it without unfolding. Inherited rows are never edited from below (no settings form, no drag); their menu leads to the level that owns them, and "Set as active here" is the one thing a page decides about them. The terminal messages are the empty-state pattern on the terminal's surface, not dialogs.

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
        Due        18 Dec 2026  In 27 days                        <- the epic's own date
        Folders    > [f] ~\dev\acta  +1  from project Acta
        Id         E-3
        Updated    2 h ago by Claude Code
    24  Description
        Everything that runs Claude Code inside Acta.
    32  Knowledge  2                                          [+]
        ...
    32  [Open|Overdue|Next 7 days|All|Closed]  4 topics            [+]   28  toolbar
     8       Title                        Due Entries   Updated   28  header, type.small
        ------------------------------------------------------------    border.subtle
        (.)  Embed the terminal b..  ! 18 Nov      14   2 h ago   32  ! clock-alert, warning
        (=)  Terminal scrollback  *     Today       6   Mon       32  * unread dot
        ( )  Links in terminal ou..       Tue       0   12 Nov    32  due soon: text.secondary
        (o)  Session restore                        1   9 Nov     32  no date: empty cell
        |24 |<- title, fills ->|  |<- 88 ->|<- 56 ->|<- 72 ->|
```

Annotations: the epic's progress is computed (topics Done over topics not Dropped). The drawing is made on Saturday 21 Nov 2026. The Due column and the Overdue and Next 7 days segments are there because topics of this epic have dates; with none, the toolbar reads Open / All / Closed and the column is absent (components-records.md 5). The first topic is open and was due 18 Nov, so it is overdue: icon and text in `warning`, tooltip "Overdue by 3 days, due 18 Nov 2026". The table shows open topics by default, in the tree's order; a project page shows the same table grouped by epic. Under 520 of column width the Entries column goes.

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

### 8.16 Updater window

```
            +--------------------------------------------------+
            | 24                                               |
            |  [A] Acta                         <- mark 24, type.ui-strong
            | 16                                               |
            |  Downloading update...            <- type.ui     |
            | 12                                               |
            |  [==================--------------------------]  |  4 high
            |                                             42%  |  type.small, tnum
            +--------------------------------------------------+
              360 x 132, bg.base, border.default, no caption buttons; same in both themes
```

Annotations: shown only when the main window is not ready 300 ms after launch; closed in the frame the main window opens. "Checking for updates..." has no bar; "Restarting..." has a full one. There is no button.

### 8.17 Workbench with the right sidebar shown (4.8)

```
+------------------------------------------------------------------------------------------------------+
| [<] [>]  Acta / Terminal / Embed the terminal  [Search Ctrl K]  [t][i][l][>_][c]  [L][B][R] [#] - [] X | 36
+----------------------+-------------------------------------+-----------------------------------------+
| Projects     [.] [-] | Embed the terminal bound to the     | [Terminal] Chat   on topic E.. [r][.][-]| 28 headers
|----------------------|  project                            |-----------------------------------------|
| v Acta               |  Status   [(.) Active v]            |  > claude                               |
|    Inbox          4  |  Epic     Terminal                  |                                         |
|    Knowledge         |  ...                                |                                         |
|  v Terminal          |                                     |                                         |
|    [(.) Embed th..]  |                                     |                                         |
|----------------------|                                     |                                         |
|  [t] Trash           |                                     |                                         |
|  [s] Settings        |                                     |                                         |
+----------------------+-------------------------------------+-----------------------------------------+
| [p] Acta   (v) Claude Code: ready                                                           Acta 0.4.1 | 24
+------------------------------------------------------------------------------------------------------+
  260                    page area, min 420                    648 right sidebar
  view toggles: [t] Projects  [i] Inbox  [l] Knowledge  [>_] Terminal  [c] Chat (Windows)
  area toggles: [L] left  [B] bottom  [R] right, then [#] the Layout menu; [L] and [R] fill.selected
```

Annotations: the default layout once the terminal has been shown. Both sidebars carry an area header: "Projects" alone on the left, two view tabs on the right (Terminal active in `fill.selected`, Chat in `text.tertiary`), then the active view's context and buttons, `[.]` the view menu, `[-]` hide the area. In the titlebar the Projects and Terminal toggles are selected; Inbox, Knowledge and Chat are not, and stay visible: they are the way back. Hidden areas and views cost nothing but their 28 toggle.

### 8.18 Dragging the Chat to the bottom of the page area

```
+----------------------+-------------------------------------+-----------------------------------------+
| Projects     [.] [-] | Embed the terminal bound to the     | [Terminal] (Chat) on topic E..[r][.][-] |
|----------------------|  project                            |-----------------------------------------|
| v Acta               |  ...                                |  > claude                               |
|  ...                 |                                     |                                         |
|                      |                                     |                                         |
|                      |  +- - - - - - - - - - - - - - - -+  |                                         |
|                      |  |                               |  |                                         |
|                      |  |  bottom panel, 320, as it     |  |                                         |
|                      |  |  will be after the drop       |  |                                         |
|                      |  |              +--------------+ |  |                                         |
|                      |  |              | [c] Chat     | |  |                                         |
|                      |  +- - - - - - - +--------------+-+  |                                         |
|                      |~~~~~~~~ 24 strip of the hidden bottom panel ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~|
+----------------------+-------------------------------------+-----------------------------------------+
  (Chat) source tab at 60%; [c] Chat the drag chip, bg.overlay, 12 right and below the pointer;
  dashed box = the drop preview: 2px accent outline, fill.hover, radius.md, inset 4
```

Annotations: the pointer is on the 24 strip along the bottom of the page area, which stands for the hidden bottom panel; the preview shows the panel at its remembered 320, exactly where the chat will land. Had the chat been the visible tab of the right sidebar, its web view would be hidden for the whole drag, its place showing "Hidden while you rearrange the layout." Releasing shows the bottom panel with the chat, focuses it, and toasts "View moved to the bottom panel: Chat", Undo, Reset layout. `Esc`, or a release over the titlebar, puts the tab back at full opacity and changes nothing.

### 8.19 Page area split in two, terminal in the bottom panel, Inbox as a view

```
+-----------------------+------------------------------+-------------------------------+
|Projects [Inbox] [.][-]| [(.) Embed the t.. x] (=) S..| [d] Resize protocol         x |  28 / 32
|-----------------------|------------------------------+-------------------------------|
| Acta                  |    32                        |    32                         |
| [Note for Acta      ] |    Embed the terminal        |    Resize protocol            |
|                 [Add] |    bound to the project      |    How the grid tells the     |
| +-------------------+ |    ...                       |    shell its size...          |
| | Terminal should   | |                              |                               |
| | remember its..    | |                              |                               |
| +-------------------+ +------------------------------+-------------------------------+
|                       | Terminal  on topic Embed the terminal              [r][.][-] | 28
|                       |--------------------------------------------------------------|
|                       |  > claude                                                    |
+-----------------------+--------------------------------------------------------------+
  300 left sidebar:       two page groups, row direction, 50 / 50; each with its tab bar;
  Projects and Inbox      the focused group (left) drives the breadcrumb; bottom panel
  as view tabs            under the page area only, 240
```

Annotations: the Inbox was dragged from its page tab into the left sidebar's header, so it became the Inbox view and follows the active project ("Acta" is its context). It drops its page title; its capture editor and cards take the full 276 of width. The second page group was made by dragging "Resize protocol" to the right edge of the page area. Each group is at least 400 wide; a narrower window folds the split (4.8.10) and lists "Group 2" in the tab bar's menu. The right sidebar is hidden; its toggle in the titlebar still shows, as do the Terminal and Chat toggles.

### 8.20 Project page with its Overview

At rest, something needs attention (Saturday 21 Nov 2026):

```
        Acta                                                       <- type.title
    12  Progress   14 of 31 topics done  [====--------]            <- properties block
        Due        None
        Folders    > [f] ~\dev\acta
        Updated    2 h ago by Claude Code
    24  Overview                                       [More v]    28  header, type.label
     4  (=)  2 blocked topics                                      32  circle-pause, warning
        (!)  1 overdue topic                                       32  clock-alert, warning
        (.)  3 active topics with no activity for 7 days           32  circle-dot, text.secondary
        [i]  6 notes in the inbox, the oldest for 12 days          32  inbox, text.secondary
    32  Description
        ...
```

At rest, nothing needs attention:

```
    24  Overview  Nothing needs attention.                [More v]    28  the whole section
    32  Description
```

"More" expanded:

```
    24  Overview  Nothing needs attention.                [Less ^]
    16  Last 7 days                                                16  type.small, text.tertiary
     4  [4 topics done] · [2 decisions] · [3 new topics]           28  inline ghost links
    16  Epics
     4  No epic           [==========--]   5 of 6                  28
        Terminal          [====--------]   3 of 7   ! 18 Nov       28  overdue epic date, warning
        Core UI           [=-----------]   1 of 9      Tue         28
        2 complete epics                                           28  text.tertiary, not a control
    16  Knowledge
     4  5 memories not updated for 90 days                         28
        |<- name, fills ->|<- 120 ->|12|<- 56 ->|<- 88 ->|
```

Epic page, the mini version collapsed (the default) and expanded:

```
        Terminal                                                   <- type.title
    12  Progress   3 of 7 topics done  [====--------]
        ...
    24  > Needs attention  2                                       28  collapsed
    32  Description

    24  v Needs attention  2                                       28  expanded
     4  (=)  1 blocked topic                                       32
        (!)  1 overdue topic                                       32
```

Annotations: no figure is drawn at zero and no line exists for a normal state; a fresh inbox (newest note 2 days old) shows nothing here, because the tree's Inbox count already says it. The Overview spends no accent: colour is `warning` on the two glyphs that mean a real problem, nothing else. Every line, link and epic row opens the list behind its number (components-records.md 5); the topic table then shows a filter chip in place of its segmented control, for example `[Blocked x]  2 topics`. An epic with nothing to report has no section at all.

### 8.21 Topic page with questions and Report to

Content column only (560); the drawing is made on Saturday 21 Nov 2026.

```
        Embed the terminal bound to the project                     <- type.title
    12  Status     [(.) Active v]
        Due        15 Dec 2026  In 24 days
        Report to  Marie                                            <- link: person page
        Epic       Terminal
        Folders    > [f] ~\dev\acta-wt\embed-terminal  +1
        Id         T-42
        Updated    2 h ago by Claude Code
    24  +--------------------------------------------------------+
        | Current state ...                                      |
        +--------------------------------------------------------+
    24  Description
        Run Claude Code inside Acta, in the project directory, ...
    32  Questions  2                                          [+]   28 header, count = open
     4  (?) Can we keep the v1 export format until March,     (A)   text wraps, never cut
            or must both formats ship together?
            Asked Marie · Answer by 18 Nov 2026  Overdue by 3 days  <- last run in warning
     4  (?) Which Windows versions must the installer       * [>]   Claude Code asked it;
            support?                                                unread dot
            Asked Legal team
        > 3 closed                                                  ghost row, folded
    32  Knowledge  3                                          [+]
        ...
    32  Timeline ...
```

Answering the first question (`A`):

```
        (?) Can we keep the v1 export format until March,     (A)
            or must both formats ship together?
            Asked Marie · Answer by 18 Nov 2026  Overdue by 3 days
         8  +----------------------------------------------------+
            | Yes, until 31 March. After that only v2.           |   answer editor, focused
            +----------------------------------------------------+
         8  [x] Also log it as a decision       [Cancel] [Save answer]
```

The "3 closed" row unfolded, and the composer open (`Q`), with the person field's suggestions:

```
        v 3 closed
        (v) Can the export file leave the EU?                 (A)
            Inside the EEA only; ask again for anything else.        <- answer, type.reading
            Asked Legal team · Answer recorded 19 Nov 2026 by Alexandre · Logged as a decision
        (-) Do we need a second signing key?                  [>]
            Dropped 17 Nov 2026 by Claude Code
        ...
        (?) [What do you need to know?                          ]   composer, last open row
         8  Asked [Ma|                     ]   Answer by [None]
                  +-------------------------+
                  | Marie            2 open |   suggestions, 4 below, floating
                  | Marius           1 open |   "Ma" matched in weight 600
                  +-------------------------+
```

Annotations: `(?)` is `circle-question-mark` (open, `text.secondary`), `(v)` `circle-check` and `(-)` `circle-slash` (closed, `text.tertiary`); `(A)` and `[>]` are the author marks of the timeline, at the right, for who created the question. The overdue phrase is the Due row's ("Overdue by 3 days", `warning`), so lateness reads in words. Report to is set and the topic is open, so no run follows the name; once the topic is Done it reads "Marie  Not reported yet". The decision checkbox is there because the question belongs to a topic; on an epic or a project page it is absent. On epic and project pages the section ends with the roll-up row ("4 more open in the topics of this epic, 1 overdue"). 8.1 predates this section; the order drawn here is the one of layout.md 4.2.

### 8.22 Questions page (project)

```
        Questions                                                   <- type.title
     8  Acta · 7 open, 12 closed                                    <- type.small tertiary
    24  [Open|Overdue|Closed|All]                                   28, Overdue only while one is late
    16  Project Acta                                          1     28 group header, link
        (?) Do we keep a free tier on the website?            (A)
            Asked Alexandre's accountant · Answer by 4 Dec 2026  In 13 days
        Epic Terminal                                         2
        (?) Is ConPTY acceptable as the minimum on Windows?   [>]
        (?) Who reviews the terminal colours for contrast?    (A)
            Asked Design review group
        Topic Embed the terminal                              2
        (?) Can we keep the v1 export format until March,     (A)
            or must both formats ship together?
            Asked Marie · Answer by 18 Nov 2026  Overdue by 3 days
        (?) Which Windows versions must the installer       * [>]
            support?
            Asked Legal team
        ...
```

Annotations: groups are owners, widest first, as on the knowledge base page; rows are the question rows of 8.21, and `Enter` opens the owner's page with the question focused. With Overdue selected the list is ungrouped, earliest answer date first, each meta line starting "in Topic Embed the terminal". Scoped to an epic (from its roll-up row) the scope line reads "Epic Terminal · 3 open, 5 closed" followed by the ghost link "Whole project".

### 8.23 Person page

```
        Marie                                                       <- type.title, as written
     8  In Acta · 2 questions to ask, 3 things to report
    32  Questions asked                                             28 header
        (?) Can we keep the v1 export format until March,     (A)
            or must both formats ship together?
            in Topic Embed the terminal · Answer by 18 Nov 2026  Overdue by 3 days
        (?) Is the 4 Dec review still on?                      (A)
            in Project Acta · Answer by 4 Dec 2026  In 13 days
        > 1 closed
    32  To report
        (v) Encrypted backups                      Not reported yet   32, text.secondary
        [l] Terminal                                   3 of 7 done   epic: its progress
        (.) Embed the terminal bound to the project         Active   open topic: status word
        [n] Ask about the review room for Thursday             Tue   open note: capture time
        > 2 reported
```

Annotations: `[l]` is `layers` (an epic), `[n]` `sticky-note` (a note). Questions are ordered by lateness here, not by their owner's order: the reader is about to talk to Marie. `Space` on "Encrypted backups" marks it reported; it moves into "2 reported" on the next frame and the change is one `Mod+Z` step, with no toast. Reached from any "Marie" link and from "Go to person...". The tab is not restored after a restart (4.7).

### 8.24 Quick capture with Report to, and the note card

```
    +--------------------------------------------------------------+
    | Note for Acta                                        [Acta]  |   640, bg.overlay, radius.xl
    | Ask about the review room for Thursday                       |
    |                                                              |
    | Report to  [Marie                ]        Ctrl Enter to save |   28, Tab from the editor
    +--------------------------------------------------------------+

        +--------------------------------------------------------+
        | Ask about the review room for Thursday                 |   note card in the inbox
        | 10 min ago · Report to Marie                [->][e][x] |   meta: name is a link
        +--------------------------------------------------------+
```

Annotations: the field starts empty on every capture and is optional; `Mod+Enter` saves from the editor or the field. Made into a topic, the note gives "Marie" to the topic's Report to. In the inbox toolbar, "Report to: anyone" appears next to Open / Filed while a listed note names a person.

### 8.25 Note with files, and the viewer

A note card in the inbox, with two images and two other files; one chip is focused:

```
        +--------------------------------------------------------+
        | Whiteboard from the review, and the vendor's quote.    |   body, type.reading
     8  | +------+ +------+                                      |   tiles 64 x 64, 8 apart
        | |######| |######|                                      |   thumbnails, cover crop
        | +------+ +------+                                      |
     8  | [f] Quote-Q4-va…dor.pdf 2.1 MB  [f] notes.md 4 KB      |   chips 28, border.default
        |  ^ focus ring on the first chip                        |
     8  | 10 min ago · Report to Marie              [->][e][x]   |   meta row
        +--------------------------------------------------------+
```

Quick capture after pasting a screenshot (`Mod+V`) and dropping a PDF, while a third file is still being stored:

```
    +--------------------------------------------------------------+
    | Note for Acta                                        [Acta]  |
    | Error on the export screen, and the log Marie sent.          |
    | +------+                                                     |
    | |######|  [f] export-log.txt 812 KB  [f] trace.zip Adding… 40%|
    | +------+                                                     |
    | (i) Files over 10 MB are kept in the database and in each of |   type.small tertiary, once
    |     its snapshots.                                           |
    | Report to  [Marie            ]   [@]     Ctrl Enter to save  |   [@] paperclip, Ctrl Shift A
    +--------------------------------------------------------------+
```

The viewer, on the second of two images of the note:

```
  +------------------------------------------------------------------------------------------+
  |  IMG_2041.jpg   3.4 MB · 3024 x 4032                 2 of 2  [<] [>]   [Save as...]  [x] |  44, border.default under
  |------------------------------------------------------------------------------------------|
  |                                                                                          |
  |                               +----------------------+                                   |
  |                               |                      |                                   |   image fitted, bg.sunken
  |                               |                      |                                   |   around it; 0 toggles
  |                               |                      |                                   |   fit / actual size
  |                               +----------------------+                                   |
  |                                                                                          |
  +------------------------------------------------------------------------------------------+
    inset 32 from the window, bg.overlay, radius.xl, shadow.overlay, over the scrim
```

Annotations: `[f]` is `file-text`, `[@]` `paperclip`, `(i)` `info`. Images come before other files; names are cut in the middle so the extension stays. The quick capture's third file shows "Adding… 40%" because it has taken over 400 ms; the note can be saved meanwhile. A file over 25 MB would instead leave the line "trace.zip is 48 MB; the limit is 25 MB per file." in `danger` with `triangle-alert`. PDFs offer Save as (and, once ADR 0012 accepts it, Open in another app); the viewer is for images and text-like files only. Made into a topic, the note's files go to the topic's description; the filed card then lists them in `text.secondary` under "Moved to T-51".
