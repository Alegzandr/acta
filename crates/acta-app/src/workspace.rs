//! The open database as the window sees it: the tree of every project,
//! reloaded whenever anything commits, by Acta or by another process
//! (docs/design/interaction.md 6.5), and the writes the pages make, with
//! their undo (6.4).
//!
//! Every read and write runs on the background executor (`Db::run`); the UI
//! thread only swaps in the result. Pages observe the workspace and reload
//! their own data when its revision moves.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use acta_core::model::{
    Author, EpicId, FolderId, KbId, Owner, ProjectId, ProjectTree, Record, Status, TopicId,
};
use acta_core::store::{EpicPatch, ProjectPatch, TopicPatch};
use acta_core::{ChangeWatcher, Store};
use gpui_kit::{Context, EventEmitter, Task};

use crate::db::Db;
use crate::i18n::Key;

/// docs/design/interaction.md 6.5: changes land within 100 ms of the write.
const POLL: Duration = Duration::from_millis(100);
/// How often the daily snapshot is looked at while the app runs.
const SNAPSHOT_CHECK: Duration = Duration::from_secs(60 * 60);
/// docs/design/interaction.md 6.4: the last 100 actions.
const UNDO_DEPTH: usize = 100;

/// Something the human did that `Mod+Z` takes back, stored as the action
/// that reverses it. Claude Code's writes are never here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Undo {
    /// A delete: restore from the trash.
    Restore(Record),
    /// A restore from the trash: delete again.
    Delete(Record),
    Status(TopicId, Status),
    TopicTitle(TopicId, String),
    EpicTitle(EpicId, String),
    ProjectTitle(ProjectId, String),
    /// A folder that was set somewhere: put the old path back.
    FolderPath(FolderId, String),
    /// A folder that was added: remove it.
    RemoveFolder(FolderId),
    IncludeInherited(Owner, bool),
    KbRevision(KbId, i64),
    /// Several reversals taken back as one step (a bulk repair).
    Many(Vec<Undo>),
}

impl Undo {
    /// Apply the reversal. Restoring a deleted record and deleting a
    /// restored one are each other's undo, so redo is not needed here.
    pub fn apply(&self, store: &mut Store) -> acta_core::Result<()> {
        let me = Author::Human;
        match self {
            Undo::Restore(record) => store.restore(*record, me),
            Undo::Delete(record) => store.delete(*record, me),
            Undo::Status(id, status) => store.update_topic(
                *id,
                TopicPatch {
                    status: Some(*status),
                    ..TopicPatch::default()
                },
                me,
            ),
            Undo::TopicTitle(id, title) => store.update_topic(
                *id,
                TopicPatch {
                    title: Some(title.clone()),
                    ..TopicPatch::default()
                },
                me,
            ),
            Undo::EpicTitle(id, title) => store.update_epic(
                *id,
                EpicPatch {
                    title: Some(title.clone()),
                    ..EpicPatch::default()
                },
                me,
            ),
            Undo::ProjectTitle(id, title) => store.update_project(
                *id,
                ProjectPatch {
                    title: Some(title.clone()),
                    ..ProjectPatch::default()
                },
                me,
            ),
            // The old path may no longer exist (that is why it was
            // located); it is put back as stored.
            Undo::FolderPath(id, path) => store.restore_folder_path(*id, path, me),
            Undo::RemoveFolder(id) => store.delete(Record::Folder(*id), me),
            Undo::IncludeInherited(owner, include) => {
                store.set_include_inherited(*owner, *include, me)
            }
            Undo::KbRevision(id, rev) => store.kb_restore_revision(*id, *rev, me).map(|_| ()),
            Undo::Many(all) => all.iter().try_for_each(|undo| undo.apply(store)),
        }
    }
}

/// The bounded undo stack.
#[derive(Clone, Debug, Default)]
pub struct UndoStack(Vec<Undo>);

impl UndoStack {
    pub fn push(&mut self, undo: Undo) {
        self.0.push(undo);
        if self.0.len() > UNDO_DEPTH {
            self.0.remove(0);
        }
    }

    pub fn pop(&mut self) -> Option<Undo> {
        self.0.pop()
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
}

pub enum WorkspaceEvent {
    Toast(Toast),
    /// The record shown may be gone; pages check.
    Reloaded,
}

/// The offer after a Locate (docs/design/components-folders.md 5,
/// Repairing the rest): the same root change for other folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkOffer {
    pub old_root: String,
    pub new_root: String,
    pub count: usize,
}

pub struct Workspace {
    pub db: Db,
    pub bulk: Option<BulkOffer>,
    pub projects: Vec<ProjectTree>,
    /// Bumped on every reload, by any process's commit.
    pub revision: u64,
    undo: UndoStack,
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
            undo: UndoStack::default(),
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

    /// Run a write in the background. On success the tree reloads and the
    /// result is returned; on failure a toast says so and `None` comes back
    /// (the caller puts the user's text back where it was).
    pub fn write<R: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        failure: Key,
        f: impl FnOnce(&mut Store) -> acta_core::Result<R> + Send + 'static,
    ) -> Task<Option<R>> {
        let task = self.db.run(cx, f);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |ws, cx| {
                ws.reload(cx);
                match result {
                    Ok(value) => Some(value),
                    Err(err) => {
                        crate::log::line(&format!("store: write failed: {}", err_kind(&err)));
                        ws.toast_error(failure, cx);
                        None
                    }
                }
            })
            .ok()
            .flatten()
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

    /// A write the human can take back with `undo`, announced by a toast
    /// with Undo when `message` is given (deletes, moves, folder removals).
    pub fn write_undoable(
        &mut self,
        cx: &mut Context<Self>,
        undo: Undo,
        message: Option<String>,
        f: impl FnOnce(&mut Store) -> acta_core::Result<()> + Send + 'static,
    ) {
        let task = self.write(cx, Key::ErrorSave, f);
        cx.spawn(async move |this, cx| {
            if task.await.is_some() {
                let _ = this.update(cx, |ws, cx| {
                    ws.undo.push(undo);
                    if let Some(message) = message {
                        cx.emit(WorkspaceEvent::Toast(Toast {
                            message,
                            kind: ToastKind::Undo,
                        }));
                    }
                });
            }
        })
        .detach();
    }

    pub fn push_undo(&mut self, undo: Undo) {
        self.undo.push(undo);
    }

    /// After a Locate from `old` to `new`, look for other folders the same
    /// change would resolve; offer it when there are any.
    pub fn offer_bulk_repair(&mut self, old: String, new: String, cx: &mut Context<Self>) {
        let Some((old_root, new_root)) = crate::folders_model::roots(&old, &new) else {
            return;
        };
        let (old_r, new_r) = (old_root.clone(), new_root.clone());
        let task = self.db.run(cx, move |store| {
            let paths: Vec<String> = store.all_folders()?.into_iter().map(|f| f.path).collect();
            Ok(
                crate::folders_model::candidates(&paths, &old_r, &new_r, |p| {
                    std::path::Path::new(p).is_dir()
                })
                .len(),
            )
        });
        cx.spawn(async move |ws, cx| {
            if let Ok(count) = task.await
                && count > 0
            {
                let _ = ws.update(cx, |ws, cx| {
                    ws.bulk = Some(BulkOffer {
                        old_root,
                        new_root,
                        count,
                    });
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// "Apply to 7 folders": one undoable step.
    pub fn apply_bulk(&mut self, cx: &mut Context<Self>) {
        let Some(offer) = self.bulk.take() else {
            return;
        };
        cx.notify();
        let locale = crate::settings::locale(cx);
        let task = self.write(cx, Key::ErrorSave, move |store| {
            let before: Vec<(FolderId, String)> = store
                .all_folders()?
                .into_iter()
                .map(|f| (f.id, f.path))
                .collect();
            let moved = store.relocate_folders(
                &offer.old_root,
                std::path::Path::new(&offer.new_root),
                Author::Human,
            )?;
            Ok(before
                .into_iter()
                .filter(|(id, _)| moved.contains(id))
                .collect::<Vec<_>>())
        });
        cx.spawn(async move |ws, cx| {
            if let Some(moved) = task.await {
                let _ = ws.update(cx, |ws, cx| {
                    let n = moved.len() as u64;
                    ws.undo.push(Undo::Many(
                        moved
                            .into_iter()
                            .map(|(id, path)| Undo::FolderPath(id, path))
                            .collect(),
                    ));
                    let message = crate::i18n::tr(
                        locale,
                        Key::FoldersBulkDone,
                        &[("n", crate::i18n::Arg::Count(n))],
                    );
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

    /// `Mod+Z`: take back the newest action.
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(undo) = self.undo.pop() else {
            return;
        };
        self.write_detached(cx, Key::ErrorSave, move |store| undo.apply(store));
    }

    pub fn toast(&mut self, message: String, kind: ToastKind, cx: &mut Context<Self>) {
        cx.emit(WorkspaceEvent::Toast(Toast { message, kind }));
    }

    pub fn toast_error(&mut self, key: Key, cx: &mut Context<Self>) {
        let locale = crate::settings::locale(cx);
        let message = crate::i18n::text(locale, key);
        self.toast(message, ToastKind::Error, cx);
    }
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

    #[test]
    fn undo_puts_back_a_status_a_title_and_a_deleted_topic() {
        let mut temp = temp_store();
        let store = &mut temp.store;
        let (_, id) = topic(store);

        store
            .update_topic(
                id,
                TopicPatch {
                    status: Some(Status::Active),
                    title: Some("Renamed".into()),
                    ..TopicPatch::default()
                },
                Author::Human,
            )
            .unwrap();
        Undo::Status(id, Status::Planned).apply(store).unwrap();
        Undo::TopicTitle(id, "Embed the terminal".into())
            .apply(store)
            .unwrap();
        let back = store.topic(id).unwrap();
        assert_eq!(back.status, Status::Planned);
        assert_eq!(back.title, "Embed the terminal");

        store.delete(Record::Topic(id), Author::Human).unwrap();
        assert!(store.topic(id).is_err());
        Undo::Restore(Record::Topic(id)).apply(store).unwrap();
        assert!(store.topic(id).is_ok());
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
        Undo::FolderPath(id, stored.clone()).apply(store).unwrap();
        assert_eq!(store.folder(id).unwrap().path, stored);
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
        assert_eq!(trees[0].loose_topics.len(), 1);
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
    fn error_kinds_carry_no_text() {
        let err = acta_core::Error::Invalid("title of Alexandre's secret".into());
        assert_eq!(err_kind(&err), "Invalid");
    }
}
