# Acta

Local-first desktop app that complements Claude Code: a project tracker for the human and a structured, scoped memory for the agent. Rust + GPUI, Windows / macOS / Linux.

## Where things are
- `PLAN.md` - milestones and status. Update it when a milestone moves.
- `DESIGN.md` - design system. UI code must conform to it.
- `docs/decisions/` - ADRs. One short file per decision; read the relevant ones before changing architecture.
- `.claude/agents/` - the team: product-designer, gpui-engineer, integration-engineer, release-engineer, reviewer.
- `spikes/` - throwaway experiments backing the ADRs. Not part of the workspace, never depended upon.

## Conventions
- Everything in English: code, docs, commits.
- Commits: imperative subject, no co-author trailers, no generated-by lines.
- Sample and test data uses the name "Alexandre".
- Before any commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Every non-trivial change is reviewed by the `reviewer` agent, who did not write it, before it lands on `main`.
- Agent-facing output (MCP, CLI, hooks) is compact by design: summaries first, pagination, no dumps.
- No plaintext user data or key material on disk, in logs or in process arguments.
- Nothing that costs money or cannot be undone without asking the user.
