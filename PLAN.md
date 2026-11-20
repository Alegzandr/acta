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
- [ ] ADR 0001: UI stack (GPUI version, source, component strategy), verified on Windows locally and on macOS/Linux in CI
- [ ] ADR 0002: Terminal embedding
- [ ] ADR 0003: Encrypted storage and key management
- [ ] ADR 0004: Packaging, silent install, auto-update
- [ ] ADR 0005: Claude Code integration (MCP, CLI, hooks, skill, setup)
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

## M3 - MCP, CLI, Claude Code setup
- [ ] `acta mcp` (stdio) and `acta <command>` CLI sharing the domain layer
- [ ] Compact responses with enforced size budgets
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

## Open questions for the user
- Code signing (Windows certificate, Apple Developer membership) costs money. Builds are unsigned until decided.
