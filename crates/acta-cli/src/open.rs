//! Where the CLI finds the store, and how open failures are told without
//! naming the database path or anything about the key.

use acta_core::{OpenError, Store, StoreConfig};
use std::path::PathBuf;

/// Development and test override of the data directory, honoured by debug
/// builds only. It fails closed: a release build that sees it refuses to
/// open any store (hooks stay silent, commands say why) instead of falling
/// back to the real data directory and keychain, so a test or a manual check
/// run against a release binary can never reach real data. With it, the key
/// is kept in a key file in that directory (a store ADR 0003 already uses on
/// macOS and as the Linux fallback), so several processes of a test share
/// it; `ACTA_DEV_KEYSTORE=memory` keeps it in memory instead.
pub const DEV_DATA_DIR: &str = "ACTA_DEV_DATA_DIR";
#[cfg(debug_assertions)]
pub const DEV_KEYSTORE: &str = "ACTA_DEV_KEYSTORE";

const DEV_IN_RELEASE: &str = "ACTA_DEV_DATA_DIR is set, and this release build ignores it; it opens no store while it is set. Unset it to use your Acta data.";

#[derive(Clone)]
pub struct Opener {
    pub config: Option<StoreConfig>,
    /// The development override is in effect.
    pub dev: bool,
    /// Why no store may be opened at all (the override in a release build).
    pub refusal: Option<&'static str>,
}

impl Opener {
    pub fn from_env() -> Opener {
        let requested = std::env::var_os(DEV_DATA_DIR).is_some_and(|v| !v.is_empty());
        match dev_config() {
            Some(config) => Opener {
                config: Some(config),
                dev: true,
                refusal: None,
            },
            None if requested => Opener {
                config: None,
                dev: true,
                refusal: Some(DEV_IN_RELEASE),
            },
            None => Opener {
                config: StoreConfig::os_default(),
                dev: false,
                refusal: None,
            },
        }
    }

    /// The data directory, without opening anything (hooks' pre-check).
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.config.as_ref().map(|c| c.data_dir.clone())
    }

    /// Opens the store only if its database already exists: setup's checks
    /// and reports must never create a database (and its key).
    pub fn open_existing(&self) -> Option<Result<Store, String>> {
        let dir = self.data_dir()?;
        dir.join(acta_core::store::DB_FILE)
            .is_file()
            .then(|| self.open())
    }

    pub fn open(&self) -> Result<Store, String> {
        if let Some(why) = self.refusal {
            return Err(why.into());
        }
        let config = self
            .config
            .as_ref()
            .ok_or("Acta cannot find its data directory (no user profile directories).")?;
        Store::open(config).map_err(|e| describe(&e))
    }
}

#[cfg(debug_assertions)]
fn dev_config() -> Option<StoreConfig> {
    use acta_core::keys::{KEY_FILE, KeyFile, KeyStores, MemoryKeyStore};
    use std::sync::Arc;
    let dir = std::env::var_os(DEV_DATA_DIR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)?;
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

#[cfg(not(debug_assertions))]
fn dev_config() -> Option<StoreConfig> {
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::keys::MemoryKeyStore;

    #[test]
    fn open_existing_never_creates_a_database() {
        let tmp = tempfile::tempdir().unwrap();
        let keys = MemoryKeyStore::new();
        let opener = Opener {
            config: Some(StoreConfig {
                data_dir: tmp.path().to_path_buf(),
                keys: keys.stores(),
            }),
            dev: true,
            refusal: None,
        };
        assert!(opener.open_existing().is_none());
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        opener.open().unwrap();
        assert!(opener.open_existing().is_some_and(|r| r.is_ok()));
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::*;

    #[test]
    fn a_refusing_opener_reaches_no_store() {
        let opener = Opener {
            config: None,
            dev: true,
            refusal: Some(DEV_IN_RELEASE),
        };
        assert!(opener.data_dir().is_none());
        assert!(opener.open_existing().is_none());
        assert_eq!(opener.open().err().as_deref(), Some(DEV_IN_RELEASE));
    }
}
