//! Acta's shared, UI-free layer: app identity and per-OS paths, the
//! encrypted store (ADR 0003) and the domain model (ADR 0005 to 0008). No
//! GPUI type appears here, so the app, the CLI and the MCP server share it.

pub mod agent;
pub mod claude_settings;
pub mod date;
pub mod db;
pub mod error;
pub mod folder_identity;
pub mod folder_path;
pub mod fsutil;
pub mod git_cmd;
pub mod git_guard;
pub mod keys;
pub mod launch_plan;
pub mod link_index;
pub mod markdown;
pub mod model;
pub mod paths;
pub mod person;
pub mod prereqs;
pub mod presence;
pub mod process;
pub mod schema;
pub mod snapshots;
mod sql;
pub mod store;
pub mod time;
pub mod usage;
pub mod voice;

pub use db::ChangeWatcher;
pub use error::{Error, OpenError, Result};
pub use store::{Store, StoreConfig};

/// The `PATH` for children started by tests: this process's, without the
/// build-script output folders `cargo test` adds to it. whisper-rs-sys
/// (dictation, ADR 0014) adds over a hundred of them, which pushes `PATH`
/// past what `cmd.exe` accepts, and a test's shell then finds nothing.
#[cfg(test)]
pub(crate) fn test_child_path() -> std::ffi::OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let kept = std::env::split_paths(&path).filter(|dir| {
        let parts: Vec<_> = dir.components().map(|c| c.as_os_str().to_owned()).collect();
        let build = parts.iter().position(|c| c == "build");
        !build.is_some_and(|i| parts[i..].iter().any(|c| c == "out"))
    });
    std::env::join_paths(kept).unwrap_or(path)
}

/// Display name of the application.
pub const APP_NAME: &str = "Acta";
/// Velopack pack id (ADR 0004). Also the install directory name on Windows.
pub const PACK_ID: &str = "Acta";
/// Public repository; releases published here are the update feed.
pub const REPO_URL: &str = "https://github.com/Alegzandr/acta";
/// Version shared by every executable of the workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A random UUID, version 4: an opaque id for another program that checks
/// the form strictly (Claude Code's `--session-id`), and for tokens.
/// `None` only if the system random source fails.
pub fn random_uuid() -> Option<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).ok()?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    Some(uuid::Uuid::from_bytes(b).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_a_semver_triple() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }

    #[test]
    fn random_uuids_are_version_4() {
        let a = random_uuid().unwrap();
        assert_ne!(a, random_uuid().unwrap());
        let u = uuid::Uuid::parse_str(&a).unwrap();
        assert_eq!(u.get_version_num(), 4);
        assert_eq!(u.get_variant(), uuid::Variant::RFC4122);
    }

    #[test]
    fn repo_url_has_no_trailing_slash() {
        assert!(REPO_URL.starts_with("https://github.com/"));
        assert!(!REPO_URL.ends_with('/'));
    }
}
