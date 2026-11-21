//! `preferences.json` (docs/design/layout.md 4.7): settings that must be readable before
//! the database opens, so that first run comes up in the user's theme and
//! language.
//!
//! It lives in Acta's data directory (`acta_core::paths::data_dir`) and holds
//! settings only: never a title, a path, a draft or any other content
//! (CLAUDE.md). A missing file means defaults, silently; an unreadable,
//! malformed or unknown-version file means defaults and one log line with
//! the kind of error, never its text. Writes replace the file atomically
//! (temporary file in the same directory, then rename) on a writer thread
//! of their own, so the UI thread never waits on the disk and a later
//! change is never overwritten by an earlier one. At quit, and before an
//! update restarts the app, [`Writer::flush`] writes what is pending on the
//! calling thread, since the process may end before the thread runs.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};

use acta_core::paths::{self, BaseDirs, Os};

use crate::i18n::Locale;
use crate::theme::ThemeSetting;

pub const FILE_NAME: &str = "preferences.json";
/// The schema version this build reads and writes.
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub theme: ThemeSetting,
    pub language: Locale,
    /// The page shown at quit, as a route key (`T-42`, `inbox:P-1`): a
    /// record id or a fixed page key, never a title.
    pub page: Option<String>,
    /// Record ids of the projects and epics folded in the tree.
    pub collapsed: Vec<String>,
    /// Keys of the "3 closed" groups unfolded in the tree (`closed:E-3`).
    pub closed_shown: Vec<String>,
    /// Whether the sidebar is hidden (`Mod+B`).
    pub sidebar_hidden: bool,
    /// Terminal text size, 11 to 18; `None` is the default 13.
    pub terminal_text_size: Option<u8>,
    /// The chat panel lives in the bottom dock (default: the right one).
    pub chat_bottom: bool,
    /// The user has opened claude.ai in the panel: showing it starts the
    /// web view without asking again. Cleared by "Sign out and clear".
    pub chat_started: bool,
    /// The sign-in help notice was dismissed ("Got it").
    pub chat_help_dismissed: bool,
    /// "Sign out and clear" has not finished deleting the web view's
    /// profile: it is deleted at the next launch, before any web view.
    pub chat_clear_pending: bool,
    /// Pane sizes set by dragging a resize handle (docs/design/layout.md
    /// 4.2, 4.5); `None` is the default size.
    pub sidebar_width: Option<u16>,
    pub terminal_width: Option<u16>,
    pub chat_width: Option<u16>,
    pub chat_height: Option<u16>,
    /// The version that last ran ("0.4.1"): a launch on another version
    /// shows "Updated to ..." in the status bar for that session.
    pub last_version: Option<String>,
    /// The open tabs, as route keys (record ids or fixed page keys, never a
    /// title), when there are two or more; and the active one's index.
    pub tabs: Vec<String>,
    pub active_tab: usize,
}

/// The file as stored. Every field but the version may be missing (it then
/// takes its default); fields this build does not know are ignored.
#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    #[serde(default)]
    theme: StoredTheme,
    #[serde(default)]
    language: StoredLanguage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    page: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    collapsed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    closed_shown: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    sidebar_hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_text_size: Option<u8>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    chat_bottom: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    chat_started: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    chat_help_dismissed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    chat_clear_pending: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sidebar_width: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_width: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chat_width: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chat_height: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tabs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    active_tab: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// A stored version: digits and dots only, short.
fn is_version(v: &str) -> bool {
    !v.is_empty() && v.len() <= 16 && v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// A stored pane size: kept only when it is a plausible number of pixels.
fn pane_size(size: Option<u16>) -> Option<u16> {
    size.filter(|s| (100..=8000).contains(s))
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StoredLanguage {
    #[default]
    En,
    Fr,
}

impl From<StoredLanguage> for Locale {
    fn from(value: StoredLanguage) -> Self {
        match value {
            StoredLanguage::En => Locale::En,
            StoredLanguage::Fr => Locale::Fr,
        }
    }
}

impl From<Locale> for StoredLanguage {
    fn from(value: Locale) -> Self {
        match value {
            Locale::En => StoredLanguage::En,
            Locale::Fr => StoredLanguage::Fr,
        }
    }
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StoredTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl From<StoredTheme> for ThemeSetting {
    fn from(value: StoredTheme) -> Self {
        match value {
            StoredTheme::System => ThemeSetting::System,
            StoredTheme::Light => ThemeSetting::Light,
            StoredTheme::Dark => ThemeSetting::Dark,
        }
    }
}

impl From<ThemeSetting> for StoredTheme {
    fn from(value: ThemeSetting) -> Self {
        match value {
            ThemeSetting::System => StoredTheme::System,
            ThemeSetting::Light => StoredTheme::Light,
            ThemeSetting::Dark => StoredTheme::Dark,
        }
    }
}

/// Why the file was not used. Carries no text from the file or the system.
#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    Unreadable(io::ErrorKind),
    Malformed,
    UnsupportedVersion(u32),
}

/// The file's path for this user, or `None` when the data directory is
/// unknown (then nothing is read or written).
pub fn path() -> Option<PathBuf> {
    Some(paths::data_dir(Os::CURRENT, &BaseDirs::from_env())?.join(FILE_NAME))
}

/// Parse the file's contents.
pub fn parse(text: &str) -> Result<Preferences, LoadError> {
    #[derive(Deserialize)]
    struct Versioned {
        version: u32,
    }
    let versioned: Versioned = serde_json::from_str(text).map_err(|_| LoadError::Malformed)?;
    if versioned.version != VERSION {
        return Err(LoadError::UnsupportedVersion(versioned.version));
    }
    let stored: Stored = serde_json::from_str(text).map_err(|_| LoadError::Malformed)?;
    Ok(Preferences {
        theme: stored.theme.into(),
        language: stored.language.into(),
        page: stored.page.filter(|key| is_reference(key)),
        collapsed: stored
            .collapsed
            .into_iter()
            .filter(|k| is_reference(k))
            .collect(),
        closed_shown: stored
            .closed_shown
            .into_iter()
            .filter(|k| is_reference(k))
            .collect(),
        sidebar_hidden: stored.sidebar_hidden,
        terminal_text_size: stored
            .terminal_text_size
            .filter(|size| (11..=18).contains(size)),
        chat_bottom: stored.chat_bottom,
        chat_started: stored.chat_started,
        chat_help_dismissed: stored.chat_help_dismissed,
        chat_clear_pending: stored.chat_clear_pending,
        sidebar_width: pane_size(stored.sidebar_width),
        terminal_width: pane_size(stored.terminal_width),
        chat_width: pane_size(stored.chat_width),
        chat_height: pane_size(stored.chat_height),
        last_version: stored.last_version.filter(|v| is_version(v)),
        tabs: stored
            .tabs
            .into_iter()
            .filter(|k| is_reference(k))
            .collect(),
        active_tab: stored.active_tab,
    })
}

/// The file's contents for `prefs`.
pub fn serialize(prefs: &Preferences) -> String {
    let stored = Stored {
        version: VERSION,
        theme: prefs.theme.into(),
        language: prefs.language.into(),
        page: prefs.page.clone(),
        collapsed: prefs.collapsed.clone(),
        closed_shown: prefs.closed_shown.clone(),
        sidebar_hidden: prefs.sidebar_hidden,
        terminal_text_size: prefs.terminal_text_size,
        chat_bottom: prefs.chat_bottom,
        chat_started: prefs.chat_started,
        chat_help_dismissed: prefs.chat_help_dismissed,
        chat_clear_pending: prefs.chat_clear_pending,
        sidebar_width: prefs.sidebar_width,
        terminal_width: prefs.terminal_width,
        chat_width: prefs.chat_width,
        chat_height: prefs.chat_height,
        last_version: prefs.last_version.clone(),
        tabs: prefs.tabs.clone(),
        active_tab: prefs.active_tab,
    };
    // A struct of plain enums and numbers always serializes.
    let mut text = serde_json::to_string_pretty(&stored).unwrap_or_default();
    text.push('\n');
    text
}

/// Whether `key` has the shape of a record id or a fixed page key
/// (`T-42`, `closed:E-3`, `inbox:P-1`, `trash`): short, ASCII letters,
/// digits, `-` and `:` only. Anything else (a title someone typed into the
/// file) is dropped on load and so never written back.
fn is_reference(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 32
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':')
}

/// Read the preferences at `path`. A missing file is not an error.
pub fn load_from(path: &Path) -> Result<Preferences, LoadError> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(err) => Err(LoadError::Unreadable(err.kind())),
    }
}

/// Read this user's preferences, falling back to the defaults. Called once
/// at launch, before any window exists: the file is a few bytes.
///
/// A file this build cannot use (malformed, or written by another schema
/// version) is left in place and the defaults are used; the next change of
/// a setting replaces it with a version-1 file. A newer Acta that finds it
/// then reads version 1, which every later version must accept.
pub fn load() -> Preferences {
    let Some(path) = path() else {
        return Preferences::default();
    };
    remove_stale_temporaries(&path);
    load_from(&path).unwrap_or_else(|err| {
        crate::log::line(&format!(
            "preferences: not used, defaults instead (the next change replaces the file): {err:?}"
        ));
        Preferences::default()
    })
}

/// Delete `preferences.json.<pid>.tmp` files a crash left between writing
/// and renaming. Errors are ignored: a leftover costs a few bytes.
fn remove_stale_temporaries(path: &Path) {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let prefix = format!("{}.", name.to_string_lossy());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.file_name();
        let file = file.to_string_lossy();
        if file.starts_with(&prefix) && file.ends_with(".tmp") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Replace the file at `path` with `prefs`: write a temporary file next to
/// it, flush it to disk, then rename it over the old one, so a crash leaves
/// either the old file or the new one, never half of one.
pub fn save_to(path: &Path, prefs: &Preferences) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!("{FILE_NAME}.{}.tmp", std::process::id()));
    let written = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(serialize(prefs).as_bytes())?;
        file.sync_all()
    })();
    let result = written.and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Writes `prefs` as change number `seq` unless a later change is already on
/// disk. The lock also serializes the writer thread and [`Writer::flush`].
fn write_if_newer(path: &Path, written: &Mutex<u64>, seq: u64, prefs: &Preferences) {
    let mut last = written.lock().unwrap_or_else(|poison| poison.into_inner());
    if seq <= *last {
        return;
    }
    match save_to(path, prefs) {
        Ok(()) => *last = seq,
        Err(err) => crate::log::line(&format!("preferences: could not save: {:?}", err.kind())),
    }
}

/// Saves preferences off the UI thread. Every change gets a number; the
/// newest one wins, whether the writer thread or [`Writer::flush`] writes it.
pub struct Writer {
    path: Option<PathBuf>,
    sender: Option<Sender<(u64, Preferences)>>,
    seq: u64,
    written: Arc<Mutex<u64>>,
}

impl Writer {
    /// A writer for `path`. `None` writes nothing (no data directory).
    pub fn new(path: Option<PathBuf>) -> Self {
        let written = Arc::new(Mutex::new(0));
        let sender = path.clone().and_then(|path| {
            let (sender, receiver) = mpsc::channel::<(u64, Preferences)>();
            let written = Arc::clone(&written);
            let spawned = thread::Builder::new()
                .name("acta-preferences".into())
                .spawn(move || {
                    while let Ok(mut latest) = receiver.recv() {
                        // Only the newest of a burst of changes is written.
                        while let Ok(newer) = receiver.try_recv() {
                            latest = newer;
                        }
                        write_if_newer(&path, &written, latest.0, &latest.1);
                    }
                });
            match spawned {
                Ok(_) => Some(sender),
                Err(err) => {
                    crate::log::line(&format!(
                        "preferences: no writer thread, saving on quit only: {:?}",
                        err.kind()
                    ));
                    None
                }
            }
        });
        Self {
            path,
            sender,
            seq: 0,
            written,
        }
    }

    /// Queue `prefs` for the writer thread.
    pub fn save(&mut self, prefs: Preferences) {
        self.seq += 1;
        if let Some(sender) = &self.sender {
            let _ = sender.send((self.seq, prefs));
        }
    }

    /// Write `prefs` now, on the calling thread: at quit, and before an
    /// update restarts the app, when the writer thread may be killed with
    /// the process before it runs. Anything still queued is older and is
    /// then skipped.
    pub fn flush(&mut self, prefs: &Preferences) {
        let Some(path) = &self.path else {
            return;
        };
        let on_disk = *self
            .written
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if on_disk == self.seq {
            // Every change is on disk already.
            return;
        }
        self.seq += 1;
        write_if_newer(path, &self.written, self.seq, prefs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let prefs = load_from(&tmp.path().join(FILE_NAME)).unwrap();
        assert_eq!(prefs, Preferences::default());
        assert_eq!(prefs.theme, ThemeSetting::System);
        assert_eq!(prefs.language, Locale::En);
    }

    #[test]
    fn saved_preferences_read_back_and_leave_no_temporary_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ActaData").join(FILE_NAME);
        let prefs = Preferences {
            theme: ThemeSetting::Light,
            language: Locale::Fr,
            page: Some("T-42".into()),
            collapsed: vec!["P-1".into(), "E-3".into()],
            closed_shown: vec!["closed:E-3".into()],
            sidebar_hidden: true,
            terminal_text_size: Some(15),
            chat_bottom: true,
            chat_started: true,
            chat_help_dismissed: true,
            chat_clear_pending: true,
            sidebar_width: Some(300),
            terminal_width: Some(520),
            chat_width: Some(480),
            chat_height: Some(240),
            last_version: Some("0.3.0".into()),
            tabs: vec!["T-42".into(), "settings".into()],
            active_tab: 1,
        };
        save_to(&path, &prefs).unwrap();
        assert_eq!(load_from(&path).unwrap(), prefs);

        let dark = Preferences {
            theme: ThemeSetting::Dark,
            ..Preferences::default()
        };
        save_to(&path, &dark).unwrap();
        assert_eq!(load_from(&path).unwrap(), dark);

        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [FILE_NAME]);
    }

    #[test]
    fn the_file_holds_settings_only() {
        let text = serialize(&Preferences {
            theme: ThemeSetting::Dark,
            language: Locale::Fr,
            ..Preferences::default()
        });
        let expected = "{\n  \"version\": 1,\n  \"theme\": \"dark\",\n  \"language\": \"fr\"\n}\n";
        assert_eq!(text, expected);
    }

    #[test]
    fn corrupt_or_unknown_files_are_refused_with_their_kind() {
        assert_eq!(parse(""), Err(LoadError::Malformed));
        assert_eq!(parse("{\"theme\": \"dark\"}"), Err(LoadError::Malformed));
        assert_eq!(parse("not json at all"), Err(LoadError::Malformed));
        assert_eq!(
            parse("{\"version\": 1, \"theme\": \"sepia\"}"),
            Err(LoadError::Malformed)
        );
        assert_eq!(
            parse("{\"version\": 1, \"language\": \"de\"}"),
            Err(LoadError::Malformed)
        );
        assert_eq!(
            parse("{\"version\": 2, \"theme\": \"dark\"}"),
            Err(LoadError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn missing_fields_default_and_unknown_fields_are_ignored() {
        assert_eq!(parse("{\"version\": 1}"), Ok(Preferences::default()));
        let prefs = parse("{\"version\": 1, \"theme\": \"light\", \"future\": [1, 2]}").unwrap();
        assert_eq!(prefs.theme, ThemeSetting::Light);
    }

    #[test]
    fn only_record_ids_and_page_keys_are_kept() {
        let prefs = parse(
            "{\"version\": 1, \"page\": \"Embed the terminal\", \"collapsed\": [\"P-1\", \"my secret title\"], \"terminal_text_size\": 40}",
        )
        .unwrap();
        assert_eq!(prefs.page, None);
        assert_eq!(prefs.collapsed, ["P-1"]);
        assert_eq!(prefs.terminal_text_size, None);
    }

    #[test]
    fn implausible_pane_sizes_are_dropped() {
        let prefs =
            parse(r#"{"version":1,"sidebar_width":12,"terminal_width":520,"chat_height":9000}"#)
                .unwrap();
        assert_eq!(prefs.sidebar_width, None);
        assert_eq!(prefs.terminal_width, Some(520));
        assert_eq!(prefs.chat_height, None);
        let odd = parse(r#"{"version":1,"last_version":"Alexandre"}"#).unwrap();
        assert_eq!(odd.last_version, None);
    }

    #[test]
    fn a_directory_in_place_of_the_file_is_unreadable_not_a_crash() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        fs::create_dir(&path).unwrap();
        assert!(matches!(load_from(&path), Err(LoadError::Unreadable(_))));
    }

    #[test]
    fn a_flush_writes_the_last_change_now_and_queued_ones_never_overwrite_it() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        let mut writer = Writer::new(Some(path.clone()));
        let prefs = |theme| Preferences {
            theme,
            ..Preferences::default()
        };
        for theme in [ThemeSetting::Dark, ThemeSetting::Dark, ThemeSetting::Light] {
            writer.save(prefs(theme));
        }
        // The quit path: no Drop, no waiting for the thread.
        writer.flush(&prefs(ThemeSetting::Light));
        assert_eq!(load_from(&path).unwrap().theme, ThemeSetting::Light);
        // Whatever the thread still had queued is older and is skipped.
        thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(load_from(&path).unwrap().theme, ThemeSetting::Light);
    }

    #[test]
    fn a_flush_without_changes_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        let mut writer = Writer::new(Some(path.clone()));
        writer.flush(&Preferences::default());
        assert!(!path.exists());
    }

    #[test]
    fn the_writer_thread_saves_the_newest_change() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        let mut writer = Writer::new(Some(path.clone()));
        writer.save(Preferences {
            language: Locale::Fr,
            ..Preferences::default()
        });
        // Wait on the writer's own record of what it wrote, not on the
        // file's contents, with a deadline only a hung thread can miss: on a
        // loaded machine the thread may start late.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while *writer.written.lock().unwrap() < 1 {
            assert!(std::time::Instant::now() < deadline, "never written");
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(load_from(&path).map(|p| p.language), Ok(Locale::Fr));
    }

    #[test]
    fn stale_temporary_files_are_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        fs::write(tmp.path().join("preferences.json.4242.tmp"), "{").unwrap();
        fs::write(tmp.path().join("notes.tmp"), "kept").unwrap();
        fs::write(&path, "{\"version\": 1}").unwrap();
        remove_stale_temporaries(&path);
        let mut names: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["notes.tmp", FILE_NAME]);
    }
}
