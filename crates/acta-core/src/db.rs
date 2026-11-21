//! Opening the encrypted database (ADR 0003): the open protocol, the
//! per-connection pragmas, the migration ladder with its pre-migration
//! snapshot and self-restore (ADR 0006), and change polling.

use crate::error::OpenError;
use crate::fsutil;
use crate::keys::{Key, KeyStore, KeyStores, MARKER_FILE};
use crate::snapshots::{self, SnapshotKind};
use rusqlite::{Connection, ErrorCode, OpenFlags, TransactionBehavior};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// File name of the database inside the data directory.
pub const DB_FILE: &str = "acta.db";
const LOCK_FILE: &str = "acta.init.lock";
/// The WAL is truncated back to this size after a checkpoint.
const JOURNAL_SIZE_LIMIT: i64 = 4 * 1024 * 1024;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// How a connection is used.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The live database: WAL, foreign keys.
    Live { create: bool },
    /// A snapshot file: read only, its journal mode untouched.
    Snapshot,
}

/// Opens one connection with a raw key and applies the pragmas in the order
/// of ADR 0003. A key that does not decrypt the file is `KeyWrongOrCorrupt`.
pub(crate) fn connect(path: &Path, key: &Key, mode: Mode) -> Result<Connection, OpenError> {
    let mut flags = OpenFlags::SQLITE_OPEN_NO_MUTEX;
    match mode {
        Mode::Live { create } => {
            flags |= OpenFlags::SQLITE_OPEN_READ_WRITE;
            if create {
                flags |= OpenFlags::SQLITE_OPEN_CREATE;
            }
        }
        Mode::Snapshot => flags |= OpenFlags::SQLITE_OPEN_READ_ONLY,
    }
    let conn = Connection::open_with_flags(path, flags)?;
    // SQLCipher logs decryption failures to stderr by default; we report them.
    conn.execute_batch("PRAGMA cipher_log_level = NONE;")?;
    // Raw key: no PBKDF2. Must be the first statement that touches the file.
    let mut sql = format!("PRAGMA key = \"x'{}'\";", key.to_hex()).into_bytes();
    let keyed = conn.execute_batch(std::str::from_utf8(&sql).unwrap_or_default());
    sql.fill(0);
    std::hint::black_box(&sql);
    keyed?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // The first real read: where a wrong key surfaces as SQLITE_NOTADB.
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => {}
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::NotADatabase => {
            return Err(OpenError::KeyWrongOrCorrupt {
                db: path.to_path_buf(),
            });
        }
        Err(e) => return Err(e.into()),
    }
    if let Mode::Live { .. } = mode {
        conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA journal_size_limit = {JOURNAL_SIZE_LIMIT};"
        ))?;
    }
    Ok(conn)
}

/// What [`open_or_init`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    Created,
    Existing,
}

/// The open protocol of ADR 0003. The only code allowed to create a key or a
/// database; it never creates either over an existing database file.
///
/// | database | key | result |
/// |---|---|---|
/// | absent | absent | generate, store, read back, create |
/// | absent | present | create with that key |
/// | present | absent | `KeyMissing`, nothing touched |
/// | present | does not decrypt | `KeyWrongOrCorrupt`, nothing touched |
pub(crate) fn open_or_init(
    dir: &Path,
    stores: &KeyStores,
) -> Result<(Connection, Key, Opened), OpenError> {
    fsutil::create_private_dir(dir)?;
    // Serialises first runs between processes (GUI and MCP starting together).
    let lock = fs::File::create(dir.join(LOCK_FILE))?;
    lock.lock()?;
    let result = open_locked(dir, stores);
    let _ = lock.unlock();
    result
}

fn open_locked(dir: &Path, stores: &KeyStores) -> Result<(Connection, Key, Opened), OpenError> {
    let db = dir.join(DB_FILE);
    let marker = read_marker(dir)?;
    let store_err = |e: crate::keys::KeyStoreError| OpenError::KeyStore(e.0);

    if db.exists() {
        let key = match &marker {
            Some(name) => named_store(stores, name)?.load().map_err(store_err)?,
            // No marker: look in every store, and record the one that has it.
            None => {
                let mut found = None;
                let mut last_err = None;
                for store in &stores.0 {
                    match store.load() {
                        Ok(Some(key)) => {
                            found = Some((key, store.name()));
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => last_err = Some(e),
                    }
                }
                match (found, last_err) {
                    (Some((key, name)), _) => {
                        write_marker(dir, name)?;
                        Some(key)
                    }
                    (None, Some(e)) => return Err(store_err(e)),
                    (None, None) => None,
                }
            }
        };
        let key = key.ok_or(OpenError::KeyMissing { db: db.clone() })?;
        let conn = connect(&db, &key, Mode::Live { create: false })?;
        return Ok((conn, key, Opened::Existing));
    }

    let (key, name) = match &marker {
        Some(name) => {
            let store = named_store(stores, name)?;
            (
                key_for_new_db(store.as_ref()).map_err(store_err)?,
                store.name(),
            )
        }
        None => {
            let mut chosen = None;
            let mut last_err = None;
            for store in &stores.0 {
                match key_for_new_db(store.as_ref()) {
                    Ok(key) => {
                        chosen = Some((key, store.name()));
                        break;
                    }
                    Err(e) => last_err = Some(e),
                }
            }
            match chosen {
                Some(c) => c,
                None => {
                    return Err(OpenError::KeyStore(
                        last_err.map_or_else(|| "no key store".into(), |e| e.0),
                    ));
                }
            }
        }
    };
    if marker.is_none() {
        write_marker(dir, name)?;
    }
    let conn = connect(&db, &key, Mode::Live { create: true })?;
    for suffix in ["", "-wal", "-shm"] {
        fsutil::make_private(&PathBuf::from(format!("{}{suffix}", db.display())))?;
    }
    Ok((conn, key, Opened::Created))
}

/// The key a new database gets from a store: the one already there, or a new
/// one, stored and read back before any data is written with it.
fn key_for_new_db(store: &dyn KeyStore) -> Result<Key, crate::keys::KeyStoreError> {
    if let Some(key) = store.load()? {
        return Ok(key);
    }
    let key = Key::generate().map_err(|e| crate::keys::KeyStoreError(e.to_string()))?;
    store.save(&key)?;
    if store.load()?.as_ref() != Some(&key) {
        return Err(crate::keys::KeyStoreError(
            "the key store did not return the key it was given".into(),
        ));
    }
    Ok(key)
}

fn named_store<'a>(stores: &'a KeyStores, name: &str) -> Result<&'a Arc<dyn KeyStore>, OpenError> {
    stores.by_name(name).ok_or_else(|| {
        OpenError::KeyStore(format!(
            "the database's key is kept in `{name}`, which this build does not offer"
        ))
    })
}

fn read_marker(dir: &Path) -> Result<Option<String>, OpenError> {
    match fs::read_to_string(dir.join(MARKER_FILE)) {
        Ok(text) => Ok(Some(text.trim().to_owned()).filter(|t| !t.is_empty())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn write_marker(dir: &Path, name: &str) -> Result<(), OpenError> {
    let mut file = fsutil::create_new_private_file(&dir.join(MARKER_FILE))?;
    file.write_all(name.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

pub(crate) fn user_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
}

/// Runs the ladder up to the end of `steps`. A database that is behind gets a
/// migration snapshot first; if a step fails, that snapshot is restored into
/// the live database and the error says so. A database ahead of this build
/// is left alone (ADR 0003: an older binary keeps working).
pub(crate) fn migrate(
    conn: &mut Connection,
    dir: &Path,
    key: &Key,
    steps: &[&str],
) -> Result<(), OpenError> {
    let start = user_version(conn)?;
    if start >= steps.len() as i64 {
        return Ok(());
    }
    let snapshot = if start > 0 {
        Some(
            snapshots::take(conn, dir, SnapshotKind::Migration).map_err(|e| match e {
                crate::Error::Io(e) => OpenError::Io(e),
                crate::Error::Db(e) => OpenError::Db(e),
                other => OpenError::Io(std::io::Error::other(other.to_string())),
            })?,
        )
    } else {
        None
    };
    let result = run_steps(conn, steps);
    match (result, snapshot) {
        (Ok(()), _) => Ok(()),
        (Err(e), Some(snap)) => {
            let message = e.to_string();
            restore_into(conn, &snap.path, key)?;
            Err(OpenError::MigrationFailed {
                message,
                restored_from: Some(snap.path),
            })
        }
        (Err(e), None) => Err(OpenError::MigrationFailed {
            message: e.to_string(),
            restored_from: None,
        }),
    }
}

fn run_steps(conn: &mut Connection, steps: &[&str]) -> rusqlite::Result<()> {
    for (i, sql) in steps.iter().enumerate() {
        let to = i as i64 + 1;
        // IMMEDIATE and a re-read: two processes cannot both apply a step.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if user_version(&tx)? >= to {
            continue;
        }
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", to)?;
        tx.commit()?;
    }
    Ok(())
}

/// Copies a snapshot over the live database with the backup API, so other
/// processes see it as an ordinary commit (ADR 0003).
pub(crate) fn restore_into(
    conn: &mut Connection,
    snapshot: &Path,
    key: &Key,
) -> Result<(), OpenError> {
    let src = connect(snapshot, key, Mode::Snapshot)?;
    let backup = rusqlite::backup::Backup::new(&src, conn)?;
    backup.run_to_completion(256, Duration::from_millis(5), None)?;
    Ok(())
}

/// Detects commits made by other connections, in this process or another,
/// with `PRAGMA data_version` (ADR 0003). Poll it from a background thread
/// (the GUI does every 100 ms while a window is visible) and reload what is
/// shown when it returns `true`. It holds its own connection, so commits made
/// through the [`crate::Store`] are seen too.
pub struct ChangeWatcher {
    conn: Connection,
    last: i64,
}

impl ChangeWatcher {
    pub(crate) fn new(conn: Connection) -> rusqlite::Result<Self> {
        let last = data_version(&conn)?;
        Ok(Self { conn, last })
    }

    /// `true` once for every batch of commits since the previous call.
    pub fn changed(&mut self) -> rusqlite::Result<bool> {
        let now = data_version(&self.conn)?;
        let changed = now != self.last;
        self.last = now;
        Ok(changed)
    }
}

fn data_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.pragma_query_value(None, "data_version", |r| r.get(0))
}

/// Key-loss recovery (ADR 0006): moves an unreadable database aside, with
/// its WAL and shared-memory files, so a new one can be created next to it
/// and an export imported into it. Nothing is deleted; the returned path is
/// the database file kept aside. Only on an explicit user action.
pub fn set_aside_unreadable(data_dir: &Path) -> std::io::Result<PathBuf> {
    let stamp = crate::time::file_stamp(crate::time::now_ms());
    let db = data_dir.join(DB_FILE);
    let aside = data_dir.join(format!("acta.unreadable-{stamp}.db"));
    fs::rename(&db, &aside)?;
    for suffix in ["-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", db.display()));
        if from.exists() {
            fs::rename(&from, PathBuf::from(format!("{}{suffix}", aside.display())))?;
        }
    }
    Ok(aside)
}
