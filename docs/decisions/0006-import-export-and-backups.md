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
- Amended by the user on 2026-11-22: the trash page offers "Delete permanently..." on a selection and "Empty trash...", each behind a confirmation that names the count, purging the records and their unreferenced blobs at once (components-app.md 5, Trash page; data.md 11, request 25). Snapshots taken before keep them until rotation, and the dialog says so. Ctrl+Z still never destroys anything.

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
Retention keeps the 3 newest `restore` snapshots plus the oldest of the latest run of them (restores with no import or manual snapshot between; daily snapshots, and a migration taken when a restored older schema is reopened, do not break a run). That oldest one is the live state before the user started restoring, so restores in a row can no longer rotate it away. At most 4 restore snapshots are kept.

## Amendment (2026-11-22): backups are encrypted, plaintext export is gone
Decided by the user: a backup must be as simple as choosing a passphrase, and Acta had no users before, so nothing written in the old format needs reading. This supersedes the plaintext `.acta.json` export of Decision 1, its "not encrypted" warning and the "optional passphrase" sentence.

- **One format, `.acta-backup`**: the export document (now called the payload, still versioned, still JSON, still a merge keyed on UUID on import) encrypted with a passphrase. There is no plaintext export and no plaintext import, in the app or the CLI. The payload starts at version 5; versions 1 to 4 only ever existed as plaintext files and are refused with a clear message, and the import code for them is removed.
- **Container: age v1** (https://age-encryption.org/v1, crate `age` 0.11, MIT/Apache-2.0; 0.11 rather than 0.12 because 0.12 brings P-256 code under RUSTSEC-2026-0387, which backups never use) with its scrypt recipient: scrypt N=2^18, r=8, p=1 (256 MiB, about a second) wraps a random file key; the payload is ChaCha20-Poly1305 in authenticated 64 KiB chunks with a marked last chunk (STREAM), so a truncated or altered file is refused before anything is imported. The file starts with age's own magic and version line. A file that asks for more than N=2^20 is refused before any work (no memory exhaustion by a hostile file). Chosen over our own argon2id + XChaCha20-Poly1305 format because it is a published, reviewed specification with independent implementations: `age -d` or `rage -d` opens a backup without Acta, which keeps Decision 1's promise that the data is readable without Acta and without the keychain. Cost: a few dozen crates (RustCrypto and age's localisation stack), all under licences about.toml accepts; scrypt is optimised in debug builds too.
- **Streaming and bounds**: the JSON is encrypted as it is produced and decrypted into the parser as it is read: the whole text is never held, only the records it parses (tested with a counting allocator). The decrypted payload is capped at 512 MiB while it is read (`MAX_IMPORT_BYTES`), and the file size is checked before the passphrase is asked.
- **The passphrase**: at least 8 characters, no other rule; a strength hint (weak, fair, strong) is shown, never enforced. Asked twice for a backup, once for an import. Never stored, logged or passed as an argument: the app reads masked fields and drops them once read; the CLI reads the terminal without echo and refuses without a terminal unless `--passphrase-stdin` is given. A wrong passphrase says so and changes nothing; a damaged or truncated file says so and changes nothing.
- **Claude Code** still never exports, imports or resets: the CLI refuses `acta export` and `acta import` under Claude Code, and nothing is exposed over MCP.
- **Reset Acta** (Settings, Data; requested by the user the same day): a confirmation that says it cannot be undone, counts what is erased (projects, topics, notes, documents, memories, files, contacts, trash included), offers "Back up first...", has "Also sign out of the chat and browser" (off) and, when models are downloaded, "Also delete the voice models" (off), and enables its button only once the word "reset" ("réinitialiser") is typed. It takes Acta's entries out of every folder's Claude Code settings through the ledger (as `acta setup --remove`); when a folder cannot be reached it asks first ("Reset anyway", or Cancel, which puts the reachable folders back). It then stores a random token in the database, writes it with two flags in `reset-requested`, clears the record ids from `preferences.json` (folds, open Overview sections, page tabs naming a record; the chat's state too when signing out) and restarts. At the next start, before the database is opened and under the init lock every opening process takes, nothing is erased unless the database holds the token (a request file written by anyone else is dropped), no Claude Code session is live in presence, and an exclusive-lock probe shows no other connection (in WAL every open connection holds a shared lock, on every system); a database still in use keeps the request for the next start and a toast says so. Then the database, presence, usage, snapshots, the hooks' link index, kept unreadable databases, the key store marker and the key are deleted; if a database appears after the key is deleted, the key and marker are put back. Theme, language, sizes, zoom, the plugin directory and the log stay. On the key-loss screen, where no store can hold a token, the same dialog erases in the app's own process before anything opens the database (folder settings cannot be read there and stay). There is no CLI or MCP path. Code: `acta-core/src/reset.rs`.
