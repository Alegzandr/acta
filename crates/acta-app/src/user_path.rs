//! Makes the exported `acta` reachable from a shell (ADR 0004).
//!
//! Windows: the export directory is appended to `HKCU\Environment\Path`.
//! macOS and Linux: `~/.local/bin/acta` is a symlink to the exported file.
//! The decisions are pure functions; only thin wrappers touch the registry or
//! the file system.

use std::io;

use crate::cli_export::Layout;

/// Put the exported CLI on the user's PATH. Returns what was done, for the log.
pub fn publish(layout: &Layout) -> io::Result<&'static str> {
    #[cfg(windows)]
    {
        let changed = registry::edit(|current| path_var::with_dir(current, &dir_text(layout)))?;
        Ok(if changed {
            "added to user PATH"
        } else {
            "already on user PATH"
        })
    }
    #[cfg(unix)]
    {
        symlink::ensure(layout)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = layout;
        Ok("no PATH integration on this platform")
    }
}

/// Take the export directory off the user's PATH again (uninstall).
#[cfg(windows)]
pub fn unpublish(layout: &Layout) -> io::Result<&'static str> {
    let changed = registry::edit(|current| path_var::without_dir(current, &dir_text(layout)))?;
    Ok(if changed {
        "removed from user PATH"
    } else {
        "was not on user PATH"
    })
}

#[cfg(windows)]
fn dir_text(layout: &Layout) -> String {
    layout.dir.to_string_lossy().into_owned()
}

/// Editing of a Windows `Path` value: `;`-separated, case-insensitive.
#[cfg(any(windows, test))]
mod path_var {
    fn same_dir(entry: &str, dir: &str) -> bool {
        let trim = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_ascii_lowercase();
        trim(entry) == trim(dir)
    }

    /// `current` with `dir` appended, or `None` when it is already listed.
    /// Existing entries are kept byte for byte.
    pub fn with_dir(current: &str, dir: &str) -> Option<String> {
        if current.split(';').any(|entry| same_dir(entry, dir)) {
            return None;
        }
        Some(if current.is_empty() || current.ends_with(';') {
            format!("{current}{dir}")
        } else {
            format!("{current};{dir}")
        })
    }

    /// `current` without `dir`, or `None` when it is not listed. Every other
    /// entry, their order and a trailing `;` are preserved.
    pub fn without_dir(current: &str, dir: &str) -> Option<String> {
        if !current.split(';').any(|entry| same_dir(entry, dir)) {
            return None;
        }
        let trailing = current.ends_with(';');
        let body = current.strip_suffix(';').unwrap_or(current);
        let mut kept = body
            .split(';')
            .filter(|entry| !same_dir(entry, dir))
            .collect::<Vec<_>>()
            .join(";");
        if trailing && !kept.is_empty() {
            kept.push(';');
        }
        Some(kept)
    }
}

#[cfg(windows)]
mod registry {
    use std::io;

    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, RegType};
    use winreg::{RegKey, RegValue};

    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };

    /// The text of a `REG_SZ` / `REG_EXPAND_SZ` value: UTF-16LE, usually
    /// NUL-terminated (the registry does not guarantee it).
    pub(super) fn decode(bytes: &[u8]) -> String {
        let wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&wide)
            .trim_end_matches('\0')
            .to_string()
    }

    pub(super) fn encode(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    /// Tell running programs (Explorer, new terminals) that the environment changed.
    fn broadcast() {
        let scope: Vec<u16> = "Environment\0".encode_utf16().collect();
        let mut result = 0usize;
        // SAFETY: `scope` is a NUL-terminated UTF-16 string that outlives the
        // call, and `result` is a valid out pointer. The call is bounded by
        // its own timeout.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                scope.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                2000,
                &mut result,
            );
        }
    }

    /// Apply `change` to `HKCU\Environment\Path`, keeping the value type
    /// (`REG_EXPAND_SZ` entries such as `%USERPROFILE%\bin` must stay
    /// expandable). `change` returns `None` when nothing has to be written.
    pub fn edit(change: impl FnOnce(&str) -> Option<String>) -> io::Result<bool> {
        let env = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
        let (current, vtype) = match env.get_raw_value("Path") {
            Ok(value) => {
                if !matches!(value.vtype, RegType::REG_SZ | RegType::REG_EXPAND_SZ) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "user Path has an unexpected registry type",
                    ));
                }
                (decode(&value.bytes), value.vtype)
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                (String::new(), RegType::REG_EXPAND_SZ)
            }
            Err(err) => return Err(err),
        };
        let Some(updated) = change(&current) else {
            return Ok(false);
        };
        let value = RegValue {
            bytes: encode(&updated),
            vtype,
        };
        env.set_raw_value("Path", &value)?;
        broadcast();
        Ok(true)
    }
}

/// What to do about `~/.local/bin/acta` given what is there now.
#[cfg(any(unix, test))]
mod link_rule {
    use std::path::{Path, PathBuf};

    #[derive(Debug, PartialEq, Eq)]
    pub enum Existing {
        Missing,
        SymlinkTo(PathBuf),
        /// A regular file or directory the user (or another tool) put there.
        Other,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum Action {
        Create,
        Replace,
        Keep,
        /// Never overwrite something that is not our symlink.
        LeaveAlone,
    }

    pub fn decide(existing: &Existing, target: &Path) -> Action {
        match existing {
            Existing::Missing => Action::Create,
            Existing::SymlinkTo(current) if current == target => Action::Keep,
            Existing::SymlinkTo(_) => Action::Replace,
            Existing::Other => Action::LeaveAlone,
        }
    }
}

#[cfg(unix)]
mod symlink {
    use std::fs;
    use std::io;

    use super::link_rule::{Action, Existing, decide};
    use crate::cli_export::Layout;

    pub fn ensure(layout: &Layout) -> io::Result<&'static str> {
        let Some(link_dir) = &layout.link_dir else {
            return Ok("no link directory");
        };
        let target = layout.exported_exe();
        let link = link_dir.join(layout.exe_name);
        let existing = match fs::symlink_metadata(&link) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Existing::Missing,
            Err(err) => return Err(err),
            Ok(meta) if meta.file_type().is_symlink() => Existing::SymlinkTo(fs::read_link(&link)?),
            Ok(_) => Existing::Other,
        };
        match decide(&existing, &target) {
            Action::Keep => Ok("symlink already in place"),
            Action::LeaveAlone => Ok("a different file is at the link path; left untouched"),
            Action::Create => {
                fs::create_dir_all(link_dir)?;
                std::os::unix::fs::symlink(&target, &link)?;
                Ok("symlink created")
            }
            Action::Replace => {
                fs::remove_file(&link)?;
                std::os::unix::fs::symlink(&target, &link)?;
                Ok("symlink updated")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::link_rule::{Action, Existing, decide};
    use super::path_var::{with_dir, without_dir};

    const DIR: &str = r"C:\Users\Alexandre\AppData\Local\ActaCli\bin";

    #[test]
    fn appends_to_an_existing_value() {
        assert_eq!(
            with_dir(r"C:\tools;%USERPROFILE%\bin", DIR),
            Some(format!(r"C:\tools;%USERPROFILE%\bin;{DIR}"))
        );
    }

    #[test]
    fn appends_after_a_trailing_separator_without_doubling_it() {
        assert_eq!(
            with_dir(r"C:\tools;", DIR),
            Some(format!(r"C:\tools;{DIR}"))
        );
    }

    #[test]
    fn empty_value_becomes_just_the_directory() {
        assert_eq!(with_dir("", DIR), Some(DIR.to_string()));
    }

    #[test]
    fn adding_twice_changes_nothing() {
        let once = with_dir(r"C:\tools", DIR).unwrap();
        assert_eq!(with_dir(&once, DIR), None);
    }

    #[test]
    fn presence_ignores_case_and_a_trailing_backslash() {
        let listed = format!(r"C:\tools;{}\", DIR.to_uppercase());
        assert_eq!(with_dir(&listed, DIR), None);
    }

    #[test]
    fn a_longer_directory_with_the_same_prefix_is_not_a_match() {
        let other = format!(r"{DIR}-old");
        assert!(with_dir(&other, DIR).is_some());
        assert_eq!(without_dir(&other, DIR), None);
    }

    #[test]
    fn removal_restores_the_original_value_exactly() {
        for original in [
            r"C:\tools;%USERPROFILE%\bin",
            r"C:\tools;",
            r"C:\a;;C:\b",
            "",
        ] {
            let added = with_dir(original, DIR).unwrap();
            let removed = without_dir(&added, DIR).unwrap();
            let expected = original.strip_suffix(';').unwrap_or(original);
            // Appending after a trailing `;` consumes it; nothing else changes.
            assert_eq!(removed, expected, "from {original:?}");
        }
    }

    #[test]
    fn removal_keeps_other_entries_order_and_trailing_separator() {
        let value = format!(r"C:\a;{DIR};C:\b;");
        assert_eq!(without_dir(&value, DIR), Some(r"C:\a;C:\b;".to_string()));
    }

    #[test]
    fn removing_an_absent_directory_changes_nothing() {
        assert_eq!(without_dir(r"C:\tools", DIR), None);
        assert_eq!(without_dir("", DIR), None);
    }

    #[cfg(windows)]
    #[test]
    fn registry_text_round_trips_with_or_without_a_terminator() {
        use super::registry::{decode, encode};

        let value = format!("%USERPROFILE%\\bin;caf\u{e9};{DIR}");
        let bytes = encode(&value);
        assert_eq!(bytes.len(), (value.encode_utf16().count() + 1) * 2);
        assert_eq!(bytes[bytes.len() - 2..], [0, 0]);
        assert_eq!(decode(&bytes), value);
        // Values written without the terminator, or empty, read the same.
        assert_eq!(decode(&bytes[..bytes.len() - 2]), value);
        assert_eq!(decode(&[]), "");
        assert_eq!(decode(&encode("")), "");
    }

    #[test]
    fn symlink_rule() {
        let target = Path::new("/home/alexandre/.local/share/acta/bin/acta");
        assert_eq!(decide(&Existing::Missing, target), Action::Create);
        assert_eq!(
            decide(&Existing::SymlinkTo(target.to_path_buf()), target),
            Action::Keep
        );
        assert_eq!(
            decide(
                &Existing::SymlinkTo(PathBuf::from("/elsewhere/acta")),
                target
            ),
            Action::Replace
        );
        assert_eq!(decide(&Existing::Other, target), Action::LeaveAlone);
    }
}
