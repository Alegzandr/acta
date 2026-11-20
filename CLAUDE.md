# Acta

Local-first desktop app that complements Claude Code: a project tracker for the human and a structured, scoped memory for the agent. Rust + GPUI, Windows / macOS / Linux.

## Where things are
- `PLAN.md` - milestones and status. Update it when a milestone moves.
- `DESIGN.md` - design system. UI code must conform to it.
- `docs/decisions/` - ADRs. One short file per decision; read the relevant ones before changing architecture.
- `.claude/agents/` - the team: product-designer, gpui-engineer, integration-engineer, release-engineer, reviewer.
- `spikes/` - throwaway experiments backing the ADRs. Not part of the workspace, never depended upon.
- Building the encrypted storage on Windows needs a native Perl for vendored OpenSSL (ADR 0003). Git's Perl does not work. On the lead's machine a portable Strawberry Perl lives at `F:/dev/acta-tools/strawberry-perl`; set `OPENSSL_SRC_PERL` to its `perl/bin/perl.exe` in the shell that runs cargo. CI runners already have Perl.

## Conventions
- Everything in English: code, docs, commits.
- Commits: imperative subject, no co-author trailers, no generated-by lines.
- Sample and test data uses the name "Alexandre".
- Before any commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Branch workflow: each agent works on its own branch (`<role>/<slug>`) in its own git worktree under `../acta-wt/` and commits there. Agents never push, merge or touch `main`.
- The lead orchestrates: assigns work, has every branch reviewed by the `reviewer` agent (who did not write it), merges into `main`, pushes, and verifies the result (local checks, then CI).
- Agent-facing output (MCP, CLI, hooks) is compact by design: summaries first, pagination, no dumps.
- No plaintext user data or key material on disk, in logs or in process arguments.
- Zero budget: the user pays for nothing. No code signing, no paid services or runners. Builds are unsigned by decision.
- Headless Claude Code runs (`claude -p`) are allowed for testing the integration: they draw on the user's subscription quota, not money. Use Haiku unless the test needs more, keep runs few, and state the count in your report. Never set or use an API key.
- Nothing that cannot be undone without asking the user.
- The lead's machine is in use while agents work. Never send synthetic keyboard or mouse input, never bring a test window to the foreground or take focus. Test windows are driven by window messages or the app's own test hooks, and captured with PrintWindow. What needs real input is left unverified and reported.
- The web view profile of the chat panel (ADR 0009) is the one accepted exception to the plaintext rule: its cookies are managed by the system web view, in Acta's data directory.
