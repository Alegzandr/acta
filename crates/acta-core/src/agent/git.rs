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
    /// Current branch; `None` when detached or not known.
    pub branch: Option<String>,
    /// Whether HEAD could be read: `false` when the file is missing or
    /// unreadable, or in a reftable repository (HEAD names `.invalid`).
    pub head_known: bool,
    /// The git directory shared by all worktrees (holds `info/exclude`).
    pub common_dir: PathBuf,
    /// The default branch: the target of `refs/remotes/origin/HEAD`, else
    /// `main` if that branch exists, else `master`, else `None`.
    pub default_branch: Option<String>,
    /// The branch the current branch merges from (`branch.<name>.merge` in
    /// the repository's config), short form.
    pub upstream_branch: Option<String>,
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
        let head = fs::read_to_string(git_dir.join("HEAD")).ok();
        let head = head.as_deref().map(str::trim);
        let named = head.and_then(|h| h.strip_prefix("ref: refs/heads/"));
        let head_known = head.is_some_and(|h| !h.is_empty()) && named != Some(".invalid");
        let branch = named.filter(|_| head_known).map(str::to_owned);
        let default_branch = default_branch(&common);
        let upstream_branch = branch.as_deref().and_then(|b| upstream(&common, b));
        return Some(GitInfo {
            root: root.to_path_buf(),
            main_root,
            branch,
            head_known,
            common_dir: common,
            default_branch,
            upstream_branch,
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

/// `branch.<name>.merge` from the repository's config, without
/// `refs/heads/`. Sections are matched exactly, keys without case.
fn upstream(common: &Path, branch: &str) -> Option<String> {
    let text = fs::read_to_string(common.join("config")).ok()?;
    let wanted = format!("[branch \"{branch}\"]");
    let mut inside = false;
    let mut found = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == wanted;
        } else if inside
            && let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case("merge")
        {
            let v = v.trim().trim_matches('"');
            found = Some(v.strip_prefix("refs/heads/").unwrap_or(v).to_owned());
        }
    }
    found
}

/// The commit a local branch points at, loose or packed (ADR 0015, 4:
/// the flags of branch memories, read from the files).
pub fn branch_oid(common: &Path, name: &str) -> Option<String> {
    let mut loose = common.join("refs").join("heads");
    for part in name.split('/') {
        loose.push(part);
    }
    if let Ok(t) = fs::read_to_string(&loose) {
        return Some(t.trim().to_owned()).filter(|t| !t.is_empty());
    }
    let wanted = format!("refs/heads/{name}");
    fs::read_to_string(common.join("packed-refs"))
        .ok()?
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with('^'))
        .find_map(|l| {
            let (oid, r) = l.split_once(' ')?;
            (r.trim() == wanted).then(|| oid.to_owned())
        })
}

/// The commit `HEAD` of the worktree containing `dir` names (ADR 0015, 1:
/// file flags are cached per folder and `HEAD`), read from the files.
pub fn head_oid(dir: &Path) -> Option<String> {
    let info = inspect(dir)?;
    let dot = info.root.join(".git");
    let git_dir = if dot.is_dir() {
        dot
    } else {
        let text = fs::read_to_string(&dot).ok()?;
        resolve(
            &info.root,
            text.lines().next()?.strip_prefix("gitdir:")?.trim(),
        )
    };
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: refs/heads/") {
        Some(b) => branch_oid(&info.common_dir, b),
        None => Some(head.to_owned()).filter(|h| h.len() >= 40),
    }
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
    fn reftable_head_is_unknown_and_upstream_is_read() {
        let tmp = tempfile::tempdir().unwrap();
        let git = tmp.path().join("r").join(".git");
        fs::create_dir_all(&git).unwrap();
        fs::write(
            git.join("HEAD"),
            "ref: refs/heads/.invalid
",
        )
        .unwrap();
        let info = inspect(&tmp.path().join("r")).unwrap();
        assert!(!info.head_known);
        assert_eq!(info.branch, None);
        fs::write(
            git.join("HEAD"),
            "ref: refs/heads/feature
",
        )
        .unwrap();
        fs::write(
            git.join("config"),
            "[core]
	bare = false
[branch \"feature\"]
	remote = origin
	Merge = refs/heads/main
[branch \"other\"]
	merge = refs/heads/x
",
        )
        .unwrap();
        let info = inspect(&tmp.path().join("r")).unwrap();
        assert!(info.head_known);
        assert_eq!(info.upstream_branch.as_deref(), Some("main"));
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
