//! Minimal diagnostic log for the install, export and update paths.
//!
//! A GUI-subsystem process has no console, so lines go to `acta.log` in the
//! system temp directory. Messages carry no user data: operations and error
//! kinds only. Logging never fails the caller.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 256 * 1024;

fn path() -> PathBuf {
    std::env::temp_dir().join("acta.log")
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Start the log over once it has grown past its cap. Called once at launch.
pub fn rotate() {
    let path = path();
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::remove_file(path);
    }
}

pub fn line(message: &str) {
    // Unit tests exercise code that logs; they must not write to the real log.
    if cfg!(test) {
        return;
    }
    if cfg!(debug_assertions) {
        eprintln!("[acta] {message}");
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path()) {
        let _ = writeln!(
            file,
            "{} [{} pid {}] {message}",
            now_ms(),
            acta_core::VERSION,
            std::process::id()
        );
    }
}
