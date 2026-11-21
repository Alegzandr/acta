//! Where the CLI finds the store, and how open failures are told without
//! naming the database path or anything about the key.

use acta_core::keys::{KEY_FILE, KeyFile, KeyStores, MemoryKeyStore};
use acta_core::{OpenError, Store, StoreConfig};
use std::path::PathBuf;
use std::sync::Arc;

/// Development and test override of the data directory. With it, the key is
/// kept in a key file in that directory (a store ADR 0003 already uses on
/// macOS and as the Linux fallback), so several processes of a test share
/// it; the real keychain is never touched. `ACTA_DEV_KEYSTORE=memory` keeps
/// it in memory instead, for one-process tests.
pub const DEV_DATA_DIR: &str = "ACTA_DEV_DATA_DIR";
pub const DEV_KEYSTORE: &str = "ACTA_DEV_KEYSTORE";

#[derive(Clone)]
pub struct Opener {
    pub config: Option<StoreConfig>,
}

impl Opener {
    pub fn from_env() -> Opener {
        let dev = std::env::var_os(DEV_DATA_DIR)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        let config = match dev {
            Some(dir) => {
                let keys = if std::env::var(DEV_KEYSTORE).is_ok_and(|v| v == "memory") {
                    MemoryKeyStore::new().stores()
                } else {
                    KeyStores(vec![Arc::new(KeyFile::new(dir.join(KEY_FILE)))])
                };
                Some(StoreConfig {
                    data_dir: dir,
                    keys,
                })
            }
            None => StoreConfig::os_default(),
        };
        Opener { config }
    }

    /// The data directory, without opening anything (hooks' pre-check).
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.config.as_ref().map(|c| c.data_dir.clone())
    }

    pub fn open(&self) -> Result<Store, String> {
        let config = self
            .config
            .as_ref()
            .ok_or("Acta cannot find its data directory (no user profile directories).")?;
        Store::open(config).map_err(|e| describe(&e))
    }
}

/// One line, no path, no key detail.
pub fn describe(e: &OpenError) -> String {
    match e {
        OpenError::KeyMissing { .. } => {
            "Acta's database key is missing from the keychain; open the Acta app to recover.".into()
        }
        OpenError::KeyWrongOrCorrupt { .. } => {
            "Acta's database cannot be opened (wrong key or damaged file); open the Acta app to recover.".into()
        }
        OpenError::KeyStore(_) => "Acta cannot reach the keychain holding its database key.".into(),
        OpenError::MigrationFailed { .. } => {
            "Acta's database upgrade failed and was rolled back; open the Acta app for details.".into()
        }
        OpenError::Io(e) => format!("Acta cannot open its database: {}", e.kind()),
        OpenError::Db(_) => "Acta cannot open its database (database error).".into(),
    }
}
