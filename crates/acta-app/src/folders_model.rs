//! Working folders (docs/design/components-folders.md 5; ADR 0011): the
//! view model of the folder row, the list, the switcher and the terminal
//! header, pure so it is tested without a window; and repairing (Repairing
//! the rest): after one Locate, the part of the path that changed is taken
//! as the old and the new root, and other folders that were not found and
//! exist under the new root are offered the same change.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use acta_core::agent::git;
use acta_core::folder_path;
use acta_core::git_cmd::{GitError, StatusSummary};
use acta_core::launch_plan::{LeftOut, PlanFolder, launch_plan};
use acta_core::model::{CommitMode, FolderId, Owner};
use acta_core::paths::Os;

use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::Icon;

/// One folder a page sees, in list order (own, then the epic's, then the
/// project's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderItem {
    pub id: FolderId,
    /// As stored.
    pub path: String,
    /// The name shown: the user's, else the last segment of the path.
    pub name: String,
    /// The name as set: empty follows the path.
    pub own_name: String,
    /// One of the twelve icon names, or empty for automatic.
    pub icon: String,
    /// The level the folder belongs to.
    pub origin: Owner,
    /// The page's own folder (editable here).
    pub own: bool,
    pub active: bool,
    pub commit_mode: CommitMode,
    pub commit_as_user: Option<bool>,
    pub commit_on_default: bool,
}

/// What the files say about a folder's checkout (no process is run).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkout {
    /// A linked git worktree, not the main checkout.
    pub worktree: bool,
    pub branch: Option<String>,
    pub default_branch: Option<String>,
}

/// What the disk says about a folder, read off the UI thread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Disk {
    /// `None` outside a git repository.
    pub git: Option<Checkout>,
    /// Another checkout of the active folder's repository, inherited: left
    /// out of `--add-dir` (ADR 0011, 1).
    pub other_checkout: bool,
}

/// What the disk says about each folder, by stored path: its checkout
/// (from the files, no process) and whether it is another checkout of
/// the active folder's repository, by the launch plan's own rule. Reads
/// the disk: call it off the UI thread.
pub fn read_disk(items: &[FolderItem]) -> HashMap<String, Disk> {
    let plan = launch_plan(
        &items
            .iter()
            .map(|f| PlanFolder {
                path: PathBuf::from(&f.path),
                own: f.own,
                active: f.active,
            })
            .collect::<Vec<_>>(),
    );
    items
        .iter()
        .map(|f| {
            let path = Path::new(&f.path);
            let git = git::inspect(path).map(|g| Checkout {
                worktree: g.is_linked_worktree(),
                branch: g.branch.clone(),
                default_branch: g.default_branch.clone(),
            });
            let other_checkout = plan
                .left_out
                .iter()
                .any(|(p, why)| *why == LeftOut::OtherCheckout && p == path);
            (
                f.path.clone(),
                Disk {
                    git,
                    other_checkout,
                },
            )
        })
        .collect()
}

/// The twelve icons a folder may be given, with their names (catalogue
/// `folders.icon.*`), in the order of the settings form.
pub const ICONS: [(&str, Icon, Key); 12] = [
    ("folder-open", Icon::FolderOpen, Key::FoldersIconFolder),
    ("folder-git-2", Icon::FolderGit2, Key::FoldersIconRepo),
    ("git-branch", Icon::GitBranch, Key::FoldersIconWorktree),
    ("code", Icon::Code, Key::FoldersIconCode),
    ("book-open", Icon::BookOpen, Key::FoldersIconDocs),
    ("package", Icon::Package, Key::FoldersIconPackage),
    ("server", Icon::Server, Key::FoldersIconServer),
    ("globe", Icon::Globe, Key::FoldersIconWeb),
    ("smartphone", Icon::Smartphone, Key::FoldersIconMobile),
    ("database", Icon::Database, Key::FoldersIconData),
    ("flask-conical", Icon::FlaskConical, Key::FoldersIconLab),
    ("palette", Icon::Palette, Key::FoldersIconDesign),
];

/// The icon detection gives: a branch for a linked worktree, a git folder
/// for a repository, a folder otherwise.
pub fn automatic_icon(git: Option<&Checkout>) -> Icon {
    match git {
        Some(c) if c.worktree => Icon::GitBranch,
        Some(_) => Icon::FolderGit2,
        None => Icon::FolderOpen,
    }
}

/// The icon a folder shows: `folder-x` while it is not found, else its
/// own, else the automatic one.
pub fn shown_icon(setting: &str, git: Option<&Checkout>, missing: bool) -> Icon {
    if missing {
        return Icon::FolderX;
    }
    ICONS
        .iter()
        .find(|(name, _, _)| *name == setting)
        .map(|(_, icon, _)| *icon)
        .unwrap_or_else(|| automatic_icon(git))
}

/// The groups of the list and the switcher: each level's folders, in list
/// order. The page's own group always comes first, even empty (it then
/// reads "No folders of its own."); another level without folders is
/// absent.
pub fn groups(page: Owner, items: &[FolderItem]) -> Vec<(Owner, Vec<usize>)> {
    let mut out: Vec<(Owner, Vec<usize>)> = vec![(page, Vec::new())];
    for (i, item) in items.iter().enumerate() {
        match out.iter_mut().find(|(o, _)| *o == item.origin) {
            Some((_, list)) => list.push(i),
            None => out.push((item.origin, vec![i])),
        }
    }
    out
}

/// A group's line: "This topic", "From epic Terminal", "From project Acta".
pub fn group_label(locale: Locale, page: Owner, origin: Owner, name: &str) -> String {
    if origin == page {
        let key = match page {
            Owner::Topic(_) => Key::FoldersGroupTopic,
            Owner::Epic(_) => Key::FoldersGroupEpic,
            Owner::Project(_) => Key::FoldersGroupProject,
        };
        return text(locale, key);
    }
    let key = match origin {
        Owner::Project(_) => Key::FoldersGroupFromProject,
        _ => Key::FoldersGroupFromEpic,
    };
    tr(locale, key, &[("name", Arg::Text(name))])
}

/// The git state of a folder as the row shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitLine {
    /// The branch, or the short commit of a detached head.
    pub branch: String,
    pub detached: bool,
    pub changed: u32,
    /// Behind the upstream as of the last fetch.
    pub behind: u32,
    /// The branch has an upstream: Pull is offered.
    pub upstream: bool,
    /// An automatic commit mode: the `git-commit-horizontal` mark.
    pub auto: Option<CommitMode>,
}

/// The git state, or `None` when there is nothing to show: outside git, a
/// state not known yet, or no `git` command (ADR 0011, 2: never
/// "Checking...").
pub fn git_line(
    disk: Option<&Disk>,
    status: Option<&StatusSummary>,
    mode: CommitMode,
) -> Option<GitLine> {
    let checkout = disk?.git.as_ref()?;
    let branch = status
        .and_then(|s| s.branch.clone())
        .or_else(|| checkout.branch.clone());
    let (branch, detached) = match branch {
        Some(b) => (b, false),
        None => (status?.head.clone()?, true),
    };
    Some(GitLine {
        branch,
        detached,
        changed: status.map_or(0, |s| s.changed),
        behind: status.map_or(0, |s| s.behind),
        upstream: status.is_some_and(|s| s.upstream),
        auto: mode.automatic().then_some(mode),
    })
}

/// "topic/embed", or "detached at a1b2c3d".
pub fn branch_text(locale: Locale, line: &GitLine) -> String {
    if line.detached {
        tr(
            locale,
            Key::FoldersGitDetached,
            &[("commit", Arg::Text(&line.branch))],
        )
    } else {
        line.branch.clone()
    }
}

/// The tooltip of the automatic-commit mark.
pub fn auto_tip(mode: CommitMode) -> Option<Key> {
    match mode {
        CommitMode::Manual => None,
        CommitMode::Commit => Some(Key::FoldersGitAutoCommit),
        CommitMode::Push => Some(Key::FoldersGitAutoPush),
    }
}

/// The line under the commit-mode control, describing the selected mode.
pub fn mode_hint(mode: CommitMode) -> Key {
    match mode {
        CommitMode::Manual => Key::FoldersCommitManualHint,
        CommitMode::Commit => Key::FoldersCommitCommitHint,
        CommitMode::Push => Key::FoldersCommitPushHint,
    }
}

/// What a failed git action says (a toast); `branch` and `folder` name
/// what it acted on.
pub fn git_error_text(locale: Locale, err: &GitError, branch: &str, folder: &str) -> String {
    let b = [("branch", Arg::Text(branch))];
    let other = |message: &str| {
        tr(
            locale,
            Key::FoldersGitErrOther,
            &[("message", Arg::Text(message))],
        )
    };
    match err {
        GitError::NoGit => text(locale, Key::FoldersGitErrNoGit),
        GitError::TimedOut => text(locale, Key::FoldersGitErrTimedOut),
        GitError::NoUpstream => tr(locale, Key::FoldersGitErrNoUpstream, &b),
        GitError::Detached => text(locale, Key::FoldersGitErrDetached),
        GitError::NotFastForward => tr(locale, Key::FoldersGitErrDiverged, &b),
        GitError::LocalChanges => tr(
            locale,
            Key::FoldersGitErrLocalChanges,
            &[("folder", Arg::Text(folder))],
        ),
        GitError::SignInNeeded => text(locale, Key::FoldersGitErrSignIn),
        GitError::Network => text(locale, Key::FoldersGitErrNetwork),
        GitError::BranchExists => tr(locale, Key::FoldersGitErrExists, &b),
        GitError::InvalidBranchName => tr(locale, Key::FoldersGitErrInvalidName, &b),
        GitError::BranchNotFound => tr(locale, Key::FoldersGitErrNoBranch, &b),
        GitError::CheckedOutElsewhere => tr(locale, Key::FoldersGitErrElsewhere, &b),
        GitError::NotARepository => other(&err.to_string()),
        GitError::Other(message) => other(message),
    }
}

/// Whether a git action in `folder` touches files a session launched with
/// `dir` (its working directory or an `--add-dir` folder) can see: the
/// same folder, one inside the other.
pub fn overlaps(dir: &str, folder: &str) -> bool {
    folder_path::contains(Os::CURRENT, dir, folder).is_some()
        || folder_path::contains(Os::CURRENT, folder, dir).is_some()
}

/// What the terminal header offers when the session is on something else
/// (layout.md 4.5, Mismatch).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mismatch {
    None,
    /// Only the session page's active folder changed: "Restart in <name>".
    RestartIn(String),
    /// Started from another page, or the commit mode of its folder changed.
    RestartHere,
}

/// The mismatch of a running session: `other_page` when it was started
/// from another page than the active one; `running_in` the folder it runs
/// in; `page_active` its page's active folder now (path, name);
/// `mode_changed` when the commit settings of the folder it runs in
/// changed since it started.
pub fn mismatch(
    other_page: bool,
    running_in: &str,
    page_active: Option<(&str, &str)>,
    mode_changed: bool,
) -> Mismatch {
    if other_page {
        return Mismatch::RestartHere;
    }
    if let Some((path, name)) = page_active
        && !folder_path::same(Os::CURRENT, path, running_in)
    {
        return Mismatch::RestartIn(name.to_owned());
    }
    if mode_changed {
        return Mismatch::RestartHere;
    }
    Mismatch::None
}

/// The roots of a relocation: `D:\work\acta` located at
/// `C:\Users\Alexandre\dev\acta` gives `D:\work` and `C:\Users\Alexandre\dev`
/// (the longest common trailing segments are dropped). `None` when nothing
/// but the last segments differ in a way that gives no root (one path is a
/// suffix of the other).
pub fn roots(old: &str, new: &str) -> Option<(String, String)> {
    let split =
        |p: &str| -> Vec<String> { p.split(['/', '\\']).map(str::to_owned).collect::<Vec<_>>() };
    let old_parts = split(old);
    let new_parts = split(new);
    let mut common = 0;
    while common < old_parts.len().saturating_sub(1)
        && common < new_parts.len().saturating_sub(1)
        && old_parts[old_parts.len() - 1 - common]
            .eq_ignore_ascii_case(&new_parts[new_parts.len() - 1 - common])
    {
        common += 1;
    }
    let old_root = rejoin(old, old_parts.len() - common);
    let new_root = rejoin(new, new_parts.len() - common);
    if old_root.is_empty() || new_root.is_empty() || old_root == new_root {
        return None;
    }
    Some((old_root, new_root))
}

/// The first `segments` segments of `path`, with its own separators.
fn rejoin(path: &str, segments: usize) -> String {
    let mut seen = 0;
    for (i, c) in path.char_indices() {
        if c == '/' || c == '\\' {
            seen += 1;
            if seen == segments {
                return path[..i].to_owned();
            }
        }
    }
    path.to_owned()
}

/// The folders the same change would resolve: not found now, under
/// `old_root`, and existing under `new_root`, as `(id, path, new path)`.
/// `exists` checks the disk, so call this off the UI thread and never
/// while holding the store.
pub fn candidates<Id: Copy>(
    folders: &[(Id, String)],
    old_root: &str,
    new_root: &str,
    exists: impl Fn(&str) -> bool,
) -> Vec<(Id, String, String)> {
    let os = Os::CURRENT;
    let old_root = folder_path::normalize(os, old_root);
    let sep = if new_root.contains('\\') { "\\" } else { "/" };
    folders
        .iter()
        .filter(|(_, path)| folder_path::contains(os, &old_root, path).is_some())
        .filter(|(_, path)| !exists(path))
        .filter_map(|(id, path)| {
            let rest = path
                .get(old_root.len()..)
                .unwrap_or("")
                .trim_start_matches(['/', '\\']);
            let candidate = if rest.is_empty() {
                new_root.to_owned()
            } else {
                format!("{new_root}{sep}{rest}")
            };
            exists(&candidate).then(|| (*id, path.clone(), candidate))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{EpicId, ProjectId, TopicId};

    fn item(id: i64, origin: Owner, own: bool, active: bool) -> FolderItem {
        FolderItem {
            id: FolderId(id),
            path: format!("/dev/f{id}"),
            name: format!("f{id}"),
            own_name: String::new(),
            icon: String::new(),
            origin,
            own,
            active,
            commit_mode: CommitMode::Manual,
            commit_as_user: None,
            commit_on_default: false,
        }
    }

    #[test]
    fn the_own_group_comes_first_even_empty_and_levels_follow_in_order() {
        let topic = Owner::Topic(TopicId(1));
        let epic = Owner::Epic(EpicId(2));
        let project = Owner::Project(ProjectId(3));
        let items = [
            item(1, epic, false, true),
            item(2, project, false, false),
            item(3, project, false, false),
        ];
        assert_eq!(
            groups(topic, &items),
            vec![(topic, vec![]), (epic, vec![0]), (project, vec![1, 2])]
        );
        let items = [item(1, topic, true, true), item(2, project, false, false)];
        assert_eq!(
            groups(topic, &items),
            vec![(topic, vec![0]), (project, vec![1])]
        );
        assert_eq!(group_label(Locale::En, topic, topic, "Embed"), "This topic");
        assert_eq!(
            group_label(Locale::En, topic, epic, "Terminal"),
            "From epic Terminal"
        );
        assert_eq!(
            group_label(Locale::Fr, topic, project, "Acta"),
            "Du projet Acta"
        );
    }

    #[test]
    fn icons_follow_the_setting_then_detection_and_missing_wins() {
        let repo = Checkout::default();
        let worktree = Checkout {
            worktree: true,
            ..Checkout::default()
        };
        assert_eq!(shown_icon("", None, false), Icon::FolderOpen);
        assert_eq!(shown_icon("", Some(&repo), false), Icon::FolderGit2);
        assert_eq!(shown_icon("", Some(&worktree), false), Icon::GitBranch);
        assert_eq!(shown_icon("book-open", Some(&repo), false), Icon::BookOpen);
        assert_eq!(shown_icon("book-open", None, true), Icon::FolderX);
        // An unknown name (a newer build's) falls back to automatic.
        assert_eq!(shown_icon("rocket", None, false), Icon::FolderOpen);
        let names: Vec<&str> = ICONS.iter().map(|(n, _, _)| *n).collect();
        assert_eq!(names, acta_core::model::FOLDER_ICONS);
    }

    #[test]
    fn the_git_line_shows_only_what_is_known() {
        let in_git = Disk {
            git: Some(Checkout {
                branch: Some("topic/embed".into()),
                ..Checkout::default()
            }),
            other_checkout: false,
        };
        assert_eq!(git_line(None, None, CommitMode::Manual), None);
        assert_eq!(
            git_line(Some(&Disk::default()), None, CommitMode::Manual),
            None,
            "outside git: nothing"
        );
        // From the files alone: the branch, nothing else yet.
        let line = git_line(Some(&in_git), None, CommitMode::Commit).unwrap();
        assert_eq!(line.branch, "topic/embed");
        assert_eq!((line.changed, line.behind, line.upstream), (0, 0, false));
        assert_eq!(line.auto, Some(CommitMode::Commit));
        // The status wins once it arrives.
        let status = StatusSummary {
            branch: Some("main".into()),
            upstream: true,
            behind: 2,
            changed: 3,
            ..StatusSummary::default()
        };
        let line = git_line(Some(&in_git), Some(&status), CommitMode::Manual).unwrap();
        assert_eq!(
            (
                line.branch.as_str(),
                line.changed,
                line.behind,
                line.upstream
            ),
            ("main", 3, 2, true)
        );
        assert_eq!(line.auto, None);
        // Detached: the short commit, once the status names it.
        let detached = Disk {
            git: Some(Checkout::default()),
            other_checkout: false,
        };
        assert_eq!(git_line(Some(&detached), None, CommitMode::Manual), None);
        let status = StatusSummary {
            head: Some("a1b2c3d".into()),
            ..StatusSummary::default()
        };
        let line = git_line(Some(&detached), Some(&status), CommitMode::Manual).unwrap();
        assert!(line.detached);
        assert_eq!(branch_text(Locale::En, &line), "detached at a1b2c3d");
    }

    #[test]
    fn the_header_offers_restart_in_or_here_by_what_changed() {
        let here = "/dev/acta-wt/embed";
        let other = Some(("/dev/acta-docs", "acta-docs"));
        let same = Some((here, "embed"));
        assert_eq!(mismatch(false, here, same, false), Mismatch::None);
        assert_eq!(
            mismatch(false, here, other, false),
            Mismatch::RestartIn("acta-docs".into()),
            "only the page's active folder changed"
        );
        assert_eq!(
            mismatch(false, here, same, true),
            Mismatch::RestartHere,
            "the commit mode changed: its instruction is out of date"
        );
        assert_eq!(mismatch(true, here, other, false), Mismatch::RestartHere);
        assert_eq!(mismatch(true, here, same, false), Mismatch::RestartHere);
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = acta_core::git_cmd::command(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn the_disk_tells_a_worktree_and_the_other_checkout_of_its_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("acta");
        std::fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q", "-b", "main"]);
        git(
            &main,
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
        let wt = tmp.path().join("embed");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "topic/embed",
                wt.to_str().unwrap(),
            ],
        );
        let plain = tmp.path().join("notes");
        std::fs::create_dir_all(&plain).unwrap();
        let topic = Owner::Topic(TopicId(1));
        let project = Owner::Project(ProjectId(3));
        let mut a = item(1, topic, true, true);
        a.path = wt.to_string_lossy().into_owned();
        let mut b = item(2, topic, true, false);
        b.path = plain.to_string_lossy().into_owned();
        let mut c = item(3, project, false, false);
        c.path = main.to_string_lossy().into_owned();
        let disk = read_disk(&[a.clone(), b.clone(), c.clone()]);
        let wt_git = disk[&a.path].git.as_ref().unwrap();
        assert!(wt_git.worktree);
        assert_eq!(wt_git.branch.as_deref(), Some("topic/embed"));
        assert_eq!(wt_git.default_branch.as_deref(), Some("main"));
        assert!(!disk[&a.path].other_checkout);
        assert!(disk[&b.path].git.is_none(), "a plain folder");
        assert!(
            disk[&c.path].other_checkout,
            "the inherited main checkout is left out here"
        );
        assert!(!disk[&c.path].git.as_ref().unwrap().worktree);
        assert_eq!(
            shown_icon("", disk[&a.path].git.as_ref(), false),
            Icon::GitBranch
        );
    }

    #[test]
    fn a_session_sees_its_working_directory_its_add_dirs_and_what_is_inside() {
        let sep = std::path::MAIN_SEPARATOR;
        let root = if cfg!(windows) { "C:" } else { "" };
        let p = |s: &str| format!("{root}{sep}{}", s.replace('/', &sep.to_string()));
        let dirs = [p("dev/acta-wt/embed"), p("dev/alacritty-fork")];
        let busy = |folder: &str| dirs.iter().any(|d| overlaps(d, folder));
        assert!(busy(&p("dev/acta-wt/embed")), "the working directory");
        assert!(busy(&p("dev/alacritty-fork")), "an --add-dir folder");
        assert!(busy(&p("dev/alacritty-fork/crates")), "inside an --add-dir");
        assert!(busy(&p("dev")), "containing one");
        assert!(!busy(&p("dev/acta")), "a sibling is not seen");
    }

    #[test]
    fn git_refusals_name_what_they_stopped() {
        let t = |e: GitError| git_error_text(Locale::En, &e, "main", "acta");
        assert!(t(GitError::NotFastForward).starts_with("main and its upstream have diverged"));
        assert!(t(GitError::LocalChanges).contains("Local changes in acta"));
        assert!(t(GitError::Other("fatal: x".into())).ends_with("fatal: x"));
        for e in [
            GitError::NoGit,
            GitError::TimedOut,
            GitError::NoUpstream,
            GitError::Detached,
            GitError::SignInNeeded,
            GitError::Network,
            GitError::BranchExists,
            GitError::InvalidBranchName,
            GitError::BranchNotFound,
            GitError::CheckedOutElsewhere,
        ] {
            assert!(!t(e).contains('{'));
        }
    }

    #[test]
    fn the_changed_part_is_the_root() {
        assert_eq!(
            roots("D:\\work\\acta", "C:\\Users\\Alexandre\\dev\\acta"),
            Some(("D:\\work".into(), "C:\\Users\\Alexandre\\dev".into()))
        );
        assert_eq!(
            roots("/mnt/old/clients/acta", "/home/alexandre/clients/acta"),
            Some(("/mnt/old".into(), "/home/alexandre".into()))
        );
        assert_eq!(roots("D:\\work\\acta", "D:\\work\\acta"), None);
    }

    #[test]
    fn only_missing_folders_that_exist_under_the_new_root_are_offered() {
        let on_disk = ["/home/a/dev/acta-docs", "/home/a/dev/acta"];
        let exists = |p: &str| on_disk.contains(&p);
        let paths: Vec<(u8, String)> = [
            "/mnt/old/acta-docs",
            "/mnt/old/design",
            "/elsewhere/acta",
            "/home/a/dev/acta",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, p)| (i as u8, p.to_string()))
        .collect();
        if cfg!(windows) {
            // Path normalisation on Windows upper-cases drive letters and
            // keeps the separators; the rule is checked with Windows paths.
            let on_disk = ["C:\\dev\\acta-docs"];
            let exists = |p: &str| on_disk.contains(&p);
            let paths = vec![
                (1u8, "D:\\work\\acta-docs".to_string()),
                (2u8, "D:\\work\\design".to_string()),
            ];
            assert_eq!(
                candidates(&paths, "D:\\work", "C:\\dev", exists),
                [(
                    1u8,
                    "D:\\work\\acta-docs".to_string(),
                    "C:\\dev\\acta-docs".to_string()
                )]
            );
        } else {
            assert_eq!(
                candidates(&paths, "/mnt/old", "/home/a/dev", exists),
                [(
                    0u8,
                    "/mnt/old/acta-docs".to_string(),
                    "/home/a/dev/acta-docs".to_string()
                )]
            );
        }
    }
}
