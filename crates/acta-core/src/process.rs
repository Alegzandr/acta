//! Running a child process with a time limit: the app's setup check and the
//! CLI's calls to `claude` must never wait forever on a command that hangs.

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
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
/// when it was still running after `limit`: it is then killed and reaped.
pub fn output_within(command: &mut Command, limit: Duration) -> io::Result<Option<Bounded>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    // The pipes are drained on their own threads, so a chatty child never
    // blocks on a full pipe while we wait.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let Some(status) = status else {
        kill(&mut child);
        return Ok(None);
    };
    Ok(Some(Bounded {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    }))
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut out);
        }
        out
    })
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
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
