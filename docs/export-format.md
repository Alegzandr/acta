# The `.acta-backup` format

ADR 0006 (amended 2026-11-22) makes this format a public contract: a backup written by any Acta can be opened without Acta, and imported by any later Acta. It is the only way data leaves Acta: there is no plaintext export. The code is `crates/acta-core/src/store/backup.rs` (the container) and `crates/acta-core/src/store/exchange.rs` (the payload). This build writes payload version 5.

## Container

A standard age v1 file (https://age-encryption.org/v1, C2SP), binary (not armored), with exactly one scrypt recipient stanza:

- The file starts with age's magic and version line, `age-encryption.org/v1`, then the stanza `-> scrypt <salt> <log N>`, the header MAC, a 16-byte nonce and the payload.
- The passphrase (UTF-8, as typed, at least 8 characters) goes through scrypt with N = 2^18, r = 8, p = 1 to unwrap the random 128-bit file key. Acta refuses a file asking for more than N = 2^20 before running scrypt.
- The payload is ChaCha20-Poly1305 in 64 KiB chunks (STREAM), each authenticated, the last one marked: a truncated or altered file fails as a whole.

Any age implementation opens it with the passphrase: `age -d -o backup.json acta-2026-11-22.acta-backup` (or `rage -d`). The result is the payload below. Keep that file private: it is plaintext.

## Payload

One UTF-8 JSON object, pretty-printed, ending with a newline.

| Field | Type | Meaning |
|---|---|---|
| `format` | `"acta"` | Identifies the file. |
| `version` | integer | `5`. An Acta refuses a payload whose version is above the one it writes. Versions 1 to 4 were plaintext `.acta.json` files (Acta 0.3 to 0.6); no Acta imports them. |
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
- **epics**: `project`, `title`, `description`, `state` (current state), `position`.
- **topics**: `project`, `epic` (null for a topic outside any epic), `title`, `description`, `status` (`draft`, `planned`, `active`, `blocked`, `done`, `dropped`), `state`, `state_updated_at`, `state_updated_by`, `position`.
- **entries** (timeline): `topic`, `seq` (number in the topic; null for system events), `kind` (`progress`, `decision`, `discussion`, `system`), `author`, `text`, and for system events `event` (`created`, `status`, `moved`, `state`, `restored`), `old_value`, `new_value`. For `status` the values are status words; for `moved` they are epic UUIDs (null for "no epic"); for `state` `old_value` is the replaced state; `edited` (an entry changed in place) has its previous kind and text as JSON in `old_value` (`{"kind":"progress","text":"..."}`) and the entry's number in `new_value`.
- **notes**: `project`, `text`, `author`, `filed_topic` (UUID of the topic it was turned into or filed under, or null while in the inbox), `filed_at`, and, when set, `filed_epic` and `filed_by`.
- **knowledge**: `project`, `epic`, `topic` (at most one of the last two is set; neither means the project's own), `kind` (`document`, `memory`), `title`, `summary`, `body` (Markdown), `rev` (current revision number), `created_by`, `revisions`: every revision, oldest first, each with `uuid`, `rev`, `title`, `summary`, `body`, `author`, `change` (`created`, `created_from_file`, `edited`, `replaced_section`, `appended_section`, `moved`, `restored`), `section` (the heading concerned, or `rev N` for a restore), `created_at`.
- **folders**: `project`, `epic`, `topic` (as for knowledge), `path` (absolute, canonical on the machine it came from), `position`, `local` (always true: a path means something only on its machine).

Also:
- **projects**, **epics**, **topics**: `active_folder` (UUID of the active folder) and `due_on` (`YYYY-MM-DD`), each absent when unset. Epics and topics no longer carry `include_inherited_folders`.
- **folders**: `name`, `icon`, `commit_mode` (`manual`, `commit`, `push`), `commit_as_user` (optional), `commit_on_default`. Commit settings are exported but never imported: a folder new here, or given a new path, is `manual`; a known folder keeps its local settings.
- **entries**: the event `due`, with old and new due days (null for none).
- **questions**: the common fields, `project`, `epic`, `topic` (the owner is the topic, else the epic, else the project), `text`, `asked_to` (contact UUID), `expected_on` (`YYYY-MM-DD`), `status` (`open`, `answered`, `dropped`), `answer`, `resolved_at`, `resolved_by`, `decision_entry` (UUID of the Decision entry logged with the answer), `position`, `created_by`.
- **contacts**: the common fields, `project`, `epic` (both null for a contact of all projects), `name`, `job_title`, `company`, `address_note`, `created_by`, `merged_into` (the contact it was merged into, optional).
- **profile**: `name` and `job_title` of the user, in a whole-database export only.
- **epics**, **topics**: `report_to` (contact UUID), `reported_at`, `reported_by`, each absent when unset. **notes**: `report_to`.
- **entries**: the events `report` (old and new are contact UUIDs) and `reported` (the contact's UUID in `new_value` when marked reported, in `old_value` when unmarked).
- **blobs**: one per distinct file content, sorted by `hash`: `hash` (SHA-256 of the content, lowercase hex), `size` (bytes), `media_type`, and `content` (standard base64), absent when the export was made without files.
- **attachments**: the common fields, `project`, exactly one of `note`, `entry`, `topic` (its description) or `epic` (its description), `blob` (the hash), `name`, `size`, `position`, `moved_from_note` (the note a filing moved it from, optional), `created_by`.
- **notes**: `filed_entry`, the entry a note was filed as (optional).
- **knowledge**: `verified_at`, `verified_by`, `state` (`live`, `superseded`, `retired`), `superseded_by` (entry UUID), `context_branch`, `context_folder` (folder UUID), `created_topic` (topic UUID), and `links`, its sources: each with `uuid`, `kind`, `target_uuid` (a record) or `target_text` (a commit or a path), `folder` (a path's folder UUID), `blob`, `created_at`, `created_by`. Each revision has `branch`, the branch of the session that wrote it.
- **kb_pairs**: duplicate and conflict pairs the checker found: `a`, `b` (entry UUIDs), `kind`, `state`, `score`, `created_at`, `created_by`, `resolved_at`, `resolved_by`.

Claude Code session ids and the quality log are local and never exported. A contact reference is always a UUID.

## Import

A merge keyed on UUID, in one transaction, after an automatic `import` snapshot. Unknown records are created. A known record is updated only when the file's `updated_at` is newer; otherwise it is skipped. Nothing is deleted: a newer version may bring a trashed record back, never trash a live one. A revision whose UUID is known is skipped; one whose number is already taken in that entry's local history is kept under the next free number. After the merge an entry's `rev` points at the newest revision holding its current title, summary and body; if none does, that version is saved as a new revision first. An imported timeline entry whose `seq` is already taken in its topic gets the next free number (its UUID is unchanged). A folder whose path could name another computer (`\\server\share`, `//server`, a device path) is left out and reported. A backup whose payload is larger than 512 MiB is refused: by its file size before the passphrase is asked, and while it is decrypted in any case. A wrong passphrase, a damaged or a truncated file is refused before anything is imported. Importing the same file twice changes nothing. A dry run reports what would be created, updated and skipped, per kind, and keeps nothing.

A backup is the one place user data leaves the encrypted store, encrypted with a passphrase the user chooses, written only on an explicit user action, to a path the user chooses.
