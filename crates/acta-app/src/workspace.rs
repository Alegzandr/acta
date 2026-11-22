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

use acta_core::folder_identity::{self, Known, Monitor};
use acta_core::model::{
    Author, CommitMode, EpicId, FolderId, KbId, Note, NoteId, Owner, ProjectId, ProjectTree,
    Record, Status, TopicId,
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Several steps taken back as one, all or none.
    Many(Vec<Undo>),
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
            Undo::FolderPath { id, expect, .. } => store.folder(*id)?.path == *expect,
            Undo::KbRevision { id, expect, .. } => store.kb_entry(*id)?.rev == *expect,
            Undo::KbOwner { id, expect, .. } => store.kb_entry(*id)?.owner == *expect,
            Undo::Reorder { record, expect, .. } => store.order_index(*record)? == *expect,
            Undo::Entry { id, expect, .. } => {
                let e = store.entry(*id)?;
                (e.text, e.kind) == *expect
            }
            Undo::Note { id, expect, .. } => store.note(*id)?.text == *expect,
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
                Undo::KbRevision {
                    id: *id,
                    back: *expect,
                    expect: store.kb_entry(*id)?.rev,
                }
            }
            Undo::KbOwner { id, back, expect } => {
                store.kb_move(*id, *back, me)?;
                Undo::KbOwner {
                    id: *id,
                    back: *expect,
                    expect: store.kb_entry(*id)?.owner,
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
    pub bulk: Option<BulkOffer>,
    pub projects: Vec<ProjectTree>,
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
            unread: Unread::default(),
            revision: 0,
            git_epoch: 0,
            undo: HashMap::new(),
            redo: HashMap::new(),
            actions: HashMap::new(),
            undoing: false,
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
            ))
        });
        self._reload = cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |ws, cx| match loaded {
                Ok((projects, unread, folders)) => {
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
        let task = self.db.run(cx, f);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |ws, cx| {
                ws.reload(cx);
                match result {
                    Ok(value) => Ok(value),
                    Err(acta_core::Error::StaleRevision { .. }) => Err(WriteError::Stale),
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
        self.undoable(cx, project, None, move |store| {
            let back = store.topic(id)?.epic;
            store.update_topic(
                id,
                TopicPatch {
                    epic: Some(epic),
                    ..TopicPatch::default()
                },
                Author::Human,
            )?;
            Ok((
                Undo::TopicEpic {
                    id,
                    back,
                    expect: epic,
                },
                (),
            ))
        });
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
        cx.emit(WorkspaceEvent::Toast(Toast { message, kind }));
    }

    pub fn toast_error(&mut self, key: Key, cx: &mut Context<Self>) {
        let locale = crate::settings::locale(cx);
        let message = text(locale, key);
        self.toast(message, ToastKind::Error, cx);
    }
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
        Record::Folder(_) | Record::Question(_) => return None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::temp_store;
    use acta_core::store::NewTopic;

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
