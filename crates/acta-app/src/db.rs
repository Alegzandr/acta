//! The open store, shared with background work.
//!
//! `acta_core::Store` is `Send`, not `Sync`, and every call may wait on the
//! disk or on another process's write lock, so the UI thread never touches
//! it: each query or command runs on GPUI's background executor, one at a
//! time behind a mutex.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use acta_core::{OpenError, Result, Store, StoreConfig};
use gpui_kit::{App, Task};

#[derive(Clone)]
pub struct Db(Arc<Mutex<Store>>);

impl Db {
    pub fn new(store: Store) -> Self {
        Self(Arc::new(Mutex::new(store)))
    }

    /// The store, for code already off the UI thread (and for tests).
    pub fn lock(&self) -> MutexGuard<'_, Store> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Run `f` against the store on a background thread.
    pub fn run<R: Send + 'static>(
        &self,
        cx: &App,
        f: impl FnOnce(&mut Store) -> Result<R> + Send + 'static,
    ) -> Task<Result<R>> {
        let db = self.clone();
        cx.background_executor()
            .spawn(async move { f(&mut db.lock()) })
    }
}

/// How the database came up at launch.
pub enum Opened {
    Ready(Store),
    /// The file exists and cannot be read with any key Acta has: the
    /// key-loss recovery screen (docs/design/components-app.md 5, 8.9). The
    /// file is never touched.
    KeyLoss {
        db: PathBuf,
        /// A key was found but does not open the file (damaged, or the key
        /// of another database), rather than no key at all.
        unreadable: bool,
    },
    /// Anything else: no data directory, a key store that failed, a damaged
    /// file after a failed migration. Logged by kind; the window shows the
    /// key-loss screen too, since its actions are the safe ones.
    Failed {
        db: Option<PathBuf>,
    },
}

/// The configuration of this run: the OS location and key stores, except in
/// a debug build started with `ACTA_DEV_KEY_FILE=1`, which keeps the key in a
/// file next to the database so that captures and manual checks never write
/// to the system keychain of the machine they run on.
pub fn config() -> Option<StoreConfig> {
    let mut config = StoreConfig::os_default()?;
    if cfg!(debug_assertions) && std::env::var_os("ACTA_DEV_KEY_FILE").is_some_and(|v| v == "1") {
        let file = acta_core::keys::KeyFile::new(config.data_dir.join(acta_core::keys::KEY_FILE));
        config.keys = acta_core::keys::KeyStores(vec![Arc::new(file)]);
    }
    Some(config)
}

/// A requested reset could not be done at this start (shown once as a
/// toast by the shell).
pub static RESET_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Open the store (blocking: call it off the UI thread).
pub fn open(config: Option<StoreConfig>) -> Opened {
    let Some(config) = config else {
        crate::log::line("store: no data directory");
        return Opened::Failed { db: None };
    };
    let db = config.data_dir.join(acta_core::store::DB_FILE);
    // Reset Acta (Settings, Data): erased now, before anything opens the
    // database. When another process holds it nothing is erased, and the
    // folder settings come back with the sync below.
    match acta_core::reset::wipe_if_requested(&config.data_dir, &config.keys) {
        Ok(None) => {}
        Ok(Some(wiped)) => crate::log::line(&format!(
            "reset: data erased ({} item(s) could not be deleted{})",
            wiped.left.len(),
            if wiped.db_reappeared {
                "; a database appeared meanwhile and keeps its key"
            } else {
                ""
            }
        )),
        // A session still runs: tried again at the next start.
        Err(acta_core::reset::ResetError::InUse) => {
            crate::log::line("reset: not done: a Claude Code session holds the data");
            RESET_FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // A request this database did not make: dropped, nothing erased.
        Err(err) => crate::log::line(&format!("reset: request dropped: {err}")),
    }
    match Store::open(&config) {
        Ok(mut store) => {
            // Folders changed while Acta was closed (a commit mode set
            // while a folder was offline): Claude Code's settings follow
            // the store (ADR 0011).
            match acta_core::claude_settings::sync_all(&mut store) {
                Ok((written, _)) if written > 0 => crate::log::line(&format!(
                    "folders: Claude Code settings brought in line in {written}"
                )),
                Ok(_) => {}
                Err(_) => crate::log::line("folders: Claude Code settings could not be checked"),
            }
            // No session Acta started can outlive it: their records and
            // per-session settings files go (ADR 0011 addendum 2026-11-23).
            if acta_core::claude_settings::page::end_all_sessions(&mut store).is_err() {
                crate::log::line("sessions: launch records could not be cleared");
            }
            Opened::Ready(store)
        }
        Err(OpenError::KeyMissing { db, .. }) => {
            crate::log::line("store: the database exists and its key is missing");
            Opened::KeyLoss {
                db,
                unreadable: false,
            }
        }
        Err(OpenError::KeyWrongOrCorrupt { db }) => {
            crate::log::line("store: the database cannot be read with its key");
            Opened::KeyLoss {
                db,
                unreadable: true,
            }
        }
        Err(err) => {
            crate::log::line(&format!("store: could not open: {}", open_error_kind(&err)));
            Opened::Failed { db: Some(db) }
        }
    }
}

/// The kind of an open error, for the log: never a path or a message that
/// could carry user text.
fn open_error_kind(err: &OpenError) -> &'static str {
    match err {
        OpenError::KeyMissing { .. } => "key missing",
        OpenError::KeyWrongOrCorrupt { .. } => "wrong key or damaged file",
        OpenError::KeyStore(_) => "key store failed",
        OpenError::MigrationFailed { .. } => "migration failed",
        OpenError::Io(_) => "i/o",
        OpenError::Db(_) => "database",
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use acta_core::keys::MemoryKeyStore;

    /// A store in a temporary directory with an in-memory key: never the
    /// real keychain or data directory.
    pub struct TempStore {
        pub store: Store,
        pub keys: MemoryKeyStore,
        pub dir: tempfile::TempDir,
    }

    impl TempStore {
        pub fn config(&self) -> StoreConfig {
            StoreConfig {
                data_dir: self.dir.path().to_path_buf(),
                keys: self.keys.stores(),
            }
        }
    }

    pub fn temp_store() -> TempStore {
        let dir = tempfile::tempdir().unwrap();
        let keys = MemoryKeyStore::new();
        let config = StoreConfig {
            data_dir: dir.path().to_path_buf(),
            keys: keys.stores(),
        };
        let store = Store::open(&config).unwrap();
        TempStore { store, keys, dir }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::temp_store;
    use super::*;

    #[test]
    fn a_lost_key_is_key_loss_and_the_file_is_left_alone() {
        let temp = temp_store();
        let config = temp.config();
        let db_path = temp.store.db_path();
        drop(temp.store);
        let before = std::fs::read(&db_path).unwrap();
        temp.keys.forget();
        match open(Some(config)) {
            Opened::KeyLoss { db, unreadable } => {
                assert_eq!(db, db_path);
                assert!(!unreadable, "a missing key is not a damaged file");
            }
            _ => panic!("expected key loss"),
        }
        assert_eq!(std::fs::read(&db_path).unwrap(), before);
    }

    #[test]
    fn no_data_directory_is_a_failure_not_a_panic() {
        assert!(matches!(open(None), Opened::Failed { db: None }));
    }

    #[test]
    fn an_existing_database_opens_again() {
        let temp = temp_store();
        let config = temp.config();
        drop(temp.store);
        assert!(matches!(open(Some(config)), Opened::Ready(_)));
    }
}
