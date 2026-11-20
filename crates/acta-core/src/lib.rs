//! Acta's shared, UI-free layer. For now: app identity and per-OS paths.

pub mod paths;

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
