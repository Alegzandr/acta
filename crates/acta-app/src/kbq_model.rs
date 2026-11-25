//! Memory quality as the app shows it (docs/design/components-knowledge.md
//! 5, Memory quality; ADR 0015): the reasons an entry is to check, in the
//! table's order, and their lines; the one notice an entry shows; which
//! entry survives a merge; the "To check" list that never loses a row
//! under the reader; the provenance line; and the section-level merge of
//! a save made over a newer revision.

use std::collections::{HashMap, HashSet};

use acta_core::agent::kbq::Flag;
use acta_core::kb_quality::KbFlag;
use acta_core::model::{
    Author, ChangeKind, EpicId, KbEntry, KbId, KbKind, KbState, Owner, Status, TopicId,
};
use acta_core::store::KbRevisionSummary;

use crate::i18n::{Arg, Key, Locale, text, tr};

/// A reason to check an entry, in the order of the table (most telling
/// first). A file reason names its first file and counts the others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    Conflict(KbId),
    FileGone { file: String, more: usize },
    BranchGone(String),
    FileChanged { file: String, more: usize },
    BranchMerged(String),
    Duplicate(KbId),
    FolderRemoved,
    TopicClosed(Status),
    Old { days: i64 },
}

impl Reason {
    fn rank(&self) -> u8 {
        match self {
            Reason::Conflict(_) => 0,
            Reason::FileGone { .. } => 1,
            Reason::BranchGone(_) => 2,
            Reason::FileChanged { .. } => 3,
            Reason::BranchMerged(_) => 4,
            Reason::Duplicate(_) => 5,
            Reason::FolderRemoved => 6,
            Reason::TopicClosed(_) => 7,
            Reason::Old { .. } => 8,
        }
    }

    /// Mark as still true clears it (Old, File changed, File gone).
    pub fn verify_clears(&self) -> bool {
        matches!(
            self,
            Reason::Old { .. } | Reason::FileChanged { .. } | Reason::FileGone { .. }
        )
    }

    /// The tooltip and notice line: one whole catalogue string.
    pub fn line(&self, locale: Locale) -> String {
        match self {
            Reason::Conflict(id) => tr(
                locale,
                Key::KbReasonConflict,
                &[("id", Arg::Text(&id.to_string()))],
            ),
            Reason::Duplicate(id) => tr(
                locale,
                Key::KbReasonDuplicate,
                &[("id", Arg::Text(&id.to_string()))],
            ),
            Reason::FileGone { file, more } => file_line(
                locale,
                file,
                *more,
                Key::KbReasonFileGone,
                Key::KbReasonFileGoneMore,
            ),
            Reason::FileChanged { file, more } => file_line(
                locale,
                file,
                *more,
                Key::KbReasonFileChanged,
                Key::KbReasonFileChangedMore,
            ),
            Reason::BranchGone(b) => {
                tr(locale, Key::KbReasonBranchGone, &[("branch", Arg::Text(b))])
            }
            Reason::BranchMerged(b) => tr(
                locale,
                Key::KbReasonBranchMerged,
                &[("branch", Arg::Text(b))],
            ),
            Reason::FolderRemoved => text(locale, Key::KbReasonFolderRemoved),
            Reason::TopicClosed(Status::Dropped) => text(locale, Key::KbReasonTopicDropped),
            Reason::TopicClosed(_) => text(locale, Key::KbReasonTopicDone),
            Reason::Old { days } => tr(
                locale,
                Key::KbReasonOld,
                &[("days", Arg::Count((*days).max(0) as u64))],
            ),
        }
    }
}

/// The last segment of a stored path (a reason names the file only).
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn file_line(locale: Locale, file: &str, more: usize, one: Key, many: Key) -> String {
    let name = file_name(file);
    if more == 0 {
        tr(locale, one, &[("file", Arg::Text(name))])
    } else {
        tr(
            locale,
            many,
            &[("file", Arg::Text(name)), ("n", Arg::Count(more as u64))],
        )
    }
}

/// An entry's flags (the store's and git's) as reasons, in the table's
/// order, file flags of one kind folded into one line.
pub fn reasons(flags: &[Flag]) -> Vec<Reason> {
    let mut out: Vec<Reason> = Vec::new();
    for flag in flags {
        let reason = match flag {
            Flag::Store(KbFlag::Conflict(id)) => Reason::Conflict(*id),
            Flag::Store(KbFlag::Duplicate(id)) => Reason::Duplicate(*id),
            Flag::Store(KbFlag::FolderRemoved) => Reason::FolderRemoved,
            Flag::Store(KbFlag::TopicClosed(s)) => Reason::TopicClosed(*s),
            Flag::Store(KbFlag::Old { days }) => Reason::Old { days: *days },
            Flag::BranchGone(b) => Reason::BranchGone(b.clone()),
            Flag::BranchMerged(b) => Reason::BranchMerged(b.clone()),
            Flag::FileGone(f) | Flag::FileChanged(f) => {
                let gone = matches!(flag, Flag::FileGone(_));
                let same = out.iter_mut().find_map(|r| match r {
                    Reason::FileGone { more, .. } if gone => Some(more),
                    Reason::FileChanged { more, .. } if !gone => Some(more),
                    _ => None,
                });
                if let Some(more) = same {
                    *more += 1;
                    continue;
                }
                let file = f.clone();
                if gone {
                    Reason::FileGone { file, more: 0 }
                } else {
                    Reason::FileChanged { file, more: 0 }
                }
            }
        };
        out.push(reason);
    }
    out.sort_by_key(Reason::rank);
    out
}

/// The reasons left once the human marked the entry as still true.
pub fn after_verify(reasons: &[Reason]) -> Vec<Reason> {
    reasons
        .iter()
        .filter(|r| !r.verify_clears())
        .cloned()
        .collect()
}

/// Which entry survives a merge from the duplicate notice: the older one
/// (the lower id), so the action names its direction. Returns `(survivor,
/// merged)`.
pub fn merge_direction(this: KbId, other: KbId) -> (KbId, KbId) {
    if this.0 < other.0 {
        (this, other)
    } else {
        (other, this)
    }
}

/// The duplicate notice's merge label: "Merge into K-4" on the newer
/// entry, "Merge K-11 into this" on the older.
pub fn merge_label(locale: Locale, this: KbId, other: KbId) -> String {
    let id = other.to_string();
    if merge_direction(this, other).0 == this {
        tr(locale, Key::KbMergeHere, &[("id", Arg::Text(&id))])
    } else {
        tr(locale, Key::KbMergeIntoId, &[("id", Arg::Text(&id))])
    }
}

/// What the notice of a reasons-only entry offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeAction {
    Verify,
    /// Move to the epic (or the project) above its closed topic.
    MoveUp(Owner),
    Retire,
}

/// The one notice an entry shows, the first that applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Superseded {
        by: KbId,
        at: i64,
    },
    Retired {
        by: Author,
        at: i64,
        reason: Option<String>,
    },
    Conflict(KbId),
    Duplicate(KbId),
    Reasons(Vec<Reason>, NoticeAction),
}

/// The newest revision of `change` in a history (newest first).
fn newest(history: &[KbRevisionSummary], change: ChangeKind) -> Option<&KbRevisionSummary> {
    history.iter().find(|h| h.change == change)
}

/// Picks the notice (components-knowledge.md 5, Notice). `above` is the
/// level a closed topic's entry moves up to (its epic, else its project).
pub fn notice(
    entry: &KbEntry,
    history: &[KbRevisionSummary],
    reasons: &[Reason],
    above: Option<Owner>,
) -> Option<Notice> {
    match entry.state {
        KbState::Superseded => {
            let at = newest(history, ChangeKind::Superseded)
                .map_or(entry.stamp.updated_at, |h| h.created_at);
            return entry.superseded_by.map(|by| Notice::Superseded { by, at });
        }
        KbState::Retired => {
            let rev = newest(history, ChangeKind::Retired);
            return Some(Notice::Retired {
                by: rev.map_or(entry.stamp.updated_by, |h| h.author),
                at: rev.map_or(entry.stamp.updated_at, |h| h.created_at),
                reason: rev
                    .and_then(|h| h.section.clone())
                    .filter(|s| !s.is_empty()),
            });
        }
        KbState::Live => {}
    }
    if let Some(id) = reasons.iter().find_map(|r| match r {
        Reason::Conflict(id) => Some(*id),
        _ => None,
    }) {
        return Some(Notice::Conflict(id));
    }
    if let Some(id) = reasons.iter().find_map(|r| match r {
        Reason::Duplicate(id) => Some(*id),
        _ => None,
    }) {
        return Some(Notice::Duplicate(id));
    }
    if reasons.is_empty() {
        return None;
    }
    let action = if reasons.iter().any(Reason::verify_clears) {
        NoticeAction::Verify
    } else if reasons.iter().any(|r| matches!(r, Reason::TopicClosed(_)))
        && let Some(above) = above
    {
        NoticeAction::MoveUp(above)
    } else {
        NoticeAction::Retire
    };
    Some(Notice::Reasons(reasons.to_vec(), action))
}

/// The "To check" list: ordered by the first reason in the table's order,
/// then by verified date, oldest first.
pub fn to_check_order(flagged: &mut [(KbId, Vec<Reason>, i64)]) {
    flagged.sort_by(|a, b| {
        let first = |r: &[Reason]| r.first().map_or(u8::MAX, Reason::rank);
        first(&a.1)
            .cmp(&first(&b.1))
            .then(a.2.cmp(&b.2))
            .then(a.0.0.cmp(&b.0.0))
    });
}

/// A row never leaves under the reader (components-knowledge.md 5): the
/// rows already listed stay where they are; a newly flagged entry (a git
/// reason arriving late) is inserted after the listed row that precedes
/// it in `fresh`, or first.
pub fn keep_listed(listed: &[KbId], fresh: &[KbId]) -> Vec<KbId> {
    let mut out: Vec<KbId> = listed.to_vec();
    for (i, id) in fresh.iter().enumerate() {
        if out.contains(id) {
            continue;
        }
        let after = fresh[..i]
            .iter()
            .rev()
            .find_map(|prev| out.iter().position(|x| x == prev));
        match after {
            Some(at) => out.insert(at + 1, *id),
            None => out.insert(0, *id),
        }
    }
    out
}

/// A segment of the provenance line, each a whole catalogue string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// "Written by Claude Code 19 Nov 2026", when it changed since.
    Written { by: Author, at: i64 },
    /// "From topic T-12": the topic Claude Code worked on.
    From(TopicId),
    /// "On branch feature/kb": where it was written, off the default.
    Branch(String),
    /// "Checked 2 Nov 2026 by Alexandre": a Mark as still true.
    Checked { by: Author, at: i64 },
}

/// Whether a change asserts the text (sets the verified date).
fn asserts(change: ChangeKind) -> bool {
    matches!(
        change,
        ChangeKind::Created
            | ChangeKind::CreatedFromFile
            | ChangeKind::Edited
            | ChangeKind::ReplacedSection
            | ChangeKind::AppendedSection
            | ChangeKind::Restored
            | ChangeKind::Merged
            | ChangeKind::Verified
    )
}

/// The provenance line's segments (components-knowledge.md 5,
/// Provenance). `history` is newest first; `defaults` are the default
/// branches of the project's repositories (a branch among them is not
/// shown).
pub fn provenance(
    entry: &KbEntry,
    history: &[KbRevisionSummary],
    defaults: &[String],
) -> Vec<Provenance> {
    let mut out = Vec::new();
    if entry.rev > 1 {
        out.push(Provenance::Written {
            by: entry.created_by,
            at: entry.stamp.created_at,
        });
    }
    if let Some(t) = entry.created_topic
        && entry.owner != Owner::Topic(t)
    {
        out.push(Provenance::From(t));
    }
    let first_branch = history.last().and_then(|h| h.branch.clone());
    if entry.context_branch.is_none()
        && let Some(b) = first_branch
        && !defaults.contains(&b)
    {
        out.push(Provenance::Branch(b));
    }
    if history
        .iter()
        .find(|h| asserts(h.change))
        .is_some_and(|h| h.change == ChangeKind::Verified)
    {
        out.push(Provenance::Checked {
            by: entry.verified_by,
            at: entry.verified_at,
        });
    }
    out
}

/// The `.document` / `.memory` key of a pair.
pub fn by_kind(kind: KbKind, document: Key, memory: Key) -> Key {
    match kind {
        KbKind::Document => document,
        KbKind::Memory => memory,
    }
}

/// A body cut into its preamble and sections, each with its heading line.
fn sections(body: &str) -> Vec<(Option<String>, String)> {
    crate::kb_model::body_parts(body)
        .into_iter()
        .map(|(h, text)| {
            (
                h.map(|_| text.lines().next().unwrap_or("").to_owned()),
                text,
            )
        })
        .collect()
}

/// A save made over a newer revision (components-knowledge.md 5, Saving
/// over a newer revision): when both sides kept the title, the summary and
/// the headings, and changed different sections, the merged body (each
/// section from whoever changed it). `None`: they overlap, or one side
/// changed the whole body, the title or the summary.
pub fn merge_over(
    base: (&str, &str, &str),
    mine: (&str, &str, &str),
    theirs: (&str, &str, &str),
) -> Option<String> {
    let (bt, bs, bb) = base;
    let (mt, ms, mb) = mine;
    let (tt, ts, tb) = theirs;
    if mt.trim() != bt.trim()
        || ms.trim() != bs.trim()
        || tt.trim() != bt.trim()
        || ts.trim() != bs.trim()
    {
        return None;
    }
    let (b, m, t) = (sections(bb), sections(mb), sections(tb));
    let heads = |v: &[(Option<String>, String)]| v.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    // One section only (no heading): a whole-body change on both sides.
    if b.len() < 2 || heads(&b) != heads(&m) || heads(&b) != heads(&t) {
        return None;
    }
    let mut parts = Vec::with_capacity(b.len());
    for ((base, mine), theirs) in b.iter().zip(&m).zip(&t) {
        let part = match (mine.1 == base.1, theirs.1 == base.1) {
            (true, _) => &theirs.1,
            (false, true) => &mine.1,
            (false, false) if mine.1 == theirs.1 => &mine.1,
            (false, false) => return None,
        };
        parts.push(part.clone());
    }
    Some(join_parts(&parts, mb))
}

fn join_parts(parts: &[String], like: &str) -> String {
    let mut out = parts.join("\n");
    if like.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The flags a page knows, the store's (with the page's query) and git's
/// (read later, off the UI thread), and the entries just marked as still
/// true (their mark goes at once; the write follows).
#[derive(Clone, Debug, Default)]
pub struct FlagSet {
    store: HashMap<KbId, Vec<Flag>>,
    git: HashMap<KbId, Vec<Flag>>,
    verified: HashSet<KbId>,
}

impl FlagSet {
    pub fn set_store(&mut self, flags: Vec<(KbId, Vec<Flag>)>) {
        self.store = flags.into_iter().collect();
    }

    /// Git's flags of these entries, read after the write a verify made.
    pub fn set_git(&mut self, flags: Vec<(KbId, Vec<Flag>)>) {
        for (id, f) in flags {
            self.verified.remove(&id);
            self.git.insert(id, f);
        }
    }

    pub fn mark_verified(&mut self, id: KbId) {
        self.verified.insert(id);
    }

    /// Git's flags of one entry as read (a source's "changed since
    /// checked"), none once it was just marked as still true.
    pub fn git_of(&self, id: KbId) -> Vec<Flag> {
        if self.verified.contains(&id) {
            return Vec::new();
        }
        self.git.get(&id).cloned().unwrap_or_default()
    }

    pub fn reasons(&self, id: KbId) -> Vec<Reason> {
        let mut flags: Vec<Flag> = self.store.get(&id).cloned().unwrap_or_default();
        flags.extend(self.git.get(&id).cloned().unwrap_or_default());
        let all = reasons(&flags);
        if self.verified.contains(&id) {
            after_verify(&all)
        } else {
            all
        }
    }
}

/// Whether two levels share a scope (the merge picker: its level, the
/// levels above and the levels below). `epic_of` is a topic's epic.
pub fn shares_scope(a: Owner, b: Owner, epic_of: impl Fn(TopicId) -> Option<EpicId>) -> bool {
    let holds = |wide: Owner, narrow: Owner| match (wide, narrow) {
        (Owner::Project(_), _) => true,
        (Owner::Epic(e), Owner::Topic(t)) => epic_of(t) == Some(e),
        _ => false,
    };
    a == b || holds(a, b) || holds(b, a)
}

/// What a session's revisions say about it, for a history tooltip: the
/// topic of the session if known (the entry's creating session, or a
/// live session's focus).
pub fn session_topic(
    session: Option<&str>,
    entry: &KbEntry,
    live: &HashMap<String, TopicId>,
) -> Option<TopicId> {
    let s = session?;
    if let Some(t) = live.get(s) {
        return Some(*t);
    }
    (entry.created_session.as_deref() == Some(s))
        .then_some(entry.created_topic)
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{ProjectId, Stamp};

    fn entry(id: i64) -> KbEntry {
        KbEntry {
            id: KbId(id),
            kind: KbKind::Memory,
            owner: Owner::Topic(TopicId(3)),
            project: ProjectId(1),
            title: "Shell detection".into(),
            summary: "Use COMSPEC on Windows".into(),
            body: "COMSPEC names cmd.exe.".into(),
            rev: 1,
            created_by: Author::Claude,
            stamp: Stamp {
                uuid: String::new(),
                created_at: 1_000,
                updated_at: 1_000,
                updated_by: Author::Claude,
                deleted_at: None,
                deleted_by: None,
            },
            verified_at: 1_000,
            verified_by: Author::Claude,
            state: KbState::Live,
            superseded_by: None,
            context_branch: None,
            context_folder: None,
            created_session: None,
            created_topic: None,
        }
    }

    fn rev(rev: i64, change: ChangeKind, author: Author, at: i64) -> KbRevisionSummary {
        KbRevisionSummary {
            rev,
            author,
            change,
            section: None,
            size: 0,
            created_at: at,
            session: None,
            branch: None,
        }
    }

    #[test]
    fn reasons_follow_the_table_and_fold_files() {
        let flags = vec![
            Flag::Store(KbFlag::Old { days: 140 }),
            Flag::FileChanged("crates/a/src/kb.rs".into()),
            Flag::Store(KbFlag::Duplicate(KbId(4))),
            Flag::FileChanged("crates/a/src/b.rs".into()),
            Flag::BranchGone("feature/kb".into()),
            Flag::FileGone("src/gone.rs".into()),
            Flag::Store(KbFlag::Conflict(KbId(9))),
        ];
        let r = reasons(&flags);
        assert_eq!(
            r,
            vec![
                Reason::Conflict(KbId(9)),
                Reason::FileGone {
                    file: "src/gone.rs".into(),
                    more: 0
                },
                Reason::BranchGone("feature/kb".into()),
                Reason::FileChanged {
                    file: "crates/a/src/kb.rs".into(),
                    more: 1
                },
                Reason::Duplicate(KbId(4)),
                Reason::Old { days: 140 },
            ]
        );
        assert_eq!(
            r[3].line(Locale::En),
            "kb.rs and 1 more file changed since they were checked"
        );
        assert_eq!(r[0].line(Locale::En), "May contradict K-9");
        assert_eq!(r[5].line(Locale::Fr), "Non vérifiée depuis 140 jours");
        assert_eq!(
            Reason::TopicClosed(Status::Dropped).line(Locale::En),
            "Its topic was dropped"
        );
        // Mark as still true clears age and files, not the rest.
        assert_eq!(
            after_verify(&r),
            vec![
                Reason::Conflict(KbId(9)),
                Reason::BranchGone("feature/kb".into()),
                Reason::Duplicate(KbId(4)),
            ]
        );
    }

    #[test]
    fn the_older_entry_survives_a_merge_and_the_label_says_which_way() {
        assert_eq!(merge_direction(KbId(11), KbId(4)), (KbId(4), KbId(11)));
        assert_eq!(merge_direction(KbId(4), KbId(11)), (KbId(4), KbId(11)));
        assert_eq!(merge_label(Locale::En, KbId(11), KbId(4)), "Merge into K-4");
        assert_eq!(
            merge_label(Locale::En, KbId(4), KbId(11)),
            "Merge K-11 into this"
        );
        assert_eq!(
            merge_label(Locale::Fr, KbId(11), KbId(4)),
            "Fusionner dans K-4"
        );
    }

    #[test]
    fn one_notice_per_entry_the_first_that_applies() {
        let e = entry(11);
        assert_eq!(notice(&e, &[], &[], None), None);
        let both = vec![Reason::Conflict(KbId(9)), Reason::Duplicate(KbId(4))];
        assert_eq!(
            notice(&e, &[], &both, None),
            Some(Notice::Conflict(KbId(9)))
        );
        let dup = vec![
            Reason::FileChanged {
                file: "a.rs".into(),
                more: 0,
            },
            Reason::Duplicate(KbId(4)),
        ];
        assert_eq!(
            notice(&e, &[], &dup, None),
            Some(Notice::Duplicate(KbId(4)))
        );
        // Reasons: verify when one clears, else move up, else retire.
        let closed = vec![Reason::TopicClosed(Status::Done)];
        let epic = Owner::Epic(acta_core::model::EpicId(2));
        assert_eq!(
            notice(&e, &[], &closed, Some(epic)),
            Some(Notice::Reasons(closed.clone(), NoticeAction::MoveUp(epic)))
        );
        let old = vec![Reason::TopicClosed(Status::Done), Reason::Old { days: 95 }];
        assert_eq!(
            notice(&e, &[], &old, Some(epic)),
            Some(Notice::Reasons(old.clone(), NoticeAction::Verify))
        );
        let merged = vec![Reason::BranchMerged("feature/kb".into())];
        assert_eq!(
            notice(&e, &[], &merged, None),
            Some(Notice::Reasons(merged.clone(), NoticeAction::Retire))
        );
        // A superseded or retired entry shows its state, never a reason.
        let mut gone = entry(9);
        gone.state = KbState::Superseded;
        gone.superseded_by = Some(KbId(21));
        let history = vec![
            rev(3, ChangeKind::Superseded, Author::Human, 5_000),
            rev(2, ChangeKind::Edited, Author::Claude, 2_000),
        ];
        assert_eq!(
            notice(&gone, &history, &both, None),
            Some(Notice::Superseded {
                by: KbId(21),
                at: 5_000
            })
        );
        gone.state = KbState::Retired;
        gone.superseded_by = None;
        let mut retired = rev(3, ChangeKind::Retired, Author::Claude, 6_000);
        retired.section = Some("The store moved to schema 7.".into());
        assert_eq!(
            notice(&gone, &[retired], &[], None),
            Some(Notice::Retired {
                by: Author::Claude,
                at: 6_000,
                reason: Some("The store moved to schema 7.".into())
            })
        );
    }

    #[test]
    fn the_to_check_list_orders_by_reason_then_age_and_keeps_its_rows() {
        let mut flagged = vec![
            (KbId(1), vec![Reason::Old { days: 100 }], 300),
            (KbId(2), vec![Reason::Old { days: 200 }], 100),
            (KbId(3), vec![Reason::Conflict(KbId(1))], 900),
            (KbId(4), vec![Reason::Duplicate(KbId(5))], 50),
        ];
        to_check_order(&mut flagged);
        let ids: Vec<i64> = flagged.iter().map(|f| f.0.0).collect();
        assert_eq!(ids, vec![3, 4, 2, 1]);
        // Verified rows stay; a git reason arriving late is inserted in
        // its place without moving the others.
        let listed = vec![KbId(3), KbId(4), KbId(2), KbId(1)];
        let fresh = vec![KbId(3), KbId(7), KbId(1)];
        assert_eq!(
            keep_listed(&listed, &fresh),
            vec![KbId(3), KbId(7), KbId(4), KbId(2), KbId(1)]
        );
        assert_eq!(
            keep_listed(&[], &[KbId(8), KbId(9)]),
            vec![KbId(8), KbId(9)]
        );
        assert_eq!(
            keep_listed(&[KbId(1)], &[KbId(6), KbId(1)]),
            vec![KbId(6), KbId(1)]
        );
    }

    #[test]
    fn provenance_shows_only_what_the_meta_line_does_not() {
        let mut e = entry(11);
        e.created_session = Some("s1".into());
        e.created_topic = Some(TopicId(12));
        let first = KbRevisionSummary {
            branch: Some("feature/terminal".into()),
            ..rev(1, ChangeKind::Created, Author::Claude, 1_000)
        };
        // Unchanged since written, by its own topic's session, on main.
        e.created_topic = Some(TopicId(3));
        let on_main = KbRevisionSummary {
            branch: Some("main".into()),
            ..first.clone()
        };
        assert!(provenance(&e, &[on_main], &["main".into()]).is_empty());
        e.created_topic = Some(TopicId(12));
        e.rev = 3;
        e.verified_by = Author::Human;
        e.verified_at = 9_000;
        let history = vec![
            rev(3, ChangeKind::Verified, Author::Human, 9_000),
            rev(2, ChangeKind::Edited, Author::Human, 5_000),
            first.clone(),
        ];
        assert_eq!(
            provenance(&e, &history, &["main".into()]),
            vec![
                Provenance::Written {
                    by: Author::Claude,
                    at: 1_000
                },
                Provenance::From(TopicId(12)),
                Provenance::Branch("feature/terminal".into()),
                Provenance::Checked {
                    by: Author::Human,
                    at: 9_000
                },
            ]
        );
        // A branch memory says its branch in the Branch row instead; a
        // write after the verification hides "Checked".
        e.context_branch = Some("feature/terminal".into());
        let history = vec![
            rev(4, ChangeKind::Edited, Author::Claude, 10_000),
            rev(3, ChangeKind::Verified, Author::Human, 9_000),
            first,
        ];
        assert_eq!(
            provenance(&e, &history, &[]),
            vec![
                Provenance::Written {
                    by: Author::Claude,
                    at: 1_000
                },
                Provenance::From(TopicId(12)),
            ]
        );
    }

    #[test]
    fn a_save_over_a_newer_revision_merges_only_disjoint_sections() {
        let base = "Intro\n# Storage\nSQLite.\n# Keys\nKeychain.\n";
        let mine = "Intro\n# Storage\nSQLCipher.\n# Keys\nKeychain.\n";
        let theirs = "Intro\n# Storage\nSQLite.\n# Keys\nCredential Manager.\n";
        let t = ("Doc", "Sum", "");
        let w = |b| (t.0, t.1, b);
        assert_eq!(
            merge_over(w(base), w(mine), w(theirs)).as_deref(),
            Some("Intro\n# Storage\nSQLCipher.\n# Keys\nCredential Manager.\n")
        );
        // Both changed Storage: no merge.
        let both = "Intro\n# Storage\nPostgres.\n# Keys\nKeychain.\n";
        assert_eq!(merge_over(w(base), w(mine), w(both)), None);
        // A heading changed, a title changed, or a body without sections.
        let renamed = "Intro\n# Store\nSQLite.\n# Keys\nKeychain.\n";
        assert_eq!(merge_over(w(base), w(mine), w(renamed)), None);
        assert_eq!(merge_over(w(base), ("Doc 2", "Sum", mine), w(theirs)), None);
        assert_eq!(merge_over(w("a"), w("b"), w("a")), None);
    }

    #[test]
    fn a_verified_entry_loses_its_mark_at_once_and_git_settles_it() {
        let mut set = FlagSet::default();
        let id = KbId(4);
        set.set_store(vec![(
            id,
            vec![
                Flag::Store(KbFlag::Old { days: 120 }),
                Flag::Store(KbFlag::Duplicate(KbId(9))),
            ],
        )]);
        set.set_git(vec![(id, vec![Flag::FileChanged("src/kb.rs".into())])]);
        assert_eq!(set.reasons(id).len(), 3);
        set.mark_verified(id);
        assert_eq!(set.reasons(id), vec![Reason::Duplicate(KbId(9))]);
        // The write landed: the store no longer says old, git re-read.
        set.set_store(vec![(id, vec![Flag::Store(KbFlag::Duplicate(KbId(9)))])]);
        set.set_git(vec![(id, Vec::new())]);
        assert_eq!(set.reasons(id), vec![Reason::Duplicate(KbId(9))]);
        assert!(set.reasons(KbId(5)).is_empty());
    }

    #[test]
    fn levels_share_a_scope_up_and_down() {
        let epic_of = |t: TopicId| (t.0 == 1).then_some(EpicId(7));
        let (p, e, t, loose) = (
            Owner::Project(ProjectId(1)),
            Owner::Epic(EpicId(7)),
            Owner::Topic(TopicId(1)),
            Owner::Topic(TopicId(2)),
        );
        assert!(shares_scope(t, t, epic_of));
        assert!(shares_scope(t, e, epic_of) && shares_scope(e, t, epic_of));
        assert!(shares_scope(loose, p, epic_of));
        assert!(!shares_scope(loose, e, epic_of));
        assert!(!shares_scope(loose, t, epic_of));
        assert!(!shares_scope(Owner::Epic(EpicId(8)), e, epic_of));
    }

    #[test]
    fn a_session_is_named_by_its_topic_when_known() {
        let mut e = entry(1);
        e.created_session = Some("s1".into());
        e.created_topic = Some(TopicId(12));
        let mut live = HashMap::new();
        assert_eq!(session_topic(Some("s1"), &e, &live), Some(TopicId(12)));
        assert_eq!(session_topic(Some("s2"), &e, &live), None);
        live.insert("s2".to_owned(), TopicId(51));
        assert_eq!(session_topic(Some("s2"), &e, &live), Some(TopicId(51)));
        assert_eq!(session_topic(None, &e, &live), None);
    }
}
