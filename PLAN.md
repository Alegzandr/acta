# Plan

Each milestone ends with a working, tested, released build. Status: `todo`, `doing`, `done`.

| # | Milestone | Release | Status |
|---|-----------|---------|--------|
| M0 | Research, spikes, ADRs, DESIGN.md | - | doing |
| M1 | Pipeline: empty window that installs silently and updates itself | v0.1.x | todo |
| M2 | Data model and core UI | v0.2.0 | todo |
| M3 | MCP server, CLI, Claude Code setup; Acta tracks itself | v0.3.0 | todo |
| M4 | Integrated terminal running Claude Code | v0.4.0 | todo |
| M5 | Notes inbox and notes-to-topics flow | v0.5.0 | todo |
| M6 | Polish | v1.0.0 | todo |

## M0 - Research and spikes
- [x] ADR 0001: UI stack - gpui-kit 0.10.0 (GPUI snapshot gpui-pre 0.3.12), proven on Windows; macOS/Linux to prove in M1 CI
- [x] ADR 0002: Terminal embedding - alacritty_terminal 0.26.0 for emulation and PTY, own GPUI element; headless spike proven on Windows with Claude Code
- [ ] ADR 0003: Encrypted storage and key management
- [ ] ADR 0004: Packaging, silent install, auto-update
- [ ] ADR 0005: Claude Code integration (MCP, CLI, hooks, skill, setup)
- [x] ADR 0006: Import, export and backups
- [ ] DESIGN.md

## M1 - Pipeline
- [ ] Cargo workspace, empty GPUI window
- [ ] CI: fmt, clippy, tests on every push, all three OSes
- [ ] Release workflow on `v*` tags: build all platforms, publish installers and updater artifacts
- [ ] Silent per-user install verified on Windows; macOS and Linux verified as far as CI allows
- [ ] Self-update proven end to end: install v0.1.0, publish v0.1.1, app updates at launch

## M2 - Data model and core UI
- [ ] Encrypted database, key in OS keychain, migrations
- [ ] Domain: Project > Epic > Topic, timeline entries, statuses, current-state summary
- [ ] App shell: sidebar, topic view, timeline, command palette, keyboard navigation
- [ ] Create, edit, reorder, change status; live refresh when another process writes
- [ ] No data loss (ADR 0006): stable UUIDs, soft delete with trash, automatic snapshots before migrations and daily, self-restore on failed migration
- [ ] Export and import of the open `.acta.json` format from the command palette, with the lossless round-trip test in CI
- [ ] Key-loss recovery path: never overwrite an unreadable database, offer import

## M3 - MCP, CLI, Claude Code setup
- [ ] `acta mcp` (stdio) and `acta <command>` CLI sharing the domain layer
- [ ] Compact responses with enforced size budgets
- [ ] `acta export`, `acta import`, `acta snapshot restore` in the CLI (not exposed over MCP)
- [ ] Hooks, skill, slash commands; `acta setup` one-step install
- [ ] Dogfooding: this project is tracked in Acta from here on

## M4 - Terminal
- [ ] Embedded terminal bound to the active project's directory, launching Claude Code
- [ ] Rendering, input, resize, scrollback, copy/paste, links

## M5 - Notes
- [ ] Per-project inbox, quick capture
- [ ] Claude Code turns notes into topics and epics; human refines in the UI

## M6 - Polish
- [ ] Design review of every screen against DESIGN.md
- [ ] Performance pass, empty and error states, onboarding
- [ ] Documentation

## Settled with the user
- 2026-11-20: no spending at all. No code-signing certificate, no Apple Developer membership, no paid runners or services. Builds are unsigned; only free tooling.
