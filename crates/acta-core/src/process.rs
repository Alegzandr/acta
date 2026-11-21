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
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, tree) = spawn_in_tree(command)?;
    // The pipes are drained on their own threads, so a chatty child never
    // blocks on a full pipe while we wait.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
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
    let Ok(out) = stdout.recv_timeout(left()) else {
        return killed(&mut child);
    };
    let Ok(err) = stderr.recv_timeout(left()) else {
        return killed(&mut child);
    };
    Ok(Some(Bounded {
        status,
        stdout: out,
        stderr: err,
    }))
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut out);
        }
        let _ = tx.send(out);
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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_SUSPENDED: u32 = 0x0000_0004;
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = command.spawn()?;
        let (tree, resumed) = Tree::of(&child);
        if resumed {
            return Ok((child, tree));
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
        pub(super) fn of(child: &Child) -> (Tree, bool) {
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
