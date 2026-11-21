# Plan

Each milestone ends with a working, tested, released build. Status: `todo`, `doing`, `done`.

| # | Milestone | Release | Status |
|---|-----------|---------|--------|
| M0 | Research, spikes, ADRs, DESIGN.md | - | done |
| M1 | Pipeline: window that installs silently and updates itself; design foundations; app icon | v0.1.x | done |
| M2 | Data model and core UI | v0.3.0 | done |
| M3 | MCP server, CLI, Claude Code setup; Acta tracks itself | v0.3.0 | done (Acta tracks itself since 2026-11-21: project P-1 linked to this repository) |
| M4 | Integrated terminal running Claude Code | v0.3.0 | doing (docked; live Claude Code session to verify) |
| M5 | Claude chat panel beside the work (Windows and macOS; link on Linux; ADR 0009) | v0.3.x | done on Windows (user checks pending: keyboard in the page, a real sign-in) |
| M6 | Notes inbox and notes-to-topics flow | v0.3.x | done |
| M8 | VS Code-style docking layout; agent browser for Claude Code (ADR 0010) | v0.4.0 | done |
| M7 | Polish | v0.4.0 | doing (hands-on QA of drag and drop, agent browser Stop/Done, live Claude Code in the terminal) |

## M0 - Research and spikes
- [x] ADR 0001: UI stack - gpui-kit 0.10.0 (GPUI snapshot gpui-pre 0.3.12), proven on Windows; macOS/Linux to prove in M1 CI
- [x] ADR 0002: Terminal embedding - alacritty_terminal 0.26.0 for emulation and PTY, own GPUI element; headless spike proven on Windows with Claude Code
- [x] ADR 0003: Encrypted storage - SQLCipher via rusqlite 0.43, raw key in the OS keychain, proven on Windows; macOS key store decided by an M1 runner test
- [x] ADR 0004: Packaging and updates - Velopack 1.2.161, two executables (GUI + CLI/MCP) in one package; install, self-update and uninstall proven on Windows
- [x] ADR 0005: Claude Code integration - rmcp 3.11.0, 9 tools, 4 hooks, local plugin installed by `acta setup`
- [x] ADR 0006: Import, export and backups
- [x] ADR 0007: Knowledge base (documents and agent memory) on projects, epics and topics
- [x] DESIGN.md - accepted; Source Sans 3 conditional on a GPUI rendering check on Windows in M1

## M1 - Pipeline
- [x] Cargo workspace, empty GPUI window (built and run on Windows; macOS and Linux to be proven by CI)
- [x] Design foundations: tokens for both themes, fonts (Inter replaces Source Sans 3), Light / Dark / System, en/fr string table, persisted preferences, shell skeleton, updater window, Settings (Appearance) (built and captured on Windows only)
  - Temporary deviations from DESIGN.md until the data exists: both status bar items open the top of Settings (not Settings at Claude Code or at About), and Settings shows only the Appearance section (Theme, Language).
- [x] CI: fmt, clippy, tests on every push, all three OSes (green since the Rust 1.100.0 bump)
- [x] Release workflow on `v*` tags: builds all four channels, publishes installers, feeds, deltas, SHA256SUMS and attestations (v0.1.0 published; v0.1.1 and v0.1.2 are tags without a release, see ADR 0004 amendments)
- [x] Repository settings: `v*` tag ruleset, immutable releases, `release` environment limited to `v*`
- [x] App icon: "Companion" (planet, ring, moon), rendered by `scripts/icon/`; shipped in v0.1.3
- [x] Self-update proven end to end by `update-e2e.yml` on Windows, macOS and Linux runners (v0.1.3 -> v0.1.4, run 44851174491; Windows also v0.1.0 -> v0.1.3). v0.1.4 moved the update source off the GitHub API; its first proof is the next release
- [x] Silent per-user install and Windows uninstall verified (same workflow)
- [x] macOS keychain test with ad-hoc signed builds (ADR 0003, `platform-checks.yml` run 44851373960): keychain prompts after every update, so macOS keeps the key file; SQLCipher builds and encrypted snapshots pass on macOS and Linux runners

## M2 - Data model and core UI
- [x] Encrypted database, key in OS keychain, migrations (`acta-core`; built and tested on Windows only, macOS and Linux by CI)
- [x] Domain: Project > Epic > Topic, timeline entries, statuses, current-state summary (`acta-core` store; data model requests 5, 8, 9, 10, 11, 12, 14 and the store side of 6 included)
- [ ] App shell: sidebar, topic view, timeline, command palette, keyboard navigation - built on `gpui-engineer/m2-ui` (Windows only); tabs and the chat panel are M5; pointer and key paths beyond the captures not exercised by hand
- [ ] Create, edit, reorder, change status; live refresh when another process writes - built (100 ms watcher, undo stack); refresh from a second process not yet observed in the running app
- [ ] Working folders on projects, epics and topics with inheritance (ADR 0008) - store and UI built (row, list, inherited label, not found, Locate, bulk repair, terminal prompt); drag reorder and the narrow two-line form to do; superseded in part by ADR 0011 (below)
- [ ] Knowledge base (ADR 0007): documents and agent memories on projects, epics and topics, with summaries, revisions, inheritance, roll-up browsing, reading view and editor - store and UI built; memory edit in place, move and change kind to do
- [ ] No data loss (ADR 0006): stable UUIDs, soft delete with trash, automatic snapshots before migrations and daily, self-restore on failed migration - store done; trash page, daily snapshot and snapshot restore built in the UI
- [ ] Export and import of the open `.acta.json` format from the command palette, with the lossless round-trip test in CI - format, merge import and round-trip test done; palette commands, export dialog and import preview built
- [ ] Key-loss recovery path: never overwrite an unreadable database, offer import - typed open errors and recovery screen built (Import, Try again, Start empty keeps the file aside)

## M3 - MCP, CLI, Claude Code setup
- [x] `acta mcp` (stdio, rmcp 3.11) and `acta <command>` CLI sharing the domain layer (`acta_core::agent`)
- [x] Compact responses with enforced size budgets: one renderer, a budget test per tool on pathological data and on real `tools/call` results over stdio; `tools/list` under 7,000 chars, instructions under 800
- [x] Knowledge tools: list with summaries, outline, read by section or lines, scoped search, section-level writes, near-duplicate memory refusal; the skill routes durable memory to Acta
- [x] `acta export`, `acta import`, `acta snapshot list|restore` in the CLI (not exposed over MCP)
- [x] Hooks, skill, slash commands; `acta setup` one-step install, `--check`, `--remove` (proven with the real `claude` CLI against a temporary config directory, and one headless Haiku run with the generated plugin); Windows only so far
- [x] `acta link` at any level; sessions started outside Acta resolve their topic from the folder (ADR 0008); salted-hash pre-check file for hooks
- [ ] `mcp_tool` hooks after `/clear`, after compaction and on `SubagentStart` seen in an interactive session; macOS and Linux runs
- [ ] `--json` output of the CLI agent commands (ADR 0005, section 4)
- [ ] Dogfooding: this project is tracked in Acta from here on

## M4 - Terminal
- [ ] Embedded terminal launching Claude Code in the effective working folder of the topic, epic or project it was opened from (ADR 0008)
- [ ] Claude integration pass on `integration-engineer/claude-integration` (ADR 0005 and 0008 amendments of 2026-11-21): terminal passes `--plugin-dir` and resumes each page's conversation (`--session-id`/`--resume`), session presence (`presence.db`, hooks, `acta sessions`; status bar, tree glyph, page line, Sessions view, terminal context, optional taskbar flash), plan usage through `acta statusline` (wraps an existing status line) shown in the status bar, Set up also links every attached folder. Left: an interactive Claude Code session driving the hooks for real; the UI checked by eye
- [ ] Rendering, input, resize, scrollback, copy/paste, links
- [ ] Follow-up: per-row shaping cache keyed by row content, skipping rows `Term::damage()` leaves untouched (ADR 0002 rendering plan); the view relies on GPUI's per-frame line layout cache until then

## M5 - Claude chat panel
- [x] ADR 0009 accepted with conditions: claude.ai in the system web view, Windows proven by the spike, macOS unproven, Linux gets a link; M5 opens with the keyboard handoff check, a real sign-in by the user and a macOS build
- [ ] First: keyboard handoff while the page has focus, a real sign-in by the user, a macOS build (the three things the spike could not check)
- [x] DESIGN.md: specify the chat panel (placement, toggle, placeholder under overlays, failure and sign-in help states) - `docs/design/components-app.md`, `docs/design/screens.md` 8.13
- [ ] Side panel with a persistent session, toggled from the keyboard - built on `gpui-engineer/m5-chat` for Windows: chat tool window in the right or bottom dock, lazy web view with profile in `<data dir>/webview`, allow-list, hidden under overlays, runtime-missing / could-not-open / sign-in help states, kept keys routed through WebView2's `AcceleratorKeyPressed`; macOS and Linux get the link (ADR 0009 changelog 2026-11-21); the kept-key route and a real sign-in are not yet exercised

## M6 - Notes
- [ ] Per-project inbox, quick capture (inbox page and Mod+N in M2)
- [x] Agent triage: `find(kind="note")` lists the inbox; one all-or-nothing `create` call turns notes into topics (draft, in an existing epic or an epic of the same call with `epic="@k"`), epics, or entries of existing topics, and `dismiss` closes the rest; `update(N-n, status=open)` puts a note back. Schema 2: notes keep `filed_epic_id` and `filed_by`. CLI: `acta create entry|dismiss|plan`. Proven by an end-to-end stdio test
- [x] `/acta:triage` skill: read, group, propose one compact plan, apply after confirmation (or with `go`), report ids
- [x] Inbox UI: the Filed list (it was empty before), links to the topic or epic, "by Claude Code", back to inbox
- [ ] Headless `claude -p` triage run: a temporary `CLAUDE_CONFIG_DIR` has no login, and copying credentials into it was refused; needs the lead's call
- [ ] "File with Claude Code" button (screens.md 8.2): needs a way to run a command in the terminal dock

## M8 - Workbench layout and agent browser
- [ ] Workbench layout of workbench.md 4.8 - built on `gpui-engineer/m8-workbench` (Windows; macOS and Linux by cross-target clippy only): layout model, persistence and M7 migration, areas, area headers and view tabs, view and area toggles, Layout and view menus, page groups (up to three) and their tab bars, resize handles with snap to hide, drag and drop with drop zones and preview, Inbox and Knowledge as views, the chat web view moved (never recreated) and hidden during drags, undo of every layout step (Mod+Z and "Undo layout change"), the move toast, Reset layout (menu, palette, toast, Settings), narrow folding. Left: dragging and dropping by hand (needs real input), the overflow chevron of view tabs, Linux/macOS runs
- [ ] Agent browser (ADR 0010) - built on `integration-engineer/m8-agent-browser` (Windows): crate `acta-browser`, the Browser view, `acta mcp browser` with 9 tools, setup's second server and allow rule, the debugging-port check. Verified through the real app and 2 headless Haiku runs. Left: Stop/Resume/Done by hand, a real sign-in, the blocked-sites setting, macOS and Linux (Claude in Chrome there)

## M7 - Polish
- [x] Design review of every screen against DESIGN.md (6 blockers, 21 should, 8 nice)
- [ ] Review findings applied on `gpui-engineer/m7-polish` (Windows; macOS and Linux by cross-target clippy only): pane sizes and the sidebar giving way at 880 x 560, resize handles, keyboard focus ring, Claude Code status and Settings section, damaged database vs lost key, measured property labels, entry menu, empty states, palette, knowledge, inbox, dialogs, terminal padding, first run, tabs, Keyboard shortcuts page, Updated to indicator. Left: unread dots (read markers, data request 1), the two-line row form under 520, "File with Claude Code", the document menu's Go to heading / Save as file / Move, the update indicator's ready and failed states, the auto-memory setting, the pseudo-language switch
- [ ] Performance pass, empty and error states, onboarding
- [ ] Documentation

## Folder sets and commit modes (ADR 0011)
Specified on `integration-engineer/folders-v2-spec` (components-folders.md, screens.md 8.11, layout.md 4.5, catalogue 10.4.3). Built in this order; each step lands with its tests.
- [ ] Store (`acta-core`): schema V3 (name, icon, commit fields, `active_folder_id` on projects, epics and topics); `visible_folders(owner)` replacing `effective_folders` (own, epic's, project's, deduplicated by path identity, each with its origin and an active flag) and the default rule; `set_active_folder`, `set_folder_name`, `set_folder_icon`, `set_commit_settings`, `move_folder`; `set_include_inherited` removed; pointer cleared when the folder is removed or the owner moves out of sight of it; export and import of the new fields and of `active_folder` by UUID; old files import with defaults. Tests: default rule at each level, pointer to an inherited folder, pointer invalidated by remove and by moving a topic to another epic, dedup keeps the nearest, V2 to V3 migration keeps every old primary as the active folder, export round trip with the new fields, an old export imports.
- [ ] Git and launch plan (`acta-core`): `inspect` extended with the default branch; `launch_plan(owner)` returning working directory, `--add-dir` list and left-out folders with their reason (not found, other checkout of the same repository). Tests on temporary repositories with a linked worktree: main checkout left out of a worktree topic's plan, a package folder inside the active checkout kept, a separate repository kept, own folders never left out.
- [ ] Claude Code settings writer (`acta-core`, replacing the auto-memory writer in `acta-cli/src/link.rs`): merge into `.claude/settings.local.json`, ledger in local values, `info/exclude`, tracked-file refusal, user keys kept, exact removal, migration of the `automemory.` markers. Tests: each mode writes exactly the ADR 0011 JSON; switching modes adds and removes only ledger entries; a user's own `attribution`, `env` and rules survive every transition; removal deletes only a file Acta created and left empty; a tracked file is never written; idempotence (writing twice changes nothing).
- [ ] Agent and CLI: briefing line (folders and commit mode, at most 300 chars, before memory titles) with a budget test on a seeded database with long names; the `Commits` section of `using-acta`; `acta hook pre-git` with the `if`-scoped `PreToolUse` handlers in `hooks.json` and a pure `git_guard::check(command, folder, git)` unit-tested on a table of commands (`--no-verify`, `-n`, `-f`, `--force-with-lease`, `+main`, `--delete`, `--mirror`, `git push origin x`, plain `git push` in each mode, default branch with and without opt-in, detached head); the stdio end-to-end test drives `hook pre-git` and `hook session-start` and asserts sizes; `acta folders [list|active|commit]` with `commit` refused under `CLAUDECODE=1`.
- [ ] Terminal launch: start in the active folder with the launch plan; header switcher and the "Restart in ..." / "Restart here" mismatch, including a commit-mode change; never an automatic restart. Tests: launch plan applied to the spawned command line (working directory, `--add-dir` order), mismatch states from session and stored values.
- [ ] UI: folded row with the switcher and git state, the grouped list, active mark, folder settings form, other-checkout line, palette commands, Settings default, catalogue 10.4.3 moved into 10.4. Captures at 468 and 720 in both themes and both languages, against screens.md 8.11.
- [ ] Prove with headless Haiku runs (at most four, ADR 0011 Consequences): automatic commit without a prompt (heredoc message), the briefing overriding the built-in "only when asked", a guard deny reaching the model, `attribution` removing the trailer from a real commit.

## Due dates
Requested by the user on 2026-11-21; specified on `product-designer/due-dates` (components-records.md 5: Due row, due date picker, topic table; layout.md 4.2.1: due marks; copy.md 10.2.1: forms and typed dates; data.md 11 request 16: model and agent contract; catalogue 10.4.4). Built after the branches in flight land, in this order; each step lands with its tests.
- [ ] Store (`acta-core`): additive migration to the next schema version after ADR 0011's V3: nullable `due_on` (`YYYY-MM-DD`, checked) on projects, epics and topics, partial index on `(project_id, due_on)`; `set_due` writing the system event (request 10) and `updated_by`; pure `due_state(due_on, today, open)` shared by UI and agent. Export field `due_on`, format version up by one, the previous version still imports. Tests: migration keeps every row and adds no date; `due_state` across overdue, today, soon, later, a closed topic, an epic with no topic and a complete one; day counts across a daylight-saving change; export round trip with and without dates; an old export imports.
- [ ] Date parser (`acta-core`, pure): the grammar of copy.md 10.2.1 in English and French, `today` passed in. Table-driven tests on every row of the grammar, a weekday typed on that weekday, `31/02`, years 1999 and 2100, `mar` alone and `15 mar`, accents and trailing periods.
- [ ] Agent and CLI (integration-engineer; adds a changelog line to ADR 0005): `update(due)` on T, E and P ids with the exact refusals; `find(due=overdue|week)`; `create` items with `due`; due segments in `brief` (topic, epic, project overview) and so in the session-start briefing; `acta update --due`, `acta find --due`, `--json` fields. Tests: budget tests on a seeded database where every record is dated and overdue; `tools/list` under 7,000 chars; the stdio end-to-end test sets, finds and clears a date. At most one headless Haiku run, to see the agent set a date from a plain request.
- [ ] UI (gpui-engineer): Due row, the picker (input, preview, calendar, keyboard), `D` on pages and rows, palette commands, due marks in the tree with their precedence, the Due column with sort and the two filter segments, re-evaluation at midnight and on resume, catalogue 10.4.4 moved into 10.4. Captures in both themes and both languages: a topic page in each due state, the picker with valid and invalid input, the epic table of screens.md 8.14, the tree at 260 and 200 with an overdue topic, one also unread, and a blocked overdue one. What needs real input (typing into the picker, clicking a day) is reported unverified.

## Settled with the user
- 2026-11-21: working in Acta must mean the fewest logins possible and Claude Code having, as directly as possible, everything it needs: Acta embeds what Claude Code uses (terminal, browser, chat) in the most practical way; it targets Claude Code on a subscription, not the API. Sessions persist so each login happens once.
- 2026-11-20: design direction is inspired by Obsidian, Notion and JetBrains IDEs: a workbench (tree, document-like pages, tool windows for the terminal and the chat panel, search everywhere). A "Project Ledger" direction (parchment, serif, engravings) was briefed and withdrawn by the user the same day; do not revive it. Kept from that brief as functional requirements: Light / Dark / System setting, UI in English (default) and French from a central string table. DESIGN.md is being revised on `product-designer/workbench`.
- 2026-11-20: no spending at all. No code-signing certificate, no Apple Developer membership, no paid runners or services. Builds are unsigned; only free tooling.

- 2026-11-21: v0.3.0 released (M2 store + UI, M3 MCP/CLI/setup, docked terminal). Self-update through the API-free source proven on Windows, macOS and Linux (update-e2e run 44870763799, 0.1.4 -> 0.3.0). Items the M2 UI simplified (memory editing in place, pane resize handles, folder drag reorder, persistent palette recents, revision line counts) move to M7.
- 2026-11-21: v0.4.0 released (workbench layout, agent browser, visual polish, QA fixes). Self-update 0.3.0 -> 0.4.0 proven on Windows, macOS and Linux (update-e2e run 44919159521).
- 2026-11-21: topics, epics and projects can carry an optional due date (one calendar day, no time, no recurrence); specified in the design docs, data model request 16.
