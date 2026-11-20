//! Linux desktop integration for the AppImage (ADR 0004): at start, an
//! installed build writes its own `.desktop` file and icon under
//! `$XDG_DATA_HOME`, so the app shows up in the launcher. Idempotent: files
//! are only written when missing or different (the AppImage was moved).
//!
//! Compiled for Linux and for tests, so the logic is exercised on every host.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The 512 x 512 application icon.
const ICON: &[u8] = include_bytes!("../../../assets/icons/acta.png");
const ICON_SIZE_DIR: &str = "512x512";

/// Matches the window's `app_id` (shell.rs), which is how a compositor ties
/// a running window to this entry.
const ENTRY_NAME: &str = "acta.desktop";
const ICON_NAME: &str = "acta";

/// The `Exec` value that starts `appimage`, quoted as the Desktop Entry
/// specification requires. `None` for a path that cannot be expressed in a
/// desktop file (not UTF-8, or containing a line break).
pub fn exec_value(appimage: &Path) -> Option<String> {
    let path = appimage.to_str()?;
    if path.contains(['\n', '\r']) {
        return None;
    }
    let mut value = String::with_capacity(path.len() + 2);
    value.push('"');
    for c in path.chars() {
        match c {
            // Reserved inside a quoted argument: escaped with a backslash,
            // which the string-value level of the file format wants doubled.
            '"' | '`' | '$' => {
                value.push_str("\\\\");
                value.push(c);
            }
            '\\' => value.push_str("\\\\\\\\"),
            // A literal percent sign; a single one starts a field code.
            '%' => value.push_str("%%"),
            _ => value.push(c),
        }
    }
    value.push('"');
    Some(value)
}

/// The contents of the desktop file for `appimage`.
pub fn entry(appimage: &Path) -> Option<String> {
    let exec = exec_value(appimage)?;
    Some(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Comment=Project tracker and scoped memory for Claude Code\n\
         Exec={exec}\n\
         Icon={ICON_NAME}\n\
         Terminal=false\n\
         Categories=Development;ProjectManagement;\n\
         StartupWMClass={ICON_NAME}\n",
        name = acta_core::APP_NAME,
    ))
}

pub fn entry_path(data_home: &Path) -> PathBuf {
    data_home.join("applications").join(ENTRY_NAME)
}

pub fn icon_path(data_home: &Path) -> PathBuf {
    data_home
        .join("icons")
        .join("hicolor")
        .join(ICON_SIZE_DIR)
        .join("apps")
        .join(format!("{ICON_NAME}.png"))
}

/// Write `bytes` to `path` unless it already holds exactly them.
fn write_if_different(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    if fs::read(path).is_ok_and(|current| current == bytes) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, bytes)?;
    Ok(true)
}

/// Install or refresh the desktop entry and the icon under `data_home`
/// (`$XDG_DATA_HOME`). Returns whether anything was written.
pub fn install(data_home: &Path, appimage: &Path) -> io::Result<bool> {
    let entry = entry(appimage).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the AppImage path cannot be written to a desktop file",
        )
    })?;
    let icon_written = write_if_different(&icon_path(data_home), ICON)?;
    let entry_written = write_if_different(&entry_path(data_home), entry.as_bytes())?;
    Ok(icon_written || entry_written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_path_is_just_quoted() {
        let path = Path::new("/home/alexandre/.local/share/acta/Acta.AppImage");
        assert_eq!(
            exec_value(path).unwrap(),
            "\"/home/alexandre/.local/share/acta/Acta.AppImage\""
        );
    }

    #[test]
    fn reserved_characters_are_escaped() {
        // Spaces need the quotes only.
        assert_eq!(
            exec_value(Path::new("/opt/My Apps/Acta.AppImage")).unwrap(),
            "\"/opt/My Apps/Acta.AppImage\""
        );
        assert_eq!(
            exec_value(Path::new("/a/$HOME/b")).unwrap(),
            "\"/a/\\\\$HOME/b\""
        );
        assert_eq!(
            exec_value(Path::new("/a/say \"hi\"/`x`")).unwrap(),
            "\"/a/say \\\\\"hi\\\\\"/\\\\`x\\\\`\""
        );
        assert_eq!(
            exec_value(Path::new("/a/back\\slash")).unwrap(),
            "\"/a/back\\\\\\\\slash\""
        );
        assert_eq!(
            exec_value(Path::new("/a/100%/b")).unwrap(),
            "\"/a/100%%/b\""
        );
    }

    #[test]
    fn a_path_with_a_line_break_is_refused() {
        assert_eq!(exec_value(Path::new("/a/b\nExec=evil")), None);
        assert_eq!(entry(Path::new("/a/b\rc")), None);
    }

    #[test]
    fn the_entry_names_the_app_the_icon_and_the_window_class() {
        let entry = entry(Path::new("/opt/Acta.AppImage")).unwrap();
        let lines: Vec<&str> = entry.lines().collect();
        assert_eq!(lines[0], "[Desktop Entry]");
        for expected in [
            "Type=Application",
            "Name=Acta",
            "Exec=\"/opt/Acta.AppImage\"",
            "Icon=acta",
            "Terminal=false",
            "StartupWMClass=acta",
        ] {
            assert!(lines.contains(&expected), "missing {expected}");
        }
        assert!(entry.ends_with('\n'));
    }

    #[test]
    fn install_writes_both_files_once() {
        let tmp = tempfile::tempdir().unwrap();
        let data_home = tmp.path().join("share");
        let appimage = tmp.path().join("Acta.AppImage");

        assert!(install(&data_home, &appimage).unwrap());

        let entry_file = data_home.join("applications").join("acta.desktop");
        let icon_file = data_home
            .join("icons")
            .join("hicolor")
            .join("512x512")
            .join("apps")
            .join("acta.png");
        assert_eq!(
            fs::read_to_string(&entry_file).unwrap(),
            entry(&appimage).unwrap()
        );
        assert_eq!(fs::read(&icon_file).unwrap(), ICON);
        // PNG signature, and the size the directory name promises.
        assert_eq!(ICON[..8], [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(ICON[16..24], [0, 0, 2, 0, 0, 0, 2, 0]);

        // Nothing to do the second time.
        assert!(!install(&data_home, &appimage).unwrap());
    }

    #[test]
    fn a_moved_appimage_rewrites_the_entry_and_keeps_the_icon() {
        let tmp = tempfile::tempdir().unwrap();
        let data_home = tmp.path().join("share");
        install(&data_home, &tmp.path().join("old").join("Acta.AppImage")).unwrap();
        let moved = tmp.path().join("new").join("Acta.AppImage");

        assert!(install(&data_home, &moved).unwrap());

        assert_eq!(
            fs::read_to_string(entry_path(&data_home)).unwrap(),
            entry(&moved).unwrap()
        );
        assert!(!install(&data_home, &moved).unwrap());
    }

    #[test]
    fn other_entries_in_the_directory_are_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let data_home = tmp.path().join("share");
        let other = data_home.join("applications").join("other.desktop");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, b"[Desktop Entry]\n").unwrap();

        install(&data_home, &tmp.path().join("Acta.AppImage")).unwrap();

        assert_eq!(fs::read(&other).unwrap(), b"[Desktop Entry]\n");
    }
}
