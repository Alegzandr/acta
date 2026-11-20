# 0003 - Encrypted storage and key management

Status: accepted for Windows and Linux. The macOS key store is provisional: the key file is a departure from the product requirement (key in the OS keychain) based on untested reasoning, so the keychain test listed under Consequences runs on a macOS runner in M1 and decides; the keychain wins if it can be made prompt-free.
Date: 2026-11-20

## Context
- One database, read and written concurrently by the GUI, several `acta mcp` servers and short-lived CLI/hook processes. Thousands of rows, Markdown bodies of tens of KB (ADR 0007), full-text search, encrypted at rest, key outside the data directory.
- Versions checked on crates.io on 2026-11-20:
  - `rusqlite` 0.43.0 (2026-11-15) / `libsqlite3-sys` 0.40.0, MIT. Feature `bundled-sqlcipher` compiles SQLCipher 4.20.0 (SQLite 3.54.1, BSD-3) from `sqlcipher-src` =420.0.0 with FTS5 and `SQLITE_TEMP_STORE=2` (temp data in memory). Crypto provider, read in its `build.rs`: OpenSSL (system, or built from source with `bundled-sqlcipher-vendored-openssl`), or CommonCrypto on Apple targets when no OpenSSL is configured. Nothing else natively: the libtomcrypt provider (restored in SQLCipher 4.14) and SQLite3 Multiple Ciphers are only wired for wasm; PRs rusqlite#1726 and #1811 are open. https://github.com/rusqlite/rusqlite
  - Vendored OpenSSL (`openssl-src` 300.6.1, OpenSSL 3.6.3) needs **Perl at build time**. On Windows it must be a native Perl: the Perl shipped with Git for Windows fails in `./Configure` (`Can't locate Locale/Maketext/Simple.pm`); Strawberry Perl works (`OPENSSL_SRC_PERL` selects the binary). NASM is optional (absent: `no-asm`). GitHub's `windows-2025` and `windows-2022` images ship Perl (5.42.0 / 5.32.1) and no NASM. https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md
  - `keyring` 4.2.0 (2026-08-29, MIT/Apache-2.0), default feature `v1`: Windows Credential Manager (generic credential, per user, not per application), macOS login keychain via `SecKeychainAddGenericPassword`/`SecKeychainFindGenericPassword`, Linux Secret Service over `zbus` (pure Rust: no libdbus or libsecret at build time; a running, unlocked Secret Service at run time; none on headless boxes or WSL; KDE Wallet stores UTF-8 text only). https://github.com/open-source-cooperative/keyring-rs
  - `rusqlite_migration` 2.7.0 requires `rusqlite ^0.41` and does not resolve with 0.43. `refinery` 0.10.0 is heavier than the need.
  - Turso 0.8.4: FTS is Tantivy-based, no FTS5, SQL "partially supported", multi-process use undocumented. `libsql` 0.10.0: encryption needs cmake, async API, fork in maintenance. redb 5.0.0 / fjall 3.1.13: one process per database, no SQL, no FTS.
- macOS (ADR 0004): builds are ad-hoc signed and two executables read the key. A legacy keychain item is readable without a prompt only by the application that created it, identified by its code signature; an ad-hoc signature has no stable designated requirement, so every update and the second executable would raise a password prompt, including from a background hook or MCP server. The data-protection keychain needs entitlements that an ad-hoc signature cannot carry. This is reasoning from the API the crate calls plus developer reports ("Always allow only works until I change the code", https://developer.apple.com/forums/thread/734689); not tested by us, see the M1 test below.

## Decision
**Engine.** SQLite through `rusqlite`, encrypted with SQLCipher 4 (AES-256-CBC, HMAC-SHA512 per page; pages, WAL and FTS index all go through the codec).

```toml
[target.'cfg(not(target_os = "macos"))'.dependencies]
rusqlite = { version = "0.43.0", default-features = false, features = ["bundled-sqlcipher-vendored-openssl", "backup"] }
[target.'cfg(target_os = "macos")'.dependencies]
rusqlite = { version = "0.43.0", default-features = false, features = ["bundled-sqlcipher", "backup"] }  # CommonCrypto

[dependencies]
keyring = "4.2.0"
directories = "6.0.0"
getrandom = "0.4"
```

**Perl, weighed.** This engine needs native Perl to build on Windows: already on GitHub runners, one install on a developer machine (`winget install StrawberryPerl.StrawberryPerl`, then `OPENSSL_SRC_PERL`), and 4 min 34 s of clean build against 13 s. The option without that dependency exists and is proven in the spike (feature `ltc`: SQLCipher with libtomcrypt compiled by an 80-line `build.rs`, C compiler only, files interchangeable with the OpenSSL build). It is rejected as the default because we would own a cryptographic build configuration: the first version of that `build.rs`, copied from the wasm reference, would have left Unix builds seeding the page-IV generator from clock jitter instead of `/dev/urandom`, with nothing failing. A stock feature flag maintained upstream is worth four minutes of cached CI time. It stays in the spike as the fallback.

**Key.** 256 random bits from the OS RNG at first run, given to SQLCipher as a raw key (`PRAGMA key = "x'<64 hex>'"`), so no PBKDF2 on open. Stored as 64 hex characters (text, for KDE Wallet). Never in arguments, environment or logs.
- Windows: Credential Manager through `keyring`. Both executables read it silently (proven with two different binaries).
- Linux: Secret Service through `keyring`; if it is unavailable when the database is created, key file.
- macOS: key file, always. `~/Library/Application Support/Acta/data/acta.key`, mode 0600, directory 0700. This is a documented, weaker choice: it protects a database file or snapshot that leaves the machine alone, not against another process of the same user, and not a backup that contains both files. The alternative is a password prompt per executable per update, which also breaks hooks.
- The store used is written once, at creation, in a `keystore` marker file next to the database and never re-decided: a Secret Service that is momentarily unreachable is an error, not a reason to make a new key.

**Open protocol** (one function, the only code allowed to create a key or a database; first run serialised by an exclusive lock on `acta.init.lock`):

| Database file | Key | Result |
|---|---|---|
| absent | absent | generate key, store it, read it back, then create the database |
| absent | present | create the database with that key |
| present | absent | `KeyMissing`: refuse, touch nothing, hand over to ADR 0006 recovery |
| present | present, `SELECT count(*) FROM sqlite_master` fails with `SQLITE_NOTADB` | `KeyWrongOrCorrupt`: refuse, touch nothing |

**Pragmas, per connection, in this order:** `cipher_log_level = NONE`; `key`; `busy_timeout = 5000`; the probe query; `journal_mode = WAL`; `synchronous = NORMAL`; `foreign_keys = ON`. Every write transaction is `BEGIN IMMEDIATE`. No read transaction is held open across UI idle time, so checkpoints are never starved. To add in M2, untested: `journal_size_limit` of a few MiB.

**Change notification.** The GUI keeps one connection on a background thread and reads `PRAGMA data_version` every 100 ms while a window is visible; when the value changes it reloads the visible views. The value changes for commits by other connections only, across processes. No IPC, no file watcher.

**Schema** (ADR 0006, 0007). Every table: `id INTEGER PRIMARY KEY` (local, feeds `T-42` aliases and FTS rowids), `uuid TEXT NOT NULL UNIQUE` (UUIDv7), `created_at`, `updated_at`, `deleted_at` (integer Unix ms; null means live). `STRICT` tables.

**Migrations.** Hand-rolled ladder on `PRAGMA user_version`, each step applied inside one `BEGIN IMMEDIATE` transaction that re-reads the version, after a pre-migration snapshot. Per ADR 0004, a step between adjacent releases is additive only: new tables, new nullable or defaulted columns, new indexes; no rename, drop or change of meaning; code never uses `SELECT *` or positional inserts. A binary that finds a `user_version` above its own keeps working. A `meta` row `min_compatible_version` is raised only by the second release of a two-step destructive change; a binary older than it refuses to write and asks for a restart.

**Full-text search.** FTS5 external-content tables kept in sync by triggers, `tokenize = 'unicode61 remove_diacritics 2'`, results joined to the base table to filter `deleted_at`, excerpts with `snippet()`, order by `rank`. Indexed: topic title, description and current state; timeline entries; notes; document title, summary and current body. Document revisions are not indexed.

**Larger text (ADR 0007).** Default 4096-byte pages (80 reserved per page for IV and HMAC); bodies of tens of KB go to overflow pages, which is fine at this scale. Each revision is a full copy, so revisions dominate file and snapshot size: 1,000 revisions of 30 KB are about 30 MB, times up to 12 retained snapshots. The WAL is checkpointed at the default 1,000 pages (about 4 MB). Measure before adding a retention rule.

**Snapshots (ADR 0006).** `VACUUM INTO '<tmp>'` then rename: one read transaction, so it is consistent while other processes write, compacted, a single file with no WAL, encrypted with the same key. Restore into a live database with the backup API (snapshot as source), so other processes see it as an ordinary commit.

**Location.** Windows `%LocalAppData%\ActaData\acta.db` (not `%LocalAppData%\Acta`, which is the Velopack root that uninstall deletes); macOS `~/Library/Application Support/Acta/data/`; Linux `$XDG_DATA_HOME/acta/data/`. Base directory from `directories::BaseDirs::data_local_dir()`. Unix: directory 0700, files 0600. Windows: the profile ACL inherited from `%LocalAppData%`.

## Alternatives
- SQLCipher with libtomcrypt built by us: no Perl, 13 s build, proven on Windows; rejected as default for the reason above.
- SQLite3 Multiple Ciphers: self-contained crypto, but no maintained rusqlite path (`sqlite3mc-src` is used for wasm only).
- `bundled-sqlcipher` against a system OpenSSL on Windows or Linux: ships or depends on a `libcrypto` shared library.
- Turso, libSQL, redb, fjall, sled with application-level encryption: see Context; none gives multi-process access, FTS5 and whole-file encryption together.
- Passphrase key: 66 ms of PBKDF2 per open with OpenSSL, 471-515 ms with libtomcrypt, paid by every hook.
- macOS login keychain: a prompt per executable per update. Keychain item open to all applications (`security add-generic-password -A`): possibly prompt-free, unverified, and the key would transit through another tool.
- Notification by watching the WAL file, or by local IPC: platform-dependent events, or a discovery and lifecycle protocol, to save at most 100 ms.
- `rusqlite_migration`: incompatible with rusqlite 0.43 today, and a ladder is 15 lines.

## Consequences
- Windows developers need Strawberry Perl; CI needs nothing extra. Linux builds need `perl` and `make` (on GitHub runners). `Swatinem/rust-cache` hides the OpenSSL build after the first run.
- The MSVC link of vendored OpenSSL prints about 800 `LNK4099` (missing PDB) lines as rustc linker warnings. They do not affect `cargo clippy`; a build or test run under `RUSTFLAGS=-D warnings` needs `-A linker_messages` or equivalent. To settle in M1.
- Encryption protects the files at rest and in copies. It does not protect against code running as the same user on any OS; on macOS it does not protect a full-disk backup either. Say so in the documentation.
- Losing the key makes the database unreadable by design; the answer is the export of ADR 0006.
- A CI test must assert `PRAGMA cipher_version` and `cipher_provider`, and repeat the plaintext scan, on all three OSes, so a build that silently links an unencrypted or different SQLite fails.
- To verify in M1 on real runners, because it could not be done here: the macOS CommonCrypto build and the Linux vendored-OpenSSL build; Secret Service present and absent on Linux; on macOS, create a keychain item with ad-hoc build A and read it with a rebuilt ad-hoc build B and with the CLI (expected: prompt, or `errSecInteractionNotAllowed` when headless), then the same with an item created by `security add-generic-password -A`. If that last case is silent, revisit the key file.
- To verify in M2: restore through the backup API while an MCP server holds the database open; `journal_size_limit`; behaviour when the busy timeout is exceeded.
- Revisit when rusqlite gains a non-OpenSSL native provider (#1811), which would remove the Perl requirement.

## Evidence
Spike: `spikes/storage`, run with `cargo run --release -- run` (add `--no-default-features --features ltc` for libtomcrypt). Windows 11 26300, rustc 1.97.1 MSVC, Strawberry Perl 5.42.3 (portable, on PATH for the build only), no NASM. Not run on macOS or Linux. Credential service `acta-spike`, deleted at the end. 28 checks pass with OpenSSL 3.6.3, 29 with libtomcrypt 1.18.2 (the difference is the final credential-deletion check, skipped in the run that kept its database for the cross-provider test).
- Build: clean release build 4 min 34 s with vendored OpenSSL (8.1 MB executable); 12.7 s with libtomcrypt (3.7 MB). With Git's Perl the OpenSSL build fails in 4.5 s.
- Key in Credential Manager, created and read back; a second open reuses it. Keychain fetch 0.5 ms.
- No plaintext: a marker string, `SQLite format 3`, `CREATE TABLE` and the FTS table name are absent from `acta.db`, `acta.db-wal` (20,632 bytes, not checkpointed) and `acta.db-shm`; the same scan finds the marker in an unencrypted control database.
- Wrong key and no key fail with `SQLITE_NOTADB`. With the credential deleted, the open function returns `KeyMissing`, leaves the file byte-identical and creates no key.
- Concurrency: 5 processes, 250 single-row transactions each, no busy error, 1,250 of 1,250 rows present, every (writer, sequence) pair exactly once. 0.34 to 1.3 s in total across runs (950 to 3,700 commits/s).
- Snapshot during those writes: `VACUUM INTO` 4 to 200 ms, backup API 8 to 500 ms; both open with the same key, fail with another, pass `integrity_check`, contain the marker rows and FTS index, and hold a row count between the counts read before and after (for example 679 for a live count going 679 to 748).
- Notification: one `data_version` read costs 2.1 us; own commits do not change it; 20 of 20 commits from another process detected. Latency at a 100 ms poll: median 41-60 ms, max 100 ms; at 25 ms: median 14-16 ms, max 25 ms.
- FTS5: prefix query, diacritics folding (`decision` finds `Décision`), `snippet()` and `rank` work; soft-deleted rows are filtered by the join.
- Cold start, 29 runs after a warm-up: whole process (spawn, keychain, open, key, one query) median 9.1 ms, max 9.5 ms; open + key + query 2.2 ms. First run 15 ms.
- A database written by the OpenSSL build opens in the libtomcrypt build, from a different executable, without a prompt.
