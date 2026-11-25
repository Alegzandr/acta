//! Git marks and Uncommitted changes: `status --porcelain=v2 -z` (5.R.3,
//! Git mark; 5.R.5, Uncommitted changes).

use super::{GitError, Repo, STATUS_LIMIT};
use std::collections::HashMap;

/// A file's git mark (5.R.3). The letter is git's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflict,
}

impl Mark {
    pub fn letter(self) -> char {
        match self {
            Mark::Modified => 'M',
            Mark::Added => 'A',
            Mark::Deleted => 'D',
            Mark::Renamed => 'R',
            Mark::Untracked => 'U',
            Mark::Conflict => 'C',
        }
    }
}

/// One changed path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusEntry {
    /// Relative to the repository root, with `/`.
    pub path: String,
    pub mark: Mark,
    /// A rename's source path.
    pub from: Option<String>,
    /// A whole untracked folder (git lists it once, with a trailing `/`).
    pub is_dir: bool,
}

/// The working tree's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// HEAD's commit; `None` before the first commit.
    pub head: Option<String>,
    /// The current branch; `None` when detached.
    pub branch: Option<String>,
    /// Changed paths in git's order.
    pub entries: Vec<StatusEntry>,
}

impl Status {
    /// The count of the Uncommitted changes row and the folder row.
    pub fn changed(&self) -> usize {
        self.entries.len()
    }

    /// Marks by path, relative to `prefix` (a folder of the repository,
    /// `""` for the root, as [`Repo::prefix`]); paths outside it are left
    /// out. Used by the Files tree, whose root is the folder shown.
    pub fn marks_under(&self, prefix: &str) -> HashMap<String, Mark> {
        let prefix = prefix.trim_end_matches('/');
        self.entries
            .iter()
            .filter_map(|e| {
                let rel = if prefix.is_empty() {
                    e.path.as_str()
                } else {
                    e.path.strip_prefix(prefix)?.strip_prefix('/')?
                };
                Some((rel.to_owned(), e.mark))
            })
            .collect()
    }
}

impl Repo {
    /// `status --porcelain=v2 -z --branch --untracked-files=normal`, 2 s.
    pub fn status(&self) -> Result<Status, GitError> {
        let out = self.text(
            &[
                "status",
                "--porcelain=v2",
                "-z",
                "--branch",
                "--untracked-files=normal",
                "--ignored=no",
            ],
            STATUS_LIMIT,
        )?;
        Ok(parse_status(&out))
    }
}

/// Parses `status --porcelain=v2 -z --branch`.
pub(super) fn parse_status(out: &str) -> Status {
    let mut s = Status::default();
    let mut fields = out.split('\0');
    while let Some(rec) = fields.next() {
        if let Some(h) = rec.strip_prefix("# branch.oid ") {
            s.head = (h != "(initial)").then(|| h.to_owned());
        } else if let Some(h) = rec.strip_prefix("# branch.head ") {
            s.branch = (h != "(detached)").then(|| h.to_owned());
        } else if let Some(rest) = rec.strip_prefix("1 ") {
            // XY sub mH mI mW hH hI path
            let mut p = rest.splitn(8, ' ');
            let xy = p.next().unwrap_or("..");
            if let Some(path) = p.nth(6) {
                s.entries.push(entry(path, ordinary(xy), None));
            }
        } else if let Some(rest) = rec.strip_prefix("2 ") {
            // XY sub mH mI mW hH hI Xscore path, then the source path.
            let mut p = rest.splitn(9, ' ');
            let xy = p.next().unwrap_or("..");
            let from = fields.next().map(str::to_owned);
            if let Some(path) = p.nth(7) {
                let mark = if xy.contains('R') || xy.contains('C') {
                    Mark::Renamed
                } else {
                    ordinary(xy)
                };
                s.entries.push(entry(path, mark, from));
            }
        } else if let Some(rest) = rec.strip_prefix("u ") {
            // XY sub m1 m2 m3 mW h1 h2 h3 path
            if let Some(path) = rest.splitn(10, ' ').nth(9) {
                s.entries.push(entry(path, Mark::Conflict, None));
            }
        } else if let Some(path) = rec.strip_prefix("? ") {
            s.entries.push(entry(path, Mark::Untracked, None));
        }
    }
    s
}

fn ordinary(xy: &str) -> Mark {
    let mut c = xy.chars();
    let (x, y) = (c.next().unwrap_or('.'), c.next().unwrap_or('.'));
    if x == 'D' || y == 'D' {
        Mark::Deleted
    } else if x == 'A' {
        Mark::Added
    } else {
        Mark::Modified
    }
}

fn entry(path: &str, mark: Mark, from: Option<String>) -> StatusEntry {
    let is_dir = path.ends_with('/');
    StatusEntry {
        path: path.trim_end_matches('/').to_owned(),
        mark,
        from,
        is_dir,
    }
}
