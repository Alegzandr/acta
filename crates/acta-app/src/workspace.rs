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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acta_core::model::{
    Author, EpicId, FolderId, KbId, Note, NoteId, Owner, ProjectId, ProjectTree, Record, Status,
    TopicId,
};
use acta_core::store::{EpicPatch, ProjectPatch, TopicPatch};
use acta_core::{ChangeWatcher, Store};
use gpui_kit::{App, Context, EventEmitter, Task};

use crate::db::Db;
use crate::i18n::{Arg, Key, text, tr};

/// docs/design/interaction.md 6.5: changes land within 100 ms of the write.
const POLL: Duration = Duration::from_millis(100);
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
    /// A folder set or located: put the old path back as stored.
    FolderPath {
        id: FolderId,
        back: String,
        expect: String,
    },
    IncludeInherited {
        owner: Owner,
        back: bool,
    },
    KbRevision {
        id: KbId,
        back: i64,
        expect: i64,
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

fn is_live(store: &Store, record: Record) -> acta_core::Result<bool> {
    let found = match record {
        Record::Project(id) => store.project(id).map(|_| ()),
        Record::Epic(id) => store.epic(id).map(|_| ()),
        Record::Topic(id) => store.topic(id).map(|_| ()),
        Record::Entry(id) => store.entry(id).map(|_| ()),
        Record::Note(id) => store.note(id).map(|_| ()),
        Record::Kb(id) => store.kb_entry(id).map(|_| ()),
        Record::Folder(id) => store.folder(id).map(|_| ()),
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
            Undo::FolderPath { id, expect, .. } => store.folder(*id)?.path == *expect,
            Undo::IncludeInherited { owner, back } => {
                let current = match owner {
                    Owner::Project(_) => false,
                    Owner::Epic(id) => store.epic(*id)?.include_inherited_folders,
                    Owner::Topic(id) => store.topic(*id)?.include_inherited_folders,
                };
                current == !*back
            }
            Undo::KbRevision { id, expect, .. } => store.kb_entry(*id)?.rev == *expect,
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

    fn revert(&self, store: &mut Store) -> acta_core::Result<()> {
        let me = Author::Human;
        match self {
            Undo::Restore(record) => store.restore(*record, me),
            Undo::Delete(record) => store.delete(*record, me),
            Undo::Status { id, back, .. } => store.update_topic(
                *id,
                TopicPatch {
                    status: Some(*back),
                    ..TopicPatch::default()
                },
                me,
            ),
            Undo::TopicTitle { id, back, .. } => store.update_topic(
                *id,
                TopicPatch {
                    title: Some(back.clone()),
                    ..TopicPatch::default()
                },
                me,
            ),
            Undo::EpicTitle { id, back, .. } => store.update_epic(
                *id,
                EpicPatch {
                    title: Some(back.clone()),
                    ..EpicPatch::default()
                },
                me,
            ),
            Undo::TopicEpic { id, back, .. } => store.update_topic(
                *id,
                TopicPatch {
                    epic: Some(*back),
                    ..TopicPatch::default()
                },
                me,
            ),
            Undo::ProjectTitle { id, back, .. } => store.update_project(
                *id,
                ProjectPatch {
                    title: Some(back.clone()),
                    ..ProjectPatch::default()
                },
                me,
            ),
            Undo::FolderPath { id, back, .. } => store.restore_folder_path(*id, back, me),
            Undo::IncludeInherited { owner, back } => {
                store.set_include_inherited(*owner, *back, me)
            }
            Undo::KbRevision { id, back, .. } => {
                store.kb_restore_revision(*id, *back, me).map(|_| ())
            }
            Undo::Reorder { record, back, .. } => store.reorder(*record, *back, me),
            Undo::Entry { id, back, .. } => store.update_entry(*id, &back.0, back.1, me),
            Undo::Note { id, back, .. } => store.update_note(*id, back, me),
            Undo::Unfile(id) => store.unfile_note(*id, me),
            Undo::Refile(before) => store.restore_note_fate(before.id, before, me),
            Undo::Many(all) => all.iter().try_for_each(|undo| undo.revert(store)),
        }
    }

    /// Check and take back in one transaction. `Ok(false)`: something else
    /// changed it since, nothing was written.
    pub fn apply(&self, store: &mut Store) -> acta_core::Result<bool> {
        store.atomically(|store| {
            if !self.matches(store)? {
                return Ok(false);
            }
            self.revert(store)?;
            Ok(true)
        })
    }
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
    /// Bumped on every reload, by any process's commit.
    pub revision: u64,
    undo: HashMap<Option<ProjectId>, UndoStack>,
    /// An undo is being written; a second `Mod+Z` waits for it.
    undoing: bool,
    _reload: Task<()>,
    _watch: Task<()>,
    _snapshots: Task<()>,
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
        let mut workspace = Self {
            db,
            bulk: None,
            projects: Vec::new(),
            revision: 0,
            undo: HashMap::new(),
            undoing: false,
            _reload: Task::ready(()),
            _watch: watch,
            _snapshots: snapshots,
        };
        workspace.reload(cx);
        workspace
    }

    /// Reload the tree; pages observing the workspace reload their data.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let task = self.db.run(cx, |store| load_trees(store));
        self._reload = cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |ws, cx| match loaded {
                Ok(projects) => {
                    ws.projects = projects;
                    ws.revision += 1;
                    cx.emit(WorkspaceEvent::Reloaded);
                    cx.notify();
                }
                Err(err) => crate::log::line(&format!("store: reload failed: {}", err_kind(&err))),
            });
        });
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
        cx.spawn(async move |this, cx| {
            let (undo, value) = task.await?;
            let _ = this.update(cx, |ws, cx| {
                ws.undo.entry(project).or_default().push(undo);
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
    pub fn delete(&mut self, record: Record, project: Option<ProjectId>, cx: &mut Context<Self>) {
        let locale = crate::settings::locale(cx);
        let message = deleted_key(record).map(|key| text(locale, key));
        self.undoable(cx, project, message, move |store| {
            store.delete(record, Author::Human)?;
            Ok((Undo::Restore(record), ()))
        });
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
                ws.write_undoable(cx, project, None, move |store| {
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
                        Path::new(p).is_dir()
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
        let task = self.write_undoable(cx, project, None, move |store| {
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
    pub fn undo(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
        if self.undoing {
            return;
        }
        let Some(step) = self.undo.get(&project).and_then(|s| s.last()).cloned() else {
            return;
        };
        self.undoing = true;
        let applied = step.clone();
        let task = self.write(cx, Key::ErrorSave, move |store| applied.apply(store));
        cx.spawn(async move |ws, cx| {
            let result = task.await;
            let _ = ws.update(cx, |ws, cx| {
                ws.undoing = false;
                // A failed write keeps the step for another try.
                if let Ok(done) = result {
                    if let Some(stack) = ws.undo.get_mut(&project) {
                        stack.remove(&step);
                    }
                    if !done {
                        ws.toast_error(Key::UndoChanged, cx);
                    }
                }
            });
        })
        .detach();
    }

    /// The stack `Mod+Z` takes on a page of `active`: its project's, or,
    /// when that is empty, the steps that belong to no project (a new
    /// project).
    fn undo_key(&self, active: Option<ProjectId>) -> Option<ProjectId> {
        let has = |key| self.undo.get(&key).is_some_and(|s| s.last().is_some());
        if has(active) { active } else { None }
    }

    /// `Mod+Z` on a page of `active`.
    pub fn undo_active(&mut self, active: Option<ProjectId>, cx: &mut Context<Self>) {
        let key = self.undo_key(active);
        self.undo(key, cx);
    }

    /// The order of the step `Mod+Z` would take on a page of `active`.
    pub fn newest_undo(&self, active: Option<ProjectId>) -> Option<u64> {
        self.undo.get(&self.undo_key(active))?.newest()
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
pub fn missing_paths(paths: impl IntoIterator<Item = String>) -> std::collections::HashSet<String> {
    paths
        .into_iter()
        .filter(|p| !Path::new(p).is_dir())
        .collect()
}

/// The workspace of the app, for code that only has an `App`.
pub fn project_of(ws: &gpui_kit::Entity<Workspace>, owner: Owner, cx: &App) -> Option<ProjectId> {
    ws.read(cx).project_of(owner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::temp_store;
    use acta_core::store::NewTopic;

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
        assert!(undo.apply(store).unwrap());
        let back = store.entry(e.id).unwrap();
        assert_eq!(
            (back.text.as_str(), back.kind),
            ("Alexandre wrote this", EntryKind::Progress)
        );
        // Applied twice: the store no longer holds what the edit left.
        assert!(!undo.apply(store).unwrap());

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
        assert!(undo.apply(store).unwrap());
        assert_eq!(store.note(note).unwrap().text, "Alexandre's note");
        assert!(!undo.apply(store).unwrap());
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
        assert!(step.apply(store).unwrap());
        assert_eq!(store.topic(id).unwrap().status, Status::Planned);

        store.delete(Record::Topic(id), Author::Human).unwrap();
        assert!(Undo::Restore(Record::Topic(id)).apply(store).unwrap());
        assert!(store.topic(id).is_ok());
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
        assert!(step.apply(store).unwrap());
        let after = store.note(note).unwrap();
        assert_eq!(
            (after.fate(), after.filed_at, after.filed_by),
            (before.fate(), before.filed_at, Some(Author::Claude))
        );
        // Filed again by someone else in between: the undo is skipped.
        store.unfile_note(note, Author::Human).unwrap();
        store.dismiss_notes(&[note], Author::Claude).unwrap();
        assert!(!step.apply(store).unwrap());
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
        assert!(!step.apply(store).unwrap(), "skipped");
        assert_eq!(store.topic(id).unwrap().status, Status::Blocked);
        // A restore whose record is live again is skipped too.
        assert!(!Undo::Restore(Record::Topic(id)).apply(store).unwrap());
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
        assert!(!both.apply(store).unwrap());
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
        assert!(step.apply(store).unwrap());
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
        assert!(step.apply(store).unwrap());
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
        assert!(step.apply(store).unwrap());
        assert_eq!(store.topic(id).unwrap().epic, None);
        // Moved again by someone else: skipped.
        store
            .update_topic(id, patch(Some(epic)), Author::Claude)
            .unwrap();
        store.update_topic(id, patch(None), Author::Claude).unwrap();
        assert!(!step.apply(store).unwrap());
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
}
