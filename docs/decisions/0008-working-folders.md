# 0008 - Working folders

Status: accepted, amended by 0011 (folder sets, the active folder, always inherited, commit modes)
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

## Live files (2026-11-21)
The user asked that Acta never block renaming or moving a folder and always show its current state.
- **Nothing is held open on a user folder.** On Windows, a directory handle, even opened with `FILE_SHARE_DELETE`, stops every folder above it from being renamed or deleted (checked: a `ReadDirectoryChangesW` watcher on `parent\child` lets `child` be renamed and fails the rename of `parent` with "access denied"). So there is no file-system watcher: the app polls each attached folder every 500 ms with `GetFileAttributesW` (`stat` elsewhere), which leaves nothing open. An identity is read through a handle that shares read, write and delete and is closed at once.
- **Identity.** Each folder's identity is the volume and file id (Windows `FILE_ID_INFO`; Unix `st_dev`, `st_ino`) plus the creation time where the file system keeps one, against an id reused by a newer folder. It is a local value (`folderid.<id>`), never exported: like the path, it means something on this machine only. It is read when a folder is attached or located, and learned by the monitor otherwise.
- **Following.** When a folder with a known identity goes missing, Acta searches, off the UI thread and in at most 4,000 directories, three levels below the nearest folder above the old path that still exists, then below that folder's parent. Exactly one folder with the identity: the record takes its path (stored form), keeping everything else, and what depends on the path follows in the same step: the auto-memory marker of `acta link` and the hooks' pre-check file. No match, several matches, or no identity on record (an import, a file system without stable ids): the folder stays not found, with Locate, as before. A move to another volume changes the identity, so it is never followed silently. A folder deleted and made again under the same path stays attached (the path is what the user chose) and its identity is refreshed. Nothing goes on a timeline (the path is content); the app log says how many folders were relocated, never which.
- **Always current.** A folder appearing, disappearing or being followed reloads every page, so names (the last path segment), the not-found state, the folder list and the terminal's launch use the current path. A failed search is not repeated each tick; regaining window focus searches again and re-reads every identity.
- **What Acta cannot release.** A process's working directory is held the same way: the integrated terminal's shell and Claude Code hold the folder they run in, and Windows refuses to rename it, or any folder above it, while the session runs. That is the user's own session and is not worked around; Acta cannot observe the refused rename, so the UI says nothing. Acta's own process leaves the directory it was started from (it moves to its install folder at launch), so starting Acta from a terminal inside a project does not hold that project.
- **Git.** Reading git state stays file reads with no process (`agent::git::inspect`). When the working-tree count of ADR 0011 is built, `git status` runs with `--no-optional-locks` and `GIT_OPTIONAL_LOCKS=0`, so Acta never takes `index.lock`.
- Sessions started outside Acta while Acta is closed still resolve against the old path until the app runs once and follows the folder (hooks do not search).

## Evidence
None yet. Design decision. `--add-dir` exists in the Claude Code CLI as of the lead's knowledge; to be confirmed against the installed version when M4 is built.

## Changelog
- 2026-11-21: "Live files" added: no handle held on user folders, presence polled, renamed or moved folders followed by identity, Acta leaves its launch directory.
- 2026-11-21: ADR 0011 replaces the primary folder with an active folder chosen per page, makes inheritance unconditional (the include-inherited switch and its ruling are withdrawn; another checkout of the same repository as the active folder is left out instead), and adds names, icons and commit modes. Path identity, reverse resolution, not-found and repair, the empty case and the process-argument ruling stand. `--add-dir` confirmed in `claude --help` 2.1.330.
