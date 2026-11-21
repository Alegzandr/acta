# Plan

Each milestone ends with a working, tested, released build. Status: `todo`, `doing`, `done`.

| # | Milestone | Release | Status |
|---|-----------|---------|--------|
| M0 | Research, spikes, ADRs, DESIGN.md | - | done |
| M1 | Pipeline: window that installs silently and updates itself; design foundations; app icon | v0.1.x | done |
| M2 | Data model and core UI | v0.2.0 | todo |
| M3 | MCP server, CLI, Claude Code setup; Acta tracks itself | v0.3.0 | todo |
| M4 | Integrated terminal running Claude Code | v0.4.0 | todo |
| M5 | Claude chat panel beside the work (Windows and macOS; link on Linux; ADR 0009) | v0.5.0 | todo |
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
- [ ] Follow-up: per-row shaping cache keyed by row content, skipping rows `Term::damage()` leaves untouched (ADR 0002 rendering plan); the view relies on GPUI's per-frame line layout cache until then

## M5 - Claude chat panel
- [x] ADR 0009 accepted with conditions: claude.ai in the system web view, Windows proven by the spike, macOS unproven, Linux gets a link; M5 opens with the keyboard handoff check, a real sign-in by the user and a macOS build
- [ ] First: keyboard handoff while the page has focus, a real sign-in by the user, a macOS build (the three things the spike could not check)
- [x] DESIGN.md: specify the chat panel (placement, toggle, placeholder under overlays, failure and sign-in help states) - `docs/design/components-app.md`, `docs/design/screens.md` 8.13
- [ ] Side panel with a persistent session, toggled from the keyboard

## M6 - Notes
- [ ] Per-project inbox, quick capture
- [ ] Claude Code turns notes into topics and epics; human refines in the UI

## M7 - Polish
- [ ] Design review of every screen against DESIGN.md
- [ ] Performance pass, empty and error states, onboarding
- [ ] Documentation

## Settled with the user
- 2026-11-20: design direction is inspired by Obsidian, Notion and JetBrains IDEs: a workbench (tree, document-like pages, tool windows for the terminal and the chat panel, search everywhere). A "Project Ledger" direction (parchment, serif, engravings) was briefed and withdrawn by the user the same day; do not revive it. Kept from that brief as functional requirements: Light / Dark / System setting, UI in English (default) and French from a central string table. DESIGN.md is being revised on `product-designer/workbench`.
- 2026-11-20: no spending at all. No code-signing certificate, no Apple Developer membership, no paid runners or services. Builds are unsigned; only free tooling.
