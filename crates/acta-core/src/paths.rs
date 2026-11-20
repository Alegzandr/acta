//! Per-OS locations: the data directory (ADR 0003), the exported CLI
//! (ADR 0004) and the diagnostic log.
//!
//! Every function is pure: it takes the OS and the base directories as
//! arguments, so all three layouts are tested on any host.
//!
//! | | Windows | macOS | Linux |
//! |---|---|---|---|
//! | data | `%LOCALAPPDATA%\ActaData` | `~/Library/Application Support/Acta/data` | `$XDG_DATA_HOME/acta/data` |
//! | CLI root | `%LOCALAPPDATA%\ActaCli` | `~/Library/Application Support/ActaCli` | `$XDG_DATA_HOME/acta-cli` |
//! | log | `%LOCALAPPDATA%\ActaData\logs\acta.log` | `~/Library/Logs/Acta/acta.log` | `$XDG_STATE_HOME/acta/acta.log` |
//!
//! The CLI root holds nothing but the exported CLI and is deleted whole at
//! uninstall. On every OS it is a directory of its own beside the one that
//! holds the data, so deleting it can never take the database or its key
//! along.

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
    /// `$XDG_DATA_HOME` when set to an absolute path; only read on Linux.
    pub xdg_data_home: Option<PathBuf>,
    /// `$XDG_STATE_HOME` when set to an absolute path; only read on Linux.
    pub xdg_state_home: Option<PathBuf>,
}

impl BaseDirs {
    pub fn from_env() -> Self {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        // The XDG specification says a relative value must be ignored.
        let xdg = |name: &str| var(name).filter(|dir| dir.is_absolute());
        Self {
            home: if cfg!(windows) {
                var("USERPROFILE")
            } else {
                var("HOME")
            },
            local_app_data: var("LOCALAPPDATA"),
            xdg_data_home: xdg("XDG_DATA_HOME"),
            xdg_state_home: xdg("XDG_STATE_HOME"),
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

/// Where the encrypted database, its key file and its snapshots live
/// (ADR 0003), with `preferences.json` (settings only, DESIGN.md 4.7) and,
/// on Windows, the diagnostic log. Uninstall never deletes it, and the
/// directories that do get deleted are proven to stay clear of it.
///
/// On Windows this is deliberately not under `%LOCALAPPDATA%\Acta`: that is
/// the Velopack root, which uninstall deletes.
pub fn data_dir(os: Os, base: &BaseDirs) -> Option<PathBuf> {
    match os {
        Os::Windows => Some(base.local_app_data.as_ref()?.join("ActaData")),
        Os::MacOs => Some(
            mac_app_support(base.home.as_ref()?)
                .join("Acta")
                .join("data"),
        ),
        Os::Linux => Some(xdg_data_home(base)?.join("acta").join("data")),
    }
}

/// The directory that holds the exported CLI and nothing else. Uninstall
/// deletes it whole, so it must never contain anything of the user's.
pub fn cli_root(os: Os, base: &BaseDirs) -> Option<PathBuf> {
    match os {
        Os::Windows => Some(base.local_app_data.as_ref()?.join("ActaCli")),
        Os::MacOs => Some(mac_app_support(base.home.as_ref()?).join("ActaCli")),
        Os::Linux => Some(xdg_data_home(base)?.join("acta-cli")),
    }
}

/// Stable per-user directory the CLI is exported to, outside the Velopack
/// root: `bin` inside [`cli_root`].
pub fn cli_export_dir(os: Os, base: &BaseDirs) -> Option<PathBuf> {
    Some(cli_root(os, base)?.join("bin"))
}

/// Directory holding the `acta` symlink on macOS and Linux (`~/.local/bin`).
/// Windows puts the export directory itself on the user PATH instead.
pub fn cli_link_dir(os: Os, base: &BaseDirs) -> Option<PathBuf> {
    match os {
        Os::Windows => None,
        Os::MacOs | Os::Linux => Some(base.home.as_ref()?.join(".local").join("bin")),
    }
}

/// The diagnostic log of the GUI: per user, never in a shared temp directory.
pub fn log_file(os: Os, base: &BaseDirs) -> Option<PathBuf> {
    let dir = match os {
        Os::Windows => data_dir(os, base)?.join("logs"),
        Os::MacOs => base
            .home
            .as_ref()?
            .join("Library")
            .join("Logs")
            .join("Acta"),
        Os::Linux => match &base.xdg_state_home {
            Some(dir) => dir.join("acta"),
            None => base
                .home
                .as_ref()?
                .join(".local")
                .join("state")
                .join("acta"),
        },
    };
    Some(dir.join("acta.log"))
}

fn mac_app_support(home: &Path) -> PathBuf {
    home.join("Library").join("Application Support")
}

/// `$XDG_DATA_HOME`, or its default `~/.local/share` (Linux).
pub fn xdg_data_home(base: &BaseDirs) -> Option<PathBuf> {
    match &base.xdg_data_home {
        Some(dir) => Some(dir.clone()),
        None => Some(base.home.as_ref()?.join(".local").join("share")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Os; 3] = [Os::Windows, Os::MacOs, Os::Linux];

    fn home() -> PathBuf {
        PathBuf::from("home").join("alexandre")
    }

    fn base() -> BaseDirs {
        BaseDirs {
            home: Some(home()),
            local_app_data: Some(PathBuf::from("appdata").join("Local")),
            ..BaseDirs::default()
        }
    }

    fn with_xdg() -> BaseDirs {
        BaseDirs {
            xdg_data_home: Some(PathBuf::from("xdg").join("data")),
            xdg_state_home: Some(PathBuf::from("xdg").join("state")),
            ..base()
        }
    }

    #[test]
    fn windows_layout() {
        let local = PathBuf::from("appdata").join("Local");
        let b = base();
        assert_eq!(data_dir(Os::Windows, &b), Some(local.join("ActaData")));
        assert_eq!(cli_root(Os::Windows, &b), Some(local.join("ActaCli")));
        assert_eq!(
            cli_export_dir(Os::Windows, &b),
            Some(local.join("ActaCli").join("bin"))
        );
        assert_eq!(cli_link_dir(Os::Windows, &b), None);
        assert_eq!(
            log_file(Os::Windows, &b),
            Some(local.join("ActaData").join("logs").join("acta.log"))
        );
        assert_eq!(packaged_cli_name(Os::Windows), "acta-cli.exe");
        assert_eq!(exported_cli_name(Os::Windows), "acta.exe");
    }

    #[test]
    fn macos_layout() {
        let support = home().join("Library").join("Application Support");
        let b = base();
        assert_eq!(
            data_dir(Os::MacOs, &b),
            Some(support.join("Acta").join("data"))
        );
        assert_eq!(cli_root(Os::MacOs, &b), Some(support.join("ActaCli")));
        assert_eq!(
            cli_export_dir(Os::MacOs, &b),
            Some(support.join("ActaCli").join("bin"))
        );
        assert_eq!(
            cli_link_dir(Os::MacOs, &b),
            Some(home().join(".local").join("bin"))
        );
        assert_eq!(
            log_file(Os::MacOs, &b),
            Some(
                home()
                    .join("Library")
                    .join("Logs")
                    .join("Acta")
                    .join("acta.log")
            )
        );
        assert_eq!(exported_cli_name(Os::MacOs), "acta");
    }

    #[test]
    fn linux_layout_without_xdg_variables() {
        let share = home().join(".local").join("share");
        let b = base();
        assert_eq!(
            data_dir(Os::Linux, &b),
            Some(share.join("acta").join("data"))
        );
        assert_eq!(cli_root(Os::Linux, &b), Some(share.join("acta-cli")));
        assert_eq!(
            cli_export_dir(Os::Linux, &b),
            Some(share.join("acta-cli").join("bin"))
        );
        assert_eq!(
            cli_link_dir(Os::Linux, &b),
            Some(home().join(".local").join("bin"))
        );
        assert_eq!(
            log_file(Os::Linux, &b),
            Some(
                home()
                    .join(".local")
                    .join("state")
                    .join("acta")
                    .join("acta.log")
            )
        );
        assert_eq!(packaged_cli_name(Os::Linux), "acta-cli");
    }

    #[test]
    fn linux_layout_follows_the_xdg_variables() {
        let b = with_xdg();
        let data = PathBuf::from("xdg").join("data");
        assert_eq!(
            data_dir(Os::Linux, &b),
            Some(data.join("acta").join("data"))
        );
        assert_eq!(
            cli_export_dir(Os::Linux, &b),
            Some(data.join("acta-cli").join("bin"))
        );
        assert_eq!(
            log_file(Os::Linux, &b),
            Some(
                PathBuf::from("xdg")
                    .join("state")
                    .join("acta")
                    .join("acta.log")
            )
        );
        // The symlink directory is not an XDG data location.
        assert_eq!(
            cli_link_dir(Os::Linux, &b),
            Some(home().join(".local").join("bin"))
        );
        // The XDG variables alone are enough for data, CLI and log.
        let only_xdg = BaseDirs {
            home: None,
            local_app_data: None,
            ..with_xdg()
        };
        assert!(data_dir(Os::Linux, &only_xdg).is_some());
        assert!(cli_root(Os::Linux, &only_xdg).is_some());
        assert!(log_file(Os::Linux, &only_xdg).is_some());
    }

    #[test]
    fn xdg_variables_change_nothing_on_windows_and_macos() {
        for os in [Os::Windows, Os::MacOs] {
            assert_eq!(data_dir(os, &with_xdg()), data_dir(os, &base()));
            assert_eq!(cli_root(os, &with_xdg()), cli_root(os, &base()));
            assert_eq!(log_file(os, &with_xdg()), log_file(os, &base()));
        }
    }

    /// The CLI root is deleted whole at uninstall. On no OS, with or without
    /// XDG variables, may that touch the data directory, the log or the link
    /// directory.
    #[test]
    fn deleting_the_cli_root_can_never_delete_user_data() {
        for os in ALL {
            for b in [base(), with_xdg()] {
                let root = cli_root(os, &b).unwrap();
                let data = data_dir(os, &b).unwrap();
                let log = log_file(os, &b).unwrap();

                assert_ne!(root, data, "{os:?}");
                assert!(!data.starts_with(&root), "{os:?}: data inside the root");
                assert!(!root.starts_with(&data), "{os:?}: root inside the data");
                assert!(!log.starts_with(&root), "{os:?}: log inside the root");
                if let Some(link_dir) = cli_link_dir(os, &b) {
                    assert!(!link_dir.starts_with(&root), "{os:?}");
                }
                // The export itself is what the root is for.
                assert!(cli_export_dir(os, &b).unwrap().starts_with(&root));
            }
        }
    }

    #[test]
    fn nothing_on_windows_lives_in_the_velopack_root() {
        // Velopack installs to %LOCALAPPDATA%\<pack id>, kills every process
        // running from there on update and deletes it on uninstall.
        let velopack = PathBuf::from("appdata").join("Local").join(crate::PACK_ID);
        let b = base();
        for path in [
            cli_root(Os::Windows, &b).unwrap(),
            data_dir(Os::Windows, &b).unwrap(),
            log_file(Os::Windows, &b).unwrap(),
        ] {
            assert!(!path.starts_with(&velopack), "{path:?}");
        }
    }

    #[test]
    fn missing_base_dirs_give_none_instead_of_a_relative_path() {
        let empty = BaseDirs::default();
        for os in ALL {
            assert_eq!(data_dir(os, &empty), None);
            assert_eq!(cli_root(os, &empty), None);
            assert_eq!(cli_export_dir(os, &empty), None);
            assert_eq!(cli_link_dir(os, &empty), None);
            assert_eq!(log_file(os, &empty), None);
        }
    }
}
