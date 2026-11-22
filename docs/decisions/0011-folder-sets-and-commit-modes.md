# 0011 - Folder sets, the active folder and commit modes

Status: accepted (design; the Claude Code behaviours marked "to prove" are proven by the end-to-end runs listed under Consequences before the feature ships)
Date: 2026-11-21

## Context
ADR 0008 gives each project, epic and topic an ordered list of working folders: the first is *primary*, the rest *additional*, nearest level wins, and an "include inherited" switch (off by default) adds the parent's folders. The user asked for more on 2026-11-21:

1. Any number of folders per level, each with a name and an icon, and one of them chosen as the *active* folder, switched instantly.
2. Folders are **always** inherited: a topic sees its own folders, its epic's and its project's. No switch.
3. For a folder in a git repository, a commit mode: Manual (default), Commit automatically, or Commit and push automatically; with an automatic mode, an option "Commit in my name, without Co-Authored-By Claude".

Facts checked on 2026-11-21 against the installed Claude Code 2.1.330 (Windows 11) and the current docs:

| Fact | Source |
|------|--------|
| `--add-dir <directories...>`: "More directories Claude can access". Files in additional directories follow the permission rules of the working directory; most `.claude/` configuration is not discovered from them. | `claude --help` (2.1.330); docs/permissions "Working directories" |
| Commit and PR attribution is the `attribution` setting: `{ "commit": string, "pr": string, "sessionUrl": bool }`. Empty `commit` / `pr` hide the attribution text including trailers; `sessionUrl: false` omits the `Claude-Session` trailer and PR link. `attribution: false` is the same as all three, but "older Claude Code versions reject true or false here, so use the object form in settings files". | schema embedded in the 2.1.330 binary (`attribution`, `commitTrailers` resolution) |
| `includeCoAuthoredBy` is deprecated: "Use attribution instead". When both are set, `attribution` decides. | same binary; docs/settings-reference |
| `includeGitInstructions` (default true) puts Claude Code's commit and PR workflow instructions in the system prompt. Those instructions say "NEVER commit changes unless the user explicitly asks you to", "NEVER update the git config", "NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless the user explicitly requests it". | same binary |
| `.claude/settings.local.json`: per user, per project, above `.claude/settings.json` and user settings. Its allow rules apply without workspace trust while untracked; tracked, they wait for trust. Claude Code reloads settings files when they change, permissions included. | docs/settings |
| Where Claude Code reads the local file: in a git repository, at the repository root (for a worktree, the main checkout's root); but next to `.claude/settings.json` in the starting directory on Windows, outside git, or when the root is the home folder. "It still reads a file an earlier version left [in the starting directory] alongside the root file; where both set the same key, the root's value applies, and permission rules from both files apply." | docs/settings "Where Claude Code keeps the local file" |
| Permission lists merge across files; rules are evaluated deny, then ask, then allow; a deny in any scope beats an allow in any other. | docs/permissions, docs/settings |
| Bash rules: `Bash(git commit *)` matches `git commit` and every argument list; `Bash(git push)` with no `*` matches exactly `git push`; a trailing ` *` also matches the bare command. Compound commands must have every part allowed. A rule matches the text Claude writes, so `git -C . push` escapes `Bash(git push *)`: "isn't a security boundary around the program". | docs/permissions "Wildcard patterns", "What a Bash rule doesn't match" |
| A `PreToolUse` hook can deny with `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":...}}`; a hook block takes precedence over allow rules; hooks never bypass deny and ask rules. The handler field `if` (one permission rule, such as `"Bash(git *)"`) runs the hook only on matching tool calls, each subcommand checked. | docs/hooks "Common fields", docs/permissions "Extend permissions with hooks" |
| A settings file can carry an `env` block; in an untracked local file it applies without trust. | docs/settings |

## Decision

### 1. Folders: a set per level, always inherited, one active
- **Any number of folders per level**, ordered. Each has a **name** (default: the last segment of its path, stored as empty so it follows a Locate) and an **icon**: *Automatic* (default: `git-branch` for a linked git worktree, `folder-git-2` for a repository, `folder-open` otherwise) or one of twelve Lucide icons: `folder-open`, `folder-git-2`, `git-branch`, `code`, `book-open`, `package`, `server`, `globe`, `smartphone`, `database`, `flask-conical`, `palette`. `folder-x` replaces any icon while the folder is not found.
- **Always inherited.** The folders a page sees are its own, then its epic's own, then its project's own (an epic: its own, then its project's), each group in its order; a path present at two levels is shown once, at the nearest. The "include inherited" switch is gone. Inherited folders are shown with their origin and edited at their own level.
- **One active folder per page.** The active folder is where Claude Code starts (the session's working directory). Every other folder the page sees is an additional folder (`--add-dir`), with one exception below. A page stores an optional pointer to any folder it sees, inherited ones included. Without a pointer (or when it points at a folder the page no longer sees), the active folder is the default: the page's first own folder if it has own folders, otherwise the parent page's active folder. So a topic with nothing of its own follows its epic's choice, and a topic given its own worktree starts there with no extra step. "Primary" disappears from the model and the UI.
- **Switching is instant** from the folder row on the page, the folder switcher in the terminal header, and the palette ("Switch active folder..."). It is one undoable write.
- **A running terminal session is never restarted by a switch.** The terminal header marks the mismatch (layout 4.5: the context turns `text.primary`) and offers "Restart in <name>"; with a process running it asks first (6.4). Changing the commit mode of the folder a session runs in marks the same mismatch, since the instruction the session received is out of date (the permission rules and the guard, below, apply at once).
- **The worktree concern** (a topic in its own git worktree now also receives the main checkout of the same repository). Leaving it to the active folder and a label is not enough: the agent would see every file twice and could edit the wrong copy. Rule, with no setting: at launch, **a folder inherited by the page that is another checkout of the same git repository as the active folder (same git common directory, different worktree root) is left out of `--add-dir`**, and the list says so on that row. A folder inside the active folder's checkout, or containing it (a monorepo package and its root), is still passed. Own folders are always passed: putting two worktrees on one topic is a deliberate choice. No per-folder "hide here" exception is added; revisit if dogfooding shows another case.
- **The briefing labels it.** The session-start briefing carries one line naming the active folder and the others: "Work in embed-terminal (active). Also open: acta-docs, acta (reference)." Names, not paths.
- **Reverse resolution is unchanged** (ADR 0008): the launch directory of a session started outside Acta maps to the deepest attached folder and focuses its owner. Such a session's working folder is its launch directory; the active pointer is a launch setting for Acta's terminal only.
- **Path identity, not-found, Locate and bulk repair** are unchanged from ADR 0008.

### 2. Git detection
For each folder the page shows, Acta reads, off the UI thread, whether the folder is in a git repository, whether it is a linked worktree, its branch (or detached commit) and the default branch, from the files (`.git`, `HEAD`, `commondir`, `refs/remotes/origin/HEAD`), with the existing `agent::git::inspect` and no process. The working tree state (changed files) comes from `git status --porcelain` run with a 2 s timeout, when the page opens and when the window regains focus; without a `git` command, or past the timeout, it is simply not shown. The UI shows branch and changed-file count discreetly (components-folders.md). **Default branch**: the target of `refs/remotes/origin/HEAD` when it exists, else `main` if that branch exists, else `master` if it exists, else none.

### 3. Commit modes
A per-folder setting, offered only for a folder in a git repository: **Manual** (default), **Commit automatically**, **Commit and push automatically**. It applies to sessions whose working directory is that folder (the active folder for Acta's terminal, the launch folder otherwise). Commits in an additional folder are always manual. Two options belong to the automatic modes:

- **"Commit in my name, without Co-Authored-By Claude"**: per folder, with a default in Settings > Claude Code (off on a new install). It writes the `attribution` object below. Commits always use the user's own git identity: Acta and the instructions never set `user.name` or `user.email`; with no identity configured, git refuses the commit and Claude Code reports it. The folder's form names the identity git will use ("Commits use your git identity: Alexandre"), read with `git config user.name`.
- **"Also on <default branch>"**: off by default. Without it, automatic commits and pushes are refused on the repository's default branch; the agent asks the user instead. Any other branch is allowed.

**What each mode tells Claude Code (instruction).** The skill `acta:using-acta` gains a "Commits" section with the rules; the briefing carries only the folder's mode, so the rules are not repeated each session:
- Briefing, Manual: "Commits in embed-terminal: only when the user asks." (Claude Code's own instructions already say this.)
- Briefing, automatic: "Commits in embed-terminal: automatic, standing request from the user; not on main. See using-acta." With push: "automatic, then git push; not on main."
- Skill, automatic: after each completed change whose checks pass (run the project's tests and linters first), commit it: stage only the files of that change by path, one logical change per commit, message in the repository's style (read `git log --oneline -5`), else an imperative subject under 72 characters and a body saying why. Never commit failing work, secrets or generated noise; never `--no-verify` or `-n`; never amend or rewrite history; never change git config; if a hook fails, fix the cause and commit again, or stop and tell the user. Put the short hash in the next Acta `log` entry.
- Skill, commit and push: after each commit, run exactly `git push`, with no arguments. If it fails for any reason (no upstream, rejected, authentication, anything asking for input), stop pushing for the rest of the session and tell the user what git said. Never add a remote, set an upstream, force, delete, push tags or another branch, or retry with other options.

**What Acta writes to Claude Code's settings.** In the folder's `.claude/settings.local.json` (the starting directory, which Claude Code reads on Windows and, alongside the repository-root file, on macOS and Linux). Manual writes nothing. Commit automatically writes:

```json
{
  "permissions": {
    "allow": ["Bash(git add *)", "Bash(git commit *)"],
    "deny": [
      "Bash(git * --no-verify*)",
      "Bash(git push *--force*)",
      "Bash(git push -f*)",
      "Bash(git push * -f*)"
    ]
  }
}
```

Commit and push automatically adds `"Bash(git push)"` (exact: no arguments) to `allow`, and an `env` block so a push that needs a sign-in fails instead of waiting for input or opening a window:

```json
{ "env": { "GIT_TERMINAL_PROMPT": "0", "GCM_INTERACTIVE": "never" } }
```

"Commit in my name, without Co-Authored-By Claude" adds:

```json
{ "attribution": { "commit": "", "pr": "", "sessionUrl": false } }
```

The object form is used, not `false`, because older versions reject the boolean; `includeCoAuthoredBy` is deprecated and never written.

Push needs nothing else: Claude Code's permission system can scope a rule to one folder only through the file the rule lives in, which is exactly this per-folder local file, so the exact `Bash(git push)` rule allows pushes there and nowhere else. Any other spelling of a push (with a remote, a branch or options) is not matched and Claude Code asks the user, which is the intended behaviour.

**Writing rules (never clobber).** One writer in `acta-core` (it absorbs the `autoMemoryEnabled` writer of ADR 0005, now in `acta-cli/src/link.rs`): parse the file (refuse to touch it if it is not a JSON object), add only what is absent, never change a key or rule the user set (an existing `attribution` or `env` variable of the user's is left alone and the folder's form says so), write atomically, and record in a ledger exactly which keys, rules and variables Acta added and whether it created the file. Changing the mode rewrites only ledger entries; Manual, removing the folder, `acta link --remove` and `acta setup --remove` take out exactly what the ledger lists and delete the file only if Acta created it and it is now empty. A file Acta creates is excluded through the repository's `info/exclude` (never a tracked `.gitignore`, never the global excludes). If the file is tracked by git, Acta does not write it and the form says "Claude Code will ask before each commit". On macOS and Linux, if the repository-root local file sets `attribution` itself, that value wins (Claude Code's rule); the form says so.

**The guard (enforcement, not only instruction).** The plugin's `hooks.json` gains two `PreToolUse` handlers, `matcher: "Bash"`, with `if: "Bash(git commit *)"` and `if: "Bash(git push *)"`, both exec form `acta hook pre-git`. It exits silently, before touching the keychain, unless the hook's `cwd` is inside an attached folder (the salted-hash pre-check of ADR 0005). Then, for a folder in an automatic mode, it parses the command and denies with a one-line reason when: the command skips hooks (`--no-verify`, `-n` on commit); the branch is the default branch and the folder has not opted in; a push is forced or rewrites (`-f`, `--force*`, a `+refspec`, `--delete`, `--mirror`). In Commit and push automatically it also denies a push with any argument ("run exactly git push"). In Commit automatically a plain push is left to the normal flow, so Claude Code asks the user. It never answers "allow", so it can only make Claude Code stricter. In Manual it does nothing: the user may still ask Claude Code to commit, and Claude Code asks as it does today. Its output is at most 200 chars.

**Never set by the agent.** No MCP tool changes a commit mode, the active folder or a folder's settings. The CLI command that does (`acta folders commit`) refuses when `CLAUDECODE=1`: "Commit modes are set by the user in Acta." The refusal reads the environment, so it stops the usual case, not a determined one (an agent could unset the variable); what holds is that `acta folders commit` is not allowed by any rule Acta writes, so Claude Code still asks the user before running it.

### 4. Data
Schema step V3, additive:

```sql
ALTER TABLE folders ADD COLUMN name TEXT NOT NULL DEFAULT '';          -- '' = last path segment
ALTER TABLE folders ADD COLUMN icon TEXT NOT NULL DEFAULT '';          -- '' = automatic; else one of the 12 names
ALTER TABLE folders ADD COLUMN commit_mode TEXT NOT NULL DEFAULT 'manual'
    CHECK (commit_mode IN ('manual', 'commit', 'push'));
ALTER TABLE folders ADD COLUMN commit_as_user INTEGER;                  -- NULL = the Settings default
ALTER TABLE folders ADD COLUMN commit_on_default INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN active_folder_id INTEGER REFERENCES folders(id);
ALTER TABLE epics    ADD COLUMN active_folder_id INTEGER REFERENCES folders(id);
ALTER TABLE topics   ADD COLUMN active_folder_id INTEGER REFERENCES folders(id);
```

`include_inherited_folders` stays in the schema (additive rule), is no longer read or written, and is ignored on import. The Settings default for "commit in my name" and the settings-file ledger are local values (`meta`, `local.` prefix: not exported); the ledger replaces the `automemory.<path>` markers, migrated on first run.

**Export (ADR 0006).** A folder exports `name`, `icon`, `commit_mode`, `commit_as_user` and `commit_on_default` with its path, which stays marked `local`; a project, epic or topic exports `active_folder` as the folder's UUID. Old files import with the defaults. Acta writes a folder's Claude Code settings only once that folder exists on this machine (at import, or when it is located); the import preview counts folders with an automatic commit mode.

**Migration from today's data.** No pointer is set: the default rule makes each page's active folder its old primary (first own folder, else the inherited primary). Commit modes start at Manual, which is today's behaviour. The one visible change is inheritance: a page with own folders now also passes its parents' folders, except other checkouts of the same repository. The release notes say so.

## Alternatives
- Keep nearest-wins with an opt-in switch: reversed by the user.
- A per-folder "hide here" exception: more state and UI for the one case the same-repository rule already settles.
- Always pass every visible folder and rely on the briefing label: the agent searches and edits both checkouts.
- `--settings <json>` on every terminal launch instead of the local file: exact for Acta's sessions, but sessions started outside Acta would not get the rules, it adds a second mechanism, and it would put more on the command line (ADR 0008 ruling). Kept as a fallback if the starting-directory file stops being read.
- `includeGitInstructions: false` to remove the built-in "never commit unless asked": it also removes the built-in safety rules (no `--no-verify`, no config changes, no force push to main). The briefing states the user's standing request instead; to prove.
- Allow rules only, no guard: a rule matches text, not intent; the guard parses the command and knows the branch. Guard only, no rules: every automatic commit would prompt.
- Setting `user.name`/`user.email` or a commit author for Claude: the user wants their own identity; Acta never edits git config.
- Protecting a configurable list of branches: the server enforces its own protections (a rejected push stops the agent); the default branch is the one that matters locally.

## Consequences
- Supersedes in ADR 0008: "primary" (now active, with a pointer and a default rule), nearest-wins and include-inherited (now always inherited, with the same-repository exclusion), and the ruling "Include inherited stays off by default". ADR 0008 keeps path identity, reverse resolution, not-found and repair, the empty case and the process-argument ruling.
- Amends ADR 0005: hooks gain the `PreToolUse` `pre-git` guard (scoped by `if`, so the hot path is untouched: it spawns only for `git commit` and `git push`); the session-start briefing gains the folders and commit line, at most 300 chars, written before memory titles, so titles are dropped first under the 1,200-char budget; the skill gains a "Commits" section (the skill stays under 80 lines).
- The settings writer touches user repositories. Ledger, exclusion, the tracked-file refusal and exact removal are what make it safe; each is a test.
- To prove before shipping, with headless Haiku runs (at most four): an automatic-mode session commits without a prompt using `git commit -m "$(cat <<'EOF' ... EOF)"` (compound and heredoc matching of `Bash(git commit *)`); the briefing overrides the built-in "never commit unless asked"; the guard's deny reaches the model and blocks a `--no-verify` commit; `attribution` with empty strings removes the trailer from a real commit. `GCM_INTERACTIVE=never` is read from Git Credential Manager's documentation, not run.
- Revisit if Claude Code stops reading the starting-directory local file in a repository (then fall back to `--settings`), or gains per-directory permission scopes.

## Evidence
Read, not run: docs/settings, docs/permissions, docs/hooks (code.claude.com, 2026-11-21); the `attribution`, `includeCoAuthoredBy` and `includeGitInstructions` schema and the built-in git instructions, read from the installed 2.1.330 binary; `claude --help` for `--add-dir` and `--settings`. No headless run was made for this decision.

Run on 2026-11-21 (`integration-engineer/git-core`), four headless `claude -p --model haiku` runs, Claude Code 2.1.330 on Windows 11, each in a temporary repository on a feature branch with Acta's plugin from `--plugin-dir`, `--setting-sources project,local`, `--permission-mode acceptEdits` and `--no-session-persistence` (the login is a file in the default configuration directory, so the runs used it with user settings left out), settings written by `acta folders commit`:
- Commit automatically, in my name, "create hello.txt and commit it": `printf ... && git add hello.txt && git commit -m "..."` ran with no permission denial; the commit has no trailer.
- Same mode, message passed as `git add notes.txt && git commit -m "$(cat <<'EOF' ... EOF)"`: ran with no denial (compound and heredoc match `Bash(git commit *)`); no trailer.
- Commit automatically, asked to run `git commit -n`: the `pre-git` guard denied it; the model quoted the reason and committed again without `-n`.
- Control, Commit automatically without "in my name", same prompt as the first: the commit carries `Co-Authored-By: Claude ...`. So the empty `attribution` is what removed it. Haiku also tried `git -c user.name=... commit`, which `Bash(git commit *)` does not match: it was refused (in `-p`, a prompt is a denial), and it recovered with a plain commit.
Guard hardening after review (2026-11-21): the guard reads through shell keywords and groups (`{ }`, `!`, `if`/`while` bodies, `time`), wrappers (`env` with options, assignments and `-S`, `command`, `builtin`, `exec`, `nohup`, `nice`, `timeout`, `stdbuf`, `setsid`, `sudo`, `doas`, `xargs`), `bash|sh|zsh -c` strings and `eval`; denies a commit or push with any `-c`, `--config-env` or `GIT_CONFIG_*` set in the command, an inline alias, arguments supplied by `xargs`, and anything it cannot read that mentions commit or push ("run git directly"); resolves config aliases with `git config --get alias.<name>`; treats `--prune` as deleting; in push mode also refuses a plain push whose upstream is the default branch; and refuses when HEAD or the default branch cannot be read (reftable). Its handlers match `Bash(git *)`, git by path, and the commands Claude Code does not strip before matching (`env`, `sudo`, `exec`, `eval`, `xargs`, `bash`, `sh`, `zsh`); it returns before reading any file unless a commit, a push, an alias or an unreadable git call is there. Forms none of these handlers match (a script file, a runner such as `watch`) also match none of the allow rules, so Claude Code asks.
A fifth run, after the hardening, same isolation: asked to run exactly `{ git add todo.txt && git commit -n -m "Add todo"; }` in Commit automatically, the guard denied it with the `-n` reason, the model quoted it and made no commit.
Second review (2026-11-21): the guard judges each commit or push in the folder it acts in (`cd`, `pushd`, `-C`, `--work-tree`, `--git-dir`, `env -C`, `sudo -D`; both the hook's directory and where the `cd`s lead, since a subshell's `cd` may not apply), with that folder's commit settings from the store's columns; a directory it cannot work out, a manual folder or an unlinked one targeted from an automatic session are refused. In a session the pre-check links, a store or folder Acta cannot read is refused (fail closed); an alias lookup has 2 s. Any variable on a commit or push other than LANG, LC_*, TZ and TERM is refused (HUSKY, SKIP, LEFTHOOK, PRE_COMMIT_ALLOW_NO_CONFIG...). The columns are the source of a folder's commit settings; the ledger records what was written; one path attached at several levels takes the strictest settings (lowest mode, each option only if all have it); a human import, a Locate and a bulk repair bring the files in line.
Run on 2026-11-22 (`integration-engineer/folders-agent`), one headless Haiku run, same isolation plus `--strict-mcp-config`, Commit automatically in my name, on a feature branch, prompt "Create a file greeting.txt ... Then tell me what you did." (no mention of committing): the briefing reached the model (it quoted it) but it did not commit: "The Acta briefing mentions automatic commits, but you didn't ask for one in this request." So a line that names the mode is not enough against the built-in instruction. The briefing line now states the action: "Commits here: commit each finished change without being asked (the user's standing request); not on main." (with push: "commit and push each finished change ..."); the skill's Commits section uses the same words. A second run the same day, same isolation and prompt, with this wording: Haiku created the file and committed it on the feature branch without a prompt or a denial ("I committed without asking because the session's project briefing says to commit each finished change on this branch"); the commit has no trailer. Proven: the briefing line, worded as the action, overrides the built-in "only when asked"; `includeGitInstructions` stays on and `--append-system-prompt` is not needed.

## Addendum (2026-11-21): pull and branch switching from the folder row
The user asked for a Pull button and a branch selector on git folders. These are human actions run by Acta, never by Claude Code.
- **Pull:** `git pull --ff-only` on the current branch with its existing upstream, off the UI thread, with `GIT_TERMINAL_PROMPT=0`. A non-fast-forward, a missing upstream, a sign-in prompt or local changes that would be overwritten stop it with a short message and nothing changed. The button shows "behind N" when `git fetch` (run on demand, never in the background without the user) reports it.
- **Branch selector:** lists local branches (and remote branches as "track"), current one checked; switching runs `git switch <branch>` (or `git switch --track origin/<b>`). If the working tree has changes git would refuse to carry, Acta says so and changes nothing: no stash, no force. "New branch..." creates from the current HEAD with `git switch -c`.
- A running Claude Code session in that folder is told nothing automatically; the terminal header shows the new branch, and switching while a session is working asks for confirmation first.
- Strings, wireframe and tests are part of the folder-sets implementation (components-folders.md is updated with it).

## Addendum (2026-11-21): live files
ADR 0008 "Live files" applies to folder sets: a followed folder keeps its name, icon, commit mode and every page's active pointer (they hang on the record, not the path); a name left empty follows the new last segment. The git state is refreshed when the folder monitor reloads the pages and on window focus, never by a watcher on `.git`; `git status` runs with `--no-optional-locks` and `GIT_OPTIONAL_LOCKS=0`. When the settings ledger is keyed by path, it follows the folder in the same step as the auto-memory marker.
