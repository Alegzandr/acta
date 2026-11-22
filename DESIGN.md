# Acta design system

Single source of truth for every pixel and every key press: this index plus the parts in `docs/design/`. UI code conforms to them; if a screen needs something that is not there, change the design docs first, in the same change, and say why.

Identity: **a workbench for long work.** A tree on the left, a page in the middle, tool windows at the side, one palette that reaches everything. Neutral, quiet surfaces in dark and in light, one verdigris accent, one sans for everything a person reads and one mono for the machine. The content belongs to the user and to Claude Code; the chrome recedes behind it.

Units are logical pixels (GPUI `px`). `Mod` means `Cmd` on macOS and `Ctrl` on Windows and Linux.

## Direction

Decided by the user on 2026-11-20. Acta takes its structure from three tools its users already live in, Obsidian, Notion and the JetBrains IDEs, and is none of them: what is borrowed is structure and behaviour, never branding, icons or palettes. Everything in the design docs follows from this section; where a later section and this one disagree, this one wins and the later one is a defect.

| From | Acta takes | Acta leaves, and why |
|---|---|---|
| **Obsidian** | The tree sidebar with collapsible nodes. Chrome that recedes behind content the user owns. Markdown as the format people and the agent write, rendered at rest and raw while editing. A properties block at the top of a page. Tabs. The quick switcher. A neutral grey dark theme. | Backlinks: links between records are not in the model yet (11). An outline pane: "Go to heading" in the palette is the outline, the same one the agent sees. Graph, canvas, ribbon, plugins, community themes: nothing in Acta's data asks for them. |
| **Notion** | A page is a title, typed properties, then the body. A generous measure. The breadcrumb in a top bar. Controls revealed on hover. A slash menu in editors. Collections of records as quiet tables with filter and sort. A near-white light theme and unshowy type. | Blocks as the storage model: text stays Markdown, which Claude Code writes natively (ADR 0007). Boards: six statuses do not fit a page column, and `S` already changes a status in one key. User-defined properties, page icons, covers, emoji, comments: each makes the agent's writes less predictable or adds decoration. |
| **JetBrains IDEs (new UI)** | Tool windows in areas around the page, each with a header, a toggle shortcut and a remembered size. The terminal as a tool window. Search everywhere: one palette with scopes. A slim status bar. Compact density. Unambiguous focus and selection. A top bar that stays out of the way. | Icon stripes: a few views do not pay for 80px of permanent chrome, so their toggles live in the titlebar. Menu bar, action toolbar, gutter icons, hover popups, double-Shift: an IDE's density of affordances is wrong for a record that is mostly read. |

**Amendment, decided by the user on 2026-11-21: the workbench is rearrangeable, as in VS Code.** Every area around the page can be shown, hidden and resized, every view (the tree, the inbox, knowledge, the terminal, the chat, the agent browser) can be dragged to another area or into the page area as a tab, and the page area can be split into up to three groups. This replaces two earlier refusals: split panes (left out of Obsidian above, because a second page column did not fit at common widths; it is now allowed, and folds back to one group when it does not fit) and "one tool window per dock". The user added the condition that makes it acceptable: **nothing moved or closed is ever hard to put back**: every view keeps a visible toggle, every layout change is undoable, and Reset layout is one click away. The model, the defaults and the recovery rules are in 4.8 ([workbench.md](docs/design/workbench.md)); the default arrangement is the workbench described here, unchanged.

The synthesis is one object, a workbench, not three apps side by side: one tree (projects, epics, topics, with each project's inbox and knowledge), pages that read like documents (a topic is a page with properties, a current state and a timeline; a knowledge entry is a Markdown document), tool windows for the terminal and the chat panel, tabs only when more than one page is open, one palette, one status bar.

Requirements that hold whatever the look: two complete themes, dark and light, both first-class, with a System / Light / Dark setting (2); the interface in English (default) and French, selectable and persisted, with user and agent content never translated (10); every element on screen backed by real data from the model (11).

Refused outright: serif or editorial display type, parchment or paper textures, illustrations and engravings, Roman numerals, burgundy, gradients, glass effects, dashboards made of cards. A "Project Ledger" direction (parchment, serif headings, engravings) was briefed and withdrawn by the user on 2026-11-20; do not revive it.

## 1. Principles

1. **The record is the interface.** Topic title, current state and timeline are the product; chrome exists to reach them. If a pixel is neither content nor a control the user will use this week, remove it. A surface with nothing on it is correct, not unfinished.
2. **One accent, spent on "here" and "go".** Verdigris marks only: keyboard focus, the current selection marker, the primary action, links, the Active status and unread changes. Never decoration, never a second accent. If two accent things compete on one screen, one of them is wrong.
3. **Hierarchy comes from type and space, then tone, then lines.** Reach for size/weight/spacing first, a surface step second, a 1px border third. Shadows only on things that float above the window. Boxes inside boxes are a defect.
4. **Keyboard first, pointer complete.** Every action has a key path through the command palette at minimum; every key path has a visible pointer equivalent. A feature without a palette command is not shipped.
5. **Instant means the next frame.** Selection, hover, focus, navigation and typing never animate and never wait on disk. Motion is reserved for things that appear from nowhere (overlays, toasts, remote changes) and exists to explain, not to please.
6. **The agent writes; the human is never moved.** Changes made by Claude Code appear live but never take focus, never scroll the viewport, never overwrite a field being edited. The reader's place is sacred.
7. **Undo over asking.** Anything reversible happens immediately with an undo. A dialog is an admission that we could not make the action reversible.

**Never:** a person as free text (every field that names someone is a contact, components-contacts.md 5); gradients; glow or coloured shadows; blur; emoji anywhere (UI, sample data, empty states); illustrations or mascots; all-caps labels; coloured left-stripe borders on cards; pill-shaped buttons; more than two font weights; a spinner before 400 ms; skeleton shimmer; badges with counts above 99; toasts for things the user can already see changed; modal dialogs for reversible actions; a robot or sparkle icon for Claude Code; pure black `#000` as a surface, text or border colour (it exists only as the translucent `scrim` and `shadow.overlay`), pure white `#FFF` as text or as a dark-theme colour; a second typeface for display; textures; centred body text; tooltips as the only way to learn what a control does when a label fits.

## Parts

Load only the part a task needs. Section numbers are global: "4.7" or "DESIGN 4.7" means section 4.7 in whichever part holds it.

| Part | Sections | Holds |
|---|---|---|
| [color.md](docs/design/color.md) | 2 | Themes, the colour token table (2.1), measured contrast (2.2), topic statuses (2.3), terminal palette (2.4). |
| [typography.md](docs/design/typography.md) | 3 | Families (3.1), the type scale and roles (3.2), long-form text and documents (3.3). |
| [layout.md](docs/design/layout.md) | 4 | Spacing, heights, radii, icons (4.1); app shell (4.2), the Projects tree: structure, signals, reveal, long trees, empty states (4.2.1), titlebar (4.3), tabs (4.4), tool windows (4.5), status bar (4.6), `preferences.json` (4.7). |
| [workbench.md](docs/design/workbench.md) | 4.8 | Workbench layout: areas, views, area header and view tabs, showing and hiding, resizing, moving views by drag and drop and by command, page groups, web views during drags, narrow windows, recovering the layout, persistence, defaults and Reset layout. |
| [components.md](docs/design/components.md) | 5 | States and focus ring; button, inputs, editors, list row, tree row (kinds, states, drag, sticky project row), status indicator, segmented control, slash menu, notice, checkbox, palette, menus, tooltip, toast, dialog, empty state, scrollbar, resize handle, kbd, update indicator. |
| [components-records.md](docs/design/components-records.md) | 5 | Timeline entry, current-state panel, note card, properties block, due row and due date picker, topic table, Overview section and filter chip. |
| [components-questions.md](docs/design/components-questions.md) | 5 | Questions and Report to: naming, the Questions section and question row, composer, answering and the decision offer, Report to (row, popover, notes, filing), the Questions page, signals elsewhere. (The free-text person value and the person page were replaced by contacts.) |
| [components-contacts.md](docs/design/components-contacts.md) | 5 | Contacts and your profile: the rule "a person is always a contact" and its audit, the contact record, levels (all projects, project, epic) and moving between them, the contact picker, the Contacts page, the contact page, merging, deleting, the Profile in Settings, what Claude Code sees. |
| [components-attachments.md](docs/design/components-attachments.md) | 5 | Files on notes, entries and descriptions (ADR 0012): the attachment strip, adding by paste, drop and button, refusals, the viewer, Save as and Open in another app, where files show elsewhere. |
| [components-voice.md](docs/design/components-voice.md) | 5 | Dictation (ADR 0014): where the microphone is, the button and its states, taking dictation, errors, first use, Settings > Voice, keys. |
| [components-knowledge.md](docs/design/components-knowledge.md) | 5 | Knowledge section, knowledge base page, document reading view and editor, history; memory quality (ADR 0015): reasons to check, the to-check mark, Mark as still true, notices, superseded and retired, merge, provenance and sources. |
| [components-folders.md](docs/design/components-folders.md) | 5 | Working folders: rows, the switcher, inheritance, the active folder, git state, commit modes, not found and repair, picker, paths, terminal launch. |
| [components-app.md](docs/design/components-app.md) | 5 | Chat panel, settings page, shortcuts page, export, import, trash, snapshot restore, key-loss recovery, updater window. |
| [interaction.md](docs/design/interaction.md) | 6 | Keyboard map (6.1), focus (6.2), motion and latency budgets (6.3), writes, undo and confirmation (6.4), changes from other processes (6.5). |
| [screens.md](docs/design/screens.md) | 8 | Wireframes: topic page (8.1), inbox (8.2), palette (8.3), terminal (8.4), first run (8.5), knowledge (8.6, 8.7, 8.10), import preview (8.8), key loss (8.9), working folders (8.11), settings (8.12), chat (8.13), epic (8.14), tab bar (8.15), updater (8.16), workbench with the right sidebar (8.17), dragging a view (8.18), split page area and views moved (8.19), project page with its Overview (8.20), questions and Report to on a topic (8.21), questions page (8.22), person page (8.23), quick capture with Report to (8.24), the contact page (8.23, was the person page), the Contacts page (8.28), the contact picker (8.29), Settings > Profile (8.30), a note with files and the viewer (8.25), Ready for Claude Code (8.26), dictation (8.27), knowledge to check (8.31), a memory with provenance and a notice (8.32), history with state changes (8.33). |
| [copy.md](docs/design/copy.md) | 7, 10.1 to 10.3 | Voice; languages, French conventions, dates, times and numbers, due dates and typed dates (10.2.1), layout under longer text. |
| [catalogue.md](docs/design/catalogue.md) | 10.4 to 10.4.12 | Every UI string by key, English and French. Code and tests read 10.4; 10.4.1 to 10.4.6, 10.4.10 and 10.4.11 note how keys moved into it; 10.4.7 to 10.4.9 and 10.4.12 hold keys pending implementation. |
| [data.md](docs/design/data.md) | 11 | What model data every element shows; data model requests. |

Sections 1 (principles), 9 (review checklist) and the Direction are in this file. The token table (2.1), the contrast table (2.2) and the catalogue (10.4) are parsed by tests in `crates/acta-app` (`theme.rs`, `i18n/mod.rs`): keep their headings and table shapes.

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

**Behaviour**
- [ ] Feedback on the next frame; nothing in the "never animates" list animates; budgets in 6.3 hold with 10,000 topics.
- [ ] Writes are optimistic; failure restores state and keeps the user's text.
- [ ] Every destructive action is undoable, or is one of the confirmed cases in 6.4. No path deletes for good, overwrites a document without a revision, or writes plaintext to disk without the warning.
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
