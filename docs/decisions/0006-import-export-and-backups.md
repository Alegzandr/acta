# 0006 - Import, export and backups

Status: accepted
Date: 2026-11-20

## Context
The user requires that nothing can ever be lost. Acta's data lives in one encrypted database whose key is in the OS keychain. That design has three loss scenarios the database alone does not cover:

1. The keychain entry is lost (OS reinstall, new machine, profile reset): the database is intact but unreadable.
2. The database is damaged (failed migration after an auto-update, disk error, a bug).
3. The user or the agent deletes or overwrites something by mistake.

An export that only the same installation can read protects against none of the first.

## Decision
Three mechanisms, each answering one scenario.

**1. Export and import in an open format (answers 1, and lock-in).**
- One file, `.acta.json`: versioned, documented, human-readable JSON containing everything - projects, epics, topics, timeline entries, notes, ordering, statuses, authorship, timestamps, and stable UUIDs for every record.
- Scope: everything, or a single project.
- Plaintext by default, because the point is that the file is readable without Acta and without the keychain. The UI says so at export time. Optional passphrase encryption is offered for users who store the file somewhere shared.
- Import is a merge keyed on UUID: unknown records are created, known records are updated only if the imported version is newer, nothing is ever deleted by an import. Importing the same file twice changes nothing. Import runs in one transaction and reports what it created, updated and skipped before the user confirms.
- An older format version is always importable by a newer Acta. A newer file is refused by an older Acta with a clear message.
- Available from the command palette and from the CLI (`acta export`, `acta import`). Not exposed as MCP tools: an agent has no reason to dump or bulk-load the database, and a dump is the opposite of context economy.

**2. Automatic snapshots (answers 2).**
- A consistent copy of the encrypted database is taken before every schema migration, before every import, before every snapshot restore, and once per day of use, with rolling retention (7 daily, 4 weekly, plus the last one of each other kind).
- Snapshots stay encrypted with the same key and live next to the database. Restoring one is a command-palette action and a CLI command.
- If a migration fails, the app restores the pre-migration snapshot by itself and reports it.

**3. Soft delete (answers 3).**
- Deleting a project, epic, topic, entry or note marks it deleted. It can be restored from a trash view for 30 days, then it is purged. Exports include trashed records, flagged.
- There is no "empty trash" or "delete now" in v1: no key press destroys data. Known limit: something sensitive pasted by mistake stays in the encrypted database and its snapshots until retention expires. Revisit if users ask for immediate erasure.

**Key-loss recovery.** If the database exists but its key does not, the app does not create a fresh database over it. It keeps the unreadable file aside, says exactly what happened, and offers to import an export. The first export is suggested once real data exists; the app then shows the age of the last export in settings, without nagging.

## Alternatives
- Export as a copy of the encrypted database only: useless when the key is lost, which is the worst case.
- Storing a recovery key in a file: moves the secret to disk and defeats the keychain.
- Markdown export as the canonical format: pleasant to read but not lossless. May be added later as a one-way export.
- Cloud sync or backup: contradicts local-first and the zero budget.

## Consequences
- Every record needs a stable UUID and an `updated_at` from the first schema. Short ids shown to agents (`T-42`) are per-installation aliases and are not the identity used by import.
- The export format is a public contract. It gets a version number, a schema document in `docs/`, and a test for each past version.
- Invariant enforced in CI: export, import into an empty database, export again - both files are identical.
- A plaintext export is the one place user data leaves the encrypted store. It happens only on an explicit user action, to a path the user chooses.

## Evidence
None yet. The snapshot mechanism depends on the engine chosen in ADR 0003 and is proven there.

## Amendment (2026-11-22): restore snapshots
Retention keeps the 3 newest `restore` snapshots plus the oldest of the latest run of them (restores with no import, migration or manual snapshot between; daily ones do not break a run). That oldest one is the live state before the user started restoring, so restores in a row can no longer rotate it away. At most 4 restore snapshots are kept.
