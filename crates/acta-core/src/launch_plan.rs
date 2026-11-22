//! Where Claude Code starts and what else it opens (ADR 0011, section 1):
//! the active folder is the working directory, every other folder the page
//! sees is an `--add-dir`, except a folder not found and an inherited
//! folder that is another checkout of the same git repository as the
//! active folder (same git common directory, different worktree root).
//!
//! Pure over its inputs plus file reads (`agent::git::inspect`, existence):
//! the caller passes the page's visible folders in order (own, then the
//! epic's, then the project's) with the active one marked.

use crate::agent::git;
use crate::folder_path;
use crate::paths::Os;
use std::fs;
use std::path::{Path, PathBuf};

/// One folder the page sees, in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanFolder {
    pub path: PathBuf,
    /// The page's own folder (never left out as another checkout).
    pub own: bool,
    /// The page's active folder.
    pub active: bool,
}

/// Why a visible folder is not passed to Claude Code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftOut {
    NotFound,
    /// Another checkout of the active folder's repository.
    OtherCheckout,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchPlan {
    /// The active folder; `None` when there is none or it is not found.
    pub working_dir: Option<PathBuf>,
    /// The `--add-dir` arguments, in order.
    pub add_dirs: Vec<PathBuf>,
    /// The folders left out, with their reason, in order.
    pub left_out: Vec<(PathBuf, LeftOut)>,
}

/// The repository identity of a folder: canonical common dir and root.
fn checkout(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let info = git::inspect(dir)?;
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    Some((canon(&info.common_dir), canon(&info.root)))
}

pub fn launch_plan(folders: &[PlanFolder]) -> LaunchPlan {
    let mut plan = LaunchPlan::default();
    let active = folders.iter().find(|f| f.active);
    if let Some(a) = active {
        if a.path.is_dir() {
            plan.working_dir = Some(a.path.clone());
        } else {
            plan.left_out.push((a.path.clone(), LeftOut::NotFound));
        }
    }
    let active_checkout = plan.working_dir.as_deref().and_then(checkout);
    let mut seen: Vec<String> = active
        .map(|a| vec![a.path.to_string_lossy().into_owned()])
        .unwrap_or_default();
    for f in folders.iter().filter(|f| !f.active) {
        let text = f.path.to_string_lossy();
        if seen
            .iter()
            .any(|s| folder_path::same(Os::CURRENT, s, &text))
        {
            continue;
        }
        seen.push(text.into_owned());
        if !f.path.is_dir() {
            plan.left_out.push((f.path.clone(), LeftOut::NotFound));
            continue;
        }
        if !f.own
            && let Some((common, root)) = &active_checkout
            && let Some((c, r)) = checkout(&f.path)
            && &c == common
            && &r != root
        {
            plan.left_out.push((f.path.clone(), LeftOut::OtherCheckout));
            continue;
        }
        plan.add_dirs.push(f.path.clone());
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let out = crate::git_cmd::command(dir).args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn repo(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(
            dir,
            &[
                "-c",
                "user.name=Alexandre",
                "-c",
                "user.email=alexandre@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "First",
            ],
        );
    }

    fn f(path: &Path, own: bool, active: bool) -> PlanFolder {
        PlanFolder {
            path: path.to_path_buf(),
            own,
            active,
        }
    }

    #[test]
    fn worktree_topic_leaves_the_main_checkout_out() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("acta");
        repo(&main);
        let package = main.join("crates").join("core");
        fs::create_dir_all(&package).unwrap();
        let wt = tmp.path().join("wt").join("m3");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "m3", wt.to_str().unwrap()],
        );
        let wt_package = wt.join("crates");
        fs::create_dir_all(&wt_package).unwrap();
        let docs = tmp.path().join("acta-docs");
        repo(&docs);
        let plain = tmp.path().join("notes");
        fs::create_dir_all(&plain).unwrap();
        let gone = tmp.path().join("gone");

        // Topic: its own worktree (active); inherited: the main checkout, a
        // package inside the main checkout, a separate repository, a plain
        // folder, a folder not found, and a package inside the worktree.
        let plan = launch_plan(&[
            f(&wt, true, true),
            f(&main, false, false),
            f(&package, false, false),
            f(&docs, false, false),
            f(&plain, false, false),
            f(&gone, false, false),
            f(&wt_package, false, false),
        ]);
        assert_eq!(plan.working_dir.as_deref(), Some(wt.as_path()));
        assert_eq!(plan.add_dirs, vec![docs.clone(), plain.clone(), wt_package]);
        assert_eq!(
            plan.left_out,
            vec![
                (main.clone(), LeftOut::OtherCheckout),
                (package.clone(), LeftOut::OtherCheckout),
                (gone.clone(), LeftOut::NotFound),
            ]
        );

        // Own folders are never left out: two worktrees on one topic.
        let plan = launch_plan(&[f(&wt, true, true), f(&main, true, false)]);
        assert_eq!(plan.add_dirs, vec![main.clone()]);
        assert!(plan.left_out.is_empty());

        // Active in the main checkout: a package inside it and the root
        // containing an active package are kept; the worktree is left out.
        let plan = launch_plan(&[
            f(&package, true, true),
            f(&main, false, false),
            f(&wt, false, false),
        ]);
        assert_eq!(plan.working_dir.as_deref(), Some(package.as_path()));
        assert_eq!(plan.add_dirs, vec![main.clone()]);
        assert_eq!(plan.left_out, vec![(wt.clone(), LeftOut::OtherCheckout)]);

        // Active not found: no working directory, nothing compared.
        let plan = launch_plan(&[f(&gone, true, true), f(&main, false, false)]);
        assert_eq!(plan.working_dir, None);
        assert_eq!(plan.add_dirs, vec![main.clone()]);
        assert_eq!(plan.left_out, vec![(gone, LeftOut::NotFound)]);

        // A path seen twice is passed once; the active one is not repeated.
        let plan = launch_plan(&[
            f(&docs, true, true),
            f(&plain, true, false),
            f(&plain, false, false),
            f(&docs, false, false),
        ]);
        assert_eq!(plan.add_dirs, vec![plain]);
    }
}
