//! The working folders of a page (docs/design/components-folders.md 5;
//! screens.md 8.11; ADR 0011 and its addenda): the folded row with the
//! switcher and the git state, the grouped list with the active mark, the
//! row menus, the folder settings form, and the human git actions (Pull,
//! Fetch, the branch selector). Disk and git run off the UI thread; the
//! git state is read when the page opens, when the window regains focus
//! and when the folder monitor reloads the pages, never by watching `.git`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use acta_core::claude_settings::page::{self as page_commit, PageCommit};
use acta_core::claude_settings::{self, FileState};
use acta_core::folder_path;
use acta_core::git_cmd::{self, Branches, GitError, Pulled, StatusSummary, SwitchTo};
use acta_core::model::{CommitMode, FolderId};
use acta_core::paths::Os;
use acta_core::presence::SessionState;

use super::*;
use crate::dialogs::Confirm;
use crate::folders_model::{self as fm, Disk, FolderItem, GitLine};
use crate::folders_view::{self, SwitchActions, SwitchEntry, SwitchGroup};
use crate::palette::OnPick;
use crate::record::Folders;
use crate::workspace::ToastKind;

/// The state of the folder surface of one page.
#[derive(Default)]
pub(super) struct FolderUi {
    /// The row unfolded into the list.
    pub(super) open: bool,
    /// The switcher's menu, with its highlighted entry.
    switcher: Option<usize>,
    /// A list row's menu.
    row_menu: Option<FolderId>,
    /// The folder whose settings form is unfolded, and its name field.
    settings: Option<FolderId>,
    name_input: Option<Entity<InputState>>,
    _name_sub: Option<Subscription>,
    /// The branch selector of a folder.
    branch_menu: Option<BranchMenu>,
    /// Where the switcher's, a branch's and a row's menus hang.
    switch_anchor: menu::AnchorBounds,
    branch_anchor: menu::AnchorBounds,
    row_anchor: menu::AnchorBounds,
    /// Disk facts and git state, by stored path.
    disk: HashMap<String, Disk>,
    status: HashMap<String, StatusSummary>,
    /// `git config user.name` and the settings file, for the form.
    identity: HashMap<String, Option<String>>,
    files: HashMap<String, FileState>,
    /// The workspace's git epoch last read.
    git_seen: Option<u64>,
    /// Folders a git action runs in: their buttons wait.
    busy: HashSet<String>,
    /// One focus handle per folder of the list, and the list's order as
    /// last drawn: the rows are keyboard stops (interaction.md 6.1).
    row_focus: HashMap<FolderId, FocusHandle>,
    row_order: std::cell::RefCell<Vec<FolderId>>,
    _git: Option<Task<()>>,
}

struct BranchMenu {
    folder: FolderId,
    /// `None` while the list loads.
    branches: Option<Branches>,
    /// "New branch...": the name field.
    new_branch: Option<Entity<InputState>>,
    _sub: Option<Subscription>,
}

impl FolderUi {
    /// A menu or form of the folders is open (the chat's web view hides).
    #[cfg(windows)]
    pub(super) fn any_open(&self) -> bool {
        self.switcher.is_some() || self.row_menu.is_some() || self.branch_menu.is_some()
    }

    /// `Esc`: closes the newest menu or form. Whether there was one.
    fn close_one(&mut self) -> bool {
        if self.branch_menu.take().is_some()
            || self.row_menu.take().is_some()
            || self.switcher.take().is_some()
        {
            return true;
        }
        if self.settings.take().is_some() {
            self.name_input = None;
            self._name_sub = None;
            return true;
        }
        false
    }
}

/// Whether a Claude Code session started in Acta's terminal is working in
/// `path` now (the addendum: switching a branch then asks first).
fn session_working_in(path: &str, cx: &App) -> bool {
    let Some(running) = cx.try_global::<crate::terminal_dock::RunningIn>() else {
        return false;
    };
    let live = crate::sessions::live(cx);
    running.0.iter().any(|(dir, token)| {
        fm::overlaps(&dir.to_string_lossy(), path)
            && crate::sessions::of_launch(live, token)
                .is_some_and(|s| s.state == SessionState::Working)
    })
}

impl RecordPage {
    fn folders(&self) -> Option<&Folders> {
        self.data.as_ref().map(|d| &d.folders)
    }

    fn folder(&self, id: FolderId) -> Option<FolderItem> {
        self.folders()?.get(id).cloned()
    }

    /// "Show working folders": the folder row unfolded.
    pub fn show_folders(&mut self, cx: &mut Context<Self>) {
        self.folder_ui.open = true;
        cx.notify();
    }

    #[cfg(windows)]
    pub(super) fn folder_menu_open(&self) -> bool {
        self.folder_ui.any_open()
    }

    /// The workspace's git epoch moved since the git state was read.
    pub(super) fn folder_ui_epoch_moved(&self, epoch: u64) -> bool {
        self.data.is_some() && self.folder_ui.git_seen.is_some_and(|seen| seen != epoch)
    }

    /// Keys while a folder menu is open: the switcher steps with `Up` /
    /// `Down` and chooses with `Enter` or `Space`; `Esc` folds the newest
    /// menu or form. Whether the key was taken.
    pub(super) fn folder_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.modifiers.modified() {
            return false;
        }
        let key = event.keystroke.key.as_str();
        if let (Some(at), Some(folders)) = (self.folder_ui.switcher, self.folders()) {
            let locale = settings::locale(cx);
            let entries = folders_view::choosable(&self.switch_groups(folders, locale));
            match key {
                "up" => self.folder_ui.switcher = Some(at.saturating_sub(1)),
                "down" => {
                    self.folder_ui.switcher = Some((at + 1).min(entries.len().saturating_sub(1)))
                }
                "enter" | "space" => {
                    let edit = entries.len().saturating_sub(2);
                    match entries.get(at).copied().flatten() {
                        Some(id) => self.set_active(id, cx),
                        None if at == edit => {
                            self.folder_ui.switcher = None;
                            self.folder_ui.open = true;
                        }
                        None => {
                            self.folder_ui.switcher = None;
                            self.add_folder(cx);
                        }
                    }
                }
                "escape" => self.folder_ui.switcher = None,
                // Type-ahead by name (components.md 5, Context menu).
                typed if typed.chars().count() == 1 => {
                    let groups = self.switch_groups(folders, locale);
                    match folders_view::type_ahead(&groups, at, typed) {
                        Some(i) => self.folder_ui.switcher = Some(i),
                        None => return false,
                    }
                }
                _ => return false,
            }
            let _ = window;
            cx.stop_propagation();
            cx.notify();
            return true;
        }
        if key == "escape" && self.folders_escape(cx) {
            cx.stop_propagation();
            return true;
        }
        false
    }

    /// Before drawing: a focus handle for each folder of the page.
    pub(super) fn folder_handles(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<FolderId> = self
            .folders()
            .map(|f| f.items.iter().map(|i| i.id).collect())
            .unwrap_or_default();
        for id in ids {
            self.folder_ui
                .row_focus
                .entry(id)
                .or_insert_with(|| cx.focus_handle());
        }
    }

    /// The folder row with the keyboard.
    pub(super) fn focused_folder(&self, window: &Window) -> Option<FolderId> {
        self.folder_ui
            .row_focus
            .iter()
            .find(|(_, h)| h.is_focused(window))
            .map(|(id, _)| *id)
    }

    fn focus_folder(&mut self, id: FolderId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(h) = self.folder_ui.row_focus.get(&id) {
            window.focus(h, cx);
        }
        cx.notify();
    }

    /// `Enter` or `Right` on the folder row: the list unfolds and the
    /// keyboard goes to the active folder.
    pub(super) fn enter_folders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_ui.open = true;
        if let Some(active) = self.folders().and_then(|f| f.active()).map(|f| f.id) {
            self.focus_folder(active, window, cx);
        }
        cx.notify();
    }

    /// Keys on the folder row (one of the page's blocks): `Enter` / `Right`
    /// unfold and go in, `Left` / `Esc` fold, `Space` / `Alt+Down` open the
    /// switcher. Whether the key was taken.
    pub(super) fn folder_row_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let k = &event.keystroke;
        let alt_only = k.modifiers.alt && !k.modifiers.control && !k.modifiers.platform;
        let plain = !k.modifiers.modified();
        match k.key.as_str() {
            "right" if plain => self.enter_folders(window, cx),
            "left" | "escape" if plain && self.folder_ui.open => {
                self.folder_ui.open = false;
                cx.notify();
            }
            "space" if plain => self.folder_ui.switcher = Some(0),
            "down" if alt_only => self.folder_ui.switcher = Some(0),
            _ => return false,
        }
        cx.stop_propagation();
        cx.notify();
        true
    }

    /// Keys on a folder of the list (interaction.md 6.1): on any folder,
    /// `Space` makes it active, `Enter` reveals it (Locate when not found),
    /// `Mod+C` copies its path, `Shift+F10` or the menu key opens its menu;
    /// on an own folder, `Alt+Up` / `Alt+Down` reorder, `F2` opens its
    /// settings, `E` chooses another folder, `Delete` removes it. `Up` /
    /// `Down` move between folders, `Left` / `Esc` fold the list.
    fn on_folder_key(
        &mut self,
        id: FolderId,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(folders) = self.folders() else {
            return;
        };
        let Some(f) = folders.get(id).cloned() else {
            return;
        };
        let missing = folders.is_missing(&f.path);
        let k = &event.keystroke;
        let m = &k.modifiers;
        let plain = !m.modified();
        let alt_only = m.alt && !m.control && !m.platform && !m.shift;
        let secondary = if cfg!(target_os = "macos") {
            m.platform && !m.control
        } else {
            m.control && !m.platform
        };
        let action = folder_key(
            &k.key,
            plain,
            alt_only,
            secondary && !m.alt && !m.shift,
            m.shift && !m.control && !m.platform && !m.alt,
            m.platform,
        );
        let Some(action) = action else { return };
        let own = f.own;
        match action {
            FolderKey::Step(up) => {
                let order = self.folder_ui.row_order.borrow().clone();
                let at = order.iter().position(|r| *r == id).unwrap_or(0);
                let to = if up {
                    at.checked_sub(1)
                } else {
                    (at + 1 < order.len()).then_some(at + 1)
                };
                match to.and_then(|i| order.get(i).copied()) {
                    Some(next) => self.focus_folder(next, window, cx),
                    None if up => self.focus_block(record::Block::Folders, window, cx),
                    None => {
                        let next = record::step_block(
                            &self.block_order,
                            Some(record::Block::Folders),
                            false,
                        );
                        if let Some(next) = next {
                            self.focus_block(next, window, cx);
                        }
                    }
                }
            }
            FolderKey::Activate if !missing => self.set_active(id, cx),
            FolderKey::Open if missing => self.locate(f.origin, f.path.clone(), cx),
            FolderKey::Open => cx.reveal_path(Path::new(&f.path)),
            FolderKey::Copy => cx.write_to_clipboard(ClipboardItem::new_string(f.path.clone())),
            FolderKey::Menu => {
                self.folder_ui.row_menu = Some(id);
                self.folder_ui.switcher = None;
                self.folder_ui.branch_menu = None;
            }
            FolderKey::Move(delta) if own => {
                self.move_folder(id, delta, cx);
                self.focus_folder(id, window, cx);
            }
            FolderKey::Settings if own => self.open_folder_settings(id, window, cx),
            FolderKey::Settings => self.open_page_commit(id, cx),
            FolderKey::Other if own => self.choose_other(id, cx),
            FolderKey::Remove if own => {
                self.remove_folder(id, cx);
                self.focus_block(record::Block::Folders, window, cx);
            }
            FolderKey::Fold => {
                self.folder_ui.open = false;
                self.focus_block(record::Block::Folders, window, cx);
            }
            // On an inherited folder those keys do nothing; a folder not
            // found cannot be made active.
            _ => {}
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// The palette's commands on the focused folder (interaction.md 6.1).
    pub fn focused_folder_commands(
        &self,
        window: &Window,
    ) -> Option<crate::commands::FocusedFolder> {
        let id = self.focused_folder(window)?;
        let folders = self.folders()?;
        let f = folders.get(id)?;
        let own: Vec<FolderId> = folders
            .items
            .iter()
            .filter(|item| item.own)
            .map(|item| item.id)
            .collect();
        let (up, down) = if f.own {
            move_options(&own, id)
        } else {
            (false, false)
        };
        Some(crate::commands::FocusedFolder {
            own: f.own,
            active: f.active,
            missing: folders.is_missing(&f.path),
            up,
            down,
            level: match self.owner {
                Owner::Topic(_) => crate::commands::FolderLevel::Topic,
                Owner::Epic(_) => crate::commands::FolderLevel::Epic,
                Owner::Project(_) => crate::commands::FolderLevel::Project,
            },
        })
    }

    /// A palette command on the focused folder.
    pub fn run_folder_command(
        &mut self,
        command: crate::commands::FolderCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::commands::FolderCommand as C;
        let Some(id) = self.focused_folder(window) else {
            return;
        };
        let Some(f) = self.folder(id) else { return };
        match command {
            C::Active => self.set_active(id, cx),
            C::Settings => self.open_folder_settings(id, window, cx),
            C::Up => self.move_folder(id, -1, cx),
            C::Down => self.move_folder(id, 1, cx),
            C::Other => self.choose_other(id, cx),
            C::Locate => self.locate(f.origin, f.path.clone(), cx),
            C::Copy => cx.write_to_clipboard(ClipboardItem::new_string(f.path.clone())),
            C::Show => cx.reveal_path(Path::new(&f.path)),
            C::Remove(_) => self.remove_folder(id, cx),
        }
        cx.notify();
    }

    /// `Esc` on the page: folds the newest folder menu or form.
    pub(super) fn folders_escape(&mut self, cx: &mut Context<Self>) -> bool {
        let closed = self.folder_ui.close_one();
        if closed {
            cx.notify();
        }
        closed
    }

    // ---- reading the disk and git ----

    /// Reads what the disk says about the page's folders, and their git
    /// state when the workspace's git epoch moved (or a folder is new),
    /// off the UI thread; until it arrives the row renders without it.
    pub(super) fn read_git(&mut self, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let items = data.folders.items.clone();
        let missing = data.folders.missing.clone();
        let epoch = self.ws.read(cx).git_epoch;
        let force = self.folder_ui.git_seen != Some(epoch);
        let known: HashSet<String> = self.folder_ui.status.keys().cloned().collect();
        let task = cx.background_executor().spawn(async move {
            let disk = fm::read_disk(&items);
            let mut status = HashMap::new();
            for f in &items {
                let in_git = disk.get(&f.path).is_some_and(|d| d.git.is_some());
                if !in_git || missing.contains(&f.path) {
                    status.insert(f.path.clone(), None);
                } else if force || !known.contains(&f.path) {
                    status.insert(f.path.clone(), git_cmd::status_summary(Path::new(&f.path)));
                }
            }
            (disk, status)
        });
        self.folder_ui._git = Some(cx.spawn(async move |page, cx| {
            let (disk, status) = task.await;
            let _ = page.update(cx, |page, cx| {
                let ui = &mut page.folder_ui;
                ui.disk = disk;
                for (path, s) in status {
                    match s {
                        Some(s) => {
                            ui.status.insert(path, s);
                        }
                        None => {
                            ui.status.remove(&path);
                        }
                    }
                }
                if force {
                    ui.git_seen = Some(epoch);
                }
                cx.notify();
            });
        }));
    }

    /// The git state of one folder read again (after a git action).
    fn read_status(&mut self, path: String, cx: &mut Context<Self>) {
        let p = path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { git_cmd::status_summary(Path::new(&p)) });
        cx.spawn(async move |page, cx| {
            let status = task.await;
            let _ = page.update(cx, |page, cx| {
                match status {
                    Some(s) => {
                        page.folder_ui.status.insert(path, s);
                    }
                    None => {
                        page.folder_ui.status.remove(&path);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The git identity and the settings file of a folder, for its form.
    fn read_form_facts(&mut self, path: String, cx: &mut Context<Self>) {
        let p = PathBuf::from(&path);
        let recorded = self
            .ws
            .read(cx)
            .db
            .run(cx, move |store| claude_settings::recorded(store, &p));
        cx.spawn(async move |page, cx| {
            let recorded = recorded.await.unwrap_or_default();
            let p = PathBuf::from(&path);
            let facts = cx
                .background_executor()
                .spawn(async move {
                    (
                        git_cmd::user_name(&p),
                        claude_settings::file_state(&p, &recorded),
                    )
                })
                .await;
            let _ = page.update(cx, |page, cx| {
                page.folder_ui.identity.insert(path.clone(), facts.0);
                page.folder_ui.files.insert(path, facts.1);
                cx.notify();
            });
        })
        .detach();
    }

    // ---- writes ----

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

    /// Attach `path` to `owner`, and make it the page's active folder when
    /// `active`; undoable as one step. A folder already in `owner`'s list
    /// adds nothing (the list unfolds on it).
    fn attach(&mut self, owner: Owner, path: PathBuf, active: bool, cx: &mut Context<Self>) {
        self.folder_ui.open = true;
        let project = self.project_id();
        let page_owner = self.owner;
        let ws = self.ws.clone();
        let resolved = cx
            .background_executor()
            .spawn(async move { folder_path::canonical(&path).map(|stored| (path, stored)) });
        cx.spawn(async move |_, cx| {
            let Ok((path, stored)) = resolved.await else {
                ws.update(cx, |ws, cx| ws.toast_error(Key::ErrorSave, cx));
                return;
            };
            ws.update(cx, |ws, cx| {
                ws.write_folders(cx, project, None, move |store| {
                    let existing = store
                        .folders(owner)?
                        .into_iter()
                        .find(|f| folder_path::same(Os::CURRENT, &f.path, &stored));
                    let mut steps = Vec::new();
                    let id = match existing {
                        Some(f) => f.id,
                        None => {
                            let id = store.add_folder(owner, &path, Author::Human)?;
                            steps.push(Undo::Delete(Record::Folder(id)));
                            id
                        }
                    };
                    if active {
                        let back = crate::workspace::active_pointer(store, page_owner)?;
                        if back != Some(id) {
                            store.set_active_folder(page_owner, Some(id), Author::Human)?;
                            // Taken back first: the pointer, then the folder.
                            steps.insert(
                                0,
                                Undo::ActiveFolder {
                                    owner: page_owner,
                                    back,
                                    expect: Some(id),
                                },
                            );
                        }
                    }
                    Ok((Undo::Many(steps), ()))
                })
                .detach();
            });
        })
        .detach();
    }

    /// "Add folder...": appended to this page's own list; the default rule
    /// makes a first own folder active.
    pub fn add_folder(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner;
        self.pick_folder(cx, move |page, path, cx| {
            page.attach(owner, path, false, cx)
        });
    }

    /// "Set working folder...": adds the folder to the open page and makes
    /// it active there; with no folder anywhere up the chain, attaches it
    /// to the project (components-folders.md 5).
    pub fn set_folder(&mut self, cx: &mut Context<Self>) {
        let Some(data) = &self.data else { return };
        let nothing_anywhere = data.folders.items.is_empty();
        let (target, active) = if nothing_anywhere {
            (Owner::Project(data.project.0), false)
        } else {
            (self.owner, true)
        };
        self.pick_folder(cx, move |page, path, cx| {
            page.attach(target, path, active, cx)
        });
    }

    /// "Locate...": repair the folder at `path` owned by `from`, in place,
    /// through the workspace's shared Locate (undo and bulk offer).
    pub(super) fn locate(&mut self, from: Owner, path: String, cx: &mut Context<Self>) {
        let project = self.project_id();
        let toast = (from != self.owner).then(|| {
            let locale = settings::locale(cx);
            let folder = self
                .folders()
                .and_then(|f| f.items.iter().find(|i| i.path == path))
                .map(|i| i.name.clone())
                .unwrap_or_else(|| paths_view::last_segment(&path).to_owned());
            let level = self
                .folders()
                .map(|f| f.level_name(from).to_owned())
                .unwrap_or_default();
            tr(
                locale,
                Key::FoldersLocatedFor,
                &[("folder", Arg::Text(&folder)), ("name", Arg::Text(&level))],
            )
        });
        self.pick_folder(cx, move |page, chosen, cx| {
            let task = page.ws.update(cx, |ws, cx| {
                ws.locate(FolderRef::At(from, path), chosen, project, cx)
            });
            let ws = page.ws.clone();
            cx.spawn(async move |_, cx| {
                if task.await
                    && let Some(message) = toast
                {
                    ws.update(cx, |ws, cx| ws.toast(message, ToastKind::Undo, cx));
                }
            })
            .detach();
        });
    }

    /// "Choose another folder...": the same repair, by id.
    fn choose_other(&mut self, id: FolderId, cx: &mut Context<Self>) {
        let project = self.project_id();
        self.pick_folder(cx, move |page, chosen, cx| {
            page.ws.update(cx, |ws, cx| {
                ws.locate(FolderRef::Id(id), chosen, project, cx).detach()
            });
        });
    }

    /// "Remove from this topic": detaches the folder and takes out the
    /// Claude Code settings Acta wrote there; the toast's Undo puts both
    /// back.
    fn remove_folder(&mut self, id: FolderId, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id) else {
            return;
        };
        let locale = settings::locale(cx);
        let key = match self.owner {
            Owner::Topic(_) => Key::FoldersRemovedTopic,
            Owner::Epic(_) => Key::FoldersRemovedEpic,
            Owner::Project(_) => Key::FoldersRemovedProject,
        };
        let message = tr(locale, key, &[("folder", Arg::Text(&folder.name))]);
        let record = Record::Folder(id);
        let project = self.project_id();
        self.folder_ui.row_menu = None;
        if self.folder_ui.settings == Some(id) {
            self.folder_ui.settings = None;
        }
        self.ws.update(cx, |ws, cx| {
            ws.write_folders(cx, project, Some(message), move |store| {
                store.delete(record, Author::Human)?;
                Ok((Undo::Restore(record), ()))
            })
            .detach();
        });
    }

    /// Makes a folder the page's active folder (ADR 0011): one undoable
    /// write, the folder's position unchanged. The active one: nothing.
    pub fn set_active(&mut self, id: FolderId, cx: &mut Context<Self>) {
        self.folder_ui.switcher = None;
        self.folder_ui.row_menu = None;
        cx.notify();
        if self.folder(id).is_some_and(|f| f.active) {
            return;
        }
        let owner = self.owner;
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let back = crate::workspace::active_pointer(store, owner)?;
                store.set_active_folder(owner, Some(id), Author::Human)?;
                Ok((
                    Undo::ActiveFolder {
                        owner,
                        back,
                        expect: Some(id),
                    },
                    (),
                ))
            });
        });
    }

    /// "Move up" / "Move down" in the page's own list. The order is the
    /// order of the switcher and of `--add-dir`; it never changes which
    /// folder is active.
    fn move_folder(&mut self, id: FolderId, delta: i64, cx: &mut Context<Self>) {
        self.folder_ui.row_menu = None;
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let record = Record::Folder(id);
                let back = store.order_index(record)?;
                store.move_folder(id, delta, Author::Human)?;
                Ok((
                    Undo::Reorder {
                        record,
                        back,
                        expect: store.order_index(record)?,
                    },
                    (),
                ))
            });
        });
    }

    fn set_name(&mut self, id: FolderId, name: String, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id) else {
            return;
        };
        let name = name.trim().to_owned();
        if name == folder.own_name {
            return;
        }
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let back = store.folder(id)?.name;
                store.set_folder_name(id, &name, Author::Human)?;
                Ok((
                    Undo::FolderName {
                        id,
                        back,
                        expect: store.folder(id)?.name,
                    },
                    (),
                ))
            });
        });
    }

    fn set_icon(&mut self, id: FolderId, icon: &'static str, cx: &mut Context<Self>) {
        if self.folder(id).is_some_and(|f| f.icon == icon) {
            return;
        }
        let project = self.project_id();
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, project, None, move |store| {
                let back = store.folder(id)?.icon;
                store.set_folder_icon(id, icon, Author::Human)?;
                Ok((
                    Undo::FolderIcon {
                        id,
                        back,
                        expect: icon.to_owned(),
                    },
                    (),
                ))
            });
        });
    }

    /// A commit mode or one of its options: the columns, then the folder's
    /// Claude Code settings (ADR 0011, 3); one undoable step.
    pub(super) fn set_commit(
        &mut self,
        id: FolderId,
        mode: CommitMode,
        as_user: Option<bool>,
        on_default: bool,
        cx: &mut Context<Self>,
    ) {
        let project = self.project_id();
        let path = self.folder(id).map(|f| f.path);
        let task = self.ws.update(cx, |ws, cx| {
            ws.write_folders(cx, project, None, move |store| {
                let f = store.folder(id)?;
                let back = (f.commit_mode, f.commit_as_user, f.commit_on_default);
                store.set_commit_settings(id, mode, as_user, on_default, Author::Human)?;
                Ok((
                    Undo::FolderCommit {
                        id,
                        back,
                        expect: (mode, as_user, on_default),
                    },
                    (),
                ))
            })
        });
        cx.spawn(async move |page, cx| {
            if task.await.is_ok()
                && let Some(path) = path
            {
                let _ = page.update(cx, |page, cx| page.read_form_facts(path, cx));
            }
        })
        .detach();
    }

    /// "For this page" on a folder the page sees (ADR 0011 addendum
    /// 2026-11-23): the page's own choice, `None` to use what it inherits.
    /// One undoable step; no settings file changes (sessions started from
    /// this page get the rules with their own launch).
    pub(super) fn set_page_commit(
        &mut self,
        id: FolderId,
        value: Option<PageCommit>,
        cx: &mut Context<Self>,
    ) {
        let project = self.project_id();
        let owner = self.owner;
        let task = self.ws.update(cx, |ws, cx| {
            ws.write_folders(cx, project, None, move |store| {
                let back = page_commit::page_commit(store, owner, id)?;
                page_commit::set_page_commit(store, owner, id, value, Author::Human)?;
                Ok((
                    Undo::PageCommit {
                        owner,
                        folder: id,
                        back,
                        expect: value,
                    },
                    (),
                ))
            })
        });
        task.detach();
    }

    /// "Commits for this page..." on an inherited folder: the page's own
    /// commit choice unfolds under the row (the rest is edited at the
    /// folder's own level).
    pub fn open_page_commit(&mut self, id: FolderId, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id).filter(|f| !f.own) else {
            return;
        };
        // Commit modes exist only for a folder in a git repository.
        if !self
            .folder_ui
            .disk
            .get(&folder.path)
            .is_some_and(|d| d.git.is_some())
        {
            return;
        }
        self.folder_ui.open = true;
        self.folder_ui.row_menu = None;
        self.folder_ui.switcher = None;
        self.folder_ui.settings = Some(id);
        self.folder_ui.name_input = None;
        self.folder_ui._name_sub = None;
        self.read_form_facts(folder.path, cx);
        cx.notify();
    }

    /// "Folder settings..." (own folders only): the form unfolds under the
    /// row, the name field focused with its text selected.
    pub fn open_folder_settings(
        &mut self,
        id: FolderId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(folder) = self.folder(id).filter(|f| f.own) else {
            return;
        };
        self.folder_ui.open = true;
        self.folder_ui.row_menu = None;
        self.folder_ui.switcher = None;
        let placeholder = paths_view::last_segment(&folder.path).to_owned();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(folder.own_name.clone())
        });
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        let sub = cx.subscribe_in(
            &input,
            window,
            move |page, state, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let name = state.read(cx).value().to_string();
                    page.set_name(id, name, cx);
                    page.folder_ui.settings = None;
                    page.folder_ui.name_input = None;
                    window.focus(&page.focus, cx);
                    cx.notify();
                }
                InputEvent::Blur => {
                    let name = state.read(cx).value().to_string();
                    page.set_name(id, name, cx);
                }
                _ => {}
            },
        );
        self.folder_ui.settings = Some(id);
        self.folder_ui.name_input = Some(input);
        self.folder_ui._name_sub = Some(sub);
        self.read_form_facts(folder.path, cx);
        cx.notify();
    }

    // ---- git actions (ADR 0011 addendum: human actions, never Claude Code's) ----

    /// Runs `then` now, or after the confirmation when a Claude Code
    /// session started in Acta's terminal is working in the folder.
    fn guarded(
        &mut self,
        folder: &FolderItem,
        body: String,
        confirm: Key,
        then: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        if !session_working_in(&folder.path, cx) {
            then(self, cx);
            return;
        }
        let locale = settings::locale(cx);
        let page = cx.entity().downgrade();
        let then = std::rc::Rc::new(then);
        cx.emit(PageEvent::Confirm(Confirm {
            title: tr(
                locale,
                Key::FoldersGitBusyTitle,
                &[("folder", Arg::Text(&folder.name))],
            ),
            body,
            confirm: text(locale, confirm),
            on_confirm: OnPick::new(move |_, _, cx| {
                let then = then.clone();
                let _ = page.update(cx, |page, cx| then(page, cx));
            }),
        }));
    }

    /// Runs a git action in `folder` off the UI thread, then toasts its
    /// refusal or `done`'s message, and reads the git state again. A
    /// refusal names `named`, else the current branch.
    fn run_git<R: Send + 'static>(
        &mut self,
        folder: &FolderItem,
        named: Option<String>,
        action: impl FnOnce(&Path) -> Result<R, GitError> + Send + 'static,
        done: impl FnOnce(R, Locale) -> Option<String> + 'static,
        cx: &mut Context<Self>,
    ) {
        let path = folder.path.clone();
        if !self.folder_ui.busy.insert(path.clone()) {
            return;
        }
        let name = folder.name.clone();
        let branch = named.unwrap_or_else(|| {
            self.folder_ui
                .status
                .get(&path)
                .and_then(|s| s.branch.clone())
                .unwrap_or_default()
        });
        let p = path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { action(Path::new(&p)) });
        cx.notify();
        cx.spawn(async move |page, cx| {
            let result = task.await;
            let _ = page.update(cx, |page, cx| {
                page.folder_ui.busy.remove(&path);
                let locale = settings::locale(cx);
                let toast = match result {
                    Ok(value) => done(value, locale).map(|m| (m, ToastKind::Done)),
                    Err(err) => Some((
                        fm::git_error_text(locale, &err, &branch, &name),
                        ToastKind::Error,
                    )),
                };
                if let Some((message, kind)) = toast {
                    page.ws.update(cx, |ws, cx| ws.toast(message, kind, cx));
                }
                page.read_status(path, cx);
            });
        })
        .detach();
    }

    /// "Pull": `git pull --ff-only` on the current branch.
    pub fn pull(&mut self, id: FolderId, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id) else {
            return;
        };
        self.folder_ui.branch_menu = None;
        let locale = settings::locale(cx);
        let body = text(locale, Key::FoldersGitBusyPull);
        let f = folder.clone();
        self.guarded(
            &folder,
            body,
            Key::FoldersGitBusyConfirmPull,
            move |page, cx| {
                let branch = page
                    .folder_ui
                    .status
                    .get(&f.path)
                    .and_then(|s| s.branch.clone())
                    .unwrap_or_default();
                page.run_git(
                    &f,
                    None,
                    git_cmd::pull_ff_only,
                    move |pulled, locale| {
                        let b = Arg::Text(&branch);
                        Some(match pulled {
                            Pulled::UpToDate => {
                                tr(locale, Key::FoldersGitUpToDate, &[("branch", b)])
                            }
                            Pulled::FastForwarded(n) => tr(
                                locale,
                                Key::FoldersGitPulled,
                                &[("n", Arg::Count(n as u64)), ("branch", b)],
                            ),
                        })
                    },
                    cx,
                )
            },
            cx,
        );
    }

    /// "Fetch": run on demand only; the row then shows "behind N".
    pub fn fetch(&mut self, id: FolderId, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id) else {
            return;
        };
        self.folder_ui.branch_menu = None;
        self.run_git(&folder, None, git_cmd::fetch, |(), _| None, cx);
    }

    /// The branch selector: opens at once, the list follows.
    pub fn open_branch_menu(&mut self, id: FolderId, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id) else {
            return;
        };
        self.folder_ui.switcher = None;
        self.folder_ui.row_menu = None;
        self.folder_ui.branch_menu = Some(BranchMenu {
            folder: id,
            branches: None,
            new_branch: None,
            _sub: None,
        });
        cx.notify();
        let path = folder.path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { git_cmd::list_branches(Path::new(&path)) });
        cx.spawn(async move |page, cx| {
            let listed = task.await;
            let _ = page.update(cx, |page, cx| match listed {
                Ok(branches) => {
                    if let Some(menu) = page.folder_ui.branch_menu.as_mut()
                        && menu.folder == id
                    {
                        menu.branches = Some(branches);
                    }
                    cx.notify();
                }
                Err(err) => {
                    page.folder_ui.branch_menu = None;
                    let locale = settings::locale(cx);
                    let message = fm::git_error_text(locale, &err, "", &folder.name);
                    page.ws
                        .update(cx, |ws, cx| ws.toast(message, ToastKind::Error, cx));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn switch_branch(&mut self, id: FolderId, to: SwitchTo, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id) else {
            return;
        };
        self.folder_ui.branch_menu = None;
        cx.notify();
        let locale = settings::locale(cx);
        let name = match &to {
            SwitchTo::Local(b) | SwitchTo::Track(b) => b.clone(),
        };
        let body = tr(
            locale,
            Key::FoldersGitBusySwitch,
            &[("branch", Arg::Text(&name))],
        );
        let f = folder.clone();
        self.guarded(
            &folder,
            body,
            Key::FoldersGitBusyConfirmSwitch,
            move |page, cx| {
                let to = to.clone();
                page.run_git(
                    &f,
                    Some(name.clone()),
                    move |dir| git_cmd::switch_branch(dir, &to),
                    |(), _| None,
                    cx,
                );
            },
            cx,
        );
    }

    /// "New branch...": the menu turns into a name field.
    fn start_new_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.folder_ui.branch_menu.as_mut() else {
            return;
        };
        let id = menu.folder;
        let locale = settings::locale(cx);
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(text(locale, Key::FoldersGitNewBranchHint))
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        let sub = cx.subscribe_in(
            &input,
            window,
            move |page, state, event: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let name = state.read(cx).value().trim().to_owned();
                    page.create_branch(id, name, cx);
                }
            },
        );
        menu.new_branch = Some(input);
        menu._sub = Some(sub);
        cx.notify();
    }

    fn create_branch(&mut self, id: FolderId, name: String, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(id) else {
            return;
        };
        if name.is_empty() {
            return;
        }
        self.folder_ui.branch_menu = None;
        cx.notify();
        let locale = settings::locale(cx);
        let body = tr(
            locale,
            Key::FoldersGitBusySwitch,
            &[("branch", Arg::Text(&name))],
        );
        let f = folder.clone();
        self.guarded(
            &folder,
            body,
            Key::FoldersGitBusyConfirmSwitch,
            move |page, cx| {
                let name = name.clone();
                page.run_git(
                    &f,
                    Some(name.clone()),
                    move |dir| git_cmd::create_branch(dir, &name),
                    |(), _| None,
                    cx,
                );
            },
            cx,
        );
    }

    /// Development captures (`ACTA_DEV_OVERLAY=folders-*`): the list
    /// unfolded, the switcher open, the active folder's settings form, or
    /// its branch selector.
    #[cfg(debug_assertions)]
    pub fn dev_folders(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_ui.open = which != "folders-switcher";
        match which {
            "folders-switcher" => self.folder_ui.switcher = Some(0),
            "folders-form" => {
                if let Some(id) = self.settings_target() {
                    self.open_folder_settings(id, window, cx)
                }
            }
            "folders-branch" => {
                if let Some(id) = self.git_target() {
                    self.open_branch_menu(id, cx)
                }
            }
            // A running session's page changes under it (layout.md 4.5).
            "terminal-switch" => {
                self.folder_ui.open = false;
                let next = self
                    .folders()
                    .and_then(|f| f.items.iter().find(|i| i.own && !i.active))
                    .map(|i| i.id);
                if let Some(id) = next {
                    self.set_active(id, cx)
                }
            }
            "terminal-commit" => {
                self.folder_ui.open = false;
                if let Some(f) = self.commit_target() {
                    self.set_commit(f.id, CommitMode::Commit, f.commit_as_user, false, cx)
                }
            }
            _ => {}
        }
        cx.notify();
    }

    // ---- palette ----

    /// "Switch active folder...": the palette lists the folders the page
    /// sees, each with its origin as its path and "Active" on the current
    /// one; the chosen one becomes active.
    pub fn pick_active_folder(&mut self, cx: &mut Context<Self>) {
        let Some(folders) = self.folders() else {
            return;
        };
        let locale = settings::locale(cx);
        let items: Vec<FolderItem> = folders
            .items
            .iter()
            .filter(|f| !folders.is_missing(&f.path))
            .cloned()
            .collect();
        if items.is_empty() {
            return;
        }
        let choices = items
            .iter()
            .map(|f| {
                let origin =
                    fm::group_label(locale, self.owner, f.origin, folders.level_name(f.origin));
                let detail = if f.active {
                    format!("{origin} \u{b7} {}", text(locale, Key::FoldersActive))
                } else {
                    origin
                };
                let disk = self.folder_ui.disk.get(&f.path);
                crate::palette::Choice {
                    title: f.name.clone(),
                    detail,
                    lead: crate::palette::Lead::Icon(fm::shown_icon(
                        &f.icon,
                        disk.and_then(|d| d.git.as_ref()),
                        false,
                    )),
                    trailing: String::new(),
                    refusal: None,
                }
            })
            .collect();
        let ids: Vec<FolderId> = items.iter().map(|f| f.id).collect();
        let page = cx.entity().downgrade();
        let on_pick = OnPick::new(move |i, _, cx| {
            if let Some(id) = ids.get(i).copied() {
                let _ = page.update(cx, |page, cx| page.set_active(id, cx));
            }
        });
        cx.emit(PageEvent::Pick(
            crate::palette::Picker {
                label: text(locale, Key::CmdFolderSwitch),
                choices,
            },
            on_pick,
        ));
    }

    /// The folder commands the palette offers on this page.
    pub fn folder_commands(&self) -> crate::commands::FolderCommands {
        let switch = self
            .folders()
            .is_some_and(|f| f.items.iter().any(|i| !f.is_missing(&i.path)));
        crate::commands::FolderCommands {
            switch,
            commit: self.commit_target().is_some(),
            settings: self.settings_target().is_some(),
            git: self.git_target().is_some(),
        }
    }

    /// The page's active folder when it is the page's own and in git:
    /// what "Set commit mode..." acts on.
    pub fn commit_target(&self) -> Option<FolderItem> {
        let folders = self.folders()?;
        folders
            .active()
            .filter(|f| f.own && !folders.is_missing(&f.path))
            .filter(|f| {
                self.folder_ui
                    .disk
                    .get(&f.path)
                    .is_some_and(|d| d.git.is_some())
            })
            .cloned()
    }

    /// "Set commit mode...": Manual, Commit automatically, Commit and push
    /// automatically, for the page's active folder.
    pub fn pick_commit_mode(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = self.commit_target() else {
            return;
        };
        let locale = settings::locale(cx);
        let modes = [
            (CommitMode::Manual, Key::CmdCommitModeManual),
            (CommitMode::Commit, Key::CmdCommitModeCommit),
            (CommitMode::Push, Key::CmdCommitModePush),
        ];
        let choices = modes
            .iter()
            .map(|(mode, key)| crate::palette::Choice {
                title: text(locale, *key),
                detail: if *mode == folder.commit_mode {
                    format!(
                        "{} \u{b7} {}",
                        folder.name,
                        text(locale, Key::FoldersActive)
                    )
                } else {
                    folder.name.clone()
                },
                lead: crate::palette::Lead::Icon(Icon::GitCommitHorizontal),
                trailing: String::new(),
                refusal: None,
            })
            .collect();
        let page = cx.entity().downgrade();
        let on_pick = OnPick::new(move |i, _, cx| {
            if let Some((mode, _)) = modes.get(i).copied() {
                let f = folder.clone();
                let _ = page.update(cx, |page, cx| {
                    page.set_commit(f.id, mode, f.commit_as_user, f.commit_on_default, cx)
                });
            }
        });
        cx.emit(PageEvent::Pick(
            crate::palette::Picker {
                label: text(locale, Key::CmdCommitMode),
                choices,
            },
            on_pick,
        ));
    }

    /// The active folder, when it is in git: what "Pull", "Fetch" and
    /// "Switch branch..." from the palette act on.
    pub fn git_target(&self) -> Option<FolderId> {
        let folders = self.folders()?;
        let active = folders.active()?;
        (!folders.is_missing(&active.path)
            && self
                .folder_ui
                .disk
                .get(&active.path)
                .is_some_and(|d| d.git.is_some()))
        .then_some(active.id)
    }

    /// "Folder settings..." from the palette: the active folder's, when it
    /// is the page's own.
    pub fn settings_target(&self) -> Option<FolderId> {
        self.folders()?.active().filter(|f| f.own).map(|f| f.id)
    }

    // ---- rendering ----

    fn switch_groups(&self, folders: &Folders, locale: Locale) -> Vec<SwitchGroup> {
        fm::groups(self.owner, &folders.items)
            .into_iter()
            .filter(|(_, list)| !list.is_empty())
            .map(|(origin, list)| SwitchGroup {
                label: fm::group_label(locale, self.owner, origin, folders.level_name(origin)),
                entries: list
                    .into_iter()
                    .map(|i| {
                        let f = &folders.items[i];
                        let missing = folders.is_missing(&f.path);
                        let disk = self.folder_ui.disk.get(&f.path);
                        SwitchEntry {
                            id: f.id,
                            icon: fm::shown_icon(
                                &f.icon,
                                disk.and_then(|d| d.git.as_ref()),
                                missing,
                            ),
                            name: f.name.clone(),
                            active: f.active,
                            missing,
                        }
                    })
                    .collect(),
            })
            .collect()
    }

    fn git_line_of(&self, f: &FolderItem, folders: &Folders) -> Option<GitLine> {
        if folders.is_missing(&f.path) {
            return None;
        }
        fm::git_line(
            self.folder_ui.disk.get(&f.path),
            self.folder_ui.status.get(&f.path),
            f.effective,
        )
    }

    /// The git state with the branch as the branch selector's button.
    fn render_git(
        &self,
        id: &str,
        f: &FolderItem,
        line: &GitLine,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let folder = f.id;
        let hover = t.fill_hover;
        let label = fm::branch_text(locale, line);
        let open = self
            .folder_ui
            .branch_menu
            .as_ref()
            .is_some_and(|m| m.folder == folder);
        let branch = div()
            .relative()
            .min_w_0()
            .child(
                ui::focusable(
                    div().id(SharedString::from(format!("{id}-branch"))),
                    t,
                    false,
                )
                .flex()
                .items_center()
                .h(px(20.))
                .px(px(4.))
                .min_w_0()
                .rounded(px(RADIUS_SM))
                .hover(move |s| s.bg(hover))
                .when(line.detached, |this| {
                    this.font_family(type_scale::CODE.family())
                })
                .child(div().min_w_0().truncate().child(label))
                .when(!open, |this| {
                    this.tooltip(ui::tip(text(locale, Key::FoldersGitBranchTip)))
                })
                .on_click(cx.listener(move |page, _, _, cx| {
                    cx.stop_propagation();
                    if page
                        .folder_ui
                        .branch_menu
                        .as_ref()
                        .is_some_and(|m| m.folder == folder)
                    {
                        page.folder_ui.branch_menu = None;
                        cx.notify();
                    } else {
                        page.open_branch_menu(folder, cx);
                    }
                })),
            )
            .when(open, |this| {
                this.child(self.folder_ui.branch_anchor.measure_keyed(folder.0 as u64))
                    .child(self.render_branch_menu(f, line, locale, t, cx))
            })
            .into_any_element();
        folders_view::git_state(
            SharedString::from(format!("{id}-git")),
            line,
            branch,
            locale,
            t,
        )
        .into_any_element()
    }

    fn render_branch_menu(
        &self,
        f: &FolderItem,
        line: &GitLine,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(menu_state) = &self.folder_ui.branch_menu else {
            return div().into_any_element();
        };
        let folder = f.id;
        let busy = self.folder_ui.busy.contains(&f.path);
        let mut panel =
            menu::panel("branch-menu", t).on_mouse_down_out(cx.listener(|page, _, _, cx| {
                page.folder_ui.branch_menu = None;
                cx.notify();
            }));
        if let Some(input) = &menu_state.new_branch {
            let input = input.clone();
            panel = panel
                .child(
                    type_scale::SMALL
                        .el()
                        .px(px(8.))
                        .py(px(4.))
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::FoldersGitNewBranchHint)),
                )
                .child(div().px(px(4.)).child(Input::new(&input)))
                .child(
                    div().flex().flex_row().justify_end().p(px(4.)).child(
                        ui::button(
                            "branch-create",
                            text(locale, Key::FoldersGitCreate),
                            Variant::Primary,
                            false,
                            t,
                        )
                        .on_click(cx.listener(move |page, _, _, cx| {
                            let name = input.read(cx).value().trim().to_owned();
                            page.create_branch(folder, name, cx);
                        })),
                    ),
                );
            return self
                .folder_ui
                .branch_anchor
                .below_keyed(folder.0 as u64, panel, menu::Align::Start, 1)
                .unwrap_or_else(|| div().into_any_element());
        }
        if line.upstream {
            let hint = (line.behind > 0).then(|| {
                menu::hint(
                    tr(
                        locale,
                        Key::FoldersGitBehind,
                        &[("n", Arg::Count(line.behind as u64))],
                    ),
                    t,
                )
            });
            let pull = if busy {
                menu::disabled_item("branch-pull", text(locale, Key::FoldersGitPull), hint, t)
            } else {
                menu::item(
                    "branch-pull",
                    None,
                    text(locale, Key::FoldersGitPull),
                    hint,
                    false,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| page.pull(folder, cx)))
            };
            let fetch = if busy {
                menu::disabled_item("branch-fetch", text(locale, Key::FoldersGitFetch), None, t)
            } else {
                menu::item(
                    "branch-fetch",
                    None,
                    text(locale, Key::FoldersGitFetch),
                    None,
                    false,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| page.fetch(folder, cx)))
            };
            panel = panel.child(pull).child(fetch).child(menu::separator(t));
        }
        if let Some(branches) = &menu_state.branches {
            let label = |key: Key| {
                type_scale::SMALL
                    .el()
                    .flex()
                    .items_center()
                    .h(px(24.))
                    .px(px(8.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, key))
            };
            panel = panel.child(label(Key::FoldersGitLocal));
            for (i, b) in branches.local.iter().enumerate() {
                let current = branches.current.as_deref() == Some(b.as_str());
                let to = SwitchTo::Local(b.clone());
                panel = panel.child(
                    menu::item(
                        SharedString::from(format!("branch-local-{i}")),
                        None,
                        b.clone(),
                        current.then(|| menu::check(t)),
                        false,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| {
                        if current {
                            page.folder_ui.branch_menu = None;
                            cx.notify();
                        } else {
                            page.switch_branch(folder, to.clone(), cx)
                        }
                    })),
                );
            }
            if !branches.remote.is_empty() {
                panel = panel.child(label(Key::FoldersGitRemote));
                for (i, b) in branches.remote.iter().enumerate() {
                    let to = SwitchTo::Track(b.clone());
                    panel = panel.child(
                        menu::item(
                            SharedString::from(format!("branch-remote-{i}")),
                            None,
                            b.clone(),
                            Some(menu::hint(text(locale, Key::FoldersGitTrack), t)),
                            false,
                            t,
                        )
                        .on_click(cx.listener(move |page, _, _, cx| {
                            page.switch_branch(folder, to.clone(), cx)
                        })),
                    );
                }
            }
            panel = panel.child(menu::separator(t)).child(
                menu::item(
                    "branch-new",
                    None,
                    text(locale, Key::FoldersGitNewBranch),
                    None,
                    false,
                    t,
                )
                .on_click(cx.listener(|page, _, window, cx| page.start_new_branch(window, cx))),
            );
        }
        self.folder_ui
            .branch_anchor
            .below_keyed(folder.0 as u64, panel, menu::Align::Start, 1)
            .unwrap_or_else(|| div().into_any_element())
    }

    pub(super) fn render_folders(
        &self,
        data: &Loaded,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let folders = &data.folders;
        let Some(active) = folders.active().cloned() else {
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
        let open = self.folder_ui.open;
        let active_missing = folders.is_missing(&active.path);
        let others = folders.items.len().saturating_sub(1);
        let active_disk = self.folder_ui.disk.get(&active.path);
        let glyph = fm::shown_icon(
            &active.icon,
            active_disk.and_then(|d| d.git.as_ref()),
            active_missing,
        );
        // The switcher, with its menu anchored under it.
        let switcher = {
            let menu_open = self.folder_ui.switcher;
            let mut tip = vec![active.path.clone()];
            if !active.own {
                tip.push(fm::group_label(
                    locale,
                    self.owner,
                    active.origin,
                    folders.level_name(active.origin),
                ));
            }
            if let Some(line) = self.git_line_of(&active, folders) {
                let mut git = fm::branch_text(locale, &line);
                if line.changed > 0 {
                    git.push_str(" \u{b7} ");
                    git.push_str(&tr(
                        locale,
                        Key::FoldersGitChanged,
                        &[("n", Arg::Count(line.changed as u64))],
                    ));
                }
                if line.behind > 0 {
                    git.push_str(" \u{b7} ");
                    git.push_str(&tr(
                        locale,
                        Key::FoldersGitBehind,
                        &[("n", Arg::Count(line.behind as u64))],
                    ));
                }
                tip.push(git);
                if let Some(key) = line.auto.and_then(fm::auto_tip) {
                    tip.push(text(locale, key));
                }
            }
            let tip = tip.join("\n");
            let button = folders_view::switcher(
                "folder-switcher",
                glyph,
                &active.name,
                active_missing,
                false,
                t,
            )
            .when(menu_open.is_none(), |this| this.tooltip(ui::tip(tip)))
            .on_click(cx.listener(|page, _, _, cx| {
                cx.stop_propagation();
                page.folder_ui.switcher = match page.folder_ui.switcher {
                    Some(_) => None,
                    None => Some(0),
                };
                page.folder_ui.row_menu = None;
                page.folder_ui.branch_menu = None;
                cx.notify();
            }));
            let mut wrap = div()
                .relative()
                .min_w(px(crate::terminal_dock::SWITCHER_MIN))
                .child(button.w_full().overflow_hidden())
                .child(self.folder_ui.switch_anchor.measure());
            if menu_open.is_some() {
                let groups = self.switch_groups(folders, locale);
                wrap = wrap.child(folders_view::switcher_menu(
                    "folder-switch-menu",
                    &self.folder_ui.switch_anchor,
                    &groups,
                    menu_open,
                    locale,
                    t,
                    cx,
                    SwitchActions {
                        choose: std::rc::Rc::new(|page: &mut Self, id, _, cx| {
                            page.set_active(id, cx)
                        }),
                        edit: std::rc::Rc::new(|page: &mut Self, _, cx| {
                            page.folder_ui.switcher = None;
                            page.folder_ui.open = true;
                            cx.notify();
                        }),
                        add: std::rc::Rc::new(|page: &mut Self, _, cx| {
                            page.folder_ui.switcher = None;
                            page.add_folder(cx);
                        }),
                        close: std::rc::Rc::new(|page: &mut Self, _, cx| {
                            page.folder_ui.switcher = None;
                            cx.notify();
                        }),
                    },
                ));
            }
            wrap
        };
        let count = (others > 0).then(|| folders_view::count(locale, others, t));
        // Under 360 of column the not-found words drop; `folder-x` (in the
        // switcher) and Locate remain.
        let words = ui::column_width() >= 360.;
        let state = active_missing.then(|| {
            let from = active.origin;
            let path = active.path.clone();
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .flex_shrink_0()
                .when(words, |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .min_w_0()
                            .truncate()
                            .text_color(t.text_secondary)
                            .child(text(locale, Key::FoldersNotFound)),
                    )
                })
                .child(
                    ui::button(
                        "folder-locate",
                        text(locale, Key::FoldersLocate),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .h(px(24.))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        cx.stop_propagation();
                        page.locate(from, path.clone(), cx)
                    })),
                )
        });
        let chevron = icon(
            if open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            16.,
            t.text_secondary,
        );
        let header = self
            .block_el(record::Block::Folders, div().id("folder-row"), t)
            .px(px(8.))
            .ml(px(-8.))
            .w_full()
            .rounded(px(RADIUS_SM))
            .hover({
                let hover = t.fill_hover;
                move |s| s.bg(hover)
            })
            .on_click(cx.listener(|page, _, _, cx| {
                page.folder_ui.open = !page.folder_ui.open;
                cx.notify();
            }));
        let header = if open {
            // Unfolded, the row is the list's header: chevron, switcher,
            // "Working folders" and the count of folders seen.
            header
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .child(chevron)
                .child(switcher)
                .child(
                    type_scale::LABEL
                        .el()
                        .ml(px(4.))
                        .flex_shrink_0()
                        .text_color(t.text_secondary)
                        .child(text(locale, Key::FoldersTitle)),
                )
                .child(
                    type_scale::SMALL
                        .el()
                        .flex_shrink_0()
                        .text_color(t.text_tertiary)
                        .font_features(type_scale::tabular())
                        .child(number(locale, folders.items.len() as u64)),
                )
        } else {
            // At rest: the switcher and the count, one line at every width
            // (clarity review 2026-11-22); the rest is the switcher's tooltip.
            header
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .min_w_0()
                .child(chevron)
                .child(switcher)
                .children(count)
                .children(state.map(|s| s.ml(px(4.))))
        };
        if !open {
            return header.into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(header)
            .child(self.render_folder_list(folders, locale, t, cx))
            .into_any_element()
    }

    fn render_folder_list(
        &self,
        folders: &Folders,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut list = div().flex().flex_col().pl(px(24.));
        let mut order = Vec::new();
        let active_name = folders.active().map(|f| f.name.clone()).unwrap_or_default();
        for (origin, items) in fm::groups(self.owner, &folders.items) {
            list = list.child(
                type_scale::SMALL
                    .el()
                    .mt(px(8.))
                    .mb(px(2.))
                    .text_color(t.text_tertiary)
                    .child(fm::group_label(
                        locale,
                        self.owner,
                        origin,
                        folders.level_name(origin),
                    )),
            );
            if items.is_empty() {
                list = list.child(
                    type_scale::SMALL
                        .el()
                        .pl(px(32.))
                        .h(px(24.))
                        .text_color(t.text_tertiary)
                        .child(text(locale, Key::FoldersNoOwn)),
                );
            }
            for i in items {
                let f = folders.items[i].clone();
                order.push(f.id);
                list = list.child(self.render_folder_row(&f, folders, &active_name, locale, t, cx));
                if self.folder_ui.settings == Some(f.id) {
                    list = list.child(if f.own {
                        self.render_folder_form(&f, folders, locale, t, cx)
                    } else {
                        self.render_page_commit_form(&f, folders, locale, t, cx)
                    });
                }
            }
        }
        *self.folder_ui.row_order.borrow_mut() = order;
        list = list
            .child(
                div().pt(px(4.)).flex().child(
                    ui::button(
                        "folder-add",
                        text(locale, Key::FoldersAdd),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .ml(px(-12.))
                    .child(icon(Icon::Plus, 16., t.text_secondary))
                    .flex_row_reverse()
                    .on_click(cx.listener(|page, _, _, cx| page.add_folder(cx))),
                ),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .mt(px(8.))
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::FoldersHint)),
            );
        list.into_any_element()
    }

    fn render_folder_row(
        &self,
        f: &FolderItem,
        folders: &Folders,
        active_name: &str,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = f.id;
        let row_id = SharedString::from(format!("folder-{}", id.0));
        let missing = folders.is_missing(&f.path);
        let disk = self.folder_ui.disk.get(&f.path);
        let glyph = fm::shown_icon(&f.icon, disk.and_then(|d| d.git.as_ref()), missing);
        let git = self
            .git_line_of(f, folders)
            .map(|line| (self.render_git(&row_id, f, &line, locale, t, cx), line));
        let busy = self.folder_ui.busy.contains(&f.path);
        let home = paths_view::home();
        let shown = paths_view::shorten_home(&f.path, home.as_deref());
        let hover = t.fill_hover;
        let group = row_id.clone();
        // The active mark: `circle-dot` in `accent` on the active folder;
        // elsewhere `circle`, on hover and keyboard focus only.
        let mark = if f.active {
            div()
                .size(px(16.))
                .flex_shrink_0()
                .child(icon(Icon::CircleDot, 16., t.accent))
                .into_any_element()
        } else {
            ui::focusable(
                div().id(SharedString::from(format!("{row_id}-mark"))),
                t,
                false,
            )
            .size(px(16.))
            .flex_shrink_0()
            .rounded(px(8.))
            .invisible()
            .group_hover(group.clone(), |s| s.visible())
            .when(!missing, |this| {
                this.child(icon(Icon::Circle, 16., t.text_tertiary))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        cx.stop_propagation();
                        page.set_active(id, cx)
                    }))
            })
            .into_any_element()
        };
        let mut first = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .h(px(24.))
            .min_w_0()
            .child(mark)
            .child(icon(
                glyph,
                16.,
                if missing { t.warning } else { t.text_secondary },
            ))
            .child(
                type_scale::UI
                    .el()
                    .min_w(px(48.))
                    .truncate()
                    .text_color(if f.active && !missing {
                        t.text_primary
                    } else {
                        t.text_secondary
                    })
                    .child(f.name.clone()),
            );
        let mut pull = None;
        if let Some((git_el, line)) = git {
            first = first.child(div().min_w_0().child(git_el));
            if line.upstream && !line.detached {
                let label = text(locale, Key::FoldersGitPull);
                pull = Some(
                    ui::button(
                        SharedString::from(format!("{row_id}-pull")),
                        label,
                        Variant::Ghost,
                        busy,
                        t,
                    )
                    .h(px(24.))
                    .px(px(8.))
                    .when(!busy, |this| {
                        this.on_click(cx.listener(move |page, _, _, cx| {
                            cx.stop_propagation();
                            page.pull(id, cx)
                        }))
                    }),
                );
            }
        }
        first = first.child(div().flex_1());
        if missing {
            let (from, path) = (f.origin, f.path.clone());
            first = first
                .child(
                    type_scale::SMALL
                        .el()
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_secondary)
                        .child(text(locale, Key::FoldersNotFound)),
                )
                .child(
                    ui::button(
                        SharedString::from(format!("{row_id}-locate")),
                        text(locale, Key::FoldersLocate),
                        Variant::Ghost,
                        false,
                        t,
                    )
                    .h(px(24.))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        cx.stop_propagation();
                        page.locate(from, path.clone(), cx)
                    })),
                );
        }
        first = first.children(pull);
        if f.active {
            first = first.child(
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .text_color(t.text_tertiary)
                    .child(text(locale, Key::FoldersActive)),
            );
        }
        let menu_open = self.folder_ui.row_menu == Some(id);
        first = first.child(
            div()
                .relative()
                .flex_shrink_0()
                .child(
                    div()
                        .invisible()
                        .group_hover(group.clone(), |s| s.visible())
                        .when(menu_open, |this| this.visible())
                        .child(
                            ui::ghost_icon_button(
                                format!("{row_id}-more"),
                                Icon::Ellipsis,
                                24.,
                                menu_open,
                                false,
                                true,
                                t,
                            )
                            .on_click(cx.listener(
                                move |page, _, _, cx| {
                                    cx.stop_propagation();
                                    page.folder_ui.row_menu =
                                        (page.folder_ui.row_menu != Some(id)).then_some(id);
                                    page.folder_ui.switcher = None;
                                    page.folder_ui.branch_menu = None;
                                    cx.notify();
                                },
                            )),
                        ),
                )
                .when(menu_open, |this| {
                    this.child(self.folder_ui.row_anchor.measure_keyed(id.0 as u64))
                        .child(self.render_row_menu(f, folders, locale, t, cx))
                }),
        );
        let handle = self.folder_ui.row_focus.get(&id).cloned();
        let mut row = ui::focusable(div().id(row_id.clone()), t, false)
            .when_some(handle, |this, h| this.track_focus(&h))
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, window, cx| {
                page.on_folder_key(id, event, window, cx)
            }))
            .group(group)
            .flex()
            .flex_col()
            .justify_center()
            .min_h(px(52.))
            .py(px(6.))
            .px(px(8.))
            .ml(px(-8.))
            .rounded(px(RADIUS_SM))
            .hover(move |s| s.bg(hover))
            .child(first)
            .child(
                type_scale::CODE
                    .el()
                    .pl(px(48.))
                    .h(px(20.))
                    .min_w_0()
                    .truncate()
                    .text_color(t.text_tertiary)
                    .child(paths_view::fit(&shown, paths_view::budget(320.))),
            );
        if disk.is_some_and(|d| d.other_checkout) && !f.active {
            row = row.child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(6.))
                    .pl(px(48.))
                    .pt(px(2.))
                    .child(
                        div()
                            .pt(px(1.))
                            .child(icon(Icon::Info, 14., t.text_secondary)),
                    )
                    .child(
                        type_scale::SMALL
                            .el()
                            .flex_1()
                            .min_w_0()
                            .text_color(t.text_secondary)
                            .child(tr(
                                locale,
                                Key::FoldersOtherCheckout,
                                &[("name", Arg::Text(active_name))],
                            )),
                    ),
            );
        }
        if let Some(also) = folders.also.get(&f.id) {
            row = row.child(self.render_also(&row_id, also, locale, t, cx));
        }
        row.into_any_element()
    }

    /// "Also attached to topic Scrollback." under an own folder that
    /// another topic (or epic) of the project has too, the other's name a
    /// link; the tooltip says where a session started there outside Acta
    /// opens (components-folders.md 5, Same folder on two topics).
    fn render_also(
        &self,
        row_id: &SharedString,
        also: &crate::record::Also,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let route = |o: Owner| match o {
            Owner::Project(id) => Route::Project(id),
            Owner::Epic(id) => Route::Epic(id),
            Owner::Topic(id) => Route::Topic(id),
        };
        let level = crate::questions_page::owner_label(locale, also.level.0, &also.level.1);
        let tip = tr(locale, Key::FoldersAlsoTip, &[("level", Arg::Text(&level))]);
        let sentence: AnyElement = match also.others.as_slice() {
            [(owner, name)] => {
                let key = match owner {
                    Owner::Epic(_) => Key::FoldersAlsoEpic,
                    _ => Key::FoldersAlsoTopic,
                };
                let (before, after) = crate::question_view::around_name(locale, key, name);
                let to = route(*owner);
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .child(before)
                    .child(
                        crate::question_view::name_link(
                            SharedString::from(format!("{row_id}-also")),
                            name,
                            false,
                            locale,
                            t,
                        )
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(PageEvent::Open(to));
                        })),
                    )
                    .child(after)
                    .into_any_element()
            }
            // Two or more: each of the first two names a link carrying its
            // level word ("topic Scrollback"), then how many more.
            others => {
                const A: &str = "\u{1}";
                const B: &str = "\u{2}";
                let line = also_line(locale, others.len(), A, B);
                let mut parts = div().flex().flex_row().flex_wrap();
                let mut rest = line.as_str();
                for (i, mark) in [A, B].into_iter().enumerate() {
                    let Some((before, after)) = rest.split_once(mark) else {
                        continue;
                    };
                    let (owner, name) = &others[i];
                    let label = also_name(locale, *owner, name);
                    let to = route(*owner);
                    parts = parts.child(before.to_owned()).child(
                        crate::question_view::name_link(
                            SharedString::from(format!("{row_id}-also-{i}")),
                            &label,
                            false,
                            locale,
                            t,
                        )
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(PageEvent::Open(to));
                        })),
                    );
                    rest = after;
                }
                parts.child(rest.to_owned()).into_any_element()
            }
        };
        div()
            .id(SharedString::from(format!("{row_id}-also-line")))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(6.))
            .pl(px(48.))
            .pt(px(2.))
            .tooltip(ui::tip(tip))
            .child(
                div()
                    .pt(px(1.))
                    .child(icon(Icon::Info, 14., t.text_secondary)),
            )
            .child(
                type_scale::SMALL
                    .el()
                    .flex_1()
                    .min_w_0()
                    .text_color(t.text_secondary)
                    .child(sentence),
            )
            .into_any_element()
    }

    fn render_row_menu(
        &self,
        f: &FolderItem,
        folders: &Folders,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = f.id;
        let path = f.path.clone();
        let show_key = if cfg!(target_os = "macos") {
            Key::FoldersShowM
        } else if cfg!(windows) {
            Key::FoldersShowW
        } else {
            Key::FoldersShowL
        };
        let item =
            |n: &'static str, key: Key| menu::item(n, None, text(locale, key), None, false, t);
        let mut panel =
            menu::panel("folder-row-menu", t).on_mouse_down_out(cx.listener(|page, _, _, cx| {
                page.folder_ui.row_menu = None;
                cx.notify();
            }));
        if !f.active && !folders.is_missing(&f.path) {
            panel = panel.child(
                item("fm-active", Key::FoldersSetActive)
                    .on_click(cx.listener(move |page, _, _, cx| page.set_active(id, cx))),
            );
        }
        if f.own {
            panel = panel.child(item("fm-settings", Key::FoldersSettings).on_click(
                cx.listener(move |page, _, window, cx| page.open_folder_settings(id, window, cx)),
            ));
        }
        let reveal = path.clone();
        let copy = path.clone();
        panel = panel
            .child(
                item("fm-show", show_key).on_click(cx.listener(move |page, _, _, cx| {
                    page.folder_ui.row_menu = None;
                    cx.reveal_path(Path::new(&reveal));
                    cx.notify();
                })),
            )
            .child(item("fm-copy", Key::FoldersCopyPath).on_click(cx.listener(
                move |page, _, _, cx| {
                    page.folder_ui.row_menu = None;
                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                    cx.notify();
                },
            )));
        if f.own {
            let remove_key = match self.owner {
                Owner::Topic(_) => Key::FoldersRemoveTopic,
                Owner::Epic(_) => Key::FoldersRemoveEpic,
                Owner::Project(_) => Key::FoldersRemoveProject,
            };
            let own: Vec<FolderId> = folders
                .items
                .iter()
                .filter(|item| item.own)
                .map(|item| item.id)
                .collect();
            let (up, down) = move_options(&own, id);
            panel =
                panel
                    .when(up, |this| {
                        this.child(item("fm-up", Key::FoldersMoveUp).on_click(
                            cx.listener(move |page, _, _, cx| page.move_folder(id, -1, cx)),
                        ))
                    })
                    .when(down, |this| {
                        this.child(item("fm-down", Key::FoldersMoveDown).on_click(
                            cx.listener(move |page, _, _, cx| page.move_folder(id, 1, cx)),
                        ))
                    })
                    .child(
                        item("fm-other", Key::FoldersChooseOther).on_click(cx.listener(
                            move |page, _, _, cx| {
                                page.folder_ui.row_menu = None;
                                page.choose_other(id, cx)
                            },
                        )),
                    )
                    .child(menu::separator(t))
                    .child(
                        item("fm-remove", remove_key).on_click(
                            cx.listener(move |page, _, _, cx| page.remove_folder(id, cx)),
                        ),
                    );
        } else {
            let level = folders.level_name(f.origin).to_owned();
            let route = match f.origin {
                Owner::Project(id) => Route::Project(id),
                Owner::Epic(id) => Route::Epic(id),
                Owner::Topic(id) => Route::Topic(id),
            };
            let label = match f.origin {
                Owner::Project(_) => tr(
                    locale,
                    Key::FoldersGroupFromProject,
                    &[("name", Arg::Text(&level))],
                ),
                _ => tr(locale, Key::FoldersEditIn, &[("name", Arg::Text(&level))]),
            };
            let in_git = self
                .folder_ui
                .disk
                .get(&f.path)
                .is_some_and(|d| d.git.is_some());
            if in_git {
                panel = panel
                    .child(item("fm-for-page", Key::FoldersForPageMenu).on_click(
                        cx.listener(move |page, _, _, cx| page.open_page_commit(id, cx)),
                    ));
            }
            panel = panel.child(
                menu::item("fm-edit-in", None, label, None, false, t).on_click(cx.listener(
                    move |page, _, _, cx| {
                        page.folder_ui.row_menu = None;
                        cx.emit(PageEvent::Open(route));
                    },
                )),
            );
        }
        self.folder_ui
            .row_anchor
            .below_keyed(id.0 as u64, panel, menu::Align::End, 1)
            .unwrap_or_else(|| div().into_any_element())
    }

    /// The folder settings form, unfolded in place under an own folder.
    fn render_folder_form(
        &self,
        f: &FolderItem,
        folders: &Folders,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = f.id;
        let disk = self.folder_ui.disk.get(&f.path);
        let checkout = disk.and_then(|d| d.git.as_ref());
        let label = |key: Key| {
            type_scale::LABEL
                .el()
                .w(px(56.))
                .flex_shrink_0()
                .text_color(t.text_secondary)
                .child(text(locale, key))
        };
        let mut form = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .pl(px(48.))
            .pt(px(12.))
            .pb(px(8.))
            .on_mouse_down_out(cx.listener(move |page, _, _, cx| {
                if page.folder_ui.settings == Some(id) && page.folder_ui.branch_menu.is_none() {
                    if let Some(input) = page.folder_ui.name_input.clone() {
                        let name = input.read(cx).value().to_string();
                        page.set_name(id, name, cx);
                    }
                    page.folder_ui.settings = None;
                    page.folder_ui.name_input = None;
                    cx.notify();
                }
            }));
        if let Some(input) = &self.folder_ui.name_input {
            form = form.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(label(Key::FoldersName))
                    .child(div().w(px(240.)).child(Input::new(input))),
            );
        }
        // Icon: Automatic, then the twelve, 28 x 28 ghost toggles. Under
        // 520 they wrap to two lines of seven and six.
        let auto_glyph = fm::automatic_icon(checkout);
        let toggle = |n: String, glyph: Icon, selected: bool, tip: String| {
            let hover = t.fill_hover;
            ui::focusable(div().id(SharedString::from(n)), t, false)
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(RADIUS_SM))
                .when(selected, |this| {
                    this.bg(t.fill_selected)
                        .border(px(1.))
                        .border_color(t.border_strong)
                })
                .when(!selected, |this| this.hover(move |s| s.bg(hover)))
                .child(icon(
                    glyph,
                    16.,
                    if selected {
                        t.text_primary
                    } else {
                        t.text_secondary
                    },
                ))
                .tooltip(ui::tip(tip))
        };
        // Rows of equal length: 14 icons that do not fit on one line go
        // 7 and 7, never 12 and 2 (QA 2026-11-22). The room is the column
        // less the list's indent, the label and its gap.
        let label_w = ui::text_width(type_scale::SMALL, &text(locale, Key::FoldersIcon), cx);
        let room = ui::column_width() - 24. - label_w - 8. - 16.;
        let mut icons = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .flex_1()
            .min_w_0()
            .gap(px(ICON_GAP))
            .when_some(balanced_width(fm::ICONS.len() + 1, room), |this, w| {
                this.max_w(px(w))
            })
            .child(
                toggle(
                    format!("fi-auto-{}", id.0),
                    auto_glyph,
                    f.icon.is_empty(),
                    text(locale, Key::FoldersIconAutoHint),
                )
                .on_click(cx.listener(move |page, _, _, cx| page.set_icon(id, "", cx))),
            );
        for (name, glyph, key) in fm::ICONS {
            icons = icons.child(
                toggle(
                    format!("fi-{name}-{}", id.0),
                    glyph,
                    f.icon == name,
                    text(locale, key),
                )
                .on_click(cx.listener(move |page, _, _, cx| page.set_icon(id, name, cx))),
            );
        }
        form = form.child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(8.))
                .child(div().pt(px(6.)).child(label(Key::FoldersIcon)))
                .child(icons),
        );
        // Commits by Claude Code: only for a folder in git.
        if let Some(checkout) = checkout {
            let mode = f.commit_mode;
            let (as_user_set, on_default) = (f.commit_as_user, f.commit_on_default);
            let mut segments = ui::segmented(t);
            for (m, key) in [
                (CommitMode::Manual, Key::FoldersCommitManual),
                (CommitMode::Commit, Key::FoldersCommitCommit),
                (CommitMode::Push, Key::FoldersCommitPush),
            ] {
                segments = segments.child(
                    ui::segment(
                        SharedString::from(format!("fc-{key:?}")),
                        text(locale, key),
                        m == mode,
                        t,
                    )
                    .on_click(cx.listener(move |page, _, _, cx| {
                        if m != mode {
                            page.set_commit(id, m, as_user_set, on_default, cx)
                        }
                    })),
                );
            }
            form = form.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        type_scale::LABEL
                            .el()
                            .text_color(t.text_secondary)
                            .child(text(locale, Key::FoldersCommits)),
                    )
                    .child(segments)
                    .child(
                        type_scale::SMALL
                            .el()
                            .max_w(px(480.))
                            .text_color(t.text_tertiary)
                            .child(text(locale, fm::mode_hint(mode))),
                    ),
            );
            let files = self
                .folder_ui
                .files
                .get(&f.path)
                .copied()
                .unwrap_or_default();
            if mode.automatic() {
                let as_user = as_user_set.unwrap_or(folders.as_user_default);
                let check_row = |n: String, checked: bool, label: String| {
                    let hover = t.fill_hover;
                    ui::focusable(div().id(SharedString::from(n)), t, false)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .min_h(px(24.))
                        .px(px(4.))
                        .ml(px(-4.))
                        .rounded(px(RADIUS_SM))
                        .hover(move |s| s.bg(hover))
                        .child(ui::checkbox(checked, t))
                        .child(
                            type_scale::UI
                                .el()
                                .flex_1()
                                .min_w_0()
                                .text_color(t.text_primary)
                                .child(label),
                        )
                };
                let mut options = div().flex().flex_col().gap(px(4.)).child(
                    check_row(
                        format!("fc-as-user-{}", id.0),
                        as_user,
                        text(locale, Key::FoldersCommitAsUser),
                    )
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.set_commit(id, mode, Some(!as_user), on_default, cx)
                    })),
                );
                if let Some(default) = &checkout.default_branch {
                    options = options.child(
                        check_row(
                            format!("fc-on-default-{}", id.0),
                            on_default,
                            tr(
                                locale,
                                Key::FoldersCommitOnDefault,
                                &[("branch", Arg::Text(default))],
                            ),
                        )
                        .on_click(cx.listener(move |page, _, _, cx| {
                            page.set_commit(id, mode, as_user_set, !on_default, cx)
                        })),
                    );
                }
                let identity = self.folder_ui.identity.get(&f.path);
                let identity_line = match identity {
                    Some(Some(name)) => Some(
                        type_scale::SMALL
                            .el()
                            .text_color(t.text_tertiary)
                            .child(tr(
                                locale,
                                Key::FoldersCommitIdentity,
                                &[("name", Arg::Text(name))],
                            ))
                            .into_any_element(),
                    ),
                    Some(None) => Some(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(6.))
                            .child(icon(Icon::TriangleAlert, 14., t.warning))
                            .child(
                                type_scale::SMALL
                                    .el()
                                    .max_w(px(460.))
                                    .text_color(t.text_tertiary)
                                    .child(text(locale, Key::FoldersCommitNoIdentity)),
                            )
                            .into_any_element(),
                    ),
                    None => None,
                };
                options = options.children(identity_line);
                form = form.child(options);
                // The file line, or what replaces it when it applies.
                let line = if files.tracked {
                    text(locale, Key::FoldersCommitTracked)
                } else if as_user && files.root_attribution {
                    text(locale, Key::FoldersCommitRootWins)
                } else if as_user && files.user_attribution {
                    tr(
                        locale,
                        Key::FoldersCommitUserOwn,
                        &[("key", Arg::Text("attribution"))],
                    )
                } else if mode == CommitMode::Push && files.user_env {
                    tr(
                        locale,
                        Key::FoldersCommitUserOwn,
                        &[("key", Arg::Text("env"))],
                    )
                } else {
                    text(locale, Key::FoldersCommitFile)
                };
                form = form.child(
                    type_scale::SMALL
                        .el()
                        .max_w(px(480.))
                        .text_color(t.text_tertiary)
                        .child(line),
                );
            }
        }
        form.into_any_element()
    }
}

impl RecordPage {
    /// "For this page" (ADR 0011 addendum 2026-11-23), unfolded under an
    /// inherited git folder: Use folder setting / Manual / Commit / Commit
    /// and push, one line saying what applies, and with an automatic choice
    /// its two options.
    fn render_page_commit_form(
        &self,
        f: &FolderItem,
        folders: &Folders,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = f.id;
        let checkout = self
            .folder_ui
            .disk
            .get(&f.path)
            .and_then(|d| d.git.as_ref());
        let mode_name = |m: CommitMode| {
            text(
                locale,
                match m {
                    CommitMode::Manual => Key::FoldersCommitManual,
                    CommitMode::Commit => Key::FoldersCommitCommit,
                    CommitMode::Push => Key::FoldersCommitPush,
                },
            )
        };
        let chosen = f.page_commit;
        let mut segments = ui::segmented(t).child(
            ui::segment(
                SharedString::from(format!("fp-inherit-{}", id.0)),
                text(locale, Key::FoldersForPageInherit),
                chosen.is_none(),
                t,
            )
            .on_click(cx.listener(move |page, _, _, cx| {
                if chosen.is_some() {
                    page.set_page_commit(id, None, cx)
                }
            })),
        );
        for (m, key) in [
            (CommitMode::Manual, Key::FoldersCommitManual),
            (CommitMode::Commit, Key::FoldersCommitCommit),
            (CommitMode::Push, Key::FoldersCommitPush),
        ] {
            let selected = chosen.is_some_and(|c| c.mode == m);
            segments = segments.child(
                ui::segment(
                    SharedString::from(format!("fp-{key:?}-{}", id.0)),
                    text(locale, key),
                    selected,
                    t,
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    if !selected {
                        let value = PageCommit {
                            mode: m,
                            ..chosen.unwrap_or_default()
                        };
                        page.set_page_commit(id, Some(value), cx)
                    }
                })),
            );
        }
        let line = match (chosen, f.inherited) {
            (Some(c), _) => text(locale, fm::mode_hint(c.mode)),
            (None, (mode, None)) => tr(
                locale,
                Key::FoldersForPageFolder,
                &[("mode", Arg::Text(&mode_name(mode)))],
            ),
            (None, (mode, Some(from))) => tr(
                locale,
                Key::FoldersForPageFrom,
                &[
                    (
                        "from",
                        Arg::Text(&fm::group_label(
                            locale,
                            self.owner,
                            from,
                            folders.level_name(from),
                        )),
                    ),
                    ("mode", Arg::Text(&mode_name(mode))),
                ],
            ),
        };
        let mut form = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pl(px(48.))
            .pt(px(12.))
            .pb(px(8.))
            .on_mouse_down_out(cx.listener(move |page, _, _, cx| {
                if page.folder_ui.settings == Some(id) && page.folder_ui.branch_menu.is_none() {
                    page.folder_ui.settings = None;
                    cx.notify();
                }
            }))
            .child(
                type_scale::LABEL
                    .el()
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::FoldersForPage)),
            )
            .child(segments)
            .child(
                type_scale::SMALL
                    .el()
                    .max_w(px(480.))
                    .text_color(t.text_tertiary)
                    .child(line),
            );
        if let Some(c) = chosen.filter(|c| c.mode.automatic()) {
            let as_user = c.as_user.unwrap_or(folders.as_user_default);
            let check_row = |n: String, checked: bool, label: String| {
                let hover = t.fill_hover;
                ui::focusable(div().id(SharedString::from(n)), t, false)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .min_h(px(24.))
                    .px(px(4.))
                    .ml(px(-4.))
                    .rounded(px(RADIUS_SM))
                    .hover(move |s| s.bg(hover))
                    .child(ui::checkbox(checked, t))
                    .child(
                        type_scale::UI
                            .el()
                            .flex_1()
                            .min_w_0()
                            .text_color(t.text_primary)
                            .child(label),
                    )
            };
            form = form.child(
                check_row(
                    format!("fp-as-user-{}", id.0),
                    as_user,
                    text(locale, Key::FoldersCommitAsUser),
                )
                .on_click(cx.listener(move |page, _, _, cx| {
                    let value = PageCommit {
                        as_user: Some(!as_user),
                        ..c
                    };
                    page.set_page_commit(id, Some(value), cx)
                })),
            );
            if let Some(default) = checkout.and_then(|g| g.default_branch.as_ref()) {
                form = form.child(
                    check_row(
                        format!("fp-on-default-{}", id.0),
                        c.on_default,
                        tr(
                            locale,
                            Key::FoldersCommitOnDefault,
                            &[("branch", Arg::Text(default))],
                        ),
                    )
                    .on_click(cx.listener(move |page, _, _, cx| {
                        let value = PageCommit {
                            on_default: !c.on_default,
                            ..c
                        };
                        page.set_page_commit(id, Some(value), cx)
                    })),
                );
            }
        }
        form.into_any_element()
    }
}

/// The icon toggles of the folder form: 28 wide, 2 apart.
const ICON_SIZE: f32 = 28.;
const ICON_GAP: f32 = 2.;

/// The width that lays `count` icon toggles out in rows of equal length
/// within `room`: `None` when they fit on one line (or the room is not
/// known), else the width of the longest balanced row.
/// The "Also attached to" sentence for `count` other owners (two or
/// more), with `a` and `b` standing where the first two names go.
fn also_line(locale: Locale, count: usize, a: &str, b: &str) -> String {
    if count == 2 {
        tr(
            locale,
            Key::FoldersAlsoTwo,
            &[("a", Arg::Text(a)), ("b", Arg::Text(b))],
        )
    } else {
        tr(
            locale,
            Key::FoldersAlsoMore,
            &[
                ("a", Arg::Text(a)),
                ("b", Arg::Text(b)),
                ("n", Arg::Count(count.saturating_sub(2) as u64)),
            ],
        )
    }
}

/// An owner's name with its level word: "topic Scrollback".
fn also_name(locale: Locale, owner: Owner, name: &str) -> String {
    let key = match owner {
        Owner::Epic(_) => Key::FoldersAlsoNameEpic,
        _ => Key::FoldersAlsoNameTopic,
    };
    tr(locale, key, &[("name", Arg::Text(name))])
}

/// What a key does on a folder of the list (interaction.md 6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FolderKey {
    /// `Up` (true) / `Down`.
    Step(bool),
    /// `Space`: make it active here.
    Activate,
    /// `Enter`: reveal it, or Locate when not found.
    Open,
    /// `Mod+C`: copy its full path.
    Copy,
    /// `Shift+F10`, the menu key.
    Menu,
    /// `Alt+Up` (-1) / `Alt+Down` (+1), own folders only.
    Move(i64),
    /// `F2`: its settings, own folders only.
    Settings,
    /// `E`: "Choose another folder...", own folders only.
    Other,
    /// `Delete`, `Cmd+Backspace`: remove it from this level, own only.
    Remove,
    /// `Left`, `Esc`: fold the list.
    Fold,
}

/// The key's action on a folder row: `key` is GPUI's key name; `plain`
/// no modifier, `alt` Alt alone, `secondary` the platform's `Mod` alone,
/// `shift` Shift alone, `platform` Cmd (or the Windows key) held.
fn folder_key(
    key: &str,
    plain: bool,
    alt: bool,
    secondary: bool,
    shift: bool,
    platform: bool,
) -> Option<FolderKey> {
    Some(match key {
        "up" if plain => FolderKey::Step(true),
        "down" if plain => FolderKey::Step(false),
        "up" if alt => FolderKey::Move(-1),
        "down" if alt => FolderKey::Move(1),
        "space" if plain => FolderKey::Activate,
        "enter" if plain => FolderKey::Open,
        "c" if secondary => FolderKey::Copy,
        "f10" if shift => FolderKey::Menu,
        "menu" if plain => FolderKey::Menu,
        "f2" if plain => FolderKey::Settings,
        "e" if plain => FolderKey::Other,
        "delete" if plain => FolderKey::Remove,
        "backspace" if platform => FolderKey::Remove,
        "left" | "escape" if plain => FolderKey::Fold,
        _ => return None,
    })
}

fn balanced_width(count: usize, room: f32) -> Option<f32> {
    let step = ICON_SIZE + ICON_GAP;
    let per_line = (((room + ICON_GAP) / step).floor() as usize).max(1);
    if count <= per_line || !room.is_finite() {
        return None;
    }
    let lines = count.div_ceil(per_line);
    let per_row = count.div_ceil(lines);
    Some(per_row as f32 * step - ICON_GAP)
}

/// Whether "Move up" and "Move down" apply to the own folder `id` among
/// `own`, in stored order: never for an only folder, never past an end.
fn move_options(own: &[FolderId], id: FolderId) -> (bool, bool) {
    match own.iter().position(|f| *f == id) {
        Some(at) => (at > 0, at + 1 < own.len()),
        None => (false, false),
    }
}

#[cfg(test)]
mod form_tests {
    use super::*;

    #[test]
    fn two_other_owners_are_named_and_more_are_counted() {
        let en = Locale::En;
        assert_eq!(
            also_line(
                en,
                2,
                &also_name(en, Owner::Topic(acta_core::model::TopicId(2)), "Scrollback"),
                "topic Links"
            ),
            "Also attached to topic Scrollback and topic Links."
        );
        assert_eq!(
            also_line(en, 4, "topic A", "topic B"),
            "Also attached to topic A, topic B and 2 more."
        );
        assert_eq!(
            also_line(Locale::Fr, 3, "sujet A", "sujet B"),
            "Également rattaché à sujet A, à sujet B et à 1 autre."
        );
        assert_eq!(
            also_name(
                Locale::Fr,
                Owner::Epic(acta_core::model::EpicId(1)),
                "Notes"
            ),
            "epic Notes"
        );
    }

    #[test]
    fn folder_rows_take_the_keys_of_the_keyboard_map() {
        let k = |key, plain, alt, secondary, shift, platform| {
            folder_key(key, plain, alt, secondary, shift, platform)
        };
        assert_eq!(
            k("space", true, false, false, false, false),
            Some(FolderKey::Activate)
        );
        assert_eq!(
            k("enter", true, false, false, false, false),
            Some(FolderKey::Open)
        );
        assert_eq!(
            k("up", false, true, false, false, false),
            Some(FolderKey::Move(-1))
        );
        assert_eq!(
            k("down", false, true, false, false, false),
            Some(FolderKey::Move(1))
        );
        assert_eq!(
            k("down", true, false, false, false, false),
            Some(FolderKey::Step(false))
        );
        assert_eq!(
            k("f2", true, false, false, false, false),
            Some(FolderKey::Settings)
        );
        assert_eq!(
            k("e", true, false, false, false, false),
            Some(FolderKey::Other)
        );
        assert_eq!(
            k("delete", true, false, false, false, false),
            Some(FolderKey::Remove)
        );
        assert_eq!(
            k("backspace", false, false, false, false, true),
            Some(FolderKey::Remove)
        );
        assert_eq!(k("backspace", true, false, false, false, false), None);
        assert_eq!(
            k("c", false, false, true, false, false),
            Some(FolderKey::Copy)
        );
        assert_eq!(k("c", true, false, false, false, false), None);
        assert_eq!(
            k("f10", false, false, false, true, false),
            Some(FolderKey::Menu)
        );
        assert_eq!(
            k("escape", true, false, false, false, false),
            Some(FolderKey::Fold)
        );
        // A letter with a modifier is someone else's shortcut.
        assert_eq!(k("e", false, false, true, false, false), None);
    }

    #[test]
    fn an_only_folder_moves_nowhere_and_the_ends_move_one_way() {
        let a = FolderId(1);
        let b = FolderId(2);
        let c = FolderId(3);
        assert_eq!(move_options(&[a], a), (false, false));
        assert_eq!(move_options(&[a, b, c], a), (false, true));
        assert_eq!(move_options(&[a, b, c], b), (true, true));
        assert_eq!(move_options(&[a, b, c], c), (true, false));
        assert_eq!(move_options(&[a, b], c), (false, false));
    }

    #[test]
    fn icons_that_wrap_go_in_rows_of_equal_length() {
        // Room for 12: 14 icons go 7 and 7.
        assert_eq!(balanced_width(14, 12. * 30.), Some(7. * 30. - 2.));
        // Room for all 14: one line.
        assert_eq!(balanced_width(14, 14. * 30.), None);
        // Room for 5: 14 go 5, 5, 4.
        assert_eq!(balanced_width(14, 5. * 30.), Some(5. * 30. - 2.));
        assert_eq!(balanced_width(14, f32::MAX), None);
    }
}
