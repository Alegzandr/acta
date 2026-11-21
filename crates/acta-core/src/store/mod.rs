//! The store: one open, encrypted database and the typed queries and commands
//! the app, the CLI and the MCP server share.
//!
//! A `Store` owns one connection. It is `Send`, not `Sync`: the GUI keeps it
//! on a background executor and never calls it from the UI thread. Each
//! command is one `BEGIN IMMEDIATE` transaction (ADR 0003); no read
//! transaction is held between calls. List calls return summaries and pages
//! (ADR 0005); bodies are read one record at a time.

mod exchange;
mod folders;
mod kb;
mod records;
mod search;
mod timeline;
mod trash;

pub use exchange::{
    ExportFile, FILE_EXTENSION, FORMAT_NAME, FORMAT_VERSION, ImportCounts, ImportReport, XEntry,
    XEpic, XFolder, XKb, XNote, XProject, XRevision, XStamp, XTopic,
};
pub use folders::{EffectiveFolder, EffectiveFolders, FolderResolution};
pub use kb::{KbCounts, KbEdit, KbFilter, KbRevisionSummary, KbUpdate, NewKb};
pub use records::{EpicPatch, NewTopic, ProjectPatch, TopicFilter, TopicPatch};
pub use search::{SNIPPET_CLOSE, SNIPPET_OPEN, SearchHit, SearchKind, SearchQuery};
pub use timeline::{EntryFilter, NewEntry};
pub use trash::{TRASH_RETENTION_MS, TrashItem};

use crate::db::{self, ChangeWatcher, Mode, Opened};
use crate::error::{Error, OpenError, Result};
use crate::keys::{Key, KeyStores};
use crate::model::{Author, EpicId, Owner, ProjectId, Stamp, TopicId};
use crate::paths::{self, BaseDirs, Os};
use crate::schema;
use crate::snapshots::{self, Snapshot, SnapshotKind};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use std::path::{Path, PathBuf};

pub use crate::db::DB_FILE;

/// Where the database is and how its key is found. Tests pass a temporary
/// directory and a [`crate::keys::MemoryKeyStore`].
#[derive(Clone)]
pub struct StoreConfig {
    pub data_dir: PathBuf,
    pub keys: KeyStores,
}

impl StoreConfig {
    /// The real location and the OS key stores (ADR 0003). `None` when the
    /// base directories are not known.
    pub fn os_default() -> Option<StoreConfig> {
        let data_dir = paths::data_dir(Os::CURRENT, &BaseDirs::from_env())?;
        let keys = KeyStores::os_default(&data_dir);
        Some(StoreConfig { data_dir, keys })
    }
}

pub struct Store {
    pub(crate) conn: Connection,
    key: Key,
    data_dir: PathBuf,
    opened: Opened,
    writable: bool,
    /// Set when the last rewrite of the hooks' pre-check file failed.
    link_index_failed: std::cell::Cell<bool>,
}

/// `meta` key raised by the second release of a two-step destructive change
/// (ADR 0003); a build whose schema is older refuses to write.
const META_MIN_COMPATIBLE: &str = "min_compatible_version";
/// When the user last exported (data model request 5). Local, not exported.
const META_LAST_EXPORT: &str = "last_export_at";

impl Store {
    /// Opens the database, creating it and its key on first run, and brings
    /// the schema up to date. See [`OpenError`] for the cases the UI turns
    /// into key-loss recovery.
    pub fn open(config: &StoreConfig) -> Result<Store, OpenError> {
        Self::open_with(config, schema::MIGRATIONS)
    }

    pub(crate) fn open_with(config: &StoreConfig, steps: &[&str]) -> Result<Store, OpenError> {
        let (mut conn, key, opened, lock) = db::open_or_init(&config.data_dir, &config.keys)?;
        db::migrate(&mut conn, &config.data_dir, &key, steps)?;
        drop(lock);
        let min: Option<i64> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [META_MIN_COMPATIBLE],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse().ok());
        let writable = min.is_none_or(|min| min <= schema::SCHEMA_VERSION);
        Ok(Store {
            conn,
            key,
            data_dir: config.data_dir.clone(),
            opened,
            writable,
            link_index_failed: std::cell::Cell::new(false),
        })
    }

    /// Whether this open created the database.
    pub fn was_created(&self) -> bool {
        self.opened == Opened::Created
    }

    /// `false` when a newer Acta raised the minimum compatible schema.
    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join(DB_FILE)
    }

    /// A watcher on its own connection; see [`ChangeWatcher`].
    pub fn watcher(&self) -> Result<ChangeWatcher> {
        let conn = db::connect(&self.db_path(), &self.key, Mode::Live { create: false })
            .map_err(open_to_error)?;
        Ok(ChangeWatcher::new(conn)?)
    }

    /// `PRAGMA cipher_version` and `cipher_provider`: a CI test asserts them
    /// so a build that links a plain SQLite fails (ADR 0003).
    pub fn cipher_info(&self) -> Result<(String, String)> {
        let version = self
            .conn
            .query_row("PRAGMA cipher_version", [], |r| r.get(0))?;
        let provider = self
            .conn
            .query_row("PRAGMA cipher_provider", [], |r| r.get(0))?;
        Ok((version, provider))
    }

    // ---- snapshots (ADR 0006) ----

    pub fn snapshot(&self, kind: SnapshotKind) -> Result<Snapshot> {
        snapshots::take(&self.conn, &self.data_dir, kind)
    }

    /// Takes the daily snapshot if none was taken today (UTC). The app calls
    /// this on a background thread after start and once a day; hooks and the
    /// CLI do not, so they stay fast.
    pub fn snapshot_if_due(&self) -> Result<Option<Snapshot>> {
        let all = snapshots::list(&self.data_dir)?;
        if snapshots::daily_due(&all, crate::time::now_ms()) {
            self.snapshot(SnapshotKind::Daily).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn snapshots(&self) -> Result<Vec<Snapshot>> {
        snapshots::list(&self.data_dir)
    }

    /// Replaces the live database with a snapshot, after taking a `restore`
    /// snapshot of the current state. Other processes see an ordinary commit.
    pub fn restore_snapshot(&mut self, snapshot: &Path) -> Result<()> {
        self.check_writable()?;
        let _lock = db::InitLock::acquire(&self.data_dir)?;
        // Open the source first, and keep it out of retention: restoring a
        // `restore` snapshot must not delete it by taking the next one.
        let src = db::connect(snapshot, &self.key, db::Mode::Snapshot).map_err(open_to_error)?;
        snapshots::take_protecting(
            &self.conn,
            &self.data_dir,
            SnapshotKind::Restore,
            Some(snapshot),
        )?;
        db::restore_from(&mut self.conn, &src).map_err(open_to_error)?;
        drop(src);
        db::migrate(
            &mut self.conn,
            &self.data_dir,
            &self.key,
            schema::MIGRATIONS,
        )
        .map_err(open_to_error)?;
        let _ = self.refresh_link_index();
        Ok(())
    }

    // ---- local values ----

    /// A value of this installation, kept in the encrypted database and never
    /// exported: the agent focus per session and per worktree (ADR 0005).
    pub fn local_value(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [format!("local.{key}")],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Sets (`Some`) or removes (`None`) a local value.
    pub fn set_local_value(&mut self, key: &str, value: Option<&str>) -> Result<()> {
        let key = format!("local.{key}");
        let tx = self.write()?;
        match value {
            Some(v) => tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, v],
            )?,
            None => tx.execute("DELETE FROM meta WHERE key = ?1", [key])?,
        };
        tx.commit()?;
        Ok(())
    }

    /// Every local value whose key starts with `prefix`, keys as passed to
    /// [`Store::set_local_value`].
    pub fn local_values(&self, prefix: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM meta WHERE substr(key, 1, ?2) = ?1 ORDER BY key")?;
        let full = format!("local.{prefix}");
        let rows = stmt.query_map(rusqlite::params![full, full.chars().count() as i64], |r| {
            let k: String = r.get(0)?;
            Ok((k["local.".len()..].to_owned(), r.get(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- export history (request 5) ----

    /// Records that the user saved an export now.
    pub fn record_export(&mut self) -> Result<()> {
        let tx = self.write()?;
        tx.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            rusqlite::params![META_LAST_EXPORT, crate::time::now_ms().to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn last_export_at(&self) -> Result<Option<i64>> {
        let v: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [META_LAST_EXPORT],
                |r| r.get(0),
            )
            .ok();
        Ok(v.and_then(|v| v.parse().ok()))
    }

    // ---- helpers ----

    pub(crate) fn check_writable(&self) -> Result<()> {
        if self.writable {
            Ok(())
        } else {
            Err(Error::ReadOnly)
        }
    }

    /// A write transaction (`BEGIN IMMEDIATE`).
    pub(crate) fn write(&mut self) -> Result<Transaction<'_>> {
        self.check_writable()?;
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?)
    }
}

fn open_to_error(e: OpenError) -> Error {
    match e {
        OpenError::Io(e) => Error::Io(e),
        OpenError::Db(e) => Error::Db(e),
        other => Error::Io(std::io::Error::other(other.to_string())),
    }
}

pub(crate) fn new_uuid() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// The common columns, in the order [`stamp`] reads them.
pub(crate) const STAMP_COLS: &str =
    "uuid, created_at, updated_at, updated_by, deleted_at, deleted_by";

/// Reads [`STAMP_COLS`] starting at column `at`.
pub(crate) fn stamp(row: &Row<'_>, at: usize) -> rusqlite::Result<Stamp> {
    Ok(Stamp {
        uuid: row.get(at)?,
        created_at: row.get(at + 1)?,
        updated_at: row.get(at + 2)?,
        updated_by: row.get::<_, Author>(at + 3)?,
        deleted_at: row.get(at + 4)?,
        deleted_by: row.get(at + 5)?,
    })
}

/// The owner columns of a live owner: project, epic, topic.
pub(crate) fn owner_ids(
    conn: &Connection,
    owner: Owner,
) -> Result<(i64, Option<i64>, Option<i64>)> {
    use records::{read_epic, read_project, read_topic};
    Ok(match owner {
        Owner::Project(id) => (read_project(conn, id)?.id.0, None, None),
        Owner::Epic(id) => (read_epic(conn, id)?.project.0, Some(id.0), None),
        Owner::Topic(id) => (read_topic(conn, id)?.project.0, None, Some(id.0)),
    })
}

/// Reads an owner from `project_id, epic_id, topic_id` columns at `at`.
pub(crate) fn owner_at(row: &Row<'_>, at: usize) -> rusqlite::Result<Owner> {
    let project: i64 = row.get(at)?;
    let epic: Option<i64> = row.get(at + 1)?;
    let topic: Option<i64> = row.get(at + 2)?;
    Ok(match (epic, topic) {
        (_, Some(t)) => Owner::Topic(TopicId(t)),
        (Some(e), None) => Owner::Epic(EpicId(e)),
        (None, None) => Owner::Project(ProjectId(project)),
    })
}

/// SQL condition selecting rows owned exactly by `owner` (no alias).
pub(crate) fn owner_condition(owner: Owner) -> String {
    match owner {
        Owner::Project(id) => {
            format!(
                "project_id = {} AND epic_id IS NULL AND topic_id IS NULL",
                id.0
            )
        }
        Owner::Epic(id) => format!("epic_id = {}", id.0),
        Owner::Topic(id) => format!("topic_id = {}", id.0),
    }
}

/// A title or text that must not be blank.
pub(crate) fn required(field: &str, value: &str) -> Result<String> {
    let v = value.trim();
    if v.is_empty() {
        Err(Error::Invalid(format!("{field} must not be empty")))
    } else {
        Ok(v.to_owned())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::keys::MemoryKeyStore;

    pub struct TestStore {
        pub store: Store,
        pub keys: MemoryKeyStore,
        pub dir: tempfile::TempDir,
    }

    impl TestStore {
        pub fn config(&self) -> StoreConfig {
            StoreConfig {
                data_dir: self.dir.path().to_path_buf(),
                keys: self.keys.stores(),
            }
        }
    }

    pub fn open() -> TestStore {
        let dir = tempfile::tempdir().unwrap();
        let keys = MemoryKeyStore::new();
        let config = StoreConfig {
            data_dir: dir.path().to_path_buf(),
            keys: keys.stores(),
        };
        let store = Store::open(&config).unwrap();
        TestStore { store, keys, dir }
    }
}

#[cfg(test)]
mod tests;
