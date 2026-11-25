//! Temporary repositories built with `git init` (author and committer
//! Alexandre, fixed dates, never a network), one per case of 5.R.10's test
//! list. After every call: no `index.lock`, and the index unchanged.

use super::fs::{
    Entry, EntryKind, FileContent, list_dir, rank, read_file, runs_as_program, search_files,
};
use super::graph::{LaneBuilder, Line, Row};
use super::*;
use crate::git_cmd::command;
use crate::paths::Os;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime};

const T0: i64 = 1_790_000_000;

struct TestRepo {
    tmp: tempfile::TempDir,
    dir: PathBuf,
    clock: Cell<i64>,
}

impl TestRepo {
    fn new() -> TestRepo {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("work");
        fs::create_dir_all(&dir).unwrap();
        let r = TestRepo {
            tmp,
            dir,
            clock: Cell::new(T0),
        };
        r.git(&["init", "-q", "-b", "main"]);
        configure(&r.dir);
        r
    }

    fn tmp(&self) -> &Path {
        self.tmp.path()
    }

    /// Runs git as a user would, with the next fixed date.
    fn git(&self, args: &[&str]) -> String {
        git_in(&self.dir, args, &self.clock)
    }

    fn write(&self, rel: &str, text: &str) {
        let p = self.dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn commit(&self, rel: &str, text: &str, msg: &str) -> String {
        self.write(rel, text);
        self.git(&["add", "--", rel]);
        self.git(&["commit", "-q", "-m", msg]);
        self.head()
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn repo(&self) -> Repo {
        Repo::open(&self.dir).unwrap()
    }

    /// The index's state before a read.
    fn guard(&self) -> Guard {
        Guard::new(&self.dir)
    }
}

fn configure(dir: &Path) {
    let c = Cell::new(T0);
    for (k, v) in [
        ("user.name", "Alexandre"),
        ("user.email", "alexandre@example.com"),
        ("commit.gpgsign", "false"),
        ("tag.gpgsign", "false"),
        ("core.autocrlf", "false"),
    ] {
        git_in(dir, &["config", k, v], &c);
    }
}

fn git_in(dir: &Path, args: &[&str], clock: &Cell<i64>) -> String {
    let t = clock.get() + 60;
    clock.set(t);
    let date = format!("{t} +0000");
    let out = command(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Alexandre")
        .env("GIT_AUTHOR_EMAIL", "alexandre@example.com")
        .env("GIT_COMMITTER_NAME", "Alexandre")
        .env("GIT_COMMITTER_EMAIL", "alexandre@example.com")
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// No `index.lock` and an unchanged index after a call.
struct Guard {
    git_dir: PathBuf,
    index: Option<(SystemTime, u64)>,
}

impl Guard {
    fn new(dir: &Path) -> Guard {
        let git_dir = git_dir_of(dir);
        let index = fs::metadata(git_dir.join("index"))
            .ok()
            .map(|m| (m.modified().unwrap(), m.len()));
        Guard { git_dir, index }
    }

    fn check(&self) {
        assert!(
            !self.git_dir.join("index.lock").exists(),
            "an index.lock was left"
        );
        let now = fs::metadata(self.git_dir.join("index"))
            .ok()
            .map(|m| (m.modified().unwrap(), m.len()));
        assert_eq!(now, self.index, "the index changed");
    }
}

fn git_dir_of(dir: &Path) -> PathBuf {
    let dot = dir.join(".git");
    if dot.is_dir() {
        return dot;
    }
    let text = fs::read_to_string(&dot).unwrap();
    PathBuf::from(text.trim().strip_prefix("gitdir:").unwrap().trim())
}

/// Every commit of the query, all pages, with the guard checked after each.
fn all_commits(r: &TestRepo, q: &LogQuery) -> Vec<Commit> {
    let repo = r.repo();
    let g = r.guard();
    let mut out = Vec::new();
    loop {
        let page = repo.log_page(q, out.len()).unwrap();
        g.check();
        let more = page.more;
        out.extend(page.commits);
        if !more {
            return out;
        }
    }
}

fn rows(commits: &[Commit], head: Option<String>, shallow: Vec<String>) -> Vec<Row> {
    let mut b = LaneBuilder::new(head, shallow);
    commits.iter().map(|c| b.push(&c.id, &c.parents)).collect()
}

/// The lanes leaving a row's bottom edge are the lanes entering the next
/// row's top edge; nothing is left open after a complete history.
fn assert_continuous(rows: &[Row]) {
    let bottom = |r: &Row| -> BTreeSet<u16> {
        r.lines
            .iter()
            .filter_map(|s| match s.line {
                Line::Through(l) | Line::Out(l) => Some(l),
                Line::In(_) => None,
            })
            .collect()
    };
    let top = |r: &Row| -> BTreeSet<u16> {
        r.lines
            .iter()
            .filter_map(|s| match s.line {
                Line::Through(l) | Line::In(l) => Some(l),
                Line::Out(_) => None,
            })
            .collect()
    };
    if let Some(first) = rows.first() {
        assert!(top(first).is_empty(), "the first row has lines from above");
    }
    for w in rows.windows(2) {
        assert_eq!(bottom(&w[0]), top(&w[1]), "rows do not join: {w:?}");
    }
    if let Some(last) = rows.last() {
        assert!(bottom(last).is_empty(), "lines left open at the end");
    }
}

fn outs(r: &Row) -> Vec<u16> {
    r.lines
        .iter()
        .filter_map(|s| match s.line {
            Line::Out(l) => Some(l),
            _ => None,
        })
        .collect()
}

fn ins(r: &Row) -> Vec<u16> {
    r.lines
        .iter()
        .filter_map(|s| match s.line {
            Line::In(l) => Some(l),
            _ => None,
        })
        .collect()
}

#[test]
fn linear_history_is_one_accent_lane() {
    let r = TestRepo::new();
    let a = r.commit("a.txt", "1\n", "First");
    let b = r.commit("a.txt", "2\n", "Second");
    let c = r.commit("a.txt", "3\n", "Third");
    let commits = all_commits(&r, &LogQuery::default());
    let ids: Vec<_> = commits.iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids, vec![c.clone(), b, a]);
    assert!(commits[0].head);
    assert_eq!(commits[0].author, "Alexandre");
    assert!(commits[0].time > T0);
    let rows = rows(&commits, Some(c), vec![]);
    assert_continuous(&rows);
    assert!(rows.iter().all(|r| r.lane == 0 && r.head_chain));
    assert!(rows[0].head && !rows[1].head);
    assert!(rows.iter().flat_map(|r| &r.lines).all(|s| s.accent));
}

#[test]
fn branch_and_merge_bend_in_and_out() {
    let r = TestRepo::new();
    let a = r.commit("a.txt", "1\n", "Base");
    r.git(&["switch", "-q", "-c", "resize"]);
    let b = r.commit("b.txt", "b\n", "Resize");
    r.git(&["switch", "-q", "main"]);
    let c = r.commit("a.txt", "2\n", "Main work");
    r.git(&[
        "merge",
        "-q",
        "--no-ff",
        "-m",
        "Merge branch resize",
        "resize",
    ]);
    let m = r.head();
    let commits = all_commits(&r, &LogQuery::default());
    let ids: Vec<_> = commits.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids[0], m);
    assert_eq!(ids[3], a);
    let rows = rows(&commits, Some(m.clone()), vec![]);
    assert_continuous(&rows);
    assert!(rows[0].merge && rows[0].head);
    assert_eq!(outs(&rows[0]), vec![0, 1]);
    let row_of = |id: &str| &rows[ids.iter().position(|i| *i == id).unwrap()];
    assert!(row_of(&c).head_chain && row_of(&c).lane == 0);
    assert!(!row_of(&b).head_chain && row_of(&b).lane == 1);
    // The base is awaited by the side lane first: the main line bends out
    // to it (5.R.10, Lanes) and carries the accent there.
    assert_eq!(outs(row_of(&c)), vec![1]);
    assert!(
        row_of(&c)
            .lines
            .iter()
            .any(|s| s.line == Line::Out(1) && s.accent)
    );
    assert_eq!(ins(&rows[3]), vec![1], "one line ends at the base");
    assert!(rows[3].head_chain && rows[3].lines.iter().all(|s| s.accent));
    // The accent follows the first parents only.
    let accent: Vec<_> = rows[0].lines.iter().map(|s| (s.line, s.accent)).collect();
    assert_eq!(accent, vec![(Line::Out(0), true), (Line::Out(1), false)]);
}

#[test]
fn octopus_merge_fans_out() {
    let r = TestRepo::new();
    r.commit("a.txt", "1\n", "Base");
    for b in ["x", "y", "z"] {
        r.git(&["switch", "-q", "-c", b, "main"]);
        r.commit(&format!("{b}.txt"), "1\n", b);
    }
    r.git(&["switch", "-q", "main"]);
    r.git(&["merge", "-q", "-m", "Octopus", "x", "y", "z"]);
    let commits = all_commits(&r, &LogQuery::default());
    assert_eq!(commits[0].parents.len(), 3);
    let rows = rows(&commits, Some(r.head()), vec![]);
    assert_continuous(&rows);
    assert_eq!(outs(&rows[0]).len(), 3);
    assert!(rows.iter().map(Row::width).max().unwrap() >= 3);
}

#[test]
fn criss_cross_and_two_roots_stay_continuous() {
    let r = TestRepo::new();
    r.commit("a.txt", "1\n", "Base");
    r.git(&["switch", "-q", "-c", "side"]);
    r.commit("s.txt", "1\n", "Side 1");
    r.git(&["switch", "-q", "main"]);
    r.commit("m.txt", "1\n", "Main 1");
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    r.git(&["switch", "-q", "side"]);
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge main", "main~1"]);
    r.git(&["switch", "-q", "main"]);
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge side again", "side"]);
    // A second root, merged in.
    r.git(&["switch", "-q", "--orphan", "other"]);
    r.commit("o.txt", "1\n", "Other root");
    r.git(&["switch", "-q", "main"]);
    r.git(&[
        "merge",
        "-q",
        "--allow-unrelated-histories",
        "-m",
        "Join roots",
        "other",
    ]);
    let commits = all_commits(&r, &LogQuery::default());
    assert_eq!(commits.iter().filter(|c| c.parents.is_empty()).count(), 2);
    let rows = rows(&commits, Some(r.head()), vec![]);
    assert_continuous(&rows);
    let chain = rows.iter().filter(|r| r.head_chain).count();
    assert_eq!(chain, 5, "Join, Merge again, Merge side, Main 1, Base");
}

#[test]
fn detached_head_is_marked_and_chained() {
    let r = TestRepo::new();
    r.commit("a.txt", "1\n", "One");
    let two = r.commit("a.txt", "2\n", "Two");
    r.commit("a.txt", "3\n", "Three");
    r.git(&["switch", "-q", "--detach", "HEAD~1"]);
    let repo = r.repo();
    let g = r.guard();
    let refs = repo.refs().unwrap();
    g.check();
    assert_eq!(refs.branch, None);
    assert_eq!(refs.head.as_deref(), Some(two.as_str()));
    let s = repo.status().unwrap();
    g.check();
    assert_eq!((s.branch, s.head.as_deref()), (None, Some(two.as_str())));
    let commits = all_commits(&r, &LogQuery::default());
    assert_eq!(commits.len(), 3);
    assert!(commits[1].head && !commits[0].head);
    let rows = rows(&commits, refs.head.clone(), vec![]);
    assert!(!rows[0].head_chain && rows[1].head && rows[2].head_chain);
    let only_head = all_commits(
        &r,
        &LogQuery {
            selection: Selection::Head,
            ..LogQuery::default()
        },
    );
    assert_eq!(only_head.len(), 2);
}

#[test]
fn refs_on_one_commit_and_the_snapshot() {
    let r = TestRepo::new();
    let c = r.commit("a.txt", "1\n", "Release");
    r.git(&["tag", "v1"]);
    r.git(&["tag", "-a", "-m", "Annotated", "v1-note"]);
    r.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    r.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/acta.git",
    ]);
    r.git(&["config", "branch.main.remote", "origin"]);
    r.git(&["config", "branch.main.merge", "refs/heads/main"]);
    r.git(&["branch", "feature"]);
    let commits = all_commits(&r, &LogQuery::default());
    assert_eq!(commits.len(), 1);
    let mut names: Vec<_> = commits[0]
        .refs
        .iter()
        .map(|r| (r.kind, r.name.as_str()))
        .collect();
    names.sort_by_key(|(_, n)| n.to_string());
    assert_eq!(
        names,
        vec![
            (RefKind::Local, "feature"),
            (RefKind::Local, "main"),
            (RefKind::Remote, "origin/main"),
            (RefKind::Tag, "v1"),
            (RefKind::Tag, "v1-note"),
        ]
    );
    let repo = r.repo();
    let g = r.guard();
    let snap = repo.refs().unwrap();
    g.check();
    assert_eq!(snap.at(&c).count(), 5, "the annotated tag is peeled");
    assert_eq!(snap.upstream("main"), Some("origin/main"));
    assert_eq!(snap.branch.as_deref(), Some("main"));
    assert_eq!(snap.branches(RefKind::Remote).count(), 1);
    // Staying current: a new commit changes the snapshot.
    assert_eq!(repo.refs().unwrap(), snap);
    r.commit("a.txt", "2\n", "Next");
    assert_ne!(repo.refs().unwrap(), snap);
    let only = all_commits(
        &r,
        &LogQuery {
            selection: Selection::Ref("refs/heads/feature".into()),
            ..LogQuery::default()
        },
    );
    assert_eq!(only.len(), 1);
}

/// A fast-import stream of `n` commits on `main`: a side branch of three
/// commits forks 20 commits back and merges in every 100 commits, and one
/// long branch spans `long` (fork, join) when given.
fn import(dir: &Path, n: usize, long: Option<(usize, usize)>) {
    let mut s = String::new();
    let mut mark = 0usize;
    let mut main: Vec<usize> = Vec::new();
    let mut long_tip = None;
    let mut count = 0usize;
    let mut side = 0usize;
    let mut commit = |s: &mut String, r: &str, from: Option<usize>, merge: Option<usize>| {
        mark += 1;
        let t = T0 + mark as i64;
        let msg = format!("Commit {mark}");
        s.push_str(&format!(
            "commit {r}\nmark :{mark}\nauthor Alexandre <alexandre@example.com> {t} +0000\n\
             committer Alexandre <alexandre@example.com> {t} +0000\ndata {}\n{msg}\n",
            msg.len()
        ));
        if let Some(f) = from {
            s.push_str(&format!("from :{f}\n"));
        }
        if let Some(m) = merge {
            s.push_str(&format!("merge :{m}\n"));
        }
        let body = format!("{mark}\n");
        s.push_str(&format!(
            "M 100644 inline f{}.txt\ndata {}\n{body}\n",
            mark % 7,
            body.len()
        ));
        mark
    };
    while count < n {
        if let Some((fork, _)) = long
            && count == fork
        {
            long_tip = Some(commit(
                &mut s,
                "refs/heads/long",
                main.last().copied(),
                None,
            ));
            count += 1;
            continue;
        }
        if let Some((_, join)) = long
            && count == join
            && let Some(tip) = long_tip
        {
            let m = commit(&mut s, "refs/heads/main", main.last().copied(), Some(tip));
            main.push(m);
            count += 1;
            continue;
        }
        if count % 100 == 99 && main.len() > 20 && count + 4 <= n {
            side += 1;
            let r = format!("refs/heads/side{side}");
            let mut tip = commit(&mut s, &r, Some(main[main.len() - 20]), None);
            tip = commit(&mut s, &r, Some(tip), None);
            tip = commit(&mut s, &r, Some(tip), None);
            let m = commit(&mut s, "refs/heads/main", main.last().copied(), Some(tip));
            main.push(m);
            count += 4;
            continue;
        }
        let m = commit(&mut s, "refs/heads/main", main.last().copied(), None);
        main.push(m);
        count += 1;
    }
    let mut child = command(dir)
        .args(["fast-import", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn a_page_boundary_inside_a_merge_gives_the_lanes_of_one_page() {
    let r = TestRepo::new();
    // The long branch forks before commit 1,000 of the list and joins after.
    import(&r.dir, 1_600, Some((300, 1_400)));
    let repo = r.repo();
    let g = r.guard();
    let first = repo.log_page(&LogQuery::default(), 0).unwrap();
    g.check();
    assert_eq!(first.commits.len(), PAGE_SIZE);
    assert!(first.more && !first.capped);
    let paged = all_commits(&r, &LogQuery::default());
    assert_eq!(paged.len(), 1_600);
    // The same list read at once.
    let whole = repo
        .text(
            &[
                "log",
                "--topo-order",
                "--parents",
                "-z",
                "--decorate=full",
                "--no-mailmap",
                "--format=%H%x1f%P%x1f%an%x1f%cn%x1f%at%x1f%D%x1f%s",
                "--branches",
                "--remotes",
                "--tags",
                "HEAD",
                "--",
            ],
            PAGE_LIMIT,
        )
        .unwrap();
    let whole = parse_page(&whole);
    assert_eq!(paged, whole);
    let head = repo.head_from_files();
    let a = rows(&paged, head.clone(), vec![]);
    assert_continuous(&a);
    // The rows on both sides of the boundary have the long lane open.
    assert!(a[999].lines.len() > 1 && a[1000].lines.len() > 1);
    // Fed page by page.
    let mut b = LaneBuilder::new(head, vec![]);
    let mut paged_rows = Vec::new();
    for chunk in paged.chunks(PAGE_SIZE) {
        paged_rows.extend(chunk.iter().map(|c| b.push(&c.id, &c.parents)));
    }
    assert_eq!(paged_rows, a);
}

#[test]
fn path_filter_rewrites_parents_across_two_branches() {
    let r = TestRepo::new();
    r.commit("f.txt", "1\n", "Base");
    r.commit("other.txt", "1\n", "Not f");
    r.git(&["switch", "-q", "-c", "side"]);
    r.commit("f.txt", "1\nside\n", "Side changes f");
    r.commit("g.txt", "1\n", "Side changes g");
    r.git(&["switch", "-q", "main"]);
    r.commit("f.txt", "main\n1\n", "Main changes f");
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    let q = LogQuery {
        path: Some("f.txt".into()),
        ..LogQuery::default()
    };
    let commits = all_commits(&r, &q);
    let subjects: Vec<_> = commits.iter().map(|c| c.subject.as_str()).collect();
    assert!(!subjects.contains(&"Not f") && !subjects.contains(&"Side changes g"));
    assert!(subjects.contains(&"Side changes f") && subjects.contains(&"Main changes f"));
    let ids: BTreeSet<_> = commits.iter().map(|c| c.id.as_str()).collect();
    for c in &commits {
        for p in &c.parents {
            assert!(ids.contains(p.as_str()), "parent not rewritten: {p}");
        }
    }
    assert_continuous(&rows(&commits, r.repo().head_from_files(), vec![]));
}

#[test]
fn shallow_clone_ends_in_a_stub() {
    let r = TestRepo::new();
    for i in 0..4 {
        r.commit("a.txt", &format!("{i}\n"), &format!("C{i}"));
    }
    let url = format!("file://{}", r.dir.to_string_lossy().replace('\\', "/"));
    let clone = r.tmp().join("shallow");
    git_in(
        r.tmp(),
        &["clone", "-q", "--depth", "2", &url, "shallow"],
        &Cell::new(T0),
    );
    let repo = Repo::open(&clone).unwrap();
    let g = Guard::new(&clone);
    let boundary = repo.shallow_boundary();
    assert_eq!(boundary.len(), 1);
    let page = repo.log_page(&LogQuery::default(), 0).unwrap();
    g.check();
    assert_eq!(page.commits.len(), 2);
    let rows = rows(&page.commits, repo.head_from_files(), boundary);
    assert!(rows[1].stub && !rows[0].stub);
}

#[test]
fn subjects_keep_separators_and_non_ascii_text() {
    let r = TestRepo::new();
    let subject = "Fix \u{1f}field\u{1e} and caf\u{e9} \u{65e5}\u{672c}";
    r.write("a.txt", "1\n");
    r.git(&["add", "a.txt"]);
    r.git(&["commit", "-q", "-m", subject, "-m", "Body line"]);
    let commits = all_commits(&r, &LogQuery::default());
    assert_eq!(commits[0].subject, subject);
    let repo = r.repo();
    let d = repo.commit_details(&commits[0].id).unwrap();
    assert_eq!(d.subject(), subject);
    assert_eq!(d.body(), Some("Body line"));
    assert_eq!(d.author, "Alexandre");
    assert!(d.parents.is_empty());
    assert_eq!(d.files.len(), 1);
    assert_eq!(d.files[0].kind, ChangeKind::Added);
}

#[test]
fn text_filter_matches_message_or_author_and_hash_lookup() {
    let r = TestRepo::new();
    r.commit("a.txt", "1\n", "Unrelated");
    r.write("b.txt", "1\n");
    r.git(&["add", "b.txt"]);
    r.git(&[
        "commit",
        "-q",
        "-m",
        "Plain",
        "--author",
        "Bob Martin <bob@example.com>",
    ]);
    let bob = r.head();
    r.commit("c.txt", "1\n", "Fix BOB's resize");
    let q = LogQuery {
        text: Some("bob".into()),
        ..LogQuery::default()
    };
    let found = all_commits(&r, &q);
    let subjects: Vec<_> = found.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, vec!["Fix BOB's resize", "Plain"]);
    let repo = r.repo();
    let g = r.guard();
    let c = repo.find_commit(&bob[..8]).unwrap().unwrap();
    g.check();
    assert_eq!(c.id, bob);
    assert_eq!(c.author, "Bob Martin");
    assert_eq!(repo.find_commit("deadbeef").unwrap(), None);
    assert_eq!(repo.find_commit("--all").unwrap(), None);
}

#[test]
fn rename_and_merge_details_and_diffs() {
    let r = TestRepo::new();
    let text: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    r.commit("src/old.rs", &text, "Add");
    r.git(&["mv", "src/old.rs", "src/new.rs"]);
    r.write("src/new.rs", &text.replace("line 10\n", "line ten\n"));
    r.git(&["add", "-A"]);
    r.git(&["commit", "-q", "-m", "Rename"]);
    let repo = r.repo();
    let g = r.guard();
    let d = repo.commit_details(&r.head()).unwrap();
    g.check();
    assert_eq!(d.files.len(), 1);
    let f = &d.files[0];
    assert_eq!(
        (f.kind, f.path.as_str(), f.from.as_deref(), f.stats),
        (
            ChangeKind::Renamed,
            "src/new.rs",
            Some("src/old.rs"),
            Some((1, 1))
        )
    );
    let diff = repo.commit_diff(&d.id, f).unwrap();
    g.check();
    assert_eq!((diff.added, diff.removed, diff.hunks.len()), (1, 1, 1));
    let removed = diff.hunks[0]
        .lines
        .iter()
        .find(|l| l.kind == LineKind::Removed)
        .unwrap();
    assert_eq!((removed.text.as_str(), removed.old), ("line 10", Some(10)));

    // A merge lists its files against the first parent.
    r.git(&["switch", "-q", "-c", "side"]);
    r.commit("side.txt", "s\n", "Side");
    r.git(&["switch", "-q", "main"]);
    r.commit("main.txt", "m\n", "Main");
    r.git(&["merge", "-q", "--no-ff", "-m", "Merge", "side"]);
    let g = r.guard();
    let m = repo.commit_details(&r.head()).unwrap();
    g.check();
    assert_eq!(m.parents.len(), 2);
    let paths: Vec<_> = m.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["side.txt"]);
    assert_eq!(m.totals(), (1, 0));
    let md = repo.commit_diff(&m.id, &m.files[0]).unwrap();
    g.check();
    assert_eq!(md.added, 1);
}

fn png(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([shade, 0, 0]));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn binary_files_and_their_blobs() {
    let r = TestRepo::new();
    let before = png(4, 3, 10);
    fs::write(r.dir.join("logo.png"), &before).unwrap();
    r.git(&["add", "logo.png"]);
    r.git(&["commit", "-q", "-m", "Logo"]);
    let first = r.head();
    let after = png(8, 6, 200);
    fs::write(r.dir.join("logo.png"), &after).unwrap();
    r.git(&["commit", "-q", "-am", "Bigger logo"]);
    let repo = r.repo();
    let g = r.guard();
    let d = repo.commit_details(&r.head()).unwrap();
    assert_eq!(d.files[0].stats, None, "binary: no counts");
    let diff = repo.commit_diff(&d.id, &d.files[0]).unwrap();
    assert!(diff.binary && diff.hunks.is_empty());
    assert_eq!(repo.blob(&first, "logo.png").unwrap(), Some(before.clone()));
    assert_eq!(repo.blob(&d.id, "logo.png").unwrap(), Some(after.clone()));
    assert_eq!(
        repo.blob_size(&first, "logo.png").unwrap(),
        before.len() as u64
    );
    assert!(repo.blob(&first, "missing.png").is_err());
    g.check();
    match read_file(&r.dir, "logo.png").unwrap() {
        FileContent::Image { width, height, .. } => assert_eq!((width, height), (8, 6)),
        other => panic!("not an image: {other:?}"),
    }
}

#[test]
fn a_six_thousand_line_diff_is_cut_at_five_thousand() {
    let r = TestRepo::new();
    r.commit("big.txt", "start\n", "Start");
    let text: String = (0..6_000).map(|i| format!("row {i}\n")).collect();
    r.commit("big.txt", &format!("start\n{text}"), "Grow");
    let repo = r.repo();
    let g = r.guard();
    let d = repo.commit_details(&r.head()).unwrap();
    assert_eq!(d.files[0].stats, Some((6_000, 0)));
    let diff = repo.commit_diff(&d.id, &d.files[0]).unwrap();
    g.check();
    assert!(diff.truncated);
    let lines: usize = diff.hunks.iter().map(|h| h.lines.len()).sum();
    assert_eq!(lines, DIFF_LINES);
}

#[test]
fn every_status_kind_and_the_uncommitted_files() {
    let r = TestRepo::new();
    r.commit("mod.txt", "1\n", "Base");
    r.commit("del.txt", "1\n", "Del");
    r.commit("ren.txt", "a\nb\nc\nd\n", "Ren");
    r.commit("conflict.txt", "base\n", "Conflict base");
    r.git(&["switch", "-q", "-c", "other"]);
    r.commit("conflict.txt", "other\n", "Other side");
    r.git(&["switch", "-q", "main"]);
    r.commit("conflict.txt", "main\n", "Main side");
    // A conflicted merge: the user's git writes the index, not Acta.
    let out = command(&r.dir)
        .args(["merge", "-q", "other"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    r.write("mod.txt", "2\n");
    fs::remove_file(r.dir.join("del.txt")).unwrap();
    r.git(&["mv", "ren.txt", "renamed.txt"]);
    r.write("added.txt", "new\n");
    r.git(&["add", "added.txt"]);
    r.write("untracked.txt", "u\n");
    r.write("newdir/inner.txt", "u\n");
    r.write("sub/tracked.txt", "1\n");
    r.git(&["add", "sub/tracked.txt"]);
    let repo = r.repo();
    let g = r.guard();
    let s = repo.status().unwrap();
    g.check();
    let mut marks: Vec<_> = s
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.mark.letter(), e.is_dir))
        .collect();
    marks.sort();
    assert_eq!(
        marks,
        vec![
            ("added.txt", 'A', false),
            ("conflict.txt", 'C', false),
            ("del.txt", 'D', false),
            ("mod.txt", 'M', false),
            ("newdir", 'U', true),
            ("renamed.txt", 'R', false),
            ("sub/tracked.txt", 'A', false),
            ("untracked.txt", 'U', false),
        ]
    );
    let renamed = s.entries.iter().find(|e| e.mark == Mark::Renamed).unwrap();
    assert_eq!(renamed.from.as_deref(), Some("ren.txt"));
    assert_eq!(s.changed(), 8);
    let under = s.marks_under("sub");
    assert_eq!(under.len(), 1);
    assert_eq!(under.get("tracked.txt"), Some(&Mark::Added));
    let files = repo.uncommitted_files().unwrap();
    g.check();
    let m = files.iter().find(|f| f.path == "mod.txt").unwrap();
    assert_eq!((m.kind, m.stats), (ChangeKind::Modified, Some((1, 1))));
    assert!(
        files
            .iter()
            .any(|f| f.path == "untracked.txt" && f.kind == ChangeKind::Added)
    );
    let d = repo.worktree_diff(m).unwrap();
    g.check();
    assert_eq!((d.added, d.removed), (1, 1));
    let dir = files.iter().find(|f| f.path == "newdir").unwrap();
    assert!(dir.is_dir);
    assert!(repo.worktree_diff(dir).is_err());
    let u = files.iter().find(|f| f.path == "untracked.txt").unwrap();
    let d = repo.worktree_diff(u).unwrap();
    g.check();
    assert_eq!((d.added, d.hunks[0].lines[0].text.as_str()), (1, "u"));
}

#[test]
fn an_empty_repository_reads_as_no_commits() {
    let r = TestRepo::new();
    r.write("first.txt", "1\n");
    let repo = r.repo();
    let g = r.guard();
    let page = repo.log_page(&LogQuery::default(), 0).unwrap();
    assert!(page.commits.is_empty() && !page.more);
    let head = repo
        .log_page(
            &LogQuery {
                selection: Selection::Head,
                ..LogQuery::default()
            },
            0,
        )
        .unwrap();
    assert!(head.commits.is_empty());
    let s = repo.status().unwrap();
    assert_eq!(
        (s.head.as_deref(), s.branch.as_deref(), s.changed()),
        (None, Some("main"), 1)
    );
    let snap = repo.refs().unwrap();
    assert!(snap.refs.is_empty() && snap.head.is_none());
    let files = repo.uncommitted_files().unwrap();
    assert_eq!(files.len(), 1);
    g.check();
}

#[test]
fn a_linked_worktree_reads_like_its_main_checkout() {
    let r = TestRepo::new();
    r.commit(".gitignore", "target/\n", "Ignore target");
    fs::write(r.dir.join(".git/info/exclude"), "secret.txt\n").unwrap();
    let wt = r.tmp().join("wt");
    r.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "wt-branch",
        wt.to_str().unwrap(),
    ]);
    fs::create_dir_all(wt.join("target")).unwrap();
    fs::write(wt.join("secret.txt"), "x").unwrap();
    fs::write(wt.join("note.txt"), "x").unwrap();
    let repo = Repo::open(&wt).unwrap();
    let g = Guard::new(&wt);
    let snap = repo.refs().unwrap();
    assert_eq!(snap.branch.as_deref(), Some("wt-branch"));
    let s = repo.status().unwrap();
    g.check();
    assert_eq!(s.entries.len(), 1, "only note.txt: {s:?}");
    let names: Vec<_> = list_dir(&wt, "", false)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec![".gitignore", "note.txt"]);
}

#[test]
fn listing_hides_what_git_ignores() {
    let r = TestRepo::new();
    r.write(".gitignore", "target/\n*.log\n!keep.log\n");
    r.write("sub/.gitignore", "local.txt\n");
    fs::write(r.dir.join(".git/info/exclude"), "secret.txt\n").unwrap();
    for f in [
        "target/debug/app",
        "a.log",
        "keep.log",
        "secret.txt",
        "file10.txt",
        "file2.txt",
        "File3.txt",
        ".github/ci.yml",
        "sub/local.txt",
        "sub/kept.txt",
        "src/main.rs",
    ] {
        r.write(f, "x");
    }
    let names = |rel: &str, show: bool| -> Vec<(String, bool)> {
        list_dir(&r.dir, rel, show)
            .unwrap()
            .into_iter()
            .map(|e: Entry| (e.name, e.ignored))
            .collect()
    };
    let plain = |v: Vec<(String, bool)>| v.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
    assert_eq!(
        plain(names("", false)),
        vec![
            ".github",
            "src",
            "sub",
            ".gitignore",
            "file2.txt",
            "File3.txt",
            "file10.txt",
            "keep.log"
        ]
    );
    let shown = names("", true);
    assert!(shown.contains(&("target".into(), true)));
    assert!(shown.contains(&("a.log".into(), true)));
    assert!(shown.contains(&("secret.txt".into(), true)));
    assert!(shown.contains(&("keep.log".into(), false)));
    assert!(!shown.iter().any(|(n, _)| n == ".git"));
    assert_eq!(plain(names("sub", false)), vec![".gitignore", "kept.txt"]);
    // Inside an ignored folder everything is ignored.
    assert_eq!(names("target", true), vec![("debug".into(), true)]);
    assert_eq!(names("target/debug", true), vec![("app".into(), true)]);
    assert!(names("target", false).is_empty());
    assert_eq!(
        list_dir(&r.dir, "src", false).unwrap()[0].kind,
        EntryKind::File
    );

    // The search walk applies the same rules.
    let s = search_files(&r.dir, false, super::fs::SEARCH_LIMIT);
    let mut files = s.files.clone();
    files.sort();
    assert_eq!(
        files,
        vec![
            ".github/ci.yml",
            ".gitignore",
            "File3.txt",
            "file10.txt",
            "file2.txt",
            "keep.log",
            "src/main.rs",
            "sub/.gitignore",
            "sub/kept.txt",
        ]
    );
    assert!(!s.capped);
    let all = search_files(&r.dir, true, super::fs::SEARCH_LIMIT);
    assert!(all.files.contains(&"target/debug/app".to_owned()));
    assert!(!all.files.iter().any(|f| f.starts_with(".git/")));

    // Outside a repository nothing is hidden.
    let plain_dir = r.tmp().join("plain");
    fs::create_dir_all(&plain_dir).unwrap();
    fs::write(plain_dir.join(".gitignore"), "*.log\n").unwrap();
    fs::write(plain_dir.join("a.log"), "x").unwrap();
    assert_eq!(list_dir(&plain_dir, "", false).unwrap().len(), 2);
    assert_eq!(
        search_files(&plain_dir, false, super::fs::SEARCH_LIMIT)
            .files
            .len(),
        2
    );
}

/// A link to a folder: a symlink on Unix, a junction on Windows (which
/// needs no privilege, unlike a symlink there).
fn link_dir(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let out = std::process::Command::new("cmd")
            .arg("/c")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(out.status.success(), "mklink: {out:?}");
    }
}

#[test]
fn links_to_folders_are_listed_never_walked_and_kept_inside() {
    let r = TestRepo::new();
    r.write("real/inside.txt", "x");
    link_dir(&r.dir.join("real"), &r.dir.join("link"));
    let outside = r.tmp().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "Alexandre's secret").unwrap();
    link_dir(&outside, &r.dir.join("away"));
    let e = list_dir(&r.dir, "", false).unwrap();
    for name in ["link", "away"] {
        assert!(
            e.iter()
                .any(|e| e.name == name && e.kind == EntryKind::LinkDir),
            "{name}: {e:?}"
        );
    }
    // Search lists neither link as a file, nor walks into them.
    let s = search_files(&r.dir, false, super::fs::SEARCH_LIMIT);
    assert_eq!(s.files, vec!["real/inside.txt"]);
    // A link inside the worktree expands; one leading out does not.
    assert_eq!(
        list_dir(&r.dir, "link", false).unwrap()[0].name,
        "inside.txt"
    );
    let err = list_dir(&r.dir, "away", false).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(matches!(
        read_file(&r.dir, "link/inside.txt").unwrap(),
        FileContent::Text { .. }
    ));
    assert_eq!(
        read_file(&r.dir, "away/secret.txt").unwrap(),
        FileContent::LinkOutside
    );
    assert_eq!(read_file(&r.dir, "real").unwrap(), FileContent::NotAFile);
}

#[cfg(unix)]
#[test]
fn a_fifo_is_never_opened() {
    let r = TestRepo::new();
    let st = std::process::Command::new("mkfifo")
        .arg(r.dir.join("pipe"))
        .status()
        .unwrap();
    assert!(st.success());
    // Opening a FIFO for reading would block until a writer comes.
    assert_eq!(read_file(&r.dir, "pipe").unwrap(), FileContent::NotAFile);
}

#[test]
fn a_blobless_clone_never_downloads_for_a_read() {
    let r = TestRepo::new();
    r.commit("a.txt", "one\n", "One");
    r.commit("a.txt", "two\n", "Two");
    r.commit("b.txt", "b\n", "Three");
    r.git(&["config", "uploadpack.allowFilter", "true"]);
    r.git(&["config", "uploadpack.allowAnySHA1InWant", "true"]);
    let url = format!("file://{}", r.dir.to_string_lossy().replace('\\', "/"));
    git_in(
        r.tmp(),
        &["clone", "-q", "--filter=blob:none", &url, "partial"],
        &Cell::new(T0),
    );
    let clone = r.tmp().join("partial");
    let packs = || {
        fs::read_dir(clone.join(".git/objects/pack"))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".pack")
            })
            .count()
    };
    let before = packs();
    let repo = Repo::open(&clone).unwrap();
    let g = Guard::new(&clone);
    let page = repo.log_page(&LogQuery::default(), 0).unwrap();
    assert_eq!(page.commits.len(), 3);
    // "Two" changed a.txt from "one": that blob was never downloaded.
    let two = &page.commits[1].id;
    let file = ChangedFile {
        path: "a.txt".into(),
        from: None,
        kind: ChangeKind::Modified,
        stats: None,
        is_dir: false,
    };
    assert_eq!(repo.commit_diff(two, &file), Err(GitError::NotDownloaded));
    assert_eq!(
        repo.blob(&page.commits[2].id, "a.txt"),
        Err(GitError::NotDownloaded)
    );
    g.check();
    assert_eq!(packs(), before, "a read fetched objects");
}

#[test]
fn paths_are_literal_never_globs() {
    let r = TestRepo::new();
    r.commit("app/[id]/page.tsx", "1\n", "Dynamic route");
    r.commit("app/i/page.tsx", "1\n", "Static route");
    let q = LogQuery {
        path: Some("app/[id]/page.tsx".into()),
        ..LogQuery::default()
    };
    let found = all_commits(&r, &q);
    let subjects: Vec<_> = found.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, vec!["Dynamic route"]);
    let q = LogQuery {
        path: Some(":(glob)app/*".into()),
        ..LogQuery::default()
    };
    assert!(all_commits(&r, &q).is_empty(), "pathspec magic was read");
}

#[test]
fn a_type_change_reads_as_two_parts_without_header_lines() {
    let r = TestRepo::new();
    let text: String = (1..=5).map(|i| format!("line {i}\n")).collect();
    r.commit("f", &text, "File");
    // The file becomes a link, written to the index only (no privilege
    // needed on Windows).
    fs::write(r.tmp().join("target"), "elsewhere/f").unwrap();
    let blob = r.git(&[
        "hash-object",
        "-w",
        r.tmp().join("target").to_str().unwrap(),
    ]);
    r.git(&[
        "update-index",
        "--cacheinfo",
        &format!("120000,{},f", blob.trim()),
    ]);
    r.git(&["commit", "-q", "-m", "Link"]);
    let repo = r.repo();
    let g = r.guard();
    let d = repo.commit_details(&r.head()).unwrap();
    assert_eq!(d.files.len(), 1);
    assert_eq!(
        d.files[0].kind,
        ChangeKind::Modified,
        "a type change reads M"
    );
    let diff = repo.commit_diff(&d.id, &d.files[0]).unwrap();
    g.check();
    assert_eq!(diff.hunks.len(), 2);
    assert_eq!((diff.removed, diff.added), (5, 1));
    let texts: Vec<_> = diff
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .map(|l| l.text.as_str())
        .collect();
    assert!(
        texts
            .iter()
            .all(|t| !t.starts_with("-- a/") && !t.starts_with("++ b/"))
    );
    assert_eq!(texts.last(), Some(&"elsewhere/f"));
}

#[test]
fn configured_diff_programs_never_run() {
    let r = TestRepo::new();
    r.commit(".gitattributes", "*.txt diff=evil\n", "Attributes");
    r.commit("a.txt", "one\n", "One");
    r.commit("a.txt", "two\n", "Two");
    r.write("a.txt", "three\n");
    r.git(&["config", "diff.external", "sh -c 'echo ran > ext-ran'"]);
    r.git(&[
        "config",
        "diff.evil.textconv",
        "sh -c 'echo ran > conv-ran; cat \"$0\"'",
    ]);
    let repo = r.repo();
    let g = r.guard();
    let d = repo.commit_details(&r.head()).unwrap();
    let diff = repo.commit_diff(&d.id, &d.files[0]).unwrap();
    assert_eq!((diff.added, diff.removed), (1, 1));
    let files = repo.uncommitted_files().unwrap();
    let w = repo.worktree_diff(&files[0]).unwrap();
    assert_eq!(w.added, 1);
    g.check();
    assert!(!r.dir.join("ext-ran").exists(), "diff.external ran");
    assert!(!r.dir.join("conv-ran").exists(), "textconv ran");
    // A plain `git diff`, as a user's would, does run them: the test is real.
    let out = command(&r.dir).args(["diff", "HEAD"]).output().unwrap();
    assert!(out.status.success());
    assert!(r.dir.join("ext-ran").exists() || r.dir.join("conv-ran").exists());
}

#[test]
fn short_ids_are_refused_below_seven_digits() {
    let r = TestRepo::new();
    let id = r.commit("a.txt", "1\n", "One");
    let repo = r.repo();
    assert!(repo.commit_details(&id[..6]).is_err());
    assert!(repo.commit_details(&id[..7]).is_ok());
    assert!(repo.blob(&id[..5], "a.txt").is_err());
}

#[test]
fn ranking_puts_name_matches_and_short_paths_first() {
    let paths: Vec<String> = [
        "src/resize/mod.rs",
        "docs/rsize.md",
        "src/term/resize.rs",
        "README.md",
    ]
    .map(str::to_owned)
    .to_vec();
    let (m, total) = rank(&paths, "rsize", 200);
    assert_eq!(total, 3);
    let got: Vec<_> = m.iter().map(|r| paths[r.index].as_str()).collect();
    assert_eq!(
        got,
        vec!["docs/rsize.md", "src/term/resize.rs", "src/resize/mod.rs"]
    );
    assert!(m[0].in_name && !m[2].in_name);
    assert_eq!(m[0].positions, vec![5, 6, 7, 8, 9]);
    let (m, total) = rank(&paths, "RS", 1);
    assert_eq!((m.len(), total), (1, 3));
    assert_eq!(rank(&paths, "zzz", 200).1, 0);
}

#[test]
fn the_viewer_reads_text_binary_and_large_files() {
    let tmp = tempfile::tempdir().unwrap();
    let p = |n: &str| tmp.path().join(n);
    fs::write(p("bom.txt"), "\u{feff}Alexandre\r\nline 2\n").unwrap();
    match read_file(tmp.path(), "bom.txt").unwrap() {
        FileContent::Text {
            text, truncated, ..
        } => {
            assert_eq!(text, "Alexandre\r\nline 2\n");
            assert!(!truncated);
        }
        other => panic!("{other:?}"),
    }
    fs::write(p("nul.bin"), b"abc\0def").unwrap();
    assert_eq!(
        read_file(tmp.path(), "nul.bin").unwrap(),
        FileContent::Binary { size: 7 }
    );
    fs::write(p("latin.txt"), b"caf\xe9\n").unwrap();
    assert_eq!(
        read_file(tmp.path(), "latin.txt").unwrap(),
        FileContent::NotUtf8 { size: 5 }
    );
    // 3 MB of text, a two-byte character across the 1 MB cut.
    let mut big = "a".repeat(super::fs::TEXT_BYTES - 1);
    big.push('\u{e9}');
    big.push_str(&"b".repeat(2 * 1024 * 1024));
    fs::write(p("big.txt"), &big).unwrap();
    match read_file(tmp.path(), "big.txt").unwrap() {
        FileContent::Text {
            text,
            size,
            truncated,
        } => {
            assert!(truncated);
            assert_eq!(size, big.len() as u64);
            assert_eq!(text.len(), super::fs::TEXT_BYTES - 1);
        }
        other => panic!("{other:?}"),
    }
    assert!(read_file(tmp.path(), "missing").is_err());
}

#[test]
fn programs_are_never_opened() {
    let pathext = ".COM;.EXE;.BAT;.CMD";
    let w = |n: &str| runs_as_program(n, false, Os::Windows, pathext);
    assert!(w("setup.EXE") && w("run.cmd") && w("x.ps1") && w("a.lnk") && w("go.url"));
    assert!(w("trick.exe.") && w("trick.exe "));
    assert!(w("tool.jar") && w("panel.cpl") && w("old.pif"));
    assert!(!w("notes.md") && !w("Makefile") && !w("photo.png"));
    let m = |n: &str, x: bool| runs_as_program(n, x, Os::MacOs, "");
    assert!(m("Acta.app", false) && m("go.command", false) && m("tool", true));
    assert!(m("Disk.tool", false) && !m("notes.md", false));
    let l = |n: &str, x: bool| runs_as_program(n, x, Os::Linux, "");
    assert!(l("acta.desktop", false) && l("Acta.AppImage", false) && l("run.sh", true));
    assert!(!l("run.sh", false));
}

#[test]
fn no_git_and_not_a_repository_are_told_apart() {
    let r = TestRepo::new();
    r.commit("a.txt", "1\n", "One");
    let none = Repo::open_with(&r.dir, std::ffi::OsStr::new("acta-no-such-git-alexandre")).unwrap();
    assert_eq!(none.status(), Err(GitError::NoGit));
    let plain = r.tmp().join("plain");
    fs::create_dir_all(&plain).unwrap();
    assert!(matches!(Repo::open(&plain), Err(GitError::NotARepository)));
}

#[test]
fn a_git_that_hangs_is_killed_at_the_limit() {
    let r = TestRepo::new();
    #[cfg(windows)]
    let slow = {
        let p = r.tmp().join("slow-git.cmd");
        fs::write(
            &p,
            "@\"%SystemRoot%\\System32\\ping.exe\" -n 30 127.0.0.1 >NUL\r\n",
        )
        .unwrap();
        p
    };
    #[cfg(unix)]
    let slow = {
        use std::os::unix::fs::PermissionsExt as _;
        let p = r.tmp().join("slow-git");
        fs::write(&p, "#!/bin/sh\nsleep 30\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    };
    let repo = Repo::open_with(&r.dir, slow.as_os_str()).unwrap();
    let start = Instant::now();
    assert_eq!(repo.status(), Err(GitError::TimedOut));
    let took = start.elapsed();
    assert!(
        took >= STATUS_LIMIT && took < STATUS_LIMIT + Duration::from_secs(3),
        "{took:?}"
    );
}

/// Release build: `cargo test --release -p acta-core --lib timing --
/// --ignored --nocapture`. 50,000 commits with a merge every 100.
#[test]
#[ignore]
fn timing_on_fifty_thousand_commits() {
    let r = TestRepo::new();
    let t = Instant::now();
    import(&r.dir, 50_000, None);
    let generated = t.elapsed();
    let repo = r.repo();
    let t = Instant::now();
    let page = repo.log_page(&LogQuery::default(), 0).unwrap();
    let first_page = t.elapsed();
    let t = Instant::now();
    let mut b = LaneBuilder::new(repo.head_from_files(), vec![]);
    let rows: Vec<Row> = page
        .commits
        .iter()
        .map(|c| b.push(&c.id, &c.parents))
        .collect();
    let first_lanes = t.elapsed();
    assert_eq!(rows.len(), PAGE_SIZE);
    let t = Instant::now();
    let mut all = page.commits.len();
    let mut lanes = Duration::ZERO;
    let mut more = page.more;
    while more {
        let p = repo.log_page(&LogQuery::default(), all).unwrap();
        let l = Instant::now();
        for c in &p.commits {
            b.push(&c.id, &c.parents);
        }
        lanes += l.elapsed();
        all += p.commits.len();
        more = p.more;
    }
    let rest = t.elapsed();
    assert_eq!(all, 50_000);
    println!(
        "generated {generated:?}; first page {first_page:?}, its lanes {first_lanes:?}; \
         the other 49 pages {rest:?} (lanes {lanes:?})"
    );
}
