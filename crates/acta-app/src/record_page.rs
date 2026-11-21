//! The project, epic and topic pages (docs/design/layout.md 4.2; screens.md
//! 8.1, 8.6, 8.14): title, the properties block, then the sections in their
//! fixed order. A topic: current state, description, knowledge, timeline.
//! An epic: description, knowledge, topic table. A project: description,
//! knowledge, the topic table grouped by epic.
//!
//! Writes are sent to the workspace and the page reloads when its revision
//! moves, whoever wrote; an editor being used is never overwritten by a
//! reload (interaction.md 6.5 rule 4: the human's text stays).

use std::path::PathBuf;

use acta_core::model::{
    Author, Entry, EntryId, EntryKind, KbId, KbKind, KbSummary, Owner, Record, Status, TopicId,
    TopicSummary,
};
use acta_core::store::{EpicPatch, NewEntry, ProjectPatch, TopicPatch};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext, ClipboardItem, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, PathPromptOptions,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window, anchored,
    deferred, div, point, px,
};

use crate::i18n::{Arg, Key, KeyName, Locale, number, text, tr};
use crate::icons::{Icon, icon};
use crate::kb_model;
use crate::markdown_view;
use crate::menu;
use crate::paths_view;
use crate::record::{self, EntryFilterKind, Loaded};
use crate::route::Route;
use crate::settings;
use crate::table::{self, Column};
use crate::theme::{RADIUS_MD, RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{FolderRef, Undo, Workspace};

/// What a page asks the window to do.
#[derive(Clone, Debug, PartialEq)]
pub enum PageEvent {
    Open(Route),
    /// The page now stands for another route, without a history step (a
    /// draft that was just saved).
    Replace(Route),
    /// Start a document or memory owned by this page's record.
    NewKb(crate::kb_page::Draft),
    /// The page learned which project it belongs to (a document opened by
    /// id): the active project follows it.
    Located(acta_core::model::ProjectId),
}

gpui_kit::actions!(
    acta_page,
    [
        /// `S`: the status menu of the page's topic.
        OpenStatusMenu,
        /// `E` / `F2`: rename the page's record.
        EditTitle,
    ]
);

pub struct RecordPage {
    ws: Entity<Workspace>,
    pub owner: Owner,
    data: Option<Loaded>,
    /// The record went to the trash while open (interaction.md 6.5 rule 6).
    gone: bool,
    filter: EntryFilterKind,
    focus: FocusHandle,
    title_edit: Option<Entity<InputState>>,
    saving_title: bool,
    state_edit: Option<Entity<TextareaState>>,
    description_edit: Option<Entity<TextareaState>>,
    composer: Entity<TextareaState>,
    composer_kind: EntryKind,
    composer_open: bool,
    /// The status menu, with its highlighted row.
    status_menu: Option<usize>,
    /// The entry menu (components-records.md 5, Timeline entry).
    entry_menu: Option<EntryMenu>,
    /// An entry being edited in place, with its editor.
    entry_edit: Option<(EntryId, Entity<TextareaState>)>,
    /// The knowledge section's New menu, with its highlighted row.
    kb_new_menu: Option<usize>,
    folders_open: bool,
    table_filter: table::Filter,
    table_sort: Option<table::Sort>,
    kb_open: Option<KbId>,
    kb_inherited_open: bool,
    id_copied: bool,
    seen_revision: u64,
    _load: Task<()>,
    _subs: Vec<Subscription>,
}

/// The entry menu: Edit, Copy as Markdown, Change type, Delete; "Change
/// type" turns it into the three types.
#[derive(Clone, Debug)]
struct EntryMenu {
    entry: Entry,
    kinds: bool,
    highlight: usize,
}

/// The types an entry can be changed to, in menu order.
const ENTRY_KINDS: [EntryKind; 3] = [
    EntryKind::Progress,
    EntryKind::Decision,
    EntryKind::Discussion,
];

impl EventEmitter<PageEvent> for RecordPage {}

impl Focusable for RecordPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// The record a page is about, as a trash record.
fn record_of(owner: Owner) -> Record {
    match owner {
        Owner::Project(id) => Record::Project(id),
        Owner::Epic(id) => Record::Epic(id),
        Owner::Topic(id) => Record::Topic(id),
    }
}

impl RecordPage {
    #[cfg(windows)]
    /// The status menu is open (the chat's web view hides under it).
    pub fn menu_open(&self) -> bool {
        self.status_menu.is_some() || self.entry_menu.is_some() || self.kb_new_menu.is_some()
    }

    pub fn new(
        ws: Entity<Workspace>,
        owner: Owner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let locale = settings::locale(cx);
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 16)
                .placeholder(text(locale, Key::ComposerPlaceholder))
        });
        let mut subs = vec![cx.observe(&ws, |page, _, cx| page.reload_if_moved(cx))];
        subs.push(cx.subscribe_in(&composer, window, Self::on_composer));
        let mut page = Self {
            ws,
            owner,
            data: None,
            gone: false,
            filter: EntryFilterKind::All,
            focus: cx.focus_handle(),
            title_edit: None,
            saving_title: false,
            state_edit: None,
            description_edit: None,
            composer,
            composer_kind: EntryKind::Progress,
            composer_open: false,
            status_menu: None,
            entry_menu: None,
            entry_edit: None,
            kb_new_menu: None,
            folders_open: false,
            table_filter: table::Filter::Open,
            table_sort: None,
            kb_open: None,
            kb_inherited_open: false,
            id_copied: false,
            seen_revision: 0,
            _load: Task::ready(()),
            _subs: subs,
        };
        page.reload(cx);
        page
    }

    pub fn topic(&self) -> Option<TopicId> {
        match self.owner {
            Owner::Topic(id) => Some(id),
            _ => None,
        }
    }

    fn reload_if_moved(&mut self, cx: &mut Context<Self>) {
        let revision = self.ws.read(cx).revision;
        if revision != self.seen_revision {
            self.seen_revision = revision;
            self.reload(cx);
        }
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner;
        let filter = self.filter;
        let task = self
            .ws
            .read(cx)
            .db
            .run(cx, move |store| record::load(store, owner, filter));
        self._load = cx.spawn(async move |page, cx| {
            let loaded = task.await;
            let paths = loaded.as_ref().ok().map(|d| d.folders.paths());
            let _ = page.update(cx, |page, cx| {
                match loaded {
                    Ok(mut data) => {
                        // Until the check answers, folders render as before
                        // (found when new), so nothing flickers.
                        if let Some(old) = &page.data {
                            data.folders.missing = old.folders.missing.clone();
                        }
                        page.data = Some(data);
                        page.gone = false;
                    }
                    // Deleted (here or by another process): stay readable
                    // with the notice, keep the last data.
                    Err(acta_core::Error::NotFound(_)) => page.gone = true,
                    Err(err) => crate::log::line(&format!(
                        "page: load failed: {}",
                        crate::workspace::err_kind(&err)
                    )),
                }
                cx.notify();
            });
            // Existence on this computer, checked off the store.
            if let Some(paths) = paths {
                let missing = cx
                    .background_executor()
                    .spawn(async move { crate::workspace::missing_paths(paths) })
                    .await;
                let _ = page.update(cx, |page, cx| {
                    if let Some(data) = page.data.as_mut() {
                        data.folders.missing = missing;
                    }
                    cx.notify();
                });
            }
        });
    }

    // ---- writes ----

    fn project_id(&self) -> Option<acta_core::model::ProjectId> {
        self.data.as_ref().map(|d| d.project.0)
    }

    fn set_status(&mut self, status: Status, cx: &mut Context<Self>) {
        let Some(id) = self.topic() else { return };
        let Some(old) = self
            .data
            .as_ref()
            .and_then(|d| d.topic.as_ref())
            .map(|t| t.status)
        else {
            return;
        };
        self.status_menu = None;
        if old == status {
            cx.notify();
            return;
        }
        // Optimistic: the page shows it in this frame; the reload after the
        // write shows what was stored.
        if let Some(topic) = self.data.as_mut().and_then(|d| d.topic.as_mut()) {
            topic.status = status;
        }
        cx.notify();
        self.ws.update(cx, |ws, cx| ws.set_status(id, status, cx));
    }

    pub fn edit_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let title = data.title.clone();
        let state = cx.new(|cx| InputState::new(window, cx).default_value(title));
        state.update(cx, |state, cx| state.focus(window, cx));
        let sub =
            cx.subscribe_in(
                &state,
                window,
                |page, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => page.commit_title(window, cx),
                    InputEvent::Blur => page.commit_title(window, cx),
                    _ => {}
                },
            );
        self._subs.push(sub);
        self.title_edit = Some(state);
        cx.notify();
    }

    /// Save the title. The editor stays, with the typed text, until the
    /// write succeeds; a failure leaves it open (interaction.md 6.4).
    fn commit_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving_title {
            return;
        }
        let Some(state) = self.title_edit.clone() else {
            return;
        };
        let new = state.read(cx).value().trim().to_owned();
        let Some(data) = self.data.as_ref() else {
            return;
        };
        if new.is_empty() || new == data.title {
            self.title_edit = None;
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        self.saving_title = true;
        let owner = self.owner;
        let project = self.project_id();
        let task = self.ws.update(cx, |ws, cx| {
            ws.write_undoable(cx, project, None, move |store| {
                let me = Author::Human;
                let undo = match owner {
                    Owner::Topic(id) => {
                        let back = store.topic(id)?.title;
                        store.update_topic(
                            id,
                            TopicPatch {
                                title: Some(new.clone()),
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
                                title: Some(new.clone()),
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
                                title: Some(new.clone()),
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
        });
        cx.spawn_in(window, async move |page, cx| {
            let saved = task.await.is_ok();
            let _ = page.update_in(cx, |page, window, cx| {
                page.saving_title = false;
                if saved {
                    page.title_edit = None;
                    window.focus(&page.focus, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn cancel_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.title_edit = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn open_editor(&mut self, description: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let locale = settings::locale(cx);
        let (value, placeholder) = if description {
            (data.description.clone(), Key::DescriptionPlaceholder)
        } else {
            (
                data.topic
                    .as_ref()
                    .map(|t| t.state.clone())
                    .unwrap_or_default(),
                Key::StatePlaceholder,
            )
        };
        let state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 16)
                .placeholder(text(locale, placeholder))
                .default_value(value)
        });
        state.update(cx, |state, cx| state.focus(window, cx));
        let sub = cx.subscribe_in(
            &state,
            window,
            move |page, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    page.commit_editor(description, window, cx);
                }
            },
        );
        self._subs.push(sub);
        if description {
            self.description_edit = Some(state);
        } else {
            self.state_edit = Some(state);
        }
        cx.notify();
    }

    /// Save the description or current state. The editor stays open with
    /// the typed text until the write succeeds; a failure keeps it.
    fn commit_editor(&mut self, description: bool, window: &mut Window, cx: &mut Context<Self>) {
        let editor = if description {
            self.description_edit.clone()
        } else {
            self.state_edit.clone()
        };
        let Some(editor) = editor else { return };
        let value = editor.read(cx).value().trim_end().to_owned();
        let owner = self.owner;
        let task = self.ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| {
                let me = Author::Human;
                match (owner, description) {
                    (Owner::Topic(id), true) => store.update_topic(
                        id,
                        TopicPatch {
                            description: Some(value),
                            ..TopicPatch::default()
                        },
                        me,
                    ),
                    (Owner::Topic(id), false) => store.update_topic(
                        id,
                        TopicPatch {
                            state: Some(value),
                            ..TopicPatch::default()
                        },
                        me,
                    ),
                    (Owner::Epic(id), _) => store.update_epic(
                        id,
                        EpicPatch {
                            description: Some(value),
                            ..EpicPatch::default()
                        },
                        me,
                    ),
                    (Owner::Project(id), _) => store.update_project(
                        id,
                        ProjectPatch {
                            description: Some(value),
                            ..ProjectPatch::default()
                        },
                        me,
                    ),
                }
            })
        });
        cx.spawn_in(window, async move |page, cx| {
            if task.await.is_ok() {
                let _ = page.update_in(cx, |page, window, cx| {
                    let open = if description {
                        &mut page.description_edit
                    } else {
                        &mut page.state_edit
                    };
                    // Only the editor that was saved closes.
                    if open.as_ref() == Some(&editor) {
                        *open = None;
                        window.focus(&page.focus, cx);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// `Esc` in an editor: close it; its text is dropped from the editor
    /// but the stored value was never changed (drafts are request 15).
    fn cancel_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let had = self.state_edit.take().is_some()
            | self.description_edit.take().is_some()
            | self.status_menu.take().is_some()
            | self.entry_menu.take().is_some()
            | self.entry_edit.take().is_some()
            | self.kb_new_menu.take().is_some();
        if self.composer_open {
            self.composer_open = false;
        }
        if had {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// `Mod+E`: the composer at the top of the timeline, preset to Progress.
    pub fn focus_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.topic().is_none() {
            return;
        }
        self.composer_open = true;
        self.composer_kind = EntryKind::Progress;
        self.composer
            .update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    fn on_composer(
        &mut self,
        _: &Entity<TextareaState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Focus => {
                self.composer_open = true;
                cx.notify();
            }
            InputEvent::PressEnter {
                secondary: true, ..
            } => self.add_entry(window, cx),
            _ => {}
        }
    }

    fn add_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(topic) = self.topic() else { return };
        let body = self.composer.read(cx).value().trim().to_owned();
        if body.is_empty() {
            return;
        }
        let kind = self.composer_kind;
        self.composer
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.composer_open = false;
        window.focus(&self.focus, cx);
        cx.notify();
        let restore = body.clone();
        let task = self.ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSaveEntry, move |store| {
                store.log(
                    topic,
                    NewEntry {
                        kind,
                        text: body,
                        state: None,
                        status: None,
                    },
                    Author::Human,
                )
            })
        });
        // A failed write puts the text back (interaction.md 6.4).
        cx.spawn_in(window, async move |page, cx| {
            if task.await.is_err() {
                let _ = page.update_in(cx, |page, window, cx| {
                    page.composer
                        .update(cx, |state, cx| state.set_value(restore, window, cx));
                    page.composer_open = true;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    // ---- the entry menu ----

    fn open_entry_menu(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.entry_menu = Some(EntryMenu {
            entry,
            kinds: false,
            highlight: 0,
        });
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Choose row `index` of the open entry menu.
    fn choose_entry_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.entry_menu.take() else {
            return;
        };
        let entry = menu.entry;
        if menu.kinds {
            if let Some(kind) = ENTRY_KINDS.get(index) {
                self.write_entry(&entry, entry.text.clone(), *kind, cx);
            }
            cx.notify();
            return;
        }
        match index {
            0 => self.start_entry_edit(entry, window, cx),
            1 => cx.write_to_clipboard(ClipboardItem::new_string(entry.text.clone())),
            2 => {
                let at = ENTRY_KINDS
                    .iter()
                    .position(|k| *k == entry.kind)
                    .unwrap_or(0);
                self.entry_menu = Some(EntryMenu {
                    entry,
                    kinds: true,
                    highlight: at,
                });
            }
            _ => self.delete_entry(&entry, cx),
        }
        cx.notify();
    }

    /// Edit: the body becomes the multi-line editor in place.
    fn start_entry_edit(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        let id = entry.id;
        let state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 16)
                .default_value(entry.text.clone())
        });
        state.update(cx, |state, cx| state.focus(window, cx));
        let sub = cx.subscribe_in(
            &state,
            window,
            move |page, editor, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    let text = editor.read(cx).value().trim_end().to_owned();
                    if !text.trim().is_empty() {
                        page.write_entry(&entry, text, entry.kind, cx);
                        page.entry_edit = None;
                        window.focus(&page.focus, cx);
                        cx.notify();
                    }
                }
            },
        );
        self._subs.push(sub);
        self.entry_edit = Some((id, state));
        cx.notify();
    }

    /// Save an entry's text and type, undoably.
    fn write_entry(
        &mut self,
        entry: &Entry,
        text: String,
        kind: EntryKind,
        cx: &mut Context<Self>,
    ) {
        if text == entry.text && kind == entry.kind {
            return;
        }
        let id = entry.id;
        let back = (entry.text.clone(), entry.kind);
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                store.update_entry(id, &text, kind, Author::Human)?;
                let now = store.entry(id)?;
                Ok((
                    Undo::Entry {
                        id,
                        back,
                        expect: (now.text, now.kind),
                    },
                    (),
                ))
            });
        });
    }

    fn delete_entry(&mut self, entry: &Entry, cx: &mut Context<Self>) {
        let record = Record::Entry(entry.id);
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| ws.delete(record, project, cx));
    }

    fn restore_record(&mut self, cx: &mut Context<Self>) {
        let record = record_of(self.owner);
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                store.restore(record, Author::Human)?;
                Ok((Undo::Delete(record), ()))
            });
        });
    }

    /// New document or memory owned by this page's record: the editor
    /// opens empty and the entry exists once saved.
    pub fn new_kb(&mut self, kind: KbKind, cx: &mut Context<Self>) {
        cx.emit(PageEvent::NewKb(crate::kb_page::Draft {
            owner: self.owner,
            kind,
        }));
    }

    /// "Show working folders": the folder row unfolded.
    pub fn show_folders(&mut self, cx: &mut Context<Self>) {
        self.folders_open = true;
        cx.notify();
    }

    // ---- folders ----

    /// The native folder picker, then `then` with the chosen folder.
    fn pick_folder(
        &mut self,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, PathBuf, &mut Context<Self>) + 'static,
    ) {
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn(async move |page, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.pop() else { return };
            let _ = page.update(cx, |page, cx| then(page, path, cx));
        })
        .detach();
    }

    /// Attach `path` to `owner` (as its primary when `primary`), undoably.
    /// The path is resolved off the store first.
    fn attach(&mut self, owner: Owner, path: PathBuf, primary: bool, cx: &mut Context<Self>) {
        self.folders_open = true;
        let project = self.project_id();
        let ws = self.ws.clone();
        let resolved = cx
            .background_executor()
            .spawn(async move { acta_core::folder_path::canonical(&path).map(|_| path) });
        cx.spawn(async move |_, cx| {
            let Ok(path) = resolved.await else {
                ws.update(cx, |ws, cx| ws.toast_error(Key::ErrorSave, cx));
                return;
            };
            ws.update(cx, |ws, cx| {
                ws.undoable(cx, project, None, move |store| {
                    let id = store.add_folder(owner, &path, Author::Human)?;
                    if primary {
                        store.reorder(Record::Folder(id), 0, Author::Human)?;
                    }
                    Ok((Undo::Delete(Record::Folder(id)), ()))
                });
            });
        })
        .detach();
    }

    /// "Add folder...": append to this page's own list.
    pub fn add_folder(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner;
        self.pick_folder(cx, move |page, path, cx| {
            page.attach(owner, path, false, cx)
        });
    }

    /// "Set working folder...": replace the page's own primary, or attach to
    /// the project when no level has a folder (components-folders.md 5).
    pub fn set_folder(&mut self, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let nothing_anywhere = data.folders.effective.is_empty();
        let own_primary = data.folders.own.first().map(|f| f.id);
        let target = if nothing_anywhere {
            Owner::Project(data.project.0)
        } else {
            self.owner
        };
        self.pick_folder(cx, move |page, path, cx| match own_primary {
            // Replacing the own primary keeps its place: the same repair
            // as Locate.
            Some(id) if !nothing_anywhere => {
                page.folders_open = true;
                let project = page.project_id();
                page.ws.update(cx, |ws, cx| {
                    ws.locate(FolderRef::Id(id), path, project, cx).detach()
                });
            }
            _ => page.attach(target, path, true, cx),
        });
    }

    /// "Locate...": repair the folder at `path` owned by `from`, in place,
    /// through the workspace's shared Locate (undo and bulk offer).
    fn locate(&mut self, from: Owner, path: String, cx: &mut Context<Self>) {
        let project = self.project_id();
        self.pick_folder(cx, move |page, chosen, cx| {
            page.ws.update(cx, |ws, cx| {
                ws.locate(FolderRef::At(from, path), chosen, project, cx)
                    .detach()
            });
        });
    }

    fn remove_folder(
        &mut self,
        id: acta_core::model::FolderId,
        path: &str,
        cx: &mut Context<Self>,
    ) {
        let locale = settings::locale(cx);
        let folder = paths_view::last_segment(path).to_owned();
        let key = match self.owner {
            Owner::Topic(_) => Key::FoldersRemovedTopic,
            Owner::Epic(_) => Key::FoldersRemovedEpic,
            Owner::Project(_) => Key::FoldersRemovedProject,
        };
        let message = tr(locale, key, &[("folder", Arg::Text(&folder))]);
        let record = Record::Folder(id);
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, Some(message), move |store| {
                store.delete(record, Author::Human)?;
                Ok((Undo::Restore(record), ()))
            });
        });
    }

    fn make_primary(&mut self, id: acta_core::model::FolderId, cx: &mut Context<Self>) {
        let project = self.project_id();
        self.ws
            .update(cx, |ws, cx| ws.reorder(Record::Folder(id), 0, project, cx));
    }

    fn toggle_include(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner;
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let back = match owner {
                    Owner::Project(_) => false,
                    Owner::Epic(id) => store.epic(id)?.include_inherited_folders,
                    Owner::Topic(id) => store.topic(id)?.include_inherited_folders,
                };
                store.set_include_inherited(owner, !back, Author::Human)?;
                Ok((Undo::IncludeInherited { owner, back }, ()))
            });
        });
    }

    // ---- keys ----

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.entry_menu.as_mut() {
            if event.keystroke.modifiers.modified() {
                return;
            }
            let rows = if menu.kinds { ENTRY_KINDS.len() } else { 4 };
            match event.keystroke.key.as_str() {
                "up" => menu.highlight = menu.highlight.saturating_sub(1),
                "down" => menu.highlight = (menu.highlight + 1).min(rows - 1),
                "enter" | "space" => {
                    let at = menu.highlight;
                    self.choose_entry_item(at, window, cx);
                }
                "escape" => self.entry_menu = None,
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let Some(highlight) = self.status_menu else {
            return;
        };
        let key = event.keystroke.key.as_str();
        if event.keystroke.modifiers.modified() {
            return;
        }
        match key {
            "up" => self.status_menu = Some(highlight.saturating_sub(1)),
            "down" => self.status_menu = Some((highlight + 1).min(5)),
            "enter" | "space" => self.set_status(Status::ALL[highlight], cx),
            "escape" => self.status_menu = None,
            digit if digit.len() == 1 => {
                if let Some(n) = digit.chars().next().and_then(|c| c.to_digit(10))
                    && (1..=6).contains(&n)
                {
                    self.set_status(Status::ALL[n as usize - 1], cx);
                } else {
                    return;
                }
            }
            _ => return,
        }
        let _ = window;
        cx.stop_propagation();
        cx.notify();
    }
}

// ---- rendering ----

/// `{when} by {author}`, the long form: "2 h ago by Claude Code".
fn updated_line(locale: Locale, at: i64, by: Author) -> String {
    let when = when::long_alone(locale, at);
    let author = when::author(locale, by);
    tr(
        locale,
        Key::CommonBy,
        &[("when", Arg::Text(&when)), ("author", Arg::Text(&author))],
    )
}

fn entry_kind_meta(kind: EntryKind) -> (Icon, Key) {
    match kind {
        EntryKind::Progress | EntryKind::System => (Icon::GitCommitHorizontal, Key::EntryProgress),
        EntryKind::Decision => (Icon::Signpost, Key::EntryDecision),
        EntryKind::Discussion => (Icon::MessageSquare, Key::EntryDiscussion),
    }
}

impl RecordPage {
    fn render_title(&self, data: &Loaded, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        if let Some(state) = &self.title_edit {
            return div()
                .border_b_1()
                .border_color(t.border_strong)
                .text_size(px(24.))
                .line_height(px(30.))
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .on_action(
                    cx.listener(|page, _: &Escape, window, cx| page.cancel_title(window, cx)),
                )
                .child(Input::new(state).appearance(false))
                .into_any_element();
        }
        type_scale::TITLE
            .el()
            .id("page-title")
            .text_color(t.text_primary)
            .cursor_text()
            .on_click(cx.listener(|page, _, window, cx| page.edit_title(window, cx)))
            .child(data.title.clone())
            .into_any_element()
    }

    fn render_properties(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut rows: Vec<(String, AnyElement)> = Vec::new();
        let updated = div()
            .id("prop-updated")
            .child(updated_line(locale, data.updated_at, data.updated_by))
            .into_any_element();
        match self.owner {
            Owner::Topic(id) => {
                let topic = data.topic.as_ref();
                let status = topic.map(|t| t.status).unwrap_or(Status::Planned);
                let status_button = ui::ghost_value("prop-status", t)
                    .child(ui::status_glyph(status, 16., t))
                    .child(text(locale, ui::status_key(status)))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        page.status_menu =
                            Some(Status::ALL.iter().position(|s| *s == status).unwrap_or(0));
                        window.focus(&page.focus, cx);
                        cx.notify();
                    }));
                let menu = self
                    .status_menu
                    .map(|highlight| self.render_status_menu(status, highlight, t, cx));
                rows.push((
                    text(locale, Key::PropStatus),
                    div().child(status_button).children(menu).into_any_element(),
                ));
                let epic = match &data.epic {
                    Some((epic, name)) => {
                        let epic = *epic;
                        link(("prop-epic", 0usize), name.clone(), t)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(PageEvent::Open(Route::Epic(epic)))
                            }))
                            .into_any_element()
                    }
                    None => div()
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::CommonNone))
                        .into_any_element(),
                };
                rows.push((text(locale, Key::PropEpic), epic));
                rows.push((
                    text(locale, Key::PropFolders),
                    self.render_folders(data, locale, t, cx),
                ));
                rows.push((
                    text(locale, Key::PropId),
                    self.render_id(id.to_string(), locale, t, cx),
                ));
            }
            Owner::Epic(id) => {
                rows.push((text(locale, Key::PropProgress), progress(data, locale, t)));
                rows.push((
                    text(locale, Key::PropFolders),
                    self.render_folders(data, locale, t, cx),
                ));
                rows.push((
                    text(locale, Key::PropId),
                    self.render_id(id.to_string(), locale, t, cx),
                ));
            }
            Owner::Project(_) => {
                rows.push((text(locale, Key::PropProgress), progress(data, locale, t)));
                rows.push((
                    text(locale, Key::PropFolders),
                    self.render_folders(data, locale, t, cx),
                ));
            }
        }
        rows.push((text(locale, Key::PropUpdated), updated));
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        let width = ui::label_width(&labels, cx);
        div()
            .flex()
            .flex_col()
            .mt(px(12.))
            .children(
                rows.into_iter()
                    .map(|(label, value)| ui::property(label, width, value, t)),
            )
            .into_any_element()
    }

    fn render_id(
        &self,
        id: String,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shown = if self.id_copied {
            text(locale, Key::CommonCopied)
        } else {
            id.clone()
        };
        ui::ghost_value("prop-id", t)
            .child(
                type_scale::CODE
                    .el()
                    .text_color(t.text_secondary)
                    .child(shown),
            )
            .on_click(cx.listener(move |page, _, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(id.clone()));
                page.id_copied = true;
                cx.notify();
                cx.spawn(async move |page, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(1500))
                        .await;
                    let _ = page.update(cx, |page, cx| {
                        page.id_copied = false;
                        cx.notify();
                    });
                })
                .detach();
            }))
            .into_any_element()
    }

    fn render_status_menu(
        &self,
        current: Status,
        highlight: usize,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locale = settings::locale(cx);
        let items = Status::ALL.into_iter().enumerate().map(|(i, status)| {
            let trailing = if status == current {
                menu::check(t)
            } else {
                menu::hint((i + 1).to_string(), t)
            };
            menu::item(
                ("status-item", i),
                Some(ui::status_glyph(status, 16., t).into_any_element()),
                text(locale, ui::status_key(status)),
                Some(trailing),
                i == highlight,
                t,
            )
            .on_click(cx.listener(move |page, _, _, cx| page.set_status(status, cx)))
        });
        deferred(
            anchored().offset(point(px(-8.), px(30.))).child(
                menu::panel("status-menu", t)
                    .on_mouse_down_out(cx.listener(|page, _, _, cx| {
                        page.status_menu = None;
                        cx.notify();
                    }))
                    .children(items),
            ),
        )
        .into_any_element()
    }

    fn render_folders(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let folders = &data.folders;
        let home = paths_view::home();
        let shown = |path: &str| paths_view::shorten_home(path, home.as_deref());
        let Some(primary) = folders.primary() else {
            // Nothing attached anywhere up the chain.
            return div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .w_full()
                .child(icon(Icon::FolderOpen, 16., t.text_secondary))
                .child(
                    type_scale::UI
                        .el()
                        .flex_1()
                        .min_w_0()
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::FoldersNone)),
                )
                .child(
                    ui::button(
                        "folder-set",
                        text(locale, Key::FoldersSet),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| page.set_folder(cx))),
                )
                .into_any_element();
        };
        let primary_missing = folders.is_missing(&primary.path);
        let extra = folders.effective.len().saturating_sub(1);
        let missing_extra = folders.missing_additional();
        let origin = folders.inherited_from.as_ref().map(|(from, name)| {
            let key = match from {
                Owner::Project(_) => Key::FoldersFromProject,
                _ => Key::FoldersFromEpic,
            };
            let short: String = if name.chars().count() > 24 {
                name.chars().take(23).chain(['\u{2026}']).collect()
            } else {
                name.clone()
            };
            type_scale::SMALL
                .el()
                .flex_shrink_0()
                .text_color(t.text_tertiary)
                .child(tr(locale, key, &[("name", Arg::Text(&short))]))
        });
        let (glyph, glyph_color, path_color) = if primary_missing {
            (Icon::FolderX, t.warning, t.text_secondary)
        } else {
            (Icon::FolderOpen, t.text_secondary, t.text_primary)
        };
        let budget = paths_view::budget(220.);
        let header = ui::focusable(div().id("folder-row"), t, false)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(8.))
            .ml(px(-8.))
            .rounded(px(RADIUS_SM))
            .hover({
                let hover = t.fill_hover;
                move |s| s.bg(hover)
            })
            .on_click(cx.listener(|page, _, _, cx| {
                page.folders_open = !page.folders_open;
                cx.notify();
            }))
            .child(icon(
                if self.folders_open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                16.,
                t.text_secondary,
            ))
            .child(icon(glyph, 16., glyph_color))
            .child(
                type_scale::CODE
                    .el()
                    .min_w_0()
                    .truncate()
                    .text_color(path_color)
                    .child(paths_view::fit(&shown(&primary.path), budget)),
            )
            .when(extra > 0, |this| {
                this.child(
                    type_scale::SMALL
                        .el()
                        .ml(px(4.))
                        .text_color(t.text_tertiary)
                        .font_features(type_scale::tabular())
                        .child(format!("+{}", number(locale, extra as u64))),
                )
            })
            .when(missing_extra > 0 && !primary_missing, |this| {
                this.child(icon(Icon::FolderX, 14., t.warning)).child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_secondary)
                        .child(tr(
                            locale,
                            Key::FoldersNNotFound,
                            &[("n", Arg::Count(missing_extra as u64))],
                        )),
                )
            })
            .children(origin)
            .when(primary_missing, |this| {
                let from = primary.from;
                let path = primary.path.clone();
                this.child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_secondary)
                        .child(text(locale, Key::FoldersNotFound)),
                )
                .child(
                    ui::button(
                        "folder-locate",
                        text(locale, Key::FoldersLocate),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| {
                        cx.stop_propagation();
                        page.locate(from, path.clone(), cx)
                    })),
                )
            });
        if !self.folders_open {
            return header.into_any_element();
        }
        let mut list = div().flex().flex_col().pl(px(24.));
        let own_paths: Vec<_> = folders.own.iter().collect();
        for (i, folder) in own_paths.iter().enumerate() {
            let missing = folders.is_missing(&folder.path);
            let id = folder.id;
            let path = folder.path.clone();
            let path_for_remove = folder.path.clone();
            let owner = self.owner;
            list = list.child(
                div()
                    .id(("own-folder", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .child(icon(
                        if missing {
                            Icon::FolderX
                        } else {
                            Icon::FolderOpen
                        },
                        16.,
                        if missing { t.warning } else { t.text_secondary },
                    ))
                    .child(
                        type_scale::CODE
                            .el()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(if i == 0 && !missing {
                                t.text_primary
                            } else {
                                t.text_secondary
                            })
                            .child(paths_view::fit(&shown(&folder.path), budget)),
                    )
                    .when(missing, |this| {
                        this.child(
                            type_scale::SMALL
                                .el()
                                .text_color(t.text_secondary)
                                .child(text(locale, Key::FoldersNotFound)),
                        )
                        .child(
                            ui::button(
                                ("own-locate", i),
                                text(locale, Key::FoldersLocate),
                                Variant::Ghost,
                                false,
                                t,
                            )
                            .on_click(cx.listener(
                                move |page, _, _, cx| page.locate(owner, path.clone(), cx),
                            )),
                        )
                    })
                    .when(i == 0 && !missing, |this| {
                        this.child(
                            type_scale::SMALL
                                .el()
                                .text_color(t.text_tertiary)
                                .child(text(locale, Key::FoldersPrimary)),
                        )
                    })
                    .when(i > 0, |this| {
                        this.child(
                            ui::button(
                                ("own-primary", i),
                                text(locale, Key::FoldersMakePrimary),
                                Variant::Ghost,
                                false,
                                t,
                            )
                            .on_click(cx.listener(move |page, _, _, cx| page.make_primary(id, cx))),
                        )
                    })
                    .child(
                        ui::ghost_icon_button(
                            format!("own-remove-{i}"),
                            Icon::X,
                            24.,
                            false,
                            false,
                            true,
                            t,
                        )
                        .on_click(cx.listener(move |page, _, _, cx| {
                            page.remove_folder(id, &path_for_remove, cx)
                        })),
                    ),
            );
        }
        let has_own = !folders.own.is_empty();
        let above_name = folders
            .above_from
            .as_ref()
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        if has_own && !folders.above.is_empty() {
            let checked = folders.include_inherited;
            list = list.child(
                div()
                    .id("include-inherited")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .on_click(cx.listener(|page, _, _, cx| page.toggle_include(cx)))
                    .child(checkbox(checked, t))
                    .child(
                        type_scale::UI
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::FoldersInclude)),
                    ),
            );
            if !checked {
                list = list.child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                    locale,
                    Key::FoldersNotUsed,
                    &[
                        ("n", Arg::Count(folders.above.len() as u64)),
                        ("name", Arg::Text(&above_name)),
                    ],
                )));
            }
        }
        let inherited: Vec<_> = if has_own {
            if folders.include_inherited {
                folders.above.clone()
            } else {
                Vec::new()
            }
        } else {
            folders.effective.clone()
        };
        if !inherited.is_empty() {
            let group = folders
                .inherited_from
                .as_ref()
                .or(folders.above_from.as_ref())
                .map(|(from, name)| {
                    let key = match from {
                        Owner::Project(_) => Key::KbFromProject,
                        _ => Key::KbFromEpic,
                    };
                    tr(locale, key, &[("name", Arg::Text(name))])
                })
                .unwrap_or_default();
            list = list.child(
                type_scale::SMALL
                    .el()
                    .mt(px(8.))
                    .text_color(t.text_tertiary)
                    .child(group),
            );
            for (i, folder) in inherited.iter().enumerate() {
                let missing = folders.is_missing(&folder.path);
                let from = folder.from;
                let path = folder.path.clone();
                list = list.child(
                    div()
                        .id(("inherited-folder", i))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .child(icon(
                            if missing {
                                Icon::FolderX
                            } else {
                                Icon::FolderOpen
                            },
                            16.,
                            if missing { t.warning } else { t.text_secondary },
                        ))
                        .child(
                            type_scale::CODE
                                .el()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(t.text_secondary)
                                .child(paths_view::fit(&shown(&folder.path), budget)),
                        )
                        .when(missing, |this| {
                            this.child(
                                type_scale::SMALL
                                    .el()
                                    .text_color(t.text_secondary)
                                    .child(text(locale, Key::FoldersNotFound)),
                            )
                            .child(
                                ui::button(
                                    ("inherited-locate", i),
                                    text(locale, Key::FoldersLocate),
                                    Variant::Ghost,
                                    false,
                                    t,
                                )
                                .on_click(cx.listener(
                                    move |page, _, _, cx| page.locate(from, path.clone(), cx),
                                )),
                            )
                        })
                        .when(i == 0 && !has_own && !missing, |this| {
                            this.child(
                                type_scale::SMALL
                                    .el()
                                    .text_color(t.text_tertiary)
                                    .child(text(locale, Key::FoldersPrimary)),
                            )
                        }),
                );
            }
        }
        let (button_key, add) = if has_own || self.owner.level() == acta_core::model::Level::Project
        {
            (Key::FoldersAdd, true)
        } else {
            (Key::FoldersSetHere, false)
        };
        list = list.child(
            div().pt(px(4.)).child(
                ui::button(
                    "folder-add",
                    text(locale, button_key),
                    Variant::Ghost,
                    false,
                    t,
                )
                .ml(px(-12.))
                .on_click(cx.listener(move |page, _, _, cx| {
                    if add {
                        page.add_folder(cx)
                    } else {
                        page.set_folder(cx)
                    }
                })),
            ),
        );
        let hint = if has_own || self.owner.level() == acta_core::model::Level::Project {
            text(locale, Key::FoldersHint)
        } else {
            tr(
                locale,
                Key::FoldersUses,
                &[(
                    "name",
                    Arg::Text(
                        &folders
                            .inherited_from
                            .as_ref()
                            .map(|f| f.1.clone())
                            .unwrap_or_default(),
                    ),
                )],
            )
        };
        list = list.child(
            type_scale::SMALL
                .el()
                .mt(px(8.))
                .text_color(t.text_tertiary)
                .child(hint),
        );
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(header)
            .child(list)
            .into_any_element()
    }

    fn render_state(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let topic = data.topic.as_ref();
        let meta = topic
            .and_then(|t| t.state_updated_at.zip(t.state_updated_by))
            .map(|(at, by)| {
                let when = when::long(locale, at);
                let author = when::author(locale, by);
                tr(
                    locale,
                    Key::StateUpdated,
                    &[("when", Arg::Text(&when)), ("author", Arg::Text(&author))],
                )
            });
        let body: AnyElement = if let Some(editor) = &self.state_edit {
            editor_box(editor, locale, t, cx)
        } else {
            let state = topic.map(|t| t.state.clone()).unwrap_or_default();
            if state.trim().is_empty() {
                type_scale::READING
                    .el()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::StatePlaceholder))
                    .into_any_element()
            } else {
                markdown_view::short("state-body", state, t).into_any_element()
            }
        };
        div()
            .id("current-state")
            .flex()
            .flex_col()
            .gap(px(8.))
            .mt(px(24.))
            .p(px(16.))
            .rounded(px(RADIUS_MD))
            .bg(t.bg_raised)
            .border_1()
            .border_color(t.border_subtle)
            .when(self.state_edit.is_none(), |this| {
                this.cursor_text().on_click(
                    cx.listener(|page, _, window, cx| page.open_editor(false, window, cx)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(ui::label(text(locale, Key::SectionState), t))
                    .children(
                        meta.map(|m| type_scale::SMALL.el().text_color(t.text_tertiary).child(m)),
                    ),
            )
            .child(body)
            .into_any_element()
    }

    fn render_description(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body: AnyElement = if let Some(editor) = &self.description_edit {
            editor_box(editor, locale, t, cx)
        } else if data.description.trim().is_empty() {
            let placeholder = match self.owner {
                Owner::Topic(_) => Key::DescriptionPlaceholder,
                Owner::Epic(_) => Key::DescriptionPlaceholderEpic,
                Owner::Project(_) => Key::DescriptionPlaceholderProject,
            };
            type_scale::READING
                .el()
                .text_color(t.text_tertiary)
                .child(text(locale, placeholder))
                .into_any_element()
        } else {
            markdown_view::short("description-body", data.description.clone(), t).into_any_element()
        };
        div()
            .id("description")
            .flex()
            .flex_col()
            .gap(px(8.))
            .mt(px(24.))
            .when(self.description_edit.is_none(), |this| {
                this.cursor_text()
                    .on_click(cx.listener(|page, _, window, cx| page.open_editor(true, window, cx)))
            })
            .child(ui::label(text(locale, Key::SectionDescription), t))
            .child(body)
            .into_any_element()
    }

    fn render_kb_row(
        &self,
        entry: &KbSummary,
        index: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = entry.id;
        let kind = entry.kind;
        let open = self.kb_open == Some(id);
        let human = when::initial(&when::human_name(locale));
        let row = ui::focusable(div().id(("kb-row", index)), t, false)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(8.))
            .rounded(px(RADIUS_SM))
            .hover({
                let hover = t.fill_hover;
                move |s| s.bg(hover)
            })
            .on_click(cx.listener(move |page, _, _, cx| match kind {
                KbKind::Document => cx.emit(PageEvent::Open(Route::Kb(id))),
                KbKind::Memory => {
                    page.kb_open = if page.kb_open == Some(id) {
                        None
                    } else {
                        Some(id)
                    };
                    cx.notify();
                }
            }))
            .child(icon(
                if kind == KbKind::Document {
                    Icon::FileText
                } else {
                    Icon::Pin
                },
                16.,
                t.text_secondary,
            ))
            .child(
                type_scale::UI
                    .el()
                    .flex_shrink_0()
                    .max_w(gpui_kit::relative(0.4))
                    .truncate()
                    .text_color(t.text_primary)
                    .child(entry.title.clone()),
            )
            .child(
                type_scale::UI
                    .el()
                    .ml(px(4.))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text_secondary)
                    .child(entry.summary.clone()),
            )
            .child(ui::author_mark(entry.updated_by, &human, t))
            .child(
                type_scale::SMALL
                    .el()
                    .w(px(72.))
                    .flex_shrink_0()
                    .text_right()
                    .font_features(type_scale::tabular())
                    .text_color(t.text_tertiary)
                    .child(when::compact(locale, entry.updated_at)),
            );
        if !open {
            return row.into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .child(row)
            .child(
                div()
                    .pl(px(32.))
                    .pb(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        type_scale::READING
                            .el()
                            .text_color(t.text_primary)
                            .child(entry.summary.clone()),
                    )
                    .child(
                        div().flex().flex_row().gap(px(4.)).ml(px(-12.)).child(
                            ui::button(
                                ("kb-open", index),
                                text(locale, Key::CommonOpen),
                                Variant::Ghost,
                                false,
                                t,
                            )
                            .on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Kb(id))),
                            )),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn render_knowledge(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kb = &data.kb;
        let own_count = kb.own.len();
        let mut section = div().flex().flex_col().mt(px(32.)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .child(ui::label(text(locale, Key::SectionKnowledge), t))
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .font_features(type_scale::tabular())
                        .child(number(locale, own_count as u64)),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .child(
                            ui::ghost_icon_button("kb-new", Icon::Plus, 24., false, false, true, t)
                                .tooltip(ui::tip(format!(
                                    "{} / {}",
                                    text(locale, Key::KbNewDocument),
                                    text(locale, Key::KbNewMemory)
                                )))
                                .on_click(cx.listener(|page, _, window, cx| {
                                    page.kb_new_menu = Some(0);
                                    window.focus(&page.focus, cx);
                                    cx.notify();
                                })),
                        )
                        .children(
                            self.kb_new_menu
                                .map(|h| self.render_kb_new_menu(h, locale, t, cx)),
                        ),
                ),
        );
        if kb.own.is_empty() {
            let empty = match self.owner {
                Owner::Topic(_) => Key::KbEmpty,
                Owner::Epic(_) => Key::KbEmptyEpic,
                Owner::Project(_) => Key::KbEmptyProject,
            };
            section = section.child(
                type_scale::UI
                    .el()
                    .mt(px(4.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, empty)),
            );
        }
        let mut index = 0;
        for entry in kb.own.iter().take(kb_model::SECTION_LIMIT) {
            section = section.child(self.render_kb_row(entry, index, locale, t, cx));
            index += 1;
        }
        if own_count > kb_model::SECTION_LIMIT {
            let project = data.project.0;
            section = section.child(
                ui::button(
                    "kb-show-all",
                    tr(
                        locale,
                        Key::CommonShowAllN,
                        &[("n", Arg::Count(own_count as u64))],
                    ),
                    Variant::Ghost,
                    false,
                    t,
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(PageEvent::Open(Route::Knowledge(project)))
                })),
            );
        }
        let inherited = kb.inherited_count();
        if inherited > 0 {
            if self.kb_inherited_open {
                for (owner, entries) in &kb.inherited {
                    let name = entries
                        .first()
                        .map(|_| owner_label(*owner, data))
                        .unwrap_or_default();
                    let key = match owner {
                        Owner::Project(_) => Key::KbFromProject,
                        _ => Key::KbFromEpic,
                    };
                    section = section.child(
                        type_scale::SMALL
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_tertiary)
                            .child(tr(locale, key, &[("name", Arg::Text(&name))])),
                    );
                    for entry in entries {
                        section = section.child(self.render_kb_row(entry, index, locale, t, cx));
                        index += 1;
                    }
                }
            } else {
                // From the project alone (an epic, or a topic outside any
                // epic), the row names it.
                let only_project = kb
                    .inherited
                    .iter()
                    .all(|(owner, _)| matches!(owner, Owner::Project(_)));
                let label = if only_project {
                    tr(
                        locale,
                        Key::KbMoreInheritedProject,
                        &[
                            ("n", Arg::Count(inherited as u64)),
                            ("name", Arg::Text(&data.project.1)),
                        ],
                    )
                } else {
                    tr(
                        locale,
                        Key::KbMoreInherited,
                        &[("n", Arg::Count(inherited as u64))],
                    )
                };
                section = section.child(div().ml(px(-12.)).child(
                    ui::button("kb-inherited", label, Variant::Ghost, false, t).on_click(
                        cx.listener(|page, _, _, cx| {
                            page.kb_inherited_open = true;
                            cx.notify();
                        }),
                    ),
                ));
            }
        }
        if data.kb_below > 0 {
            let key = match self.owner {
                Owner::Epic(_) => Key::KbMoreEpic,
                _ => Key::KbMoreProject,
            };
            let project = data.project.0;
            section = section.child(
                div().ml(px(-12.)).child(
                    ui::button(
                        "kb-below",
                        tr(locale, key, &[("n", Arg::Count(data.kb_below as u64))]),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(PageEvent::Open(Route::Knowledge(project)))
                    })),
                ),
            );
        }
        section.into_any_element()
    }

    fn render_timeline(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = EntryFilterKind::ALL
            .into_iter()
            .enumerate()
            .map(|(i, filter)| {
                let key = match filter {
                    EntryFilterKind::All => Key::TimelineAll,
                    EntryFilterKind::Progress => Key::EntryProgress,
                    EntryFilterKind::Decisions => Key::EntryDecisions,
                    EntryFilterKind::Discussion => Key::EntryDiscussion,
                };
                ui::segment(
                    ("timeline-filter", i),
                    text(locale, key),
                    self.filter == filter,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.filter = filter;
                    page.reload(cx);
                }))
            })
            .collect::<Vec<_>>();
        let composer = self.render_composer(locale, t, cx);
        let mut list = div().flex().flex_col().gap(px(24.)).mt(px(24.));
        let human = when::initial(&when::human_name(locale));
        // System events other than status changes have no line in the
        // catalogue yet (docs/design/data.md 11, request 10): not shown.
        let shown: Vec<&Entry> = data
            .entries
            .iter()
            .filter(|e| {
                e.kind != EntryKind::System
                    || e.event
                        .as_ref()
                        .is_some_and(|ev| ev.kind == acta_core::model::EventKind::Status)
            })
            .collect();
        // The empty state is about what is shown: a timeline of hidden
        // system events alone is empty too.
        if shown.iter().all(|e| e.kind == EntryKind::System) {
            list = list.child(
                type_scale::READING
                    .el()
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::TimelineEmpty)),
            );
        }
        for (i, entry) in shown.into_iter().enumerate() {
            list = list.child(self.render_entry(entry, i, &human, locale, t, cx));
        }
        div()
            .flex()
            .flex_col()
            .mt(px(32.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(ui::label(text(locale, Key::SectionTimeline), t))
                    .child(ui::segmented(t).children(filters)),
            )
            .child(div().mt(px(12.)).child(composer))
            .child(list)
            .into_any_element()
    }

    fn render_composer(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let kinds = [
            EntryKind::Progress,
            EntryKind::Decision,
            EntryKind::Discussion,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            let (_, key) = entry_kind_meta(kind);
            ui::segment(
                ("composer-kind", i),
                text(locale, key),
                self.composer_kind == kind,
                t,
            )
            .on_click(cx.listener(move |page, _, _, cx| {
                page.composer_kind = kind;
                cx.notify();
            }))
        })
        .collect::<Vec<_>>();
        let macos = cfg!(target_os = "macos");
        let keys = format!(
            "{}+{}",
            KeyName::Mod.label(locale, macos),
            KeyName::Enter.label(locale, macos)
        );
        let hint = tr(locale, Key::ComposerHint, &[("keys", Arg::Text(&keys))]);
        div()
            .flex()
            .flex_col()
            .rounded(px(RADIUS_MD))
            .bg(t.bg_raised)
            .border_1()
            .border_color(t.border_strong)
            .px(px(16.))
            .py(px(12.))
            .child(
                type_scale::READING.el().child(
                    Textarea::new(&self.composer)
                        .appearance(false)
                        .text_size(px(type_scale::READING.size))
                        .line_height(px(type_scale::READING.line)),
                ),
            )
            .when(self.composer_open, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .mt(px(8.))
                        .child(ui::segmented(t).children(kinds))
                        .child(div().flex_1())
                        .child(
                            type_scale::SMALL
                                .el()
                                .text_color(t.text_tertiary)
                                .child(hint),
                        )
                        .child(
                            ui::button(
                                "composer-add",
                                text(locale, Key::ComposerAdd),
                                Variant::Primary,
                                false,
                                t,
                            )
                            .on_click(
                                cx.listener(|page, _, window, cx| page.add_entry(window, cx)),
                            ),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_entry(
        &self,
        entry: &Entry,
        index: usize,
        human: &str,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if entry.kind == EntryKind::System {
            let line = system_line(entry, locale);
            // The gutter shows the glyph of the status it was set to.
            let glyph = event_status(entry)
                .map(|s| ui::status_glyph(s, 14., t))
                .unwrap_or_else(|| icon(Icon::CircleDot, 14., t.text_tertiary));
            return div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child(div().w(px(20.)).flex().justify_center().child(glyph))
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(line),
                )
                .into_any_element();
        }
        let (glyph, key) = entry_kind_meta(entry.kind);
        let decision = entry.kind == EntryKind::Decision;
        let name = when::author(locale, entry.author);
        let name_el = match entry.author {
            Author::Claude => type_scale::CODE.el().text_color(t.text_primary).child(name),
            Author::Human => type_scale::UI_STRONG
                .el()
                .text_color(t.text_primary)
                .child(name),
        };
        let entry_for_menu = entry.clone();
        let menu_open = self
            .entry_menu
            .as_ref()
            .is_some_and(|m| m.entry.id == entry.id);
        let menu = menu_open.then(|| self.render_entry_menu(locale, t, cx));
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(20.))
            .child(name_el)
            .child(icon(glyph, 14., t.text_secondary))
            .child(
                type_scale::SMALL
                    .el()
                    .text_color(if decision {
                        t.text_primary
                    } else {
                        t.text_secondary
                    })
                    .when(decision, |this| {
                        this.font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    })
                    .child(text(locale, key)),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .font_features(type_scale::tabular())
                    .text_color(t.text_tertiary)
                    .child(when::long_alone(locale, entry.stamp.created_at)),
            )
            // An entry edited after it was written says so; the previous
            // wording is kept in an `edited` event (ADR 0005, append-only).
            .when(entry.stamp.updated_at != entry.stamp.created_at, |this| {
                let mark = if entry.stamp.updated_by != entry.author {
                    let name = when::author(locale, entry.stamp.updated_by);
                    tr(locale, Key::EntryEditedBy, &[("name", Arg::Text(&name))])
                } else {
                    text(locale, Key::EntryEdited)
                };
                this.child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_tertiary)
                        .child(mark),
                )
            })
            .child(div().flex_1())
            .child(
                div()
                    .when(!menu_open, |this| {
                        this.invisible()
                            .group_hover(SharedString::from(format!("entry-{index}")), |s| {
                                s.visible()
                            })
                    })
                    .child(
                        ui::ghost_icon_button(
                            format!("entry-menu-{index}"),
                            Icon::Ellipsis,
                            24.,
                            menu_open,
                            false,
                            true,
                            t,
                        )
                        .on_click(cx.listener(
                            move |page, _, window, cx| {
                                page.open_entry_menu(entry_for_menu.clone(), window, cx)
                            },
                        )),
                    )
                    .children(menu),
            );
        let editing = self
            .entry_edit
            .as_ref()
            .filter(|(id, _)| *id == entry.id)
            .map(|(_, editor)| editor.clone());
        let body: AnyElement = match editing {
            Some(editor) => editor_box(&editor, locale, t, cx),
            None => {
                markdown_view::short(("entry", index), entry.text.clone(), t).into_any_element()
            }
        };
        let content = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .flex_1()
            .min_w_0()
            .child(header)
            .child(body);
        let entry_for_keys = entry.clone();
        ui::focusable(div().id(("entry-row", index)), t, false)
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, window, cx| {
                // Shift+F10 or the context-menu key: the entry menu.
                let k = &event.keystroke;
                if (k.key == "f10" && k.modifiers.shift) || k.key == "menu" {
                    page.open_entry_menu(entry_for_keys.clone(), window, cx);
                    cx.stop_propagation();
                }
            }))
            .rounded(px(RADIUS_MD))
            .group(SharedString::from(format!("entry-{index}")))
            .flex()
            .flex_row()
            .gap(px(12.))
            .when(decision, |this| {
                this.px(px(16.))
                    .py(px(12.))
                    .rounded(px(RADIUS_MD))
                    .border_1()
                    .border_color(t.border_default)
            })
            .child(ui::author_mark(entry.author, human, t))
            .child(content)
            .into_any_element()
    }

    fn render_table(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = table::Filter::ALL
            .into_iter()
            .enumerate()
            .map(|(i, filter)| {
                let key = match filter {
                    table::Filter::Open => Key::TableFilterOpen,
                    table::Filter::All => Key::TableFilterAll,
                    table::Filter::Closed => Key::TableFilterClosed,
                };
                ui::segment(
                    ("table-filter", i),
                    text(locale, key),
                    self.table_filter == filter,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.table_filter = filter;
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();
        let all: Vec<TopicSummary> = if matches!(self.owner, Owner::Epic(_)) {
            data.topics.clone()
        } else {
            let mut all = data.loose.clone();
            for (_, _, topics) in &data.groups {
                all.extend(topics.iter().cloned());
            }
            all
        };
        if all.is_empty() {
            return self.render_no_topics(locale, t, cx);
        }
        let count = all.iter().filter(|t| self.table_filter.keeps(t)).count();
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .h(px(28.))
            .child(ui::segmented(t).children(filters))
            .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                locale,
                Key::TableCount,
                &[("n", Arg::Count(count as u64))],
            )))
            .when(self.table_sort.is_some(), |this| {
                this.child(
                    ui::button(
                        "table-reset",
                        text(locale, Key::TableReset),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.table_sort = None;
                        cx.notify();
                    })),
                )
            })
            .child(div().flex_1())
            .child(
                ui::ghost_icon_button("table-new", Icon::Plus, 24., false, false, true, t)
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(Box::new(crate::shell::NewTopic), cx);
                    })),
            );
        let header_cell = |id: &'static str,
                           column: Column,
                           key: Option<Key>,
                           width: Option<f32>,
                           right: bool,
                           page: &Self,
                           cx: &mut Context<Self>| {
            let arrow = page.table_sort.filter(|s| s.column == column).map(|s| {
                icon(
                    if s.ascending {
                        Icon::ChevronUp
                    } else {
                        Icon::ChevronDown
                    },
                    12.,
                    t.text_tertiary,
                )
            });
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .when(right, |this| this.justify_end())
                .map(|this| match width {
                    Some(w) => this.w(px(w)).flex_shrink_0(),
                    None => this.flex_1().min_w_0(),
                })
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.table_sort = Some(table::next_sort(page.table_sort, column));
                    cx.notify();
                }))
                .children(key.map(|key| text(locale, key)))
                .children(arrow)
        };
        let header = type_scale::SMALL
            .el()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(8.))
            .mt(px(8.))
            .text_color(t.text_tertiary)
            .border_b_1()
            .border_color(t.border_subtle)
            .child(header_cell(
                "th-status",
                Column::Status,
                None,
                Some(24.),
                false,
                self,
                cx,
            ))
            .child(header_cell(
                "th-title",
                Column::Title,
                Some(Key::TableTitle),
                None,
                false,
                self,
                cx,
            ))
            .child(header_cell(
                "th-entries",
                Column::Entries,
                Some(Key::TableEntries),
                Some(56.),
                true,
                self,
                cx,
            ))
            .child(header_cell(
                "th-updated",
                Column::Updated,
                Some(Key::TableUpdated),
                Some(72.),
                true,
                self,
                cx,
            ));
        let mut body = div().flex().flex_col();
        let mut row_index: usize = 0;
        let mut row = |topic: &TopicSummary, cx: &mut Context<Self>| {
            let id = topic.id;
            let open = topic.status.is_open();
            let r = ui::focusable(div().id(("table-row", row_index)), t, false)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(32.))
                .px(px(8.))
                .rounded(px(RADIUS_SM))
                .hover({
                    let hover = t.fill_hover;
                    move |s| s.bg(hover)
                })
                .on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(PageEvent::Open(Route::Topic(id)))),
                )
                .child(div().w(px(24.)).flex_shrink_0().child(ui::status_glyph(
                    topic.status,
                    14.,
                    t,
                )))
                .child(
                    type_scale::UI
                        .el()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(if open {
                            t.text_primary
                        } else {
                            t.text_tertiary
                        })
                        .child(topic.title.clone()),
                )
                .child(
                    type_scale::SMALL
                        .el()
                        .w(px(56.))
                        .flex_shrink_0()
                        .text_right()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(number(locale, topic.entries.max(0) as u64)),
                )
                .child(
                    type_scale::SMALL
                        .el()
                        .w(px(72.))
                        .flex_shrink_0()
                        .text_right()
                        .font_features(type_scale::tabular())
                        .text_color(t.text_tertiary)
                        .child(when::compact(locale, topic.updated_at)),
                );
            row_index += 1;
            r
        };
        if matches!(self.owner, Owner::Epic(_)) {
            for topic in table::rows(&data.topics, self.table_filter, self.table_sort) {
                body = body.child(row(topic, cx));
            }
        } else {
            for (epic, topics) in table::groups(&data.loose, &data.groups) {
                let (done, total) = table::progress(topics);
                let name: SharedString = match epic {
                    Some((_, name)) => name.to_owned().into(),
                    None => text(locale, Key::TreeNoEpic).into(),
                };
                let group = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(28.))
                    .px(px(8.))
                    .mt(px(8.))
                    .child(ui::label(name, t))
                    .child(
                        type_scale::SMALL
                            .el()
                            .font_features(type_scale::tabular())
                            .text_color(t.text_tertiary)
                            .child(tr(
                                locale,
                                Key::TableGroupProgress,
                                &[
                                    ("done", Arg::Count(done as u64)),
                                    ("total", Arg::Count(total as u64)),
                                ],
                            )),
                    );
                body = body.child(group);
                for topic in table::rows(topics, self.table_filter, self.table_sort) {
                    body = body.child(row(topic, cx));
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .mt(px(32.))
            .child(toolbar)
            .child(header)
            .child(body)
            .into_any_element()
    }

    /// No topics at all (components.md 5, Empty state): the heading, one
    /// sentence, then New topic with its key.
    fn render_no_topics(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let macos = cfg!(target_os = "macos");
        div()
            .flex()
            .flex_col()
            .mt(px(32.))
            .child(
                type_scale::HEADING
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, Key::TopicEmptyHeading)),
            )
            .child(
                type_scale::READING
                    .el()
                    .mt(px(8.))
                    .max_w(px(480.))
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::TopicEmptyProject)),
            )
            .child(
                div().mt(px(16.)).flex().child(
                    ui::button(
                        "table-empty-new",
                        text(locale, Key::TopicNew),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .child(ui::kbd_on_accent(
                        &[KeyName::Mod.label(locale, macos), "T"],
                        t,
                    ))
                    .on_click(cx.listener(|_, _, window, cx| {
                        window.dispatch_action(Box::new(crate::shell::NewTopic), cx);
                    })),
                ),
            )
            .into_any_element()
    }

    /// The entry menu, under its `ellipsis` button.
    fn render_entry_menu(&self, locale: Locale, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.entry_menu else {
            return div().into_any_element();
        };
        let mut panel =
            menu::panel("entry-menu", t).on_mouse_down_out(cx.listener(|page, _, _, cx| {
                page.entry_menu = None;
                cx.notify();
            }));
        if menu.kinds {
            for (i, kind) in ENTRY_KINDS.into_iter().enumerate() {
                let (glyph, key) = entry_kind_meta(kind);
                panel =
                    panel.child(
                        menu::item(
                            ("entry-kind", i),
                            Some(icon(glyph, 16., t.text_secondary).into_any_element()),
                            text(locale, key),
                            (kind == menu.entry.kind).then(|| menu::check(t)),
                            i == menu.highlight,
                            t,
                        )
                        .on_click(cx.listener(
                            move |page, _, window, cx| page.choose_entry_item(i, window, cx),
                        )),
                    );
            }
        } else {
            let items = [
                Key::CommonEdit,
                Key::EntryCopyMarkdown,
                Key::EntryChangeType,
                Key::CommonDelete,
            ];
            for (i, key) in items.into_iter().enumerate() {
                let danger = i == 3;
                if danger {
                    panel = panel.child(menu::separator(t));
                }
                panel =
                    panel.child(
                        menu::item(
                            ("entry-item", i),
                            None,
                            text(locale, key),
                            None,
                            i == menu.highlight,
                            t,
                        )
                        .when(danger, |this| this.text_color(t.danger))
                        .on_click(cx.listener(
                            move |page, _, window, cx| page.choose_entry_item(i, window, cx),
                        )),
                    );
            }
        }
        deferred(anchored().offset(point(px(-176.), px(28.))).child(panel))
            .with_priority(1)
            .into_any_element()
    }

    /// The `plus` menu of the knowledge section: New document, New memory,
    /// with their keys.
    fn render_kb_new_menu(
        &self,
        highlight: usize,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let macos = cfg!(target_os = "macos");
        let m = KeyName::Mod.label(locale, macos);
        let shift = KeyName::Shift.label(locale, macos);
        let rows = [
            (Key::KbNewDocument, Icon::FileText, "D", KbKind::Document),
            (Key::KbNewMemory, Icon::Pin, "M", KbKind::Memory),
        ];
        let mut panel =
            menu::panel("kb-new-menu", t).on_mouse_down_out(cx.listener(|page, _, _, cx| {
                page.kb_new_menu = None;
                cx.notify();
            }));
        for (i, (key, glyph, letter, kind)) in rows.into_iter().enumerate() {
            panel = panel.child(
                menu::item(
                    ("kb-new-item", i),
                    Some(icon(glyph, 16., t.text_secondary).into_any_element()),
                    text(locale, key),
                    Some(ui::kbd(&[m, shift, letter], t).into_any_element()),
                    i == highlight,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.kb_new_menu = None;
                    page.new_kb(kind, cx);
                })),
            );
        }
        deferred(anchored().offset(point(px(-176.), px(28.))).child(panel))
            .with_priority(1)
            .into_any_element()
    }
}

/// The status a system event set, if it is a status change.
fn event_status(entry: &Entry) -> Option<Status> {
    entry
        .event
        .as_ref()
        .filter(|e| e.kind == acta_core::model::EventKind::Status)
        .and_then(|e| e.new.as_deref())
        .and_then(|s| {
            Status::ALL
                .into_iter()
                .find(|st| format!("{st:?}").eq_ignore_ascii_case(s))
        })
}

fn owner_label(owner: Owner, data: &Loaded) -> String {
    match owner {
        Owner::Project(_) => data.project.1.clone(),
        Owner::Epic(_) => data.epic.as_ref().map(|e| e.1.clone()).unwrap_or_default(),
        Owner::Topic(_) => data.title.clone(),
    }
}

fn system_line(entry: &Entry, locale: Locale) -> String {
    let author = when::author(locale, entry.author);
    let when = when::long(locale, entry.stamp.created_at);
    let status = event_status(entry);
    let what = match status {
        Some(status) => {
            let name = text(locale, ui::status_key(status));
            tr(
                locale,
                Key::EventStatus,
                &[("author", Arg::Text(&author)), ("status", Arg::Text(&name))],
            )
        }
        None => entry.text.clone(),
    };
    format!("{what} \u{2013} {when}")
}

fn progress(data: &Loaded, locale: Locale, t: &Tokens) -> AnyElement {
    let (done, total) = data.progress();
    if total == 0 {
        // No topic to count: the words alone, no empty bar.
        return div()
            .text_color(t.text_tertiary)
            .child(text(locale, Key::ProgressNone))
            .into_any_element();
    }
    let fraction = if total > 0 {
        done as f32 / total as f32
    } else {
        0.
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
        .child(tr(
            locale,
            Key::PropProgressValue,
            &[
                ("done", Arg::Count(done as u64)),
                ("total", Arg::Count(total as u64)),
            ],
        ))
        .child(
            div()
                .w(px(120.))
                .h(px(4.))
                .rounded(px(RADIUS_SM))
                .bg(t.fill_selected)
                .child(
                    div()
                        .h_full()
                        .w(px(120. * fraction))
                        .rounded(px(RADIUS_SM))
                        .bg(t.text_secondary),
                ),
        )
        .into_any_element()
}

/// A link in the breadcrumb style: `text.primary`, underlined on hover.
fn link(
    id: impl Into<gpui_kit::ElementId>,
    label: String,
    t: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id.into())
        .cursor_pointer()
        .text_color(t.accent)
        .hover(|s| s.underline())
        .child(label)
}

fn checkbox(checked: bool, t: &Tokens) -> gpui_kit::Div {
    div()
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS_SM))
        .when(checked, |this| {
            this.bg(t.accent)
                .child(icon(Icon::Check, 12., t.text_on_accent))
        })
        .when(!checked, |this| {
            this.border_1().border_color(t.border_strong)
        })
}

/// A multi-line editor in its box (docs/design/components.md 5), with the
/// footer hint.
fn editor_box(
    editor: &Entity<TextareaState>,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<RecordPage>,
) -> AnyElement {
    let macos = cfg!(target_os = "macos");
    let keys = format!(
        "{}+{}",
        KeyName::Mod.label(locale, macos),
        KeyName::Enter.label(locale, macos)
    );
    div()
        .flex()
        .flex_col()
        .rounded(px(RADIUS_MD))
        .bg(t.bg_raised)
        .border_2()
        .border_color(t.focus_ring)
        .px(px(16.))
        .py(px(12.))
        .on_action(cx.listener(|page, _: &Escape, window, cx| page.cancel_editors(window, cx)))
        .child(
            type_scale::READING.el().child(
                Textarea::new(editor)
                    .appearance(false)
                    .text_size(px(type_scale::READING.size))
                    .line_height(px(type_scale::READING.line)),
            ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .h(px(28.))
                .items_center()
                .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                    locale,
                    Key::ComposerHint,
                    &[("keys", Arg::Text(&keys))],
                ))),
        )
        .into_any_element()
}

impl Render for RecordPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let _ = window;
        let mut column = ui::column();
        if self.gone {
            column = column.child(
                ui::notice(
                    Some((Icon::Info, t.text_secondary)),
                    text(locale, Key::DeletedNotice),
                    &t,
                )
                .child(
                    ui::button(
                        "deleted-restore",
                        text(locale, Key::DeletedRestore),
                        Variant::Ghost,
                        false,
                        &t,
                    )
                    .on_click(cx.listener(|page, _, _, cx| page.restore_record(cx))),
                ),
            );
        }
        if let Some(data) = self.data.clone() {
            column = column
                .child(self.render_title(&data, &t, cx))
                .child(self.render_properties(&data, locale, &t, cx));
            if self.topic().is_some() {
                column = column.child(self.render_state(&data, locale, &t, cx));
            }
            column = column
                .child(self.render_description(&data, locale, &t, cx))
                .child(self.render_knowledge(&data, locale, &t, cx));
            if self.topic().is_some() {
                column = column.child(self.render_timeline(&data, locale, &t, cx));
            } else {
                column = column.child(self.render_table(&data, locale, &t, cx));
            }
        }
        ui::page_scroll("record-page")
            .track_focus(&self.focus)
            .key_context("RecordPage")
            .on_action(cx.listener(|page, _: &OpenStatusMenu, window, cx| {
                if let Some(status) = page
                    .data
                    .as_ref()
                    .and_then(|d| d.topic.as_ref())
                    .map(|t| t.status)
                {
                    page.status_menu =
                        Some(Status::ALL.iter().position(|s| *s == status).unwrap_or(0));
                    window.focus(&page.focus, cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|page, _: &EditTitle, window, cx| page.edit_title(window, cx)))
            .on_action(cx.listener(|page, _: &Escape, window, cx| page.cancel_editors(window, cx)))
            .on_key_down(cx.listener(Self::on_key))
            .child(column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_records_are_restored_as_the_same_record() {
        assert_eq!(
            record_of(Owner::Topic(TopicId(4))),
            Record::Topic(TopicId(4))
        );
        assert_eq!(
            record_of(Owner::Epic(acta_core::model::EpicId(2))),
            Record::Epic(acta_core::model::EpicId(2))
        );
    }

    #[test]
    fn decisions_are_the_outlined_kind_and_system_events_read_as_lines() {
        assert_eq!(entry_kind_meta(EntryKind::Decision).1, Key::EntryDecision);
        assert_eq!(
            entry_kind_meta(EntryKind::Progress).0,
            Icon::GitCommitHorizontal
        );
    }
}
