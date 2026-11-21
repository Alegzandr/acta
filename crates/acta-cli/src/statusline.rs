//! `acta statusline`: Claude Code's status line command (ADR 0005 amendment,
//! 2026-11-21). Claude Code runs it with the session's JSON on stdin
//! (code.claude.com/docs/en/statusline); it keeps the plan's rate-limit
//! numbers for the app (acta_core::usage) and prints a short line for
//! Claude Code: the focus when the session runs in Acta's terminal, and
//! "5h 42% · 7d 18%".
//!
//! When the user had a status line of their own, `acta setup` kept it in its
//! record and Acta runs it first, with the same stdin, then appends its own
//! segment: the user's line is never lost. Fast path only: no database, no
//! keychain.

use acta_core::usage;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long the user's own status line may take (Claude Code cancels a run
/// that a newer update overtakes anyway).
const ORIGINAL_LIMIT: Duration = Duration::from_secs(5);

/// Acta's segment: "T-12 · 5h 42% · 7d 18%".
pub fn segment(focus: Option<&str>, u: Option<&usage::Usage>, now_ms: i64) -> String {
    let mut parts = Vec::new();
    if let Some(f) = focus {
        parts.push(f.to_owned());
    }
    if let Some(s) = u.and_then(|u| usage::status_segment(u, now_ms)) {
        parts.push(s);
    }
    parts.join(" \u{b7} ")
}

/// The user's line, then Acta's on its first line, two spaces apart.
pub fn combine(original: Option<&str>, ours: &str) -> String {
    let original = original.map(str::trim_end).filter(|o| !o.is_empty());
    match (original, ours.is_empty()) {
        (None, _) => ours.to_owned(),
        (Some(o), true) => o.to_owned(),
        (Some(o), false) => match o.split_once('\n') {
            Some((first, rest)) => format!("{first}  {ours}\n{rest}"),
            None => format!("{o}  {ours}"),
        },
    }
}

/// The status line Claude Code shows: records the usage, runs the user's
/// own command if setup kept one, and returns the text to print.
pub fn run(stdin: &str, data_dir: Option<&Path>, now_ms: i64) -> String {
    let nested = std::env::var_os(NESTED).is_some_and(|v| v == "1");
    run_with(stdin, data_dir, now_ms, nested)
}

/// Set for the user's own status line command: an `acta statusline` it
/// calls (directly or through a script) never runs that command again.
pub const NESTED: &str = "ACTA_STATUSLINE_NESTED";

/// [`run`], with whether this is a nested call (then the user's command is
/// not run: it is the one running us).
pub fn run_with(stdin: &str, data_dir: Option<&Path>, now_ms: i64, nested: bool) -> String {
    let seen = usage::parse_status_input(stdin);
    let current = data_dir.and_then(|dir| match seen {
        Some(seen) => usage::record(dir, seen, now_ms)
            .ok()
            .or_else(|| usage::read(dir)),
        None => usage::read(dir),
    });
    let focus = ["ACTA_TOPIC", "ACTA_EPIC"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| is_id(v)));
    let ours = segment(focus.as_deref(), current.as_ref(), now_ms);
    let original = data_dir
        .filter(|_| !nested)
        .and_then(original_command)
        .and_then(|cmd| run_original(&shell_for(&cmd), stdin, ORIGINAL_LIMIT));
    combine(original.as_deref(), &ours)
}

/// `T-12`, `E-3`: nothing else from the environment reaches the line.
fn is_id(v: &str) -> bool {
    v.len() <= 12
        && v.split_once('-').is_some_and(|(p, n)| {
            matches!(p, "T" | "E") && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
        })
}

/// The user's own status line command, as setup recorded it.
pub fn original_command(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(data_dir.join("claude").join("setup-record.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let cmd = v.get("statusline_original")?.get("command")?.as_str()?;
    // An Acta status line is never "the user's own": running it would
    // call Acta again.
    (!cmd.trim().is_empty() && !is_acta_statusline(cmd)).then(|| cmd.to_owned())
}

/// Whether a status line command is Acta's: an `acta` (or `acta-cli`)
/// executable, at any path, run with `statusline`.
pub fn is_acta_statusline(command: &str) -> bool {
    let command = command.trim();
    let (program, rest) = match command.strip_prefix('"') {
        Some(quoted) => match quoted.split_once('"') {
            Some((p, rest)) => (p, rest),
            None => return false,
        },
        None => command
            .split_once(char::is_whitespace)
            .unwrap_or((command, "")),
    };
    let name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    matches!(stem, "acta" | "acta-cli") && rest.split_whitespace().any(|w| w == "statusline")
}

/// The shell Claude Code would use for a status line command: `sh` on
/// macOS and Linux; on Windows Git Bash when it is installed, else
/// PowerShell (code.claude.com/docs/en/statusline, "Windows configuration").
pub fn shell_for(command: &str) -> Command {
    #[cfg(windows)]
    {
        let bash = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(git_bash_on_path)
            .or_else(|| {
                ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"]
                    .iter()
                    .filter_map(std::env::var_os)
                    .flat_map(|base| {
                        let base = std::path::PathBuf::from(base);
                        [
                            base.join("Git").join("bin").join("bash.exe"),
                            base.join("Programs")
                                .join("Git")
                                .join("bin")
                                .join("bash.exe"),
                        ]
                    })
                    .find(|p| p.is_file())
            });
        match bash {
            Some(bash) => {
                let mut c = Command::new(bash);
                c.args(["-c", command]);
                c
            }
            None => {
                let mut c = Command::new("powershell.exe");
                c.args(["-NoProfile", "-NonInteractive", "-Command", command]);
                c
            }
        }
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("/bin/sh");
        c.args(["-c", command]);
        c
    }
}

/// Git Bash from a `git.exe` on `PATH`: `<Git>/cmd/git.exe` or
/// `<Git>/bin/git.exe`, next to `<Git>/bin/bash.exe`.
#[cfg(windows)]
fn git_bash_on_path() -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .filter(|dir| dir.join("git.exe").is_file())
        .filter_map(|dir| dir.parent().map(|git| git.join("bin").join("bash.exe")))
        .find(|bash| bash.is_file())
}

/// Runs `command` with `input` on stdin; its stdout whatever the exit
/// status (a failing command may still have printed its line), `None` when
/// it could not start, ran past `limit`, or left its output open past it.
/// On a timeout the whole process tree is killed (a job object on Windows,
/// the process group elsewhere), so nothing it started lingers.
pub fn run_original(command: &Command, input: &str, limit: Duration) -> Option<String> {
    let mut cmd = Command::new(command.get_program());
    cmd.args(command.get_args())
        .env(NESTED, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // CREATE_NO_WINDOW | CREATE_SUSPENDED: the child runs only once it
        // is in its job (`tree::Tree::of`), so nothing it starts escapes.
        cmd.creation_flags(0x0800_0000 | 0x0000_0004);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().ok()?;
    let tree = tree::Tree::of(&child);
    let mut stdin = child.stdin.take()?;
    let input = input.to_owned();
    std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        let _ = tx.send(out);
    });
    let deadline = Instant::now() + limit;
    let killed = |child: &mut std::process::Child| {
        tree.kill(child);
        let _ = child.wait();
        None
    };
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => return killed(&mut child),
        }
    }
    // A process it started may still hold the output open: wait for the
    // end of output only until the deadline.
    let left = deadline.saturating_duration_since(Instant::now());
    match rx.recv_timeout(left.max(Duration::from_millis(50))) {
        Ok(out) => Some(String::from_utf8_lossy(&out).into_owned()),
        Err(_) => killed(&mut child),
    }
}

/// The process tree of a status line command.
mod tree {
    use std::process::Child;

    #[cfg(windows)]
    pub struct Tree(Option<windows_sys::Win32::Foundation::HANDLE>);

    #[cfg(windows)]
    impl Tree {
        /// A job object holding the child (and everything it starts),
        /// closed with the job (`KILL_ON_JOB_CLOSE`).
        pub fn of(child: &Child) -> Tree {
            use std::os::windows::io::AsRawHandle as _;
            use windows_sys::Win32::System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            };
            // SAFETY: plain Win32 calls; the job handle is owned by `Tree`
            // and closed in `Drop` (or here when it could not take the
            // child); the info struct lives on this frame.
            let tree = unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    Tree(None)
                } else {
                    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                    let limited = SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        &info as *const _ as *const core::ffi::c_void,
                        std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    ) != 0;
                    if limited && AssignProcessToJobObject(job, child.as_raw_handle() as _) != 0 {
                        Tree(Some(job))
                    } else {
                        // Not in a job: `kill` falls back to the child alone.
                        windows_sys::Win32::Foundation::CloseHandle(job);
                        Tree(None)
                    }
                }
            };
            resume(child.id());
            tree
        }

        pub fn kill(&self, child: &mut Child) {
            use windows_sys::Win32::System::JobObjects::TerminateJobObject;
            match self.0 {
                // SAFETY: `job` is the live handle created in `of`.
                Some(job) => unsafe {
                    TerminateJobObject(job, 1);
                },
                None => {
                    let _ = child.kill();
                }
            }
        }
    }

    /// Resumes the threads of a process started suspended (std has no
    /// handle to its first thread; there is one at this point).
    #[cfg(windows)]
    fn resume(pid: u32) {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        };
        use windows_sys::Win32::System::Threading::{
            OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
        };
        // SAFETY: a snapshot walked with a correctly sized entry; every
        // handle opened here is closed here.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snap == INVALID_HANDLE_VALUE {
                return;
            }
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            let mut more = Thread32First(snap, &mut entry) != 0;
            while more {
                if entry.th32OwnerProcessID == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                    if !thread.is_null() {
                        ResumeThread(thread);
                        CloseHandle(thread);
                    }
                }
                more = Thread32Next(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
        }
    }

    #[cfg(windows)]
    impl Drop for Tree {
        fn drop(&mut self) {
            if let Some(job) = self.0 {
                // SAFETY: closed once, here.
                unsafe {
                    windows_sys::Win32::Foundation::CloseHandle(job);
                }
            }
        }
    }

    #[cfg(unix)]
    pub struct Tree(u32);

    #[cfg(unix)]
    impl Tree {
        /// The child leads its own process group (`process_group(0)`).
        pub fn of(child: &Child) -> Tree {
            Tree(child.id())
        }

        pub fn kill(&self, child: &mut Child) {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.0)])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            let _ = child.kill();
        }
    }
}

pub fn read_stdin() -> String {
    let mut buf = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_names_the_focus_and_the_live_windows() {
        let u = usage::Usage {
            observed_at: 0,
            five_hour: Some(usage::Window {
                used: 42.0,
                resets_at: 100,
            }),
            seven_day: Some(usage::Window {
                used: 18.0,
                resets_at: 1_000,
            }),
        };
        assert_eq!(
            segment(Some("T-12"), Some(&u), 0),
            "T-12 \u{b7} 5h 42% \u{b7} 7d 18%"
        );
        assert_eq!(segment(None, Some(&u), 200_000), "7d 18%");
        assert_eq!(segment(None, None, 0), "");
        assert!(is_id("T-12") && is_id("E-3"));
        assert!(!is_id("T-") && !is_id("P-1") && !is_id("T-1; rm"));
    }

    #[test]
    fn acta_status_lines_are_recognised_at_any_path() {
        assert!(is_acta_statusline(
            "C:/Users/alexandre/AppData/Local/Acta/bin/acta.exe statusline"
        ));
        assert!(is_acta_statusline("acta statusline"));
        assert!(is_acta_statusline("\"C:\\Old Place\\acta.exe\" statusline"));
        assert!(is_acta_statusline(
            "/home/alexandre/.local/bin/acta-cli statusline"
        ));
        assert!(!is_acta_statusline("~/.claude/alexandre-line.sh"));
        assert!(!is_acta_statusline("acta brief"));
        assert!(!is_acta_statusline("my-acta statusline"));
    }

    #[test]
    fn a_nested_call_and_an_acta_original_never_run_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let claude = dir.path().join("claude");
        std::fs::create_dir_all(&claude).unwrap();
        let record = |cmd: &str| {
            std::fs::write(
                claude.join("setup-record.json"),
                serde_json::json!({"statusline_original": {"command": cmd}}).to_string(),
            )
            .unwrap();
        };
        record("echo mine");
        assert_eq!(original_command(dir.path()).as_deref(), Some("echo mine"));
        // Called from inside the user's own command: Acta's segment only.
        assert_eq!(run_with("{}", Some(dir.path()), 0, true), "");
        // An Acta command recorded as the original is ignored.
        record("C:/elsewhere/acta.exe statusline");
        assert_eq!(original_command(dir.path()), None);
    }

    #[test]
    fn the_users_line_comes_first_and_is_never_dropped() {
        assert_eq!(combine(Some("main  42%\n"), "5h 3%"), "main  42%  5h 3%");
        assert_eq!(combine(Some("a\nb\n"), "x"), "a  x\nb");
        assert_eq!(combine(Some("mine"), ""), "mine");
        assert_eq!(combine(Some("  \n"), "x"), "x");
        assert_eq!(combine(None, "x"), "x");
    }

    fn platform(script_sh: &str, script_ps: &str) -> Command {
        if cfg!(windows) {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoProfile", "-NonInteractive", "-Command", script_ps]);
            c
        } else {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", script_sh]);
            c
        }
    }

    #[test]
    fn the_users_command_gets_the_same_stdin_and_its_output_whatever_its_exit() {
        let input = r#"{"session_id":"Alexandre"}"#;
        let echo = platform(
            "read line; echo \"got $line\"",
            "$l = [Console]::In.ReadToEnd(); Write-Output \"got $l\"",
        );
        let out = run_original(&echo, input, Duration::from_secs(30)).unwrap();
        assert_eq!(out.trim(), format!("got {input}"));
        // A failing command still shows what it printed.
        let failing = platform("echo partial; exit 3", "Write-Output partial; exit 3");
        let out = run_original(&failing, "", Duration::from_secs(30)).unwrap();
        assert_eq!(out.trim(), "partial");
        // One that hangs is killed and leaves Acta's segment alone.
        let hang = platform("sleep 20", "Start-Sleep -Seconds 20");
        let start = Instant::now();
        assert_eq!(run_original(&hang, "", Duration::from_millis(500)), None);
        assert!(start.elapsed() < Duration::from_secs(10));
        // Not found: nothing, no panic.
        let missing = Command::new("acta-no-such-program");
        assert_eq!(run_original(&missing, "", Duration::from_secs(1)), None);
    }
}
