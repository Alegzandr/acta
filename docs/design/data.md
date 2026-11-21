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
| Status bar: Claude Code status | `acta setup --check` (ADR 0005) and whether the active project has a working folder. Tooltip: the latest write authored by Claude Code in the project. |
| Status bar: version, update | The running build and the updater's state (ADR 0004). |
| Settings: last export, snapshots, trash count, database path | Export history, the snapshot list and the trash of ADR 0006; the storage location of ADR 0003. |
| Settings: folders not found | Every folder record, across projects, whose path is not a folder on this machine. |
| Chat panel | Nothing from the model: it is the website. Acta holds only whether the sign-in help was dismissed (a preference). |

### Data model requests

For the lead. Each is something the design wants and the ADRs do not yet give; the design works without them today, in the way stated.

1. **Read markers.** The unread dot (tree, tabs, topic table, knowledge rows) needs, per record, when this installation last showed it: a `last_seen_at` per topic and per knowledge entry, local and not exported. Without it the dot can only cover changes that arrive while the app is running.
2. **Session presence.** To let the status bar say that a Claude Code session is live on a topic right now, `acta mcp` would have to keep a presence row (session, project, focus, heartbeat) while it runs. Until then the status bar reports setup and the last write, and makes no claim about live sessions.
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

Not requested, by decision: due dates, priorities, assignees, custom properties, tags or categories on knowledge entries, file-change tracking. None is in the user's direction, and each would make the agent's writes less predictable.
