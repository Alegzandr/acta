//! Just enough git to bind a session (ADR 0005, "Binding"), read from the
//! files, never by spawning git: the worktree root, the main checkout that
//! all worktrees of a repository share, the current branch and the
//! default branch (ADR 0011, section 2).

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitInfo {
    /// Root of the worktree containing the directory.
    pub root: PathBuf,
    /// Root of the main checkout (equal to `root` outside linked worktrees);
    /// `None` for a bare or unusual layout.
    pub main_root: Option<PathBuf>,
    /// Current branch; `None` when detached.
    pub branch: Option<String>,
    /// The git directory shared by all worktrees (holds `info/exclude`).
    pub common_dir: PathBuf,
    /// The default branch: the target of `refs/remotes/origin/HEAD`, else
    /// `main` if that branch exists, else `master`, else `None`.
    pub default_branch: Option<String>,
}

impl GitInfo {
    /// Whether this is a linked worktree rather than the main checkout.
    pub fn is_linked_worktree(&self) -> bool {
        self.main_root.as_deref() != Some(self.root.as_path())
    }

    /// Whether HEAD is on the default branch.
    pub fn on_default_branch(&self) -> bool {
        self.branch.is_some() && self.branch == self.default_branch
    }
}

/// The git worktree containing `dir`, if any.
pub fn inspect(dir: &Path) -> Option<GitInfo> {
    for root in dir.ancestors() {
        let dot = root.join(".git");
        let Ok(meta) = fs::metadata(&dot) else {
            continue;
        };
        let (git_dir, common) = if meta.is_dir() {
            (dot.clone(), dot)
        } else {
            let text = fs::read_to_string(&dot).ok()?;
            let target = text.lines().next()?.strip_prefix("gitdir:")?.trim();
            let git_dir = resolve(root, target);
            let common = match fs::read_to_string(git_dir.join("commondir")) {
                Ok(c) => resolve(&git_dir, c.trim()),
                Err(_) => git_dir.clone(),
            };
            (git_dir, common)
        };
        let main_root = if common.file_name().is_some_and(|n| n == ".git") {
            common.parent().map(Path::to_path_buf)
        } else {
            None
        };
        let branch = fs::read_to_string(git_dir.join("HEAD"))
            .ok()
            .and_then(|h| h.trim().strip_prefix("ref: refs/heads/").map(str::to_owned));
        let default_branch = default_branch(&common);
        return Some(GitInfo {
            root: root.to_path_buf(),
            main_root,
            branch,
            common_dir: common,
            default_branch,
        });
    }
    None
}

/// ADR 0011, section 2: `origin/HEAD`'s target, else `main`, else `master`.
fn default_branch(common: &Path) -> Option<String> {
    if let Ok(head) = fs::read_to_string(
        common
            .join("refs")
            .join("remotes")
            .join("origin")
            .join("HEAD"),
    ) && let Some(b) = head.trim().strip_prefix("ref: refs/remotes/origin/")
        && !b.is_empty()
    {
        return Some(b.to_owned());
    }
    ["main", "master"]
        .into_iter()
        .find(|b| branch_exists(common, b))
        .map(str::to_owned)
}

/// A local branch, loose or packed.
fn branch_exists(common: &Path, name: &str) -> bool {
    let mut loose = common.join("refs").join("heads");
    for part in name.split('/') {
        loose.push(part);
    }
    if loose.is_file() {
        return true;
    }
    let wanted = format!("refs/heads/{name}");
    fs::read_to_string(common.join("packed-refs")).is_ok_and(|t| {
        t.lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with('^'))
            .any(|l| l.split_once(' ').is_some_and(|(_, r)| r.trim() == wanted))
    })
}

fn resolve(base: &Path, target: &str) -> PathBuf {
    let p = Path::new(target);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    fs::canonicalize(&joined).unwrap_or(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_checkout_and_linked_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("acta");
        let git = main.join(".git");
        fs::create_dir_all(git.join("worktrees").join("m3")).unwrap();
        fs::create_dir_all(main.join("crates")).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let info = inspect(&main.join("crates")).unwrap();
        assert_eq!(info.root, main);
        assert_eq!(info.main_root.as_deref(), Some(main.as_path()));
        assert_eq!(info.branch.as_deref(), Some("main"));

        let wt = tmp.path().join("wt").join("m3");
        fs::create_dir_all(&wt).unwrap();
        let wt_git = git.join("worktrees").join("m3");
        fs::write(wt.join(".git"), format!("gitdir: {}\n", wt_git.display())).unwrap();
        fs::write(wt_git.join("commondir"), "../..\n").unwrap();
        fs::write(
            wt_git.join("HEAD"),
            "ref: refs/heads/integration-engineer/m3\n",
        )
        .unwrap();
        let info = inspect(&wt).unwrap();
        assert_eq!(info.root, wt);
        assert_eq!(
            fs::canonicalize(info.main_root.as_ref().unwrap()).unwrap(),
            fs::canonicalize(&main).unwrap()
        );
        assert_eq!(info.branch.as_deref(), Some("integration-engineer/m3"));
        assert!(info.is_linked_worktree());
        // The worktree reads the shared refs: `main` exists loose.
        fs::create_dir_all(git.join("refs").join("heads")).unwrap();
        fs::write(
            git.join("refs").join("heads").join("main"),
            "0123
",
        )
        .unwrap();
        assert_eq!(
            inspect(&wt).unwrap().default_branch.as_deref(),
            Some("main")
        );
    }

    #[test]
    fn default_branch_from_origin_head_then_main_then_master() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("r");
        let git = repo.join(".git");
        fs::create_dir_all(git.join("refs").join("heads")).unwrap();
        fs::write(
            git.join("HEAD"),
            "ref: refs/heads/topic
",
        )
        .unwrap();
        assert_eq!(inspect(&repo).unwrap().default_branch, None);
        fs::write(
            git.join("packed-refs"),
            "# pack-refs with: peeled
0123 refs/heads/master
",
        )
        .unwrap();
        assert_eq!(
            inspect(&repo).unwrap().default_branch.as_deref(),
            Some("master")
        );
        fs::write(
            git.join("refs").join("heads").join("main"),
            "0123
",
        )
        .unwrap();
        let info = inspect(&repo).unwrap();
        assert_eq!(info.default_branch.as_deref(), Some("main"));
        assert!(!info.on_default_branch());
        let origin = git.join("refs").join("remotes").join("origin");
        fs::create_dir_all(&origin).unwrap();
        fs::write(
            origin.join("HEAD"),
            "ref: refs/remotes/origin/topic
",
        )
        .unwrap();
        let info = inspect(&repo).unwrap();
        assert_eq!(info.default_branch.as_deref(), Some("topic"));
        assert!(info.on_default_branch());
        assert!(!info.is_linked_worktree());
    }

    #[test]
    fn detached_head_and_no_repository() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("r").join(".git")).unwrap();
        fs::write(tmp.path().join("r").join(".git").join("HEAD"), "0123abcd\n").unwrap();
        assert_eq!(inspect(&tmp.path().join("r")).unwrap().branch, None);
        fs::create_dir_all(tmp.path().join("plain")).unwrap();
        // The temp dir may itself sit in a repository; only assert the root
        // is not the plain folder.
        assert!(
            inspect(&tmp.path().join("plain")).is_none_or(|i| i.root != tmp.path().join("plain"))
        );
    }
}
