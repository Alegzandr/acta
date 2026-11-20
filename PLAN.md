# Plan

Each milestone ends with a working, tested, released build. Status: `todo`, `doing`, `done`.

| # | Milestone | Release | Status |
|---|-----------|---------|--------|
| M0 | Research, spikes, ADRs, DESIGN.md | - | done |
| M1 | Pipeline: empty window that installs silently and updates itself | v0.1.x | doing |
| M2 | Data model and core UI | v0.2.0 | todo |
| M3 | MCP server, CLI, Claude Code setup; Acta tracks itself | v0.3.0 | todo |
| M4 | Integrated terminal running Claude Code | v0.4.0 | todo |
| M5 | Claude chat panel beside the work (if the ADR 0009 spike holds) | v0.5.0 | todo |
| M6 | Notes inbox and notes-to-topics flow | v0.6.0 | todo |
| M7 | Polish | v1.0.0 | todo |

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
- [ ] CI: fmt, clippy, tests on every push, all three OSes
- [ ] Release workflow on `v*` tags: build all platforms, publish installers and updater artifacts
- [ ] Silent per-user install verified on Windows; macOS and Linux verified as far as CI allows
- [ ] Self-update proven end to end: install v0.1.0, publish v0.1.1, app updates at launch
- [ ] macOS keychain test with ad-hoc signed builds (ADR 0003), and SQLCipher builds on macOS and Linux runners

## M2 - Data model and core UI
- [ ] Encrypted database, key in OS keychain, migrations
- [ ] Domain: Project > Epic > Topic, timeline entries, statuses, current-state summary
- [ ] App shell: sidebar, topic view, timeline, command palette, keyboard navigation
- [ ] Create, edit, reorder, change status; live refresh when another process writes
- [ ] Working folders on projects, epics and topics with inheritance (ADR 0008)
- [ ] Knowledge base (ADR 0007): documents and agent memories on projects, epics and topics, with summaries, revisions, inheritance, roll-up browsing, reading view and editor
- [ ] No data loss (ADR 0006): stable UUIDs, soft delete with trash, automatic snapshots before migrations and daily, self-restore on failed migration
- [ ] Export and import of the open `.acta.json` format from the command palette, with the lossless round-trip test in CI
- [ ] Key-loss recovery path: never overwrite an unreadable database, offer import

## M3 - MCP, CLI, Claude Code setup
- [ ] `acta mcp` (stdio) and `acta <command>` CLI sharing the domain layer
- [ ] Compact responses with enforced size budgets
- [ ] Knowledge tools: list with summaries, outline, read by section, search, section-level writes; the skill routes durable memory to Acta
- [ ] `acta export`, `acta import`, `acta snapshot restore` in the CLI (not exposed over MCP)
- [ ] Hooks, skill, slash commands; `acta setup` one-step install
- [ ] `acta link` at any level; sessions started outside Acta resolve their topic from the folder (ADR 0008)
- [ ] Dogfooding: this project is tracked in Acta from here on

## M4 - Terminal
- [ ] Embedded terminal launching Claude Code in the effective working folder of the topic, epic or project it was opened from (ADR 0008)
- [ ] Rendering, input, resize, scrollback, copy/paste, links

## M5 - Claude chat panel
- [ ] ADR 0009: embedded web view for claude.ai (spike in progress; the milestone exists only if the spike holds)
- [ ] Side panel with a persistent session, toggled from the keyboard

## M6 - Notes
- [ ] Per-project inbox, quick capture
- [ ] Claude Code turns notes into topics and epics; human refines in the UI

## M7 - Polish
- [ ] Design review of every screen against DESIGN.md
- [ ] Performance pass, empty and error states, onboarding
- [ ] Documentation

## Paused on 2026-11-20 - where to resume
Nothing has been pushed to GitHub yet. All agents were stopped mid-task; their work is on disk in the worktrees under `../acta-wt/`.
- `release-engineer/m1-pipeline` (7 commits, complete): needs the reviewer pass that was interrupted, then a merge of `main` into it.
- `gpui-engineer/m1-app` (worktree `m1-app`, nothing committed yet): workspace, crates and assets exist as untracked files, unfinished and unverified. Resume the gpui-engineer on it.
- `gpui-engineer/adr-0009-chat-panel` (worktree `adr-0009`, nothing committed): research was done, the spike had just started. Resume or restart.
- Then: review both M1 branches, merge, first push, iterate CI and the release dry run to green, run `platform-checks.yml`, tag v0.1.0, install, tag v0.1.1, prove self-update.
- Before M2: put a portable Strawberry Perl in a durable folder outside the repo (the scratchpad copy can vanish) and point `OPENSSL_SRC_PERL` at it.

## Settled with the user
- 2026-11-20: no spending at all. No code-signing certificate, no Apple Developer membership, no paid runners or services. Builds are unsigned; only free tooling.
