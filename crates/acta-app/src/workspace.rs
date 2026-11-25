//! The open database as the window sees it: the tree of every project,
//! reloaded whenever anything commits, by Acta or by another process
//! (docs/design/interaction.md 6.5), and the writes the pages make, with
//! their undo (6.4).
//!
//! Every read and write runs on the background executor (`Db::run`); the UI
//! thread only swaps in the result. Pages observe the workspace and reload
//! their own data when its revision moves. Disk checks (folder existence,
//! canonical paths) run on their own, never while the store is locked.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acta_core::claude_settings::page::{self, PageCommit};
use acta_core::folder_identity::{self, Known, Monitor};
use acta_core::model::{
    Author, CommitMode, ContactId, ContactLevel, EpicId, FolderId, KbId, KbPairKind, KbPairState,
    KbState, Lift, Note, NoteId, Owner, ProjectId, ProjectTree, QuestionId, Record, Status,
    TopicId,
};
use acta_core::store::{EpicPatch, ProjectPatch, TopicPatch, Unread};
use acta_core::{ChangeWatcher, Store};
use gpui_kit::{App, Context, EventEmitter, Task};

use crate::db::Db;
use crate::i18n::{Arg, Key, text, tr};

/// docs/design/interaction.md 6.5: changes land within 100 ms of the write.
const POLL: Duration = Duration::from_millis(100);
/// How often the attached folders are looked at (ADR 0008, "Live files"):
/// a rename, a move or a deletion shows within this, plus a reload.
const FOLDER_POLL: Duration = Duration::from_millis(500);
/// How often the daily snapshot is looked at while the app runs.
const SNAPSHOT_CHECK: Duration = Duration::from_secs(60 * 60);
/// docs/design/interaction.md 6.4: the last 100 actions, per project.
const UNDO_DEPTH: usize = 100;

/// One step `Mod+Z` takes back. Each step holds the value the human's
/// action left (`expect`) and the value to put back (`back`). Undoing
/// compares the current value with `expect` inside the write transaction:
/// if anything else changed it since, Claude Code included, the step is
/// skipped, so undo never reverts someone else's write.
#[derive(Clone, Debug, PartialEq)]
pub enum Undo {
    /// The record was deleted: restore it (expects it in the trash).
    Restore(Record),
    /// The record was created or restored: delete it (expects it live).
    Delete(Record),
    Status {
        id: TopicId,
        back: Status,
        expect: Status,
    },
    TopicTitle {
        id: TopicId,
        back: String,
        expect: String,
    },
    EpicTitle {
        id: EpicId,
        back: String,
        expect: String,
    },
    /// A topic moved to another epic (`None`: out of any epic).
    TopicEpic {
        id: TopicId,
        back: Option<EpicId>,
        expect: Option<EpicId>,
    },
    ProjectTitle {
        id: ProjectId,
        back: String,
        expect: String,
    },
    /// A due date set, changed or cleared (`None`: no date).
    Due {
        owner: Owner,
        back: Option<acta_core::date::Date>,
        expect: Option<acta_core::date::Date>,
    },
    /// A page's active folder chosen (`None`: the default rule).
    ActiveFolder {
        owner: Owner,
        back: Option<FolderId>,
        expect: Option<FolderId>,
    },
    /// A folder's name changed (`""`: the last segment of its path).
    FolderName {
        id: FolderId,
        back: String,
        expect: String,
    },
    /// A folder's icon changed (`""`: automatic).
    FolderIcon {
        id: FolderId,
        back: String,
        expect: String,
    },
    /// A folder's commit mode or one of its options changed; its Claude
    /// Code settings follow (ADR 0011, 3).
    FolderCommit {
        id: FolderId,
        back: CommitColumns,
        expect: CommitColumns,
    },
    /// A page's own commit choice for a folder ("For this page"; `None`:
    /// the page uses what it inherits).
    PageCommit {
        owner: Owner,
        folder: FolderId,
        back: Option<PageCommit>,
        expect: Option<PageCommit>,
    },
    /// A folder set or located: put the old path back as stored.
    FolderPath {
        id: FolderId,
        back: String,
        expect: String,
    },
    KbRevision {
        id: KbId,
        back: i64,
        expect: i64,
    },
    /// A knowledge entry moved to another owner.
    KbOwner {
        id: KbId,
        back: Owner,
        expect: Owner,
    },
    /// A topic dragged to a place in another epic (or out of any): its
    /// epic and its index among that epic's topics, as one step.
    TopicPlace {
        id: TopicId,
        back: (Option<EpicId>, usize),
        expect: (Option<EpicId>, usize),
        /// Contacts the drop lifted (name, level now): undoing the drop
        /// leaves them at that level and says so.
        kept: Vec<(String, acta_core::model::ContactLevel)>,
    },
    /// A record moved among its siblings.
    Reorder {
        record: Record,
        back: usize,
        expect: usize,
    },
    /// A note filed by hand: back to the inbox (expects it filed).
    Unfile(NoteId),
    /// A note put back in the inbox: file it again exactly as it was
    /// (target, time, who), expecting it still open.
    Refile(Box<Note>),
    /// An entry edited or retyped in place: its text and type before.
    Entry {
        id: acta_core::model::EntryId,
        back: (String, acta_core::model::EntryKind),
        expect: (String, acta_core::model::EntryKind),
    },
    /// A note's text edited in place: the text before.
    Note {
        id: NoteId,
        back: String,
        expect: String,
    },
    /// A question changed in any way (text, contact asked, answer date,
    /// answered, dropped, reopened, moved): written back whole.
    Question {
        back: Box<acta_core::model::Question>,
        expect: Box<acta_core::model::Question>,
    },
    /// An open question moved among its owner's open questions.
    QuestionOrder {
        id: QuestionId,
        back: usize,
        expect: usize,
    },
    /// Report to of a note, topic or epic, and whether it was reported.
    ReportTo {
        record: Record,
        back: ReportState,
        expect: ReportState,
    },
    /// A contact moved to another level, or lifted by a move.
    ContactLevel {
        id: ContactId,
        back: ContactLevel,
        expect: ContactLevel,
    },
    /// A contact renamed or its details changed.
    ContactFields {
        id: ContactId,
        back: ContactFields,
        expect: ContactFields,
    },
    /// A merge of `from` into `before.id`: take it back, while the
    /// contact it went into still holds what the merge left (`after`).
    Unmerge {
        from: ContactId,
        before: Box<acta_core::model::Contact>,
        after: Box<acta_core::model::Contact>,
        uses: Vec<Record>,
        trace: acta_core::store::MergeTrace,
    },
    /// An unmerge taken back: merge again.
    Merge {
        from: ContactId,
        before: Box<acta_core::model::Contact>,
        uses: Vec<Record>,
    },
    /// Mark as still true (ADR 0015, 1): the verified date and file
    /// records before; the revision stays in history.
    KbVerify(Box<acta_core::store::KbVerifyTrace>),
    /// A verify taken back: vouch again, with the file records it made.
    KbReverify {
        id: KbId,
        rev: i64,
        blobs: Vec<(i64, Option<String>)>,
    },
    /// Mark as no longer true, Make live again, or "This replaces it" (the
    /// other entry's supersede): its state before.
    KbState(Box<acta_core::store::KbStateTrace>),
    /// A state change taken back: make it again.
    KbRestate {
        id: KbId,
        rev: i64,
        to: KbState,
        by: Option<KbId>,
    },
    /// "Keep both": the pair's state before.
    KbPair {
        a: KbId,
        b: KbId,
        kind: KbPairKind,
        back: KbPairState,
        expect: KbPairState,
    },
    /// A merge of two memories: the store's unmerge.
    KbMerge(Box<acta_core::store::KbMergeTrace>),
    /// An unmerge taken back: merge again.
    KbRemerge {
        survivor: KbId,
        other: KbId,
    },
    /// Several steps taken back as one, all or none.
    Many(Vec<Undo>),
}

/// The state of the pair of `a` and `b` of `kind` (open when none was
/// recorded).
pub fn kb_pair_state(
    store: &Store,
    a: KbId,
    b: KbId,
    kind: KbPairKind,
) -> acta_core::Result<KbPairState> {
    Ok(store
        .kb_pairs_of(a)?
        .into_iter()
        .find(|p| p.kind == kind && p.other(a) == b)
        .map_or(KbPairState::Open, |p| p.state))
}

/// The blob ids an entry's sources hold now, for the links of `ids`.
fn kb_blobs_now(
    store: &Store,
    id: KbId,
    ids: &[(i64, Option<String>)],
) -> acta_core::Result<Vec<(i64, Option<String>)>> {
    Ok(store
        .kb_links(id)?
        .into_iter()
        .filter(|l| ids.iter().any(|(link, _)| *link == l.id))
        .map(|l| (l.id, l.blob))
        .collect())
}

/// Who a record reports to and whether it was marked reported.
pub type ReportState = (Option<ContactId>, bool);

/// A contact's name, job title, company and address note.
pub type ContactFields = (String, Option<String>, Option<String>, Option<String>);

pub fn contact_fields(store: &Store, id: ContactId) -> acta_core::Result<ContactFields> {
    let c = store.contact(id)?;
    Ok((c.name, c.job_title, c.company, c.address_note))
}

/// What undo compares of a question: everything but its stamp, its place
/// among the others and the contact's current name.
type QuestionKey<'a> = (
    Owner,
    &'a str,
    Option<ContactId>,
    Option<acta_core::date::Date>,
    acta_core::model::QuestionStatus,
    Option<&'a str>,
    Option<acta_core::model::EntryId>,
);

fn question_key(q: &acta_core::model::Question) -> QuestionKey<'_> {
    (
        q.owner,
        q.text.as_str(),
        q.asked_to.as_ref().map(|c| c.id),
        q.expected_on,
        q.status,
        q.answer.as_deref(),
        q.decision_entry,
    )
}

/// Where an open question stands among its owner's open questions.
pub fn question_index(store: &Store, id: QuestionId) -> acta_core::Result<usize> {
    let q = store.question(id)?;
    Ok(store
        .questions_of(q.owner)?
        .iter()
        .filter(|x| x.is_open())
        .position(|x| x.id == id)
        .unwrap_or(0))
}

/// Report to of a note, topic or epic as stored.
pub fn report_state(store: &Store, record: Record) -> acta_core::Result<ReportState> {
    Ok(match record {
        Record::Topic(id) => {
            let r = store.topic(id)?.report;
            (r.to.map(|c| c.id), r.at.is_some())
        }
        Record::Epic(id) => {
            let r = store.epic(id)?.report;
            (r.to.map(|c| c.id), r.at.is_some())
        }
        Record::Note(id) => (store.note(id)?.report_to.map(|c| c.id), false),
        _ => (None, false),
    })
}

/// Writes a Report to state: the contact (which resets reported), then
/// reported when it was.
pub fn put_report(store: &mut Store, record: Record, state: ReportState) -> acta_core::Result<()> {
    let me = Author::Human;
    store.set_report_to(record, state.0, me)?;
    if state.0.is_some() && !matches!(record, Record::Note(_)) {
        store.set_reported(record, state.1, me)?;
    }
    Ok(())
}

/// A folder's commit columns: mode, "Commit in my name" (`None`: the
/// Settings default), "Also on <default branch>".
pub type CommitColumns = (CommitMode, Option<bool>, bool);

fn commit_columns(store: &Store, id: FolderId) -> acta_core::Result<CommitColumns> {
    let f = store.folder(id)?;
    Ok((f.commit_mode, f.commit_as_user, f.commit_on_default))
}

fn is_live(store: &Store, record: Record) -> acta_core::Result<bool> {
    let found = match record {
        Record::Project(id) => store.project(id).map(|_| ()),
        Record::Epic(id) => store.epic(id).map(|_| ()),
        Record::Topic(id) => store.topic(id).map(|_| ()),
        Record::Entry(id) => store.entry(id).map(|_| ()),
        Record::Note(id) => store.note(id).map(|_| ()),
        Record::Kb(id) => store.kb_entry(id).map(|_| ()),
        Record::Folder(id) => store.folder(id).map(|_| ()),
        Record::Question(id) => store.question(id).map(|_| ()),
        Record::Contact(id) => store.contact(id).map(|_| ()),
        Record::Attachment(id) => store.attachment(id).map(|_| ()),
    };
    match found {
        Ok(()) => Ok(true),
        Err(acta_core::Error::NotFound(_)) => Ok(false),
        Err(err) => Err(err),
    }
}

impl Undo {
    /// Whether the store still holds what the action left.
    fn matches(&self, store: &Store) -> acta_core::Result<bool> {
        Ok(match self {
            Undo::Restore(record) => !is_live(store, *record)?,
            Undo::Delete(record) => is_live(store, *record)?,
            Undo::Status { id, expect, .. } => store.topic(*id)?.status == *expect,
            Undo::TopicTitle { id, expect, .. } => store.topic(*id)?.title == *expect,
            Undo::EpicTitle { id, expect, .. } => store.epic(*id)?.title == *expect,
            Undo::TopicEpic { id, expect, .. } => store.topic(*id)?.epic == *expect,
            Undo::ProjectTitle { id, expect, .. } => store.project(*id)?.title == *expect,
            Undo::Due { owner, expect, .. } => due_on(store, *owner)? == *expect,
            Undo::ActiveFolder { owner, expect, .. } => active_pointer(store, *owner)? == *expect,
            Undo::FolderName { id, expect, .. } => store.folder(*id)?.name == *expect,
            Undo::FolderIcon { id, expect, .. } => store.folder(*id)?.icon == *expect,
            Undo::FolderCommit { id, expect, .. } => commit_columns(store, *id)? == *expect,
            Undo::PageCommit {
                owner,
                folder,
                expect,
                ..
            } => {
                // A folder gone since: nothing to put back.
                is_live(store, Record::Folder(*folder))?
                    && page::page_commit(store, *owner, *folder)? == *expect
            }
            Undo::FolderPath { id, expect, .. } => store.folder(*id)?.path == *expect,
            Undo::KbRevision { id, expect, .. } => store.kb_entry(*id)?.rev == *expect,
            Undo::KbOwner { id, expect, .. } => store.kb_entry(*id)?.owner == *expect,
            Undo::Reorder { record, expect, .. } => store.order_index(*record)? == *expect,
            Undo::TopicPlace { id, expect, .. } => {
                (
                    store.topic(*id)?.epic,
                    store.order_index(Record::Topic(*id))?,
                ) == *expect
            }
            Undo::Entry { id, expect, .. } => {
                let e = store.entry(*id)?;
                (e.text, e.kind) == *expect
            }
            Undo::Note { id, expect, .. } => store.note(*id)?.text == *expect,
            Undo::Question { expect, .. } => {
                question_key(&store.question(expect.id)?) == question_key(expect)
            }
            Undo::QuestionOrder { id, expect, .. } => question_index(store, *id)? == *expect,
            Undo::ReportTo { record, expect, .. } => report_state(store, *record)? == *expect,
            Undo::ContactLevel { id, expect, .. } => store.contact(*id)?.level == *expect,
            Undo::ContactFields { id, expect, .. } => contact_fields(store, *id)? == *expect,
            Undo::Unmerge {
                from,
                before,
                after,
                ..
            } => {
                let now = store.contact(before.id)?;
                store.contact_any(*from)?.merged_into == Some(before.id)
                    && !is_live(store, Record::Contact(*from))?
                    && (
                        &now.name,
                        &now.job_title,
                        &now.company,
                        &now.address_note,
                        now.level,
                    ) == (
                        &after.name,
                        &after.job_title,
                        &after.company,
                        &after.address_note,
                        after.level,
                    )
            }
            Undo::Merge { from, .. } => is_live(store, Record::Contact(*from))?,
            Undo::KbVerify(trace) => store.kb_entry(trace.id)?.rev == trace.rev,
            Undo::KbReverify { id, rev, .. } => {
                let e = store.kb_entry(*id)?;
                e.rev == *rev && e.state == KbState::Live
            }
            Undo::KbState(trace) => store.kb_entry(trace.id)?.rev == trace.rev,
            Undo::KbRestate { id, rev, .. } => store.kb_entry(*id)?.rev == *rev,
            Undo::KbPair {
                a, b, kind, expect, ..
            } => kb_pair_state(store, *a, *b, *kind)? == *expect,
            Undo::KbMerge(trace) => store.kb_can_unmerge(trace)?,
            Undo::KbRemerge { survivor, other } => {
                store.kb_entry(*survivor)?.state == KbState::Live
                    && store.kb_entry(*other)?.state == KbState::Live
            }
            Undo::Unfile(id) => !store.note(*id)?.is_open(),
            Undo::Refile(before) => store.note(before.id)?.is_open(),
            Undo::Many(all) => {
                for undo in all {
                    if !undo.matches(store)? {
                        return Ok(false);
                    }
                }
                true
            }
        })
    }

    /// Take the step back and return its inverse: the step that puts
    /// back what this one undid (the redo of an undo, the undo of a redo),
    /// built from the values the store holds once it is written.
    fn revert(&self, store: &mut Store) -> acta_core::Result<Undo> {
        let me = Author::Human;
        Ok(match self {
            Undo::Restore(record) => {
                store.restore(*record, me)?;
                Undo::Delete(*record)
            }
            Undo::Delete(record) => {
                store.delete(*record, me)?;
                Undo::Restore(*record)
            }
            Undo::Status { id, back, expect } => {
                store.update_topic(
                    *id,
                    TopicPatch {
                        status: Some(*back),
                        ..TopicPatch::default()
                    },
                    me,
                )?;
                Undo::Status {
                    id: *id,
                    back: *expect,
                    expect: store.topic(*id)?.status,
                }
            }
            Undo::TopicTitle { id, back, expect } => {
                store.update_topic(
                    *id,
                    TopicPatch {
                        title: Some(back.clone()),
                        ..TopicPatch::default()
                    },
                    me,
                )?;
                Undo::TopicTitle {
                    id: *id,
                    back: expect.clone(),
                    expect: store.topic(*id)?.title,
                }
            }
            Undo::EpicTitle { id, back, expect } => {
                store.update_epic(
                    *id,
                    EpicPatch {
                        title: Some(back.clone()),
                        ..EpicPatch::default()
                    },
                    me,
                )?;
                Undo::EpicTitle {
                    id: *id,
                    back: expect.clone(),
                    expect: store.epic(*id)?.title,
                }
            }
            Undo::TopicEpic { id, back, expect } => {
                store.update_topic(
                    *id,
                    TopicPatch {
                        epic: Some(*back),
                        ..TopicPatch::default()
                    },
                    me,
                )?;
                Undo::TopicEpic {
                    id: *id,
                    back: *expect,
                    expect: store.topic(*id)?.epic,
                }
            }
            Undo::ProjectTitle { id, back, expect } => {
                store.update_project(
                    *id,
                    ProjectPatch {
                        title: Some(back.clone()),
                        ..ProjectPatch::default()
                    },
                    me,
                )?;
                Undo::ProjectTitle {
                    id: *id,
                    back: expect.clone(),
                    expect: store.project(*id)?.title,
                }
            }
            Undo::Due {
                owner,
                back,
                expect,
            } => {
                store.set_due(*owner, *back, me)?;
                Undo::Due {
                    owner: *owner,
                    back: *expect,
                    expect: *back,
                }
            }
            Undo::ActiveFolder {
                owner,
                back,
                expect,
            } => {
                store.set_active_folder(*owner, *back, me)?;
                Undo::ActiveFolder {
                    owner: *owner,
                    back: *expect,
                    expect: *back,
                }
            }
            Undo::FolderName { id, back, expect } => {
                store.set_folder_name(*id, back, me)?;
                Undo::FolderName {
                    id: *id,
                    back: expect.clone(),
                    expect: store.folder(*id)?.name,
                }
            }
            Undo::FolderIcon { id, back, expect } => {
                store.set_folder_icon(*id, back, me)?;
                Undo::FolderIcon {
                    id: *id,
                    back: expect.clone(),
                    expect: store.folder(*id)?.icon,
                }
            }
            Undo::FolderCommit { id, back, expect } => {
                let (mode, as_user, on_default) = *back;
                store.set_commit_settings(*id, mode, as_user, on_default, me)?;
                Undo::FolderCommit {
                    id: *id,
                    back: *expect,
                    expect: commit_columns(store, *id)?,
                }
            }
            Undo::PageCommit {
                owner,
                folder,
                back,
                expect,
            } => {
                page::set_page_commit(store, *owner, *folder, *back, me)?;
                Undo::PageCommit {
                    owner: *owner,
                    folder: *folder,
                    back: *expect,
                    expect: page::page_commit(store, *owner, *folder)?,
                }
            }
            Undo::FolderPath { id, back, expect } => {
                store.restore_folder_path(*id, back, me)?;
                Undo::FolderPath {
                    id: *id,
                    back: expect.clone(),
                    expect: store.folder(*id)?.path,
                }
            }
            Undo::KbRevision { id, back, expect } => {
                store.kb_restore_revision(*id, *back, me)?;
                // The text changed back: its pairs are checked again, as
                // the human's writes are (components-knowledge.md 5).
                store.kb_recheck(*id, me)?;
                Undo::KbRevision {
                    id: *id,
                    back: *expect,
                    expect: store.kb_entry(*id)?.rev,
                }
            }
            Undo::KbOwner { id, back, expect } => {
                store.kb_move(*id, *back, me)?;
                store.kb_recheck(*id, me)?;
                Undo::KbOwner {
                    id: *id,
                    back: *expect,
                    expect: store.kb_entry(*id)?.owner,
                }
            }
            Undo::TopicPlace {
                id, back, expect, ..
            } => {
                put_topic(store, *id, *back, me)?;
                Undo::TopicPlace {
                    id: *id,
                    back: *expect,
                    expect: topic_place(store, *id)?,
                    kept: Vec::new(),
                }
            }
            Undo::Reorder {
                record,
                back,
                expect,
            } => {
                store.reorder(*record, *back, me)?;
                Undo::Reorder {
                    record: *record,
                    back: *expect,
                    expect: store.order_index(*record)?,
                }
            }
            Undo::Entry { id, back, expect } => {
                store.update_entry(*id, &back.0, back.1, me)?;
                let e = store.entry(*id)?;
                Undo::Entry {
                    id: *id,
                    back: expect.clone(),
                    expect: (e.text, e.kind),
                }
            }
            Undo::Note { id, back, expect } => {
                store.update_note(*id, back, me)?;
                Undo::Note {
                    id: *id,
                    back: expect.clone(),
                    expect: store.note(*id)?.text,
                }
            }
            Undo::Question { back, expect } => {
                store.put_question(back, me)?;
                Undo::Question {
                    back: expect.clone(),
                    expect: Box::new(store.question(back.id)?),
                }
            }
            Undo::QuestionOrder { id, back, expect } => {
                store.reorder_question(*id, *back, me)?;
                Undo::QuestionOrder {
                    id: *id,
                    back: *expect,
                    expect: question_index(store, *id)?,
                }
            }
            Undo::ReportTo {
                record,
                back,
                expect,
            } => {
                put_report(store, *record, *back)?;
                Undo::ReportTo {
                    record: *record,
                    back: *expect,
                    expect: report_state(store, *record)?,
                }
            }
            Undo::ContactLevel { id, back, expect } => {
                store.move_contact(*id, *back, me)?;
                Undo::ContactLevel {
                    id: *id,
                    back: *expect,
                    expect: store.contact(*id)?.level,
                }
            }
            Undo::ContactFields { id, back, expect } => {
                store.update_contact(
                    *id,
                    acta_core::store::ContactPatch {
                        name: Some(back.0.clone()),
                        job_title: Some(back.1.clone()),
                        company: Some(back.2.clone()),
                        address_note: Some(back.3.clone()),
                    },
                    me,
                )?;
                Undo::ContactFields {
                    id: *id,
                    back: expect.clone(),
                    expect: contact_fields(store, *id)?,
                }
            }
            Undo::Unmerge {
                from,
                before,
                uses,
                trace,
                ..
            } => {
                store.unmerge_contacts(*from, before, uses, trace, me)?;
                Undo::Merge {
                    from: *from,
                    before: before.clone(),
                    uses: uses.clone(),
                }
            }
            Undo::Merge { from, before, uses } => {
                let trace = store.merge_contacts(*from, before.id, me)?;
                Undo::Unmerge {
                    from: *from,
                    before: before.clone(),
                    after: Box::new(store.contact(before.id)?),
                    uses: uses.clone(),
                    trace,
                }
            }
            Undo::KbVerify(trace) => {
                let blobs = kb_blobs_now(store, trace.id, &trace.blobs)?;
                store.kb_unverify(trace)?;
                Undo::KbReverify {
                    id: trace.id,
                    rev: trace.rev,
                    blobs,
                }
            }
            Undo::KbReverify { id, rev, blobs } => {
                let ctx = acta_core::store::KbWriteCtx::default();
                Undo::KbVerify(Box::new(store.kb_verify(
                    *id,
                    Some(*rev),
                    blobs,
                    &ctx,
                    me,
                )?))
            }
            Undo::KbState(trace) => {
                let now = store.kb_entry(trace.id)?;
                let rev = store.kb_revert_state(trace, me)?;
                Undo::KbRestate {
                    id: trace.id,
                    rev,
                    to: now.state,
                    by: now.superseded_by,
                }
            }
            Undo::KbRestate { id, to, by, .. } => {
                let ctx = acta_core::store::KbWriteCtx::default();
                let trace = match (to, by) {
                    (KbState::Retired, _) => store.kb_retire(*id, None, "", &ctx, me)?,
                    (KbState::Superseded, Some(by)) => store
                        .kb_replace_with(*by, &[*id], me)?
                        .pop()
                        .ok_or_else(|| acta_core::Error::Invalid(format!("{id} not superseded")))?,
                    _ => store.kb_make_live(*id, me)?,
                };
                Undo::KbState(Box::new(trace))
            }
            Undo::KbPair {
                a,
                b,
                kind,
                back,
                expect,
            } => {
                store.kb_set_pair_state(*a, *b, *kind, *back, me)?;
                Undo::KbPair {
                    a: *a,
                    b: *b,
                    kind: *kind,
                    back: *expect,
                    expect: *back,
                }
            }
            Undo::KbMerge(trace) => {
                store.kb_unmerge(trace, me)?;
                Undo::KbRemerge {
                    survivor: trace.survivor,
                    other: trace.other,
                }
            }
            Undo::KbRemerge { survivor, other } => {
                Undo::KbMerge(Box::new(store.kb_merge(*survivor, *other, me)?))
            }
            Undo::Unfile(id) => {
                let filed = store.note(*id)?;
                store.unfile_note(*id, me)?;
                Undo::Refile(Box::new(filed))
            }
            Undo::Refile(before) => {
                store.restore_note_fate(before.id, before, me)?;
                Undo::Unfile(before.id)
            }
            Undo::Many(all) => {
                let mut inverse = Vec::with_capacity(all.len());
                for undo in all {
                    inverse.push(undo.revert(store)?);
                }
                inverse.reverse();
                Undo::Many(inverse)
            }
        })
    }

    /// Whether taking the step back changes which folders exist or their
    /// commit settings: Claude Code's settings files then follow.
    fn touches_folders(&self) -> bool {
        match self {
            // A project, epic or topic takes its folders to the trash and
            // back with it.
            Undo::Restore(
                Record::Folder(_) | Record::Project(_) | Record::Epic(_) | Record::Topic(_),
            )
            | Undo::Delete(
                Record::Folder(_) | Record::Project(_) | Record::Epic(_) | Record::Topic(_),
            )
            | Undo::FolderCommit { .. }
            | Undo::FolderPath { .. } => true,
            Undo::Many(all) => all.iter().any(Undo::touches_folders),
            _ => false,
        }
    }

    /// Check and take back in one transaction. `Ok(None)`: something else
    /// changed it since, nothing was written; otherwise the inverse step.
    pub fn apply(&self, store: &mut Store) -> acta_core::Result<Option<Undo>> {
        store.atomically(|store| {
            if !self.matches(store)? {
                return Ok(None);
            }
            self.revert(store).map(Some)
        })
    }
}

/// Whether a finished undo (or redo) keeps its inverse: a redo always goes
/// back on the undo stack; an undo's redo is dropped when a new action of
/// the project (which cleared the redo stack) came in while it was written.
fn keeps_inverse(redo: bool, actions_then: u64, actions_now: u64) -> bool {
    redo || actions_then == actions_now
}

/// The order of every undo step of the process: `Mod+Z` takes the newest
/// of a window's layout steps and its writes (workbench.md 4.8.11, rule 3;
/// interaction.md 6.4).
static UNDO_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn next_undo_seq() -> u64 {
    UNDO_SEQ.fetch_add(1, Ordering::Relaxed)
}

/// The bounded undo stack of one project, each step with its order.
#[derive(Clone, Debug, Default)]
pub struct UndoStack(Vec<(u64, Undo)>);

impl UndoStack {
    pub fn push(&mut self, undo: Undo) {
        self.0.push((next_undo_seq(), undo));
        if self.0.len() > UNDO_DEPTH {
            self.0.remove(0);
        }
    }

    pub fn last(&self) -> Option<&Undo> {
        self.0.last().map(|(_, u)| u)
    }

    /// The order of the newest step.
    pub fn newest(&self) -> Option<u64> {
        self.0.last().map(|(seq, _)| *seq)
    }

    #[cfg(test)]
    pub fn pop(&mut self) -> Option<Undo> {
        self.0.pop().map(|(_, u)| u)
    }

    /// Remove exactly `step`: the newest step equal to it, whatever was
    /// pushed while it was being written.
    pub fn remove(&mut self, step: &Undo) {
        if let Some(at) = self.0.iter().rposition(|(_, s)| s == step) {
            self.0.remove(at);
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// A toast (docs/design/components.md 5): undo and failures only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    /// An action of its own beside the message (a file's toasts).
    pub action: Option<ToastAction>,
}

/// The action a toast offers besides Undo (components-attachments.md 5,
/// Save as).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToastAction {
    /// "Show in folder": the file manager on the parent, the file selected.
    Reveal(std::path::PathBuf),
    /// Retry: Save as to the same path again.
    RetrySave(Box<acta_core::model::Attachment>, std::path::PathBuf),
    /// "Show" after "Find duplicate memories": the knowledge base page with
    /// the chip "To check".
    ShowToCheck(ProjectId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    /// With an Undo action for the newest undo step; gone after 6 s.
    Undo,
    /// Stays until dismissed.
    Error,
    /// A fact with nothing to act on (an export written); gone after 6 s.
    Done,
    /// A layout change (workbench.md 4.8.11): Undo and Reset layout; gone
    /// after 6 s.
    Layout,
    /// "Layout reset." (4.8.11): Undo only; gone after 6 s.
    LayoutReset,
    /// A move that would land out of sight: the one action that makes room
    /// and moves; gone after 6 s.
    LayoutNoRoom,
}

pub enum WorkspaceEvent {
    Toast(Toast),
    /// The record shown may be gone; pages check.
    Reloaded,
}

/// Why a write did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// A knowledge entry changed since it was read (another writer).
    Stale,
    /// A contact the write names is not visible from its record (moved
    /// meanwhile): no toast, the picker that chose it says so under the
    /// field (components-contacts.md 5, Errors).
    OutOfScope,
    /// Anything else; a toast already said so.
    Failed,
}

/// The offer after a Locate (docs/design/components-folders.md 5,
/// Repairing the rest): the same root change for exactly these folders,
/// each `(id, path now, path after)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkOffer {
    pub old_root: String,
    pub changes: Vec<(FolderId, String, String)>,
}

/// Which folder a Locate repairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderRef {
    Id(FolderId),
    /// The folder of `owner` stored as `path` (an effective folder).
    At(Owner, String),
}

pub struct Workspace {
    pub db: Db,
    /// Git reads for knowledge flags (ADR 0015, 1), cached per folder and
    /// `HEAD`; used off the UI thread only.
    pub kb_git: Arc<Mutex<acta_core::agent::kbq::GitCache>>,
    /// "Find duplicate memories" is running (the command is not listed).
    pub kb_scanning: bool,
    pub bulk: Option<BulkOffer>,
    pub projects: Vec<ProjectTree>,
    /// How many live contacts exist: the sidebar's Contacts row shows only
    /// once one does (components-contacts.md 5).
    pub contacts: usize,
    /// What Claude Code changed since this installation last showed it
    /// (the unread dots; data.md request 1), reloaded with the tree.
    pub unread: Unread,
    /// Bumped on every reload, by any process's commit.
    pub revision: u64,
    /// Bumped when the window regains focus and when the folder monitor
    /// reloads the pages: the folders' git state is read again then, never
    /// by watching `.git`.
    pub git_epoch: u64,
    undo: HashMap<Option<ProjectId>, UndoStack>,
    /// The steps undone, per project, for `Mod+Shift+Z`: each the inverse
    /// of an undo, checked the same way. A new action of the project
    /// clears its redo.
    redo: HashMap<Option<ProjectId>, UndoStack>,
    /// Bumped per project by every new action (which clears its redo): an
    /// undo that finishes after one does not leave a redo behind it.
    actions: HashMap<Option<ProjectId>, u64>,
    /// An undo or redo is being written; a second key waits for it.
    undoing: bool,
    _reload: Task<()>,
    _watch: Task<()>,
    _snapshots: Task<()>,
    /// Every live folder with its identity, reloaded with the tree, for
    /// the folder monitor.
    known_folders: Vec<Known>,
    /// The next monitor pass is thorough (the window regained focus).
    thorough: Arc<AtomicBool>,
    _folders: Task<()>,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

/// Every project's tree, in the stored order.
pub fn load_trees(store: &Store) -> acta_core::Result<Vec<ProjectTree>> {
    store
        .projects()?
        .into_iter()
        .map(|project| store.tree(project.id))
        .collect()
}

impl Workspace {
    pub fn new(db: Db, cx: &mut Context<Self>) -> Self {
        let watcher_db = db.clone();
        let watch = cx.spawn(async move |this, cx| {
            let opened = cx
                .background_executor()
                .spawn(async move { watcher_db.lock().watcher() })
                .await;
            let watcher = match opened {
                Ok(watcher) => Arc::new(Mutex::new(watcher)),
                Err(_) => {
                    crate::log::line(
                        "store: no change watcher; other processes' writes show on restart",
                    );
                    return;
                }
            };
            loop {
                cx.background_executor().timer(POLL).await;
                let watcher = Arc::clone(&watcher);
                let changed = cx
                    .background_executor()
                    .spawn(async move { poll(&watcher) })
                    .await;
                if changed && this.update(cx, |ws, cx| ws.reload(cx)).is_err() {
                    return;
                }
            }
        });
        let snapshot_db = db.clone();
        let snapshots = cx.spawn(async move |_, cx| {
            loop {
                let db = snapshot_db.clone();
                let taken = cx
                    .background_executor()
                    .spawn(async move { db.lock().snapshot_if_due() })
                    .await;
                if let Err(err) = taken {
                    crate::log::line(&format!(
                        "snapshot: daily snapshot failed: {}",
                        err_kind(&err)
                    ));
                }
                cx.background_executor().timer(SNAPSHOT_CHECK).await;
            }
        });
        let thorough = Arc::new(AtomicBool::new(true));
        let folders = Self::monitor_folders(db.clone(), Arc::clone(&thorough), cx);
        let mut workspace = Self {
            db,
            bulk: None,
            projects: Vec::new(),
            contacts: 0,
            unread: Unread::default(),
            revision: 0,
            git_epoch: 0,
            undo: HashMap::new(),
            redo: HashMap::new(),
            actions: HashMap::new(),
            undoing: false,
            kb_git: Arc::default(),
            kb_scanning: false,
            _reload: Task::ready(()),
            _watch: watch,
            _snapshots: snapshots,
            known_folders: Vec::new(),
            thorough,
            _folders: folders,
        };
        workspace.reload(cx);
        workspace
    }

    /// Reload the tree; pages observing the workspace reload their data.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let task = self.db.run(cx, |store| {
            Ok((
                load_trees(store)?,
                store.unread()?,
                store.monitored_folders()?,
                store.profile()?.name,
                store.contact_count()?,
            ))
        });
        self._reload = cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |ws, cx| match loaded {
                Ok((projects, unread, folders, name, contacts)) => {
                    crate::when::set_profile_name(name);
                    ws.contacts = contacts;
                    ws.projects = projects;
                    ws.unread = unread;
                    ws.known_folders = folders;
                    ws.revision += 1;
                    cx.emit(WorkspaceEvent::Reloaded);
                    cx.notify();
                }
                Err(err) => crate::log::line(&format!("store: reload failed: {}", err_kind(&err))),
            });
        });
    }

    /// The folder monitor (ADR 0008, "Live files"): every `FOLDER_POLL`,
    /// off the UI thread and never while holding the store, it asks whether
    /// each attached folder is still there, without keeping anything open
    /// on it. A folder that was renamed or moved is followed when its
    /// identity proves it (the store write reloads every page, the name
    /// shown being its last segment); one that appeared or went missing
    /// reloads the pages, which check presence again.
    fn monitor_folders(db: Db, thorough: Arc<AtomicBool>, cx: &mut Context<Self>) -> Task<()> {
        let monitor = Arc::new(Mutex::new(Monitor::new()));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FOLDER_POLL).await;
                let Ok(known) = this.read_with(cx, |ws, _| ws.known_folders.clone()) else {
                    return;
                };
                if known.is_empty() {
                    continue;
                }
                let deep = thorough.swap(false, Ordering::Relaxed);
                let monitor = Arc::clone(&monitor);
                let pass = cx
                    .background_executor()
                    .spawn(async move {
                        let mut monitor = monitor.lock().unwrap_or_else(|p| p.into_inner());
                        monitor.pass(&known, deep)
                    })
                    .await;
                let followed_any = !pass.followed.is_empty();
                if pass.has_writes() {
                    let db = db.clone();
                    let (learned, followed) = (pass.learned, pass.followed);
                    let applied = cx
                        .background_executor()
                        .spawn(async move { db.lock().apply_folder_pass(&learned, &followed) })
                        .await;
                    match applied {
                        // The path is user content: the log says how many.
                        Ok(moved) if !moved.is_empty() => crate::log::line(&format!(
                            "folders: {} relocated after a rename or move",
                            moved.len()
                        )),
                        Ok(_) => {}
                        Err(err) => crate::log::line(&format!(
                            "folders: could not record a relocation: {}",
                            err_kind(&err)
                        )),
                    }
                }
                if pass.changed || followed_any {
                    // A folder came back or was followed: its Claude Code
                    // settings are brought in line with the store (ADR 0011).
                    let db = db.clone();
                    cx.background_executor()
                        .spawn(async move {
                            let _ = acta_core::claude_settings::sync_all(&mut db.lock());
                        })
                        .await;
                }
                if pass.changed
                    && this
                        .update(cx, |ws, cx| {
                            ws.git_epoch += 1;
                            ws.reload(cx)
                        })
                        .is_err()
                {
                    return;
                }
            }
        })
    }

    /// The window regained focus: the next monitor pass reads every
    /// folder's identity again and searches again for missing ones, and
    /// the pages read their folders' git state again (ADR 0011 addendum,
    /// live files).
    pub fn check_folders_again(&mut self, cx: &mut Context<Self>) {
        self.thorough.store(true, Ordering::Relaxed);
        self.git_epoch += 1;
        cx.notify();
    }

    /// Whether `record` carries an unread dot (topics and knowledge only).
    pub fn is_unread(&self, record: Record) -> bool {
        match record {
            Record::Topic(id) => self.unread.topics.contains(&id),
            Record::Kb(id) => self.unread.has_kb(id),
            _ => false,
        }
    }

    /// `record` is on screen: its dot goes now, and its read marker is
    /// written in the background (local, never undone, never toasted).
    /// Nothing is written for a record without a dot.
    pub fn mark_seen(&mut self, record: Record, cx: &mut Context<Self>) {
        if !self.is_unread(record) {
            return;
        }
        match record {
            Record::Topic(id) => {
                self.unread.topics.remove(&id);
            }
            Record::Kb(id) => self.unread.kb.retain(|(k, _)| *k != id),
            _ => {}
        }
        cx.notify();
        let task = self.db.run(cx, move |store| store.mark_seen(record));
        cx.spawn(async move |_, _| {
            if let Err(err) = task.await {
                crate::log::line(&format!("store: read marker failed: {}", err_kind(&err)));
            }
        })
        .detach();
    }

    /// The project a record belongs to, from the loaded tree (`None` for
    /// knowledge entries, notes and folders, which the caller names).
    pub fn project_of(&self, owner: Owner) -> Option<ProjectId> {
        self.projects.iter().find_map(|tree| {
            let hit = match owner {
                Owner::Project(id) => tree.project.id == id,
                Owner::Epic(id) => tree.epics.iter().any(|(e, _)| e.id == id),
                Owner::Topic(id) => tree
                    .loose_topics
                    .iter()
                    .chain(tree.epics.iter().flat_map(|(_, t)| t))
                    .any(|t| t.id == id),
            };
            hit.then_some(tree.project.id)
        })
    }

    /// Run a write in the background. On success the tree reloads and the
    /// result comes back; a failure other than a stale revision toasts.
    pub fn write<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        failure: Key,
        f: impl FnOnce(&mut Store) -> acta_core::Result<R> + Send + 'static,
    ) -> Task<Result<R, WriteError>> {
        self.write_with(cx, failure, false, f)
    }

    /// [`Workspace::write`] for a write naming a contact chosen in a
    /// picker: a contact out of the record's sight is answered with
    /// [`WriteError::OutOfScope`] and no toast, the picker showing the
    /// inline error instead (components-contacts.md 5, Errors).
    pub fn write_picked<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Store) -> acta_core::Result<R> + Send + 'static,
    ) -> Task<Result<R, WriteError>> {
        self.write_with(cx, Key::ErrorSave, true, f)
    }

    /// [`Workspace::write_undoable`] for a write naming a picked contact
    /// (see [`Workspace::write_picked`]).
    pub fn write_undoable_picked<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        project: Option<ProjectId>,
        f: impl FnOnce(&mut Store) -> acta_core::Result<(Undo, R)> + Send + 'static,
    ) -> Task<Result<R, WriteError>> {
        let task = self.write_picked(cx, move |store| store.atomically(f));
        self.push_undo(task, project, None, cx)
    }

    fn write_with<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        failure: Key,
        quiet_scope: bool,
        f: impl FnOnce(&mut Store) -> acta_core::Result<R> + Send + 'static,
    ) -> Task<Result<R, WriteError>> {
        let task = self.db.run(cx, f);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |ws, cx| {
                ws.reload(cx);
                match result {
                    Ok(value) => Ok(value),
                    Err(acta_core::Error::StaleRevision { .. }) => Err(WriteError::Stale),
                    Err(acta_core::Error::OutOfScope { .. }) if quiet_scope => {
                        Err(WriteError::OutOfScope)
                    }
                    // An editor's file a purge took meanwhile says so.
                    Err(acta_core::Error::StagedGone) => {
                        ws.toast_error(Key::AttachGone, cx);
                        Err(WriteError::Failed)
                    }
                    Err(err) => {
                        crate::log::line(&format!("store: write failed: {}", err_kind(&err)));
                        ws.toast_error(failure, cx);
                        Err(WriteError::Failed)
                    }
                }
            })
            .unwrap_or(Err(WriteError::Failed))
        })
    }

    /// A write whose result nobody waits for; failures still toast.
    pub fn write_detached<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        failure: Key,
        f: impl FnOnce(&mut Store) -> acta_core::Result<R> + Send + 'static,
    ) {
        self.write(cx, failure, f).detach();
    }

    /// A write the human can take back. `f` runs in one transaction and
    /// returns its undo step, built from the values it read there, and a
    /// result. The step goes on the stack of `project`; `message` (if any)
    /// is a toast with Undo.
    pub fn write_undoable<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        project: Option<ProjectId>,
        message: Option<String>,
        f: impl FnOnce(&mut Store) -> acta_core::Result<(Undo, R)> + Send + 'static,
    ) -> Task<Result<R, WriteError>> {
        let task = self.write(cx, Key::ErrorSave, move |store| store.atomically(f));
        self.push_undo(task, project, message, cx)
    }

    /// Once `task` has written, its step goes on the stack of `project`.
    fn push_undo<R: Send + 'static>(
        &mut self,
        task: Task<Result<(Undo, R), WriteError>>,
        project: Option<ProjectId>,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) -> Task<Result<R, WriteError>> {
        cx.spawn(async move |this, cx| {
            let (undo, value) = task.await?;
            let _ = this.update(cx, |ws, cx| {
                ws.undo.entry(project).or_default().push(undo);
                ws.redo.remove(&project);
                *ws.actions.entry(project).or_default() += 1;
                if let Some(message) = message {
                    cx.emit(WorkspaceEvent::Toast(Toast {
                        message,
                        kind: ToastKind::Undo,
                        action: None,
                    }));
                }
            });
            Ok(value)
        })
    }

    /// [`Workspace::write_undoable`] for a change to folders (added,
    /// removed, located, a commit setting): Claude Code's settings files are
    /// brought in line with the store once it is written (ADR 0011, 3).
    pub fn write_folders<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        project: Option<ProjectId>,
        message: Option<String>,
        f: impl FnOnce(&mut Store) -> acta_core::Result<(Undo, R)> + Send + 'static,
    ) -> Task<Result<R, WriteError>> {
        // The write commits first; the settings files of the paths it
        // touched are brought in line afterwards, in their own store task.
        let task = self.write(cx, Key::ErrorSave, move |store| {
            with_touched(store, |store| store.atomically(f))
        });
        let task = cx.spawn(async move |ws, cx| {
            let (done, touched) = task.await?;
            let _ = ws.update(cx, |ws, cx| ws.sync_folder_paths(touched, cx));
            Ok(done)
        });
        self.push_undo(task, project, message, cx)
    }

    /// Claude Code's settings files of `paths` brought in line with the
    /// store, in the background.
    pub fn sync_folder_paths(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        self.db
            .run(cx, move |store| {
                sync_paths(store, &paths);
                Ok(())
            })
            .detach();
    }

    /// `write_undoable` for callers that do not wait.
    pub fn undoable<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        project: Option<ProjectId>,
        message: Option<String>,
        f: impl FnOnce(&mut Store) -> acta_core::Result<(Undo, R)> + Send + 'static,
    ) {
        self.write_undoable(cx, project, message, f).detach();
    }

    /// Set a topic's status; the undo holds the status read in the same
    /// transaction.
    pub fn set_status(&mut self, id: TopicId, status: Status, cx: &mut Context<Self>) {
        let project = self.project_of(Owner::Topic(id));
        self.undoable(cx, project, None, move |store| {
            let back = store.topic(id)?.status;
            store.update_topic(
                id,
                TopicPatch {
                    status: Some(status),
                    ..TopicPatch::default()
                },
                Author::Human,
            )?;
            Ok((
                Undo::Status {
                    id,
                    back,
                    expect: status,
                },
                (),
            ))
        });
    }

    /// Set, change or clear the due date of a project, epic or topic
    /// (components-records.md 5, Writes): one undo step, no toast; the
    /// store writes the system event. The undo holds the date read in the
    /// same transaction.
    pub fn set_due(
        &mut self,
        owner: Owner,
        due: Option<acta_core::date::Date>,
        cx: &mut Context<Self>,
    ) {
        let project = self.project_of(owner);
        self.undoable(cx, project, None, move |store| {
            let back = due_on(store, owner)?;
            store.set_due(owner, due, Author::Human)?;
            Ok((
                Undo::Due {
                    owner,
                    back,
                    expect: due,
                },
                (),
            ))
        });
    }

    /// Rename a project, epic or topic (the page title, the tree's inline
    /// edit), undoably.
    pub fn rename(
        &mut self,
        owner: Owner,
        title: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), WriteError>> {
        let project = self.project_of(owner);
        self.write_undoable(cx, project, None, move |store| {
            let me = Author::Human;
            let undo = match owner {
                Owner::Topic(id) => {
                    let back = store.topic(id)?.title;
                    store.update_topic(
                        id,
                        TopicPatch {
                            title: Some(title),
                            ..TopicPatch::default()
                        },
                        me,
                    )?;
                    Undo::TopicTitle {
                        id,
                        back,
                        expect: store.topic(id)?.title,
                    }
                }
                Owner::Epic(id) => {
                    let back = store.epic(id)?.title;
                    store.update_epic(
                        id,
                        EpicPatch {
                            title: Some(title),
                            ..EpicPatch::default()
                        },
                        me,
                    )?;
                    Undo::EpicTitle {
                        id,
                        back,
                        expect: store.epic(id)?.title,
                    }
                }
                Owner::Project(id) => {
                    let back = store.project(id)?.title;
                    store.update_project(
                        id,
                        ProjectPatch {
                            title: Some(title),
                            ..ProjectPatch::default()
                        },
                        me,
                    )?;
                    Undo::ProjectTitle {
                        id,
                        back,
                        expect: store.project(id)?.title,
                    }
                }
            };
            Ok((undo, ()))
        })
    }

    /// Move a topic to another epic of its project (`None`: out of any
    /// epic), undoably (interaction.md 6.1, `M`).
    pub fn move_topic(&mut self, id: TopicId, epic: Option<EpicId>, cx: &mut Context<Self>) {
        let project = self.project_of(Owner::Topic(id));
        let task = self.write_undoable(cx, project, None, move |store| {
            let back = store.topic(id)?.epic;
            store.take_lifts();
            store.update_topic(
                id,
                TopicPatch {
                    epic: Some(epic),
                    ..TopicPatch::default()
                },
                Author::Human,
            )?;
            let moved = Undo::TopicEpic {
                id,
                back,
                expect: epic,
            };
            with_lifts(store, moved)
        });
        self.toast_lifts(task, cx);
    }

    /// Once a move that may have lifted contacts is written, say so with
    /// Undo (components-contacts.md 5, Levels): one toast per lift.
    pub fn toast_lifts(
        &mut self,
        task: Task<Result<Vec<Lifted>, WriteError>>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |ws, cx| {
            let Ok(lifted) = task.await else { return };
            let _ = ws.update(cx, |ws, cx| {
                let locale = crate::settings::locale(cx);
                for l in lifted {
                    let message = crate::contacts_model::lifted(
                        locale,
                        &l.name,
                        l.to,
                        &l.record,
                        &ws.projects,
                    );
                    ws.toast(message, ToastKind::Undo, cx);
                }
            });
        })
        .detach();
    }

    /// Delete a record to the trash, with "{kind} deleted." and Undo.
    /// Delete a record, undoably. A project, epic or topic takes its
    /// folders to the trash: their Claude Code settings are taken out (and
    /// put back by the undo or a restore).
    pub fn delete(&mut self, record: Record, project: Option<ProjectId>, cx: &mut Context<Self>) {
        let locale = crate::settings::locale(cx);
        let message = deleted_key(record).map(|key| text(locale, key));
        self.write_folders(cx, project, message, move |store| {
            store.delete(record, Author::Human)?;
            Ok((Undo::Restore(record), ()))
        })
        .detach();
    }

    /// Restore a record from the trash, undoably (its folders' settings
    /// come back with it).
    pub fn restore(
        &mut self,
        record: Record,
        project: Option<ProjectId>,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), WriteError>> {
        self.write_folders(cx, project, message, move |store| {
            store.restore(record, Author::Human)?;
            Ok((Undo::Delete(record), ()))
        })
    }

    /// Several trashed records put back in one undo step (components-app.md
    /// 5, Trash page): containers first, so a topic whose epic is also
    /// selected comes back after it; one that cannot come back (its parent
    /// still in the trash) is skipped. Returns how many came back.
    pub fn restore_many(
        &mut self,
        mut records: Vec<Record>,
        project: Option<ProjectId>,
        message: impl FnOnce(usize) -> String + Send + 'static,
        cx: &mut Context<Self>,
    ) -> Task<Result<usize, WriteError>> {
        records.sort_by_key(|r| restore_rank(*r));
        let task = self.write_folders(cx, project, None, move |store| {
            let mut steps = Vec::new();
            for record in records {
                if store.restore(record, Author::Human).is_ok() {
                    steps.push(Undo::Delete(record));
                }
            }
            let n = steps.len();
            // Undone contents first, then what held them.
            steps.reverse();
            Ok((Undo::Many(steps), n))
        });
        cx.spawn(async move |ws, cx| {
            let n = task.await?;
            let _ = ws.update(cx, |ws, cx| {
                if n > 0 {
                    ws.toast(message(n), ToastKind::Undo, cx);
                }
            });
            Ok(n)
        })
    }

    /// A topic dropped in another epic (or out of any) at `index` among
    /// that epic's topics (components.md 5, Tree row, Drag to reorder):
    /// one write, one undo step, no toast; contacts the move lifts say so
    /// as `M` does.
    pub fn place_topic(
        &mut self,
        id: TopicId,
        epic: Option<EpicId>,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let project = self.project_of(Owner::Topic(id));
        let task = self.write_undoable(cx, project, None, move |store| {
            let back = topic_place(store, id)?;
            store.take_lifts();
            put_topic(store, id, (epic, index), Author::Human)?;
            // The lifts are not taken back with the drop: their undo
            // toast says the contacts stay where they are now.
            let lifts = store.take_lifts();
            let mut kept = Vec::new();
            let mut lifted = Vec::new();
            for l in lifts {
                let name = store.contact(l.contact)?.name;
                kept.push((name.clone(), l.to));
                lifted.push(Lifted {
                    name,
                    to: l.to,
                    record: store.topic(id)?.title,
                });
            }
            let moved = Undo::TopicPlace {
                id,
                back,
                expect: topic_place(store, id)?,
                kept,
            };
            Ok((moved, lifted))
        });
        self.toast_lifts(task, cx);
    }

    /// Move a record among its siblings, undoably.
    pub fn reorder(
        &mut self,
        record: Record,
        index: usize,
        project: Option<ProjectId>,
        cx: &mut Context<Self>,
    ) {
        self.undoable(cx, project, None, move |store| {
            let back = store.order_index(record)?;
            store.reorder(record, index, Author::Human)?;
            let expect = store.order_index(record)?;
            Ok((
                Undo::Reorder {
                    record,
                    back,
                    expect,
                },
                (),
            ))
        });
    }

    /// "Locate...": point the folder at `chosen`, undoably, then offer the
    /// same change to other folders not found (components-folders.md 5).
    /// Used by the record pages, Settings and the terminal pane. The path
    /// is resolved off the store. Returns whether it was written.
    pub fn locate(
        &mut self,
        target: FolderRef,
        chosen: PathBuf,
        project: Option<ProjectId>,
        cx: &mut Context<Self>,
    ) -> Task<bool> {
        let canonical = cx
            .background_executor()
            .spawn(async move { acta_core::folder_path::canonical(&chosen) });
        cx.spawn(async move |ws, cx| {
            let Ok(stored) = canonical.await else {
                let _ = ws.update(cx, |ws, cx| ws.toast_error(Key::ErrorSave, cx));
                return false;
            };
            let Ok(task) = ws.update(cx, |ws, cx| {
                let new = stored.clone();
                ws.write_folders(cx, project, None, move |store| {
                    let folder = match target {
                        FolderRef::Id(id) => store.folder(id)?,
                        FolderRef::At(owner, path) => store
                            .folders(owner)?
                            .into_iter()
                            .find(|f| f.path == path)
                            .ok_or_else(|| acta_core::Error::NotFound("folder".into()))?,
                    };
                    store.set_folder_paths(&[(folder.id, new.clone())], Author::Human)?;
                    Ok((
                        Undo::FolderPath {
                            id: folder.id,
                            back: folder.path.clone(),
                            expect: new,
                        },
                        folder.path,
                    ))
                })
            }) else {
                return false;
            };
            match task.await {
                Ok(old) => {
                    let _ = ws.update(cx, |ws, cx| ws.offer_bulk_repair(old, stored, project, cx));
                    true
                }
                Err(_) => false,
            }
        })
    }

    /// After a Locate from `old` to `new`, look for other folders the same
    /// change would resolve (checked on disk, off the store) and offer
    /// exactly those.
    fn offer_bulk_repair(
        &mut self,
        old: String,
        new: String,
        project: Option<ProjectId>,
        cx: &mut Context<Self>,
    ) {
        let Some((old_root, new_root)) = crate::folders_model::roots(&old, &new) else {
            return;
        };
        let _ = project;
        let all = self.db.run(cx, |store| store.all_folders());
        cx.spawn(async move |ws, cx| {
            let Ok(folders) = all.await else { return };
            let (old_r, new_r) = (old_root.clone(), new_root.clone());
            let changes = cx
                .background_executor()
                .spawn(async move {
                    let paths: Vec<(FolderId, String)> =
                        folders.into_iter().map(|f| (f.id, f.path)).collect();
                    crate::folders_model::candidates(&paths, &old_r, &new_r, |p| {
                        folder_identity::is_dir(Path::new(p))
                    })
                    .into_iter()
                    .filter_map(|(id, path, target)| {
                        let stored = acta_core::folder_path::canonical(Path::new(&target)).ok()?;
                        Some((id, path, stored))
                    })
                    .collect::<Vec<_>>()
                })
                .await;
            if changes.is_empty() {
                return;
            }
            let _ = ws.update(cx, |ws, cx| {
                ws.bulk = Some(BulkOffer { old_root, changes });
                cx.notify();
            });
        })
        .detach();
    }

    /// "Apply to 7 folders": one undoable step, over exactly the folders
    /// offered that still have the path they had when offered.
    pub fn apply_bulk(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
        let Some(offer) = self.bulk.take() else {
            return;
        };
        cx.notify();
        let locale = crate::settings::locale(cx);
        let task = self.write_folders(cx, project, None, move |store| {
            let mut apply = Vec::new();
            let mut steps = Vec::new();
            for (id, old, new) in offer.changes {
                if store.folder(id)?.path == old {
                    apply.push((id, new.clone()));
                    steps.push(Undo::FolderPath {
                        id,
                        back: old,
                        expect: new,
                    });
                }
            }
            store.set_folder_paths(&apply, Author::Human)?;
            Ok((Undo::Many(steps), apply.len()))
        });
        cx.spawn(async move |ws, cx| {
            if let Ok(n) = task.await {
                let _ = ws.update(cx, |ws, cx| {
                    let message = tr(locale, Key::FoldersBulkDone, &[("n", Arg::Count(n as u64))]);
                    ws.toast(message, ToastKind::Undo, cx);
                });
            }
        })
        .detach();
    }

    pub fn dismiss_bulk(&mut self, cx: &mut Context<Self>) {
        self.bulk = None;
        cx.notify();
    }

    /// `Mod+Z`: take back the newest action of `project`. The step leaves
    /// the stack once written, or when something else changed it since
    /// (then a toast says it was not undone); a failed write keeps it.
    /// A step taken back goes on the redo stack as its inverse.
    pub fn undo(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
        self.take_back(project, false, cx);
    }

    /// `Mod+Shift+Z`: put back the newest undone step of `project`, with
    /// the same check, so a redo never lands over a newer write.
    pub fn redo(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
        self.take_back(project, true, cx);
    }

    fn take_back(&mut self, project: Option<ProjectId>, redo: bool, cx: &mut Context<Self>) {
        if self.undoing {
            return;
        }
        let from = if redo { &self.redo } else { &self.undo };
        let Some(step) = from.get(&project).and_then(|s| s.last()).cloned() else {
            return;
        };
        self.undoing = true;
        let started = self.actions.get(&project).copied().unwrap_or(0);
        let applied = step.clone();
        let task = self.write(cx, Key::ErrorSave, move |store| {
            if applied.touches_folders() {
                with_touched(store, |store| applied.apply(store))
            } else {
                Ok((applied.apply(store)?, Vec::new()))
            }
        });
        cx.spawn(async move |ws, cx| {
            let result = task.await;
            let _ = ws.update(cx, |ws, cx| {
                ws.undoing = false;
                // A failed write keeps the step for another try.
                let Ok((inverse, touched)) = result else {
                    return;
                };
                ws.sync_folder_paths(touched, cx);
                // A drag that lifted contacts went back; they stay.
                if !redo && let Undo::TopicPlace { kept, .. } = &step {
                    let locale = crate::settings::locale(cx);
                    for (name, at) in kept {
                        let message =
                            crate::contacts_model::lift_kept(locale, name, *at, &ws.projects);
                        ws.toast(message, ToastKind::Done, cx);
                    }
                }
                let (from, to) = if redo {
                    (&mut ws.redo, &mut ws.undo)
                } else {
                    (&mut ws.undo, &mut ws.redo)
                };
                if let Some(stack) = from.get_mut(&project) {
                    stack.remove(&step);
                }
                let now = ws.actions.get(&project).copied().unwrap_or(0);
                match inverse {
                    Some(inverse) if keeps_inverse(redo, started, now) => {
                        to.entry(project).or_default().push(inverse)
                    }
                    Some(_) => {}
                    None if redo => ws.toast_error(Key::RedoChanged, cx),
                    None => ws.toast_error(Key::UndoChanged, cx),
                }
            });
        })
        .detach();
    }

    /// The stack `Mod+Z` (or `Mod+Shift+Z`) takes on a page of `active`:
    /// its project's, or, when that is empty, the steps that belong to no
    /// project (a new project).
    fn stack_key(
        stacks: &HashMap<Option<ProjectId>, UndoStack>,
        active: Option<ProjectId>,
    ) -> Option<ProjectId> {
        let has = |key| stacks.get(&key).is_some_and(|s| s.last().is_some());
        if has(active) { active } else { None }
    }

    /// `Mod+Z` on a page of `active`.
    pub fn undo_active(&mut self, active: Option<ProjectId>, cx: &mut Context<Self>) {
        let key = Self::stack_key(&self.undo, active);
        self.undo(key, cx);
    }

    /// `Mod+Shift+Z` on a page of `active`.
    pub fn redo_active(&mut self, active: Option<ProjectId>, cx: &mut Context<Self>) {
        let key = Self::stack_key(&self.redo, active);
        self.redo(key, cx);
    }

    /// The order of the step `Mod+Z` would take on a page of `active`.
    pub fn newest_undo(&self, active: Option<ProjectId>) -> Option<u64> {
        self.undo
            .get(&Self::stack_key(&self.undo, active))?
            .newest()
    }

    /// The order of the step `Mod+Shift+Z` would take on a page of `active`.
    pub fn newest_redo(&self, active: Option<ProjectId>) -> Option<u64> {
        self.redo
            .get(&Self::stack_key(&self.redo, active))?
            .newest()
    }

    pub fn toast(&mut self, message: String, kind: ToastKind, cx: &mut Context<Self>) {
        self.toast_with(message, kind, None, cx);
    }

    pub fn toast_with(
        &mut self,
        message: String,
        kind: ToastKind,
        action: Option<ToastAction>,
        cx: &mut Context<Self>,
    ) {
        cx.emit(WorkspaceEvent::Toast(Toast {
            message,
            kind,
            action,
        }));
    }

    pub fn toast_error(&mut self, key: Key, cx: &mut Context<Self>) {
        let locale = crate::settings::locale(cx);
        let message = text(locale, key);
        self.toast(message, ToastKind::Error, cx);
    }
}

/// A contact a move lifted: its name, the level it went to, and the title
/// of the record that needed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lifted {
    pub name: String,
    pub to: ContactLevel,
    pub record: String,
}

/// A topic's epic and its index among that epic's topics.
pub fn topic_place(store: &Store, id: TopicId) -> acta_core::Result<(Option<EpicId>, usize)> {
    Ok((store.topic(id)?.epic, store.order_index(Record::Topic(id))?))
}

/// Put a topic in `epic` at `index` among its topics: a move to the end of
/// that epic when it changes, then the reorder.
fn put_topic(
    store: &mut Store,
    id: TopicId,
    (epic, index): (Option<EpicId>, usize),
    author: Author,
) -> acta_core::Result<()> {
    if store.topic(id)?.epic != epic {
        store.update_topic(
            id,
            TopicPatch {
                epic: Some(epic),
                ..TopicPatch::default()
            },
            author,
        )?;
    }
    store.reorder(Record::Topic(id), index, author)
}

/// Inside a move's transaction, after the move: the lifts it made, as
/// undo steps taken back after the move's own, and as toasts.
pub fn with_lifts(store: &mut Store, moved: Undo) -> acta_core::Result<(Undo, Vec<Lifted>)> {
    let lifts: Vec<Lift> = store.take_lifts();
    if lifts.is_empty() {
        return Ok((moved, Vec::new()));
    }
    let mut steps = vec![moved];
    let mut lifted = Vec::new();
    for l in lifts {
        steps.push(Undo::ContactLevel {
            id: l.contact,
            back: l.from,
            expect: l.to,
        });
        let record = match l.record {
            Record::Topic(t) => store.topic(t)?.title,
            Record::Question(q) => store.question(q)?.text,
            Record::Epic(e) => store.epic(e)?.title,
            Record::Note(n) => store.note(n)?.text.lines().next().unwrap_or("").to_owned(),
            other => other.to_string(),
        };
        lifted.push(Lifted {
            name: store.contact(l.contact)?.name,
            to: l.to,
            record,
        });
    }
    Ok((Undo::Many(steps), lifted))
}

/// The toast of a delete, by kind. Folders are removed, not deleted, and
/// name themselves (components-folders.md 5).
pub fn deleted_key(record: Record) -> Option<Key> {
    Some(match record {
        Record::Project(_) => Key::ProjectDeleted,
        Record::Epic(_) => Key::EpicDeleted,
        Record::Topic(_) => Key::TopicDeleted,
        Record::Entry(_) => Key::EntryDeleted,
        Record::Note(_) => Key::NoteDeleted,
        Record::Kb(_) => Key::KbDeleted,
        Record::Question(_) => Key::QuestionDeleted,
        Record::Contact(_) => Key::ContactDeleted,
        Record::Attachment(_) => Key::AttachRemoved,
        Record::Folder(_) => return None,
    })
}

fn poll(watcher: &Mutex<ChangeWatcher>) -> bool {
    let mut watcher = watcher.lock().unwrap_or_else(|poison| poison.into_inner());
    watcher.changed().unwrap_or(false)
}

/// The kind of a store error, for the log: never a message that could
/// carry user text.
pub fn err_kind(err: &acta_core::Error) -> String {
    let debug = format!("{err:?}");
    debug
        .split(|c: char| !c.is_ascii_alphanumeric())
        .next()
        .unwrap_or("error")
        .to_owned()
}

/// Folders that are not a folder on this computer (blocking: call it off
/// the UI thread and never while holding the store).
/// Each live folder's stored path with its commit columns, sorted.
fn folder_columns(store: &Store) -> acta_core::Result<Vec<(String, CommitColumns)>> {
    let mut all: Vec<(String, CommitColumns)> = store
        .all_folders()?
        .into_iter()
        .map(|f| {
            (
                f.path,
                (f.commit_mode, f.commit_as_user, f.commit_on_default),
            )
        })
        .collect();
    all.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(format!("{:?}", a.1).cmp(&format!("{:?}", b.1)))
    });
    Ok(all)
}

/// The paths whose folders changed between two [`folder_columns`]: one
/// appeared, went or had its commit columns changed.
fn touched_paths(
    before: &[(String, CommitColumns)],
    after: &[(String, CommitColumns)],
) -> Vec<String> {
    let at = |list: &[(String, CommitColumns)], path: &str| -> Vec<CommitColumns> {
        list.iter()
            .filter(|(p, _)| p == path)
            .map(|(_, c)| *c)
            .collect()
    };
    let mut out: Vec<String> = Vec::new();
    for (path, _) in before.iter().chain(after) {
        if !out.contains(path) && at(before, path) != at(after, path) {
            out.push(path.clone());
        }
    }
    out
}

/// Runs a write of folders and returns, with its result, the paths whose
/// Claude Code settings it may change.
fn with_touched<T>(
    store: &mut Store,
    f: impl FnOnce(&mut Store) -> acta_core::Result<T>,
) -> acta_core::Result<(T, Vec<String>)> {
    let before = folder_columns(store)?;
    let done = f(store)?;
    let after = folder_columns(store)?;
    Ok((done, touched_paths(&before, &after)))
}

/// Claude Code's settings files of `paths` in line with the store's
/// columns: written for a path still attached, taken out (as the ledger
/// lists) for one no live folder has. A folder that cannot be written now
/// (offline, a malformed file) is left for the next sync; the log says how
/// many.
fn sync_paths(store: &mut Store, paths: &[String]) {
    let mut kept = 0;
    for path in paths {
        let live = store.all_folders().is_ok_and(|all| {
            all.iter()
                .any(|f| acta_core::folder_path::same(acta_core::paths::Os::CURRENT, &f.path, path))
        });
        let ok = if live {
            acta_core::claude_settings::sync_path(store, Path::new(path)).is_ok()
        } else {
            !matches!(
                acta_core::claude_settings::remove(store, Path::new(path)),
                Ok(acta_core::claude_settings::Restore::Kept) | Err(_)
            )
        };
        kept += usize::from(!ok);
    }
    if kept > 0 {
        crate::log::line(&format!(
            "folders: {kept} Claude Code settings file(s) left as they were"
        ));
    }
}

pub fn missing_paths(paths: impl IntoIterator<Item = String>) -> std::collections::HashSet<String> {
    paths
        .into_iter()
        .filter(|p| !folder_identity::is_dir(Path::new(p)))
        .collect()
}

/// The workspace of the app, for code that only has an `App`.
pub fn project_of(ws: &gpui_kit::Entity<Workspace>, owner: Owner, cx: &App) -> Option<ProjectId> {
    ws.read(cx).project_of(owner)
}

/// The active folder a page chose, `None` when it follows the default rule.
/// The due date of a project, epic or topic as stored.
pub fn due_on(store: &Store, owner: Owner) -> acta_core::Result<Option<acta_core::date::Date>> {
    Ok(match owner {
        Owner::Project(id) => store.project(id)?.due_on,
        Owner::Epic(id) => store.epic(id)?.due_on,
        Owner::Topic(id) => store.topic(id)?.due_on,
    })
}

pub fn active_pointer(store: &Store, owner: Owner) -> acta_core::Result<Option<FolderId>> {
    Ok(match owner {
        Owner::Project(id) => store.project(id)?.active_folder,
        Owner::Epic(id) => store.epic(id)?.active_folder,
        Owner::Topic(id) => store.topic(id)?.active_folder,
    })
}

/// The order a batch of restores runs in: what contains first.
fn restore_rank(record: Record) -> u8 {
    match record {
        Record::Project(_) => 0,
        Record::Epic(_) => 1,
        Record::Topic(_) => 2,
        Record::Note(_) | Record::Contact(_) => 3,
        Record::Entry(_) | Record::Kb(_) | Record::Folder(_) | Record::Question(_) => 4,
        Record::Attachment(_) => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::temp_store;
    use acta_core::store::NewTopic;

    #[test]
    fn undoing_and_redoing_a_memory_edit_checks_its_pairs_again() {
        use acta_core::model::{KbKind, KbPairKind, KbPairState};
        use acta_core::store::{KbEdit, KbUpdate, NewKb};
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, _) = topic(store);
        let memory = |title: &str, summary: &str, body: &str| NewKb {
            owner: Owner::Project(project),
            kind: KbKind::Memory,
            title: title.into(),
            summary: summary.into(),
            body: body.into(),
            from_file: false,
        };
        let theirs = store
            .kb_create(
                memory(
                    "Store tests in parallel",
                    "The store tests run in parallel",
                    "cargo test runs the store tests in parallel.",
                ),
                Author::Claude,
            )
            .unwrap();
        let mine = store
            .kb_create(
                memory(
                    "Store tests in parallel",
                    "Never run the store tests in parallel",
                    "They share a key store.",
                ),
                Author::Human,
            )
            .unwrap();
        store.kb_recheck(mine, Author::Human).unwrap();
        let state =
            |store: &Store| kb_pair_state(store, mine, theirs, KbPairKind::Conflict).unwrap();
        assert_eq!(state(store), KbPairState::Open);
        // Alexandre rewrites it about something else: the pair settles.
        let back = store.kb_entry(mine).unwrap().rev;
        let expect = store
            .kb_update(
                mine,
                KbUpdate {
                    expected_rev: None,
                    title: Some("Keychain entry name".into()),
                    summary: Some("Alexandre's key lives in the keychain".into()),
                    edit: KbEdit::Body("The keychain entry is named acta.db.".into()),
                },
                Author::Human,
            )
            .unwrap();
        store.kb_recheck(mine, Author::Human).unwrap();
        assert_eq!(state(store), KbPairState::Resolved);
        // Undo brings the text and the open pair back; redo settles it again.
        let redo = Undo::KbRevision {
            id: mine,
            back,
            expect,
        }
        .apply(store)
        .unwrap()
        .expect("nothing changed since");
        assert_eq!(state(store), KbPairState::Open);
        redo.apply(store).unwrap().expect("nothing changed since");
        assert_eq!(state(store), KbPairState::Resolved);
    }

    #[test]
    fn deleting_a_topic_takes_its_folders_settings_out_and_undo_puts_them_back() {
        use acta_core::claude_settings::{self, LOCAL_SETTINGS};
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (_, id) = topic(store);
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("embed-terminal");
        std::fs::create_dir_all(&repo).unwrap();
        let out = acta_core::git_cmd::command(&repo)
            .args(["init", "-q", "-b", "main"])
            .output()
            .unwrap();
        assert!(out.status.success());
        let folder = store
            .add_folder(Owner::Topic(id), &repo, Author::Human)
            .unwrap();
        let stored = store.folder(folder).unwrap().path;
        let file = std::path::Path::new(&stored).join(LOCAL_SETTINGS);
        let has_push = |file: &std::path::Path| {
            std::fs::read_to_string(file).is_ok_and(|t| t.contains("Bash(git push)"))
        };
        // "Commit and push" set from the form: the rules are written.
        let (_, touched) = with_touched(store, |s| {
            s.set_commit_settings(folder, CommitMode::Push, None, false, Author::Human)
        })
        .unwrap();
        assert_eq!(touched, std::slice::from_ref(&stored));
        sync_paths(store, &touched);
        assert!(has_push(&file));

        // The topic deleted: its folder goes with it, and so do the rules.
        let (undo, touched) = with_touched(store, |s| {
            s.delete(Record::Topic(id), Author::Human)?;
            Ok(Undo::Restore(Record::Topic(id)))
        })
        .unwrap();
        assert!(undo.touches_folders());
        assert_eq!(touched, std::slice::from_ref(&stored));
        sync_paths(store, &touched);
        assert!(!has_push(&file), "allow rules removed");

        // Undo: the topic, its folder and the rules are back.
        let (inverse, touched) = with_touched(store, |s| undo.apply(s)).unwrap();
        assert!(inverse.is_some());
        sync_paths(store, &touched);
        assert!(has_push(&file), "allow rules back");
        assert!(
            claude_settings::commit_settings_for(store, &stored)
                .unwrap()
                .is_some()
        );
        // Nothing else changed: nothing to sync.
        let ((), touched) = with_touched(store, |_| Ok(())).unwrap();
        assert!(touched.is_empty());
    }

    fn topic(store: &mut Store) -> (ProjectId, TopicId) {
        let project = store.create_project("Acta", "", Author::Human).unwrap();
        let topic = store
            .create_topic(
                NewTopic {
                    project,
                    epic: None,
                    title: "Embed the terminal".into(),
                    description: String::new(),
                    status: Status::Planned,
                },
                Author::Human,
            )
            .unwrap();
        (project, topic)
    }

    fn set_status(store: &mut Store, id: TopicId, status: Status, by: Author) {
        store
            .update_topic(
                id,
                TopicPatch {
                    status: Some(status),
                    ..TopicPatch::default()
                },
                by,
            )
            .unwrap();
    }

    #[test]
    fn an_entry_edit_and_a_note_edit_are_undone_and_only_while_unchanged() {
        use acta_core::model::EntryKind;
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let e = store
            .log(
                id,
                acta_core::store::NewEntry {
                    kind: EntryKind::Progress,
                    text: "Alexandre wrote this".into(),
                    state: None,
                    status: None,
                },
                Author::Human,
            )
            .unwrap();
        store
            .update_entry(
                e.id,
                "Alexandre decided this",
                EntryKind::Decision,
                Author::Human,
            )
            .unwrap();
        let undo = Undo::Entry {
            id: e.id,
            back: ("Alexandre wrote this".into(), EntryKind::Progress),
            expect: ("Alexandre decided this".into(), EntryKind::Decision),
        };
        assert!(undo.apply(store).unwrap().is_some());
        let back = store.entry(e.id).unwrap();
        assert_eq!(
            (back.text.as_str(), back.kind),
            ("Alexandre wrote this", EntryKind::Progress)
        );
        // Applied twice: the store no longer holds what the edit left.
        assert!(undo.apply(store).unwrap().is_none());

        let note = store
            .add_note(project, "Alexandre's note", Author::Human)
            .unwrap();
        store
            .update_note(note, "Alexandre's better note", Author::Human)
            .unwrap();
        let undo = Undo::Note {
            id: note,
            back: "Alexandre's note".into(),
            expect: "Alexandre's better note".into(),
        };
        assert!(undo.apply(store).unwrap().is_some());
        assert_eq!(store.note(note).unwrap().text, "Alexandre's note");
        assert!(undo.apply(store).unwrap().is_none());
    }

    #[test]
    fn undo_puts_back_what_the_human_changed() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (_, id) = topic(store);
        set_status(store, id, Status::Active, Author::Human);
        let step = Undo::Status {
            id,
            back: Status::Planned,
            expect: Status::Active,
        };
        assert!(step.apply(store).unwrap().is_some());
        assert_eq!(store.topic(id).unwrap().status, Status::Planned);

        store.delete(Record::Topic(id), Author::Human).unwrap();
        assert!(
            Undo::Restore(Record::Topic(id))
                .apply(store)
                .unwrap()
                .is_some()
        );
        assert!(store.topic(id).is_ok());
    }

    #[test]
    fn choosing_an_active_folder_is_undone_and_redone() {
        let mut temp = temp_store();
        let dir = tempfile::tempdir().unwrap();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let owner = Owner::Topic(id);
        let mut folders = Vec::new();
        for name in ["acta", "docs"] {
            let path = dir.path().join(name);
            std::fs::create_dir(&path).unwrap();
            folders.push(
                store
                    .add_folder(Owner::Project(project), &path, Author::Human)
                    .unwrap(),
            );
        }
        store
            .set_active_folder(owner, Some(folders[1]), Author::Human)
            .unwrap();
        let step = Undo::ActiveFolder {
            owner,
            back: None,
            expect: Some(folders[1]),
        };
        let redo = step.apply(store).unwrap().expect("undone");
        assert_eq!(active_pointer(store, owner).unwrap(), None);
        assert!(redo.apply(store).unwrap().is_some());
        assert_eq!(active_pointer(store, owner).unwrap(), Some(folders[1]));
        // Changed since: the undo is skipped.
        store
            .set_active_folder(owner, Some(folders[0]), Author::Human)
            .unwrap();
        assert!(step.apply(store).unwrap().is_none());
    }

    /// "For this page" is one undoable step, skipped once changed since.
    #[test]
    fn a_page_commit_choice_is_undone_and_redone() {
        let mut temp = temp_store();
        let dir = tempfile::tempdir().unwrap();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let owner = Owner::Topic(id);
        let folder = store
            .add_folder(Owner::Project(project), dir.path(), Author::Human)
            .unwrap();
        let push = Some(PageCommit {
            mode: CommitMode::Push,
            as_user: None,
            on_default: false,
        });
        page::set_page_commit(store, owner, folder, push, Author::Human).unwrap();
        let step = Undo::PageCommit {
            owner,
            folder,
            back: None,
            expect: push,
        };
        let redo = step.apply(store).unwrap().expect("undone");
        assert_eq!(page::page_commit(store, owner, folder).unwrap(), None);
        assert!(redo.apply(store).unwrap().is_some());
        assert_eq!(page::page_commit(store, owner, folder).unwrap(), push);
        page::set_page_commit(store, owner, folder, None, Author::Human).unwrap();
        assert!(step.apply(store).unwrap().is_none());
        // The folder gone since: skipped too.
        page::set_page_commit(store, owner, folder, push, Author::Human).unwrap();
        store.delete(Record::Folder(folder), Author::Human).unwrap();
        assert!(step.apply(store).unwrap().is_none());
    }

    #[test]
    fn a_due_date_is_undone_and_redone_and_never_over_claude_code() {
        use acta_core::date::Date;
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let day = Date::parse_iso("2026-12-15");
        for owner in [Owner::Topic(id), Owner::Project(project)] {
            store.set_due(owner, day, Author::Human).unwrap();
            let step = Undo::Due {
                owner,
                back: None,
                expect: day,
            };
            let redo = step.apply(store).unwrap().expect("undone");
            assert_eq!(due_on(store, owner).unwrap(), None);
            assert!(redo.apply(store).unwrap().is_some());
            assert_eq!(due_on(store, owner).unwrap(), day);
            // Claude Code changed it since: the undo is skipped.
            let later = Date::parse_iso("2026-12-20");
            store.set_due(owner, later, Author::Claude).unwrap();
            assert!(step.apply(store).unwrap().is_none());
            assert_eq!(due_on(store, owner).unwrap(), later);
        }
    }

    #[test]
    fn an_answer_and_its_decision_are_undone_and_redone_as_one_step() {
        use acta_core::store::NewQuestion;
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (_, id) = topic(store);
        let q = store
            .ask(
                NewQuestion {
                    owner: Owner::Topic(id),
                    text: "Keep v1 until March?".into(),
                    asked_to: None,
                    expected_on: None,
                },
                Author::Human,
            )
            .unwrap();
        let back = store.question(q).unwrap();
        let entry = store
            .answer(q, Some("Yes."), true, Author::Human)
            .unwrap()
            .expect("logged");
        let step = Undo::Many(vec![
            Undo::Question {
                back: Box::new(back),
                expect: Box::new(store.question(q).unwrap()),
            },
            Undo::Delete(Record::Entry(entry.id)),
        ]);
        let redo = step.apply(store).unwrap().expect("undone");
        assert!(store.question(q).unwrap().is_open());
        assert!(store.entry(entry.id).is_err(), "the decision went too");
        assert!(redo.apply(store).unwrap().is_some());
        assert!(!store.question(q).unwrap().is_open());
        assert!(store.entry(entry.id).is_ok());
    }

    #[test]
    fn report_to_and_a_contact_made_inline_go_back_together() {
        use acta_core::model::ContactLevel;
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let record = Record::Topic(id);
        let marie = store
            .create_contact("Marie", ContactLevel::Project(project), Author::Human)
            .unwrap();
        store
            .set_report_to(record, Some(marie), Author::Human)
            .unwrap();
        store.set_reported(record, true, Author::Human).unwrap();
        let step = Undo::Many(vec![
            Undo::ReportTo {
                record,
                back: (None, false),
                expect: (Some(marie), true),
            },
            Undo::Delete(Record::Contact(marie)),
        ]);
        let redo = step.apply(store).unwrap().expect("undone");
        assert_eq!(report_state(store, record).unwrap(), (None, false));
        assert!(store.contact(marie).is_err());
        assert!(redo.apply(store).unwrap().is_some());
        assert_eq!(report_state(store, record).unwrap(), (Some(marie), true));
    }

    #[test]
    fn a_merge_is_undone_with_its_references() {
        use acta_core::model::ContactLevel;
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let level = ContactLevel::Project(project);
        let a = store.create_contact("Marie", level, Author::Human).unwrap();
        let b = store
            .create_contact("Marie Dubois", level, Author::Human)
            .unwrap();
        store
            .set_report_to(Record::Topic(id), Some(a), Author::Human)
            .unwrap();
        let before = store.contact(b).unwrap();
        let uses: Vec<Record> = store
            .contact_uses(a)
            .unwrap()
            .into_iter()
            .map(|u| u.record)
            .collect();
        let trace = store.merge_contacts(a, b, Author::Human).unwrap();
        let step = Undo::Unmerge {
            from: a,
            before: Box::new(before),
            after: Box::new(store.contact(b).unwrap()),
            uses,
            trace,
        };
        // Changed since the merge: the undo is skipped.
        store
            .update_contact(
                b,
                acta_core::store::ContactPatch {
                    company: Some(Some("Acme".into())),
                    ..Default::default()
                },
                Author::Human,
            )
            .unwrap();
        assert!(step.apply(store).unwrap().is_none());
        store
            .update_contact(
                b,
                acta_core::store::ContactPatch {
                    company: Some(None),
                    ..Default::default()
                },
                Author::Human,
            )
            .unwrap();
        let redo = step.apply(store).unwrap().expect("undone");
        assert_eq!(report_state(store, Record::Topic(id)).unwrap().0, Some(a));
        assert!(redo.apply(store).unwrap().is_some());
        assert_eq!(report_state(store, Record::Topic(id)).unwrap().0, Some(b));
    }

    #[test]
    fn undo_of_back_to_inbox_files_the_note_again_exactly() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let note = store
            .add_note(project, "Alexandre: embed the terminal", Author::Human)
            .unwrap();
        store.file_notes(&[note], id, Author::Claude).unwrap();
        let before = store.note(note).unwrap();
        store.unfile_note(note, Author::Human).unwrap();
        let step = Undo::Refile(Box::new(before.clone()));
        assert!(step.apply(store).unwrap().is_some());
        let after = store.note(note).unwrap();
        assert_eq!(
            (after.fate(), after.filed_at, after.filed_by),
            (before.fate(), before.filed_at, Some(Author::Claude))
        );
        // Filed again by someone else in between: the undo is skipped.
        store.unfile_note(note, Author::Human).unwrap();
        store.dismiss_notes(&[note], Author::Claude).unwrap();
        assert!(step.apply(store).unwrap().is_none());
        assert_eq!(
            store.note(note).unwrap().fate(),
            acta_core::model::NoteFate::Dismissed
        );
    }

    #[test]
    fn undo_never_reverts_a_later_write_by_claude_code() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (_, id) = topic(store);
        set_status(store, id, Status::Active, Author::Human);
        // Claude Code writes after the human.
        set_status(store, id, Status::Blocked, Author::Claude);
        let step = Undo::Status {
            id,
            back: Status::Planned,
            expect: Status::Active,
        };
        assert!(step.apply(store).unwrap().is_none(), "skipped");
        assert_eq!(store.topic(id).unwrap().status, Status::Blocked);
        // A restore whose record is live again is skipped too.
        assert!(
            Undo::Restore(Record::Topic(id))
                .apply(store)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn redo_puts_back_what_undo_took_and_never_over_a_newer_write() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (_, id) = topic(store);
        set_status(store, id, Status::Active, Author::Human);
        let undo = Undo::Status {
            id,
            back: Status::Planned,
            expect: Status::Active,
        };
        let redo = undo.apply(store).unwrap().expect("undone");
        assert_eq!(store.topic(id).unwrap().status, Status::Planned);
        let again = redo.apply(store).unwrap().expect("redone");
        assert_eq!(store.topic(id).unwrap().status, Status::Active);
        assert_eq!(again, undo, "the redo's inverse is the undo again");
        // Undone, then Claude Code writes: the redo is skipped.
        let redo = again.apply(store).unwrap().expect("undone");
        set_status(store, id, Status::Blocked, Author::Claude);
        assert!(redo.apply(store).unwrap().is_none());
        assert_eq!(store.topic(id).unwrap().status, Status::Blocked);
    }

    #[test]
    fn a_note_made_into_a_topic_is_redone_as_one_step() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, _) = topic(store);
        let note = store
            .add_note(project, "Alexandre: scrollback", Author::Human)
            .unwrap();
        let made = store
            .note_to_topic(
                note,
                NewTopic {
                    project,
                    epic: None,
                    title: "Scrollback".into(),
                    description: String::new(),
                    status: Status::Draft,
                },
                Author::Human,
            )
            .unwrap();
        let undo = Undo::Many(vec![Undo::Unfile(note), Undo::Delete(Record::Topic(made))]);
        let redo = undo.apply(store).unwrap().expect("undone");
        assert!(store.note(note).unwrap().is_open());
        assert!(store.topic(made).is_err());
        redo.apply(store).unwrap().expect("redone");
        assert!(store.topic(made).is_ok());
        let filed = store.note(note).unwrap();
        assert!(!filed.is_open());
        assert_eq!(filed.filed_by, Some(Author::Human));
    }

    #[test]
    fn a_knowledge_move_undoes_and_redoes() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let kb = store
            .kb_create(
                acta_core::store::NewKb {
                    owner: Owner::Topic(id),
                    kind: acta_core::model::KbKind::Memory,
                    title: "Alexandre prefers tabs".into(),
                    summary: "Tabs, not spaces".into(),
                    body: "Tabs.".into(),
                    from_file: false,
                },
                Author::Human,
            )
            .unwrap();
        store
            .kb_move(kb, Owner::Project(project), Author::Human)
            .unwrap();
        let undo = Undo::KbOwner {
            id: kb,
            back: Owner::Topic(id),
            expect: Owner::Project(project),
        };
        let redo = undo.apply(store).unwrap().expect("undone");
        assert_eq!(store.kb_entry(kb).unwrap().owner, Owner::Topic(id));
        redo.apply(store).unwrap().expect("redone");
        assert_eq!(store.kb_entry(kb).unwrap().owner, Owner::Project(project));
    }

    #[test]
    fn an_undo_overtaken_by_a_new_action_leaves_no_redo() {
        assert!(keeps_inverse(false, 3, 3));
        assert!(
            !keeps_inverse(false, 3, 4),
            "a newer write cleared the redo"
        );
        assert!(keeps_inverse(true, 3, 4), "a redo is undoable again");
    }

    #[test]
    fn many_steps_undo_together_or_not_at_all() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (_, a) = topic(store);
        let (_, b) = topic(store);
        set_status(store, a, Status::Active, Author::Human);
        set_status(store, b, Status::Active, Author::Human);
        set_status(store, b, Status::Done, Author::Claude);
        let both = Undo::Many(vec![
            Undo::Status {
                id: a,
                back: Status::Planned,
                expect: Status::Active,
            },
            Undo::Status {
                id: b,
                back: Status::Planned,
                expect: Status::Active,
            },
        ]);
        assert!(both.apply(store).unwrap().is_none());
        assert_eq!(
            store.topic(a).unwrap().status,
            Status::Active,
            "a untouched"
        );
    }

    #[test]
    fn undoing_a_locate_puts_back_a_path_that_no_longer_exists() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, _) = topic(store);
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let new = dir.path().join("new");
        std::fs::create_dir(&old).unwrap();
        std::fs::create_dir(&new).unwrap();
        let id = store
            .add_folder(Owner::Project(project), &old, Author::Human)
            .unwrap();
        let stored = store.folder(id).unwrap().path;
        std::fs::remove_dir(&old).unwrap();
        store.set_folder_path(id, &new, Author::Human).unwrap();
        let now = store.folder(id).unwrap().path;
        let step = Undo::FolderPath {
            id,
            back: stored.clone(),
            expect: now,
        };
        assert!(step.apply(store).unwrap().is_some());
        assert_eq!(store.folder(id).unwrap().path, stored);
    }

    #[test]
    fn a_reorder_undoes_to_its_old_place() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, a) = topic(store);
        let b = store
            .create_topic(
                NewTopic {
                    project,
                    epic: None,
                    title: "B".into(),
                    description: String::new(),
                    status: Status::Planned,
                },
                Author::Human,
            )
            .unwrap();
        store.reorder(Record::Topic(b), 0, Author::Human).unwrap();
        let step = Undo::Reorder {
            record: Record::Topic(b),
            back: 1,
            expect: 0,
        };
        assert!(step.apply(store).unwrap().is_some());
        assert_eq!(store.order_index(Record::Topic(a)).unwrap(), 0);
    }

    #[test]
    fn a_topic_dropped_in_another_epic_goes_back_to_its_place_in_one_step() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, a) = topic(store);
        let new = |store: &mut Store, title: &str, epic| {
            store
                .create_topic(
                    NewTopic {
                        project,
                        epic,
                        title: title.into(),
                        description: String::new(),
                        status: Status::Planned,
                    },
                    Author::Human,
                )
                .unwrap()
        };
        let b = new(store, "B", None);
        let epic = store
            .create_epic(project, "Terminal", "", Author::Human)
            .unwrap();
        let x = new(store, "X", Some(epic));
        let y = new(store, "Y", Some(epic));
        // A (first of the loose topics) dropped between X and Y.
        let back = topic_place(store, a).unwrap();
        assert_eq!(back, (None, 0));
        put_topic(store, a, (Some(epic), 1), Author::Human).unwrap();
        let order = |store: &Store, e| -> Vec<TopicId> {
            store
                .tree(project)
                .unwrap()
                .epics
                .iter()
                .find(|(x, _)| Some(x.id) == e)
                .map(|(_, ts)| ts.iter().map(|t| t.id).collect())
                .unwrap_or_else(|| {
                    store
                        .tree(project)
                        .unwrap()
                        .loose_topics
                        .iter()
                        .map(|t| t.id)
                        .collect()
                })
        };
        assert_eq!(order(store, Some(epic)), vec![x, a, y]);
        let step = Undo::TopicPlace {
            id: a,
            back,
            expect: topic_place(store, a).unwrap(),
            kept: Vec::new(),
        };
        let redo = step.apply(store).unwrap().expect("unchanged since");
        assert_eq!(order(store, None), vec![a, b]);
        assert_eq!(order(store, Some(epic)), vec![x, y]);
        // The redo puts it back between X and Y.
        assert!(redo.apply(store).unwrap().is_some());
        assert_eq!(order(store, Some(epic)), vec![x, a, y]);
        // Moved since by someone else: the step is refused.
        store.reorder(Record::Topic(a), 0, Author::Claude).unwrap();
        assert!(step.apply(store).unwrap().is_none());
    }

    #[test]
    fn a_move_to_another_epic_undoes_while_unchanged() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (project, id) = topic(store);
        let epic = store
            .create_epic(project, "Terminal", "", Author::Human)
            .unwrap();
        let patch = |epic| TopicPatch {
            epic: Some(epic),
            ..TopicPatch::default()
        };
        store
            .update_topic(id, patch(Some(epic)), Author::Human)
            .unwrap();
        let step = Undo::TopicEpic {
            id,
            back: None,
            expect: Some(epic),
        };
        assert!(step.apply(store).unwrap().is_some());
        assert_eq!(store.topic(id).unwrap().epic, None);
        // Moved again by someone else: skipped.
        store
            .update_topic(id, patch(Some(epic)), Author::Claude)
            .unwrap();
        store.update_topic(id, patch(None), Author::Claude).unwrap();
        assert!(step.apply(store).unwrap().is_none());
    }

    #[test]
    fn trees_load_for_every_project_in_order() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        topic(store);
        store.create_project("Atlas", "", Author::Human).unwrap();
        let trees = load_trees(store).unwrap();
        let titles: Vec<_> = trees.iter().map(|t| t.project.title.as_str()).collect();
        assert_eq!(titles, ["Acta", "Atlas"]);
    }

    #[test]
    fn the_undo_stack_keeps_the_newest_hundred() {
        let mut stack = UndoStack::default();
        for n in 0..150 {
            stack.push(Undo::Restore(Record::Topic(TopicId(n))));
        }
        assert_eq!(stack.len(), UNDO_DEPTH);
        assert_eq!(
            stack.pop(),
            Some(Undo::Restore(Record::Topic(TopicId(149))))
        );
    }

    #[test]
    fn a_finished_undo_removes_its_own_step_not_a_newer_one() {
        let mut stack = UndoStack::default();
        let old = Undo::Restore(Record::Topic(TopicId(1)));
        let newer = Undo::Restore(Record::Topic(TopicId(2)));
        stack.push(old.clone());
        stack.push(newer.clone());
        stack.remove(&old);
        assert_eq!(stack.pop(), Some(newer));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn error_kinds_carry_no_text() {
        let err = acta_core::Error::Invalid("title of Alexandre's secret".into());
        assert_eq!(err_kind(&err), "Invalid");
    }

    #[test]
    fn mod_z_takes_the_newest_of_layout_steps_and_writes() {
        use crate::layout::{Layout, UndoLog, layout_step_first};
        let mut writes = UndoStack::default();
        let mut layout = UndoLog::default();
        let step = Undo::Delete(Record::Topic(TopicId(1)));
        // A write, then a layout change, then a write.
        writes.push(step.clone());
        layout.record(Layout::defaults(Vec::new(), true), next_undo_seq(), None);
        assert!(layout_step_first(layout.newest(), writes.newest()));
        writes.push(Undo::Delete(Record::Topic(TopicId(2))));
        assert!(!layout_step_first(layout.newest(), writes.newest()));
        // The write is taken back: the layout step is now the newest.
        writes.pop();
        assert!(layout_step_first(layout.newest(), writes.newest()));
        layout.undo(Layout::defaults(Vec::new(), true), next_undo_seq());
        assert!(!layout_step_first(layout.newest(), writes.newest()));
        assert!(!layout_step_first(None, None));
        assert!(layout_step_first(Some(3), None));
    }
}
