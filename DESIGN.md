# Acta design system

Single source of truth for every pixel and every key press: this index plus the parts in `docs/design/`. UI code conforms to them; if a screen needs something that is not there, change the design docs first, in the same change, and say why.

Identity: **a workbench for long work.** A tree on the left, a page in the middle, tool windows at the side, one palette that reaches everything. Neutral, quiet surfaces in dark and in light, one verdigris accent, one sans for everything a person reads and one mono for the machine. The content belongs to the user and to Claude Code; the chrome recedes behind it.

Units are logical pixels (GPUI `px`). `Mod` means `Cmd` on macOS and `Ctrl` on Windows and Linux.

## Direction

Decided by the user on 2026-11-20. Acta takes its structure from three tools its users already live in, Obsidian, Notion and the JetBrains IDEs, and is none of them: what is borrowed is structure and behaviour, never branding, icons or palettes. Everything in the design docs follows from this section; where a later section and this one disagree, this one wins and the later one is a defect.

| From | Acta takes | Acta leaves, and why |
|---|---|---|
| **Obsidian** | The tree sidebar with collapsible nodes. Chrome that recedes behind content the user owns. Markdown as the format people and the agent write, rendered at rest and raw while editing. A properties block at the top of a page. Tabs. The quick switcher. A neutral grey dark theme. | Split panes: the terminal and the chat are the companions that sit beside a page, and a second page column would push everything under its minimum at common widths. Backlinks: links between records are not in the model yet (11). An outline pane: "Go to heading" in the palette is the outline, the same one the agent sees. Graph, canvas, ribbon, plugins, community themes: nothing in Acta's data asks for them. |
| **Notion** | A page is a title, typed properties, then the body. A generous measure. The breadcrumb in a top bar. Controls revealed on hover. A slash menu in editors. Collections of records as quiet tables with filter and sort. A near-white light theme and unshowy type. | Blocks as the storage model: text stays Markdown, which Claude Code writes natively (ADR 0007). Boards: six statuses do not fit a page column, and `S` already changes a status in one key. User-defined properties, page icons, covers, emoji, comments: each makes the agent's writes less predictable or adds decoration. |
| **JetBrains IDEs (new UI)** | Tool windows docked right or bottom, each with a header, a toggle shortcut and a remembered size. The terminal as a tool window. Search everywhere: one palette with scopes. A slim status bar. Compact density. Unambiguous focus and selection. A top bar that stays out of the way. | Icon stripes: three tool windows do not pay for 80px of permanent chrome, so their toggles live in the titlebar. Menu bar, action toolbar, gutter icons, hover popups, double-Shift: an IDE's density of affordances is wrong for a record that is mostly read. |

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

**Never:** gradients; glow or coloured shadows; blur; emoji anywhere (UI, sample data, empty states); illustrations or mascots; all-caps labels; coloured left-stripe borders on cards; pill-shaped buttons; more than two font weights; a spinner before 400 ms; skeleton shimmer; badges with counts above 99; toasts for things the user can already see changed; modal dialogs for reversible actions; a robot or sparkle icon for Claude Code; pure black `#000` as a surface, text or border colour (it exists only as the translucent `scrim` and `shadow.overlay`), pure white `#FFF` as text or as a dark-theme colour; a second typeface for display; textures; centred body text; tooltips as the only way to learn what a control does when a label fits.

## Parts

Load only the part a task needs. Section numbers are global: "4.7" or "DESIGN 4.7" means section 4.7 in whichever part holds it.

| Part | Sections | Holds |
|---|---|---|
| [color.md](docs/design/color.md) | 2 | Themes, the colour token table (2.1), measured contrast (2.2), topic statuses (2.3), terminal palette (2.4). |
| [typography.md](docs/design/typography.md) | 3 | Families (3.1), the type scale and roles (3.2), long-form text and documents (3.3). |
| [layout.md](docs/design/layout.md) | 4 | Spacing, heights, radii, icons (4.1); app shell (4.2), titlebar (4.3), tabs (4.4), tool windows (4.5), status bar (4.6), `preferences.json` (4.7). |
| [components.md](docs/design/components.md) | 5 | States and focus ring; button, inputs, editors, list / tree row, status indicator, segmented control, slash menu, notice, checkbox, palette, menus, tooltip, toast, dialog, empty state, scrollbar, resize handle, kbd, update indicator. |
| [components-records.md](docs/design/components-records.md) | 5 | Timeline entry, current-state panel, note card, properties block, topic table. |
| [components-knowledge.md](docs/design/components-knowledge.md) | 5 | Knowledge section, knowledge base page, document reading view and editor, history. |
| [components-folders.md](docs/design/components-folders.md) | 5 | Working folders: rows, inheritance, not found and repair, picker, paths, terminal launch. |
| [components-app.md](docs/design/components-app.md) | 5 | Chat panel, settings page, shortcuts page, export, import, trash, snapshot restore, key-loss recovery, updater window. |
| [interaction.md](docs/design/interaction.md) | 6 | Keyboard map (6.1), focus (6.2), motion and latency budgets (6.3), writes, undo and confirmation (6.4), changes from other processes (6.5). |
| [screens.md](docs/design/screens.md) | 8 | Wireframes: topic page (8.1), inbox (8.2), palette (8.3), terminal (8.4), first run (8.5), knowledge (8.6, 8.7, 8.10), import preview (8.8), key loss (8.9), working folders (8.11), settings (8.12), chat (8.13), epic (8.14), tab bar (8.15), updater (8.16). |
| [copy.md](docs/design/copy.md) | 7, 10.1 to 10.3 | Voice; languages, French conventions, dates, times and numbers, layout under longer text. |
| [catalogue.md](docs/design/catalogue.md) | 10.4 | Every UI string by key, English and French. Code and tests read it. |
| [data.md](docs/design/data.md) | 11 | What model data every element shows; data model requests. |

Sections 1 (principles), 9 (review checklist) and the Direction are in this file. The token table (2.1), the contrast table (2.2) and the catalogue (10.4) are parsed by tests in `crates/acta-app` (`theme.rs`, `i18n/mod.rs`): keep their headings and table shapes.

## 9. Review checklist

Run against every screen, in dark and in light, in English and in French. One failed line blocks the change.

**Tokens and type**
- [ ] Every colour is a token from 2.1; no literal hex, no ad hoc alpha, no opacity on text to fake a text level.
- [ ] `accent` appears only on: focus, selection marker, the single primary action, links, Active, unread, cursor. Count accent elements at rest: three or fewer.
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
- [ ] Workbench: a page's location (project, epic) appears only in the titlebar breadcrumb; no tab bar with a single tab; one tool window per dock, each with its 28 header; the status bar holds only project, Claude Code status, update and version.

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

**Behaviour**
- [ ] Feedback on the next frame; nothing in the "never animates" list animates; budgets in 6.3 hold with 10,000 topics.
- [ ] Writes are optimistic; failure restores state and keeps the user's text.
- [ ] Every destructive action is undoable, or is one of the confirmed cases in 6.4. No path deletes for good, overwrites a document without a revision, or writes plaintext to disk without the warning.
- [ ] Data-safety screens (export, import, trash, snapshots, key loss) state facts and the next step: no red fills, no alarm words, no blame, exact counts and dates.
- [ ] Knowledge: kind icon, summary and author mark visible wherever an entry is listed; inherited and rolled-up entries labelled with their level; every memory written by Claude Code can be read, edited, moved and deleted by the human from the list; document body uses `type.document`, short surfaces use `type.reading`.
- [ ] Working folders: the row shows the primary effective folder and, when it is inherited, where it comes from; paths are `type.code`, middle-truncated, with native separators; a folder that was not found is marked by icon and words (not colour alone), offers Locate, and blocks nothing; removing a folder never touches the disk and is undoable; the terminal never starts anywhere but the primary folder.
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
