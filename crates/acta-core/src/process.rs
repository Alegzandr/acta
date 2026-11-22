//! Running a child process with a time limit: the app's setup check and the
//! CLI's calls to `claude` must never wait forever on a command that hangs.

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// What a bounded run produced.
#[derive(Debug)]
pub struct Bounded {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Run `command` with stdin closed and both outputs captured. `Ok(None)`
/// when it was still running after `limit`, or left its output open past
/// it: the whole process tree is then killed (a job object on Windows, the
/// process group elsewhere) and reaped. On Windows it runs with no console
/// window.
pub fn output_within(command: &mut Command, limit: Duration) -> io::Result<Option<Bounded>> {
    output_limited(
        command,
        Limits {
            time: limit,
            memory: None,
            output: None,
        },
    )
}

/// What a bounded run may use.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub time: Duration,
    /// Memory the whole tree may commit, in bytes (a job object limit on
    /// Windows, `RLIMIT_AS` on Unix). Over it, allocations fail and the
    /// child ends with an error.
    pub memory: Option<u64>,
    /// Bytes of stdout and of stderr kept; a child writing more is killed
    /// and the run gives `None`, as a timeout does.
    pub output: Option<usize>,
}

/// [`output_within`] with a memory cap and an output cap as well.
pub fn output_limited(command: &mut Command, limits: Limits) -> io::Result<Option<Bounded>> {
    output_limited_with_input(command, limits, None)
}

/// [`output_limited`], writing `input` to the child's stdin (then closing
/// it) instead of giving it none.
pub fn output_limited_with_input(
    command: &mut Command,
    limits: Limits,
    input: Option<&[u8]>,
) -> io::Result<Option<Bounded>> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, tree) = spawn_limited(command, limits.memory)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        use std::io::Write as _;
        let input = input.to_vec();
        // On its own thread: a child that never reads cannot block us.
        thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
    }
    // The pipes are drained on their own threads, so a chatty child never
    // blocks on a full pipe while we wait.
    let stdout = drain(child.stdout.take(), limits.output);
    let stderr = drain(child.stderr.take(), limits.output);
    let limit = limits.time;
    let deadline = Instant::now() + limit;
    let killed = |child: &mut Child| {
        tree.kill(child);
        let _ = child.wait();
        Ok(None)
    };
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            return killed(&mut child);
        }
        thread::sleep(Duration::from_millis(20));
    };
    // A process it started may still hold the output open: wait for the
    // end of output only until the deadline.
    let left = || {
        deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(50))
    };
    let Ok(Some(out)) = stdout.recv_timeout(left()) else {
        return killed(&mut child);
    };
    let Ok(Some(err)) = stderr.recv_timeout(left()) else {
        return killed(&mut child);
    };
    Ok(Some(Bounded {
        status,
        stdout: out,
        stderr: err,
    }))
}

/// Reads a pipe to its end on its own thread. `None` when it gave more
/// than `cap` bytes: the reading stops there.
fn drain<R: Read + Send + 'static>(
    pipe: Option<R>,
    cap: Option<usize>,
) -> mpsc::Receiver<Option<Vec<u8>>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(pipe) = pipe {
            match cap {
                Some(cap) => {
                    let _ = pipe.take(cap as u64 + 1).read_to_end(&mut out);
                    if out.len() > cap {
                        let _ = tx.send(None);
                        return;
                    }
                }
                None => {
                    let mut pipe = pipe;
                    let _ = pipe.read_to_end(&mut out);
                }
            }
        }
        let _ = tx.send(Some(out));
    });
    rx
}

pub use tree::Tree;

/// Spawns `command` in its own tree: on Windows inside a job object, with
/// no console window, started suspended so nothing it starts escapes the
/// job; elsewhere as the leader of a new process group. If the suspended
/// child cannot be resumed (the thread snapshot failed), it is killed
/// before it ran and started again outside any job.
///
/// When the returned [`Tree`] is dropped, the job is closed and every
/// process still in it is killed (`KILL_ON_JOB_CLOSE`): a helper the child
/// left running (a credential helper, git's fsmonitor daemon) does not
/// outlive the run.
pub fn spawn_in_tree(command: &mut Command) -> io::Result<(Child, Tree)> {
    spawn_limited(command, None)
}

/// [`spawn_in_tree`], with the memory the tree may commit capped.
fn spawn_limited(command: &mut Command, memory: Option<u64>) -> io::Result<(Child, Tree)> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_SUSPENDED: u32 = 0x0000_0004;
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = command.spawn()?;
        let (tree, resumed) = Tree::of(&child, memory);
        if resumed && (memory.is_none() || tree.is_job()) {
            return Ok((child, tree));
        }
        // A capped run never goes on without its cap.
        if memory.is_some() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("could not limit the child's memory"));
        }
        let _ = child.kill();
        let _ = child.wait();
        drop(tree);
        command.creation_flags(CREATE_NO_WINDOW);
        Ok((command.spawn()?, Tree::none()))
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
        if let Some(bytes) = memory {
            // SAFETY: `setrlimit` is async-signal-safe; nothing else runs
            // between fork and exec here.
            unsafe {
                command.pre_exec(move || {
                    let lim = libc::rlimit {
                        rlim_cur: bytes as libc::rlim_t,
                        rlim_max: bytes as libc::rlim_t,
                    };
                    // `RLIMIT_DATA` counts heap and private mappings on
                    // Linux (4.7 and later). macOS does not enforce it and
                    // may refuse it: the time and output caps still hold.
                    let failed = libc::setrlimit(libc::RLIMIT_DATA, &lim) != 0;
                    if failed && cfg!(target_os = "linux") {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        let child = command.spawn()?;
        let tree = Tree::of(&child);
        Ok((child, tree))
    }
}

/// The process tree of a child: what a timeout kills.
mod tree {
    use std::process::Child;

    #[cfg(windows)]
    pub struct Tree(Option<windows_sys::Win32::Foundation::HANDLE>);

    #[cfg(windows)]
    impl Tree {
        /// No job: `kill` ends the child alone.
        pub(super) fn none() -> Tree {
            Tree(None)
        }

        /// A job object holding the child (and everything it starts),
        /// closed with the job (`KILL_ON_JOB_CLOSE`), and whether the
        /// suspended child was resumed.
        pub(super) fn is_job(&self) -> bool {
            self.0.is_some()
        }

        pub(super) fn of(child: &Child, memory: Option<u64>) -> (Tree, bool) {
            use std::os::windows::io::AsRawHandle as _;
            use windows_sys::Win32::System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_JOB_MEMORY,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectExtendedLimitInformation, SetInformationJobObject,
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
                    if let Some(bytes) = memory {
                        info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
                        info.JobMemoryLimit = bytes as usize;
                    }
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
            let resumed = resume(child.id());
            (tree, resumed)
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
    /// handle to its first thread; there is one at this point). `false`
    /// when none could be resumed.
    #[cfg(windows)]
    fn resume(pid: u32) -> bool {
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
                return false;
            }
            let mut resumed = false;
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            let mut more = Thread32First(snap, &mut entry) != 0;
            while more {
                if entry.th32OwnerProcessID == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                    if !thread.is_null() {
                        resumed |= ResumeThread(thread) != u32::MAX;
                        CloseHandle(thread);
                    }
                }
                more = Thread32Next(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
            resumed
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
        pub(super) fn of(child: &Child) -> Tree {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> Command {
        if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", script]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", script]);
            c
        }
    }

    #[test]
    fn a_quick_command_returns_its_output() {
        let out = output_within(&mut shell("echo Alexandre"), Duration::from_secs(20))
            .unwrap()
            .expect("finished");
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("Alexandre"));
    }

    /// A grandchild holding the output open: the run ends at the deadline
    /// and the grandchild goes with the tree.
    #[test]
    fn a_grandchild_holding_the_output_is_killed_too() {
        let script = if cfg!(windows) {
            "start /b ping -n 30 127.0.0.1 & echo Alexandre"
        } else {
            "sleep 30 & echo Alexandre"
        };
        let started = Instant::now();
        let out = output_within(&mut shell(script), Duration::from_millis(1500)).unwrap();
        assert!(out.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn output_over_the_cap_ends_the_run() {
        let script = if cfg!(windows) {
            "for /L %i in (1,1,2000) do @echo Alexandre Alexandre Alexandre"
        } else {
            "i=0; while [ $i -lt 2000 ]; do echo Alexandre Alexandre Alexandre; i=$((i+1)); done"
        };
        let limits = |output| Limits {
            time: Duration::from_secs(20),
            memory: Some(256 * 1024 * 1024),
            output,
        };
        let out = output_limited(&mut shell(script), limits(Some(1_000))).unwrap();
        assert!(out.is_none());
        let out = output_limited(&mut shell(script), limits(Some(1_000_000)))
            .unwrap()
            .expect("under the cap");
        assert!(out.stdout.len() > 50_000);
    }

    /// A child that allocates past the cap fails instead of growing, on
    /// Windows (job object) and Linux (`RLIMIT_DATA`). macOS does not
    /// enforce a data limit: there the run only has to end within its
    /// other limits.
    #[test]
    fn a_child_over_its_memory_cap_fails() {
        let enforced = cfg!(any(windows, target_os = "linux"));
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("powershell");
            c.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$ErrorActionPreference = 'Stop'; $b = New-Object byte[] 1GB; $b[1] = 1; 'allocated'",
            ]);
            c
        } else {
            shell("head -c 400000000 /dev/zero | tr '\\0' a | awk '{ print \"allocated\" }'")
        };
        let out = output_limited(
            &mut cmd,
            Limits {
                time: Duration::from_secs(60),
                memory: Some(200 * 1024 * 1024),
                output: Some(64 * 1024),
            },
        )
        .unwrap();
        if let (Some(out), true) = (out, enforced) {
            assert!(
                !String::from_utf8_lossy(&out.stdout).contains("allocated"),
                "{out:?}"
            );
        }
    }

    #[test]
    fn input_reaches_the_child() {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("findstr");
            c.arg("Alexandre");
            c
        } else {
            Command::new("cat")
        };
        let limits = Limits {
            time: Duration::from_secs(20),
            memory: None,
            output: Some(1_000),
        };
        let out = output_limited_with_input(&mut cmd, limits, Some(b"Alexandre\n"))
            .unwrap()
            .expect("finished");
        assert!(String::from_utf8_lossy(&out.stdout).contains("Alexandre"));
    }

    #[test]
    fn a_command_past_its_limit_is_killed() {
        let script = if cfg!(windows) {
            "ping -n 30 127.0.0.1 >NUL"
        } else {
            "sleep 30"
        };
        let started = Instant::now();
        let out = output_within(&mut shell(script), Duration::from_millis(300)).unwrap();
        assert!(out.is_none());
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
