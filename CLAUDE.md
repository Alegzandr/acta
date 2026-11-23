# Acta

Local-first desktop app that complements Claude Code: a project tracker for the human and a structured, scoped memory for the agent. Rust + GPUI, Windows / macOS / Linux.

## Where things are
- `PLAN.md` - milestone status and open work only. Update it when a milestone moves; move items that are done and verified to `docs/plan-history.md` (read that only to learn how something was built).
- `DESIGN.md` - design system index (direction, principles, never-list, map of parts); the specs, including the review checklist (`checklist.md`), are in `docs/design/`, load only the part a task needs. UI code must conform to them.
- `docs/decisions/` - ADRs, one file per decision, indexed in its `README.md`. Many are long (5-42 KB): read the `Status:` line and the `## Decision` section first, then only the other sections you need (later `Amendment`/`Addendum` sections at the end can change the decision). Read the relevant ones before changing architecture.
- `.claude/agents/` - the team: product-designer, gpui-engineer, integration-engineer, release-engineer, reviewer.
- `spikes/` - throwaway experiments backing the ADRs. Not part of the workspace, never depended upon.
- Building the encrypted storage on Windows needs a native Perl for vendored OpenSSL (ADR 0003). Git's Perl does not work. On the lead's machine a portable Strawberry Perl lives at `F:/dev/acta-tools/strawberry-perl`; set `OPENSSL_SRC_PERL` to its `perl/bin/perl.exe` in the shell that runs cargo. CI on Windows runs `scripts/ci/windows-perl.ps1` before cargo: it checks the image's Strawberry Perl (`C:\Strawberry\perl\bin\perl.exe`) and sets `OPENSSL_SRC_PERL` for the job; Linux and macOS runners use their system Perl. Locally, keep `CARGO_TARGET_DIR` on a short path (8.3 form if needed): OpenSSL's Configure fails on long paths.
- Dictation's Whisper builds need libclang (`LIBCLANG_PATH`, e.g. `C:\Program Files\LLVM\bin`; CI on Windows sets it with `scripts/ci/windows-libclang.ps1`) and, for the Vulkan backend, `VULKAN_SDK`: a folder with `Include/` (Vulkan-Headers incl. `vulkan.hpp`), `Lib/vulkan-1.lib` and `Bin/glslc.exe`. The LunarG SDK provides all three; CI installs a pinned one with `scripts/ci/vulkan-sdk.ps1`/`.sh`. On the lead's machine a portable minimal SDK lives at `F:/dev/acta-tools/vulkan-sdk` (see its README.txt); set `VULKAN_SDK` to it in the shell that runs cargo. ggml-vulkan's nested CMake fails beyond 260 characters: keep the target dir very short (a `subst` drive over the scratchpad works).

## Conventions
- Everything in English: code, docs, commits.
- Commits: imperative subject, no co-author trailers, no generated-by lines.
- Sample and test data uses the name "Alexandre".
- Before any commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Branch workflow: each agent works on its own branch (`<role>/<slug>`) in its own git worktree under `../acta-wt/` and commits there. Agents never push, merge or touch `main`.
- One sub-agent at a time: the lead runs a single agent (engineer, designer or reviewer) at once, so builds never overload the lead's machine. CI on GitHub may run in parallel.
- The lead orchestrates: assigns work, has every branch reviewed by the `reviewer` agent (who did not write it), merges into `main`, pushes, and verifies the result (local checks, then CI).
- Agent-facing output (MCP, CLI, hooks) is compact by design: summaries first, pagination, no dumps.
- No plaintext user data or key material on disk, in logs or in process arguments.
- Zero budget: the user pays for nothing. No code signing, no paid services or runners. Builds are unsigned by decision.
- Headless Claude Code runs (`claude -p`) are allowed for testing the integration: they draw on the user's subscription quota, not money. Use Haiku unless the test needs more, keep runs few, and state the count in your report. Never set or use an API key.
- Nothing that cannot be undone without asking the user.
- The lead's machine is in use while agents work. Never send synthetic keyboard or mouse input, never bring a test window to the foreground or take focus. Test windows are driven by window messages or the app's own test hooks, and captured with PrintWindow. What needs real input is left unverified and reported.
- Housekeeping: close every Acta (or other) window you start as soon as you are done (WM_CLOSE, verify the process exited). One Rust target dir per agent, on C: under the session scratchpad (short 8.3 path), never inside a worktree on F:; delete it and your worktree when your branch is merged.
- `preferences.json` (theme, language, pane sizes, open tabs and tree expansion) may live outside the encrypted database: it holds settings and opaque record ids or fixed page keys only, never titles, paths, drafts or any content.
- The web view profiles of the chat panel (ADR 0009, `<data dir>/webview`) and of the agent browser (ADR 0010, `<data dir>/agent-browser`) are the accepted exceptions to the plaintext rule: their cookies are managed by the system web view, in Acta's data directory.
