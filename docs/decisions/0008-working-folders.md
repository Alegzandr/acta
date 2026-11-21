# 0008 - Working folders

Status: accepted (the `claude` flags named here are to be re-verified against the current CLI in M4)
Date: 2026-11-20

## Context
The user wants to choose the folders Claude Code works in, so that launching Claude Code from the integrated terminal opens it in the right place for a topic, an epic or a project, with inheritance where needed.

ADR 0005 already maps a repository root to a project (`acta link`) so hooks know which project a session belongs to. That is one direction only (folder to project) and one level only (project). Real work is finer: a monorepo where an epic lives in one package, a topic done in its own git worktree, a project spanning two repositories.

## Decision
**Folders are attached at any level.** A project, an epic and a topic each have an ordered list of working folders. The first is the *primary* folder (where Claude Code starts); the others are *additional* folders (given to Claude Code as extra directories it may read and edit).

**Inheritance, nearest wins.** The effective folders of a topic are its own if it has any, otherwise its epic's, otherwise its project's. Same for an epic. Setting folders on a topic replaces the inherited list for that topic only; an "also include inherited folders" switch keeps the parent's as additional folders. The UI always shows where the effective folders come from ("from epic Storage").

**Terminal launch (M4).** Opening the terminal from a topic, epic or project starts Claude Code with:
- working directory = the primary effective folder;
- each additional folder passed with `--add-dir`;
- `ACTA_PROJECT`, `ACTA_EPIC`, `ACTA_TOPIC` set as in ADR 0005, so the session is focused on what it was opened from without any lookup.

If nothing is attached anywhere up the chain, the terminal asks for a folder once and offers to save it at project level. If the primary folder no longer exists, the terminal says so and offers to pick another; it never silently starts somewhere else.

**The reverse direction, for sessions started outside Acta.** When `claude` is launched from an ordinary terminal, hooks resolve the launch directory against all attached folders, most specific first: a folder attached to a topic focuses that topic, to an epic that epic, to a project that project. A directory inside an attached folder matches it; the deepest attached folder wins. This replaces the project-only mapping of ADR 0005 and makes "one git worktree per topic" work with no ceremony: attach the worktree to the topic, and any session started there is on that topic.

**Choosing folders.** Native folder picker from the project, epic or topic view and from the command palette ("Set working folder..."); `acta link [path] [--topic|--epic|--project <id>] [--add]` in the CLI. `acta link` with no arguments keeps its ADR 0005 meaning: attach the current repository root to a project.

**Storage and privacy.** Folder paths are stored in the encrypted database. The plaintext pre-check file that lets hooks exit early in unrelated directories (ADR 0005) holds salted hashes of every attached folder, never the paths.

**Export and other machines (ADR 0006).** Paths are machine-specific. They are exported, marked as local. On import, a path that does not exist on this machine is kept but shown as unresolved, with one action to point it at the right place; nothing else is blocked by it.

## Alternatives
- One folder per project only: breaks on monorepos, multi-repository projects and worktree-per-topic work.
- Merging folders down the hierarchy instead of nearest-wins: a topic in a worktree would also get the main checkout, and the agent would edit the wrong copy.
- Deriving the folder from git branch names: implicit, fragile, and useless outside git.

## Consequences
- Schema (M2): a `folders` table (owner level and id, path, position, include-inherited flag on the owner). Additive.
- ADR 0005 binding is extended: project resolution becomes "deepest attached folder, any level"; `acta link` grows level options; the salted-hash file covers all folders.
- The same folder attached to two topics is ambiguous for the reverse direction: allowed, but then the session gets the project or epic focus and the briefing lists the candidate topics.
- The folder row, the inherited-from label, the unresolved state and the picker flow are specified in `docs/design/components-folders.md` and `docs/design/screens.md` 8.11.
- Folder settings arrive with M2 (data and UI), CLI and hook resolution with M3, terminal launch with M4.

## Rulings after the design pass (2026-11-20)
- **Terminal session scope.** One terminal session per project stays (docs/design/layout.md 4.2); sessions are not multiplied per topic. Navigation never restarts or moves a running session. When the session was started from a different topic, epic or folder than the view now shown, the terminal pane says which one it is on, and a palette command "Restart terminal here" restarts it with the current view's folders and focus. Exact surface is designed with M4.
- **Include inherited stays off by default.** A topic in its own git worktree must not also get the main checkout. The list shows which inherited folders are not used.
- **Bulk repair.** After one unresolved folder is located, Acta offers to apply the same root replacement to the other unresolved folders that share the old root. Built with import in M2.
- **Missing additional folder** does not block a launch: it is left out and reported, as the design specifies. Only a missing primary stops it.
- **Path identity.** A folder is stored as an absolute, canonical path (symlinks and junctions resolved, no trailing separator, no `\\?\` prefix, drive letter upper-case) and shown as stored. Two paths are the same folder when their stored forms are equal, compared case-insensitively on Windows and macOS. The UI, the CLI and hook resolution share one implementation of this.
- **Nothing attached anywhere.** "Set working folder..." then attaches to the project, from whichever view it is used, matching what the terminal prompt saves. Once any level has a folder, the command targets the current view.
- **Paths in process arguments.** `--add-dir <path>` exposes folder paths to other processes of the same user. Accepted: the working directory of the same process already exposes the primary folder, and the rule in CLAUDE.md protects content and key material. Nothing else from the database goes on a command line.

## Evidence
None yet. Design decision. `--add-dir` exists in the Claude Code CLI as of the lead's knowledge; to be confirmed against the installed version when M4 is built.

## Amendment (2026-11-21): what the terminal passes to Claude Code

Verified against `claude --help` of Claude Code 2.1.330 and code.claude.com/docs/en/{cli-reference, sessions, plugins/loading}.

The terminal starts `claude` in the primary effective folder with, in this order: `--add-dir` for each other folder that exists (`--add-dir <directories...>`); `--plugin-dir <data dir>/claude/plugins/acta`; then `--session-id <uuid>` or `--resume <uuid>`. Environment: `ACTA_TERMINAL=1`, `ACTA_PROJECT`, `ACTA_EPIC`, `ACTA_TOPIC` as before, and `ACTA_LAUNCH`, an opaque token per launch that ties the session's presence row to this terminal (ADR 0005 amendment).
- **`--plugin-dir`, always.** A `--plugin-dir` plugin replaces a same-named installed marketplace plugin for that session, silently (plugins/loading, "Name conflicts"). So a session started in Acta has Acta's tools, hooks and skills even if setup never ran, and always those of the running build. The allow rule names the manifest's plugin name, so it still applies. A user who wants it off sets `"acta@inline": false` in `enabledPlugins` (same page). The files are written by `acta setup --files-only` at launch, and before a start if missing.
- **Conversation per page.** Acta keeps, per topic, epic or project, the id of the last conversation started from it (an opaque UUID in the encrypted database, local). A start resumes it with `--resume <id>` when Claude Code still has its transcript (`<CLAUDE_CONFIG_DIR or ~/.claude>/projects/*/<id>.jsonl`; existence only, nothing read), else starts a new one with `--session-id <new v4 uuid>` and records it. `/clear` in Claude Code makes a new conversation; presence carries its id under the terminal's token and the page records it, so the next start resumes that one. `--resume` restores the session's permission mode but not `--add-dir` or `--plugin-dir`, which Acta passes again every time. Setting "Continue the last conversation of each page", on by default; off starts a new conversation every time. Combined with "Restart terminal here", switching topic now brings back that topic's own conversation.
- **Not passed, by decision:** `--model`, `--permission-mode` and anything that bypasses permissions (the user's own settings decide); `--append-system-prompt` (the SessionStart hook already briefs, and system prompts are recorded per conversation and replayed on resume, so a stale focus would stick); `--name` (it replaces Claude Code's generated title in `/resume`, which describes the work better than an id); `--mcp-config`, `--settings`, `--chrome` (the plugin carries the servers; the agent browser is Acta's own).
- Paths in arguments: `--plugin-dir` is a path in Acta's data directory, accepted under the same ruling as `--add-dir`; the UUIDs are opaque. On the `cmd /s /c` shim path the plugin directory is left out if it holds a character `cmd` interprets; an id that is not a UUID is never passed.

The terminal's header names its session "Claude · T-12 Embed the terminal" and adds "waiting for you" while it waits; the Sessions view's "End session" ends the terminal's process (the conversation stays resumable).
