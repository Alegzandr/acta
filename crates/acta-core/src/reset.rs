//! Reset Acta (ADR 0006, amendment of 2026-11-22): erase every record and
//! start again from first run, keeping the settings.
//!
//! Two steps, because the app holds the database open while it runs:
//!
//! 1. [`request`], with the store open: Acta's entries are taken out of
//!    every folder's Claude Code settings through the ledger (as `acta setup
//!    --remove` does), then a small request file is written. It holds two
//!    flags, nothing of the user's. The app restarts.
//! 2. [`wipe_if_requested`], at the next start, before the database is
//!    opened: the database is renamed aside first (on Windows that fails
//!    while another process, an `acta mcp` of a running Claude Code session,
//!    holds it: then nothing is erased, [`ResetError::InUse`], and the
//!    folder settings come back with the start's own sync), then every file
//!    of the user's data is deleted and the key with it.
//!
//! Erased: the database (with its WAL and shared memory), `presence.db`,
//! `usage.json`, the snapshots, the hooks' link index, unreadable databases
//! kept by a key-loss recovery, the key store marker and the key (keychain
//! entry or key file). Kept: `preferences.json` (theme, language, layout),
//! the Claude Code plugin directory, the log, and, unless asked, the voice
//! models and the chat and browser profiles (their sign-ins).
//!
//! There is no CLI or MCP path: only the app's Settings offer it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::keys::{KEY_FILE, KeyStores, MARKER_FILE};
use crate::store::{DB_FILE, Store};

/// The request file, in the data directory.
pub const REQUEST_FILE: &str = "reset-requested";
/// The chat panel's web view profile (ADR 0009).
pub const CHAT_PROFILE_DIR: &str = "webview";
/// The agent browser's web view profile (ADR 0010).
pub const BROWSER_PROFILE_DIR: &str = "agent-browser";

/// What else a reset erases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetOptions {
    /// The chat and browser profiles: their sign-ins.
    pub sign_out: bool,
    /// The downloaded dictation models.
    pub voice_models: bool,
}

#[derive(Debug)]
pub enum ResetError {
    /// Another process holds the database: nothing was erased.
    InUse,
    Io(io::Error),
}

impl std::fmt::Display for ResetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResetError::InUse => f.write_str("another process holds the database"),
            ResetError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ResetError {}

/// What a reset erases, counted for its confirmation (trash included).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResetCounts {
    pub projects: u64,
    pub topics: u64,
    pub notes: u64,
    pub documents: u64,
    pub memories: u64,
    pub files: u64,
    pub contacts: u64,
}

/// The counts of [`ResetCounts`].
pub fn counts(store: &Store) -> crate::Result<ResetCounts> {
    let n = |sql: &str| -> crate::Result<u64> {
        let v: i64 = store.conn.query_row(sql, [], |r| r.get(0))?;
        Ok(v.max(0) as u64)
    };
    Ok(ResetCounts {
        projects: n("SELECT count(*) FROM projects")?,
        topics: n("SELECT count(*) FROM topics")?,
        notes: n("SELECT count(*) FROM notes")?,
        documents: n("SELECT count(*) FROM kb_entries WHERE kind = 'document'")?,
        memories: n("SELECT count(*) FROM kb_entries WHERE kind = 'memory'")?,
        files: n("SELECT count(*) FROM attachments")?,
        contacts: n("SELECT count(*) FROM contacts")?,
    })
}

/// Step 1: takes Acta's entries out of every folder's Claude Code settings
/// and records the request. Returns the folders restored and those left
/// as they were (not on this computer). The caller restarts the app.
pub fn request(store: &mut Store, options: ResetOptions) -> crate::Result<(usize, usize)> {
    store.check_writable()?;
    let done = crate::claude_settings::remove_all(store)?;
    let text = serde_json::to_string(&options)?;
    crate::fsutil::write_private_atomic(&store.data_dir().join(REQUEST_FILE), text.as_bytes())?;
    Ok(done)
}

/// The pending request, if any. A request file that cannot be read is no
/// request: a reset is never started on a guess.
pub fn requested(data_dir: &Path) -> Option<ResetOptions> {
    let text = fs::read_to_string(data_dir.join(REQUEST_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Step 2: when a reset was requested, erases the data before the store is
/// opened. `Ok(None)`: no request. The request is consumed either way, so
/// a failed reset is not retried at every start.
pub fn wipe_if_requested(
    data_dir: &Path,
    keys: &KeyStores,
) -> Result<Option<Vec<PathBuf>>, ResetError> {
    let Some(options) = requested(data_dir) else {
        return Ok(None);
    };
    let wiped = wipe(data_dir, keys, options);
    let _ = fs::remove_file(data_dir.join(REQUEST_FILE));
    wiped.map(Some)
}

/// Erases the data (see the module). Returns what could not be deleted
/// (best effort once the database is gone).
pub fn wipe(
    data_dir: &Path,
    keys: &KeyStores,
    options: ResetOptions,
) -> Result<Vec<PathBuf>, ResetError> {
    // The gate: the database is renamed aside, or nothing happens.
    let db = data_dir.join(DB_FILE);
    let aside = data_dir.join("acta.resetting.db");
    if db.exists()
        && let Err(e) = fs::rename(&db, &aside)
    {
        return Err(match e.kind() {
            io::ErrorKind::PermissionDenied => ResetError::InUse,
            // Windows: "being used by another process" (32).
            _ if e.raw_os_error() == Some(32) => ResetError::InUse,
            _ => ResetError::Io(e),
        });
    }
    let mut left = Vec::new();
    let mut gone = |path: PathBuf| {
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        match result {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => left.push(path),
        }
    };
    let with_journal = |name: &str| -> Vec<PathBuf> {
        ["", "-wal", "-shm", "-journal"]
            .iter()
            .map(|s| data_dir.join(format!("{name}{s}")))
            .collect()
    };
    for path in with_journal("acta.resetting.db")
        .into_iter()
        .chain(with_journal(DB_FILE))
        .chain(with_journal(crate::presence::PRESENCE_FILE))
    {
        gone(path);
    }
    // Databases kept aside by a key-loss recovery hold data too.
    if let Ok(entries) = fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("acta.unreadable-") {
                gone(entry.path());
            }
        }
    }
    gone(data_dir.join(crate::usage::USAGE_FILE));
    gone(data_dir.join(crate::snapshots::SNAPSHOT_DIR));
    gone(data_dir.join(crate::link_index::FILE));
    gone(data_dir.join("acta.init.lock"));
    // The key: from the store the marker names, and the key file in any
    // case (Linux falls back to it).
    let marker = fs::read_to_string(data_dir.join(MARKER_FILE)).unwrap_or_default();
    let key_kept = keys
        .by_name(marker.trim())
        .is_some_and(|store| store.delete().is_err());
    gone(data_dir.join(KEY_FILE));
    if !key_kept {
        gone(data_dir.join(MARKER_FILE));
    }
    if options.sign_out {
        gone(data_dir.join(CHAT_PROFILE_DIR));
        gone(data_dir.join(BROWSER_PROFILE_DIR));
    }
    if options.voice_models {
        gone(crate::voice::models::models_dir(data_dir));
    }
    if key_kept {
        // The marker stays so a later reset can find the key again.
        left.push(data_dir.join(MARKER_FILE));
    }
    Ok(left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::KeyStore as _;
    use crate::model::Author;
    use crate::store::test_support;

    fn touch(path: &Path) {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn a_reset_erases_the_data_and_keeps_the_settings() {
        let mut ts = test_support::open();
        let dir = ts.dir.path().to_path_buf();
        ts.store
            .create_project("Alexandre's garden", "", Author::Human)
            .unwrap();
        ts.store
            .snapshot(crate::snapshots::SnapshotKind::Manual)
            .unwrap();
        for name in [
            "presence.db",
            "usage.json",
            "linked-folders",
            "acta.unreadable-2026-11-20-101010.db",
            "preferences.json",
            "claude/setup-record.json",
            "acta.log",
            "models/ggml-base.bin",
            "webview/EBWebView/Cookies",
            "agent-browser/Cookies",
        ] {
            touch(&dir.join(name));
        }
        assert!(dir.join("snapshots").is_dir());
        assert!(ts.keys.load().unwrap().is_some());

        // A folder with Acta's settings, written through the ledger.
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        fs::write(repo.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        crate::claude_settings::apply(
            &mut ts.store,
            repo.path(),
            crate::claude_settings::Wanted {
                auto_memory_off: Some(true),
                commit: None,
            },
        )
        .unwrap();
        let settings = repo.path().join(crate::claude_settings::LOCAL_SETTINGS);
        assert!(settings.exists());

        let c = counts(&ts.store).unwrap();
        assert_eq!((c.projects, c.topics, c.files), (1, 0, 0));
        let (restored, kept) = request(&mut ts.store, ResetOptions::default()).unwrap();
        assert_eq!((restored, kept), (1, 0));
        assert!(!settings.exists(), "the file Acta created is gone");
        assert_eq!(requested(&dir), Some(ResetOptions::default()));
        let config = ts.config();
        drop(ts.store);

        let left = wipe_if_requested(&dir, &ts.keys.stores()).unwrap().unwrap();
        assert!(left.is_empty(), "{left:?}");
        for gone in [
            "acta.db",
            "acta.db-wal",
            "presence.db",
            "usage.json",
            "linked-folders",
            "snapshots",
            "acta.unreadable-2026-11-20-101010.db",
            "keystore",
            "reset-requested",
        ] {
            assert!(!dir.join(gone).exists(), "{gone}");
        }
        assert!(ts.keys.load().unwrap().is_none(), "the key is deleted");
        for kept in [
            "preferences.json",
            "claude/setup-record.json",
            "acta.log",
            "models/ggml-base.bin",
            "webview/EBWebView/Cookies",
            "agent-browser/Cookies",
        ] {
            assert!(dir.join(kept).exists(), "{kept}");
        }
        // Consumed: the next start does nothing.
        assert!(
            wipe_if_requested(&dir, &ts.keys.stores())
                .unwrap()
                .is_none()
        );
        // And the next open is a first run.
        let fresh = Store::open(&config).unwrap();
        assert!(fresh.was_created());
    }

    #[test]
    fn sign_out_and_voice_models_go_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let keys = crate::keys::MemoryKeyStore::new();
        for name in [
            "models/a.bin",
            "webview/x",
            "agent-browser/y",
            "preferences.json",
        ] {
            touch(&dir.path().join(name));
        }
        let left = wipe(
            dir.path(),
            &keys.stores(),
            ResetOptions {
                sign_out: true,
                voice_models: true,
            },
        )
        .unwrap();
        assert!(left.is_empty());
        for name in ["models", "webview", "agent-browser"] {
            assert!(!dir.path().join(name).exists(), "{name}");
        }
        assert!(dir.path().join("preferences.json").exists());
    }

    #[test]
    fn an_unreadable_request_is_no_request() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("acta.db"));
        fs::write(dir.path().join(REQUEST_FILE), "{not json").unwrap();
        let keys = crate::keys::MemoryKeyStore::new();
        assert!(
            wipe_if_requested(dir.path(), &keys.stores())
                .unwrap()
                .is_none()
        );
        assert!(dir.path().join("acta.db").exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_database_held_by_another_process_is_not_touched() {
        let ts = test_support::open();
        let dir = ts.dir.path().to_path_buf();
        // `ts.store` is the other process's connection.
        let left = wipe(&dir, &ts.keys.stores(), ResetOptions::default());
        assert!(matches!(left, Err(ResetError::InUse)), "{left:?}");
        assert!(dir.join("acta.db").exists());
        assert!(ts.keys.load().unwrap().is_some(), "the key stays");
    }
}
