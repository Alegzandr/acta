# Acta design: review checklist

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list. Section numbers are shared across all parts. Load this part to review a screen or before calling UI work done.

## 9. Review checklist

Run against every screen, in dark and in light, in English and in French. One failed line blocks the change.

**Tokens and type**
- [ ] Every colour is a token from 2.1; no literal hex, no ad hoc alpha, no opacity on text to fake a text level.
- [ ] `accent` appears only on: focus, selection marker, the single primary action, links, Active, unread, cursor. Count accent elements at rest: three or fewer (in a list, the Active glyphs count as one element and the unread dots as one, since each is one signal repeated).
- [ ] Every text style is a role from 3.2; only weights 400 and 600; no uppercase; nothing under 12px.
- [ ] Digits that align or update use `tnum`.
- [ ] Reading text is within the content column (the Reading width setting) and left-aligned.
- [ ] Text/surface pairs used are in 2.2 and pass; `text.tertiary` is not on a selected overlay row; no coloured text on `accent.subtle`.

**Layout**
- [ ] Every gap is on the spacing scale; control heights are 24, 28, 32 or 44; radii are 4, 6, 8, 10 or 12.
- [ ] No shadow on anything that does not float; every floating layer has outline plus shadow.
- [ ] No box inside a box; no divider where spacing already separates.
- [ ] Works at 880 x 560 with the terminal open, and at 200% scale; no horizontal scroll outside code blocks.
- [ ] Titlebar follows 4.3 on the platform under review; window controls are where the platform puts them.
- [ ] Workbench: a page's location (project, epic) appears only in the titlebar breadcrumb; no tab bar with a single tab in a single group; every side area and the bottom panel has its 28 header per group, with view tabs when it holds two views or more; the status bar holds only project, Claude Code status, update and version.
- [ ] Layout (4.8): every area respects its size range and every view its minimum, in every area it can be moved to; the page area keeps 420 (400 per group); narrow windows give way in the order of 4.8.10 and nothing of it is saved; a page that is not Inbox or Knowledge never leaves the page area.

- [ ] Hover and focus never move anything: every revealed control (chips, `ellipsis`, `plus`, `x`, chevrons) sits in space reserved or empty at rest; move the pointer slowly over the properties block, the folders row, every row and header, and nothing below shifts.
- [ ] Menus set their items in `type.menu` (13 / 18) on rows of 28; the palette keeps `type.ui` on rows of 32.

**States**
- [ ] Each interactive element has default, hover, pressed, keyboard focus and disabled, as specified in 5.
- [ ] Empty, loading (over 400 ms only), error and long-content states exist: a 200-character title, a 500-entry timeline, 0 and 1,000 topics.
- [ ] Both themes: every colour comes from a token with a dark and a light value; the screen was looked at in both; switching theme moves nothing.
- [ ] Status is identifiable with the screen in greyscale.
- [ ] Human and Claude Code entries are distinguishable in greyscale and without reading the name.

**Keyboard and focus**
- [ ] Every action on the screen is reachable without a pointer and is in the palette with its shortcut shown.
- [ ] Focus is visible at every step, moves in reading order, and returns to its origin when an overlay closes.
- [ ] `Esc` goes one level out and never navigates; no shortcut steals a shell key while the terminal is focused.

**Layout and recovery** (4.8.11, run after moving each view to each area and back)
- [ ] Every view has a toggle in the titlebar that is visible while the view is hidden, in a hidden area or in a folded group, and it brings the view back to its last place and its area's remembered size; the Layout menu and "{view}: Show" do the same.
- [ ] Reset layout is one click from the Layout menu, the move toast and Settings, is in the palette, and is undoable; it keeps tabs, sessions and the chat's sign-in.
- [ ] Every layout change (move, hide, show, resize, split, join, tab move, reset) is one step of `Mod+Z` and of "Undo layout change"; moves and hides from a view menu toast with Undo and Reset layout.
- [ ] During a drag the preview shows the exact landing bounds before release; a drop lands exactly there; `Esc`, a drop outside every zone or on the view's own place changes nothing and toasts nothing.
- [ ] No view can be lost: hiding an area keeps its views' toggles; an area left empty hides; a preferences file naming a view twice or an unknown place loads with every view placed once.
- [ ] Web views (chat, agent browser) are hidden with their placeholder line for the whole of a drag, return at their new bounds without reloading, and never cover a resize handle or a drop zone.
- [ ] Keyboard: every move has a palette command and a view-menu item; `F6` / `Shift+F6` visit every shown area and group in order; every toggle key works from the page, and the kept keys of 6.1 from the terminal and the chat.

**Tree** (4.2.1, run with 0 projects, an empty epic, and a project of 140 topics at 260 and at 200 wide)
- [ ] Order under a project: Inbox, Knowledge, 4 of space, topics in no epic, epics; "N closed" last in each group; 8 of space after an expanded project only; stored order, never sorted by status.
- [ ] Weight marks structure only: projects and epics 600, every other row 400 in every state, the selected row included; no row has an icon, colour or badge the table of 5 (Tree row) does not give it.
- [ ] Where am I: the open page's row is selected and in view after every way of opening a page (pointer away from the tree); its project and epic rows are `text.primary`, every other project and epic row `text.secondary`.
- [ ] Colour in the tree is only: Active glyph and unread dot (`accent`), Blocked glyph (`warning`), the drop line and target; closed topics are `text.tertiary` glyph and title; nothing tertiary on a selected row.
- [ ] One trailing item per row at most; only the unread dot rolls up into folded rows, and folding never hides one; only Inbox shows a count, and only above 0; the `plus` appears on hover of project and epic rows without moving their title.
- [ ] Long tree: the sticky project row names the project of the rows under it; truncated titles have the title tooltip; Collapse all is in the view menu and the palette; scrolling, folding and remote changes hold the next frame with 10,000 topics.
- [ ] Drag: every move by drag has its key (`Alt+Up` / `Alt+Down`, `M`); the insertion line or target shows exactly where the row lands; `Esc` cancels; a drop is one `Mod+Z` step, with no toast.
- [ ] Empty: no project shows "No projects yet." and "New project..."; an expanded empty epic shows the "New topic" ghost row; nothing else is added for emptiness.

**Due dates** (run with no date anywhere, then with topics overdue, due today, due in 2 and in 30 days, and a closed topic with a past date, at 23:59 and after midnight)
- [ ] Nothing about dates shows until a record has one: no Due row while unset (its add chip appears only on hover or keyboard focus of the properties block); the Due column and the Overdue and Next 7 days segments appear only while a listed topic has a date.
- [ ] Overdue reads in greyscale everywhere: words on the page ("Overdue by 3 days"), `clock-alert` plus `warning` in the table, the `clock-alert` shape in the tree. A closed topic, or a complete epic or project, is never overdue anywhere, agent output included.
- [ ] Tree: only overdue and due today are drawn, neutral (`text.secondary`, `text.tertiary`), in the trailing slot, after the hover `plus` and the unread dot; never rolled up into a folded row; no date text and no colour.
- [ ] Picker: reachable by `D`, `Enter` on the row and "Set due date..."; every action works from the keyboard; the preview line shows the exact day before `Enter`; `Esc` and a click outside change nothing; English and French words parse in both languages; day-month order only.
- [ ] Setting, changing and clearing is one `Mod+Z` step, writes a system event, and shows no toast. Past midnight, relative forms and marks update without moving a row; day counts are calendar days across a daylight-saving change.
- [ ] Agent side: `brief`, `find` and the session briefing stay within their budgets with every record dated; `update` takes only `YYYY-MM-DD` or `""`.

**Overview** (components-records.md 5, screens.md 8.20; run with a calm project, then one with blocked, overdue, idle topics and a 12-day-old inbox, in both themes and languages)
- [ ] At rest the project page shows at most five attention lines, each only above zero; nothing applies: the header row and "Nothing needs attention." only; no zero, no chart, no tile, no card.
- [ ] Colour is `warning` on the Blocked and Overdue glyphs only; no accent at rest; every line reads in greyscale by glyph and words.
- [ ] "More" is collapsed by default and holds only Last 7 days, Epics and the memory line, each absent when it has nothing to say; the epic page's section is absent with nothing to report and collapsed by default otherwise.
- [ ] Every number opens the list it counts, and the list's count equals it (the one exception: "memories to check", whose list may add memories flagged by git after it opens); the chip clears with its `x`, `Backspace` or `Esc`; every line has its palette command.
- [ ] A remote write that adds or removes a line moves neither focus nor viewport; values change at local midnight without moving anything else.
- [ ] Agent side: the `Attention:` line appears only when something needs attention, is one line within 160 chars (120 on an epic), and `brief` stays within its budget with every topic blocked, overdue or idle.

**Questions, Report to and contacts** (run with no contact anywhere, then with a topic holding open, overdue, answered and dropped questions, an epic and a project with roll-ups, two contacts named Marie at nested levels and two in different projects, a contact in the trash still named, and a done topic not reported)
- [ ] Empty: the section is its header and one line; nothing about questions or contacts shows in the tree, the status bar or the tabs, and the sidebar's Contacts row exists only once a contact does; the Questions and Report to columns and the inbox's Report to filter exist only while something uses them.
- [ ] A question's state reads in greyscale (glyph shape), its lateness in words ("Overdue by 3 days"); a closed question shows no date and is never overdue, agent output included.
- [ ] Question text wraps and is never truncated; open questions keep their stored order on the page and never move under the reader; the contact page and Overdue order by lateness.
- [ ] Every action works from the keyboard: `Q` adds, `A` answers, `E` edits, `D` dates, `M` moves, `R` sets Report to, `Space` marks reported on the contact page, `M` moves a contact, `F2` renames it; all are in the palette; each change is one `Mod+Z` step (an answer and its decision entry together), with a toast only for delete.
- [ ] The decision checkbox appears only for a question on a topic; the entry it writes holds only the question and the answer, no words of Acta's.
- [ ] A person is always a contact: no input anywhere stores a typed name; the picker's value is a contact or nothing, and leaving it with unconfirmed text restores the previous value; "New contact" creates with the name only, in the same `Mod+Z` step as the write that uses it.
- [ ] Levels: the picker offers only contacts visible from the record (its epic, its project, all projects), grouped nearest first; moving a contact down is refused with the places that name it outside; moving a record never clears a contact, it lifts it with a toast and Undo; deleting a project or an epic takes its contacts to the trash and back.
- [ ] Contacts: details (job title, company, how to address them) show only when set, added from chips; two contacts with one key at overlapping levels show the duplicate notice, in different projects never; merge keeps every reference and is one step with Undo; a deleted contact still named shows as plain text with "In the trash" and is not purged while named.
- [ ] The contact's name is shown as written, never translated, in French always after a colon; the contact page and Contacts page are restored after a restart (record id, fixed key), and `preferences.json` holds no name, title or note.
- [ ] Profile: the name set in Settings names every human write, the system user name only until one is set; the profile is in the database, never in `preferences.json`.
- [ ] Report to: on topics and epics only, its row shown only when set; "Not reported yet" only on closed work; a note's person carries to the topic or epic made from it; reported is never inferred.
- [ ] Agent side: `brief`, `find` and the session briefing stay within budgets with 50 overdue questions per topic and 80-char names; question, answer, contact name, details and address note and the user's name and title are inside the data fence; the `User:` line appears only with a name set in Settings; an unknown or ambiguous name is refused with near matches or ids, never created; the agent sees only contacts in scope; projects refuse `report_to`.

**Attachments** (run with a note holding 20 files: images, a PDF, a text file, a 24 MB archive; a filed note; at 880 x 560)
- [ ] No composer on the timeline: only Claude Code adds entries; every entry can be edited, retyped and deleted with undo.
- [ ] One idea per line: no description wraps past one line at 560 in English; explanations of how something works live in tooltips or behind "Details".
- [ ] Paste, drop and the `paperclip` button each add files in quick capture, the inbox capture, a note, a timeline entry being edited and a description; text on the clipboard always pastes as text; `Mod+Shift+A` exists only in those editors.
- [ ] Every file action works from the keyboard (strip stop, `Left` / `Right`, `Enter`, `Mod+S`, `Delete`, `Shift+F10`); adding and removing are one `Mod+Z` step each, with a toast only for remove.
- [ ] Nothing decrypted touches the disk except Save as, chosen by the person, and, only once ADR 0012, 8, is accepted, the Open copy under `<data dir>/open`, deleted at quit and at the next start.
- [ ] Refusals name the file, its size and the limit; nothing over 400 ms spins; the UI never waits on a file being stored.
- [ ] Viewer: images fit and never upscale; GIFs do not animate; text shows at most 1 MB with the notice; `Esc` returns focus to the file; web views are hidden under it.
- [ ] Agent side: names and sizes only in `find`, `brief` and `entries`; `attachment` returns fenced text within 6,000 chars, one image within the pixel cap, or one line for a binary.

**Dictation** (components-voice.md 5; run with no model, while downloading, listening, writing, with the microphone blocked, at 880 x 560 in the inbox capture and quick capture)
- [ ] The microphone is in each multi-line editor and nowhere else; the footer does not move when it changes state; with Dictation off it is absent.
- [ ] Every state reads in greyscale (icon shape and words); no red and no accent for listening; the level bar follows the sound and nothing else moves.
- [ ] Text appears only at the anchor, phrase by phrase, never rewritten after it appears; a take is one undo step; `Esc` stops without cancelling the editor.
- [ ] Nothing downloads without a click that names the size; free space is shown first; a model is used only after its hash matched.
- [ ] No audio, and no dictated text outside the editor, reaches a file, a log, the clipboard or a process argument (ADR 0014, 7).

**Zoom and section folds** (layout.md 4.9, components-records.md 5; run at 90%, 100%, 150% and 200% on a 1920 x 1080 and a 1366 x 768 screen, with the terminal and the chat shown, in both languages)
- [ ] Everything scales together: type, icons, borders, the terminal grid, the chat and the agent browser; 1px lines stay crisp; no text is a scaled bitmap; the column, the sidebars and the panel keep their proportions.
- [ ] The layout at each zoom is the layout of the effective size (4.8.10), down to 760 x 480: the right sidebar becomes an overlay only below 820 effective; nothing scrolls horizontally that 4.2 does not allow.
- [ ] A window that cannot hold a step never takes it; a window made smaller by the system drops to what fits and comes back when it has room; "Zoom 150%" is in the status bar whenever the zoom is not 100% and resets it.
- [ ] `Mod+=` / `Mod+-` / `Mod+0` work from the page, the tree, the chat and the agent browser; in the terminal on Windows and Linux `Ctrl+-` still reaches Claude Code.
- [ ] An unfolded page looks exactly as before; a folded section is one header line with its chevron and count; nothing moves when a chevron appears; Claude Code writing inside a folded section shows the dot on its header.
- [ ] Collapse all and Expand all keep the reader's place; a fold follows the page kind across records and survives a restart; `preferences.json` holds only fixed fold keys and the zoom percent.

**Tools and pages** (layout.md 4.5.1; run with sessions on a topic, its epic and its project, a chat on each, and Claude Code (no project) running)
- [ ] The Terminal, Chat and Agent browser views show only the current page's instances; navigating never stops, restarts or moves one; every header names its owner first.
- [ ] Every owning page offers "Start Claude Code here" in one key; a page that owns nothing says so and offers Claude Code (no project).
- [ ] A session on another page is visible only as signals (tree glyph, status bar, Sessions view, page line) and "Show terminal" opens its own page.
- [ ] Claude Code (no project) is in the home folder, has no Acta argument, uses the `terminal` icon and its own view, and is never listed as a page's session.

**Markdown and the editor** (components-editor.md 5; run with the conformance corpus open as a document and as an entry, in both themes, at 100% and 150%, in the editor and in reading)
- [ ] Every element of the corpus renders as the element table says; nothing is dropped or half-rendered, unknown syntax shows as typed, nested numbered lists keep their numbers.
- [ ] No image loads from anywhere but an attachment (watch the network: none); raw HTML shows as text, `<br>` excepted; only `http`, `https`, `mailto` and `#anchor` links act, a middle click does nothing, a deceptive link asks first.
- [ ] A wide code block and a wide table scroll inside their block with the scrollbar shown; the page never scrolls sideways.
- [ ] Open then save with no change writes nothing; an edit leaves every byte outside it identical (compare the stored text); a task toggle changes one byte and is one undo step.
- [ ] Moving the caret between blocks never moves the line with the caret on screen; the active block shows its syntax in `text.tertiary` and nothing else changes style.
- [ ] Every editor key of the table works on AZERTY too; the palette lists the formatting commands only while an editor has focus.

**Behaviour**
- [ ] Feedback on the next frame; nothing in the "never animates" list animates; budgets in 6.3 hold with 10,000 topics.
- [ ] Writes are optimistic; failure restores state and keeps the user's text.
- [ ] Every destructive action is undoable, or is one of the confirmed cases in 6.4. No path deletes for good except the trash's confirmed "Delete permanently..." and "Empty trash...", none overwrites a document without a revision, or writes plaintext to disk without the warning.
- [ ] Data-safety screens (export, import, trash, snapshots, key loss) state facts and the next step: no red fills, no alarm words, no blame, exact counts and dates.
- [ ] Memory quality (components-knowledge.md 5; run with a healthy project, then with memories old, in conflict, duplicated, on a merged branch, with a changed and a deleted source file, superseded and retired): a healthy list shows no mark, notice, chip or ghost row; a flagged row shows one `flag` in the signal slot (the unread dot wins), its reasons in the tooltip, never a colour or a count; an entry shows at most one notice; `V` marks as still true from row, unfolded memory and page, and no row leaves the list under the reader; superseded and retired entries are reachable only through the list's last row or the palette, and Make live again brings them back; merge names its direction and is one `Mod+Z` step with Undo; every state change is one undo step; no reason or label names a session id, a score or a blob.
- [ ] Knowledge: kind icon, summary and author mark visible wherever an entry is listed; inherited and rolled-up entries labelled with their level; every memory written by Claude Code can be read, edited, moved and deleted by the human from the list; document body uses `type.document`, short surfaces use `type.reading`.
- [ ] Working folders: the row shows the active folder by name and icon, its git branch and changes when known, and, when it is inherited, where it comes from; the switcher changes the active folder from the row and from the terminal header in one step, and a running session is never restarted by it (the header offers "Restart in ..."); the list shows own and inherited folders in separate groups, inherited ones edited only at their level; the active folder is marked by shape and the word "Active", not colour alone; another checkout of the same repository is labelled as not passed; paths are `type.code`, middle-truncated, with native separators; a folder that was not found is marked by icon and words (not colour alone), offers Locate, and blocks nothing; removing a folder never touches the disk beyond the settings Acta wrote, and is undoable; the terminal never starts anywhere but the active folder.
- [ ] Commit modes: offered only for a folder in a git repository; Manual is the default; each mode is described in words under the control; "Commit in my name" and "Also on main" appear only with an automatic mode; the form names the git identity commits will use and says where Acta writes its rules, or why it does not; nothing in the UI sets git identity or pushes by itself.
- [ ] A write from another process during reading, scrolling and editing moves neither focus nor viewport, and never overwrites a field in edit.

**Copy**
- [ ] Sentence case, the vocabulary of section 7, no exclamation marks, no emoji, "Claude Code" by name.
- [ ] Errors state what failed and the next step; empty states state the purpose and the one action.
- [ ] Sample data uses "Alexandre".
- [ ] Every string comes from the catalogue (10.4) by key, in both languages; no sentence is assembled from fragments; counts use the plural forms.
- [ ] In French nothing is clipped, overlapped or abbreviated by hand: labels size to their text, and what truncates is listed in 10.3. Dates, times and numbers follow 10.2.
- [ ] Nothing the user or Claude Code wrote is translated or reformatted.
- [ ] Every value on screen comes from the model as mapped in 11; nothing is a placeholder.
- [ ] Chat panel: Acta draws nothing over the web view and never reads or changes the page; under an overlay the placeholder shows; every failure ends in a way to open claude.ai in the browser.

**Last look**
- [ ] Remove one element. If the screen still works, it stays removed.
- [ ] Would this screen be mistaken for a template, or for Obsidian, Notion or a JetBrains IDE? If yes, the answer is in type and spacing, not in adding decoration.
