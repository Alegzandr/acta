//! The commit list (5.R.5): pages of `log --topo-order --parents -z`, the
//! refs snapshot for staying current, and the hash lookup of the search.

use super::{GitError, PAGE_LIMIT, REFS_LIMIT, Repo, checked_path, is_hex_id};

/// Commits read per page.
pub const PAGE_SIZE: usize = 1_000;
/// Commits held at most; past it the list ends with the capped row.
pub const MAX_COMMITS: usize = 100_000;

/// `%H %P %an %cn %at %D %s`, fields split by U+001F, records by NUL. The
/// subject is last, so a separator inside it stays in it.
const FORMAT: &str = "--format=%H%x1f%P%x1f%an%x1f%cn%x1f%at%x1f%D%x1f%s";

/// Which commits the list shows (5.R.5, Branch chooser).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    /// Local and remote-tracking branches, tags and HEAD; never the stash.
    #[default]
    All,
    /// HEAD alone ("Current branch", or "Current commit" when detached).
    Head,
    /// One ref, by its full name (`refs/heads/main`, `refs/remotes/origin/x`).
    Ref(String),
}

/// The filters of the Commits view; they combine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogQuery {
    pub selection: Selection,
    /// Message or author, case-insensitive plain text, either matching.
    pub text: Option<String>,
    /// Only commits that change this path (relative to the root, `/`);
    /// parents are rewritten so the graph is the true graph of the list.
    pub path: Option<String>,
}

/// One commit of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    /// As git gives them: rewritten under a path filter; none for a root
    /// or a shallow clone's boundary.
    pub parents: Vec<String>,
    pub author: String,
    pub committer: String,
    /// Author date, Unix seconds.
    pub time: i64,
    /// The refs on this commit when the page was read, in git's order.
    pub refs: Vec<Ref>,
    /// HEAD is on this commit.
    pub head: bool,
    pub subject: String,
}

impl Commit {
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    /// The first 7 characters of the id.
    pub fn short(&self) -> &str {
        &self.id[..self.id.len().min(7)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// `refs/heads/...`
    Local,
    /// `refs/remotes/...`
    Remote,
    /// `refs/tags/...`
    Tag,
}

/// A ref chip: its kind and short name (`main`, `origin/main`, `v1.0`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ref {
    pub kind: RefKind,
    pub name: String,
}

impl Ref {
    /// From a full name; `None` for the stash, a remote's `HEAD` and refs
    /// outside the three kinds.
    pub fn from_full(full: &str) -> Option<Ref> {
        let (kind, name) = if let Some(n) = full.strip_prefix("refs/heads/") {
            (RefKind::Local, n)
        } else if let Some(n) = full.strip_prefix("refs/remotes/") {
            if n.ends_with("/HEAD") {
                return None;
            }
            (RefKind::Remote, n)
        } else {
            (RefKind::Tag, full.strip_prefix("refs/tags/")?)
        };
        (!name.is_empty()).then(|| Ref {
            kind,
            name: name.to_owned(),
        })
    }

    /// The full name, for [`Selection::Ref`].
    pub fn full(&self) -> String {
        let base = match self.kind {
            RefKind::Local => "refs/heads/",
            RefKind::Remote => "refs/remotes/",
            RefKind::Tag => "refs/tags/",
        };
        format!("{base}{}", self.name)
    }
}

/// A page of the list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogPage {
    pub commits: Vec<Commit>,
    /// Another page follows.
    pub more: bool,
    /// The list stops at [`MAX_COMMITS`] with older commits left unread;
    /// with a text filter, the newest [`MAX_COMMITS`] were searched.
    pub capped: bool,
}

impl Repo {
    /// The page starting `skip` commits in: at most [`PAGE_SIZE`], never
    /// past [`MAX_COMMITS`]. 10 s. No commit yet gives an empty page.
    pub fn log_page(&self, query: &LogQuery, skip: usize) -> Result<LogPage, GitError> {
        if skip >= MAX_COMMITS {
            return Ok(LogPage {
                capped: true,
                ..LogPage::default()
            });
        }
        let want = PAGE_SIZE.min(MAX_COMMITS - skip);
        let text = query
            .text
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(str::to_lowercase);
        let mut args: Vec<String> = [
            "log",
            "--topo-order",
            "--parents",
            "-z",
            "--decorate=full",
            "--no-color",
            "--no-show-signature",
            "--no-mailmap",
        ]
        .map(str::to_owned)
        .to_vec();
        if text.is_some() {
            // git ANDs `--author` with `--grep`; the filter wants either, so
            // the messages are read and matched here, the pages cut after.
            args.push(format!("{FORMAT}%x00%B"));
            args.push(format!("--max-count={MAX_COMMITS}"));
        } else {
            args.push(FORMAT.to_owned());
            args.push(format!("--max-count={}", want + 1));
            args.push(format!("--skip={skip}"));
        }
        let head = self.head_from_files();
        match &query.selection {
            Selection::All => {
                args.extend(["--branches", "--remotes", "--tags"].map(str::to_owned));
                if head.is_some() {
                    args.push("HEAD".into());
                }
            }
            Selection::Head => {
                if head.is_none() {
                    return Ok(LogPage::default());
                }
                args.push("HEAD".into());
            }
            Selection::Ref(r) => {
                if !r.starts_with("refs/") || r.contains(char::is_whitespace) {
                    return Err(GitError::BranchNotFound);
                }
                args.push(r.clone());
            }
        }
        args.push("--".into());
        if let Some(p) = query.path.as_deref().filter(|p| !p.is_empty()) {
            args.push(checked_path(p)?.to_owned());
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = match self.run(&args, PAGE_LIMIT, super::OUTPUT_BYTES) {
            Ok(o) => o,
            // `--branches --remotes --tags` with nothing to show yet.
            Err(GitError::Other(m)) if no_commits(&m) => return Ok(LogPage::default()),
            Err(e) => return Err(e),
        };
        let cut = out.truncated;
        let out = String::from_utf8_lossy(&out.stdout);
        let Some(q) = &text else {
            if cut {
                return Err(GitError::Other("git printed too much".into()));
            }
            let mut commits = parse_page(&out);
            let extra = commits.len() > want;
            commits.truncate(want);
            let end = skip + commits.len();
            return Ok(LogPage {
                commits,
                more: extra && end < MAX_COMMITS,
                capped: extra && end >= MAX_COMMITS,
            });
        };
        let (found, scanned) = matching(&out, q, cut);
        let mut commits: Vec<Commit> = found.into_iter().skip(skip).take(want + 1).collect();
        let extra = commits.len() > want;
        commits.truncate(want);
        // The scan stopped at the newest 100,000 commits (or the output
        // cap): older matches were not looked for.
        let scan_capped = cut || scanned >= MAX_COMMITS;
        Ok(LogPage {
            commits,
            more: extra,
            capped: !extra && scan_capped,
        })
    }

    /// The commit a hexadecimal query of the search names, listed first
    /// (5.R.5, Search field); `None` when it names none.
    pub fn find_commit(&self, query: &str) -> Result<Option<Commit>, GitError> {
        if !is_hex_id(query) {
            return Ok(None);
        }
        let rev = format!("{query}^{{commit}}");
        let r = self.text(
            &[
                "log",
                "--no-walk",
                "-z",
                "--decorate=full",
                "--no-color",
                "--no-show-signature",
                "--no-mailmap",
                FORMAT,
                &rev,
                "--",
            ],
            PAGE_LIMIT,
        );
        match r {
            Ok(out) => Ok(parse_page(&out).into_iter().next()),
            Err(GitError::Other(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The refs and HEAD, compared every 5 s to know when to read the
    /// first page again (5.R.5, Staying current). 2 s.
    pub fn refs(&self) -> Result<RefsSnapshot, GitError> {
        let out = self.text(
            &[
                "for-each-ref",
                "--format=%(objectname) %(*objectname) %(refname) %(upstream)",
            ],
            REFS_LIMIT,
        )?;
        let info = crate::agent::git::inspect(&self.root);
        Ok(RefsSnapshot {
            head: self.head_from_files(),
            branch: info.and_then(|i| i.branch),
            ..parse_refs(&out)
        })
    }

    /// HEAD's commit, read from the files (no process).
    pub fn head_from_files(&self) -> Option<String> {
        crate::agent::git::head_oid(&self.root)
    }

    /// The oids of a shallow clone's boundary commits (`.git/shallow`):
    /// their lines end in a stub (5.R.5, States).
    pub fn shallow_boundary(&self) -> Vec<String> {
        std::fs::read_to_string(self.common_dir.join("shallow"))
            .map(|t| t.lines().map(|l| l.trim().to_owned()).collect())
            .unwrap_or_default()
    }
}

/// The commits of a `{FORMAT}%x00%B` output whose author or message holds
/// `q` (lower case). Records are header and message, each ended by NUL;
/// a cut output drops its last, partial record. Also the count of commits
/// scanned.
fn matching(out: &str, q: &str, cut: bool) -> (Vec<Commit>, usize) {
    let mut parts: Vec<&str> = out.split('\0').collect();
    parts.pop();
    if cut {
        parts.truncate(parts.len() - parts.len() % 2);
        parts.truncate(parts.len().saturating_sub(2));
    }
    let records = parts.as_chunks::<2>().0;
    let found = records
        .iter()
        .filter_map(|[head, message]| {
            let c = parse_page(head).pop()?;
            (c.author.to_lowercase().contains(q) || message.to_lowercase().contains(q)).then_some(c)
        })
        .collect();
    (found, records.len())
}

fn no_commits(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("does not have any commits") || m.contains("bad default revision")
}

/// Parses a page printed with [`FORMAT`] and `-z`.
pub fn parse_page(out: &str) -> Vec<Commit> {
    out.split('\0')
        .filter_map(|rec| {
            let rec = rec.strip_prefix('\n').unwrap_or(rec);
            if rec.is_empty() {
                return None;
            }
            let mut f = rec.splitn(7, '\u{1f}');
            let id = f.next()?.to_owned();
            let parents = f.next()?.split_whitespace().map(str::to_owned).collect();
            let author = f.next()?.to_owned();
            let committer = f.next()?.to_owned();
            let time = f.next()?.trim().parse().unwrap_or(0);
            let (refs, head) = parse_decoration(f.next()?);
            let subject = f.next().unwrap_or("").to_owned();
            Some(Commit {
                id,
                parents,
                author,
                committer,
                time,
                refs,
                head,
                subject,
            })
        })
        .collect()
}

/// `%D` with `--decorate=full`: `HEAD -> refs/heads/main, tag:
/// refs/tags/v1, refs/remotes/origin/main`.
fn parse_decoration(d: &str) -> (Vec<Ref>, bool) {
    let mut head = false;
    let mut refs = Vec::new();
    for part in d.split(", ").map(str::trim).filter(|p| !p.is_empty()) {
        let part = if let Some(r) = part.strip_prefix("HEAD -> ") {
            head = true;
            r
        } else if part == "HEAD" {
            head = true;
            continue;
        } else {
            part
        };
        let part = part.strip_prefix("tag: ").unwrap_or(part);
        refs.extend(Ref::from_full(part));
    }
    (refs, head)
}

/// What staying current compares.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefsSnapshot {
    /// HEAD's commit; `None` before the first commit.
    pub head: Option<String>,
    /// The current branch; `None` when detached.
    pub branch: Option<String>,
    /// Each ref with the commit it names (a tag peeled), in refname order.
    pub refs: Vec<(String, Ref)>,
    /// Local branch short name and its upstream's short name (`main`,
    /// `origin/main`), for the shared chip (5.R.5, Refs).
    pub upstreams: Vec<(String, String)>,
}

impl RefsSnapshot {
    /// The refs on `oid`.
    pub fn at<'a>(&'a self, oid: &'a str) -> impl Iterator<Item = &'a Ref> + 'a {
        self.refs
            .iter()
            .filter(move |(o, _)| o == oid)
            .map(|(_, r)| r)
    }

    /// The upstream of a local branch.
    pub fn upstream(&self, local: &str) -> Option<&str> {
        self.upstreams
            .iter()
            .find(|(l, _)| l == local)
            .map(|(_, u)| u.as_str())
    }

    /// The branch chooser's groups: local, then remote branches.
    pub fn branches(&self, kind: RefKind) -> impl Iterator<Item = &Ref> {
        self.refs
            .iter()
            .map(|(_, r)| r)
            .filter(move |r| r.kind == kind)
    }
}

fn parse_refs(out: &str) -> RefsSnapshot {
    let mut s = RefsSnapshot::default();
    for line in out.lines() {
        let mut f = line.split(' ');
        let (Some(oid), Some(peeled), Some(name)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let Some(r) = Ref::from_full(name) else {
            continue;
        };
        let oid = if peeled.is_empty() { oid } else { peeled };
        if r.kind == RefKind::Local
            && let Some(u) = f.next().and_then(Ref::from_full)
        {
            s.upstreams.push((r.name.clone(), u.name));
        }
        s.refs.push((oid.to_owned(), r));
    }
    s
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn decorations_and_records_parse() {
        let out = "a\u{1f}b c\u{1f}Alexandre\u{1f}Claude\u{1f}1700000000\u{1f}HEAD -> refs/heads/main, tag: refs/tags/v1, refs/remotes/origin/main, refs/remotes/origin/HEAD, refs/stash\u{1f}Fix \u{1f} and \u{0}\nd\u{1f}\u{1f}A\u{1f}A\u{1f}1\u{1f}\u{1f}Été\u{0}";
        let c = parse_page(out);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].parents, vec!["b", "c"]);
        assert!(c[0].head && c[0].is_merge());
        assert_eq!(
            c[0].refs
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "v1", "origin/main"]
        );
        assert_eq!(c[0].subject, "Fix \u{1f} and ");
        assert!(c[1].parents.is_empty() && c[1].refs.is_empty() && !c[1].head);
        assert_eq!(c[1].subject, "Été");
        let s = parse_refs(
            "1 2 refs/tags/v1 \n3  refs/heads/main refs/remotes/origin/main\n3  refs/remotes/origin/main \n",
        );
        assert_eq!(s.refs[0].0, "2", "a tag is peeled");
        assert_eq!(s.upstream("main"), Some("origin/main"));
        assert_eq!(s.at("3").count(), 2);
        // The text filter's records: header NUL message NUL; a cut output
        // drops its partial record and the one before it.
        let rec = |id: &str, msg: &str| {
            format!("{id}\u{1f}\u{1f}Alexandre\u{1f}Alexandre\u{1f}1\u{1f}\u{1f}{msg}\0{msg}\n\0")
        };
        let out = format!(
            "{}{}{}",
            rec("a", "Fix resize"),
            rec("b", "Other"),
            rec("c", "resize again")
        );
        let (found, scanned) = matching(&out, "resize", false);
        assert_eq!((found.len(), scanned), (2, 3));
        let (found, scanned) = matching(&out[..out.len() - 5], "resize", true);
        assert_eq!((found.len(), scanned), (1, 1));
    }
}
