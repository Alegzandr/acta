//! A commit's details (5.R.5, Details pane), a file's diff (5.R.7) and a
//! file's version read into memory for the image diff.

use super::{
    BLOB_BYTES, BLOB_LIMIT, DETAILS_LIMIT, DIFF_BYTES, DIFF_LIMIT, DIFF_LINES, GitError, Repo,
    checked_id, checked_path,
};
use std::path::Path;

/// How a file changed. A type change reads as modified, a copy as added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
}

impl ChangeKind {
    pub fn letter(self) -> char {
        match self {
            ChangeKind::Modified => 'M',
            ChangeKind::Added => 'A',
            ChangeKind::Deleted => 'D',
            ChangeKind::Renamed => 'R',
        }
    }
}

/// A changed file of a commit or of the working tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    /// The path after the change (a deleted file's last path).
    pub path: String,
    /// A rename's source.
    pub from: Option<String>,
    pub kind: ChangeKind,
    /// Lines added and removed; `None` for a binary file (or an untracked
    /// file, which git does not count).
    pub stats: Option<(u32, u32)>,
    /// A whole untracked folder (git lists it once): open it in Files, it
    /// has no diff.
    pub is_dir: bool,
}

/// The details pane's content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitDetails {
    pub id: String,
    pub parents: Vec<String>,
    pub author: String,
    pub author_email: String,
    /// Author date, Unix seconds.
    pub time: i64,
    pub committer: String,
    /// The whole message: the subject, then the body after a blank line.
    pub message: String,
    /// Against the first parent for a merge, against nothing for a root.
    pub files: Vec<ChangedFile>,
}

impl CommitDetails {
    pub fn subject(&self) -> &str {
        self.message.lines().next().unwrap_or("")
    }

    /// The message after the subject, trimmed; `None` when there is none.
    pub fn body(&self) -> Option<&str> {
        let (_, body) = self.message.split_once('\n')?;
        let body = body.trim();
        (!body.is_empty()).then_some(body)
    }

    /// "+42 −7" of the files header (binary files count nothing).
    pub fn totals(&self) -> (u32, u32) {
        self.files
            .iter()
            .filter_map(|f| f.stats)
            .fold((0, 0), |(a, d), (x, y)| (a + x, d + y))
    }
}

/// A diff line's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    /// Line numbers in the old and new version (`None` on the side the
    /// line is not in).
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// git's `@@ -12,7 +12,9 @@` and the function line after it.
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// One file's diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub hunks: Vec<Hunk>,
    /// git reports the file as binary: no hunks.
    pub binary: bool,
    /// The old and new mode when they differ (a permission change).
    pub modes: Option<(String, String)>,
    /// Lines added and removed in what was read.
    pub added: u32,
    pub removed: u32,
    /// Cut at [`DIFF_LINES`] lines or [`DIFF_BYTES`] of output.
    pub truncated: bool,
}

impl Repo {
    /// Message and changed files of a commit: `show -s` and `diff-tree
    /// -r -M --root --raw --numstat -z` (a merge against its first parent).
    /// 5 s each.
    pub fn commit_details(&self, id: &str) -> Result<CommitDetails, GitError> {
        let id = checked_id(id)?;
        let head = self.text(
            &[
                "show",
                "-s",
                "--no-color",
                "--no-show-signature",
                "--no-mailmap",
                "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%B",
                id,
                "--",
            ],
            DETAILS_LIMIT,
        )?;
        let mut f = head.splitn(7, '\u{1f}');
        let mut next = || f.next().unwrap_or("").to_owned();
        let (full, parents, author, author_email, time, committer, message) =
            (next(), next(), next(), next(), next(), next(), next());
        let parents: Vec<String> = parents.split_whitespace().map(str::to_owned).collect();
        let mut args = vec![
            "diff-tree",
            "-r",
            "-M",
            "--root",
            "--no-commit-id",
            "--raw",
            "--numstat",
            "-z",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
        ];
        if let Some(p) = parents.first() {
            args.push(p);
        }
        args.push(&full);
        args.push("--");
        let files = parse_raw_numstat(&self.text(&args, DETAILS_LIMIT)?);
        Ok(CommitDetails {
            id: full.clone(),
            parents: parents.clone(),
            author,
            author_email,
            time: time.trim().parse().unwrap_or(0),
            committer,
            message: message.trim_end().to_owned(),
            files,
        })
    }

    /// The working tree's changed files against HEAD, with their counts
    /// (Uncommitted changes, opened like a commit). Untracked files are
    /// added, without counts. 5 s.
    pub fn uncommitted_files(&self) -> Result<Vec<ChangedFile>, GitError> {
        let status = self.status()?;
        let mut files = if status.head.is_some() {
            parse_raw_numstat(&self.text(
                &[
                    "diff",
                    "-M",
                    "--raw",
                    "--numstat",
                    "-z",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "HEAD",
                    "--",
                ],
                DETAILS_LIMIT,
            )?)
        } else {
            Vec::new()
        };
        for e in status.entries {
            let tracked = files.iter().any(|f| f.path == e.path);
            if !tracked {
                files.push(ChangedFile {
                    kind: match e.mark {
                        super::Mark::Deleted => ChangeKind::Deleted,
                        super::Mark::Renamed => ChangeKind::Renamed,
                        super::Mark::Modified | super::Mark::Conflict => ChangeKind::Modified,
                        super::Mark::Added | super::Mark::Untracked => ChangeKind::Added,
                    },
                    path: e.path,
                    from: e.from,
                    stats: None,
                    is_dir: e.is_dir,
                });
            }
        }
        Ok(files)
    }

    /// One file's diff in a commit, against its first parent (`diff-tree
    /// -p -M -U3`, the rename's source path given too). 5 s, 2 MB.
    pub fn commit_diff(&self, id: &str, file: &ChangedFile) -> Result<Diff, GitError> {
        let id = checked_id(id)?;
        let details_parent = self.first_parent(id)?;
        let mut args = vec![
            "diff-tree",
            "-p",
            "-M",
            "-U3",
            "--root",
            "--no-commit-id",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
        ];
        if let Some(p) = details_parent.as_deref() {
            args.push(p);
        }
        args.push(id);
        args.push("--");
        args.push(checked_path(&file.path)?);
        if let Some(from) = file.from.as_deref() {
            args.push(checked_path(from)?);
        }
        let out = self.run(&args, DIFF_LIMIT, DIFF_BYTES)?;
        Ok(parse_diff(
            &String::from_utf8_lossy(&out.stdout),
            out.truncated,
        ))
    }

    /// A working-tree file's diff against HEAD (`diff -M -U3 HEAD`); an
    /// untracked file reads as added, from the disk. 5 s, 2 MB.
    pub fn worktree_diff(&self, file: &ChangedFile) -> Result<Diff, GitError> {
        let path = checked_path(&file.path)?;
        if file.is_dir {
            return Err(GitError::Other("a folder has no diff".into()));
        }
        if file.kind == ChangeKind::Added && file.stats.is_none() {
            let untracked = self
                .text(
                    &[
                        "ls-files",
                        "-z",
                        "--others",
                        "--exclude-standard",
                        "--",
                        path,
                    ],
                    DIFF_LIMIT,
                )?
                .split('\0')
                .any(|p| p == path);
            if untracked {
                return Ok(added_from_disk(&self.root, path));
            }
        }
        let mut args = vec![
            "diff",
            "-M",
            "-U3",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "HEAD",
            "--",
            path,
        ];
        if let Some(from) = file.from.as_deref() {
            args.push(checked_path(from)?);
        }
        let out = self.run(&args, DIFF_LIMIT, DIFF_BYTES)?;
        Ok(parse_diff(
            &String::from_utf8_lossy(&out.stdout),
            out.truncated,
        ))
    }

    /// A file's content at a commit, in memory (`cat-file blob
    /// <commit>:<path>`): the two sides of an image diff, never written to
    /// disk. At most 50 MB; `Ok(None)` past it. 5 s.
    /// The path is looked up in the commit's tree first: a blob the tree
    /// names but the clone lacks is [`GitError::NotDownloaded`].
    pub fn blob(&self, id: &str, path: &str) -> Result<Option<Vec<u8>>, GitError> {
        let spec = format!("{}:{}", checked_id(id)?, checked_path(path)?);
        let oid = self.text(&["rev-parse", "--verify", "--quiet", &spec], BLOB_LIMIT)?;
        let oid = checked_id(oid.trim())?;
        match self.run(&["cat-file", "blob", oid], BLOB_LIMIT, BLOB_BYTES) {
            Ok(out) => Ok((!out.truncated).then_some(out.stdout)),
            Err(GitError::Other(_)) => Err(GitError::NotDownloaded),
            Err(e) => Err(e),
        }
    }

    /// A file's size at a commit, for "Binary file, 12 KB to 14 KB.". 5 s.
    pub fn blob_size(&self, id: &str, path: &str) -> Result<u64, GitError> {
        let spec = format!("{}:{}", checked_id(id)?, checked_path(path)?);
        let out = self.text(&["cat-file", "-s", &spec], BLOB_LIMIT)?;
        out.trim()
            .parse()
            .map_err(|_| GitError::Other("not a size".into()))
    }

    fn first_parent(&self, id: &str) -> Result<Option<String>, GitError> {
        let out = self.text(
            &["rev-list", "--parents", "-n", "1", id, "--"],
            DETAILS_LIMIT,
        )?;
        Ok(out.split_whitespace().nth(1).map(str::to_owned))
    }
}

/// `--raw --numstat -z`: the raw records (status letters, paths), then
/// the numstat records (counts) in the same order.
fn parse_raw_numstat(out: &str) -> Vec<ChangedFile> {
    let mut files = Vec::new();
    let mut f = out.split('\0').peekable();
    // Raw: ":<modes> <oids> <status>" then one path, or two for R and C.
    while let Some(rec) = f.peek() {
        let Some(meta) = rec.strip_prefix(':') else {
            break;
        };
        let status = meta.rsplit(' ').next().unwrap_or("M").to_owned();
        f.next();
        let two = status.starts_with('R') || status.starts_with('C');
        let first = f.next().unwrap_or("").to_owned();
        let (path, from) = if two {
            (f.next().unwrap_or("").to_owned(), Some(first))
        } else {
            (first, None)
        };
        let kind = match status.chars().next() {
            Some('A') | Some('C') => ChangeKind::Added,
            Some('D') => ChangeKind::Deleted,
            Some('R') => ChangeKind::Renamed,
            _ => ChangeKind::Modified,
        };
        files.push(ChangedFile {
            path,
            from: from.filter(|_| kind == ChangeKind::Renamed),
            kind,
            stats: None,
            is_dir: false,
        });
    }
    // Numstat: "a\td\tpath", or "a\td\t" then the two paths of a rename.
    let mut i = 0;
    while let Some(rec) = f.next() {
        if !rec.contains('\t') {
            continue;
        }
        let mut p = rec.splitn(3, '\t');
        let (a, d, path) = (p.next(), p.next(), p.next().unwrap_or(""));
        if path.is_empty() {
            // A rename: the two paths follow.
            f.next();
            f.next();
        }
        if let Some(file) = files.get_mut(i) {
            file.stats = match (
                a.and_then(|a| a.parse().ok()),
                d.and_then(|d| d.parse().ok()),
            ) {
                (Some(a), Some(d)) => Some((a, d)),
                _ => None,
            };
        }
        i += 1;
    }
    files
}

/// A unified diff of one file, cut at [`DIFF_LINES`] lines.
pub(super) fn parse_diff(out: &str, cut: bool) -> Diff {
    let mut d = Diff {
        truncated: cut,
        ..Diff::default()
    };
    let mut old_mode = None;
    let mut new_mode = None;
    let (mut old, mut new) = (0u32, 0u32);
    // Lines the current hunk still holds, from its header: past them, the
    // lines are headers again (a type change prints two file diffs).
    let (mut old_left, mut new_left) = (0u32, 0u32);
    let mut count = 0usize;
    let mut lines = out.split('\n').peekable();
    // The last line of a cut output may be partial.
    let mut all: Vec<&str> = Vec::new();
    while let Some(l) = lines.next() {
        if lines.peek().is_none() && (cut || l.is_empty()) {
            break;
        }
        all.push(l);
    }
    for l in all {
        let l = l.strip_suffix('\r').unwrap_or(l);
        let in_hunk = old_left > 0 || new_left > 0;
        if !in_hunk && l.starts_with("diff --git ") {
            continue;
        }
        if let (false, Some(h)) = (in_hunk, l.strip_prefix("@@ ")) {
            if count >= DIFF_LINES {
                d.truncated = true;
                break;
            }
            let (o, ol, n, nl) = hunk_range(h);
            (old, old_left, new, new_left) = (o, ol, n, nl);
            d.hunks.push(Hunk {
                header: l.to_owned(),
                lines: Vec::new(),
            });
            continue;
        }
        let hunk = match d.hunks.last_mut() {
            Some(h) if in_hunk || l.starts_with('\\') => h,
            _ => {
                if let Some(m) = l.strip_prefix("old mode ") {
                    old_mode = Some(m.to_owned());
                } else if let Some(m) = l.strip_prefix("new mode ") {
                    new_mode = Some(m.to_owned());
                } else if l.starts_with("Binary files ") || l == "GIT binary patch" {
                    d.binary = true;
                }
                continue;
            }
        };
        if count >= DIFF_LINES {
            d.truncated = true;
            break;
        }
        let (kind, text) = match l.as_bytes().first() {
            Some(b'+') => (LineKind::Added, &l[1..]),
            Some(b'-') => (LineKind::Removed, &l[1..]),
            Some(b' ') => (LineKind::Context, &l[1..]),
            // "\ No newline at end of file"
            Some(b'\\') => continue,
            None => (LineKind::Context, ""),
            _ => continue,
        };
        let (o, n) = match kind {
            LineKind::Added => {
                d.added += 1;
                new += 1;
                new_left = new_left.saturating_sub(1);
                (None, Some(new - 1))
            }
            LineKind::Removed => {
                d.removed += 1;
                old += 1;
                old_left = old_left.saturating_sub(1);
                (Some(old - 1), None)
            }
            LineKind::Context => {
                old += 1;
                new += 1;
                old_left = old_left.saturating_sub(1);
                new_left = new_left.saturating_sub(1);
                (Some(old - 1), Some(new - 1))
            }
        };
        hunk.lines.push(DiffLine {
            kind,
            old: o,
            new: n,
            text: text.to_owned(),
        });
        count += 1;
    }
    if let (Some(o), Some(n)) = (old_mode, new_mode) {
        d.modes = Some((o, n));
    }
    d
}

/// `-12,7 +12,9 @@ fn x` gives (12, 7, 12, 9); a missing count is 1.
fn hunk_range(h: &str) -> (u32, u32, u32, u32) {
    let mut parts = h.split_whitespace();
    let range = |p: Option<&str>, sign: char| {
        let r = p.and_then(|p| p.strip_prefix(sign)).unwrap_or("0,0");
        let mut it = r.split(',');
        let start = it.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let len = it.next().map_or(Some(1), |n| n.parse().ok()).unwrap_or(0);
        (start, len)
    };
    let (o, ol) = range(parts.next(), '-');
    let (n, nl) = range(parts.next(), '+');
    (o, ol, n, nl)
}

/// An untracked file as an added diff: its text when it is text, else a
/// binary diff.
fn added_from_disk(root: &Path, rel: &str) -> Diff {
    let read = super::fs::read_file(root, rel);
    let Ok(super::fs::FileContent::Text {
        text, truncated, ..
    }) = read
    else {
        return Diff {
            binary: true,
            ..Diff::default()
        };
    };
    let mut d = Diff {
        truncated,
        ..Diff::default()
    };
    let mut lines = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if i >= DIFF_LINES {
            d.truncated = true;
            break;
        }
        lines.push(DiffLine {
            kind: LineKind::Added,
            old: None,
            new: Some(i as u32 + 1),
            text: l.to_owned(),
        });
    }
    d.added = lines.len() as u32;
    if !lines.is_empty() {
        d.hunks.push(Hunk {
            header: format!("@@ -0,0 +1,{} @@", lines.len()),
            lines,
        });
    }
    d
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn raw_and_numstat_pair_up() {
        let out = ":100644 100644 a b M\0src/a.rs\0:000000 100644 0 c A\0img.png\0:100644 100644 d e R087\0old.rs\0new.rs\0\
3\t1\tsrc/a.rs\0-\t-\timg.png\x002\t2\t\0old.rs\0new.rs\0";
        let f = parse_raw_numstat(out);
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].stats, Some((3, 1)));
        assert_eq!((f[1].kind, f[1].stats), (ChangeKind::Added, None));
        assert_eq!(f[2].kind, ChangeKind::Renamed);
        assert_eq!(f[2].from.as_deref(), Some("old.rs"));
        assert_eq!(f[2].path, "new.rs");
    }

    #[test]
    fn unified_diff_parses_with_line_numbers() {
        let out = "diff --git a/x b/x\nindex 1..2 100644\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@ fn main\n a\n-b\n+c\n d\n\\ No newline at end of file\n";
        let d = parse_diff(out, false);
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.hunks[0].header, "@@ -1,3 +1,3 @@ fn main");
        let l = &d.hunks[0].lines;
        assert_eq!(l.len(), 4);
        assert_eq!(
            (l[1].kind, l[1].old, l[1].new),
            (LineKind::Removed, Some(2), None)
        );
        assert_eq!(
            (l[2].kind, l[2].old, l[2].new),
            (LineKind::Added, None, Some(2))
        );
        assert_eq!((l[3].old, l[3].new), (Some(3), Some(3)));
        assert_eq!((d.added, d.removed, d.truncated), (1, 1, false));
        let m = parse_diff(
            "diff --git a/s b/s\nold mode 100644\nnew mode 100755\n",
            false,
        );
        assert_eq!(m.modes, Some(("100644".into(), "100755".into())));
        assert!(m.hunks.is_empty());
        // Two file diffs in one output (a type change): the second one's
        // header lines are not hunk lines.
        let two = parse_diff(
            "diff --git a/f b/f\ndeleted file mode 100644\n--- a/f\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-a\n-b\n\
             diff --git a/f b/f\nnew file mode 120000\n--- /dev/null\n+++ b/f\n@@ -0,0 +1 @@\n+t\n\\ No newline at end of file\n",
            false,
        );
        assert_eq!(two.hunks.len(), 2);
        assert_eq!((two.removed, two.added), (2, 1));
        assert_eq!(two.hunks[1].lines[0].text, "t");
    }
}
