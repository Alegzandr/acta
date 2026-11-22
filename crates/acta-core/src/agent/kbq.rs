//! Memory quality on the agent side (ADR 0015): where the session works
//! (its folder and branch), the flags of an entry (the store's and the
//! git ones, read at read time and cached per folder and `HEAD`), sources
//! given as `refs`, the write-time checker's refusals, the richer stale
//! revision reply, and presence hints ("also open in a session on T-51").
//!
//! Git is read only: refs from the files, `git ls-tree` and `git
//! merge-base --is-ancestor` through [`crate::git_cmd::command`] (no
//! optional locks, 2 s limit). A slow or absent git means no git flags,
//! never a slower or failed answer.

use super::git::{self, GitInfo};
use super::render::{self, Fence};
use super::tools::Agent;
use crate::folder_path;
use crate::kb_quality::{KbFlag, Text, Verdict};
use crate::markdown;
use crate::model::{
    Author, ChangeKind, FolderId, KbEntry, KbId, KbKind, KbLinkKind, KbLogEvent, KbPairKind,
    KbPairState, Owner, Record, TopicId,
};
use crate::paths::Os;
use crate::presence::{KB_RECENT, KbTouch};
use crate::store::{KbFinding, KbLinkTarget, KbPlace, KbSource, KbWriteCtx};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Git reads for flags (ADR 0015, Consequences).
const GIT_LIMIT: Duration = Duration::from_secs(2);
/// "Also open" looks this far back (ADR 0015, 6).
const ALSO_OPEN_MS: i64 = 10 * 60_000;
/// Most sources one write may name.
pub const MAX_REFS: usize = 3;
/// Most entries one `supersedes` may name.
pub const MAX_SUPERSEDES: usize = crate::store::MAX_SUPERSEDES;

/// What the session keeps between calls for memory quality.
#[derive(Clone, Debug, Default)]
pub struct KbSession {
    /// The entries this session last read or wrote, newest first (written
    /// to its presence row by the server).
    pub touched: Vec<KbTouch>,
    /// Entries a refusal named in this session (a later edit of one is a
    /// "fixed" outcome).
    pub warned: Vec<KbId>,
    /// Blob ids at `HEAD` per worktree root: `(head, path -> blob)`.
    trees: HashMap<PathBuf, (String, HashMap<String, Option<String>>)>,
    /// Whether a branch is merged into the default one, per root, branch
    /// and the two commits.
    merged: HashMap<(PathBuf, String, String, String), bool>,
}

/// Where the session works: the attached folder containing its directory
/// (or its main checkout's), the worktree, the branch.
#[derive(Clone, Debug)]
pub struct Place {
    pub folder: FolderId,
    /// The git worktree root, else the folder's path.
    pub root: PathBuf,
    pub git: Option<GitInfo>,
}

impl Place {
    pub fn branch(&self) -> Option<&str> {
        self.git.as_ref().and_then(|g| g.branch.as_deref())
    }
}

/// A reason to look again at an entry, with its marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Flag {
    Store(KbFlag),
    FileGone(String),
    FileChanged(String),
    BranchGone(String),
    BranchMerged(String),
}

impl Flag {
    /// At most 30 chars: `?old 140d`, `?file changed`, `?branch merged`.
    pub fn marker(&self) -> String {
        match self {
            Flag::Store(f) => f.marker(),
            Flag::FileGone(_) => "?file gone".into(),
            Flag::FileChanged(_) => "?file changed".into(),
            Flag::BranchGone(_) => "?branch gone".into(),
            Flag::BranchMerged(_) => "?branch merged".into(),
        }
    }

    /// The full form for `kb_read`: the path or branch is stored text, so
    /// it goes inside the fence there.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Flag::FileGone(p) | Flag::FileChanged(p) => Some(p),
            Flag::BranchGone(b) | Flag::BranchMerged(b) => Some(b),
            Flag::Store(_) => None,
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Flag::Store(KbFlag::Conflict(_)) => 0,
            Flag::FileGone(_) | Flag::BranchGone(_) => 1,
            Flag::FileChanged(_) | Flag::BranchMerged(_) => 2,
            Flag::Store(KbFlag::Duplicate(_)) => 3,
            Flag::Store(KbFlag::FolderRemoved) => 4,
            Flag::Store(KbFlag::TopicClosed(_)) => 5,
            Flag::Store(KbFlag::Old { .. }) => 6,
        }
    }
}

fn stored(dir: &Path) -> String {
    folder_path::canonical(dir)
        .unwrap_or_else(|_| folder_path::normalize(Os::CURRENT, &dir.to_string_lossy()))
}

/// `40 s`, `3 min`, `2 h`, `5 d`.
pub fn ago(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    match s {
        0..60 => format!("{s} s"),
        60..3_600 => format!("{} min", s / 60),
        3_600..86_400 => format!("{} h", s / 3_600),
        _ => format!("{} d", s / 86_400),
    }
}

/// The blob ids of `paths` at `HEAD` (repository-relative), by one `git
/// ls-tree`. `None` when git did not answer.
fn ls_tree(root: &Path, paths: &[String]) -> Option<HashMap<String, Option<String>>> {
    let mut cmd = crate::git_cmd::command(root);
    cmd.args(["ls-tree", "--full-tree", "-z", "HEAD", "--"]);
    cmd.args(paths);
    let out = crate::process::output_within(&mut cmd, GIT_LIMIT).ok()??;
    if !out.status.success() {
        return None;
    }
    let mut found: HashMap<String, Option<String>> =
        paths.iter().map(|p| (p.clone(), None)).collect();
    for rec in out.stdout.split(|b| *b == 0) {
        let rec = String::from_utf8_lossy(rec);
        let Some((meta, path)) = rec.split_once('\t') else {
            continue;
        };
        let mut f = meta.split_whitespace();
        let (_, kind, oid) = (f.next(), f.next(), f.next());
        if kind == Some("blob")
            && let Some(oid) = oid
        {
            found.insert(path.to_owned(), Some(oid.to_owned()));
        }
    }
    Some(found)
}

/// A file path as a source names it: relative, forward slashes, no `..`.
fn clean_path(p: &str) -> Option<String> {
    let p = p.trim().replace('\\', "/");
    let p = p.trim_start_matches("./").to_owned();
    (!p.is_empty() && !p.starts_with('/') && !p.contains(':') && !p.split('/').any(|s| s == ".."))
        .then_some(p)
}

impl Agent<'_> {
    /// The session's place, or `None` outside an attached folder of its
    /// project.
    pub(crate) fn place(&self) -> Option<Place> {
        let project = self.session.project?;
        let dir = self.session.dir.as_deref()?;
        let info = git::inspect(dir);
        let mut candidates = vec![stored(dir)];
        if let Some(main) = info.as_ref().and_then(|i| i.main_root.as_deref()) {
            candidates.push(stored(main));
        }
        let folders = self.store.all_folders().ok()?;
        let folder = candidates.iter().find_map(|c| {
            folders
                .iter()
                .filter(|f| f.project == project)
                .filter_map(|f| folder_path::contains(Os::CURRENT, &f.path, c).map(|d| (d, f)))
                .max_by_key(|(d, _)| *d)
                .map(|(_, f)| f.id)
        })?;
        let root = match &info {
            Some(i) => i.root.clone(),
            None => PathBuf::from(&self.store.folder(folder).ok()?.path),
        };
        Some(Place {
            folder,
            root,
            git: info,
        })
    }

    /// The list filter's place (ADR 0015, 4).
    pub(crate) fn kb_place(&self) -> KbPlace {
        match self.place() {
            Some(p) => KbPlace {
                folder: Some(p.folder),
                branch: p.branch().map(str::to_owned),
            },
            None => KbPlace::default(),
        }
    }

    /// Provenance of an agent write (ADR 0015, 5): session id, focus
    /// topic, branch. No parameters: recorded on every write.
    pub(crate) fn write_ctx(&self) -> KbWriteCtx {
        KbWriteCtx {
            session: self.session.session_id.clone(),
            topic: match self.session.focus {
                Some(Owner::Topic(t)) => Some(t),
                _ => None,
            },
            branch: self
                .place()
                .and_then(|p| p.branch().map(str::to_owned))
                .or_else(|| {
                    self.session
                        .dir
                        .as_deref()
                        .and_then(git::inspect)
                        .and_then(|g| g.branch)
                }),
            ..KbWriteCtx::default()
        }
    }

    /// Remembers that this session read or wrote `id` at `rev`.
    pub(crate) fn touch(&mut self, id: KbId, rev: i64) {
        let t = &mut self.session.kb.touched;
        t.retain(|x| x.id != id);
        t.insert(
            0,
            KbTouch {
                id,
                rev,
                at: crate::time::now_ms(),
            },
        );
        t.truncate(KB_RECENT);
    }

    /// `K-4 is also open in a session on T-51 (read 2 min ago).` when
    /// another live session touched `id` in the last 10 minutes.
    pub(crate) fn also_open(&self, id: KbId) -> Option<String> {
        self.session.project?;
        let now = crate::time::now_ms();
        let mut presence = self.store.presence().ok()?;
        let others = presence
            .kb_touched_elsewhere(
                self.session.session_id.as_deref(),
                id,
                now - ALSO_OPEN_MS,
                now,
            )
            .ok()?;
        let (focus, at) = others.first()?;
        let on = focus.map_or(String::new(), |f| format!(" on {f}"));
        Some(format!(
            "{id} is also open in a session{on} ({} ago).",
            ago(now - at)
        ))
    }

    /// The flags of `ids` (store and git), in the order given.
    pub(crate) fn flags(&mut self, ids: &[KbId]) -> Vec<(KbId, Vec<Flag>)> {
        let today = self.session.today();
        let zone = self.session.zone;
        let store_flags = self
            .store
            .kb_store_flags(ids, zone, today)
            .unwrap_or_default();
        let mut out = Vec::with_capacity(ids.len());
        for (id, flags) in store_flags {
            let mut all: Vec<Flag> = flags.into_iter().map(Flag::Store).collect();
            all.extend(self.git_flags(id));
            all.sort_by_key(Flag::rank);
            out.push((id, all));
        }
        out
    }

    /// File and branch flags of one entry (ADR 0015, 1 and 4).
    fn git_flags(&mut self, id: KbId) -> Vec<Flag> {
        let Ok(e) = self.store.kb_entry(id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        // Branch: gone, or merged into the default branch.
        if let (Some(branch), Some(folder)) = (&e.context_branch, e.context_folder)
            && let Ok(f) = self.store.folder(folder)
            && let Some(info) = git::inspect(Path::new(&f.path))
        {
            match git::branch_oid(&info.common_dir, branch) {
                None => out.push(Flag::BranchGone(branch.clone())),
                Some(oid) => {
                    if let Some(def) = info.default_branch.as_deref()
                        && def != branch
                        && let Some(def_oid) = git::branch_oid(&info.common_dir, def)
                        && self.is_merged(&info.root, branch, &oid, def, &def_oid)
                    {
                        out.push(Flag::BranchMerged(branch.clone()));
                    }
                }
            }
        }
        // Files.
        let Ok(links) = self.store.kb_links(id) else {
            return out;
        };
        let mut by_folder: HashMap<FolderId, Vec<(String, Option<String>)>> = HashMap::new();
        for l in links {
            if let (KbLinkKind::File, KbLinkTarget::Text(path), Some(folder)) =
                (l.kind, &l.target, l.folder)
            {
                by_folder
                    .entry(folder)
                    .or_default()
                    .push((path.clone(), l.blob.clone()));
            }
        }
        for (folder, files) in by_folder {
            let Ok(f) = self.store.folder(folder) else {
                continue;
            };
            let dir = PathBuf::from(&f.path);
            match git::inspect(&dir) {
                Some(info) => {
                    let paths: Vec<String> = files.iter().map(|(p, _)| p.clone()).collect();
                    let Some(now) = self.blobs(&info.root, &paths) else {
                        continue;
                    };
                    for (path, recorded) in files {
                        match (now.get(&path).cloned().flatten(), recorded) {
                            (Some(cur), Some(rec)) if cur != rec => {
                                out.push(Flag::FileChanged(path));
                            }
                            (None, _) if !info.root.join(&path).exists() => {
                                out.push(Flag::FileGone(path));
                            }
                            _ => {}
                        }
                    }
                }
                None => {
                    for (path, _) in files {
                        let p = dir.join(&path);
                        match std::fs::metadata(&p) {
                            Err(_) => out.push(Flag::FileGone(path)),
                            Ok(m) => {
                                let modified = m
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map_or(0, |d| d.as_millis() as i64);
                                if modified > e.verified_at {
                                    out.push(Flag::FileChanged(path));
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// Blob ids at `HEAD` for `paths`, from the cache for this `HEAD` or
    /// one `git ls-tree` for the ones it lacks.
    pub(crate) fn blobs(
        &mut self,
        root: &Path,
        paths: &[String],
    ) -> Option<HashMap<String, Option<String>>> {
        let head = git::head_oid(root)?;
        let cache = &mut self.session.kb.trees;
        let entry = cache
            .entry(root.to_path_buf())
            .or_insert_with(|| (head.clone(), HashMap::new()));
        if entry.0 != head {
            *entry = (head, HashMap::new());
        }
        let missing: Vec<String> = paths
            .iter()
            .filter(|p| !entry.1.contains_key(*p))
            .cloned()
            .collect();
        if !missing.is_empty() {
            let found = ls_tree(root, &missing)?;
            entry.1.extend(found);
        }
        Some(
            paths
                .iter()
                .map(|p| (p.clone(), entry.1.get(p).cloned().flatten()))
                .collect(),
        )
    }

    fn is_merged(
        &mut self,
        root: &Path,
        branch: &str,
        oid: &str,
        def: &str,
        def_oid: &str,
    ) -> bool {
        let key = (
            root.to_path_buf(),
            branch.to_owned(),
            oid.to_owned(),
            def_oid.to_owned(),
        );
        if let Some(m) = self.session.kb.merged.get(&key) {
            return *m;
        }
        let mut cmd = crate::git_cmd::command(root);
        cmd.args([
            "merge-base",
            "--is-ancestor",
            &format!("refs/heads/{branch}"),
            &format!("refs/heads/{def}"),
        ]);
        let merged = crate::process::output_within(&mut cmd, GIT_LIMIT)
            .ok()
            .flatten()
            .is_some_and(|o| o.status.success());
        self.session.kb.merged.insert(key, merged);
        merged
    }

    /// `refs` (ADR 0015, 5): at most three comma-separated items: an Acta
    /// id (`T-12`, `T-12#14`, `Q-3`, `N-5`, `K-2`), `commit:<sha>` checked
    /// to exist, or a path relative to the repository root, checked to
    /// exist, with its blob id at `HEAD`.
    pub(crate) fn parse_refs(&mut self, refs: &str) -> Result<Vec<KbSource>, String> {
        let items: Vec<&str> = refs
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if items.len() > MAX_REFS {
            return Err(format!("refs takes at most {MAX_REFS} items"));
        }
        let mut out = Vec::new();
        for item in items {
            let upper = item.to_ascii_uppercase();
            if let Some(sha) = item.strip_prefix("commit:") {
                let sha = sha.trim().to_ascii_lowercase();
                if !(7..=40).contains(&sha.len()) || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err("commit:<sha> takes 7 to 40 hex digits".into());
                }
                let place = self
                    .place()
                    .filter(|p| p.git.is_some())
                    .ok_or("commit: needs a session in an attached git folder")?;
                let mut cmd = crate::git_cmd::command(&place.root);
                cmd.args(["cat-file", "-e", &format!("{sha}^{{commit}}")]);
                let found = crate::process::output_within(&mut cmd, GIT_LIMIT)
                    .ok()
                    .flatten()
                    .is_some_and(|o| o.status.success());
                if !found {
                    return Err(format!("no commit {sha} in this repository"));
                }
                out.push(KbSource::Commit(sha));
                continue;
            }
            let record = if let Some((t, n)) = item.split_once('#') {
                let t = TopicId::parse(t).filter(|_| upper.starts_with("T-"));
                let n = n.trim().parse::<i64>().ok();
                match (t, n) {
                    (Some(t), Some(n)) => Some(Record::Entry(
                        self.store
                            .entry_by_seq(t, n)
                            .map_err(|_| format!("no entry {t}#{n}"))?
                            .id,
                    )),
                    _ => None,
                }
            } else if let Some(t) = TopicId::parse(item).filter(|_| upper.starts_with("T-")) {
                Some(Record::Topic(t))
            } else if let Some(q) =
                crate::model::QuestionId::parse(item).filter(|_| upper.starts_with("Q-"))
            {
                Some(Record::Question(q))
            } else if let Some(n) =
                crate::model::NoteId::parse(item).filter(|_| upper.starts_with("N-"))
            {
                Some(Record::Note(n))
            } else {
                KbId::parse(item)
                    .filter(|_| upper.starts_with("K-"))
                    .map(Record::Kb)
            };
            if let Some(r) = record {
                out.push(KbSource::Record(r));
                continue;
            }
            let path =
                clean_path(item).ok_or("a file ref is a path relative to the repository root")?;
            let place = self
                .place()
                .ok_or("a file ref needs a session in an attached folder")?;
            if !place.root.join(&path).exists() {
                return Err(format!(
                    "no file {} here",
                    render::defuse(&render::clip(&path, 80))
                ));
            }
            let blob = match place.git {
                Some(_) => self
                    .blobs(&place.root, std::slice::from_ref(&path))
                    .and_then(|m| m.get(&path).cloned().flatten()),
                None => None,
            };
            out.push(KbSource::File {
                folder: place.folder,
                path,
                blob,
            });
        }
        Ok(out)
    }

    /// `supersedes`: one to three K ids of this project.
    pub(crate) fn parse_supersedes(&self, text: &str) -> Result<Vec<KbId>, String> {
        let mut ids = Vec::new();
        for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let id = KbId::parse(item)
                .filter(|_| item.to_ascii_uppercase().starts_with("K-"))
                .ok_or("supersedes takes K ids: \"K-9\" or \"K-9, K-12\"")?;
            let e = self.store.kb_entry(id).map_err(super::tools::err)?;
            self.check_project(e.project, item)?;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.is_empty() || ids.len() > MAX_SUPERSEDES {
            return Err(format!("supersedes names 1 to {MAX_SUPERSEDES} K ids"));
        }
        Ok(ids)
    }

    /// The write-time checker (ADR 0015, 2 and 3) for a memory at `owner`.
    /// `Err` is the refusal; `Ok` carries the pairs to keep apart when the
    /// identical call was repeated, and a related id for the ack.
    pub(crate) fn check_memory(
        &mut self,
        owner: Owner,
        draft: &Text<'_>,
        exclude: Option<KbId>,
        replaced: &[KbId],
    ) -> Result<Checked, String> {
        let findings: Vec<KbFinding> = self
            .store
            .kb_check(owner, draft, exclude)
            .map_err(super::tools::err)?
            .into_iter()
            .filter(|f| !replaced.contains(&f.candidate.id))
            .collect();
        if let Some(same) = findings
            .iter()
            .find(|f| f.comparison.verdict == Verdict::Same)
        {
            return Err(format!(
                "not saved: same as {} (rev {}).",
                same.candidate.id, same.candidate.rev
            ));
        }
        let warn: Vec<&KbFinding> = findings
            .iter()
            .filter(|f| matches!(f.comparison.verdict, Verdict::Conflict | Verdict::Duplicate))
            .take(2)
            .collect();
        let related = findings
            .iter()
            .find(|f| f.comparison.verdict == Verdict::Related)
            .map(|f| f.candidate.id);
        if warn.is_empty() {
            if let Some(r) = related {
                let _ = self.store.kb_log(
                    KbLogEvent::RelatedNote,
                    exclude,
                    Some(r),
                    self.session.session_id.as_deref(),
                    Author::Claude,
                );
            }
            return Ok(Checked {
                keep_apart: Vec::new(),
                related,
            });
        }
        let ids: Vec<String> = warn.iter().map(|f| f.candidate.id.to_string()).collect();
        let key = format!(
            "{owner}|{}|{}",
            crate::kb_quality::normalize(draft.title).join(" "),
            ids.join(",")
        );
        let pairs: Vec<(KbId, KbPairKind)> = warn
            .iter()
            .map(|f| {
                (
                    f.candidate.id,
                    if f.comparison.verdict == Verdict::Conflict {
                        KbPairKind::Conflict
                    } else {
                        KbPairKind::Duplicate
                    },
                )
            })
            .collect();
        let previous = self
            .store
            .local_value(REFUSED_KEY)
            .map_err(super::tools::err)?;
        if previous.as_deref() == Some(key.as_str()) {
            self.store
                .set_local_value(REFUSED_KEY, None)
                .map_err(super::tools::err)?;
            return Ok(Checked {
                keep_apart: pairs,
                related,
            });
        }
        self.store
            .set_local_value(REFUSED_KEY, Some(&key))
            .map_err(super::tools::err)?;
        for (other, kind) in &pairs {
            let event = match kind {
                KbPairKind::Conflict => KbLogEvent::ConflictWarning,
                KbPairKind::Duplicate => KbLogEvent::DuplicateWarning,
            };
            let _ = self.store.kb_log(
                event,
                exclude,
                Some(*other),
                self.session.session_id.as_deref(),
                Author::Claude,
            );
            if !self.session.kb.warned.contains(other) {
                self.session.kb.warned.push(*other);
            }
        }
        Err(self.refusal(&warn, exclude))
    }

    /// The duplicate and conflict refusal (ADR 0015, 2): at most two
    /// candidates, their titles and summaries fenced.
    fn refusal(&self, warn: &[&KbFinding], editing: Option<KbId>) -> String {
        let fence = Fence::new();
        let head: Vec<String> = warn
            .iter()
            .map(|f| {
                let c = &f.candidate;
                let what = if f.comparison.verdict == Verdict::Conflict {
                    "possible conflict with"
                } else {
                    "possible duplicate of"
                };
                format!(
                    "{what} {} rev {} ({}, verified {})",
                    c.id,
                    c.rev,
                    super::tools::level_word(c.owner),
                    render::short_date(c.verified_at)
                )
            })
            .collect();
        let body: Vec<String> = warn
            .iter()
            .map(|f| {
                let c = &f.candidate;
                format!(
                    "{} {} - {}",
                    c.id,
                    render::clip(&render::one_line(&c.title), 60),
                    render::clip(&render::one_line(&c.summary), 100)
                )
            })
            .collect();
        let first = &warn[0].candidate;
        let replace = match editing {
            Some(me) => format!(
                "replace it (kb_write id=\"{me}\" with supersedes=\"{}\")",
                first.id
            ),
            None => format!("replace it (supersedes=\"{}\")", first.id),
        };
        let out = format!(
            "not saved: {}\n{}\nReconcile: {replace}, fix it (kb_write id=\"{}\" rev={}), or, if both hold, say in this summary when it applies and repeat the call.",
            head.join("; "),
            fence.wrap(&body.join("\n")),
            first.id,
            first.rev
        );
        render::finish(out, 600)
    }

    /// The stale revision reply (ADR 0015, 6): who changed what and when,
    /// and the current text of a memory or the outline of a document.
    pub(crate) fn stale_reply(&self, cur: &KbEntry, read_rev: i64) -> String {
        let now = crate::time::now_ms();
        let last = self.store.kb_revision(cur.id, cur.rev).ok();
        let who = match &last {
            Some(r) if r.author == Author::Human => "by the user".to_owned(),
            Some(r) => {
                let session = self.store.kb_revision_session(cur.id, r.rev).ok().flatten();
                match (session, self.session.session_id.as_deref()) {
                    (Some(s), Some(me)) if s == me => "by this session".to_owned(),
                    (Some(s), _) => {
                        let on = self
                            .store
                            .presence()
                            .ok()
                            .and_then(|p| p.get(&s).ok().flatten())
                            .and_then(|l| l.focus)
                            .map_or(String::new(), |f| format!(" on {f}"));
                        format!("by another Claude Code session{on}")
                    }
                    (None, _) => "by Claude Code".to_owned(),
                }
            }
            None => String::new(),
        };
        let when = last.as_ref().map_or(String::new(), |r| {
            format!(", {} ago", ago(now - r.created_at))
        });
        let what = last.as_ref().map_or("", |r| match r.change {
            ChangeKind::Edited => "edited",
            ChangeKind::ReplacedSection => "section replaced",
            ChangeKind::AppendedSection => "section appended",
            ChangeKind::Verified => "verified",
            ChangeKind::Moved => "moved",
            ChangeKind::Restored => "restored",
            ChangeKind::Merged => "merged",
            ChangeKind::Superseded => "superseded",
            ChangeKind::Retired => "retired",
            ChangeKind::MadeLive => "made live",
            ChangeKind::Created | ChangeKind::CreatedFromFile => "created",
        });
        let also = self
            .also_open(cur.id)
            .map_or(String::new(), |l| format!("\n{l}"));
        let head = format!(
            "not saved: {} changed since rev {read_rev}: rev {} {who}{when} ({what}).",
            cur.id, cur.rev
        );
        let fence = Fence::new();
        let out = match cur.kind {
            KbKind::Memory => format!(
                "{head} Current text:\n{}\nReconcile with it, then retry with rev={}.{also}",
                fence.wrap(&format!(
                    "Title: {}\n{}",
                    render::one_line(&cur.title),
                    render::clip(&cur.body, 900)
                )),
                cur.rev
            ),
            KbKind::Document => {
                let old = self
                    .store
                    .kb_revision(cur.id, read_rev)
                    .map(|r| r.body)
                    .unwrap_or_default();
                let changed = changed_sections(&old, &cur.body);
                let (plus, minus) = line_delta(&old, &cur.body);
                let sections = match changed.len() {
                    0 => " Changed".to_owned(),
                    1 => format!(" Section {} changed", changed[0]),
                    _ => format!(" Sections {} changed", changed.join(", ")),
                };
                format!(
                    "{head}{sections} (+{plus} -{minus} lines). Outline:\n{}\nRe-read what you change, then retry with rev={}.{also}",
                    fence.wrap(&super::tools::outline_text(&cur.body, 1_000)),
                    cur.rev
                )
            }
        };
        render::finish(out, 1_600)
    }

    /// Records a "keep both": each named pair is `distinct`, so it never
    /// warns again.
    pub(crate) fn keep_apart(&mut self, me: KbId, pairs: &[(KbId, KbPairKind)]) {
        for (other, kind) in pairs {
            let _ = self.store.kb_set_pair_state(
                me,
                *other,
                *kind,
                KbPairState::Distinct,
                Author::Claude,
            );
            let _ = self.store.kb_log(
                KbLogEvent::KeptBoth,
                Some(me),
                Some(*other),
                self.session.session_id.as_deref(),
                Author::Claude,
            );
        }
    }
}

/// The room the promote hint takes in a `log` or `update` ack.
pub const PROMOTE_HINT: usize = 110;

impl Agent<'_> {
    /// Closing a topic that owns memories (ADR 0015, 1): `· T-42 owns 3
    /// memories: promote lasting ones with kb_write(id, rev,
    /// level="epic"|"project")`, once, at the change to done or dropped.
    pub(crate) fn promote_hint(
        &self,
        t: TopicId,
        before: Option<crate::model::Status>,
        after: Option<crate::model::Status>,
    ) -> String {
        use crate::model::Status;
        let closes = matches!(after, Some(Status::Done | Status::Dropped))
            && !matches!(before, Some(Status::Done | Status::Dropped));
        if !closes {
            return String::new();
        }
        let n = self
            .store
            .kb_rollup(
                Owner::Topic(t),
                &crate::store::KbFilter {
                    kind: Some(KbKind::Memory),
                    ..Default::default()
                },
                crate::model::PageReq::first(1),
            )
            .map_or(0, |p| p.total);
        if n == 0 {
            return String::new();
        }
        let what = if n == 1 {
            "1 memory".to_owned()
        } else {
            format!("{n} memories")
        };
        format!(
            "
{t} owns {what}: promote lasting ones with kb_write(id, rev, level=\"epic\"|\"project\")"
        )
    }
}

impl Agent<'_> {
    fn report_project(&self, project: Option<&str>) -> Result<crate::model::ProjectId, String> {
        match project {
            Some(p) => {
                let id = crate::model::ProjectId::parse(p)
                    .filter(|_| p.trim().to_ascii_uppercase().starts_with("P-"))
                    .ok_or("project must be P-n")?;
                self.store.project(id).map_err(super::tools::err)?;
                Ok(id)
            }
            None => self.project(),
        }
    }

    /// `acta kb check` (ADR 0015, 3, "Scan"): each live memory of the
    /// project against its prefilter; found pairs are recorded as open
    /// (a pair kept apart stays so). Never changes an entry.
    pub fn kb_scan(&mut self, project: Option<&str>) -> super::tools::Reply {
        let project = self.report_project(project)?;
        let filter = crate::store::KbFilter {
            kind: Some(KbKind::Memory),
            ..Default::default()
        };
        let mut all = Vec::new();
        loop {
            let page = self
                .store
                .kb_rollup(
                    Owner::Project(project),
                    &filter,
                    crate::model::PageReq {
                        offset: all.len() as i64,
                        limit: crate::model::PageReq::MAX_LIMIT,
                    },
                )
                .map_err(super::tools::err)?;
            let done =
                page.items.is_empty() || all.len() as i64 + page.items.len() as i64 >= page.total;
            all.extend(page.items);
            if done {
                break;
            }
        }
        let mut lines = Vec::new();
        let (mut dups, mut conflicts, mut new) = (0, 0, 0);
        let mut seen = std::collections::HashSet::new();
        for m in &all {
            let Ok(e) = self.store.kb_entry(m.id) else {
                continue;
            };
            let found = self
                .store
                .kb_check(
                    e.owner,
                    &Text {
                        title: &e.title,
                        summary: &e.summary,
                        body: &e.body,
                    },
                    Some(e.id),
                )
                .map_err(super::tools::err)?;
            for f in found {
                let kind = match f.comparison.verdict {
                    Verdict::Conflict => KbPairKind::Conflict,
                    Verdict::Duplicate | Verdict::Same => KbPairKind::Duplicate,
                    _ => continue,
                };
                let key = (
                    e.id.0.min(f.candidate.id.0),
                    e.id.0.max(f.candidate.id.0),
                    kind,
                );
                if !seen.insert(key) {
                    continue;
                }
                let before = self
                    .store
                    .kb_pairs_of(e.id)
                    .map_err(super::tools::err)?
                    .iter()
                    .any(|p| p.other(e.id) == f.candidate.id && p.kind == kind);
                let state = self
                    .store
                    .kb_record_pair(e.id, f.candidate.id, kind, f.comparison.s, self.author_of())
                    .map_err(super::tools::err)?;
                if state != KbPairState::Open {
                    continue;
                }
                new += usize::from(!before);
                match kind {
                    KbPairKind::Duplicate => dups += 1,
                    KbPairKind::Conflict => conflicts += 1,
                }
                lines.push(format!(
                    "{} ~ {} {} {:.2}",
                    e.id,
                    f.candidate.id,
                    if kind == KbPairKind::Conflict {
                        "conflict"
                    } else {
                        "dup"
                    },
                    f.comparison.s
                ));
            }
        }
        let head = format!(
            "Checked {} in {project}: {dups} duplicate and {conflicts} conflict pairs open ({new} new). They show as ?dup and ?conflict in kb.",
            if all.len() == 1 {
                "1 memory".to_owned()
            } else {
                format!("{} memories", all.len())
            }
        );
        let n = render::fitting(&lines, render::budget::FIND - 200);
        let mut out = head;
        for l in &lines[..n] {
            out.push('\n');
            out.push_str(l);
        }
        if n < lines.len() {
            out.push_str(&format!("\n+{} more", lines.len() - n));
        }
        Ok(render::finish(out, render::budget::FIND))
    }

    /// `acta kb quality` (ADR 0015, 8): six lines.
    pub fn kb_quality(&mut self, project: Option<&str>, days: i64) -> super::tools::Reply {
        let project = self.report_project(project)?;
        let days = days.clamp(1, 3_650);
        let since = crate::time::now_ms() - days * crate::time::MS_PER_DAY;
        let r = self
            .store
            .kb_quality_report(project, since, self.session.zone, self.session.today())
            .map_err(super::tools::err)?;
        let pct = |a: i64, b: i64| {
            if b == 0 {
                "n/a".to_owned()
            } else {
                format!("{}%", (a * 100 + b / 2) / b)
            }
        };
        let per100 = if r.claude_writes == 0 {
            "n/a".to_owned()
        } else {
            format!(
                "{:.1}",
                r.human_corrections as f64 * 100.0 / r.claude_writes as f64
            )
        };
        let lines = [
            format!("Memory quality for {project}, last {days} days (this machine's log)."),
            format!(
                "1 Freshness: {} of {} live memories unflagged; old {}, topic closed {}, conflict {}, dup {}, folder removed {}.",
                pct(r.unflagged, r.live_memories),
                r.live_memories,
                r.flags[0],
                r.flags[1],
                r.flags[2],
                r.flags[3],
                r.flags[4]
            ),
            format!(
                "2 Human corrections: {per100} per 100 Claude Code memory writes ({} corrections, {} writes).",
                r.human_corrections, r.claude_writes
            ),
            format!(
                "3 Missed duplicates: {} of the human's {} merges and supersedes were never flagged ({}).",
                r.missed,
                r.human_merges,
                pct(r.missed, r.human_merges)
            ),
            format!(
                "4 Warning noise: {} of {} duplicate and conflict warnings kept both ({}).",
                r.kept_both,
                r.warnings,
                pct(r.kept_both, r.warnings)
            ),
            format!(
                "5 Verification: {} verifies over {} sessions; {} of {} flagged reads then verified, fixed or retired ({}).",
                r.verifies,
                r.sessions,
                r.flagged_acted,
                r.flagged_reads,
                pct(r.flagged_acted, r.flagged_reads)
            ),
            format!(
                "6 Concurrency: {} stale-rev refusals ({} followed by a write), {} automatic merges.",
                r.stale_refused, r.stale_recovered, r.auto_merged
            ),
        ];
        Ok(render::finish(lines.join("\n"), render::budget::FIND))
    }

    fn author_of(&self) -> Author {
        self.session.author
    }
}

/// What the checker let through.
#[derive(Debug, Default)]
pub struct Checked {
    pub keep_apart: Vec<(KbId, KbPairKind)>,
    pub related: Option<KbId>,
}

/// The one local slot of the last refusal (ADR 0015, 3: the key is the
/// target owner, the normalised title and the candidates).
pub const REFUSED_KEY: &str = "kb.refused";

/// Outline numbers whose text differs between two bodies (matched by
/// number and heading).
fn changed_sections(old: &str, new: &str) -> Vec<String> {
    let before: HashMap<String, (String, Option<String>)> = markdown::outline(old)
        .into_iter()
        .map(|s| {
            let text = markdown::section_text(old, &s.number);
            (s.number, (s.title, text))
        })
        .collect();
    markdown::outline(new)
        .into_iter()
        .filter(|s| s.number.matches('.').count() == 0)
        .filter(|s| {
            let text = markdown::section_text(new, &s.number);
            before.get(&s.number) != Some(&(s.title.clone(), text))
        })
        .map(|s| s.number)
        .take(6)
        .collect()
}

/// Lines added and removed, as multisets.
fn line_delta(old: &str, new: &str) -> (usize, usize) {
    let mut count: HashMap<&str, i64> = HashMap::new();
    for l in old.lines() {
        *count.entry(l).or_default() -= 1;
    }
    for l in new.lines() {
        *count.entry(l).or_default() += 1;
    }
    let plus = count.values().filter(|v| **v > 0).sum::<i64>() as usize;
    let minus = count.values().filter(|v| **v < 0).map(|v| -v).sum::<i64>() as usize;
    (plus, minus)
}

/// The section of `new` that is the section `number` of `old`: the same
/// heading and text, found exactly once (ADR 0015, 6's merge assistance).
pub fn matching_section(old: &str, number: &str, new: &str) -> Option<String> {
    let sections = markdown::outline(old);
    let s = sections.iter().find(|s| s.number == number)?;
    let text = markdown::section_text(old, number)?;
    let hits: Vec<String> = markdown::outline(new)
        .into_iter()
        .filter(|n| {
            n.title == s.title && markdown::section_text(new, &n.number).as_ref() == Some(&text)
        })
        .map(|n| n.number)
        .collect();
    match hits.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_are_matched_by_heading_and_text_once() {
        let old = "# A\none\n# B\ntwo\n";
        let new = "# Z\nzero\n# A\none\n# B\ntwo changed\n";
        assert_eq!(matching_section(old, "1", new).as_deref(), Some("2"));
        assert_eq!(matching_section(old, "2", new), None);
        let twice = "# A\none\n# A\none\n";
        assert_eq!(matching_section(old, "1", twice), None);
        assert_eq!(changed_sections(old, new), ["1", "2", "3"]);
        assert_eq!(line_delta("a\nb\n", "a\nc\nd\n"), (2, 1));
    }

    #[test]
    fn ages_read_short() {
        assert_eq!(ago(40_000), "40 s");
        assert_eq!(ago(3 * 60_000), "3 min");
        assert_eq!(ago(2 * 3_600_000), "2 h");
        assert_eq!(ago(3 * 86_400_000), "3 d");
    }

    #[test]
    fn paths_stay_inside_the_repository() {
        assert_eq!(clean_path(".\\src\\kb.rs").as_deref(), Some("src/kb.rs"));
        assert_eq!(clean_path("../x"), None);
        assert_eq!(clean_path("/etc/passwd"), None);
        assert_eq!(clean_path("C:/x"), None);
    }
}
