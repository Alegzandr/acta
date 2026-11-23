//! Shared code of the updater spike: logging, CLI export, per-user PATH.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_FEED: &str = "http://127.0.0.1:8765/";

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Append a line to %TEMP%/acta-spike.log (survives uninstall, on purpose).
pub fn log(role: &str, msg: &str) {
    let path = std::env::temp_dir().join("acta-spike.log");
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            f,
            "{} [{} v{} pid {}] {}",
            now_ms(),
            role,
            VERSION,
            std::process::id(),
            msg
        );
    }
}

/// Stable per-user directory OUTSIDE the Velopack root. Velopack's Update.exe
/// kills every process whose image lives under its root directory when it
/// applies an update, so long-lived CLI/MCP processes must not run from there.
pub fn cli_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("ActaSpikeCli").join("bin")
}

pub fn cli_exe() -> PathBuf {
    cli_dir().join(if cfg!(windows) { "acta.exe" } else { "acta" })
}

/// Copy the packaged CLI next to the running GUI into `cli_dir()`.
/// A running `acta.exe` cannot be overwritten on Windows but it can be
/// renamed, so the old file is moved aside and deleted on a later run.
pub fn export_cli() -> std::io::Result<String> {
    let me = std::env::current_exe()?;
    let src = me
        .parent()
        .unwrap_or(Path::new("."))
        .join(if cfg!(windows) { "acta-cli.exe" } else { "acta-cli" });
    let dir = cli_dir();
    fs::create_dir_all(&dir)?;
    let dst = cli_exe();
    let marker = dir.join("version.txt");

    // Best-effort cleanup of files left by previous exports.
    let mut left = 0;
    for e in fs::read_dir(&dir)?.flatten() {
        if e.file_name().to_string_lossy().contains(".old-") && fs::remove_file(e.path()).is_err() {
            left += 1; // still running: an old MCP server keeps its own version
        }
    }

    if dst.exists() && fs::read_to_string(&marker).ok().as_deref() == Some(VERSION) {
        return Ok(format!("cli already at {VERSION}, {left} old file(s) still locked"));
    }
    if dst.exists() {
        let aside = dir.join(format!("acta.exe.old-{}", now_ms()));
        fs::rename(&dst, &aside)?;
    }
    let tmp = dir.join(format!("acta.new-{}", now_ms()));
    fs::copy(&src, &tmp)?;
    fs::rename(&tmp, &dst)?;
    fs::write(&marker, VERSION)?;
    Ok(format!("cli exported at {VERSION}, {left} old file(s) still locked"))
}

/// Remove the exported CLI directory (uninstall hook).
pub fn remove_cli() -> std::io::Result<()> {
    let dir = cli_dir();
    let root = dir.parent().map(Path::to_path_buf).unwrap_or(dir);
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[cfg(windows)]
mod path_win {
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    extern "system" {
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: *const u16,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }

    fn broadcast() {
        let env: Vec<u16> = "Environment\0".encode_utf16().collect();
        let mut res = 0usize;
        // HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG
        unsafe { SendMessageTimeoutW(0xffff, 0x001A, 0, env.as_ptr(), 0x0002, 2000, &mut res) };
    }

    fn decode(v: &RegValue) -> String {
        let w: Vec<u16> = v
            .bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&w).trim_end_matches('\0').to_string()
    }

    fn encode(s: &str, vtype: RegType) -> RegValue<'static> {
        let bytes: Vec<u8> = s
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(|u| u.to_le_bytes())
            .collect();
        RegValue {
            bytes: bytes.into(),
            vtype,
        }
    }

    /// Add or remove `dir` in HKCU\Environment\Path, keeping the value type.
    pub fn set(dir: &str, present: bool) -> std::io::Result<bool> {
        let env = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
        let (cur, vtype) = match env.get_raw_value("Path") {
            Ok(v) => (decode(&v), v.vtype),
            Err(_) => (String::new(), RegType::REG_EXPAND_SZ),
        };
        let has = cur.split(';').any(|p| p.eq_ignore_ascii_case(dir));
        if has == present {
            return Ok(false);
        }
        let new = if present {
            if cur.is_empty() || cur.ends_with(';') {
                format!("{cur}{dir}")
            } else {
                format!("{cur};{dir}")
            }
        } else {
            let trailing = cur.ends_with(';');
            let mut s = cur
                .split(';')
                .filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case(dir))
                .collect::<Vec<_>>()
                .join(";");
            if trailing {
                s.push(';');
            }
            s
        };
        env.set_raw_value("Path", &encode(&new, vtype))?;
        broadcast();
        Ok(true)
    }
}

/// Put (or remove) the CLI directory on the per-user PATH. No admin needed.
pub fn set_on_path(present: bool) -> std::io::Result<bool> {
    #[cfg(windows)]
    {
        path_win::set(&cli_dir().to_string_lossy(), present)
    }
    #[cfg(not(windows))]
    {
        let _ = present; // Unix: symlink into ~/.local/bin, not part of this spike
        Ok(false)
    }
}
