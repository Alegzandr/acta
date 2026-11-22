//! Where a tool is (ADR 0013, section 2, "Where to look"): the process
//! `PATH`; on Windows, the `PATH` as it is now in the registry, since a
//! running Acta keeps the `PATH` it started with and an install changes the
//! registry only; then known per-user and system folders. One resolver for
//! the checklist, `acta setup --check`, the terminal and the status line.
//!
//! Every search takes its directories and an `is_file` closure, so the
//! tests run on fake trees on any host.

use crate::paths::Os;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The directories everything else is derived from, read once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bases {
    /// `USERPROFILE` on Windows, `HOME` elsewhere.
    pub home: Option<PathBuf>,
    /// `%LOCALAPPDATA%` (Windows).
    pub local_app_data: Option<PathBuf>,
    /// `%ProgramFiles%` and `%ProgramW6432%` (Windows).
    pub program_files: Vec<PathBuf>,
}

impl Bases {
    pub fn from_env() -> Bases {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let mut program_files: Vec<PathBuf> = ["ProgramFiles", "ProgramW6432"]
            .iter()
            .filter_map(|n| var(n))
            .collect();
        program_files.dedup();
        Bases {
            home: var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }),
            local_app_data: var("LOCALAPPDATA"),
            program_files,
        }
    }
}

/// The known folders, searched after the `PATH`s (ADR 0013, section 2).
pub fn known_folders(os: Os, bases: &Bases) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = &bases.home {
        dirs.push(home.join(".local").join("bin"));
    }
    match os {
        Os::Windows => {
            if let Some(lad) = &bases.local_app_data {
                dirs.push(lad.join("Microsoft").join("WinGet").join("Links"));
                dirs.push(lad.join("Programs").join("Git").join("cmd"));
            }
            for pf in &bases.program_files {
                dirs.push(pf.join("Git").join("cmd"));
                dirs.push(pf.join("GitHub CLI"));
            }
        }
        Os::MacOs => {
            for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
                dirs.push(PathBuf::from(d));
            }
        }
        Os::Linux => {
            for d in ["/usr/local/bin", "/usr/bin"] {
                dirs.push(PathBuf::from(d));
            }
        }
    }
    dirs
}

/// The entries of a `PATH` value, empty ones dropped.
pub fn split_path(path: &OsStr) -> Vec<PathBuf> {
    std::env::split_paths(path)
        .filter(|p| !p.as_os_str().is_empty())
        .collect()
}

fn same_dir(os: Os, a: &Path, b: &Path) -> bool {
    if os == Os::Windows {
        let norm = |p: &Path| {
            p.to_string_lossy()
                .trim_end_matches(['\\', '/'])
                .to_lowercase()
        };
        norm(a) == norm(b)
    } else {
        a == b
    }
}

/// The search order: the process `PATH`, the registry `PATH`, the known
/// folders; each directory once.
pub fn search_dirs(
    os: Os,
    process_path: Option<&OsStr>,
    registry_path: Option<&OsStr>,
    known: &[PathBuf],
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let all = process_path
        .map(split_path)
        .unwrap_or_default()
        .into_iter()
        .chain(registry_path.map(split_path).unwrap_or_default())
        .chain(known.iter().cloned());
    for dir in all {
        if !out.iter().any(|d| same_dir(os, d, &dir)) {
            out.push(dir);
        }
    }
    out
}

/// The `PATH` handed to every child Acta starts: the process `PATH`, then
/// what the registry has that it lacks (an install made since Acta
/// started). `None` when there is nothing to add to the process's own.
pub fn fresh_path(
    os: Os,
    process_path: Option<&OsStr>,
    registry_path: Option<&OsStr>,
) -> Option<OsString> {
    let registry = registry_path?;
    let mut dirs = process_path.map(split_path).unwrap_or_default();
    let before = dirs.len();
    for dir in split_path(registry) {
        if !dirs.iter().any(|d| same_dir(os, d, &dir)) {
            dirs.push(dir);
        }
    }
    if dirs.len() == before {
        return None;
    }
    std::env::join_paths(dirs).ok()
}

/// Windows `%NAME%` references expanded with `lookup`; an unknown name is
/// left as written, as Windows does.
pub fn expand_env(text: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match lookup(name) {
                    Some(value) => out.push_str(&value),
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Which `Environment` key of the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hive {
    /// `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`.
    Machine,
    /// `HKCU\Environment`.
    User,
}

/// A string value as stored: `REG_EXPAND_SZ` (`expand`) or `REG_SZ`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegValue {
    pub expand: bool,
    pub text: String,
}

/// The registry `PATH`, machine then user as Windows builds it, with
/// `REG_EXPAND_SZ` values expanded: from the process environment, else
/// from the user's then the machine's `Environment` values (a variable set
/// by an install since Acta started).
pub fn registry_path_with(
    read: impl Fn(Hive, &str) -> Option<RegValue>,
    process_env: impl Fn(&str) -> Option<String>,
) -> Option<OsString> {
    let lookup = |name: &str| {
        process_env(name).or_else(|| {
            [Hive::User, Hive::Machine]
                .into_iter()
                .find_map(|h| read(h, name).filter(|v| !v.expand).map(|v| v.text))
        })
    };
    let parts: Vec<String> = [Hive::Machine, Hive::User]
        .into_iter()
        .filter_map(|hive| read(hive, "Path"))
        .map(|v| {
            if v.expand {
                expand_env(&v.text, lookup)
            } else {
                v.text
            }
        })
        .filter(|t| !t.is_empty())
        .collect();
    (!parts.is_empty()).then(|| OsString::from(parts.join(";")))
}

/// The registry `PATH` of this machine; `None` off Windows.
pub fn registry_path() -> Option<OsString> {
    #[cfg(windows)]
    {
        registry_path_with(registry::read, |name| std::env::var(name).ok())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
mod registry {
    use super::{Hive, RegValue};
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_EXPAND_SZ, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ,
        RRF_RT_REG_SZ, RegGetValueW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// One string value of an `Environment` key, unexpanded.
    pub fn read(hive: Hive, name: &str) -> Option<RegValue> {
        let (root, key) = match hive {
            Hive::Machine => (
                HKEY_LOCAL_MACHINE,
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            ),
            Hive::User => (HKEY_CURRENT_USER, "Environment"),
        };
        let key = wide(key);
        let name = wide(name);
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
        let mut kind = 0u32;
        let mut size = 0u32;
        // SAFETY: a size query (null buffer), then a read into a buffer of
        // that size; both strings are NUL-terminated and outlive the calls.
        unsafe {
            if RegGetValueW(
                root,
                key.as_ptr(),
                name.as_ptr(),
                flags,
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            ) != ERROR_SUCCESS
            {
                return None;
            }
            let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
            let mut size = (buf.len() * 2) as u32;
            if RegGetValueW(
                root,
                key.as_ptr(),
                name.as_ptr(),
                flags,
                &mut kind,
                buf.as_mut_ptr().cast(),
                &mut size,
            ) != ERROR_SUCCESS
            {
                return None;
            }
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(RegValue {
                expand: kind == REG_EXPAND_SZ,
                text: String::from_utf16_lossy(&buf[..len]),
            })
        }
    }
}

/// The first `name` in `dirs`. On Windows a `.exe` anywhere wins over a
/// `.cmd` shim (an npm install puts one next to a native install's older
/// copy, or alone).
pub fn locate(
    os: Os,
    name: &str,
    dirs: &[PathBuf],
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if os == Os::Windows {
        ["exe", "cmd"].iter().find_map(|ext| {
            dirs.iter()
                .map(|d| d.join(format!("{name}.{ext}")))
                .find(|p| is_file(p))
        })
    } else {
        dirs.iter().map(|d| d.join(name)).find(|p| is_file(p))
    }
}

/// `is_file` for real files. On Windows an App Execution Alias (`winget`
/// in `WindowsApps`) is a reparse point `metadata` cannot follow: it counts.
pub fn is_file(path: &Path) -> bool {
    path.is_file() || (cfg!(windows) && std::fs::symlink_metadata(path).is_ok_and(|m| !m.is_dir()))
}

/// The search directories of this process, for this OS.
pub fn system_dirs() -> Vec<PathBuf> {
    let process = std::env::var_os("PATH");
    let registry = registry_path();
    let known = known_folders(Os::CURRENT, &Bases::from_env());
    search_dirs(Os::CURRENT, process.as_deref(), registry.as_deref(), &known)
}

/// The fresh `PATH` of this process (see [`fresh_path`]).
pub fn system_fresh_path() -> Option<OsString> {
    fresh_path(
        Os::CURRENT,
        std::env::var_os("PATH").as_deref(),
        registry_path().as_deref(),
    )
}

/// How `claude` is reached on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaudeProgram {
    /// A native executable, spawned directly.
    Exe(PathBuf),
    /// A Windows `.cmd` shim, run through `%ComSpec% /d /s /c`.
    CmdShim { comspec: String, shim: PathBuf },
    /// A Unix login shell that resolves `claude` from the user's own `PATH`.
    LoginShell(String),
}

/// Find `claude` for the terminal (ADR 0002 point 3, ADR 0013): on Windows
/// through the shared search; elsewhere through a POSIX login shell, which
/// reads the user's own `PATH` from their shell files.
pub fn resolve_claude() -> Option<ClaudeProgram> {
    #[cfg(windows)]
    {
        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned());
        resolve_windows(&system_dirs(), comspec, is_file)
    }
    #[cfg(not(windows))]
    {
        let shell = std::env::var_os("SHELL");
        let home = std::env::var_os("HOME").map(PathBuf::from);
        resolve_unix(shell, home.as_deref(), |path| path.is_file())
    }
}

/// `claude.exe` anywhere on the search path wins over a `claude.cmd` shim.
pub fn resolve_windows(
    dirs: &[PathBuf],
    comspec: String,
    is_file: impl Fn(&Path) -> bool,
) -> Option<ClaudeProgram> {
    let found = locate(Os::Windows, "claude", dirs, is_file)?;
    let shim = found
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd"));
    Some(if shim {
        ClaudeProgram::CmdShim {
            comspec,
            shim: found,
        }
    } else {
        ClaudeProgram::Exe(found)
    })
}

/// Shells known to take `-l -i -c script $0 args...`; any other `$SHELL`
/// (fish, nu) falls back to the well-known install locations.
const POSIX_SHELLS: [&str; 5] = ["bash", "zsh", "sh", "dash", "ksh"];

pub fn resolve_unix(
    shell: Option<OsString>,
    home: Option<&Path>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<ClaudeProgram> {
    if let Some(shell) = shell {
        let path = PathBuf::from(&shell);
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if POSIX_SHELLS.contains(&name) && is_file(&path) {
            return Some(ClaudeProgram::LoginShell(
                shell.to_string_lossy().into_owned(),
            ));
        }
    }
    let mut candidates = Vec::new();
    if let Some(home) = home {
        candidates.push(home.join(".local/bin/claude"));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/claude"));
    candidates.push(PathBuf::from("/usr/local/bin/claude"));
    candidates
        .into_iter()
        .find(|p| is_file(p))
        .map(ClaudeProgram::Exe)
}

/// Git Bash, in the order Claude Code looks (ADR 0013, section 2):
/// `CLAUDE_CODE_GIT_BASH_PATH`, next to a `git.exe` of the search
/// (`<Git>\cmd\git.exe` or `<Git>\bin\git.exe` beside `<Git>\bin\bash.exe`),
/// then the usual install folders.
pub fn git_bash_in(
    env_path: Option<&Path>,
    dirs: &[PathBuf],
    bases: &Bases,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(p) = env_path.filter(|p| is_file(p)) {
        return Some(p.to_path_buf());
    }
    let beside_git = dirs
        .iter()
        .filter(|dir| is_file(&dir.join("git.exe")))
        .filter_map(|dir| dir.parent().map(|git| git.join("bin").join("bash.exe")));
    let usual = bases
        .program_files
        .iter()
        .map(|pf| pf.join("Git").join("bin").join("bash.exe"))
        .chain(bases.local_app_data.iter().map(|lad| {
            lad.join("Programs")
                .join("Git")
                .join("bin")
                .join("bash.exe")
        }));
    beside_git.chain(usual).find(|p| is_file(p))
}

/// Git Bash on this machine; always `None` off Windows.
pub fn git_bash() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let env = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from);
    git_bash_in(env.as_deref(), &system_dirs(), &Bases::from_env(), is_file)
}

/// The variables of a Claude Code session among `names` (an environment's
/// variable names): what a Claude Code sets for the programs it starts.
/// An Acta started from a Claude Code shell inherits them, and a `claude`
/// started by Acta would then take itself for a child of that session (no
/// transcript, a nested session). Only when the environment is such a
/// session's (`CLAUDECODE` set), since a user may set `CLAUDE_CODE_*`
/// options in their own environment.
pub fn claude_session_vars<S: AsRef<str>>(names: &[S]) -> Vec<String> {
    let is = |name: &str, want: &str| name.eq_ignore_ascii_case(want);
    let in_session = names.iter().any(|n| is(n.as_ref(), "CLAUDECODE"));
    if !in_session {
        return Vec::new();
    }
    names
        .iter()
        .map(AsRef::as_ref)
        .filter(|name| {
            let upper = name.to_ascii_uppercase();
            upper == "CLAUDECODE" || upper == "CLAUDE_PID" || upper.starts_with("CLAUDE_CODE_")
        })
        .map(str::to_owned)
        .collect()
}

/// [`claude_session_vars`] of this process's environment.
pub fn session_vars_here() -> Vec<String> {
    let names: Vec<String> = std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .collect();
    claude_session_vars(&names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn join(dirs: &[&Path]) -> OsString {
        std::env::join_paths(dirs).unwrap()
    }

    /// A stub executable named for `os` in `dir`.
    fn stub(dir: &Path, file: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let p = dir.join(file);
        fs::write(&p, "Alexandre's stub").unwrap();
        p
    }

    #[test]
    fn found_on_the_process_path() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        let want = stub(&b, "git.exe");
        stub(&a, "other.exe");
        let path = join(&[&a, &b]);
        let dirs = search_dirs(Os::Windows, Some(&path), None, &[]);
        assert_eq!(
            locate(Os::Windows, "git", &dirs, |p| p.is_file()),
            Some(want)
        );
        let unix = stub(&b, "git");
        assert_eq!(locate(Os::Linux, "git", &dirs, |p| p.is_file()), Some(unix));
    }

    #[test]
    fn found_only_in_the_registry_path_expanded() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("Alexandre");
        let want = stub(&home.join("tools"), "gh.exe");
        let process = join(&[&tmp.path().join("empty")]);
        let home_s = home.to_string_lossy().into_owned();
        // The reader is injected: the user's Path is REG_EXPAND_SZ with a
        // variable the process does not have (set by the install).
        let read = |hive: Hive, name: &str| match (hive, name) {
            (Hive::User, "Path") => Some(RegValue {
                expand: true,
                text: "%ALEX_HOME%\\tools;%UNKNOWN%\\x".into(),
            }),
            (Hive::User, "ALEX_HOME") => Some(RegValue {
                expand: false,
                text: home_s.clone(),
            }),
            (Hive::Machine, "Path") => Some(RegValue {
                expand: false,
                text: String::new(),
            }),
            _ => None,
        };
        let registry = registry_path_with(read, |_| None).unwrap();
        assert!(registry.to_string_lossy().starts_with(&home_s));
        assert!(registry.to_string_lossy().contains("%UNKNOWN%"));
        if cfg!(windows) {
            let dirs = search_dirs(Os::Windows, Some(&process), Some(&registry), &[]);
            assert_eq!(
                locate(Os::Windows, "gh", &dirs, |p| p.is_file()),
                Some(want)
            );
            let without = search_dirs(Os::Windows, Some(&process), None, &[]);
            assert_eq!(locate(Os::Windows, "gh", &without, |p| p.is_file()), None);
            // The fresh PATH for children gains the registry's new folder.
            let fresh = fresh_path(Os::Windows, Some(&process), Some(&registry)).unwrap();
            assert!(split_path(&fresh).contains(&home.join("tools")));
            assert_eq!(
                fresh_path(Os::Windows, Some(&process), Some(&process)),
                None
            );
        }
    }

    #[test]
    fn expansion_follows_windows() {
        let env = |n: &str| (n == "USERPROFILE").then(|| r"C:\Users\Alexandre".to_owned());
        assert_eq!(
            expand_env(r"%USERPROFILE%\.local\bin;%NOPE%;100%;%%", env),
            r"C:\Users\Alexandre\.local\bin;%NOPE%;100%;%%"
        );
    }

    #[test]
    fn found_only_in_a_known_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let bases = Bases {
            home: Some(tmp.path().join("home")),
            local_app_data: Some(tmp.path().join("lad")),
            program_files: vec![tmp.path().join("pf")],
        };
        let known = known_folders(Os::Windows, &bases);
        let claude = stub(&tmp.path().join("home/.local/bin"), "claude.exe");
        let git = stub(&tmp.path().join("lad/Programs/Git/cmd"), "git.exe");
        let gh = stub(&tmp.path().join("pf/GitHub CLI"), "gh.exe");
        let process = join(&[&tmp.path().join("empty")]);
        let dirs = search_dirs(Os::Windows, Some(&process), None, &known);
        let find = |n| locate(Os::Windows, n, &dirs, |p: &Path| p.is_file());
        assert_eq!(find("claude"), Some(claude));
        assert_eq!(find("git"), Some(git));
        assert_eq!(find("gh"), Some(gh));
        assert_eq!(find("winget"), None);
        let mac = known_folders(Os::MacOs, &Bases::default());
        assert!(mac.contains(&PathBuf::from("/opt/homebrew/bin")));
    }

    #[test]
    fn exe_wins_over_cmd_and_missing_is_none() {
        let a = PathBuf::from("a");
        let b = PathBuf::from("b");
        let dirs = [a.clone(), b.clone()];
        let a_cmd = a.join("claude.cmd");
        let b_exe = b.join("claude.exe");
        assert_eq!(
            locate(Os::Windows, "claude", &dirs, |p| p == a_cmd || p == b_exe),
            Some(b_exe.clone())
        );
        assert_eq!(
            locate(Os::Windows, "claude", &dirs, |p| p == a_cmd),
            Some(a_cmd.clone())
        );
        assert_eq!(locate(Os::Windows, "claude", &dirs, |_| false), None);
        assert_eq!(locate(Os::Linux, "claude", &dirs, |_| false), None);
    }

    #[test]
    fn windows_prefers_exe_anywhere_over_cmd() {
        let dirs = [PathBuf::from("a"), PathBuf::from("b")];
        let a_cmd = PathBuf::from("a").join("claude.cmd");
        let b_exe = PathBuf::from("b").join("claude.exe");
        let found = resolve_windows(&dirs, "cmd.exe".into(), |p| p == a_cmd || p == b_exe);
        assert_eq!(found, Some(ClaudeProgram::Exe(b_exe)));
        let found = resolve_windows(&dirs, "cmd.exe".into(), |p| p == a_cmd);
        assert_eq!(
            found,
            Some(ClaudeProgram::CmdShim {
                comspec: "cmd.exe".into(),
                shim: a_cmd
            })
        );
        assert_eq!(resolve_windows(&dirs, "cmd.exe".into(), |_| false), None);
    }

    #[test]
    fn unix_uses_posix_login_shell_else_known_paths() {
        let home = PathBuf::from("/home/alexandre");
        let found = resolve_unix(Some("/bin/zsh".into()), Some(&home), |_| true);
        assert_eq!(found, Some(ClaudeProgram::LoginShell("/bin/zsh".into())));
        let found = resolve_unix(Some("/usr/bin/fish".into()), Some(&home), |p| {
            p == home.join(".local/bin/claude")
        });
        assert_eq!(
            found,
            Some(ClaudeProgram::Exe(home.join(".local/bin/claude")))
        );
        let found = resolve_unix(None, None, |p| p == Path::new("/usr/local/bin/claude"));
        assert_eq!(
            found,
            Some(ClaudeProgram::Exe(PathBuf::from("/usr/local/bin/claude")))
        );
    }

    #[test]
    fn git_bash_follows_claude_codes_order() {
        let git = PathBuf::from("G");
        let dirs = [PathBuf::from("x"), git.join("cmd")];
        let bases = Bases {
            home: None,
            local_app_data: Some(PathBuf::from("L")),
            program_files: vec![PathBuf::from("P")],
        };
        let beside = git.join("bin").join("bash.exe");
        let usual = PathBuf::from("P").join("Git").join("bin").join("bash.exe");
        let custom = PathBuf::from("C").join("bash.exe");
        let git_exe = git.join("cmd").join("git.exe");
        let files = [
            beside.clone(),
            usual.clone(),
            custom.clone(),
            git_exe.clone(),
        ];
        let has = |p: &Path| files.iter().any(|f| f == p);
        assert_eq!(git_bash_in(Some(&custom), &dirs, &bases, has), Some(custom));
        assert_eq!(git_bash_in(None, &dirs, &bases, has), Some(beside.clone()));
        let no_beside = |p: &Path| p != beside && has(p);
        assert_eq!(git_bash_in(None, &dirs, &bases, no_beside), Some(usual));
        assert_eq!(git_bash_in(None, &dirs, &bases, |_| false), None);
    }

    #[test]
    fn a_claude_code_session_is_not_passed_on() {
        let names = [
            "PATH",
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_PID",
            "ANTHROPIC_MODEL",
        ];
        assert_eq!(
            claude_session_vars(&names),
            ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_PID"]
        );
        assert!(claude_session_vars(&["PATH", "CLAUDE_CODE_USE_BEDROCK"]).is_empty());
    }
}
