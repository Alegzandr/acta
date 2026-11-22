//! Git run as a process, for what the files cannot tell (ADR 0011, section
//! 2 and the 2026-11-21 addenda): the working-tree summary of the folder
//! row, and the human actions Acta runs for the user (pull, branch list,
//! switch, create). Never run by or for Claude Code.
//!
//! Every run: stdin closed, a time limit, no console window on Windows,
//! `--no-optional-locks` and `GIT_OPTIONAL_LOCKS=0` (a read never takes
//! `index.lock`), `GIT_TERMINAL_PROMPT=0` and `GCM_INTERACTIVE=never` (a
//! sign-in fails instead of waiting), `core.fsmonitor` off (a repository's
//! config could name a command to run on every status or `ls-files`),
//! messages in the C locale so the error
//! kinds can be told apart, and none of the `GIT_DIR`-style variables of a
//! surrounding git process. Nothing here stashes, forces or edits config.

use crate::process::{Bounded, output_within};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Limit of the working-tree summary (ADR 0011: past it, nothing is shown).
pub const STATUS_LIMIT: Duration = Duration::from_secs(2);
const LOCAL_LIMIT: Duration = Duration::from_secs(10);
/// The guard's alias lookup runs inside a hook: short.
const ALIAS_LIMIT: Duration = Duration::from_secs(2);
const NETWORK_LIMIT: Duration = Duration::from_secs(60);

/// Why a git action did not happen. Each kind has its own short message in
/// the UI; `Other` carries git's last line, at most 200 chars.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitError {
    /// No `git` command on this computer.
    NoGit,
    NotARepository,
    TimedOut,
    /// The current branch has no upstream to pull from.
    NoUpstream,
    /// HEAD is detached: there is no current branch.
    Detached,
    /// The branch has diverged: a fast-forward is not possible.
    NotFastForward,
    /// Local changes (or untracked files) would be overwritten.
    LocalChanges,
    /// The remote asked for a sign-in, which is never prompted for.
    SignInNeeded,
    /// The remote could not be reached.
    Network,
    BranchNotFound,
    BranchExists,
    InvalidBranchName,
    /// The branch is checked out in another worktree.
    CheckedOutElsewhere,
    Other(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GitError::NoGit => "git is not installed",
            GitError::NotARepository => "not a git repository",
            GitError::TimedOut => "git took too long",
            GitError::NoUpstream => "the branch has no upstream",
            GitError::Detached => "HEAD is detached",
            GitError::NotFastForward => "the branch has diverged; not a fast-forward",
            GitError::LocalChanges => "local changes would be overwritten",
            GitError::SignInNeeded => "the remote needs a sign-in",
            GitError::Network => "the remote could not be reached",
            GitError::BranchNotFound => "no such branch",
            GitError::BranchExists => "a branch with this name exists",
            GitError::InvalidBranchName => "not a valid branch name",
            GitError::CheckedOutElsewhere => "the branch is checked out in another worktree",
            GitError::Other(m) => m,
        };
        f.write_str(s)
    }
}

/// The base command: see the module comment.
pub fn command(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir)
        .arg("--no-optional-locks")
        .args(["-c", "core.fsmonitor=false"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C");
    for v in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_PREFIX",
        "GIT_OBJECT_DIRECTORY",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
    ] {
        c.env_remove(v);
    }
    // `GIT_CONFIG_KEY_<n>` / `GIT_CONFIG_VALUE_<n>` from a surrounding
    // process: config Acta did not choose.
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy();
        if k.starts_with("GIT_CONFIG_KEY_") || k.starts_with("GIT_CONFIG_VALUE_") {
            c.env_remove(k.as_ref());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up, and none takes focus.
        c.creation_flags(0x0800_0000);
    }
    c
}

fn run(dir: &Path, args: &[&str], limit: Duration) -> Result<Bounded, GitError> {
    match output_within(command(dir).args(args), limit) {
        Ok(Some(out)) => Ok(out),
        Ok(None) => Err(GitError::TimedOut),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(GitError::NoGit),
        Err(e) => Err(GitError::Other(e.kind().to_string())),
    }
}

/// Runs and requires success; a failure is classified from stderr.
fn run_ok(dir: &Path, args: &[&str], limit: Duration) -> Result<String, GitError> {
    let out = run(dir, args, limit)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(classify(&String::from_utf8_lossy(&out.stderr)))
    }
}

/// Maps git's (C locale) message to an error kind.
pub fn classify(stderr: &str) -> GitError {
    let s = stderr.to_ascii_lowercase();
    let has = |p: &str| s.contains(p);
    if has("not a git repository") {
        GitError::NotARepository
    } else if has("no tracking information") || has("no upstream") {
        GitError::NoUpstream
    } else if has("not currently on a branch") {
        GitError::Detached
    } else if has("would be overwritten") || has("please commit your changes or stash them") {
        GitError::LocalChanges
    } else if has("not possible to fast-forward")
        || has("diverging branches")
        || has("non-fast-forward")
    {
        GitError::NotFastForward
    } else if has("terminal prompts disabled")
        || has("could not read username")
        || has("could not read password")
        || has("authentication failed")
        || has("permission denied (publickey")
        || has("host key verification failed")
    {
        GitError::SignInNeeded
    } else if has("could not resolve host")
        || has("unable to access")
        || has("could not read from remote")
        || has("connection timed out")
        || has("connection refused")
    {
        GitError::Network
    } else if has("is already used by worktree") || has("is already checked out at") {
        GitError::CheckedOutElsewhere
    } else if has("already exists") {
        GitError::BranchExists
    } else if has("is not a valid branch name") || has("not a valid branch name") {
        GitError::InvalidBranchName
    } else if has("invalid reference") || has("did not match any") {
        GitError::BranchNotFound
    } else {
        let last = stderr
            .lines()
            .map(str::trim)
            .rfind(|l| !l.is_empty())
            .unwrap_or("git failed");
        GitError::Other(last.chars().take(200).collect())
    }
}

/// The folder row's git state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusSummary {
    /// Current branch; `None` when detached.
    pub branch: Option<String>,
    /// Whether the branch has an upstream.
    pub upstream: bool,
    /// Commits ahead of and behind the upstream, as of the last fetch.
    pub ahead: u32,
    pub behind: u32,
    /// Changed files: staged, unstaged, unmerged and untracked.
    pub changed: u32,
    /// The current commit, abbreviated to 7 characters ("detached at
    /// a1b2c3d"); `None` before the first commit.
    pub head: Option<String>,
}

/// `git status --porcelain=v2 --branch` within [`STATUS_LIMIT`]; `None`
/// without a `git` command, outside a repository or past the limit (ADR
/// 0011: then nothing is shown).
pub fn status_summary(dir: &Path) -> Option<StatusSummary> {
    let out = run_ok(dir, &["status", "--porcelain=v2", "--branch"], STATUS_LIMIT).ok()?;
    Some(parse_status(&out))
}

fn parse_status(out: &str) -> StatusSummary {
    let mut s = StatusSummary::default();
    for line in out.lines() {
        if let Some(oid) = line.strip_prefix("# branch.oid ") {
            s.head = (oid != "(initial)").then(|| oid.chars().take(7).collect());
        } else if let Some(h) = line.strip_prefix("# branch.head ") {
            s.branch = (h != "(detached)").then(|| h.to_owned());
        } else if line.starts_with("# branch.upstream ") {
            s.upstream = true;
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            for part in ab.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    s.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    s.behind = n.parse().unwrap_or(0);
                }
            }
        } else if !line.starts_with('#') && !line.is_empty() {
            s.changed += 1;
        }
    }
    s
}

/// `git fetch` of the current branch's remote, run only on demand; the
/// next [`status_summary`] then reports "behind N".
pub fn fetch(dir: &Path) -> Result<(), GitError> {
    run_ok(dir, &["fetch", "--quiet"], NETWORK_LIMIT).map(|_| ())
}

/// What a pull did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pulled {
    UpToDate,
    /// Fast-forwarded by this many commits.
    FastForwarded(u32),
}

/// `git pull --ff-only` on the current branch with its existing upstream.
/// A missing upstream, a detached HEAD, a non-fast-forward, local changes
/// in the way or a sign-in stop it with nothing changed.
pub fn pull_ff_only(dir: &Path) -> Result<Pulled, GitError> {
    let before = status_summary(dir).ok_or(GitError::NotARepository)?;
    if before.branch.is_none() {
        return Err(GitError::Detached);
    }
    if !before.upstream {
        return Err(GitError::NoUpstream);
    }
    let head =
        |d: &Path| run_ok(d, &["rev-parse", "HEAD"], LOCAL_LIMIT).map(|s| s.trim().to_owned());
    let old = head(dir)?;
    run_ok(
        dir,
        &["pull", "--ff-only", "--no-rebase", "--quiet"],
        NETWORK_LIMIT,
    )?;
    let new = head(dir)?;
    if old == new {
        return Ok(Pulled::UpToDate);
    }
    let n = run_ok(
        dir,
        &["rev-list", "--count", &format!("{old}..{new}")],
        LOCAL_LIMIT,
    )?;
    Ok(Pulled::FastForwarded(n.trim().parse().unwrap_or(0)))
}

/// The branch selector's list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Branches {
    /// The checked-out branch; `None` when detached.
    pub current: Option<String>,
    /// Local branches, sorted.
    pub local: Vec<String>,
    /// Remote branches with no local branch of the same name, such as
    /// `origin/feature` (shown as "track").
    pub remote: Vec<String>,
}

/// Local branches and the remote ones not yet tracked.
pub fn list_branches(dir: &Path) -> Result<Branches, GitError> {
    let out = run_ok(
        dir,
        &[
            "for-each-ref",
            "--format=%(HEAD) %(refname)",
            "refs/heads",
            "refs/remotes",
        ],
        LOCAL_LIMIT,
    )?;
    Ok(parse_branches(&out))
}

fn parse_branches(out: &str) -> Branches {
    let mut b = Branches::default();
    let mut remote = Vec::new();
    for line in out.lines() {
        let (mark, name) = line.split_at(line.len().min(2));
        if let Some(n) = name.strip_prefix("refs/heads/") {
            if mark.starts_with('*') {
                b.current = Some(n.to_owned());
            }
            b.local.push(n.to_owned());
        } else if let Some(n) = name.strip_prefix("refs/remotes/")
            && !n.ends_with("/HEAD")
        {
            remote.push(n.to_owned());
        }
    }
    b.remote = remote
        .into_iter()
        .filter(|r| {
            r.split_once('/')
                .is_some_and(|(_, short)| !b.local.iter().any(|l| l == short))
        })
        .collect();
    b.local.sort();
    b.remote.sort();
    b
}

/// What to switch to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwitchTo {
    /// A local branch.
    Local(String),
    /// A remote branch such as `origin/feature`: a local branch tracking it
    /// is created (`git switch --track`).
    Track(String),
}

/// A name is never taken for an option.
fn plain_name(name: &str) -> Result<&str, GitError> {
    if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_whitespace) {
        Err(GitError::InvalidBranchName)
    } else {
        Ok(name)
    }
}

/// `git switch <branch>` or `git switch --track <remote>/<branch>`. Local
/// changes git would refuse to carry stop it: no stash, no force.
pub fn switch_branch(dir: &Path, to: &SwitchTo) -> Result<(), GitError> {
    let args: Vec<&str> = match to {
        SwitchTo::Local(b) => vec!["switch", "--no-guess", plain_name(b)?],
        SwitchTo::Track(r) => vec!["switch", "--track", plain_name(r)?],
    };
    run_ok(dir, &args, LOCAL_LIMIT).map(|_| ())
}

/// "New branch...": `git switch -c <name>` from the current HEAD.
pub fn create_branch(dir: &Path, name: &str) -> Result<(), GitError> {
    let name = plain_name(name)?;
    let ok = run(dir, &["check-ref-format", "--branch", name], LOCAL_LIMIT)?;
    if !ok.status.success() {
        return Err(GitError::InvalidBranchName);
    }
    run_ok(dir, &["switch", "-c", name], LOCAL_LIMIT).map(|_| ())
}

/// The name git will commit under (`git config user.name`), for the folder
/// form's "Commits use your git identity: ...". Read only, never set.
pub fn user_name(dir: &Path) -> Option<String> {
    let out = run_ok(dir, &["config", "user.name"], LOCAL_LIMIT).ok()?;
    let name = out.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The value of `alias.<name>` in the repository's config, for the guard.
/// `Ok(None)` when there is none; an error (including past 2 s) means the
/// guard cannot read the call.
pub fn alias(dir: &Path, name: &str) -> Result<Option<String>, GitError> {
    if name.starts_with('-') || name.contains(char::is_whitespace) {
        return Ok(None);
    }
    let key = format!("alias.{name}");
    let out = run(dir, &["config", "--get", &key], ALIAS_LIMIT)?;
    match out.status.code() {
        Some(0) => {
            let v = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            Ok((!v.is_empty()).then_some(v))
        }
        // `git config --get`: 1 is "no such key".
        Some(1) => Ok(None),
        _ => Err(classify(&String::from_utf8_lossy(&out.stderr))),
    }
}

/// Whether `path` (relative to `dir`) is tracked by git.
pub fn is_tracked(dir: &Path, path: &str) -> Result<bool, GitError> {
    let out = run_ok(dir, &["ls-files", "--", path], LOCAL_LIMIT)?;
    Ok(!out.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn git(dir: &Path, args: &[&str]) {
        let out = command(dir).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A bare remote with one commit on `main`, and a clone of it.
    fn remote_and_clone(tmp: &Path) -> (PathBuf, PathBuf) {
        let bare = tmp.join("remote.git");
        let seed = tmp.join("seed");
        let clone = tmp.join("clone");
        fs::create_dir_all(&bare).unwrap();
        fs::create_dir_all(&seed).unwrap();
        git(&bare, &["init", "--bare", "-q", "-b", "main"]);
        git(&seed, &["init", "-q", "-b", "main"]);
        identity(&seed);
        fs::write(seed.join("a.txt"), "one\n").unwrap();
        git(&seed, &["add", "a.txt"]);
        git(&seed, &["commit", "-q", "-m", "First"]);
        git(&seed, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git(&seed, &["push", "-q", "-u", "origin", "main"]);
        git(tmp, &["clone", "-q", bare.to_str().unwrap(), "clone"]);
        identity(&clone);
        (seed, clone)
    }

    fn identity(dir: &Path) {
        git(dir, &["config", "user.name", "Alexandre"]);
        git(dir, &["config", "user.email", "alexandre@example.com"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
    }

    fn commit_in(dir: &Path, file: &str, text: &str) {
        fs::write(dir.join(file), text).unwrap();
        git(dir, &["add", file]);
        git(dir, &["commit", "-q", "-m", "Change"]);
    }

    #[test]
    fn status_summary_counts_changes_and_takes_no_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let (seed, clone) = remote_and_clone(tmp.path());
        let s = status_summary(&clone).unwrap();
        let head = run_ok(&clone, &["rev-parse", "--short=7", "HEAD"], LOCAL_LIMIT).unwrap();
        assert_eq!(
            s,
            StatusSummary {
                branch: Some("main".into()),
                upstream: true,
                ahead: 0,
                behind: 0,
                changed: 0,
                head: Some(head.trim().to_owned()),
            }
        );
        fs::write(clone.join("a.txt"), "two\n").unwrap();
        fs::write(clone.join("new.txt"), "x\n").unwrap();
        commit_in(&seed, "b.txt", "b\n");
        git(&seed, &["push", "-q"]);
        // An index.lock held by someone else does not stop a read.
        fs::write(clone.join(".git").join("index.lock"), "").unwrap();
        let s = status_summary(&clone).unwrap();
        assert_eq!(s.changed, 2);
        assert_eq!(s.behind, 0, "no fetch yet");
        fs::remove_file(clone.join(".git").join("index.lock")).unwrap();
        fetch(&clone).unwrap();
        assert_eq!(status_summary(&clone).unwrap().behind, 1);
        assert!(status_summary(&tmp.path().join("nowhere")).is_none());
    }

    #[test]
    fn pull_fast_forwards_or_stops_with_nothing_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let (seed, clone) = remote_and_clone(tmp.path());
        assert_eq!(pull_ff_only(&clone), Ok(Pulled::UpToDate));
        commit_in(&seed, "a.txt", "remote\n");
        git(&seed, &["push", "-q"]);
        // A local change to the same file is in the way.
        fs::write(clone.join("a.txt"), "local\n").unwrap();
        assert_eq!(pull_ff_only(&clone), Err(GitError::LocalChanges));
        assert_eq!(fs::read_to_string(clone.join("a.txt")).unwrap(), "local\n");
        git(&clone, &["checkout", "-q", "--", "a.txt"]);
        assert_eq!(pull_ff_only(&clone), Ok(Pulled::FastForwarded(1)));
        // Diverged: not a fast-forward, HEAD unchanged.
        commit_in(&seed, "s.txt", "s\n");
        git(&seed, &["push", "-q"]);
        commit_in(&clone, "c.txt", "c\n");
        let head = run_ok(&clone, &["rev-parse", "HEAD"], LOCAL_LIMIT).unwrap();
        assert_eq!(pull_ff_only(&clone), Err(GitError::NotFastForward));
        assert_eq!(
            run_ok(&clone, &["rev-parse", "HEAD"], LOCAL_LIMIT).unwrap(),
            head
        );
        // No upstream.
        git(&clone, &["switch", "-q", "-c", "solo"]);
        assert_eq!(pull_ff_only(&clone), Err(GitError::NoUpstream));
        // Detached.
        git(&clone, &["switch", "-q", "--detach"]);
        assert_eq!(pull_ff_only(&clone), Err(GitError::Detached));
    }

    #[test]
    fn branches_list_switch_and_create() {
        let tmp = tempfile::tempdir().unwrap();
        let (seed, clone) = remote_and_clone(tmp.path());
        git(&seed, &["switch", "-q", "-c", "feature"]);
        commit_in(&seed, "f.txt", "f\n");
        git(&seed, &["push", "-q", "-u", "origin", "feature"]);
        fetch(&clone).unwrap();
        let b = list_branches(&clone).unwrap();
        assert_eq!(b.current.as_deref(), Some("main"));
        assert_eq!(b.local, vec!["main"]);
        assert_eq!(b.remote, vec!["origin/feature"], "origin/main is tracked");

        switch_branch(&clone, &SwitchTo::Track("origin/feature".into())).unwrap();
        let b = list_branches(&clone).unwrap();
        assert_eq!(b.current.as_deref(), Some("feature"));
        assert!(b.remote.is_empty());

        // A change git would not carry: refused, nothing changed.
        fs::write(clone.join("f.txt"), "edited\n").unwrap();
        assert_eq!(
            switch_branch(&clone, &SwitchTo::Local("main".into())),
            Err(GitError::LocalChanges)
        );
        assert_eq!(
            list_branches(&clone).unwrap().current.as_deref(),
            Some("feature")
        );
        assert_eq!(fs::read_to_string(clone.join("f.txt")).unwrap(), "edited\n");
        git(&clone, &["checkout", "-q", "--", "f.txt"]);
        assert_eq!(
            switch_branch(&clone, &SwitchTo::Local("nope".into())),
            Err(GitError::BranchNotFound)
        );
        assert_eq!(
            switch_branch(&clone, &SwitchTo::Local("--force".into())),
            Err(GitError::InvalidBranchName)
        );
        switch_branch(&clone, &SwitchTo::Local("main".into())).unwrap();

        create_branch(&clone, "alexandre/try").unwrap();
        assert_eq!(
            list_branches(&clone).unwrap().current.as_deref(),
            Some("alexandre/try")
        );
        assert_eq!(create_branch(&clone, "main"), Err(GitError::BranchExists));
        assert_eq!(
            create_branch(&clone, "a..b"),
            Err(GitError::InvalidBranchName)
        );
        assert_eq!(
            create_branch(&clone, "-x"),
            Err(GitError::InvalidBranchName)
        );

        // A branch checked out in another worktree.
        let wt = tmp.path().join("wt");
        git(
            &clone,
            &["worktree", "add", "-q", wt.to_str().unwrap(), "feature"],
        );
        assert_eq!(
            switch_branch(&clone, &SwitchTo::Local("feature".into())),
            Err(GitError::CheckedOutElsewhere)
        );
        assert_eq!(user_name(&clone).as_deref(), Some("Alexandre"));
    }

    #[test]
    fn tracked_files_are_told_apart() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, clone) = remote_and_clone(tmp.path());
        assert!(is_tracked(&clone, "a.txt").unwrap());
        assert!(!is_tracked(&clone, ".claude/settings.local.json").unwrap());
    }

    #[test]
    fn a_repository_fsmonitor_command_never_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, clone) = remote_and_clone(tmp.path());
        // git runs a non-boolean `core.fsmonitor` as a hook, in the worktree.
        git(
            &clone,
            &["config", "core.fsmonitor", "echo ran > fsmonitor-ran"],
        );
        let hit = clone.join("fsmonitor-ran");
        assert!(is_tracked(&clone, "a.txt").unwrap());
        assert!(status_summary(&clone).is_some());
        assert!(!hit.exists(), "the repository's fsmonitor ran");
        // The guard's own config reads still work.
        git(&clone, &["config", "alias.ci", "commit"]);
        assert_eq!(alias(&clone, "ci").unwrap().as_deref(), Some("commit"));
        assert_eq!(alias(&clone, "nope").unwrap(), None);
        // A plain git, as a user's would, does run it: the test is real.
        let out = std::process::Command::new("git")
            .current_dir(&clone)
            .args(["status", "--porcelain"])
            .env_remove("GIT_DIR")
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(hit.exists(), "plain git did not run the fsmonitor hook");
    }

    #[test]
    fn porcelain_and_branch_parsing() {
        let s = parse_status("# branch.oid abc\n# branch.head (detached)\n1 .M N... a\n? b\n");
        assert_eq!(s.branch, None);
        assert_eq!(s.changed, 2);
        assert!(!s.upstream);
        let s =
            parse_status("# branch.head main\n# branch.upstream origin/main\n# branch.ab +3 -2\n");
        assert_eq!((s.ahead, s.behind, s.upstream), (3, 2, true));
        let b = parse_branches(
            "  refs/heads/b\n* refs/heads/a\n  refs/remotes/origin/HEAD\n  refs/remotes/origin/a\n  refs/remotes/up/c\n",
        );
        assert_eq!(b.current.as_deref(), Some("a"));
        assert_eq!(b.local, vec!["a", "b"]);
        assert_eq!(b.remote, vec!["up/c"]);
        assert!(
            matches!(classify("fatal: something odd\n"), GitError::Other(m) if m == "fatal: something odd")
        );
    }
}
