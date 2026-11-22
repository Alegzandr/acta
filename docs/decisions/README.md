# Decisions

Architecture decision records. One file per decision, named `NNNN-title.md`. Most are long: read the
`Status:` line and `## Decision` first, then any dated `Amendment`/`Addendum` sections at the end, which
can change it. The full status is the `Status:` line of each file; this index shortens it.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-ui-stack.md) | UI stack | accepted (macOS and Linux to be proven in CI) |
| [0002](0002-terminal.md) | Terminal embedding | accepted, except point 4 (bundled ConPTY deferred) |
| [0003](0003-storage.md) | Encrypted storage and key management | accepted on all three systems |
| [0004](0004-packaging-and-updates.md) | Packaging, silent install and auto-update | accepted (macOS and Linux paths to be proven) |
| [0005](0005-claude-code-integration.md) | Claude Code integration | accepted (parts to be proven in M3) |
| [0006](0006-import-export-and-backups.md) | Import, export and backups | accepted |
| [0007](0007-knowledge-base.md) | Knowledge base: documents and agent memory | accepted |
| [0008](0008-working-folders.md) | Working folders | accepted, amended by 0011 |
| [0009](0009-claude-chat-panel.md) | Claude chat panel (embedded claude.ai) | accepted with conditions |
| [0010](0010-agent-browser.md) | Agent browser (browser tools for Claude Code inside Acta) | accepted; built in M8 on Windows |
| [0011](0011-folder-sets-and-commit-modes.md) | Folder sets, the active folder and commit modes | accepted (design) |
| [0012](0012-attachments.md) | Attachments on notes, entries and descriptions | accepted ("open in another app" rejected) |
| [0013](0013-claude-code-prerequisites.md) | Ready for Claude Code: detecting and installing what it needs | proposed |
| [0014](0014-voice-dictation.md) | Dictation: local Whisper, on-device only | proposed |
| [0015](0015-memory-quality.md) | Memory quality | accepted |


Template:

```markdown
# NNNN - Title

Status: proposed | accepted | superseded by NNNN
Date: YYYY-MM-DD

## Context
What forces the decision. Facts, with sources and versions.

## Decision
What we do.

## Alternatives
What we rejected and why, one line each.

## Consequences
What this costs us, what to watch, when to revisit.

## Changelog
Optional. One dated line per later change; the change itself is folded into Decision and Consequences,
which always state the current truth.

## Evidence
Spike location and what it actually proved (and on which OS).
```
