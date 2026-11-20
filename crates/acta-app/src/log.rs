//! Minimal diagnostic log for the install, export and update paths.
//!
//! A GUI-subsystem process has no console, so lines go to a per-user file
//! (`acta_core::paths::log_file`): `%LOCALAPPDATA%\ActaData\logs\acta.log`,
//! `~/Library/Logs/Acta/acta.log`, `$XDG_STATE_HOME/acta/acta.log`. Never a
//! shared temp directory; on Unix the file is readable by its owner only.
//! Messages carry no user data: operations and error kinds only. Logging
//! never fails the caller.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use acta_core::paths::{self, BaseDirs, Os};

const MAX_BYTES: u64 = 256 * 1024;

/// The log file of this user, resolved once. `None` when the user's
/// directories are unknown: then nothing is logged.
fn path() -> Option<&'static Path> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| paths::log_file(Os::CURRENT, &BaseDirs::from_env()))
        .as_deref()
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Start the log over once it has grown past its cap. Called once at launch.
pub fn rotate() {
    if let Some(path) = path()
        && fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES)
    {
        let _ = fs::remove_file(path);
    }
}

/// Open `path` for appending, creating it and its directory when missing.
fn open(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Applies when the file is created; an existing file keeps its mode.
        options.mode(0o600);
    }
    options.open(path)
}

pub fn line(message: &str) {
    // Unit tests exercise code that logs; they must not write to the real log.
    if cfg!(test) {
        return;
    }
    if cfg!(debug_assertions) {
        eprintln!("[acta] {message}");
    }
    if let Some(path) = path()
        && let Ok(mut file) = open(path)
    {
        let _ = writeln!(
            file,
            "{} [{} pid {}] {message}",
            now_ms(),
            acta_core::VERSION,
            std::process::id()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_the_directory_and_appends() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("logs").join("nested").join("acta.log");

        writeln!(open(&path).unwrap(), "first").unwrap();
        writeln!(open(&path).unwrap(), "second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "first\nsecond\n");
    }

    #[cfg(unix)]
    #[test]
    fn the_log_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("acta.log");
        drop(open(&path).unwrap());

        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
