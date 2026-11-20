//! Per-OS locations of the exported CLI (ADR 0004). The data directory of
//! ADR 0003 arrives with the storage layer in M2.
//!
//! Every function is pure: it takes the OS and the base directories as
//! arguments, so all three layouts are tested on any host.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    /// The OS this build targets. Other Unix systems follow the Linux layout.
    pub const CURRENT: Os = if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Linux
    };
}

/// The directories everything else is derived from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BaseDirs {
    /// The user's home directory (`HOME`, or `USERPROFILE` on Windows).
    pub home: Option<PathBuf>,
    /// `%LOCALAPPDATA%`; only read on Windows.
    pub local_app_data: Option<PathBuf>,
}

impl BaseDirs {
    pub fn from_env() -> Self {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            home: if cfg!(windows) {
                var("USERPROFILE")
            } else {
                var("HOME")
            },
            local_app_data: var("LOCALAPPDATA"),
        }
    }
}

/// File name of the CLI inside the package, next to the GUI executable.
pub fn packaged_cli_name(os: Os) -> &'static str {
    match os {
        Os::Windows => "acta-cli.exe",
        Os::MacOs | Os::Linux => "acta-cli",
    }
}

/// File name of the CLI once exported for the user.
pub fn exported_cli_name(os: Os) -> &'static str {
    match os {
        Os::Windows => "acta.exe",
        Os::MacOs | Os::Linux => "acta",
    }
}

/// Stable per-user directory the CLI is exported to, outside the Velopack
/// root: `%LOCALAPPDATA%\ActaCli\bin`, `~/Library/Application Support/Acta/bin`,
/// `~/.local/share/acta/bin`.
pub fn cli_export_dir(os: Os, base: &BaseDirs) -> Option<PathBuf> {
    match os {
        Os::Windows => Some(base.local_app_data.as_ref()?.join("ActaCli").join("bin")),
        Os::MacOs => Some(mac_app_support(base.home.as_ref()?).join("bin")),
        Os::Linux => Some(linux_share(base.home.as_ref()?).join("bin")),
    }
}

/// Directory holding the `acta` symlink on macOS and Linux (`~/.local/bin`).
/// Windows puts the export directory itself on the user PATH instead.
pub fn cli_link_dir(os: Os, base: &BaseDirs) -> Option<PathBuf> {
    match os {
        Os::Windows => None,
        Os::MacOs | Os::Linux => Some(base.home.as_ref()?.join(".local").join("bin")),
    }
}

fn mac_app_support(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join("Acta")
}

fn linux_share(home: &Path) -> PathBuf {
    home.join(".local").join("share").join("acta")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> BaseDirs {
        BaseDirs {
            home: Some(PathBuf::from("home").join("alexandre")),
            local_app_data: Some(PathBuf::from("appdata").join("Local")),
        }
    }

    #[test]
    fn windows_layout() {
        let local = PathBuf::from("appdata").join("Local");
        assert_eq!(
            cli_export_dir(Os::Windows, &base()),
            Some(local.join("ActaCli").join("bin"))
        );
        assert_eq!(cli_link_dir(Os::Windows, &base()), None);
        assert_eq!(packaged_cli_name(Os::Windows), "acta-cli.exe");
        assert_eq!(exported_cli_name(Os::Windows), "acta.exe");
    }

    #[test]
    fn macos_layout() {
        let home = PathBuf::from("home").join("alexandre");
        let support = home
            .join("Library")
            .join("Application Support")
            .join("Acta");
        assert_eq!(
            cli_export_dir(Os::MacOs, &base()),
            Some(support.join("bin"))
        );
        assert_eq!(
            cli_link_dir(Os::MacOs, &base()),
            Some(home.join(".local").join("bin"))
        );
        assert_eq!(exported_cli_name(Os::MacOs), "acta");
    }

    #[test]
    fn linux_layout() {
        let home = PathBuf::from("home").join("alexandre");
        let share = home.join(".local").join("share").join("acta");
        assert_eq!(cli_export_dir(Os::Linux, &base()), Some(share.join("bin")));
        assert_eq!(
            cli_link_dir(Os::Linux, &base()),
            Some(home.join(".local").join("bin"))
        );
        assert_eq!(packaged_cli_name(Os::Linux), "acta-cli");
    }

    #[test]
    fn the_windows_export_is_outside_the_velopack_root() {
        // Velopack installs to %LOCALAPPDATA%\<pack id>, kills every process
        // running from there on update and deletes it on uninstall.
        let root = PathBuf::from("appdata").join("Local").join(crate::PACK_ID);
        let export = cli_export_dir(Os::Windows, &base()).unwrap();
        assert!(!export.starts_with(&root));
    }

    #[test]
    fn missing_base_dirs_give_none_instead_of_a_relative_path() {
        let empty = BaseDirs::default();
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            assert_eq!(cli_export_dir(os, &empty), None);
            assert_eq!(cli_link_dir(os, &empty), None);
        }
    }
}
