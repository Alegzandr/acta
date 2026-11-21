# The `.acta.json` export format

ADR 0006 makes this format a public contract: a file written by any Acta can be read without Acta, and imported by any later Acta. This page is its schema. The code is `crates/acta-core/src/store/exchange.rs`; a test per past version is added when version 2 exists.

## Version 1

One UTF-8 JSON object, pretty-printed, ending with a newline.

| Field | Type | Meaning |
|---|---|---|
| `format` | `"acta"` | Identifies the file. |
| `version` | integer | `1`. An Acta refuses a file whose version is above the one it writes, and reads every older one. |
| `exported_at` | integer | Unix milliseconds, UTC. |
| `project` | string or null | UUID of the exported project, or null for a whole-database export. |
| `projects`, `epics`, `topics`, `entries`, `notes`, `knowledge`, `folders` | arrays | The records, each array sorted by `uuid`. |

Every record carries:

| Field | Type | Meaning |
|---|---|---|
| `uuid` | string | UUIDv7. The identity used by import; never changes. |
| `created_at`, `updated_at` | integer | Unix milliseconds, UTC. |
| `updated_by` | `"human"` or `"claude"` | Who made the last write. |
| `deleted_at` | integer or null | Set when the record is in the trash. Trashed records are exported. |
| `deleted_by` | `"human"`, `"claude"` or null | |

Records refer to each other by UUID. Short ids (`T-42`) are per-installation and never exported.

- **projects**: `title`, `description`, `position`.
- **epics**: `project`, `title`, `description`, `state` (current state), `position`, `include_inherited_folders`.
- **topics**: `project`, `epic` (null for a topic outside any epic), `title`, `description`, `status` (`draft`, `planned`, `active`, `blocked`, `done`, `dropped`), `state`, `state_updated_at`, `state_updated_by`, `position`, `include_inherited_folders`.
- **entries** (timeline): `topic`, `seq` (number in the topic; null for system events), `kind` (`progress`, `decision`, `discussion`, `system`), `author`, `text`, and for system events `event` (`created`, `status`, `moved`, `state`, `restored`), `old_value`, `new_value`. For `status` the values are status words; for `moved` they are epic UUIDs (null for "no epic"); for `state` `old_value` is the replaced state.
- **notes**: `project`, `text`, `author`, `filed_topic` (UUID of the topic it was turned into or filed under, or null while in the inbox), `filed_at`.
- **knowledge**: `project`, `epic`, `topic` (at most one of the last two is set; neither means the project's own), `kind` (`document`, `memory`), `title`, `summary`, `body` (Markdown), `rev` (current revision number), `created_by`, `revisions`: every revision, oldest first, each with `uuid`, `rev`, `title`, `summary`, `body`, `author`, `change` (`created`, `created_from_file`, `edited`, `replaced_section`, `appended_section`, `moved`, `restored`), `section` (the heading concerned, or `rev N` for a restore), `created_at`.
- **folders**: `project`, `epic`, `topic` (as for knowledge), `path` (absolute, canonical on the machine it came from), `position`, `local` (always true: a path means something only on its machine).

## Import

A merge keyed on UUID, in one transaction, after an automatic `import` snapshot. Unknown records are created. A known record is updated only when the file's `updated_at` is newer; otherwise it is skipped. Nothing is deleted: a newer version may bring a trashed record back, never trash a live one. A revision whose UUID is known is skipped; one whose number is already taken in that entry's local history is kept under the next free number. After the merge an entry's `rev` points at the newest revision holding its current title, summary and body; if none does, that version is saved as a new revision first. An imported timeline entry whose `seq` is already taken in its topic gets the next free number (its UUID is unchanged). Importing the same file twice changes nothing. A dry run reports what would be created, updated and skipped, per kind, and keeps nothing.

The plaintext file is the one place user data leaves the encrypted store; it is written only on an explicit user action, to a path the user chooses.
