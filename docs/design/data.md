# Acta design: data mapping

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles, the never-list and the review checklist. Section numbers are shared across all parts.

## 11. Data: what every element shows

Nothing on screen is illustrative. Every value comes from the model of ADRs 0005 to 0008 (Project > Epic > Topic, statuses, current state, timeline entries, notes, knowledge entries and revisions, working folders) or is computed from it by a rule stated here.

| Element | Source |
|---|---|
| Tree: projects, epics, topics, order | The records and their stored position, never sorted by status (4.2.1). Folded projects and epics and unfolded "N closed" groups are a preference (4.7). |
| Tree: rows on the path, active project | Computed from the open page's project and epic (as the breadcrumb) and the active project (4.6). |
| Tree: "N closed" | Count of Done and Dropped topics in the group. |
| Tree: unread dot | The read markers of request 1: a topic or knowledge entry whose last write is by another process and is newer than its marker. Folded rows aggregate their topics; the Knowledge row aggregates the project's entries at every level. |
| Inbox count | Notes of the project not yet filed. Shown in the tree only above 0. |
| Knowledge count | Knowledge entries of the project at every level. Shown on the knowledge page, not in the tree (4.2.1). |
| Status glyph and word | The topic's status (2.3). Epics and projects have no status of their own. |
| Progress (epic, project, group row) | Topics Done divided by topics not Dropped, in that epic or project. Computed, never stored. |
| Id (T-42, E-7, K-7) | The per-installation short ids of ADR 0005 and 0006. |
| Updated, "by Claude Code" | `updated_at` exists on every record (ADR 0006). Who made the last write is request 9. |
| Current state, description, title | The topic's and the epic's fields. A project's description is request 14. |
| Timeline entries | Timeline entries: type, author, time, text. |
| System events in the timeline (status changes, creation, moves) | Not recorded today: request 10. |
| Trash rows: who deleted, when, from where | Soft delete exists (ADR 0006); its metadata is request 11. |
| History rows: "Replaced section Storage", "Created from file" | Revisions exist (ADR 0007); the description of each change is request 12. |
| Palette "Recent" | Request 13. |
| Drafts of unsaved editors | Request 15. |
| Entries column of the topic table | Count of timeline entries of the topic. |
| Due property, picker, Due column, due marks | The record's own due date (request 16). "Overdue", "today", "due soon" and the day counts are computed at display from the local date (10.2.1) and the record being open (components-records.md 5), never stored. |
| Overdue and Next 7 days filters, sort by Due | Request 16, over the topics the table lists; the segments exist only while one of them has a date. |
| "Set the due date" system events | Request 10, with the event kind of request 16. |
| Knowledge rows, summaries, kinds, levels, history | `kb_entries` and `kb_revisions` (ADR 0007). |
| Folders, origin label, not-found state | The `folders` table and the inheritance rule of ADR 0011 (own, then epic's, then project's); existence is checked on this machine. |
| Folder name and icon | `folders.name` (empty: the last path segment) and `folders.icon` (empty: automatic, from the git detection below). |
| Active folder, "Active" | The page's `active_folder_id` if it names a folder the page sees, else the default rule of ADR 0011 (first own folder, else the parent's active folder). |
| Git state: branch, "3 changed", worktree icon | Read on this machine, never stored: `.git`, `HEAD` and `commondir` through `agent::git::inspect`; changed files from `git status --porcelain` with a 2 s timeout. |
| "Another checkout of the same repository" | Computed at display and launch: same git common directory as the active folder, different worktree root. |
| Commit mode, "Commit in my name", "Also on main" | `folders.commit_mode`, `folders.commit_as_user` (empty: the Settings default, a local value), `folders.commit_on_default`; the default branch's name from `refs/remotes/origin/HEAD`, else `main` or `master`. |
| "Commits use your git identity: Alexandre" | `git config user.name` read in that folder when the form opens; never stored, never written. |
| "Acta writes these rules..." and its alternatives | The settings ledger (local values) and the state of the folder's `.claude/settings.local.json` (tracked by git, user-set keys). |
| Breadcrumb | The page's project and epic. |
| Tabs | A preference: per window, a list of typed references (a record id, or a fixed page key with its project id where it has one). |
| Terminal header context and switcher | The page the session was started from, the folder it runs in and the commit mode it started with, held by the app while the session lives; compared with the stored values for the mismatch (4.5). |
| Status bar: Claude Code status | `acta setup --check` (ADR 0005) and whether the active project has a working folder. Tooltip: the latest write authored by Claude Code in the project. While sessions run: their count by state, from session presence. |
| Claude Code sessions (status bar, tree glyph, page line, Sessions view, terminal context) | Session presence: one row per live session in `presence.db` (local, not exported, not in snapshots), written by the session's `acta mcp` (ADR 0005 amendment of 2026-11-21). |
| Status bar: version, update | The running build and the updater's state (ADR 0004). |
| Settings: last export, snapshots, trash count, database path | Export history, the snapshot list and the trash of ADR 0006; the storage location of ADR 0003. |
| Settings: folders not found | Every folder record, across projects, whose path is not a folder on this machine. |
| Chat panel | Nothing from the model: it is the website. Acta holds only whether the sign-in help was dismissed (a preference). |

### Data model requests

For the lead. Each is something the design wants and the ADRs do not yet give; the design works without them today, in the way stated.

1. **Read markers.** The unread dot (tree, tabs, topic table, knowledge rows) needs, per record, when this installation last showed it: a `last_seen_at` per topic and per knowledge entry, local and not exported. Without it the dot can only cover changes that arrive while the app is running.
2. **Session presence.** Done (2026-11-21): `acta mcp` keeps a row per session (opaque session id, project and focus, launch token of Acta's terminal, start, last activity, heartbeat, state, last tool name, one line of what the agent logged) in `presence.db`, encrypted with the database key, apart from the main database so heartbeats never reload the pages. Rows expire 90 s after the last heartbeat and are deleted a day later.
3. **Links between records.** Backlinks and a "mentioned in" list, taken from Obsidian, need stable references in text (for example a link form that carries the UUID) or a relation table; the short ids are per-installation aliases and cannot serve after an import. Left out of v1.
4. **Setup check as data.** The four checks of `acta setup --check` and the path of the `acta` command must be callable from the app as structured results, not parsed from CLI text.
5. **Export history.** "Last export: 12 days ago" needs the time of the last export recorded (in the database or the preferences file; it must survive an update).
6. **Folders not found, across projects.** A query over all folder records for the Settings list and for bulk repair; no schema change, but the existence check must run off the UI thread for the whole set.
7. **Preferences outside the database.** `preferences.json` (4.7) is new: plaintext, no user content, holding typed page references for tabs (record ids or fixed page keys) and record ids for tree expansion. The lead should confirm that ids in a plaintext file are acceptable under the rule on plaintext data in CLAUDE.md; if not, tabs and expansion move into the database and only theme and language stay in the file.
8. **Entry count per topic** for the topic table should come from one grouped query, not one per row, to hold the 50 ms page budget with 1,000 topics.
9. **`updated_by` on every record** (human or Claude Code), next to `updated_at`: the Updated property, the current-state meta line and the knowledge rows' author mark all show who made the last write, and the ADRs record an author only on entries and knowledge revisions.
10. **System events.** The timeline shows status changes, creation and moves with their actor and time ("Alexandre set status to Active"). They need an event record per topic (kind, actor, time, old and new value), exported like entries.
11. **Trash metadata.** The trash page shows who deleted a record, when, and where it was, and restores it there: `deleted_at`, `deleted_by` and the original parent and position on every soft-deleted record.
12. **Revision descriptions.** The history list says what each revision did ("Replaced section Storage", "Edited", "Created from file"): a change kind, and the section where there is one, stored with each knowledge revision.
13. **Palette recents.** The last topics opened, per installation: record ids only, so they may live in the preferences file under the same condition as tabs (request 7).
14. **Project description.** Project pages show a description; ADR 0005's `update` takes topic, epic and note ids only. The project needs the field, and the tool or the UI needs to be able to write it.
15. **Drafts in the database.** Unsaved editor text survives navigation and restarts (6.4). It is user content, so it is stored in the encrypted database, keyed by record and field, local and not exported; never in `preferences.json`.

16. **Due dates** (the user's requirement of 2026-11-21: a due date on a topic, an epic or a project).
    - **Model.** One optional field per project, epic and topic: `due_on`, a calendar day stored as text `YYYY-MM-DD`, no time and no time zone (10.2.1). Years 2000 to 2099. No recurrence, no reminder, no notification: Acta shows a date, it does not chase anyone. No time of day, by decision: a plan needs the day; a time adds a time zone to every reading, a second field to every write, and a class of off-by-one bugs, and deadlines with an hour belong in a calendar.
    - **Store.** Additive migration, the next schema version after those already queued (ADR 0011's V3): a nullable `due_on TEXT` column on `projects`, `epics` and `topics`, with a check constraint on the shape; no backfill; a partial index on `(project_id, due_on) WHERE due_on IS NOT NULL` for the filters and `find`. Writes go through one domain function, `set_due(record, Option<Date>)`, that also records the system event (request 10: kind `due`, old and new value) and the `updated_at` / `updated_by` of request 9. "Overdue" and "open" are computed by one pure function, `due_state(due_on, today, open) -> None | Overdue(days) | Today | Soon(days) | Later(days)`, shared by the UI and the agent output so both always agree; `today` is passed in, never read inside, so it is testable.
    - **Export.** `due_on` on project, epic and topic objects, the ISO string or absent; the export format version goes up by one with a schema note and a test for the previous version (ADR 0006); older files import with no dates; import merges per record as today.
    - **Agent** (amends ADR 0005; output stays English and compact; "today" is the local date of the `acta` process):
      - `brief` of a topic adds one segment to its header line, only when there is a date: `· due 11-27 (6d)`, `· due today`, `· due tomorrow`, `· overdue 3d (due 11-18)`; a date in another year is written `2027-02-03`; a closed topic shows `· due 11-18` and is never called overdue. `brief` of an epic gives the epic's own segment on its header and the same segment, shortened to `· due 11-27` / `· overdue 3d`, on the line of each open topic that has a date. The project overview adds one line, only when it is not empty, at most 160 chars, cut at item boundaries: `Overdue: T-42 3d, E-3 2d, T-51 1d +2 · due within 7 days: 4`. The session-start briefing is the `brief` text, so it carries the same segments within its 1,200 chars. Cost: at most 30 chars per line; the 300-token budget of `brief` is unchanged and enforced by the existing renderer.
      - `update` takes `due?`: `"YYYY-MM-DD"` sets, `""` clears. It accepts `P-n` as well as `T-n` and `E-n` for this field (projects have no other `update` field until request 14); on `N-n` it is refused. Ack: `ok T-42 · due 2026-12-15 (in 24d)` or `ok T-42 · due cleared`, so the agent sees the date it set. Natural language is refused, by design, so that the agent's writes are exact: `refused: due must be YYYY-MM-DD (2000-2099) or "" to clear, got "friday"`. Budget 40, unchanged.
      - `find` takes `due?: overdue | week`: open topics and epics whose own date is before today, or before today plus 7 (overdue included), combined with the other filters, ordered by date then by stored order; each line ends with its due segment. `limit=8`, budget 500, unchanged.
      - `create` items of kind topic or epic take `due?` with the same format, so a triage that sets dates is one call. Ack unchanged.
      - `tools/list` grows by about 250 chars of parameter descriptions; the existing test keeps it under 7,000. Budget tests add a seeded database where every record has a date and every topic is overdue.
    - **CLI.** `acta update T-42 --due 2026-12-15`, `--due ""` to clear; `acta find --due overdue|week`; `--json` carries `"due": "2026-12-15"` and `"overdue": true|false`.

Not requested, by decision: priorities, assignees, custom properties, tags or categories on knowledge entries, file-change tracking. None is in the user's direction, and each would make the agent's writes less predictable. Due dates were on this list until the user asked for them on 2026-11-21 (request 16); they are kept to one exact, optional field for that reason.
