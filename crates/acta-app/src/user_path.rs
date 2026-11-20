//! Makes the exported `acta` reachable from a shell (ADR 0004).
//!
//! Windows: the export directory is appended to `HKCU\Environment\Path`.
//! macOS and Linux: `~/.local/bin/acta` is a symlink to the exported file.
//! The decisions are pure functions; only thin wrappers touch the registry or
//! the file system.

use std::io;

use crate::cli_export::Layout;

/// What [`publish`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Published {
    /// For the log.
    pub note: &'static str,
    /// The user environment changed; running programs should be told with
    /// [`announce_environment_change`], once everything else is done.
    pub environment_changed: bool,
}

/// Put the exported CLI on the user's PATH.
pub fn publish(layout: &Layout) -> io::Result<Published> {
    #[cfg(windows)]
    {
        let dir = dir_text(layout)?;
        let changed = registry::edit(&USER_PATH, |current| path_var::with_dir(current, &dir))?;
        Ok(Published {
            note: if changed {
                "added to user PATH"
            } else {
                "already on user PATH"
            },
            environment_changed: changed,
        })
    }
    #[cfg(unix)]
    {
        symlink::ensure(layout).map(|note| Published {
            note,
            environment_changed: false,
        })
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = layout;
        Ok(Published {
            note: "no PATH integration on this platform",
            environment_changed: false,
        })
    }
}

/// A registry value under `HKEY_CURRENT_USER` holding a `;`-separated path
/// list. Always [`USER_PATH`] outside tests.
#[cfg(windows)]
#[derive(Clone, Copy, Debug)]
pub struct PathValue<'a> {
    pub subkey: &'a str,
    pub name: &'a str,
}

/// The user's `Path`: `HKCU\Environment\Path`.
#[cfg(windows)]
pub const USER_PATH: PathValue<'static> = PathValue {
    subkey: "Environment",
    name: "Path",
};

/// Take the export directory out of `value` again (uninstall). Returns
/// whether the value changed.
#[cfg(windows)]
pub fn unpublish(layout: &Layout, value: &PathValue) -> io::Result<bool> {
    let dir = dir_text(layout)?;
    registry::edit(value, |current| path_var::without_dir(current, &dir))
}

/// Tell running programs (Explorer, new terminals) that the user environment
/// changed. Best effort, and the last step of whatever changed it: nothing
/// may depend on it or wait behind it.
pub fn announce_environment_change() {
    #[cfg(windows)]
    registry::broadcast();
}

#[cfg(windows)]
fn dir_text(layout: &Layout) -> io::Result<String> {
    // Never write a lossily converted directory into the user's PATH.
    layout.dir.to_str().map(str::to_owned).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "export directory is not Unicode",
        )
    })
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

    use super::PathValue;

    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }

    /// The text of a `REG_SZ` / `REG_EXPAND_SZ` value: UTF-16LE up to the
    /// first NUL (the registry guarantees neither the terminator nor that
    /// nothing follows it). Anything that is not exactly that is an error:
    /// a value we cannot read faithfully is a value we must not rewrite.
    pub(super) fn decode(bytes: &[u8]) -> io::Result<String> {
        if !bytes.len().is_multiple_of(2) {
            return Err(invalid("registry text has an odd byte length"));
        }
        let wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let end = wide
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(wide.len());
        String::from_utf16(&wide[..end]).map_err(|_| invalid("registry text is not valid UTF-16"))
    }

    pub(super) fn encode(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    /// `WM_SETTINGCHANGE` for "Environment" to every top-level window.
    pub(super) fn broadcast() {
        let scope: Vec<u16> = "Environment\0".encode_utf16().collect();
        let mut result = 0usize;
        // SAFETY: `scope` is a NUL-terminated UTF-16 string that outlives the
        // call, and `result` is a valid out pointer. Each window gets at most
        // the timeout; hung ones are skipped.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                scope.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                250,
                &mut result,
            );
        }
    }

    /// Apply `change` to the path list in `target`, keeping the value type
    /// (`REG_EXPAND_SZ` entries such as `%USERPROFILE%\bin` must stay
    /// expandable). `change` returns `None` when nothing has to be written.
    /// Returns whether the value was written. Does not broadcast.
    pub fn edit(
        target: &PathValue,
        change: impl FnOnce(&str) -> Option<String>,
    ) -> io::Result<bool> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(target.subkey, KEY_READ | KEY_WRITE)?;
        let (current, vtype) = match key.get_raw_value(target.name) {
            Ok(value) => {
                if !matches!(value.vtype, RegType::REG_SZ | RegType::REG_EXPAND_SZ) {
                    return Err(invalid("path value has an unexpected registry type"));
                }
                (decode(&value.bytes)?, value.vtype)
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
        key.set_raw_value(target.name, &value)?;
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
    mod registry {
        use std::io::ErrorKind;

        use winreg::enums::{HKEY_CURRENT_USER, RegType};
        use winreg::{RegKey, RegValue};

        use super::super::PathValue;
        use super::super::path_var::{with_dir, without_dir};
        use super::super::registry::{decode, edit, encode};
        use super::DIR;

        const NAME: &str = "Path";

        /// A key of its own under `HKCU\Software`, deleted when dropped (also
        /// when the test panics). `HKCU\Environment` is never opened here.
        struct Scratch {
            subkey: String,
        }

        impl Scratch {
            fn new(tag: &str) -> Self {
                let subkey = format!(r"Software\ActaTest-{}-{tag}", std::process::id());
                RegKey::predef(HKEY_CURRENT_USER)
                    .create_subkey(&subkey)
                    .unwrap();
                Self { subkey }
            }

            fn target(&self) -> PathValue<'_> {
                PathValue {
                    subkey: &self.subkey,
                    name: NAME,
                }
            }

            fn set(&self, bytes: Vec<u8>, vtype: RegType) {
                let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                    .create_subkey(&self.subkey)
                    .unwrap();
                key.set_raw_value(NAME, &RegValue { bytes, vtype }).unwrap();
            }

            fn get(&self) -> Option<RegValue> {
                RegKey::predef(HKEY_CURRENT_USER)
                    .open_subkey(&self.subkey)
                    .unwrap()
                    .get_raw_value(NAME)
                    .ok()
            }
        }

        impl Scratch {
            /// Last write time of the key, in 100 ns units.
            fn last_write(&self) -> u64 {
                let info = RegKey::predef(HKEY_CURRENT_USER)
                    .open_subkey(&self.subkey)
                    .unwrap()
                    .query_info()
                    .unwrap();
                let time = info.last_write_time;
                (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
            }
        }

        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.subkey);
            }
        }

        #[test]
        fn text_round_trips_and_stops_at_the_first_nul() {
            let value = format!("%USERPROFILE%\\bin;caf\u{e9};{DIR}");
            let bytes = encode(&value);
            assert_eq!(bytes.len(), (value.encode_utf16().count() + 1) * 2);
            assert_eq!(decode(&bytes).unwrap(), value);
            // Written without the terminator, or empty: the same text.
            assert_eq!(decode(&bytes[..bytes.len() - 2]).unwrap(), value);
            assert_eq!(decode(&[]).unwrap(), "");
            assert_eq!(decode(&encode("")).unwrap(), "");
            // Bytes after the terminator are not part of the string.
            let mut padded = encode("C:\\a");
            padded.extend(encode("junk"));
            assert_eq!(decode(&padded).unwrap(), "C:\\a");
        }

        #[test]
        fn malformed_text_is_an_error_not_a_guess() {
            let odd = &encode("C:\\a")[..5];
            assert_eq!(decode(odd).unwrap_err().kind(), ErrorKind::InvalidData);
            // A lone high surrogate.
            let lone = [0x41, 0x00, 0x00, 0xD8, 0x42, 0x00];
            assert_eq!(decode(&lone).unwrap_err().kind(), ErrorKind::InvalidData);
        }

        #[test]
        fn a_reg_sz_value_stays_reg_sz() {
            let scratch = Scratch::new("sz");
            scratch.set(encode(r"C:\tools"), RegType::REG_SZ);

            assert!(edit(&scratch.target(), |current| with_dir(current, DIR)).unwrap());

            let value = scratch.get().unwrap();
            assert_eq!(value.vtype, RegType::REG_SZ);
            assert_eq!(decode(&value.bytes).unwrap(), format!(r"C:\tools;{DIR}"));
        }

        #[test]
        fn an_expandable_value_stays_expandable_and_removal_restores_it() {
            let scratch = Scratch::new("expand");
            let original = r"%USERPROFILE%\bin;C:\tools";
            scratch.set(encode(original), RegType::REG_EXPAND_SZ);

            assert!(edit(&scratch.target(), |current| with_dir(current, DIR)).unwrap());
            assert_eq!(scratch.get().unwrap().vtype, RegType::REG_EXPAND_SZ);
            assert!(edit(&scratch.target(), |current| without_dir(current, DIR)).unwrap());

            let value = scratch.get().unwrap();
            assert_eq!(value.vtype, RegType::REG_EXPAND_SZ);
            assert_eq!(value.bytes, encode(original));
        }

        #[test]
        fn a_missing_value_is_created_expandable() {
            let scratch = Scratch::new("missing");
            assert!(scratch.get().is_none());

            assert!(edit(&scratch.target(), |current| with_dir(current, DIR)).unwrap());

            let value = scratch.get().unwrap();
            assert_eq!(value.vtype, RegType::REG_EXPAND_SZ);
            assert_eq!(decode(&value.bytes).unwrap(), DIR);
        }

        #[test]
        fn nothing_is_written_when_nothing_changes() {
            let scratch = Scratch::new("unchanged");
            scratch.set(encode(&format!(r"C:\tools;{DIR}")), RegType::REG_SZ);
            let written = scratch.last_write();
            std::thread::sleep(std::time::Duration::from_millis(30));

            assert!(!edit(&scratch.target(), |current| with_dir(current, DIR)).unwrap());
            let absent = r"C:\Users\Alexandre\elsewhere";
            assert!(!edit(&scratch.target(), |current| without_dir(current, absent)).unwrap());

            assert_eq!(scratch.last_write(), written);
        }

        #[test]
        fn a_value_of_another_type_is_refused_and_untouched() {
            let scratch = Scratch::new("type");
            let bytes = 7u32.to_le_bytes().to_vec();
            scratch.set(bytes.clone(), RegType::REG_DWORD);

            let err = edit(&scratch.target(), |current| with_dir(current, DIR)).unwrap_err();

            assert_eq!(err.kind(), ErrorKind::InvalidData);
            let value = scratch.get().unwrap();
            assert_eq!((value.vtype, value.bytes), (RegType::REG_DWORD, bytes));
        }

        #[test]
        fn a_malformed_value_is_refused_and_untouched() {
            // A lone high surrogate. (An odd byte length cannot be stored for
            // a string type: the registry rewrites its last byte. `decode`
            // refuses it all the same, see above.)
            let scratch = Scratch::new("surrogate");
            let bytes = vec![0x41, 0x00, 0x00, 0xD8, 0x42, 0x00];
            scratch.set(bytes.clone(), RegType::REG_EXPAND_SZ);

            let err = edit(&scratch.target(), |current| with_dir(current, DIR)).unwrap_err();

            assert_eq!(err.kind(), ErrorKind::InvalidData);
            // Reads come back NUL-terminated; the text itself is untouched.
            assert_eq!(scratch.get().unwrap().bytes[..bytes.len()], bytes);
        }

        #[test]
        fn a_missing_key_is_an_error() {
            let target = PathValue {
                subkey: r"Software\ActaTest-no-such-key",
                name: NAME,
            };
            assert!(edit(&target, |current| with_dir(current, DIR)).is_err());
        }
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
