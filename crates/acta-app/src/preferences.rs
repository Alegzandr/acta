//! `preferences.json` (DESIGN.md 4.7): settings that must be readable before
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
//! change is never overwritten by an earlier one.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};

use acta_core::paths::{self, BaseDirs, Os};

use crate::theme::ThemeSetting;

pub const FILE_NAME: &str = "preferences.json";
/// The schema version this build reads and writes.
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub theme: ThemeSetting,
}

/// The file as stored. Every field but the version may be missing (it then
/// takes its default); fields this build does not know are ignored.
#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    #[serde(default)]
    theme: StoredTheme,
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
    })
}

/// The file's contents for `prefs`.
pub fn serialize(prefs: &Preferences) -> String {
    let stored = Stored {
        version: VERSION,
        theme: prefs.theme.into(),
    };
    // A struct of plain enums and numbers always serializes.
    let mut text = serde_json::to_string_pretty(&stored).unwrap_or_default();
    text.push('\n');
    text
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
pub fn load() -> Preferences {
    let Some(path) = path() else {
        return Preferences::default();
    };
    load_from(&path).unwrap_or_else(|err| {
        crate::log::line(&format!("preferences: not used, defaults instead: {err:?}"));
        Preferences::default()
    })
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

/// Hands preferences to a writer thread; the latest one queued wins.
pub struct Writer {
    sender: Option<Sender<Preferences>>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /// A writer for `path`. `None` writes nothing (no data directory).
    pub fn new(path: Option<PathBuf>) -> Self {
        let Some(path) = path else {
            return Self::none();
        };
        let (sender, receiver) = mpsc::channel::<Preferences>();
        let spawned = thread::Builder::new()
            .name("acta-preferences".into())
            .spawn(move || {
                while let Ok(mut prefs) = receiver.recv() {
                    // Only the newest of a burst of changes is written.
                    while let Ok(newer) = receiver.try_recv() {
                        prefs = newer;
                    }
                    if let Err(err) = save_to(&path, &prefs) {
                        crate::log::line(&format!("preferences: could not save: {:?}", err.kind()));
                    }
                }
            });
        match spawned {
            Ok(thread) => Self {
                sender: Some(sender),
                thread: Some(thread),
            },
            Err(err) => {
                crate::log::line(&format!("preferences: no writer thread: {:?}", err.kind()));
                Self::none()
            }
        }
    }

    fn none() -> Self {
        Self {
            sender: None,
            thread: None,
        }
    }

    pub fn save(&self, prefs: Preferences) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(prefs);
        }
    }
}

impl Drop for Writer {
    /// Write what is still queued before the writer goes away.
    fn drop(&mut self) {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
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
    }

    #[test]
    fn saved_preferences_read_back_and_leave_no_temporary_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ActaData").join(FILE_NAME);
        let prefs = Preferences {
            theme: ThemeSetting::Light,
        };
        save_to(&path, &prefs).unwrap();
        assert_eq!(load_from(&path).unwrap(), prefs);

        let dark = Preferences {
            theme: ThemeSetting::Dark,
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
        });
        assert_eq!(text, "{\n  \"version\": 1,\n  \"theme\": \"dark\"\n}\n");
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
    fn a_directory_in_place_of_the_file_is_unreadable_not_a_crash() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        fs::create_dir(&path).unwrap();
        assert!(matches!(load_from(&path), Err(LoadError::Unreadable(_))));
    }

    #[test]
    fn the_writer_keeps_the_last_change() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        let writer = Writer::new(Some(path.clone()));
        for theme in [ThemeSetting::Dark, ThemeSetting::Dark, ThemeSetting::Light] {
            writer.save(Preferences { theme });
        }
        drop(writer);
        assert_eq!(load_from(&path).unwrap().theme, ThemeSetting::Light);
    }
}
