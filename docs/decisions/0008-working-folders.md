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
- DESIGN.md needs the folder row, the inherited-from label, the unresolved state and the picker flow before the M2 screens are built.
- Folder settings arrive with M2 (data and UI), CLI and hook resolution with M3, terminal launch with M4.

## Rulings after the design pass (2026-11-20)
- **Terminal session scope.** One terminal session per project stays (DESIGN.md 4.2); sessions are not multiplied per topic. Navigation never restarts or moves a running session. When the session was started from a different topic, epic or folder than the view now shown, the terminal pane says which one it is on, and a palette command "Restart terminal here" restarts it with the current view's folders and focus. Exact surface is designed with M4.
- **Include inherited stays off by default.** A topic in its own git worktree must not also get the main checkout. The list shows which inherited folders are not used.
- **Bulk repair.** After one unresolved folder is located, Acta offers to apply the same root replacement to the other unresolved folders that share the old root. Built with import in M2.
- **Missing additional folder** does not block a launch: it is left out and reported, as the design specifies. Only a missing primary stops it.

## Evidence
None yet. Design decision. `--add-dir` exists in the Claude Code CLI as of the lead's knowledge; to be confirmed against the installed version when M4 is built.
