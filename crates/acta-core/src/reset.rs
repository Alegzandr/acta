//! Reset Acta (ADR 0006, amendment of 2026-11-22): erase every record and
//! start again from first run, keeping the settings.
//!
//! From the workbench, two steps, because the app holds the database open
//! while it runs:
//!
//! 1. [`request`], with the store open: Acta's entries are taken out of
//!    every folder's Claude Code settings through the ledger (as `acta setup
//!    --remove` does). When a folder cannot be reached its settings stay,
//!    and the request is not made unless the human resets anyway. Then a
//!    random token is stored in the database and written, with two flags,
//!    in a request file. The app restarts.
//! 2. [`wipe_if_requested`], at the next start, before the database is
//!    opened, under the init lock that every opening process takes: the
//!    database must hold the request's token (a request file someone else
//!    wrote erases nothing), no Claude Code session may be live in
//!    presence, and no other connection may have the database open (an
//!    exclusive-lock probe, the same on every system). Otherwise nothing
//!    is erased: [`ResetError::InUse`] keeps the request for the next start,
//!    [`ResetError::Refused`] drops it. Then every file of the user's data
//!    is deleted and the key with it.
//!
//! From the key-loss screen there is no store to hold a token: [`wipe_here`]
//! erases in the same process, from the app's own dialog, before anything
//! opens the database (which nothing can open without its key).
//!
//! Erased: the database (with its WAL and shared memory), `presence.db`,
//! `usage.json`, the snapshots, the hooks' link index, unreadable databases
//! kept by a key-loss recovery, the key store marker and the key (keychain
//! entry or key file). Kept: `preferences.json` (the app clears its record
//! ids itself), the Claude Code plugin directory, the log, and, unless
//! asked, the voice models and the chat and browser profiles (their
//! sign-ins).
//!
//! There is no CLI or MCP path: only the app offers it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db::{self, InitLock, Probe};
use crate::keys::{KEY_FILE, KeyStores, MARKER_FILE};
use crate::store::{DB_FILE, Store};

/// The request file, in the data directory.
pub const REQUEST_FILE: &str = "reset-requested";
/// The chat panel's web view profile (ADR 0009).
pub const CHAT_PROFILE_DIR: &str = "webview";
/// The agent browser's web view profile (ADR 0010).
pub const BROWSER_PROFILE_DIR: &str = "agent-browser";
/// The local value holding the request's token.
const TOKEN_KEY: &str = "reset.token";

/// What else a reset erases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetOptions {
    /// The chat and browser profiles: their sign-ins.
    pub sign_out: bool,
    /// The downloaded dictation models.
    pub voice_models: bool,
}

/// The request file: the options and the token the database holds.
#[derive(Serialize, Deserialize)]
struct RequestFile {
    #[serde(flatten)]
    options: ResetOptions,
    token: String,
}

#[derive(Debug)]
pub enum ResetError {
    /// A Claude Code session or another process holds the database:
    /// nothing was erased, the request stays for the next start.
    InUse,
    /// The request's token is not the database's: nothing was erased and
    /// the request is dropped.
    Refused,
    Io(io::Error),
}

impl std::fmt::Display for ResetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResetError::InUse => f.write_str("another process holds the database"),
            ResetError::Refused => f.write_str("the request does not match this database"),
            ResetError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ResetError {}

impl From<io::Error> for ResetError {
    fn from(e: io::Error) -> Self {
        ResetError::Io(e)
    }
}

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

/// What [`request`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requested {
    /// Recorded: restart the app. `restored` folders lost Acta's settings.
    Done { restored: usize },
    /// `kept` folders could not be reached and keep Acta's settings for
    /// Claude Code; nothing was recorded. Ask, then call again with
    /// `anyway`, or put the reachable folders back with
    /// [`crate::claude_settings::sync_all`].
    Unreachable { kept: usize },
}

/// Step 1: takes Acta's entries out of every folder's Claude Code settings
/// and, unless a folder could not be reached (and not `anyway`), records
/// the request with a new token. The caller restarts the app.
pub fn request(store: &mut Store, options: ResetOptions, anyway: bool) -> crate::Result<Requested> {
    store.check_writable()?;
    let (restored, kept) = crate::claude_settings::remove_all(store)?;
    if kept > 0 && !anyway {
        return Ok(Requested::Unreachable { kept });
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
    store.set_local_value(TOKEN_KEY, Some(&token))?;
    let text = serde_json::to_string(&RequestFile { options, token })?;
    crate::fsutil::write_private_atomic(&store.data_dir().join(REQUEST_FILE), text.as_bytes())?;
    Ok(Requested::Done { restored })
}

/// What a wipe did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wiped {
    /// What could not be deleted.
    pub left: Vec<PathBuf>,
    /// A database appeared after the key was deleted: the key and its
    /// marker were put back so it stays readable.
    pub db_reappeared: bool,
}

/// Step 2: when a reset was requested, erases the data before the store is
/// opened. `Ok(None)`: no request. See the module for when nothing is
/// erased; a request file that cannot be read is refused.
pub fn wipe_if_requested(data_dir: &Path, keys: &KeyStores) -> Result<Option<Wiped>, ResetError> {
    let request_file = data_dir.join(REQUEST_FILE);
    let text = match fs::read_to_string(&request_file) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let Ok(request) = serde_json::from_str::<RequestFile>(&text) else {
        let _ = fs::remove_file(&request_file);
        return Err(ResetError::Refused);
    };
    let wiped = wipe_locked(data_dir, keys, request.options, Some(&request.token));
    if !matches!(wiped, Err(ResetError::InUse)) {
        let _ = fs::remove_file(&request_file);
    }
    wiped.map(Some)
}

/// The key-loss screen's reset: erases now, in this process, with no
/// token. Called by the app's own dialog only, before any store is open.
pub fn wipe_here(
    data_dir: &Path,
    keys: &KeyStores,
    options: ResetOptions,
) -> Result<Wiped, ResetError> {
    wipe_locked(data_dir, keys, options, None)
}

/// Erases the data under the init lock. With `token`, the database must
/// hold it; without, a database the key does not open is erased too.
fn wipe_locked(
    data_dir: &Path,
    keys: &KeyStores,
    options: ResetOptions,
    token: Option<&str>,
) -> Result<Wiped, ResetError> {
    let _lock = InitLock::acquire(data_dir)?;
    let db = data_dir.join(DB_FILE);
    let key = db::marked_key(data_dir, keys);
    if db.exists() {
        match &key {
            Some((key, _)) => {
                if session_live(data_dir, key) {
                    return Err(ResetError::InUse);
                }
                let meta_key = format!("local.{TOKEN_KEY}");
                match db::probe_alone(data_dir, key, &meta_key) {
                    Ok(Probe::Busy) => return Err(ResetError::InUse),
                    Ok(Probe::Alone(found)) => {
                        if token.is_some() && found.as_deref() != token {
                            return Err(ResetError::Refused);
                        }
                    }
                    Ok(Probe::Unreadable) | Err(_) => {
                        if token.is_some() {
                            return Err(ResetError::Refused);
                        }
                    }
                }
            }
            // The key is gone: nothing can open the database, and nothing
            // can prove a request either.
            None => {
                if token.is_some() {
                    return Err(ResetError::Refused);
                }
            }
        }
    }
    let mut wiped = erase(data_dir, options)?;
    // The key: from the store the marker names, and the key file in any
    // case (Linux falls back to it).
    let mut key_kept = false;
    if let Some((key, store)) = &key {
        key_kept = store.delete().is_err();
        if !key_kept && db.exists() {
            // Something created a database meanwhile: it keeps its key.
            let _ = store.save(key);
            key_kept = true;
            wiped.db_reappeared = true;
        }
    }
    if !key_kept {
        remove(&data_dir.join(KEY_FILE), &mut wiped.left);
        remove(&data_dir.join(MARKER_FILE), &mut wiped.left);
    } else if !wiped.db_reappeared {
        wiped.left.push(data_dir.join(MARKER_FILE));
    }
    Ok(wiped)
}

/// A Claude Code session is live in presence.
fn session_live(data_dir: &Path, key: &crate::keys::Key) -> bool {
    if !data_dir.join(crate::presence::PRESENCE_FILE).exists() {
        return false;
    }
    match crate::presence::Presence::open(data_dir, key) {
        Ok(mut presence) => presence
            .live(crate::time::now_ms())
            .is_ok_and(|live| !live.is_empty()),
        Err(_) => false,
    }
}

fn remove(path: &Path, left: &mut Vec<PathBuf>) {
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(_) => left.push(path.to_path_buf()),
    }
}

/// Deletes the data files (not the key). The database is renamed aside
/// first: on Windows that fails while any process has it open, a second
/// gate after the probe.
fn erase(data_dir: &Path, options: ResetOptions) -> Result<Wiped, ResetError> {
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
    let mut wiped = Wiped::default();
    let left = &mut wiped.left;
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
        remove(&path, left);
    }
    // Databases kept aside by a key-loss recovery hold data too.
    if let Ok(entries) = fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("acta.unreadable-") {
                remove(&entry.path(), left);
            }
        }
    }
    remove(&data_dir.join(crate::usage::USAGE_FILE), left);
    remove(&data_dir.join(crate::snapshots::SNAPSHOT_DIR), left);
    remove(&data_dir.join(crate::link_index::FILE), left);
    if options.sign_out {
        remove(&data_dir.join(CHAT_PROFILE_DIR), left);
        remove(&data_dir.join(BROWSER_PROFILE_DIR), left);
    }
    if options.voice_models {
        remove(&crate::voice::models::models_dir(data_dir), left);
    }
    Ok(wiped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::KeyStore as _;
    use crate::model::Author;
    use crate::store::test_support::{self, TestStore};

    fn touch(path: &Path) {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    /// A store with a project and a snapshot, and the files around it.
    fn filled() -> TestStore {
        let mut ts = test_support::open();
        let dir = ts.dir.path().to_path_buf();
        ts.store
            .create_project("Alexandre's garden", "", Author::Human)
            .unwrap();
        ts.store
            .snapshot(crate::snapshots::SnapshotKind::Manual)
            .unwrap();
        for name in [
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
        ts
    }

    fn done(r: Requested) {
        assert!(matches!(r, Requested::Done { .. }), "{r:?}");
    }

    #[test]
    fn a_reset_erases_the_data_and_keeps_the_settings() {
        let mut ts = filled();
        let dir = ts.dir.path().to_path_buf();
        // A presence file whose sessions have all ended.
        drop(ts.store.presence().unwrap());
        assert!(dir.join("presence.db").exists());
        assert!(dir.join("snapshots").is_dir());

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
        assert_eq!(
            request(&mut ts.store, ResetOptions::default(), false).unwrap(),
            Requested::Done { restored: 1 }
        );
        assert!(!settings.exists(), "the file Acta created is gone");
        let text = fs::read_to_string(dir.join(REQUEST_FILE)).unwrap();
        assert!(
            !text.contains("Alexandre"),
            "flags and a token only: {text}"
        );
        let config = ts.config();
        drop(ts.store);

        let wiped = wipe_if_requested(&dir, &ts.keys.stores()).unwrap().unwrap();
        assert_eq!(wiped, Wiped::default());
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
        // Consumed: the next start does nothing, and is a first run.
        assert!(
            wipe_if_requested(&dir, &ts.keys.stores())
                .unwrap()
                .is_none()
        );
        let fresh = Store::open(&config).unwrap();
        assert!(fresh.was_created());
    }

    /// On every system: an open connection (the `acta mcp` of a running
    /// session) keeps everything, and the request waits for the next start.
    #[test]
    fn a_database_another_connection_holds_is_not_touched() {
        let mut ts = filled();
        let dir = ts.dir.path().to_path_buf();
        done(request(&mut ts.store, ResetOptions::default(), false).unwrap());
        let other = Store::open(&ts.config()).unwrap();
        drop(ts.store);
        // The probe itself, not Windows' rename gate, sees the connection.
        let key = ts.keys.load().unwrap().unwrap();
        assert_eq!(
            db::probe_alone(&dir, &key, "local.reset.token").unwrap(),
            Probe::Busy
        );
        assert!(matches!(
            wipe_if_requested(&dir, &ts.keys.stores()),
            Err(ResetError::InUse)
        ));
        assert!(dir.join("acta.db").exists());
        assert!(dir.join("snapshots").is_dir());
        assert!(ts.keys.load().unwrap().is_some(), "the key stays");
        assert!(dir.join(REQUEST_FILE).exists(), "tried again next start");
        drop(other);
        assert!(matches!(
            db::probe_alone(&dir, &key, "local.reset.token").unwrap(),
            Probe::Alone(Some(_))
        ));
        assert!(
            wipe_if_requested(&dir, &ts.keys.stores())
                .unwrap()
                .is_some()
        );
        assert!(!dir.join("acta.db").exists());
    }

    #[test]
    fn a_live_session_in_presence_keeps_everything() {
        let mut ts = filled();
        let dir = ts.dir.path().to_path_buf();
        let project = ts.store.projects().unwrap()[0].id;
        let mut presence = ts.store.presence().unwrap();
        presence
            .register(
                &crate::presence::NewSession {
                    id: "s-1".into(),
                    project,
                    focus: None,
                    launch: None,
                },
                crate::time::now_ms(),
            )
            .unwrap();
        drop(presence);
        done(request(&mut ts.store, ResetOptions::default(), false).unwrap());
        drop(ts.store);
        assert!(matches!(
            wipe_if_requested(&dir, &ts.keys.stores()),
            Err(ResetError::InUse)
        ));
        assert!(dir.join("acta.db").exists());
        assert!(dir.join(REQUEST_FILE).exists());
    }

    #[test]
    fn a_request_without_the_database_s_token_erases_nothing() {
        let ts = filled();
        let dir = ts.dir.path().to_path_buf();
        let keys = ts.keys.stores();
        drop(ts.store);
        for text in [
            r#"{"sign_out":false,"voice_models":false,"token":"00"}"#,
            r#"{"sign_out":false,"voice_models":false}"#,
            "{not json",
        ] {
            fs::write(dir.join(REQUEST_FILE), text).unwrap();
            assert!(
                matches!(wipe_if_requested(&dir, &keys), Err(ResetError::Refused)),
                "{text}"
            );
            assert!(dir.join("acta.db").exists(), "{text}");
            assert!(!dir.join(REQUEST_FILE).exists(), "dropped: {text}");
        }
    }

    #[test]
    fn an_unreachable_folder_asks_before_anything_is_recorded() {
        let mut ts = filled();
        let dir = ts.dir.path().to_path_buf();
        let gone = dir.join("no-such-folder");
        let key = format!(
            "{}{}",
            crate::claude_settings::LEDGER_PREFIX,
            crate::claude_settings::stored_form(&gone)
        );
        ts.store
            .set_local_value(
                &key,
                Some(r#"{"created":true,"keys":["autoMemoryEnabled"],"auto_memory_off":true}"#),
            )
            .unwrap();
        assert_eq!(
            request(&mut ts.store, ResetOptions::default(), false).unwrap(),
            Requested::Unreachable { kept: 1 }
        );
        assert!(!dir.join(REQUEST_FILE).exists());
        done(request(&mut ts.store, ResetOptions::default(), true).unwrap());
        assert!(dir.join(REQUEST_FILE).exists());
    }

    #[test]
    fn the_key_loss_screen_resets_in_this_process() {
        let ts = filled();
        let dir = ts.dir.path().to_path_buf();
        drop(ts.store);
        ts.keys.forget();
        let wiped = wipe_here(
            &dir,
            &ts.keys.stores(),
            ResetOptions {
                sign_out: true,
                voice_models: true,
            },
        )
        .unwrap();
        assert!(wiped.left.is_empty(), "{:?}", wiped.left);
        for name in [
            "acta.db",
            "keystore",
            "snapshots",
            "models",
            "webview",
            "agent-browser",
        ] {
            assert!(!dir.join(name).exists(), "{name}");
        }
        assert!(dir.join("preferences.json").exists());
    }
}
