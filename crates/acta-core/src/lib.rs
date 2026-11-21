//! Acta's shared, UI-free layer: app identity and per-OS paths, the
//! encrypted store (ADR 0003) and the domain model (ADR 0005 to 0008). No
//! GPUI type appears here, so the app, the CLI and the MCP server share it.

pub mod agent;
pub mod db;
pub mod error;
pub mod folder_path;
mod fsutil;
pub mod keys;
pub mod link_index;
pub mod markdown;
pub mod model;
pub mod paths;
pub mod process;
pub mod schema;
pub mod snapshots;
mod sql;
pub mod store;
pub mod time;

pub use db::ChangeWatcher;
pub use error::{Error, OpenError, Result};
pub use store::{Store, StoreConfig};

/// Display name of the application.
pub const APP_NAME: &str = "Acta";
/// Velopack pack id (ADR 0004). Also the install directory name on Windows.
pub const PACK_ID: &str = "Acta";
/// Public repository; releases published here are the update feed.
pub const REPO_URL: &str = "https://github.com/Alegzandr/acta";
/// Version shared by every executable of the workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
    fn repo_url_has_no_trailing_slash() {
        assert!(REPO_URL.starts_with("https://github.com/"));
        assert!(!REPO_URL.ends_with('/'));
    }
}
