//! The terminal tool window (docs/design/layout.md 4.5; components-folders.md
//! 5, Terminal launch; screens.md 8.4, 8.11): one session per project,
//! started by the launch plan of the page it was opened from (ADR 0011:
//! the active folder is the working directory, the other folders the page
//! sees are `--add-dir`, in list order, except a folder not found and
//! another checkout of the same repository), with the focus in `ACTA_*`.
//! Navigation, a folder switch and a commit-mode change never restart or
//! move a session: the header marks the mismatch and offers "Restart in
//! ..." or "Restart here".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use acta_core::launch_plan::{LaunchPlan, LeftOut, PlanFolder, launch_plan};
use acta_core::model::{Author, FolderId, Owner, ProjectId, Record};
use acta_core::presence::SessionState;
use acta_terminal::{
    ClaudeOptions, Conversation, Focus, LaunchError, TerminalEvent, TerminalTheme, TerminalView,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, PathPromptOptions, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};

use crate::folders_model::{self as fm, Mismatch};
use crate::folders_view::{self, SwitchActions, SwitchEntry, SwitchGroup};
use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::paths_view;
use crate::record::Folders;
use crate::settings;
use crate::theme::{Appearance, Mode, RADIUS_XS, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{CommitColumns, FolderRef, ToastKind, Undo, Workspace};

/// Where a launch comes from: the page's record and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub owner: Owner,
    pub project: ProjectId,
    pub name: String,
}

enum Pane {
    /// Folders and the program are being resolved (no message before 400 ms).
    Starting,
    Running {
        view: Entity<TerminalView>,
        origin: Origin,
        /// Where it started: the working directory, the active folder
        /// record and its commit columns then (a change marks the header).
        start: Start,
        /// `ACTA_LAUNCH`: finds this session's presence row.
        token: String,
        /// The conversation the page resumes next time; follows `/clear`.
        conversation: Option<String>,
        _sub: Subscription,
    },
    /// No folder anywhere up the chain. `prompt`: a command the launch was
    /// to start Claude Code on ("File with Claude Code"), kept only while
    /// this pane asks, so it never outlives a failed or replaced launch.
    Ask {
        origin: Origin,
        save: bool,
        prompt: Option<String>,
    },
    /// The primary folder is not on this computer.
    Missing {
        origin: Origin,
        path: PathBuf,
        from: Owner,
    },
    /// The `claude` command was not found.
    NotFound { origin: Origin },
}

/// What running a command in a project's session comes to (the inbox's
/// "File with Claude Code", screens.md 8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Send {
    /// Claude Code was idle at its prompt: the command was typed there.
    Typed,
    /// There was no live session: one starts on the command.
    Started,
    /// Claude Code is mid-task: nothing was typed. A new session on the
    /// command needs the restart confirmation (interaction.md 6.4).
    Busy,
}

/// Where a session started (layout.md 4.5, Mismatch).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Start {
    pub cwd: PathBuf,
    pub folder: Option<FolderId>,
    pub commit: Option<CommitColumns>,
}

/// The folders of a session's page, as the header shows them: refreshed
/// whenever the store changes.
#[derive(Clone, Debug, Default)]
struct SessionPage {
    folders: Folders,
    /// The commit columns of the folder the session started in, now.
    commit: Option<CommitColumns>,
}

/// The folders Acta's terminal sessions run in, with their launch tokens:
/// a git action in one of them asks first while the session works (ADR
/// 0011 addendum).
#[derive(Default)]
pub struct RunningIn(pub Vec<(PathBuf, String)>);

impl gpui_kit::Global for RunningIn {}

/// The launch plan as the command line takes it: the working directory
/// first, then the `--add-dir` folders in order. A plan with no working
/// directory stops the launch: no folder at all (the pane asks), or the
/// active folder not found (the pane offers Locate). A session is never
/// started in another folder on its own.
pub fn launch_folders(
    plan: &LaunchPlan,
    active: Option<&Path>,
) -> Result<Vec<PathBuf>, LaunchError> {
    match (&plan.working_dir, active) {
        (Some(cwd), _) => Ok(std::iter::once(cwd.clone())
            .chain(plan.add_dirs.iter().cloned())
            .collect()),
        (None, Some(active)) => Err(LaunchError::PrimaryMissing(active.to_path_buf())),
        (None, None) => Err(LaunchError::NoFolder),
    }
}

/// What a launch's queued prompt becomes: kept (on the Ask pane) only
/// when the launch stopped to ask for a folder; a started session already
/// has it, and a missing folder or command drops it.
fn queued_prompt(outcome: &Result<(), LaunchError>, prompt: Option<String>) -> Option<String> {
    match outcome {
        Err(LaunchError::NoFolder) => prompt,
        _ => None,
    }
}

/// The rule: type into a live session only when it is idle at its prompt;
/// with no live session start one on the command; never type into one
/// that is mid-task.
pub fn send_rule(live: bool, idle: bool) -> Send {
    match (live, idle) {
        (false, _) => Send::Started,
        (true, true) => Send::Typed,
        (true, false) => Send::Busy,
    }
}

pub enum DockEvent {
    /// "Restart here": the shell restarts with the active page's origin
    /// (after the confirmation when a process runs).
    RestartHere,
    /// "Restart in <folder>": the same for the session's own page.
    RestartIn(Origin),
    /// "Edit folders..." and "Add folder..." of the header switcher: on
    /// the session's page.
    EditFolders(Origin),
    AddFolder(Origin),
}

pub struct TerminalDock {
    ws: Entity<Workspace>,
    panes: HashMap<ProjectId, Pane>,
    /// A folder chosen in the prompt and not saved: used until Acta quits.
    unsaved: HashMap<ProjectId, PathBuf>,
    /// The newest launch of each project.
    generations: HashMap<ProjectId, u64>,
    pub project: Option<ProjectId>,
    /// The page shown now, set by the shell, to mark a mismatch.
    pub active: Option<Origin>,
    focus: FocusHandle,
    text_size: f32,
    mode: Option<Mode>,
    /// The folders of each session's page, for the header.
    pages: HashMap<ProjectId, SessionPage>,
    /// The header switcher's menu, with its highlighted entry.
    switcher: Option<usize>,
    seen_revision: u64,
    _refresh: Option<gpui_kit::Task<()>>,
    _ws_sub: Option<Subscription>,
}

impl EventEmitter<DockEvent> for TerminalDock {}

impl Focusable for TerminalDock {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.project.and_then(|p| self.panes.get(&p)) {
            Some(Pane::Running { view, .. }) => view.read(cx).focus_handle(cx),
            _ => self.focus.clone(),
        }
    }
}

fn focus_of(owner: Owner, project: ProjectId, epic: Option<acta_core::model::EpicId>) -> Focus {
    match owner {
        Owner::Project(id) => Focus {
            project: id.to_string(),
            epic: None,
            topic: None,
        },
        Owner::Epic(id) => Focus {
            project: project.to_string(),
            epic: Some(id.to_string()),
            topic: None,
        },
        Owner::Topic(id) => Focus {
            project: project.to_string(),
            epic: epic.map(|e| e.to_string()),
            topic: Some(id.to_string()),
        },
    }
}

impl TerminalDock {
    pub fn new(ws: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let text_size = settings::current(cx)
            .terminal_text_size
            .map(f32::from)
            .unwrap_or(acta_terminal::view::DEFAULT_TEXT_SIZE);
        let sub = cx.observe(&ws, |dock, ws, cx| {
            let revision = ws.read(cx).revision;
            if revision != dock.seen_revision {
                dock.seen_revision = revision;
                dock.refresh_pages(cx);
            }
        });
        Self {
            ws,
            panes: HashMap::new(),
            unsaved: HashMap::new(),
            generations: HashMap::new(),
            project: None,
            active: None,
            focus: cx.focus_handle(),
            text_size,
            mode: None,
            pages: HashMap::new(),
            switcher: None,
            seen_revision: 0,
            _refresh: None,
            _ws_sub: Some(sub),
        }
    }

    /// The folders of every session's page read again (off the UI
    /// thread), with the existence of each.
    fn refresh_pages(&mut self, cx: &mut Context<Self>) {
        let wanted: Vec<(ProjectId, Owner, Option<FolderId>)> = self
            .panes
            .iter()
            .filter_map(|(project, pane)| match pane {
                Pane::Running { origin, start, .. } => Some((*project, origin.owner, start.folder)),
                _ => None,
            })
            .collect();
        if wanted.is_empty() {
            self.pages.clear();
            return;
        }
        let task = self.ws.read(cx).db.run(cx, move |store| {
            let mut out = Vec::new();
            for (project, owner, folder) in wanted {
                let Ok(folders) = crate::record::load_folders(store, owner) else {
                    continue;
                };
                let commit = folder
                    .and_then(|id| store.folder(id).ok())
                    .map(|f| (f.commit_mode, f.commit_as_user, f.commit_on_default));
                out.push((project, SessionPage { folders, commit }));
            }
            Ok(out)
        });
        self._refresh = Some(cx.spawn(async move |dock, cx| {
            let Ok(mut pages) = task.await else { return };
            let pages = cx
                .background_executor()
                .spawn(async move {
                    for (_, page) in &mut pages {
                        page.folders.missing =
                            crate::workspace::missing_paths(page.folders.paths());
                    }
                    pages
                })
                .await;
            let _ = dock.update(cx, |dock, cx| {
                dock.pages = pages.into_iter().collect();
                cx.notify();
            });
        }));
    }

    pub fn has_session(&self, project: ProjectId) -> bool {
        self.panes.contains_key(&project)
    }

    /// Whether a process runs in the project's session (the restart
    /// confirmation of interaction.md 6.4 asks only then).
    pub fn is_running(&self, project: ProjectId, cx: &App) -> bool {
        matches!(self.panes.get(&project), Some(Pane::Running { view, .. }) if view.read(cx).is_running())
    }

    /// Show the session of `origin.project`, starting it from `origin` if
    /// there is none.
    pub fn show(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        self.project = Some(origin.project);
        if !self.panes.contains_key(&origin.project) {
            self.launch(origin, None, window, cx);
        }
        cx.notify();
    }

    /// End the project's session and start it from `origin`.
    pub fn restart(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        self.project = Some(origin.project);
        self.launch(origin, None, window, cx);
    }

    /// Run `command` (a slash command) in the session of `origin.project`
    /// by [`send_rule`]. On `Busy` nothing happens; the caller confirms
    /// and calls [`TerminalDock::restart_on`].
    pub fn send(
        &mut self,
        origin: Origin,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Send {
        let project = origin.project;
        self.project = Some(project);
        let live = match self.panes.get(&project) {
            Some(Pane::Running { view, .. }) if view.read(cx).is_running() => Some(view.clone()),
            _ => None,
        };
        let idle = live
            .as_ref()
            .is_some_and(|view| view.read(cx).idle_at_prompt());
        let step = send_rule(live.is_some(), idle);
        match (step, live) {
            (Send::Typed, Some(view)) => {
                view.update(cx, |view, cx| view.send_line(command, cx));
                view.read(cx).focus_handle(cx).focus(window, cx);
            }
            (Send::Started, _) => self.restart_on(origin, command, window, cx),
            _ => {}
        }
        cx.notify();
        step
    }

    /// The project's terminal screen as text (development checks only).
    #[cfg(debug_assertions)]
    pub fn dev_screen(&self, project: ProjectId, cx: &App) -> Option<String> {
        match self.panes.get(&project) {
            Some(Pane::Running { view, .. }) => Some(view.read(cx).screen_text()),
            _ => None,
        }
    }

    /// Type `text` into the project's session if Claude Code is idle
    /// (development checks only).
    #[cfg(debug_assertions)]
    pub fn dev_type_if_idle(&mut self, project: ProjectId, text: &str, cx: &mut App) -> bool {
        let Some(Pane::Running { view, .. }) = self.panes.get(&project) else {
            return false;
        };
        let view = view.clone();
        if !view.read(cx).idle_at_prompt() {
            return false;
        }
        view.update(cx, |view, cx| view.send_line(text, cx));
        true
    }

    /// Write `bytes` into the project's session as typed (development
    /// checks only: Esc to decline a question).
    #[cfg(debug_assertions)]
    pub fn dev_send_raw(&mut self, project: ProjectId, bytes: &[u8], cx: &mut App) {
        if let Some(Pane::Running { view, .. }) = self.panes.get(&project) {
            let bytes = bytes.to_vec();
            view.update(cx, |view, _| view.send_raw(bytes));
        }
    }

    /// End the project's session and start Claude Code on `command`.
    pub fn restart_on(
        &mut self,
        origin: Origin,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.project = Some(origin.project);
        self.launch(origin, Some(command.to_owned()), window, cx);
    }

    /// Start the project's session from `origin`, on `prompt` if given.
    /// The prompt goes into this launch only: kept on the Ask pane if a
    /// folder is asked first, dropped on any other outcome.
    fn launch(
        &mut self,
        origin: Origin,
        prompt: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = origin.project;
        let owner = origin.owner;
        let unsaved = self.unsaved.get(&project).cloned();
        let queued = prompt.clone();
        // Each launch of a project gets a number; a result that arrives
        // after a newer launch started is dropped.
        let generation = {
            let n = self.generations.entry(project).or_insert(0);
            *n += 1;
            *n
        };
        let new_conversations = settings::current(cx).new_conversations;
        let ws = self.ws.read(cx);
        let folders = ws.db.run(cx, move |store| {
            // The page's folders in list order, the active one marked.
            let visible = store.visible_folders(owner)?;
            let epic = match owner {
                Owner::Topic(id) => store.topic(id)?.epic,
                _ => None,
            };
            let mut folders: Vec<PlanFolder> = visible
                .folders
                .iter()
                .map(|f| PlanFolder {
                    path: PathBuf::from(&f.folder.path),
                    own: f.origin == owner,
                    active: f.active,
                })
                .collect();
            if folders.is_empty()
                && let Some(path) = unsaved
            {
                folders.push(PlanFolder {
                    path,
                    own: true,
                    active: true,
                });
            }
            let active = visible.active().map(|f| {
                let c = &f.folder;
                (
                    f.origin,
                    c.id,
                    (c.commit_mode, c.commit_as_user, c.commit_on_default),
                )
            });
            let config = acta_terminal::claude_config_dir();
            let conversation = match folders.iter().find(|f| f.active) {
                Some(f) => {
                    conversation_for(store, owner, new_conversations, config.as_deref(), &f.path)?
                }
                None => None,
            };
            Ok((folders, active, epic, conversation))
        });
        // The previous session (if any) ends when its view is dropped.
        let old = self.panes.insert(project, Pane::Starting);
        drop(old);
        cx.notify();
        cx.spawn_in(window, async move |dock, cx| {
            let Ok((folders, active, epic, conversation)) = folders.await else {
                return;
            };
            let token = acta_core::random_uuid().unwrap_or_default();
            let launch_token = token.clone();
            let conversation_id = match &conversation {
                Some(Conversation::New(id)) | Some(Conversation::Resume(id)) => Some(id.clone()),
                None => None,
            };
            // The plan (existence, git checkouts) and the search for
            // `claude` read the disk: off the store and the UI thread.
            let primary_from = active.map(|(from, _, _)| from);
            let start_folder = active.map(|(_, id, _)| id);
            let start_commit = active.map(|(_, _, c)| c);
            let result = cx
                .background_executor()
                .spawn(async move {
                    let plan = launch_plan(&folders);
                    let active = folders.iter().find(|f| f.active).map(|f| f.path.as_path());
                    let paths = launch_folders(&plan, active)?;
                    let options = ClaudeOptions {
                        plugin_dir: plugin_dir(),
                        conversation,
                        token: (!launch_token.is_empty()).then_some(launch_token),
                        prompt,
                    };
                    let mut launch = acta_terminal::launch_spec_for_claude(
                        &paths,
                        &focus_of(owner, project, epic),
                        &options,
                    )?;
                    // Not found: reported; another checkout: the list says so.
                    let missing = plan
                        .left_out
                        .into_iter()
                        .filter(|(_, why)| *why == LeftOut::NotFound)
                        .map(|(p, _)| p);
                    launch.left_out = missing.chain(launch.left_out).collect();
                    Ok(launch)
                })
                .await;
            let _ = dock.update_in(cx, |dock, window, cx| {
                if dock.generations.get(&project) != Some(&generation) {
                    return;
                }
                let pane = match result {
                    Ok(launch) => {
                        if !launch.left_out.is_empty() {
                            let locale = settings::locale(cx);
                            for folder in &launch.left_out {
                                let name =
                                    paths_view::last_segment(&folder.to_string_lossy()).to_owned();
                                let message = tr(
                                    locale,
                                    Key::TerminalLeftOut,
                                    &[("folder", Arg::Text(&name))],
                                );
                                dock.ws
                                    .update(cx, |ws, cx| ws.toast(message, ToastKind::Error, cx));
                            }
                        }
                        let start = Start {
                            cwd: launch.spec.cwd.clone(),
                            folder: start_folder,
                            commit: start_commit,
                        };
                        dock.running(
                            origin,
                            start,
                            launch.spec,
                            token,
                            conversation_id,
                            window,
                            cx,
                        )
                    }
                    Err(LaunchError::NoFolder) => Pane::Ask {
                        origin,
                        save: true,
                        prompt: queued_prompt(&Err(LaunchError::NoFolder), queued),
                    },
                    Err(LaunchError::PrimaryMissing(path)) => Pane::Missing {
                        origin,
                        path,
                        from: primary_from.unwrap_or(owner),
                    },
                    Err(LaunchError::ClaudeNotFound) => Pane::NotFound { origin },
                };
                dock.panes.insert(project, pane);
                dock.publish_tokens(cx);
                dock.refresh_pages(cx);
                cx.notify();
            });
        })
        .detach();
    }

    #[allow(clippy::too_many_arguments)]
    fn running(
        &mut self,
        origin: Origin,
        start: Start,
        spec: acta_terminal::LaunchSpec,
        token: String,
        conversation: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Pane {
        let mode = cx.global::<Appearance>().mode;
        let size = self.text_size;
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let ended = tr(
            locale,
            Key::TerminalEnded,
            &[("keys", Arg::Text(KeyName::Enter.label(locale, macos)))],
        );
        let view = cx.new(|cx| {
            let mut view = TerminalView::spawn(spec, cx);
            view.set_theme(theme_for(mode), cx);
            view.set_text_size(size, cx);
            view.set_ended_message(ended.into(), cx);
            view
        });
        let project = origin.project;
        let restart_origin = origin.clone();
        let sub = cx.subscribe_in(
            &view,
            window,
            move |dock, _, event: &TerminalEvent, window, cx| {
                match event {
                    // Restarting uses the folders of the page open at that
                    // moment; the dock restarts from where it was started when
                    // it does not know the page.
                    TerminalEvent::RestartRequested => {
                        dock.restart(restart_origin.clone(), window, cx)
                    }
                    TerminalEvent::TextSizeChanged(size) => {
                        dock.text_size = *size;
                        let stored = size.round() as u8;
                        settings::change(cx, |prefs| prefs.terminal_text_size = Some(stored));
                    }
                    _ => {}
                }
            },
        );
        let _ = project;
        view.read(cx).focus_handle(cx).focus(window, cx);
        Pane::Running {
            view,
            origin,
            start,
            token,
            conversation,
            _sub: sub,
        }
    }

    /// The Sessions view finds Acta's terminals by their launch tokens.
    fn publish_tokens(&self, cx: &mut App) {
        let tokens = self
            .panes
            .iter()
            .filter_map(|(project, pane)| match pane {
                Pane::Running { token, .. } => Some((token.clone(), *project)),
                _ => None,
            })
            .collect();
        cx.set_global(crate::sessions_view::TerminalTokens(tokens));
        let running = self
            .panes
            .values()
            .filter_map(|pane| match pane {
                Pane::Running { start, token, .. } => Some((start.cwd.clone(), token.clone())),
                _ => None,
            })
            .collect();
        cx.set_global(RunningIn(running));
    }

    /// The launch token of the project's running terminal.
    pub fn token(&self, project: ProjectId) -> Option<&str> {
        match self.panes.get(&project) {
            Some(Pane::Running { token, .. }) => Some(token),
            _ => None,
        }
    }

    /// "End session" (Sessions view): ends the terminal's process; the
    /// pane keeps its last screen and restarts on Enter.
    pub fn end(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if let Some(Pane::Running { view, .. }) = self.panes.get(&project) {
            view.update(cx, |view, cx| view.end(cx));
        }
    }

    /// Show the project's terminal session without starting one.
    pub fn select(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if self.panes.contains_key(&project) {
            self.project = Some(project);
            cx.notify();
        }
    }

    /// `/clear` gives the session a new id: the page's next start resumes
    /// that one (presence carries the id under the terminal's token).
    fn follow_conversations(&mut self, cx: &mut Context<Self>) {
        let live = crate::sessions::live(cx);
        let mut moved = Vec::new();
        for pane in self.panes.values_mut() {
            if let Pane::Running {
                token,
                conversation,
                origin,
                ..
            } = pane
                && let Some(s) = crate::sessions::of_launch(live, token)
                && conversation.as_deref() != Some(s.id.as_str())
                && acta_terminal::launch::is_uuid(&s.id)
            {
                *conversation = Some(s.id.clone());
                moved.push((origin.owner, s.id.clone()));
            }
        }
        for (owner, id) in moved {
            let key = conversation_key(owner);
            let task = self
                .ws
                .read(cx)
                .db
                .run(cx, move |store| store.set_local_value(&key, Some(&id)));
            task.detach();
        }
    }

    /// The presence state of the project's running terminal session.
    pub fn session_state(&self, project: ProjectId, cx: &App) -> Option<SessionState> {
        let token = self.token(project)?;
        crate::sessions::of_launch(crate::sessions::live(cx), token).map(|s| s.state)
    }

    pub fn change_text_size(&mut self, by: f32, cx: &mut Context<Self>) {
        use acta_terminal::view::{MAX_TEXT_SIZE, MIN_TEXT_SIZE};
        self.text_size = (self.text_size + by).clamp(MIN_TEXT_SIZE, MAX_TEXT_SIZE);
        let size = self.text_size;
        for pane in self.panes.values() {
            if let Pane::Running { view, .. } = pane {
                view.update(cx, |view, cx| view.set_text_size(size, cx));
            }
        }
        let stored = size.round() as u8;
        settings::change(cx, |prefs| prefs.terminal_text_size = Some(stored));
    }

    /// The command the Ask pane of `project` holds, if it is asking.
    fn asked_prompt(&self, project: ProjectId) -> Option<String> {
        match self.panes.get(&project) {
            Some(Pane::Ask { prompt, .. }) => prompt.clone(),
            _ => None,
        }
    }

    fn choose_folder(
        &mut self,
        origin: Origin,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn_in(window, async move |dock, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.pop() else { return };
            let _ = dock.update_in(cx, |dock, window, cx| {
                let project = origin.project;
                if save {
                    let task = dock.ws.update(cx, |ws, cx| {
                        ws.write_undoable(cx, Some(project), None, move |store| {
                            let id =
                                store.add_folder(Owner::Project(project), &path, Author::Human)?;
                            Ok((Undo::Delete(Record::Folder(id)), ()))
                        })
                    });
                    cx.spawn_in(window, async move |dock, cx| {
                        if task.await.is_ok() {
                            let _ = dock.update_in(cx, |dock, window, cx| {
                                let prompt = dock.asked_prompt(project);
                                dock.launch(origin, prompt, window, cx)
                            });
                        }
                    })
                    .detach();
                } else {
                    dock.unsaved.insert(project, path);
                    let prompt = dock.asked_prompt(project);
                    dock.launch(origin, prompt, window, cx);
                }
            });
        })
        .detach();
    }

    fn locate(
        &mut self,
        origin: Origin,
        path: PathBuf,
        from: Owner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn_in(window, async move |dock, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(new) = paths.pop() else { return };
            let _ = dock.update_in(cx, |dock, window, cx| {
                // The shared Locate: undo and the bulk offer come with it.
                let old = path.to_string_lossy().into_owned();
                let project = origin.project;
                let task = dock.ws.update(cx, |ws, cx| {
                    ws.locate(FolderRef::At(from, old), new, Some(project), cx)
                });
                cx.spawn_in(window, async move |dock, cx| {
                    if task.await {
                        let _ = dock.update_in(cx, |dock, window, cx| {
                            dock.launch(origin, None, window, cx)
                        });
                    }
                })
                .detach();
            });
        })
        .detach();
    }

    /// The header's context: "Claude · T-12 Embed the terminal".
    fn context(origin: &Origin, locale: Locale) -> String {
        format!(
            "{} \u{b7} {}",
            text(locale, Key::TerminalClaude),
            focus_label(locale, origin.owner, &origin.name)
        )
    }

    fn render_dock(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.follow_conversations(cx);
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let mode = cx.global::<Appearance>().mode;
        if self.mode != Some(mode) {
            self.mode = Some(mode);
            for pane in self.panes.values() {
                if let Pane::Running { view, .. } = pane {
                    view.update(cx, |view, cx| view.set_theme(theme_for(mode), cx));
                }
            }
        }
        let pane = self.project.and_then(|p| self.panes.get(&p));
        let surface = match mode {
            Mode::Dark => t.bg_sunken,
            Mode::Light => t.bg_raised,
        };
        let body: AnyElement = match pane {
            // screens.md 8.4: the grid is inset 12 on every side.
            Some(Pane::Running { view, .. }) => div()
                .flex_1()
                .min_h_0()
                .p(px(12.))
                .bg(surface)
                .child(view.clone())
                .into_any_element(),
            Some(Pane::Ask { origin, save, .. }) => {
                let origin = origin.clone();
                let save = *save;
                let project_name = self.project_name(origin.project, cx);
                let sentence = match origin.owner {
                    Owner::Topic(_) => tr(
                        locale,
                        Key::TerminalAskTopic,
                        &[("name", Arg::Text(&origin.name))],
                    ),
                    Owner::Epic(_) => tr(
                        locale,
                        Key::TerminalAskEpic,
                        &[("name", Arg::Text(&origin.name))],
                    ),
                    Owner::Project(_) => tr(
                        locale,
                        Key::TerminalAskProject,
                        &[("name", Arg::Text(&origin.name))],
                    ),
                };
                let project = origin.project;
                message(&t, surface)
                    .child(
                        type_scale::HEADING
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::TerminalAskTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_secondary)
                            .child(sentence),
                    )
                    .child(
                        div()
                            .id("terminal-save")
                            .mt(px(16.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .on_click(cx.listener(move |dock, _, _, cx| {
                                if let Some(Pane::Ask { save, .. }) = dock.panes.get_mut(&project) {
                                    *save = !*save;
                                }
                                cx.notify();
                            }))
                            .child(checkbox(save, &t))
                            .child(type_scale::UI.el().text_color(t.text_primary).child(tr(
                                locale,
                                Key::TerminalAskSave,
                                &[("name", Arg::Text(&project_name))],
                            ))),
                    )
                    .child(
                        type_scale::SMALL
                            .el()
                            .mt(px(4.))
                            .pl(px(24.))
                            .text_color(t.text_tertiary)
                            .child(text(locale, Key::TerminalAskUnsaved)),
                    )
                    .child(
                        div().mt(px(24.)).flex().child(
                            ui::button(
                                "terminal-choose",
                                text(locale, Key::TerminalAskChoose),
                                Variant::Primary,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |dock, _, window, cx| {
                                    dock.choose_folder(origin.clone(), save, window, cx)
                                },
                            )),
                        ),
                    )
                    .into_any_element()
            }
            Some(Pane::Missing { origin, path, from }) => {
                let (origin, path, from) = (origin.clone(), path.clone(), *from);
                let body_key = match origin.owner {
                    Owner::Topic(_) => Key::TerminalMissingBodyTopic,
                    Owner::Epic(_) => Key::TerminalMissingBodyEpic,
                    Owner::Project(_) => Key::TerminalMissingBodyProject,
                };
                let level_name = origin.name.clone();
                let retry_origin = origin.clone();
                message(&t, surface)
                    .child(
                        type_scale::HEADING
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::TerminalMissingTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_secondary)
                            .child(tr(locale, body_key, &[("name", Arg::Text(&level_name))])),
                    )
                    .child(
                        type_scale::CODE
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_primary)
                            .child(path.to_string_lossy().into_owned()),
                    )
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(
                                ui::button(
                                    "terminal-locate",
                                    text(locale, Key::TerminalMissingLocate),
                                    Variant::Primary,
                                    false,
                                    &t,
                                )
                                .on_click(cx.listener(
                                    move |dock, _, window, cx| {
                                        dock.locate(origin.clone(), path.clone(), from, window, cx)
                                    },
                                )),
                            )
                            .child(
                                ui::button(
                                    "terminal-retry",
                                    text(locale, Key::CommonTryAgain),
                                    Variant::Secondary,
                                    false,
                                    &t,
                                )
                                .on_click(cx.listener(
                                    move |dock, _, window, cx| {
                                        dock.launch(retry_origin.clone(), None, window, cx)
                                    },
                                )),
                            ),
                    )
                    .into_any_element()
            }
            Some(Pane::NotFound { origin }) => {
                let origin = origin.clone();
                message(&t, surface)
                    .child(
                        type_scale::HEADING
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::StatusbarNotFound)),
                    )
                    .child(
                        div().mt(px(24.)).flex().child(
                            ui::button(
                                "terminal-retry",
                                text(locale, Key::CommonTryAgain),
                                Variant::Secondary,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |dock, _, window, cx| {
                                    dock.launch(origin.clone(), None, window, cx)
                                },
                            )),
                        ),
                    )
                    .into_any_element()
            }
            Some(Pane::Starting) | None => div().flex_1().bg(surface).into_any_element(),
        };
        div()
            .id("terminal-dock")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(surface)
            .child(body)
            .into_any_element()
    }

    /// The session shown, and whether it was started from another page
    /// than the active one (layout.md 4.5).
    fn shown_origin(&self) -> (Option<Origin>, bool) {
        let pane = self.project.and_then(|p| self.panes.get(&p));
        let origin = match pane {
            Some(Pane::Running { origin, .. })
            | Some(Pane::Ask { origin, .. })
            | Some(Pane::Missing { origin, .. })
            | Some(Pane::NotFound { origin }) => Some(origin.clone()),
            _ => None,
        };
        let mismatch = match (&origin, &self.active) {
            (Some(started), Some(active)) => started.owner != active.owner,
            _ => false,
        };
        (origin, mismatch)
    }

    /// The mismatch of the session shown (layout.md 4.5): another page,
    /// its page's active folder changed, or the commit mode of the folder
    /// it runs in changed since it started.
    fn mismatch(&self) -> Mismatch {
        let (origin, other_page) = self.shown_origin();
        let Some(project) = origin.map(|o| o.project) else {
            return Mismatch::None;
        };
        let Some(Pane::Running { start, .. }) = self.panes.get(&project) else {
            return if other_page {
                Mismatch::RestartHere
            } else {
                Mismatch::None
            };
        };
        let page = self.pages.get(&project);
        let active = page.and_then(|p| p.folders.active());
        let mode_changed = page.is_some_and(|p| p.commit.is_some() && p.commit != start.commit);
        fm::mismatch(
            other_page,
            &start.cwd.to_string_lossy(),
            active.map(|f| (f.path.as_str(), f.name.as_str())),
            mode_changed,
        )
    }

    /// The header switcher of the session's page: its folders, grouped.
    fn switch_groups(&self, project: ProjectId, owner: Owner, locale: Locale) -> Vec<SwitchGroup> {
        let Some(page) = self.pages.get(&project) else {
            return Vec::new();
        };
        let folders = &page.folders;
        fm::groups(owner, &folders.items)
            .into_iter()
            .filter(|(_, list)| !list.is_empty())
            .map(|(origin, list)| SwitchGroup {
                label: fm::group_label(locale, owner, origin, folders.level_name(origin)),
                entries: list
                    .into_iter()
                    .map(|i| {
                        let f = &folders.items[i];
                        let missing = folders.is_missing(&f.path);
                        SwitchEntry {
                            id: f.id,
                            icon: fm::shown_icon(&f.icon, None, missing),
                            name: f.name.clone(),
                            active: f.active,
                            missing,
                        }
                    })
                    .collect(),
            })
            .collect()
    }

    /// The header switcher sets the session page's active folder, exactly
    /// as the page's own switcher does; the session keeps running.
    fn set_page_active(
        &mut self,
        owner: Owner,
        project: ProjectId,
        id: FolderId,
        cx: &mut Context<Self>,
    ) {
        self.switcher = None;
        cx.notify();
        let active = self
            .pages
            .get(&project)
            .and_then(|p| p.folders.active())
            .map(|f| f.id);
        if active == Some(id) {
            return;
        }
        self.ws.update(cx, |ws, cx| {
            ws.undoable(cx, Some(project), None, move |store| {
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

    /// The context of the terminal's area header or view strip
    /// (workbench.md 4.8.4; layout.md 4.5): "on topic Embed the terminal",
    /// the folder switcher of the session's page, "(running in ...)" when
    /// that is not where it runs, and on a mismatch the context in
    /// `text.primary` with "Restart in ..." or "Restart here".
    pub fn header_context(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let (origin, _) = self.shown_origin();
        let origin = origin?;
        let mismatch = self.mismatch();
        let marked = mismatch != Mismatch::None;
        let waiting = self
            .session_state(origin.project, cx)
            .is_some_and(|s| s == SessionState::Waiting);
        let project = origin.project;
        let running = match self.panes.get(&project) {
            Some(Pane::Running { start, .. }) => Some(start.cwd.clone()),
            _ => None,
        };
        let page_active = self.pages.get(&project).and_then(|p| {
            p.folders
                .active()
                .cloned()
                .map(|f| (f.clone(), p.folders.is_missing(&f.path)))
        });
        let switcher = page_active.map(|(active, missing)| {
            let running_elsewhere = running.as_ref().is_some_and(|cwd| {
                !acta_core::folder_path::same(
                    acta_core::paths::Os::CURRENT,
                    &cwd.to_string_lossy(),
                    &active.path,
                )
            });
            let button = folders_view::switcher(
                "terminal-switcher",
                fm::shown_icon(&active.icon, None, missing),
                &active.name,
                missing,
                &t,
            )
            .tooltip(ui::tip(active.path.clone()))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .on_click(cx.listener(|dock, _, _, cx| {
                dock.switcher = match dock.switcher {
                    Some(_) => None,
                    None => Some(0),
                };
                cx.notify();
            }));
            let mut wrap = div().relative().min_w_0().child(button);
            if self.switcher.is_some() {
                let groups = self.switch_groups(project, origin.owner, locale);
                let owner = origin.owner;
                let (edit_origin, add_origin) = (origin.clone(), origin.clone());
                wrap = wrap.child(folders_view::switcher_menu(
                    "terminal-switch-menu",
                    &groups,
                    self.switcher,
                    locale,
                    &t,
                    cx,
                    SwitchActions {
                        choose: Rc::new(move |dock: &mut Self, id, _, cx| {
                            dock.set_page_active(owner, project, id, cx)
                        }),
                        edit: Rc::new(move |dock: &mut Self, _, cx| {
                            dock.switcher = None;
                            cx.emit(DockEvent::EditFolders(edit_origin.clone()));
                            cx.notify();
                        }),
                        add: Rc::new(move |dock: &mut Self, _, cx| {
                            dock.switcher = None;
                            cx.emit(DockEvent::AddFolder(add_origin.clone()));
                            cx.notify();
                        }),
                        close: Rc::new(|dock: &mut Self, _, cx| {
                            dock.switcher = None;
                            cx.notify();
                        }),
                    },
                ));
            }
            let running_in = running.filter(|_| running_elsewhere).map(|cwd| {
                let name = self
                    .pages
                    .get(&project)
                    .and_then(|p| {
                        p.folders.items.iter().find(|f| {
                            acta_core::folder_path::same(
                                acta_core::paths::Os::CURRENT,
                                &f.path,
                                &cwd.to_string_lossy(),
                            )
                        })
                    })
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| paths_view::last_segment(&cwd.to_string_lossy()).to_owned());
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .text_color(t.text_tertiary)
                    .child(tr(
                        locale,
                        Key::TerminalRunningIn,
                        &[("name", Arg::Text(&name))],
                    ))
            });
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .min_w_0()
                .child(wrap)
                .children(running_in)
        });
        let action = match &mismatch {
            Mismatch::None => None,
            Mismatch::RestartIn(name) => Some((
                tr(locale, Key::TerminalRestartIn, &[("name", Arg::Text(name))]),
                true,
            )),
            Mismatch::RestartHere => Some((text(locale, Key::TerminalRestartHere), false)),
        };
        let restart_origin = origin.clone();
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .min_w_0()
                .child(
                    type_scale::SMALL
                        .el()
                        .min_w_0()
                        .truncate()
                        .text_color(if marked {
                            t.text_primary
                        } else {
                            t.text_tertiary
                        })
                        .child(Self::context(&origin, locale)),
                )
                .children(switcher)
                .when(waiting, |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .flex_shrink_0()
                            .text_color(t.warning)
                            .child(format!("\u{b7} {}", text(locale, Key::TerminalWaiting))),
                    )
                })
                .children(action.map(|(label, in_page)| {
                    ui::button("terminal-restart-text", label, Variant::Ghost, false, &t)
                        .h(px(24.))
                        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation()
                        })
                        .on_click(cx.listener(move |_, _, _, cx| {
                            if in_page {
                                cx.emit(DockEvent::RestartIn(restart_origin.clone()))
                            } else {
                                cx.emit(DockEvent::RestartHere)
                            }
                        }))
                }))
                .into_any_element(),
        )
    }

    /// The terminal's own header button: Restart here (`rotate-cw`).
    pub fn header_buttons(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        ui::ghost_icon_button(
            "terminal-restart",
            Icon::RotateCw,
            24.,
            false,
            false,
            true,
            &t,
        )
        .tooltip(ui::tip(text(locale, Key::CmdRestartTerminal)))
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation()
        })
        .on_click(cx.listener(|_, _, _, cx| cx.emit(DockEvent::RestartHere)))
        .into_any_element()
    }

    fn project_name(&self, id: ProjectId, cx: &App) -> String {
        self.ws
            .read(cx)
            .projects
            .iter()
            .find(|t| t.project.id == id)
            .map(|t| t.project.title.clone())
            .unwrap_or_default()
    }
}

/// "T-12 Embed the terminal", "E-3 Storage", "project Acta": how the
/// terminal and the Sessions view name a session's focus.
pub fn focus_label(locale: Locale, owner: Owner, name: &str) -> String {
    let id = owner.to_string();
    let key = match owner {
        Owner::Topic(_) => Key::TerminalOnTopic,
        Owner::Epic(_) => Key::TerminalOnEpic,
        Owner::Project(_) => Key::TerminalOnProject,
    };
    tr(
        locale,
        key,
        &[("id", Arg::Text(&id)), ("name", Arg::Text(name))],
    )
}

/// Where the page's last conversation id is kept (encrypted database,
/// local, never exported): opaque, compared and passed to Claude Code only.
fn conversation_key(owner: Owner) -> String {
    format!("terminal.conversation.{owner}")
}

/// The conversation a start from `owner` opens (ADR 0008 amendment): the
/// page's last one when the setting allows it and Claude Code still has
/// its transcript, else a new one under a fresh id, recorded as the page's.
fn conversation_for(
    store: &mut acta_core::Store,
    owner: Owner,
    always_new: bool,
    claude_config: Option<&std::path::Path>,
    cwd: &std::path::Path,
) -> acta_core::Result<Option<Conversation>> {
    let key = conversation_key(owner);
    let last = store.local_value(&key)?.filter(|id| {
        !always_new
            && claude_config.is_some_and(|dir| acta_terminal::transcript_exists(dir, cwd, id))
    });
    if let Some(id) = last {
        return Ok(Some(Conversation::Resume(id)));
    }
    let Some(id) = acta_core::random_uuid() else {
        return Ok(None);
    };
    store.set_local_value(&key, Some(&id))?;
    Ok(Some(Conversation::New(id)))
}

/// Acta's plugin for the sessions it starts (`--plugin-dir`): written by
/// `acta setup --files-only` when it is not there yet. `None` when it
/// cannot be written; the session then has only what setup installed.
fn plugin_dir() -> Option<PathBuf> {
    let dir = crate::claude_setup::plugin_dir()?;
    let manifest = dir.join(".claude-plugin").join("plugin.json");
    if !manifest.is_file() {
        crate::claude_setup::write_plugin_files();
    }
    manifest.is_file().then_some(dir)
}

fn theme_for(mode: Mode) -> TerminalTheme {
    match mode {
        Mode::Dark => TerminalTheme::dark(),
        Mode::Light => TerminalTheme::light(),
    }
}

/// The empty-state message on the terminal's surface: padding 12, text at
/// most 480 wide (screens.md 8.11).
fn message(t: &Tokens, surface: gpui_kit::Hsla) -> gpui_kit::Div {
    let _ = t;
    div()
        .flex_1()
        .flex()
        .flex_col()
        .p(px(12.))
        .max_w(px(480. + 24.))
        .bg(surface)
}

fn checkbox(checked: bool, t: &Tokens) -> gpui_kit::Div {
    div()
        .size(px(16.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS_XS))
        .when(checked, |this| {
            this.bg(t.accent)
                .child(icon(Icon::Check, 12., t.text_on_accent))
        })
        .when(!checked, |this| {
            this.border_1().border_color(t.border_strong)
        })
}

impl Render for TerminalDock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_dock(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{EpicId, TopicId};
    use acta_terminal::launch::{ClaudeProgram, build_claude_launch};

    fn git(dir: &Path, args: &[&str]) {
        let out = acta_core::git_cmd::command(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn the_launch_plan_sets_the_working_directory_and_the_add_dir_order() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let main = root.join("acta");
        std::fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q", "-b", "main"]);
        git(
            &main,
            &[
                "-c",
                "user.name=Alexandre",
                "-c",
                "user.email=alexandre@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "First",
            ],
        );
        let wt = root.join("embed-terminal");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "topic/embed",
                wt.to_str().unwrap(),
            ],
        );
        let notes = root.join("notes");
        let fork = root.join("alacritty-fork");
        let docs = root.join("acta-docs");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::create_dir_all(&fork).unwrap();
        let f = |path: &Path, own: bool, active: bool| PlanFolder {
            path: path.to_path_buf(),
            own,
            active,
        };
        // screens.md 8.11: a topic in its own worktree with a notes folder,
        // its epic's fork, its project's checkout and a docs folder that
        // is not on this computer.
        let folders = [
            f(&wt, true, true),
            f(&notes, true, false),
            f(&fork, false, false),
            f(&main, false, false),
            f(&docs, false, false),
        ];
        let plan = launch_plan(&folders);
        let paths = launch_folders(&plan, Some(&wt)).unwrap();
        let program = ClaudeProgram::Exe(PathBuf::from("claude"));
        let launch = build_claude_launch(
            &program,
            &paths,
            &Focus::default(),
            &ClaudeOptions::default(),
            |p| p.is_dir(),
        )
        .unwrap();
        assert_eq!(
            launch.spec.cwd, wt,
            "Claude Code starts in the active folder"
        );
        let s = |p: &Path| p.to_string_lossy().into_owned();
        assert_eq!(
            launch.spec.args,
            [
                "--add-dir".to_owned(),
                s(&notes),
                "--add-dir".to_owned(),
                s(&fork)
            ],
            "the others in list order, without the main checkout or the missing one"
        );
        assert_eq!(
            plan.left_out,
            [
                (main.clone(), LeftOut::OtherCheckout),
                (docs.clone(), LeftOut::NotFound)
            ]
        );

        // The main checkout made active: the worktree, an own folder, is
        // passed.
        let folders = [f(&wt, true, false), f(&main, false, true)];
        let plan = launch_plan(&folders);
        assert_eq!(
            launch_folders(&plan, Some(&main)).unwrap(),
            [main.clone(), wt.clone()]
        );

        // An active folder not found stops the launch; never elsewhere.
        let folders = [f(&docs, true, true), f(&notes, true, false)];
        let plan = launch_plan(&folders);
        assert_eq!(
            launch_folders(&plan, Some(&docs)),
            Err(LaunchError::PrimaryMissing(docs.clone()))
        );
        assert_eq!(
            launch_folders(&LaunchPlan::default(), None),
            Err(LaunchError::NoFolder)
        );
    }

    #[test]
    fn sessions_are_named_by_their_focus() {
        assert_eq!(
            focus_label(Locale::En, Owner::Topic(TopicId(12)), "Embed the terminal"),
            "T-12 Embed the terminal"
        );
        assert_eq!(
            focus_label(Locale::Fr, Owner::Project(ProjectId(1)), "Acta"),
            "projet Acta"
        );
        let origin = Origin {
            owner: Owner::Epic(EpicId(3)),
            project: ProjectId(1),
            name: "Storage".into(),
        };
        assert_eq!(
            TerminalDock::context(&origin, Locale::En),
            "Claude \u{b7} E-3 Storage"
        );
    }

    #[test]
    fn a_page_resumes_its_conversation_only_while_claude_code_has_it() {
        let mut temp = crate::db::test_support::temp_store();
        let store = &mut temp.store;
        // A Claude Code configuration of the test's own, never ~/.claude.
        let claude = tempfile::tempdir().unwrap();
        let config = Some(claude.path());
        let owner = Owner::Topic(TopicId(12));
        let cwd = PathBuf::from(if cfg!(windows) {
            "C:\\Users\\alexandre\\acta"
        } else {
            "/home/alexandre/acta"
        });
        // First start: a new conversation, recorded for the page.
        let first = conversation_for(store, owner, false, config, &cwd)
            .unwrap()
            .unwrap();
        let Conversation::New(id) = first else {
            panic!("{first:?}")
        };
        assert!(acta_terminal::launch::is_uuid(&id));
        assert_eq!(
            store.local_value(&conversation_key(owner)).unwrap(),
            Some(id.clone())
        );
        // No transcript yet: a new one again.
        let again = conversation_for(store, owner, false, config, &cwd)
            .unwrap()
            .unwrap();
        let Conversation::New(id) = again else {
            panic!("{again:?}")
        };
        // Claude Code wrote the transcript under this folder's directory:
        // the next start resumes it.
        let project = claude
            .path()
            .join("projects")
            .join(acta_terminal::launch::project_dir_name(&cwd).unwrap());
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(format!("{id}.jsonl")), "").unwrap();
        assert_eq!(
            conversation_for(store, owner, false, config, &cwd).unwrap(),
            Some(Conversation::Resume(id.clone()))
        );
        // The page's folder moved: that transcript is another folder's, so
        // a new conversation starts and becomes the page's.
        let moved = PathBuf::from(if cfg!(windows) {
            "C:\\Users\\alexandre\\elsewhere"
        } else {
            "/home/alexandre/elsewhere"
        });
        let fresh = conversation_for(store, owner, false, config, &moved)
            .unwrap()
            .unwrap();
        let Conversation::New(fresh) = fresh else {
            panic!("{fresh:?}")
        };
        assert_ne!(fresh, id);
        assert_eq!(
            store.local_value(&conversation_key(owner)).unwrap(),
            Some(fresh)
        );
        // The setting asks for a new conversation every time.
        let always = conversation_for(store, owner, true, config, &cwd)
            .unwrap()
            .unwrap();
        assert!(matches!(always, Conversation::New(other) if other != id));
    }

    #[test]
    fn the_focus_names_every_level_of_the_page() {
        let f = focus_of(Owner::Topic(TopicId(42)), ProjectId(1), Some(EpicId(3)));
        assert_eq!(f.project, "P-1");
        assert_eq!(f.epic.as_deref(), Some("E-3"));
        assert_eq!(f.topic.as_deref(), Some("T-42"));
        let f = focus_of(Owner::Project(ProjectId(2)), ProjectId(2), None);
        assert_eq!((f.epic, f.topic), (None, None));
    }

    #[test]
    fn a_queued_command_survives_only_the_folder_question() {
        let p = || Some("/acta:triage".to_owned());
        assert_eq!(queued_prompt(&Err(LaunchError::NoFolder), p()), p());
        assert_eq!(queued_prompt(&Err(LaunchError::ClaudeNotFound), p()), None);
        assert_eq!(
            queued_prompt(&Err(LaunchError::PrimaryMissing(PathBuf::new())), p()),
            None
        );
        assert_eq!(queued_prompt(&Ok(()), p()), None, "the session has it");
    }

    #[test]
    fn a_command_is_typed_only_into_an_idle_session() {
        assert_eq!(send_rule(true, true), Send::Typed);
        assert_eq!(send_rule(true, false), Send::Busy, "never mid-task");
        assert_eq!(send_rule(false, false), Send::Started);
        assert_eq!(send_rule(false, true), Send::Started);
    }
}
