//! Running an install the user confirmed (ADR 0013, section 3, "Running"):
//! one at a time, a child with no window and no Claude Code session
//! variables, its stdout and stderr in a ring of the last 400 lines with
//! control sequences stripped, its exit code mapped to Installed or Failed,
//! and a re-detection with the fresh `PATH` after success. No timeout and
//! no cancel: interrupting an installer half way does more harm than
//! waiting. The log lives in memory only; nothing here writes a file.

use super::Tool;
use std::collections::{HashMap, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// Lines kept per install.
pub const LOG_LINES: usize = 400;

/// What the runner asks of the rest of Acta: the fresh `PATH` handed to
/// every child, and whether a tool is found after an install.
pub trait InstallerEnv: Send + Sync + 'static {
    fn fresh_path(&self) -> Option<OsString>;
    /// Whether `tool` is found now, with `path` as the fresh `PATH`.
    fn found(&self, tool: Tool, path: Option<&OsStr>) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The installer ended with this code (`None`: killed by a signal).
    Exit(Option<i32>),
    /// It could not be started.
    NoStart,
    /// It succeeded, but the tool is not found yet ("Restart Acta").
    NotFound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Running,
    Installed,
    Failed(Failure),
}

/// Another install is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Busy;

struct Job {
    phase: Phase,
    started: Instant,
    ended: Option<Instant>,
    log: VecDeque<String>,
}

#[derive(Default)]
struct Shared {
    jobs: HashMap<Tool, Job>,
    busy: Option<Tool>,
    /// The `PATH` the last child was given.
    last_path: Option<OsString>,
}

/// The install runner of one Acta process. Clones share it.
#[derive(Clone)]
pub struct Installer {
    shared: Arc<(Mutex<Shared>, Condvar)>,
    env: Arc<dyn InstallerEnv>,
}

impl Installer {
    pub fn new(env: Arc<dyn InstallerEnv>) -> Installer {
        Installer {
            shared: Arc::new((Mutex::new(Shared::default()), Condvar::new())),
            env,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts `argv` for `tool`; refused while another install runs.
    pub fn start(&self, tool: Tool, argv: &[OsString]) -> Result<(), Busy> {
        let mut shared = self.lock();
        if shared.busy.is_some() {
            return Err(Busy);
        }
        shared.busy = Some(tool);
        shared.jobs.insert(
            tool,
            Job {
                phase: Phase::Running,
                started: Instant::now(),
                ended: None,
                log: VecDeque::new(),
            },
        );
        let path = self.env.fresh_path();
        shared.last_path = path.clone();
        drop(shared);

        let Some((program, args)) = argv.split_first() else {
            self.finish(tool, Phase::Failed(Failure::NoStart));
            return Ok(());
        };
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(p) = &path {
            cmd.env("PATH", p);
        }
        for v in super::locate::session_vars_here() {
            cmd.env_remove(v);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            // CREATE_NO_WINDOW: no console; an installer's own window
            // (Git's setup) still shows.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => {
                self.finish(tool, Phase::Failed(Failure::NoStart));
                return Ok(());
            }
        };
        let readers: Vec<_> = [
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        ]
        .into_iter()
        .flatten()
        .map(|pipe| {
            let me = self.clone();
            thread::spawn(move || me.read_into_log(tool, pipe))
        })
        .collect();
        let me = self.clone();
        thread::spawn(move || {
            let status = child.wait();
            for r in readers {
                let _ = r.join();
            }
            let phase = match status {
                Ok(s) if s.success() => {
                    let fresh = me.env.fresh_path();
                    if me.env.found(tool, fresh.as_deref()) {
                        Phase::Installed
                    } else {
                        Phase::Failed(Failure::NotFound)
                    }
                }
                Ok(s) => Phase::Failed(Failure::Exit(s.code())),
                Err(_) => Phase::Failed(Failure::Exit(None)),
            };
            me.finish(tool, phase);
        });
        Ok(())
    }

    fn finish(&self, tool: Tool, phase: Phase) {
        let mut shared = self.lock();
        if let Some(job) = shared.jobs.get_mut(&tool) {
            job.phase = phase;
            job.ended = Some(Instant::now());
        }
        shared.busy = None;
        drop(shared);
        self.shared.1.notify_all();
    }

    fn read_into_log(&self, tool: Tool, mut pipe: Box<dyn Read + Send>) {
        let mut pending: Vec<u8> = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            pending.extend_from_slice(&buf[..n]);
            while let Some(i) = pending.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = pending.drain(..=i).collect();
                self.push_line(tool, &line[..line.len() - 1]);
            }
        }
        if !pending.is_empty() {
            self.push_line(tool, &pending);
        }
    }

    fn push_line(&self, tool: Tool, raw: &[u8]) {
        let line = strip_controls(&String::from_utf8_lossy(raw))
            .trim_end()
            .to_owned();
        let mut shared = self.lock();
        if let Some(job) = shared.jobs.get_mut(&tool) {
            if job.log.len() == LOG_LINES {
                job.log.pop_front();
            }
            job.log.push_back(line);
        }
    }

    pub fn phase(&self, tool: Tool) -> Option<Phase> {
        self.lock().jobs.get(&tool).map(|j| j.phase)
    }

    /// The last [`LOG_LINES`] lines, oldest first.
    pub fn log(&self, tool: Tool) -> Vec<String> {
        self.lock()
            .jobs
            .get(&tool)
            .map(|j| j.log.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// How long it has run (or ran).
    pub fn elapsed(&self, tool: Tool) -> Option<Duration> {
        self.lock()
            .jobs
            .get(&tool)
            .map(|j| j.ended.unwrap_or_else(Instant::now) - j.started)
    }

    /// The install running now.
    pub fn busy(&self) -> Option<Tool> {
        self.lock().busy
    }

    /// The `PATH` the last child was given.
    pub fn last_path(&self) -> Option<OsString> {
        self.lock().last_path.clone()
    }

    /// Blocks until `tool` is no longer running, at most `limit`.
    pub fn wait(&self, tool: Tool, limit: Duration) -> Option<Phase> {
        let deadline = Instant::now() + limit;
        let mut shared = self.lock();
        loop {
            let phase = shared.jobs.get(&tool).map(|j| j.phase);
            if phase != Some(Phase::Running) {
                return phase;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return phase;
            }
            shared = self
                .shared
                .1
                .wait_timeout(shared, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

/// A line as a person reads it: escape sequences (CSI, OSC and two-byte
/// ones) and control characters removed, a carriage-return redraw kept as
/// its last state (a progress bar shows its final value).
pub fn strip_controls(line: &str) -> String {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    // CSI: parameters and intermediates up to a final byte.
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC: up to BEL or ESC \.
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Character set designation: one more byte.
                Some('(' | ')' | '*' | '+') => {
                    chars.next();
                }
                _ => {}
            },
            '\r' => out.clear(),
            '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct FakeEnv {
        /// Where the "installed" tool appears.
        expected: PathBuf,
        marker: PathBuf,
        paths: AtomicUsize,
        checked: AtomicBool,
    }

    impl InstallerEnv for FakeEnv {
        fn fresh_path(&self) -> Option<OsString> {
            // The real PATH with a marker folder; its name changes after
            // each call, as the registry would after an install.
            let n = self.paths.fetch_add(1, Ordering::SeqCst);
            let mut dirs = vec![self.marker.join(format!("fresh{n}"))];
            dirs.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            std::env::join_paths(dirs).ok()
        }

        fn found(&self, _: Tool, path: Option<&OsStr>) -> bool {
            self.checked.store(true, Ordering::SeqCst);
            assert!(path.is_some());
            self.expected.is_file()
        }
    }

    fn env(dir: &Path) -> Arc<FakeEnv> {
        Arc::new(FakeEnv {
            expected: dir.join("installed-tool"),
            marker: dir.join("marker"),
            paths: AtomicUsize::new(0),
            checked: AtomicBool::new(false),
        })
    }

    /// A fake installer: a script in `dir` that runs `body`.
    fn installer_stub(dir: &Path, name: &str, body: &str) -> OsString {
        if cfg!(windows) {
            let p = dir.join(format!("{name}.cmd"));
            std::fs::write(&p, format!("@echo off\r\n{body}")).unwrap();
            p.into_os_string()
        } else {
            let p = dir.join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}")).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            p.into_os_string()
        }
    }

    fn listing(dir: &Path) -> Vec<(PathBuf, u64)> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(listing(&p));
            } else {
                out.push((p, e.metadata().unwrap().len()));
            }
        }
        out.sort();
        out
    }

    const LONG: Duration = Duration::from_secs(60);

    #[test]
    fn a_failing_installer_keeps_its_code_and_log() {
        let tmp = tempfile::tempdir().unwrap();
        let body = if cfg!(windows) {
            "echo Installing for Alexandre\r\necho oops 1>&2\r\nexit /b 1603\r\n"
        } else {
            "echo Installing for Alexandre\necho oops 1>&2\nexit 1603\n"
        };
        let stub = installer_stub(tmp.path(), "fail", body);
        let before = listing(tmp.path());
        let inst = Installer::new(env(tmp.path()));
        inst.start(Tool::Git, &[stub]).unwrap();
        let phase = inst.wait(Tool::Git, LONG);
        // Unix exit codes are 8 bits.
        let want = if cfg!(windows) { 1603 } else { 1603 & 0xff };
        assert_eq!(phase, Some(Phase::Failed(Failure::Exit(Some(want)))));
        let log = inst.log(Tool::Git);
        assert!(
            log.contains(&"Installing for Alexandre".to_owned()),
            "{log:?}"
        );
        assert!(log.contains(&"oops".to_owned()), "{log:?}");
        assert_eq!(inst.busy(), None);
        // Nothing of the log reached a file.
        assert_eq!(listing(tmp.path()), before);
    }

    #[test]
    fn found_after_install_and_not_found_after_install() {
        let tmp = tempfile::tempdir().unwrap();
        let fake = env(tmp.path());
        let target = fake.expected.clone();
        let creates = if cfg!(windows) {
            format!("echo done> \"{}\"\r\nexit /b 0\r\n", target.display())
        } else {
            format!("echo done > '{}'\nexit 0\n", target.display())
        };
        let quiet = installer_stub(
            tmp.path(),
            "quiet",
            if cfg!(windows) {
                "exit /b 0\r\n"
            } else {
                "exit 0\n"
            },
        );
        let inst = Installer::new(fake.clone());
        inst.start(Tool::Gh, &[quiet]).unwrap();
        assert_eq!(
            inst.wait(Tool::Gh, LONG),
            Some(Phase::Failed(Failure::NotFound))
        );
        assert!(fake.checked.load(Ordering::SeqCst));

        let stub = installer_stub(tmp.path(), "creates", &creates);
        inst.start(Tool::Gh, &[stub]).unwrap();
        assert_eq!(inst.wait(Tool::Gh, LONG), Some(Phase::Installed));
        assert!(inst.elapsed(Tool::Gh).is_some());
    }

    #[test]
    fn one_install_at_a_time_and_the_fresh_path_reaches_later_children() {
        let tmp = tempfile::tempdir().unwrap();
        let slow = if cfg!(windows) {
            "ping -n 3 127.0.0.1 >NUL\r\necho PATH=%PATH%\r\nexit /b 0\r\n"
        } else {
            "sleep 2\necho PATH=$PATH\nexit 0\n"
        };
        let stub = installer_stub(tmp.path(), "slow", slow);
        let inst = Installer::new(env(tmp.path()));
        inst.start(Tool::Claude, std::slice::from_ref(&stub))
            .unwrap();
        assert_eq!(inst.phase(Tool::Claude), Some(Phase::Running));
        assert_eq!(inst.busy(), Some(Tool::Claude));
        assert_eq!(
            inst.start(Tool::Git, std::slice::from_ref(&stub)),
            Err(Busy)
        );
        assert_eq!(inst.phase(Tool::Git), None);
        inst.wait(Tool::Claude, LONG);
        let first = inst.log(Tool::Claude).join("\n");
        assert!(first.contains("fresh0"), "{first}");
        // The next child gets the PATH as it is after the first install.
        inst.start(Tool::Git, &[stub]).unwrap();
        inst.wait(Tool::Git, LONG);
        let second = inst.log(Tool::Git).join("\n");
        assert!(second.contains("fresh2"), "{second}");
        assert_eq!(
            inst.last_path()
                .map(|p| p.to_string_lossy().contains("fresh2")),
            Some(true)
        );
    }

    #[test]
    fn a_program_that_cannot_start_fails_without_start() {
        let tmp = tempfile::tempdir().unwrap();
        let inst = Installer::new(env(tmp.path()));
        let missing = tmp.path().join("no-such-installer-for-alexandre");
        inst.start(Tool::Gh, &[missing.into_os_string()]).unwrap();
        assert_eq!(
            inst.wait(Tool::Gh, LONG),
            Some(Phase::Failed(Failure::NoStart))
        );
        assert_eq!(inst.busy(), None);
    }

    #[test]
    fn the_ring_keeps_the_last_400_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let body = if cfg!(windows) {
            "for /L %%i in (1,1,450) do @echo line %%i\r\n"
        } else {
            "i=1; while [ $i -le 450 ]; do echo line $i; i=$((i+1)); done\n"
        };
        let stub = installer_stub(tmp.path(), "chatty", body);
        let inst = Installer::new(env(tmp.path()));
        inst.start(Tool::Git, &[stub]).unwrap();
        inst.wait(Tool::Git, LONG);
        let log = inst.log(Tool::Git);
        assert_eq!(log.len(), LOG_LINES);
        assert_eq!(log[0], "line 51");
        assert_eq!(log[LOG_LINES - 1], "line 450");
    }

    #[test]
    fn control_sequences_are_stripped() {
        assert_eq!(strip_controls("\u{1b}[1;32mDone\u{1b}[0m\r"), "Done");
        assert_eq!(
            strip_controls("  10%\r  55%\r 100% Alexandre"),
            " 100% Alexandre"
        );
        assert_eq!(
            strip_controls("\u{1b}]0;title\u{7}text\u{1b}]8;;x\u{1b}\\ok"),
            "textok"
        );
        assert_eq!(strip_controls("a\u{8}b\u{7}\tc\u{1b}(B"), "ab c");
        assert_eq!(strip_controls("\u{1b}[?25l\u{1b}[2K"), "");
    }
}
