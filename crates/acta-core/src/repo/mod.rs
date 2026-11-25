//! Files and commits (components-repo.md 5.R.10): what the Files and
//! Commits views and the file viewer read from a working folder. Pure and
//! UI-free; every function blocks, so the app calls them off the UI thread.
//!
//! - [`fs`]: a folder's entries with git's ignore rules matched in Rust (no
//!   process), the name search walk and its ranking, a file read for the
//!   viewer, and the "never run a program" check.
//! - [`Repo`]: git read as ADR 0011 reads it. Every call goes through one
//!   runner ([`Repo::run`]): [`crate::git_cmd::command_with`] (no optional
//!   locks, no prompt, no fsmonitor, no inherited `GIT_*`, C locale), plus
//!   `core.quotepath=off` and `GIT_PAGER=cat`, stdin closed, the output
//!   capped, and the whole process tree killed at the call's time limit.
//!   Nothing takes a lock or writes the index or a ref; diffs pass
//!   `--no-ext-diff --no-textconv --no-color` so no configured program runs.
//! - [`graph`]: the streaming lane builder of the commit graph (5.R.6).

mod details;
pub mod fs;
pub mod graph;
mod log;
mod status;

#[cfg(test)]
mod tests;

pub use crate::git_cmd::GitError;
pub use details::{ChangeKind, ChangedFile, CommitDetails, Diff, DiffLine, Hunk, LineKind};
pub use graph::{LaneBuilder, Line, Row, Segment};
pub use log::{
    Commit, LogPage, LogQuery, MAX_COMMITS, PAGE_SIZE, Ref, RefKind, RefsSnapshot, Selection,
    parse_page,
};
pub use status::{Mark, Status, StatusEntry};

use crate::git_cmd::{classify, command_with};
use crate::process::spawn_in_tree;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Time limits of 5.R.10's table.
pub const STATUS_LIMIT: Duration = Duration::from_secs(2);
pub const PAGE_LIMIT: Duration = Duration::from_secs(10);
pub const REFS_LIMIT: Duration = Duration::from_secs(2);
pub const DETAILS_LIMIT: Duration = Duration::from_secs(5);
pub const DIFF_LIMIT: Duration = Duration::from_secs(5);
pub const BLOB_LIMIT: Duration = Duration::from_secs(5);

/// Git output read for one file's diff; past it the diff is truncated.
pub const DIFF_BYTES: usize = 2 * 1024 * 1024;
/// Diff lines kept per file (5.R.7, Large).
pub const DIFF_LINES: usize = 5_000;
/// An image's version read into memory (5.R.4).
pub const BLOB_BYTES: usize = 50 * 1024 * 1024;
/// Everything else git prints (a page of 1,000 commits is about 200 KB).
pub(super) const OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const STDERR_BYTES: usize = 64 * 1024;

/// A git repository's worktree, seen from one of its folders.
#[derive(Clone, Debug)]
pub struct Repo {
    /// The worktree's root: every git call runs there and every path is
    /// relative to it, with `/`.
    root: PathBuf,
    /// The git directory shared by all worktrees.
    common_dir: PathBuf,
    /// The folder asked for, relative to `root` (`""` at the root): the
    /// default path filter of Commits for a subfolder (5.R.5, Path chip).
    prefix: String,
    git: OsString,
}

/// What a run printed.
struct Out {
    stdout: Vec<u8>,
    /// The output reached its cap: the process was killed there.
    truncated: bool,
}

impl Repo {
    /// The repository holding `folder`, found from the files (no process).
    pub fn open(folder: &Path) -> Result<Repo, GitError> {
        Repo::open_with(folder, OsStr::new("git"))
    }

    /// [`Repo::open`] running `git` as `program` (a full path to git, or a
    /// test's stand-in).
    pub fn open_with(folder: &Path, program: &OsStr) -> Result<Repo, GitError> {
        let info = crate::agent::git::inspect(folder).ok_or(GitError::NotARepository)?;
        let prefix = folder
            .strip_prefix(&info.root)
            .map(slash_path)
            .unwrap_or_default();
        Ok(Repo {
            root: info.root,
            common_dir: info.common_dir,
            prefix,
            git: program.to_owned(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder this repository was opened from, relative to the root.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The one runner: see the module comment. A non-zero exit is an
    /// error classified from git's message.
    fn run(&self, args: &[&str], limit: Duration, cap: usize) -> Result<Out, GitError> {
        let mut cmd = command_with(&self.git, &self.root);
        cmd.args(["-c", "core.quotepath=off"])
            .env("GIT_PAGER", "cat")
            .env_remove("GIT_EXTERNAL_DIFF")
            // A partial clone never downloads a missing object for a read:
            // the call fails instead ([`GitError::NotDownloaded`]).
            .env("GIT_NO_LAZY_FETCH", "1")
            // A path is a path: `app/[id]/page.tsx` is not a glob, `:(top)`
            // is not magic.
            .env("GIT_LITERAL_PATHSPECS", "1")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (mut child, tree) = match spawn_in_tree(&mut cmd) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(GitError::NoGit),
            Err(e) => return Err(GitError::Other(e.kind().to_string())),
        };
        let stdout = read_capped(child.stdout.take(), cap);
        let stderr = read_capped(child.stderr.take(), STDERR_BYTES);
        let deadline = Instant::now() + limit;
        let left = || deadline.saturating_duration_since(Instant::now());
        let kill = |child: &mut std::process::Child| {
            tree.kill(child);
            let _ = child.wait();
        };
        let Ok((out, truncated)) = stdout.recv_timeout(left()) else {
            kill(&mut child);
            return Err(GitError::TimedOut);
        };
        if truncated {
            kill(&mut child);
            return Ok(Out {
                stdout: out,
                truncated,
            });
        }
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if left().is_zero() => {
                    kill(&mut child);
                    return Err(GitError::TimedOut);
                }
                Ok(None) => thread::sleep(Duration::from_millis(2)),
                Err(e) => return Err(GitError::Other(e.kind().to_string())),
            }
        };
        if status.success() {
            return Ok(Out {
                stdout: out,
                truncated: false,
            });
        }
        let err = stderr
            .recv_timeout(left().max(Duration::from_millis(50)))
            .map(|(e, _)| e)
            .unwrap_or_default();
        let err = String::from_utf8_lossy(&err);
        if err.contains("lazy fetching disabled") {
            return Err(GitError::NotDownloaded);
        }
        Err(classify(&err))
    }

    /// [`Repo::run`] with the default output cap, as text.
    fn text(&self, args: &[&str], limit: Duration) -> Result<String, GitError> {
        let out = self.run(args, limit, OUTPUT_BYTES)?;
        if out.truncated {
            return Err(GitError::Other("git printed too much".into()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Reads a pipe on its own thread: at most `cap` bytes, and whether there
/// was more. The rest is read and dropped, so a chatty child never blocks
/// on a full pipe (a cut stdout ends when the child is killed).
fn read_capped<R: Read + Send + 'static>(
    pipe: Option<R>,
    cap: usize,
) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut out = Vec::new();
        let Some(mut pipe) = pipe else {
            let _ = tx.send((out, false));
            return;
        };
        let _ = (&mut pipe).take(cap as u64 + 1).read_to_end(&mut out);
        let more = out.len() > cap;
        out.truncate(cap);
        let _ = tx.send((out, more));
        let _ = std::io::copy(&mut pipe, &mut std::io::sink());
    });
    rx
}

/// A relative path with `/` separators.
pub(crate) fn slash_path(p: &Path) -> String {
    let mut s = String::new();
    for c in p.components() {
        if !s.is_empty() {
            s.push('/');
        }
        s.push_str(&c.as_os_str().to_string_lossy());
    }
    s
}

/// A commit id or a short form of one: hexadecimal, 4 to 64 characters.
/// Never mistaken for an option.
pub fn is_hex_id(s: &str) -> bool {
    (4..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A path argument after `--`: relative, `/`-separated, not empty.
fn checked_path(p: &str) -> Result<&str, GitError> {
    if p.is_empty() || p.starts_with('/') || p.contains('\0') {
        Err(GitError::Other("not a relative path".into()))
    } else {
        Ok(p)
    }
}

/// A commit named by the views: at least 7 hexadecimal digits, so a short
/// form is never ambiguous by construction.
fn checked_id(id: &str) -> Result<&str, GitError> {
    if id.len() >= 7 && is_hex_id(id) {
        Ok(id)
    } else {
        Err(GitError::Other("not a commit id".into()))
    }
}
