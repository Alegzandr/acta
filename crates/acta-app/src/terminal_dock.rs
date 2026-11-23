//! The terminal tool windows (docs/design/layout.md 4.5 and 4.5.1;
//! components-folders.md 5, Terminal launch; screens.md 8.4, 8.11, 8.36).
//! The Terminal view shows the Claude Code sessions of the current page:
//! each session belongs to the project, epic or topic it was started from,
//! several per page, numbered; navigation never stops, restarts or moves
//! one. A session starts by the launch plan of its page (ADR 0011: the
//! active folder is the working directory, the other folders the page sees
//! are `--add-dir`, in list order, except a folder not found and another
//! checkout of the same repository), with the focus in `ACTA_*`. A folder
//! switch or a commit-mode change marks the header with "Restart in ...".
//! The same dock, built `free`, is the Claude Code (no project) view:
//! plain `claude` in the home folder, with nothing of Acta's.

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

use crate::folders_model as fm;
use crate::folders_view::{self, SwitchActions, SwitchEntry, SwitchGroup};
use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::paths_view;
use crate::record::Folders;
use crate::settings;
use crate::theme::{Appearance, Mode, RADIUS_XS, Tokens};
use crate::tools_model::{self as tm, Holder, InstanceKey, Instances};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{CommitColumns, FolderRef, Undo, Workspace};

/// Where a launch comes from: the page's record and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub owner: Owner,
    pub project: ProjectId,
    pub name: String,
}

enum Pane {
    /// Folders and the program are being resolved (no message before 400 ms).
    Starting { origin: Option<Origin> },
    Running {
        view: Entity<TerminalView>,
        /// The page it belongs to; `None` for Claude Code (no project).
        origin: Option<Origin>,
        /// Where it started: the working directory, the active folder
        /// record and its commit columns then (a change marks the header).
        start: Start,
        /// `ACTA_LAUNCH`: finds this session's presence row (empty for a
        /// free session, which has none).
        token: String,
        /// The conversation the page resumes next time; follows `/clear`.
        conversation: Option<String>,
        /// It holds the page's "last conversation" (the page's first
        /// session); further sessions start their own and record nothing.
        page_conversation: bool,
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
    NotFound { origin: Option<Origin> },
}

impl Pane {
    fn origin(&self) -> Option<&Origin> {
        match self {
            Pane::Starting { origin } | Pane::NotFound { origin } => origin.as_ref(),
            Pane::Running { origin, .. } => origin.as_ref(),
            Pane::Ask { origin, .. } | Pane::Missing { origin, .. } => Some(origin),
        }
    }

    fn live_view(&self) -> Option<&Entity<TerminalView>> {
        match self {
            Pane::Running { view, .. } => Some(view),
            _ => None,
        }
    }
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
    /// Every folder the session was launched with: the working directory
    /// and its `--add-dir` folders.
    pub dirs: Vec<PathBuf>,
    /// Folders the page sees that were not found and were left out of
    /// the session: marked in the header's switcher, never a toast.
    pub left_out: Vec<PathBuf>,
    pub folder: Option<FolderId>,
    pub commit: Option<CommitColumns>,
}

/// The folders of a session's page, as the header shows them: refreshed
/// whenever the store changes.
#[derive(Clone, Debug, Default)]
struct SessionPage {
    folders: Folders,
    /// What the disk says of each folder (its automatic icon).
    disk: HashMap<String, fm::Disk>,
    /// The commit columns of the folder the session started in, now.
    commit: Option<CommitColumns>,
}

/// The folders Acta's terminal sessions were launched with (working
/// directory and `--add-dir` folders), each with its launch token:
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

/// Gap between the parts of the terminal header's context.
pub const HEADER_GAP: f32 = 8.;
/// The context label is not truncated below this while something else
/// can still give way; under [`CONTEXT_SHOWN`] it is left out.
pub const CONTEXT_MIN: f32 = 56.;
pub const CONTEXT_SHOWN: f32 = 24.;
/// The switcher besides its name: padding 4 + icon 16 + gap 6, gap 6 +
/// chevron 12 + padding 4.
pub const SWITCHER_CHROME: f32 = 48.;
/// The switcher at its smallest: its icon and chevron.
pub const SWITCHER_MIN: f32 = SWITCHER_CHROME;
/// What the header keeps for the context at its smallest (the switcher,
/// the "running in" mark and the restart icon, with their gaps): the view
/// tabs give way to their chevron before it.
pub const HEADER_RESERVE: f32 = SWITCHER_MIN + 2. * (ICON_SLOT + HEADER_GAP) + HEADER_GAP;
/// An icon slot: the "(running in ...)" mark or the restart icon button.
pub const ICON_SLOT: f32 = 24.;

/// The full widths of the header context's parts; `None` for a part not
/// shown, a switcher of 0 when there is none.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeaderParts {
    pub context: f32,
    pub switcher: f32,
    /// The switcher at its smallest (larger with the left-out mark).
    pub switcher_min: f32,
    pub running: Option<f32>,
    pub waiting: Option<f32>,
    pub restart: Option<f32>,
}

/// How the parts share the width: the context's and the switcher's widths,
/// and whether "(running in ...)" and the restart action keep their words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderFit {
    pub context: f32,
    pub switcher: f32,
    pub running_text: bool,
    pub restart_text: bool,
}

impl HeaderFit {
    fn total(&self, p: &HeaderParts) -> f32 {
        let parts = [
            (self.context >= CONTEXT_SHOWN).then_some(self.context),
            (p.switcher > 0.).then_some(self.switcher),
            p.running
                .map(|w| if self.running_text { w } else { ICON_SLOT }),
            p.waiting,
            p.restart
                .map(|w| if self.restart_text { w } else { ICON_SLOT }),
        ];
        let shown: Vec<f32> = parts.into_iter().flatten().collect();
        shown.iter().sum::<f32>() + HEADER_GAP * shown.len().saturating_sub(1) as f32
    }
}

/// The terminal header's context in `available` (layout.md 4.5), giving
/// way in this order: the context label truncates (to [`CONTEXT_MIN`]),
/// then "(running in ...)" becomes an icon with its words as tooltip,
/// then "Restart in ..." becomes an icon button, then the switcher's name
/// truncates, then the label goes. The sum never exceeds `available` while
/// the icons fit; nothing is drawn over anything.
pub fn fit_header(available: f32, p: HeaderParts) -> HeaderFit {
    let mut fit = HeaderFit {
        context: p.context,
        switcher: p.switcher,
        running_text: true,
        restart_text: true,
    };
    let fits = |f: &HeaderFit| f.total(&p) <= available + 0.5;
    // The context takes what the others leave, between `min` and its size.
    let settle = |f: &mut HeaderFit, min: f32| {
        let others = HeaderFit { context: 0., ..*f }.total(&p);
        let others = if others > 0. { others + HEADER_GAP } else { 0. };
        f.context = (available - others).clamp(min.min(p.context), p.context);
    };

    let min = CONTEXT_MIN;
    settle(&mut fit, min);
    if fits(&fit) {
        return fit;
    }
    for step in 0..3 {
        match step {
            0 if p.running.is_some() => fit.running_text = false,
            1 if p.restart.is_some() => fit.restart_text = false,
            2 if p.switcher > 0. => {
                let others = HeaderFit {
                    switcher: 0.,
                    context: min.min(p.context),
                    ..fit
                }
                .total(&p)
                    + HEADER_GAP;
                fit.switcher =
                    (available - others).clamp(p.switcher_min.min(p.switcher), p.switcher);
            }
            _ => continue,
        }
        settle(&mut fit, min);
        if fits(&fit) {
            return fit;
        }
    }
    // Last, the label shrinks below its minimum and goes.
    settle(&mut fit, 0.);
    if fit.context < CONTEXT_SHOWN {
        fit.context = 0.;
    }
    fit
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
    /// Restart, or "Restart in {folder}", of an instance: the shell asks
    /// first while a process runs (interaction.md 6.4).
    Restart(InstanceKey),
    /// "Edit folders..." and "Add folder..." of the header switcher: on
    /// the session's page.
    EditFolders(Origin),
    AddFolder(Origin),
    /// The one-off session of a Ready row ended (ADR 0013): the row
    /// checks again.
    OneOffEnded(crate::ready_model::Row),
    /// "Claude Code (no project)" on a page that owns nothing: show that
    /// view.
    ShowFree,
}

/// A one-off command session (ADR 0013): a program started directly in
/// the PTY for a Ready row, with a title; it ends with the process and is
/// never restarted.
struct OneOff {
    row: crate::ready_model::Row,
    title: String,
    view: Entity<TerminalView>,
    ended: bool,
    _sub: Subscription,
}

pub struct TerminalDock {
    ws: Entity<Workspace>,
    /// The Claude Code (no project) view: sessions held by nobody.
    free: bool,
    panes: Instances<Pane>,
    /// A folder chosen in the prompt and not saved: used until Acta quits.
    unsaved: HashMap<ProjectId, PathBuf>,
    /// The newest launch of each instance.
    generations: HashMap<InstanceKey, u64>,
    /// The current page (layout.md 4.5.1), set by the shell; `None` on a
    /// page that owns nothing.
    current: Option<Origin>,
    /// The current page's active folder (name, path), for its empty state.
    current_folder: Option<(String, String)>,
    focus: FocusHandle,
    /// The empty state's "Start Claude Code here", focused so `Enter`
    /// starts a session.
    start_focus: FocusHandle,
    text_size: f32,
    mode: Option<Mode>,
    /// The folders of each session's page, for the header.
    pages: HashMap<InstanceKey, SessionPage>,
    /// The header switcher's menu, with its highlighted entry.
    switcher: Option<usize>,
    switch_anchor: crate::menu::AnchorBounds,
    /// The not-found message's "Or start in another folder:" switcher.
    missing_switcher: Option<usize>,
    missing_anchor: crate::menu::AnchorBounds,
    /// The one-off session, and whether it is the one shown.
    one_off: Option<OneOff>,
    show_one_off: bool,
    seen_revision: u64,
    _refresh: Option<gpui_kit::Task<()>>,
    _folder: Option<gpui_kit::Task<()>>,
    _ws_sub: Option<Subscription>,
}

impl EventEmitter<DockEvent> for TerminalDock {}

impl Focusable for TerminalDock {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if let Some(one) = self.one_off.as_ref().filter(|_| self.show_one_off) {
            return one.view.read(cx).focus_handle(cx);
        }
        match self.shown().and_then(|k| self.panes.get(k)) {
            Some(Pane::Running { view, .. }) => view.read(cx).focus_handle(cx),
            None if self.holder().is_some() => self.start_focus.clone(),
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
    pub fn new(ws: Entity<Workspace>, free: bool, cx: &mut Context<Self>) -> Self {
        let text_size = settings::current(cx)
            .terminal_text_size
            .map(f32::from)
            .unwrap_or(acta_terminal::view::DEFAULT_TEXT_SIZE);
        let sub = cx.observe(&ws, |dock, ws, cx| {
            let revision = ws.read(cx).revision;
            if revision != dock.seen_revision {
                dock.seen_revision = revision;
                dock.refresh_pages(cx);
                dock.refresh_current_folder(cx);
            }
        });
        let max = if free { tm::FREE_MAX } else { tm::CLAUDE_MAX };
        Self {
            ws,
            free,
            panes: Instances::new(max),
            unsaved: HashMap::new(),
            generations: HashMap::new(),
            current: None,
            current_folder: None,
            focus: cx.focus_handle(),
            start_focus: cx.focus_handle(),
            text_size,
            mode: None,
            pages: HashMap::new(),
            switcher: None,
            switch_anchor: Default::default(),
            missing_switcher: None,
            missing_anchor: Default::default(),
            one_off: None,
            show_one_off: false,
            seen_revision: 0,
            _refresh: None,
            _folder: None,
            _ws_sub: Some(sub),
        }
    }

    /// Who the view shows instances of: the current page's record, the
    /// free holder for Claude Code (no project), none on a page that owns
    /// nothing.
    pub fn holder(&self) -> Option<Holder> {
        if self.free {
            Some(Holder::Free)
        } else {
            self.current.as_ref().map(|o| Holder::Page(o.owner))
        }
    }

    /// The instance in view.
    pub fn shown(&self) -> Option<InstanceKey> {
        self.panes.selected(self.holder()?)
    }

    /// The page the shell shows now (layout.md 4.5.1): the view follows
    /// it on the next frame; nothing is stopped or moved.
    pub fn set_current(&mut self, origin: Option<Origin>, cx: &mut Context<Self>) {
        if self.current == origin {
            return;
        }
        let owner_changed =
            self.current.as_ref().map(|o| o.owner) != origin.as_ref().map(|o| o.owner);
        self.current = origin;
        if owner_changed {
            self.switcher = None;
            self.current_folder = None;
            self.refresh_current_folder(cx);
        }
        cx.notify();
    }

    pub fn current(&self) -> Option<&Origin> {
        self.current.as_ref()
    }

    /// The current page's active folder, for its empty state (off the UI
    /// thread).
    fn refresh_current_folder(&mut self, cx: &mut Context<Self>) {
        let Some(owner) = self
            .current
            .as_ref()
            .map(|o| o.owner)
            .filter(|_| !self.free)
        else {
            return;
        };
        let task = self.ws.read(cx).db.run(cx, move |store| {
            // The names as the folder list shows them.
            let folders = crate::record::load_folders(store, owner)?;
            Ok(folders.active().map(|f| (f.name.clone(), f.path.clone())))
        });
        self._folder = Some(cx.spawn(async move |dock, cx| {
            let Ok(folder) = task.await else { return };
            let _ = dock.update(cx, |dock, cx| {
                if dock.current.as_ref().map(|o| o.owner) == Some(owner) {
                    dock.current_folder = folder;
                    cx.notify();
                }
            });
        }));
    }

    /// The folders of every session's page (and of a page whose active
    /// folder was not found) read again off the UI thread, with the
    /// existence of each.
    fn refresh_pages(&mut self, cx: &mut Context<Self>) {
        let wanted: Vec<(InstanceKey, Owner, Option<FolderId>)> = self
            .panes
            .iter()
            .filter_map(|(key, pane)| match pane {
                Pane::Running {
                    origin: Some(origin),
                    start,
                    ..
                } => Some((key, origin.owner, start.folder)),
                // The not-found message offers the page's other folders.
                Pane::Missing { origin, .. } => Some((key, origin.owner, None)),
                _ => None,
            })
            .collect();
        if wanted.is_empty() {
            self.pages.clear();
            return;
        }
        let task = self.ws.read(cx).db.run(cx, move |store| {
            let mut out = Vec::new();
            for (key, owner, folder) in wanted {
                let Ok(folders) = crate::record::load_folders(store, owner) else {
                    continue;
                };
                let commit = folder
                    .and_then(|id| store.folder(id).ok())
                    .map(|f| (f.commit_mode, f.commit_as_user, f.commit_on_default));
                out.push((
                    key,
                    SessionPage {
                        folders,
                        disk: HashMap::new(),
                        commit,
                    },
                ));
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
                        page.disk = fm::read_disk(&page.folders.items);
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

    /// Whether the holder is at its limit (the "New" commands disabled).
    pub fn at_limit(&self) -> bool {
        self.holder().is_some_and(|h| self.panes.at_limit(h))
    }

    /// Whether a process runs in the instance (the restart confirmation
    /// of interaction.md 6.4 asks only then).
    pub fn is_running(&self, key: InstanceKey, cx: &App) -> bool {
        self.panes
            .get(key)
            .and_then(Pane::live_view)
            .is_some_and(|view| view.read(cx).is_running())
    }

    /// Start a new session on the current page ("Start Claude Code here",
    /// ``Ctrl+Shift+` ``, the header's `plus`), or a free one. `None` at
    /// the limit or on a page that owns nothing.
    pub fn start_here(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<InstanceKey> {
        let origin = if self.free {
            None
        } else {
            Some(self.current.clone()?)
        };
        self.start(origin, None, window, cx)
    }

    /// Open a new instance for `origin` (`None`: a free session) and
    /// launch it, on `prompt` if given.
    fn start(
        &mut self,
        origin: Option<Origin>,
        prompt: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<InstanceKey> {
        let holder = match &origin {
            Some(o) => Holder::Page(o.owner),
            None => Holder::Free,
        };
        let key = self
            .panes
            .open(
                holder,
                Pane::Starting {
                    origin: origin.clone(),
                },
            )
            .ok()?;
        self.show_one_off = false;
        self.launch(key, origin, prompt, window, cx);
        Some(key)
    }

    /// End the instance's session and start it again from its own page,
    /// with the page's active folder now (Restart, "Restart in ...").
    pub fn restart(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(origin) = self.panes.get(key).map(|p| p.origin().cloned()) else {
            return;
        };
        self.show_one_off = false;
        self.panes.select(key);
        self.launch(key, origin, None, window, cx);
    }

    /// Run `command` (a slash command) in the page session of `origin` by
    /// [`send_rule`]: the selected session of the page. On `Busy` nothing
    /// happens; the caller confirms and calls
    /// [`TerminalDock::restart_on`] with the returned key.
    pub fn send(
        &mut self,
        origin: Origin,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Send, Option<InstanceKey>) {
        let holder = Holder::Page(origin.owner);
        let key = self.panes.selected(holder);
        let live = key
            .and_then(|k| self.panes.get(k))
            .and_then(Pane::live_view)
            .filter(|view| view.read(cx).is_running())
            .cloned();
        let idle = live
            .as_ref()
            .is_some_and(|view| view.read(cx).idle_at_prompt());
        let step = send_rule(live.is_some(), idle);
        let key = match (step, live) {
            (Send::Typed, Some(view)) => {
                view.update(cx, |view, cx| view.send_line(command, cx));
                view.read(cx).focus_handle(cx).focus(window, cx);
                key
            }
            (Send::Started, _) => match key {
                // An ended session of the page starts again on it.
                Some(key) => {
                    self.restart_on(key, command, window, cx);
                    Some(key)
                }
                None => self.start(Some(origin), Some(command.to_owned()), window, cx),
            },
            _ => key,
        };
        cx.notify();
        (step, key)
    }

    /// The text of the instance in view, else of the first session of the
    /// project's page (development checks only).
    #[cfg(debug_assertions)]
    fn dev_view(&self, project: ProjectId) -> Option<Entity<TerminalView>> {
        let key = self
            .shown()
            .filter(|k| self.panes.get(*k).and_then(Pane::live_view).is_some())
            .or_else(|| self.panes.selected(Holder::Page(Owner::Project(project))))?;
        self.panes.get(key).and_then(Pane::live_view).cloned()
    }

    /// The project's terminal screen as text (development checks only).
    #[cfg(debug_assertions)]
    pub fn dev_screen(&self, project: ProjectId, cx: &App) -> Option<String> {
        self.dev_view(project).map(|v| v.read(cx).screen_text())
    }

    /// Type `text` into the project's session if Claude Code is idle
    /// (development checks only).
    #[cfg(debug_assertions)]
    pub fn dev_type_if_idle(&mut self, project: ProjectId, text: &str, cx: &mut App) -> bool {
        let Some(view) = self.dev_view(project) else {
            return false;
        };
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
        if let Some(view) = self.dev_view(project) {
            let bytes = bytes.to_vec();
            view.update(cx, |view, _| view.send_raw(bytes));
        }
    }

    /// End the instance's session and start Claude Code on `command`.
    pub fn restart_on(
        &mut self,
        key: InstanceKey,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(origin) = self.panes.get(key).map(|p| p.origin().cloned()) else {
            return;
        };
        self.panes.select(key);
        self.launch(key, origin, Some(command.to_owned()), window, cx);
    }

    /// Launch the instance `key` from `origin` (a free session without
    /// one), on `prompt` if given. The prompt goes into this launch only:
    /// kept on the Ask pane if a folder is asked first, dropped on any
    /// other outcome.
    fn launch(
        &mut self,
        key: InstanceKey,
        origin: Option<Origin>,
        prompt: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Each launch of an instance gets a number; a result that arrives
        // after a newer launch started is dropped.
        let generation = {
            let n = self.generations.entry(key).or_insert(0);
            *n += 1;
            *n
        };
        // The previous session (if any) ends when its view is dropped.
        self.panes.replace(
            key,
            Pane::Starting {
                origin: origin.clone(),
            },
        );
        cx.notify();
        match origin {
            Some(origin) => self.launch_page(key, generation, origin, prompt, window, cx),
            None => self.launch_free(key, generation, window, cx),
        }
    }

    fn launch_free(
        &mut self,
        key: InstanceKey,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |dock, cx| {
            // The search for `claude` reads the disk: off the UI thread.
            let spec = cx
                .background_executor()
                .spawn(async move {
                    let program = acta_terminal::launch::resolve_claude()?;
                    let home = PathBuf::from(paths_view::home()?);
                    let mut spec = free_spec(&program, home)?;
                    if let Some(path) = acta_core::prereqs::locate::system_fresh_path() {
                        acta_terminal::launch::set_path(
                            &mut spec.env,
                            cfg!(windows),
                            &acta_terminal::launch::env_names(),
                            &path,
                        );
                    }
                    Some(spec)
                })
                .await;
            let _ = dock.update_in(cx, |dock, window, cx| {
                if dock.generations.get(&key) != Some(&generation) {
                    return;
                }
                let pane = match spec {
                    Some(spec) => {
                        let start = Start {
                            cwd: spec.cwd.clone(),
                            dirs: vec![spec.cwd.clone()],
                            left_out: Vec::new(),
                            folder: None,
                            commit: None,
                        };
                        dock.running(
                            key,
                            None,
                            start,
                            spec,
                            String::new(),
                            None,
                            false,
                            window,
                            cx,
                        )
                    }
                    None => Pane::NotFound { origin: None },
                };
                dock.panes.replace(key, pane);
                cx.notify();
            });
        })
        .detach();
    }

    fn launch_page(
        &mut self,
        key: InstanceKey,
        generation: u64,
        origin: Origin,
        prompt: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = origin.project;
        let owner = origin.owner;
        let unsaved = self.unsaved.get(&project).cloned();
        let queued = prompt.clone();
        // The page's first session holds its last conversation; every
        // further one starts a new conversation of its own.
        let page_conversation = !self.panes.iter().any(|(k, p)| {
            k.holder == key.holder
                && k != key
                && matches!(
                    p,
                    Pane::Running {
                        page_conversation: true,
                        ..
                    }
                )
        });
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
                Some(f) if page_conversation => {
                    conversation_for(store, owner, new_conversations, config.as_deref(), &f.path)?
                }
                Some(_) => acta_core::random_uuid().map(Conversation::New),
                None => None,
            };
            Ok((folders, active, epic, conversation))
        });
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
                    // The fresh `PATH` (ADR 0013): a tool installed while Acta
                    // runs is found by Claude Code.
                    if let Some(path) = acta_core::prereqs::locate::system_fresh_path() {
                        acta_terminal::launch::set_path(
                            &mut launch.spec.env,
                            cfg!(windows),
                            &acta_terminal::launch::env_names(),
                            &path,
                        );
                    }
                    // Not found: reported; another checkout: the list says so.
                    let missing = plan
                        .left_out
                        .into_iter()
                        .filter(|(_, why)| *why == LeftOut::NotFound)
                        .map(|(p, _)| p);
                    launch.left_out = missing.chain(launch.left_out).collect();
                    let dirs = paths
                        .into_iter()
                        .filter(|p| !launch.left_out.contains(p))
                        .collect::<Vec<_>>();
                    Ok((launch, dirs))
                })
                .await;
            let _ = dock.update_in(cx, |dock, window, cx| {
                if dock.generations.get(&key) != Some(&generation) {
                    return;
                }
                let pane = match result {
                    Ok((launch, dirs)) => {
                        let start = Start {
                            cwd: launch.spec.cwd.clone(),
                            dirs,
                            left_out: launch.left_out.clone(),
                            folder: start_folder,
                            commit: start_commit,
                        };
                        dock.running(
                            key,
                            Some(origin),
                            start,
                            launch.spec,
                            token,
                            conversation_id,
                            page_conversation,
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
                    Err(LaunchError::ClaudeNotFound) => Pane::NotFound {
                        origin: Some(origin),
                    },
                };
                dock.panes.replace(key, pane);
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
        key: InstanceKey,
        origin: Option<Origin>,
        start: Start,
        spec: acta_terminal::LaunchSpec,
        token: String,
        conversation: Option<String>,
        page_conversation: bool,
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
        let sub = cx.subscribe_in(
            &view,
            window,
            move |dock, _, event: &TerminalEvent, window, cx| {
                match event {
                    // Enter on an ended session: it starts again from its
                    // own page, with that page's folders now.
                    TerminalEvent::RestartRequested => dock.restart(key, window, cx),
                    TerminalEvent::TextSizeChanged(size) => {
                        dock.text_size = *size;
                        let stored = size.round() as u8;
                        settings::change(cx, |prefs| prefs.terminal_text_size = Some(stored));
                    }
                    _ => {}
                }
            },
        );
        // Focus moves into the new session only while it is in view.
        if self.shown() == Some(key) {
            view.read(cx).focus_handle(cx).focus(window, cx);
        }
        Pane::Running {
            view,
            origin,
            start,
            token,
            conversation,
            page_conversation,
            _sub: sub,
        }
    }

    /// The Sessions view finds Acta's terminals by their launch tokens.
    fn publish_tokens(&self, cx: &mut App) {
        if self.free {
            return;
        }
        let tokens = self
            .panes
            .iter()
            .filter_map(|(key, pane)| match pane {
                Pane::Running { token, .. } => Some(crate::sessions_view::InActa {
                    token: token.clone(),
                    key,
                    several: self.panes.numbers(key.holder).len() > 1,
                }),
                _ => None,
            })
            .collect();
        cx.set_global(crate::sessions_view::TerminalTokens(tokens));
        let running = self
            .panes
            .iter()
            .flat_map(|(_, pane)| match pane {
                Pane::Running { start, token, .. } => start
                    .dirs
                    .iter()
                    .map(|d| (d.clone(), token.clone()))
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        cx.set_global(RunningIn(running));
    }

    /// The launch token of a running session.
    pub fn token(&self, key: InstanceKey) -> Option<&str> {
        match self.panes.get(key) {
            Some(Pane::Running { token, .. }) if !token.is_empty() => Some(token),
            _ => None,
        }
    }

    /// "End session" (Sessions view, the view menu): ends the terminal's
    /// process; the pane keeps its last screen and restarts on Enter.
    pub fn end(&mut self, key: InstanceKey, cx: &mut Context<Self>) {
        if let Some(view) = self.panes.get(key).and_then(Pane::live_view) {
            view.update(cx, |view, cx| view.end(cx));
        }
    }

    /// The instance's `x`: its process ends and it leaves the page; the
    /// last one gone, the page shows its empty state.
    pub fn close(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let had_focus = self
            .panes
            .get(key)
            .and_then(Pane::live_view)
            .is_some_and(|v| v.read(cx).focus_handle(cx).contains_focused(window, cx));
        self.generations.remove(&key);
        self.pages.remove(&key);
        drop(self.panes.close(key));
        self.publish_tokens(cx);
        if had_focus {
            let handle = self.focus_handle(cx);
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    /// Show the instance without starting anything.
    pub fn select(&mut self, key: InstanceKey, cx: &mut Context<Self>) {
        if self.panes.select(key) {
            self.show_one_off = false;
            cx.notify();
        }
    }

    /// "Show terminal" for `owner`'s page (layout.md 4.5.1, the page
    /// line): the session waiting for the user, else the one used last.
    pub fn select_page_session(&mut self, owner: Owner, cx: &mut Context<Self>) {
        let holder = Holder::Page(owner);
        let waiting = self
            .panes
            .keys_of(holder)
            .into_iter()
            .find(|k| self.session_state(*k, cx) == Some(SessionState::Waiting));
        if let Some(key) = waiting.or_else(|| self.panes.selected(holder)) {
            self.select(key, cx);
        }
    }

    /// `Ctrl+PageDown` / `Ctrl+PageUp`: the next or previous instance.
    pub fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(holder) = self.holder() else { return };
        if self.panes.step(holder, forward).is_some() {
            self.show_one_off = false;
            let handle = self.focus_handle(cx);
            window.focus(&handle, cx);
            cx.notify();
        }
    }

    /// Start a one-off session (ADR 0013) and show it: `spec` runs the
    /// program itself, ends with it, and Enter does not restart it. A
    /// previous one-off session ends when this one replaces it.
    pub fn open_one_off(
        &mut self,
        row: crate::ready_model::Row,
        title: String,
        spec: acta_terminal::LaunchSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = cx.global::<Appearance>().mode;
        let size = self.text_size;
        let ended = text(settings::locale(cx), Key::TerminalEndedOnce);
        let view = cx.new(|cx| {
            let mut view = TerminalView::spawn(spec, cx);
            view.set_theme(theme_for(mode), cx);
            view.set_text_size(size, cx);
            view.set_ended_message(ended.into(), cx);
            view
        });
        let sub = cx.subscribe_in(
            &view,
            window,
            move |dock, _, event: &TerminalEvent, _, cx| {
                match event {
                    TerminalEvent::Exited(_) => {
                        if let Some(one) = dock.one_off.as_mut()
                            && !one.ended
                        {
                            one.ended = true;
                            cx.emit(DockEvent::OneOffEnded(one.row));
                            cx.notify();
                        }
                    }
                    TerminalEvent::TextSizeChanged(size) => {
                        dock.text_size = *size;
                        let stored = size.round() as u8;
                        settings::change(cx, |prefs| prefs.terminal_text_size = Some(stored));
                    }
                    // No restart: it was one command.
                    _ => {}
                }
            },
        );
        view.read(cx).focus_handle(cx).focus(window, cx);
        self.one_off = Some(OneOff {
            row,
            title,
            view,
            ended: false,
            _sub: sub,
        });
        self.show_one_off = true;
        cx.notify();
    }

    /// Show the one-off session, if there is one.
    pub fn select_one_off(&mut self, cx: &mut Context<Self>) {
        if self.one_off.is_some() {
            self.show_one_off = true;
            cx.notify();
        }
    }

    /// Close an ended one-off session: the page's session shows again.
    fn close_one_off(&mut self, cx: &mut Context<Self>) {
        if self.one_off.as_ref().is_some_and(|o| o.ended) {
            self.one_off = None;
            self.show_one_off = false;
            cx.notify();
        }
    }

    /// `/clear` gives the session a new id: the page's next start resumes
    /// that one (presence carries the id under the terminal's token).
    fn follow_conversations(&mut self, cx: &mut Context<Self>) {
        if self.free {
            return;
        }
        let live = crate::sessions::live(cx);
        let mut moved = Vec::new();
        for (_, pane) in self.panes.iter_mut() {
            if let Pane::Running {
                token,
                conversation,
                origin: Some(origin),
                page_conversation,
                ..
            } = pane
                && let Some(s) = crate::sessions::of_launch(live, token)
                && conversation.as_deref() != Some(s.id.as_str())
                && acta_terminal::launch::is_uuid(&s.id)
            {
                *conversation = Some(s.id.clone());
                if *page_conversation {
                    moved.push((origin.owner, s.id.clone()));
                }
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

    /// The presence state of a running session.
    pub fn session_state(&self, key: InstanceKey, cx: &App) -> Option<SessionState> {
        let token = self.token(key)?;
        crate::sessions::of_launch(crate::sessions::live(cx), token).map(|s| s.state)
    }

    /// Whether any session of this dock waits for the user (the busy dot
    /// of the view's tab and toggle).
    pub fn any_waiting(&self, cx: &App) -> bool {
        self.panes
            .iter()
            .any(|(k, _)| self.session_state(k, cx) == Some(SessionState::Waiting))
    }

    pub fn change_text_size(&mut self, by: f32, cx: &mut Context<Self>) {
        use acta_terminal::view::{MAX_TEXT_SIZE, MIN_TEXT_SIZE};
        self.text_size = (self.text_size + by).clamp(MIN_TEXT_SIZE, MAX_TEXT_SIZE);
        let size = self.text_size;
        self.apply_text_size(size, cx);
        let stored = size.round() as u8;
        settings::change(cx, |prefs| prefs.terminal_text_size = Some(stored));
    }

    pub fn text_size(&self) -> f32 {
        self.text_size
    }

    /// The other dock's text size changed: follow it.
    pub fn apply_text_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.text_size = size;
        for (_, pane) in self.panes.iter() {
            if let Pane::Running { view, .. } = pane {
                view.update(cx, |view, cx| view.set_text_size(size, cx));
            }
        }
    }

    /// The command the Ask pane of `key` holds, if it is asking.
    fn asked_prompt(&self, key: InstanceKey) -> Option<String> {
        match self.panes.get(key) {
            Some(Pane::Ask { prompt, .. }) => prompt.clone(),
            _ => None,
        }
    }

    fn choose_folder(
        &mut self,
        key: InstanceKey,
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
                                let prompt = dock.asked_prompt(key);
                                dock.launch(key, Some(origin), prompt, window, cx)
                            });
                        }
                    })
                    .detach();
                } else {
                    dock.unsaved.insert(project, path);
                    let prompt = dock.asked_prompt(key);
                    dock.launch(key, Some(origin), prompt, window, cx);
                }
            });
        })
        .detach();
    }

    fn locate(
        &mut self,
        key: InstanceKey,
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
                            dock.launch(key, Some(origin), None, window, cx)
                        });
                    }
                })
                .detach();
            });
        })
        .detach();
    }

    /// The header's context for a page session: its owner first,
    /// "Topic T-42 Embed the terminal" (layout.md 4.5.1).
    fn context(origin: &Origin, locale: Locale) -> String {
        owner_label(locale, origin.owner, &origin.name)
    }

    /// The instance strip (layout.md 4.5.1): from two instances, one
    /// compact tab each, the active one selected, `x` on hover and on the
    /// active one.
    fn strip(&self, holder: Holder, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let keys = self.panes.keys_of(holder);
        if keys.len() < 2 {
            return None;
        }
        let locale = settings::locale(cx);
        let shown = self.shown();
        let tabs: Vec<AnyElement> = keys
            .into_iter()
            .map(|key| {
                let active = shown == Some(key);
                let id = format!("terminal-inst-{}", key.n);
                let label = tr(
                    locale,
                    Key::ToolClaudeN,
                    &[("n", Arg::Count(u64::from(key.n)))],
                );
                // The session's `bot` in its state colour (none for a free
                // session, which has no presence).
                let lead = self.session_state(key, cx).map(|s| {
                    icon(Icon::Bot, 14., crate::sessions::state_color(s, t)).into_any_element()
                });
                crate::tool_strip::tab(id.clone(), lead, label, active, t)
                    .on_click(cx.listener(move |dock, _, window, cx| {
                        dock.select(key, cx);
                        let handle = dock.focus_handle(cx);
                        window.focus(&handle, cx);
                    }))
                    .child(
                        crate::tool_strip::close(
                            format!("terminal-inst-close-{}", key.n),
                            id,
                            active,
                            t,
                        )
                        .tooltip(ui::tip(text(locale, Key::CommonClose)))
                        .on_click(cx.listener(
                            move |dock, _, window, cx| {
                                cx.stop_propagation();
                                dock.close(key, window, cx)
                            },
                        )),
                    )
                    .into_any_element()
            })
            .collect();
        Some(crate::tool_strip::strip(tabs, t))
    }

    /// The empty state of the view (layout.md 4.5.1): a page with no
    /// session, a page that owns nothing, or Claude Code (no project).
    fn empty_state(
        &self,
        t: &Tokens,
        surface: gpui_kit::Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locale = settings::locale(cx);
        let heading = |s: String| type_scale::HEADING.el().text_color(t.text_primary).child(s);
        let sentence = |s: String| {
            type_scale::READING
                .el()
                .mt(px(8.))
                .text_color(t.text_secondary)
                .child(s)
        };
        let start = |label: String, cx: &mut Context<Self>| {
            div().mt(px(24.)).flex().child(
                ui::button("terminal-start", label, Variant::Primary, false, t)
                    .track_focus(&self.start_focus)
                    .on_click(cx.listener(|dock, _, window, cx| {
                        dock.start_here(window, cx);
                    })),
            )
        };
        if self.free {
            return message(t, surface)
                .child(heading(text(locale, Key::ToolFreeTitle)))
                .child(sentence(text(locale, Key::ToolFreeBody)))
                .child(start(text(locale, Key::ToolFreeStart), cx))
                .into_any_element();
        }
        let Some(origin) = self.current.clone() else {
            return message(t, surface)
                .child(sentence(text(locale, Key::ToolNoOwner)).mt(px(0.)))
                .child(
                    div().mt(px(16.)).flex().child(
                        ui::button(
                            "terminal-free",
                            text(locale, Key::ToolFree),
                            Variant::Ghost,
                            false,
                            t,
                        )
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DockEvent::ShowFree))),
                    ),
                )
                .into_any_element();
        };
        let lower = level_word(locale, origin.owner).to_lowercase();
        // French says "cet epic", "ce sujet" (the plural form chosen by
        // the elision, catalogue 10.4.17).
        let elision = if matches!(origin.owner, Owner::Epic(_)) {
            2
        } else {
            1
        };
        let none = tr(
            locale,
            Key::ToolClaudeNone,
            &[
                ("level_lower", Arg::Text(&lower)),
                ("level_lower", Arg::Count(elision)),
            ],
        );
        let body = self.current_folder.clone().map(|(name, path)| {
            sentence(tr(
                locale,
                Key::ToolClaudeNoneBody,
                &[
                    ("folder", Arg::Text(&name)),
                    ("level_lower", Arg::Text(&lower)),
                ],
            ))
            .id("terminal-start-folder")
            .tooltip(ui::tip(path))
        });
        message(t, surface)
            .child(heading(none))
            .children(body)
            .child(start(text(locale, Key::ToolClaudeStart), cx))
            .into_any_element()
    }

    fn render_dock(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.follow_conversations(cx);
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let mode = cx.global::<Appearance>().mode;
        if self.mode != Some(mode) {
            self.mode = Some(mode);
            for (_, pane) in self.panes.iter() {
                if let Pane::Running { view, .. } = pane {
                    view.update(cx, |view, cx| view.set_theme(theme_for(mode), cx));
                }
            }
            if let Some(one) = &self.one_off {
                one.view
                    .update(cx, |view, cx| view.set_theme(theme_for(mode), cx));
            }
        }
        let surface = match mode {
            Mode::Dark => t.bg_sunken,
            Mode::Light => t.bg_raised,
        };
        let one_off = self
            .one_off
            .as_ref()
            .filter(|_| self.show_one_off)
            .map(|o| o.view.clone());
        let key = self.shown();
        let pane = key.and_then(|k| self.panes.get(k));
        let body: AnyElement =
            match pane {
                // A one-off session of the Ready block (ADR 0013).
                _ if one_off.is_some() => div()
                    .flex_1()
                    .min_h_0()
                    .p(px(12.))
                    .bg(surface)
                    .children(one_off.clone())
                    .into_any_element(),
                // screens.md 8.4: the grid is inset 12 on every side.
                Some(Pane::Running { view, .. }) => div()
                    .flex_1()
                    .min_h_0()
                    .p(px(12.))
                    .bg(surface)
                    .child(view.clone())
                    .into_any_element(),
                Some(Pane::Ask { origin, save, .. }) => {
                    let key = key.expect("a pane has a key");
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
                                    if let Some(Pane::Ask { save, .. }) = dock.panes.get_mut(key) {
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
                                        dock.choose_folder(key, origin.clone(), save, window, cx)
                                    },
                                )),
                            ),
                        )
                        .into_any_element()
                }
                Some(Pane::Missing { origin, path, from }) => {
                    let key = key.expect("a pane has a key");
                    let (origin, path, from) = (origin.clone(), path.clone(), *from);
                    let body_key = match origin.owner {
                        Owner::Topic(_) => Key::TerminalMissingBodyTopic,
                        Owner::Epic(_) => Key::TerminalMissingBodyEpic,
                        Owner::Project(_) => Key::TerminalMissingBodyProject,
                    };
                    let level_name = origin.name.clone();
                    let other = self.other_folder_switcher(key, &origin, locale, &t, cx);
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
                                            dock.locate(
                                                key,
                                                origin.clone(),
                                                path.clone(),
                                                from,
                                                window,
                                                cx,
                                            )
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
                                        move |dock, _, window, cx| dock.restart(key, window, cx),
                                    )),
                                ),
                        )
                        .children(other)
                        .into_any_element()
                }
                Some(Pane::NotFound { .. }) => {
                    let key = key.expect("a pane has a key");
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
                                    move |dock, _, window, cx| dock.restart(key, window, cx),
                                )),
                            ),
                        )
                        .into_any_element()
                }
                Some(Pane::Starting { .. }) => div().flex_1().bg(surface).into_any_element(),
                None => self.empty_state(&t, surface, cx),
            };
        let strip = match self.holder() {
            Some(holder) if one_off.is_none() => self.strip(holder, &t, cx),
            _ => None,
        };
        div()
            .id(if self.free {
                "free-claude-dock"
            } else {
                "terminal-dock"
            })
            .key_context("ToolView")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(surface)
            .children(strip)
            .child(body)
            .into_any_element()
    }

    /// The folder mismatch of the session in view (layout.md 4.5.1,
    /// Restart): the page's active folder name to "Restart in".
    fn mismatch(&self) -> Option<String> {
        let key = self.shown()?;
        let Some(Pane::Running { start, .. }) = self.panes.get(key) else {
            return None;
        };
        let page = self.pages.get(&key);
        let active = page.and_then(|p| p.folders.active());
        let mode_changed = page.is_some_and(|p| p.commit.is_some() && p.commit != start.commit);
        tm::restart_in(
            &start.cwd.to_string_lossy(),
            active.map(|f| (f.path.as_str(), f.name.as_str())),
            mode_changed,
        )
    }

    /// The header switcher of the session's page: its folders, grouped.
    fn switch_groups(&self, key: InstanceKey, owner: Owner, locale: Locale) -> Vec<SwitchGroup> {
        let Some(page) = self.pages.get(&key) else {
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
                            icon: fm::shown_icon(
                                &f.icon,
                                page.disk.get(&f.path).and_then(|d| d.git.as_ref()),
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

    /// "Or start in another folder:" and the switcher, under the not-found
    /// message, when the page sees another folder that exists: choosing
    /// makes it active for the page and starts the session there
    /// (components-folders.md 5, Terminal launch).
    fn other_folder_switcher(
        &self,
        key: InstanceKey,
        origin: &Origin,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (project, owner) = (origin.project, origin.owner);
        let groups = self.switch_groups(key, owner, locale);
        let entries = || groups.iter().flat_map(|g| g.entries.iter());
        if !entries().any(|e| !e.missing && !e.active) {
            return None;
        }
        let active = entries().find(|e| e.active)?.clone();
        let button = folders_view::switcher(
            "terminal-missing-switcher",
            active.icon,
            &active.name,
            active.missing,
            false,
            t,
        )
        .on_click(cx.listener(|dock, _, _, cx| {
            dock.missing_switcher = match dock.missing_switcher {
                Some(_) => None,
                None => Some(0),
            };
            cx.notify();
        }));
        let mut wrap = div()
            .relative()
            .flex()
            .child(button)
            .child(self.missing_anchor.measure());
        if self.missing_switcher.is_some() {
            let start = origin.clone();
            let (edit_origin, add_origin) = (origin.clone(), origin.clone());
            wrap = wrap.child(folders_view::switcher_menu(
                "terminal-missing-menu",
                &self.missing_anchor,
                &groups,
                self.missing_switcher,
                locale,
                t,
                cx,
                SwitchActions {
                    choose: Rc::new(move |dock: &mut Self, id, window, cx| {
                        dock.missing_switcher = None;
                        dock.set_page_active(owner, key, project, id, cx);
                        // The write is queued first, so the launch reads
                        // the new active folder.
                        dock.launch(key, Some(start.clone()), None, window, cx);
                    }),
                    edit: Rc::new(move |dock: &mut Self, _, cx| {
                        dock.missing_switcher = None;
                        cx.emit(DockEvent::EditFolders(edit_origin.clone()));
                        cx.notify();
                    }),
                    add: Rc::new(move |dock: &mut Self, _, cx| {
                        dock.missing_switcher = None;
                        cx.emit(DockEvent::AddFolder(add_origin.clone()));
                        cx.notify();
                    }),
                    close: Rc::new(|dock: &mut Self, _, cx| {
                        dock.missing_switcher = None;
                        cx.notify();
                    }),
                },
            ));
        }
        Some(
            div()
                .mt(px(16.))
                .flex()
                .flex_col()
                .items_start()
                .gap(px(4.))
                .child(
                    type_scale::SMALL
                        .el()
                        .text_color(t.text_secondary)
                        .child(text(locale, Key::TerminalMissingOther)),
                )
                .child(wrap)
                .into_any_element(),
        )
    }

    /// The header switcher sets the session page's active folder, exactly
    /// as the page's own switcher does; the session keeps running.
    fn set_page_active(
        &mut self,
        owner: Owner,
        key: InstanceKey,
        project: ProjectId,
        id: FolderId,
        cx: &mut Context<Self>,
    ) {
        self.switcher = None;
        cx.notify();
        let active = self
            .pages
            .get(&key)
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
    /// `available` is the width the header leaves it (the tabs and the
    /// buttons keep theirs); the parts give way by [`fit_header`], so
    /// nothing overlaps at any width.
    pub fn header_context(
        &mut self,
        available: Option<f32>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        // A one-off session: its command (or "Claude Code"), protocol text.
        if let Some(one) = self.one_off.as_ref().filter(|_| self.show_one_off) {
            return Some(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .min_w_0()
                    .child(icon(Icon::SquareTerminal, 14., t.text_tertiary))
                    .child(
                        type_scale::CODE
                            .el()
                            .min_w_0()
                            .truncate()
                            .text_color(t.text_secondary)
                            .child(one.title.clone()),
                    )
                    .into_any_element(),
            );
        }
        let locale = settings::locale(cx);
        let label = |s: String, tip: Option<String>| {
            type_scale::SMALL
                .el()
                .id("terminal-context")
                .min_w_0()
                .truncate()
                .text_color(t.text_tertiary)
                .child(s)
                .when_some(tip, |this, tip| this.tooltip(ui::tip(tip)))
                .into_any_element()
        };
        // Claude Code (no project): where it runs, the home path as tooltip.
        if self.free {
            return Some(label(
                text(locale, Key::ToolFreeContext),
                paths_view::home(),
            ));
        }
        let key = self.shown();
        let shown_origin = key
            .and_then(|k| self.panes.get(k))
            .and_then(|p| p.origin().cloned());
        let (Some(key), Some(origin)) = (key, shown_origin) else {
            // The empty state: the owner alone.
            let origin = self.current.clone()?;
            return Some(label(Self::context(&origin, locale), None));
        };
        let mismatch = self.mismatch();
        let marked = mismatch.is_some();
        let waiting = self
            .session_state(key, cx)
            .is_some_and(|s| s == SessionState::Waiting);
        let project = origin.project;
        let running = match self.panes.get(key) {
            Some(Pane::Running { start, .. }) => Some(start.cwd.clone()),
            _ => None,
        };
        let page = self.pages.get(&key);
        let page_active = page.and_then(|p| {
            p.folders
                .active()
                .cloned()
                .map(|f| (p.folders.is_missing(&f.path), f))
        });
        let running_note = match (&running, &page_active) {
            (Some(cwd), Some((_, active)))
                if !acta_core::folder_path::same(
                    acta_core::paths::Os::CURRENT,
                    &cwd.to_string_lossy(),
                    &active.path,
                ) =>
            {
                let cwd = cwd.to_string_lossy();
                let name = page
                    .and_then(|p| {
                        p.folders.items.iter().find(|f| {
                            acta_core::folder_path::same(
                                acta_core::paths::Os::CURRENT,
                                &f.path,
                                &cwd,
                            )
                        })
                    })
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| paths_view::last_segment(&cwd).to_owned());
                Some(tr(
                    locale,
                    Key::TerminalRunningIn,
                    &[("name", Arg::Text(&name))],
                ))
            }
            _ => None,
        };
        let action = mismatch
            .as_ref()
            .map(|name| tr(locale, Key::ToolRestartIn, &[("folder", Arg::Text(name))]));
        // Folders left out of the session (not found): one mark in the
        // switcher, the names in its tooltip.
        let left_out: Vec<String> = match self.panes.get(key) {
            Some(Pane::Running { start, .. }) => start
                .left_out
                .iter()
                .map(|p| {
                    let p = p.to_string_lossy();
                    let name = page
                        .and_then(|pg| pg.folders.items.iter().find(|f| f.path == p))
                        .map(|f| f.name.clone())
                        .unwrap_or_else(|| paths_view::last_segment(&p).to_owned());
                    tr(
                        locale,
                        Key::TerminalLeftOut,
                        &[("folder", Arg::Text(&name))],
                    )
                })
                .collect(),
            _ => Vec::new(),
        };
        let mark = if left_out.is_empty() { 0. } else { 14. + 6. };
        let context = Self::context(&origin, locale);
        let waiting_text = format!("\u{b7} {}", text(locale, Key::TerminalWaiting));
        // Every part at full size, measured with the font.
        let parts = HeaderParts {
            context: ui::text_width(type_scale::SMALL, &context, cx),
            switcher: page_active.as_ref().map_or(0., |(_, f)| {
                ui::text_width(type_scale::UI, &f.name, cx) + SWITCHER_CHROME + mark
            }),
            switcher_min: SWITCHER_MIN + mark,
            running: running_note
                .as_ref()
                .map(|n| ui::text_width(type_scale::SMALL, n, cx)),
            waiting: waiting.then(|| ui::text_width(type_scale::SMALL, &waiting_text, cx)),
            restart: action
                .as_ref()
                .map(|label| ui::text_width(type_scale::UI, label, cx) + 24.),
        };
        let fit = fit_header(available.unwrap_or(f32::MAX), parts);
        let switcher = page_active.map(|(missing, active)| {
            let git = page
                .and_then(|p| p.disk.get(&active.path))
                .and_then(|d| d.git.clone());
            let button = folders_view::switcher(
                "terminal-switcher",
                fm::shown_icon(&active.icon, git.as_ref(), missing),
                &active.name,
                missing,
                true,
                &t,
            )
            .w_full()
            .overflow_hidden()
            .when(self.switcher.is_none(), |this| {
                this.tooltip(ui::tip(active.path.clone()))
            })
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
            let alert = (!left_out.is_empty()).then(|| {
                div()
                    .id("terminal-left-out")
                    .flex_shrink_0()
                    .child(icon(Icon::TriangleAlert, 14., t.warning))
                    .tooltip(ui::tip(left_out.join("\n")))
            });
            let mut wrap = div()
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .flex_shrink_0()
                .w(px(fit.switcher))
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .child(button)
                        .child(self.switch_anchor.measure()),
                )
                .children(alert);
            if self.switcher.is_some() {
                let groups = self.switch_groups(key, origin.owner, locale);
                let owner = origin.owner;
                let (edit_origin, add_origin) = (origin.clone(), origin.clone());
                wrap = wrap.child(folders_view::switcher_menu(
                    "terminal-switch-menu",
                    &self.switch_anchor,
                    &groups,
                    self.switcher,
                    locale,
                    &t,
                    cx,
                    SwitchActions {
                        choose: Rc::new(move |dock: &mut Self, id, _, cx| {
                            dock.set_page_active(owner, key, project, id, cx)
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
            wrap
        });
        // "(running in ...)": the words, or an `info` mark carrying them.
        let running_el = running_note.map(|note| {
            if fit.running_text {
                type_scale::SMALL
                    .el()
                    .flex_shrink_0()
                    .text_color(t.text_tertiary)
                    .child(note)
                    .into_any_element()
            } else {
                div()
                    .id("terminal-running-in")
                    .flex_shrink_0()
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(Icon::Info, 14., t.text_tertiary))
                    .tooltip(ui::tip(note))
                    .into_any_element()
            }
        });
        let action_el = action.map(|label| {
            let el = if fit.restart_text {
                ui::button("terminal-restart-text", label, Variant::Ghost, false, &t).h(px(24.))
            } else {
                ui::ghost_icon_button(
                    "terminal-restart-text",
                    Icon::RotateCw,
                    24.,
                    false,
                    false,
                    true,
                    &t,
                )
                .tooltip(ui::tip(label))
            };
            el.on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(DockEvent::Restart(key))))
        });
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(HEADER_GAP))
                .min_w_0()
                .overflow_hidden()
                .when(fit.context >= CONTEXT_SHOWN, |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .flex_shrink_0()
                            .w(px(fit.context))
                            .truncate()
                            .text_color(if marked {
                                t.text_primary
                            } else {
                                t.text_tertiary
                            })
                            .child(context),
                    )
                })
                .children(switcher)
                .children(running_el)
                .when(waiting, |this| {
                    this.child(
                        type_scale::SMALL
                            .el()
                            .flex_shrink_0()
                            .text_color(t.warning)
                            .child(waiting_text),
                    )
                })
                .children(action_el)
                .into_any_element(),
        )
    }

    /// The view's own header buttons (layout.md 4.5.1): `plus` (a new
    /// session here, disabled at the limit) and Restart (`rotate-cw`) of
    /// the session in view.
    pub fn header_buttons(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        // A one-off session has no restart; once ended it can be closed.
        if let Some(one) = self.one_off.as_ref().filter(|_| self.show_one_off) {
            if !one.ended {
                return div().into_any_element();
            }
            return ui::ghost_icon_button(
                "terminal-one-off-close",
                Icon::X,
                24.,
                false,
                false,
                true,
                &t,
            )
            .tooltip(ui::tip(text(locale, Key::CommonClose)))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .on_click(cx.listener(|dock, _, _, cx| dock.close_one_off(cx)))
            .into_any_element();
        }
        let Some(holder) = self.holder() else {
            return div().into_any_element();
        };
        let limit = self.panes.at_limit(holder);
        let new_tip = if limit {
            tr(
                locale,
                Key::ToolLimit,
                &[("n", Arg::Count(self.panes.max() as u64))],
            )
        } else if self.free {
            text(locale, Key::ToolFreeNew)
        } else {
            text(locale, Key::ToolClaudeNew)
        };
        let plus = ui::ghost_icon_button("terminal-new", Icon::Plus, 24., false, limit, true, &t)
            .tooltip(ui::tip(new_tip))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .when(!limit, |this| {
                this.on_click(cx.listener(|dock, _, window, cx| {
                    dock.start_here(window, cx);
                }))
            });
        let restart = self.shown().map(|key| {
            ui::ghost_icon_button(
                "terminal-restart",
                Icon::RotateCw,
                24.,
                false,
                false,
                true,
                &t,
            )
            .tooltip(ui::tip(text(locale, Key::CmdClaudeRestart)))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(DockEvent::Restart(key))))
        });
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(plus)
            .children(restart)
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

/// The level word of `level.*` ("Topic", "Epic", "Project"; « Sujet »).
pub fn level_word(locale: Locale, owner: Owner) -> String {
    let key = match owner {
        Owner::Topic(_) => Key::LevelTopic,
        Owner::Epic(_) => Key::LevelEpic,
        Owner::Project(_) => Key::LevelProject,
    };
    tr(locale, key, &[("name", Arg::Text(""))])
        .trim()
        .to_owned()
}

/// "Topic T-42 Embed the terminal" (`tool.owner`): how every tool view's
/// header names the page it belongs to (layout.md 4.5.1).
pub fn owner_label(locale: Locale, owner: Owner, title: &str) -> String {
    let id = owner.to_string();
    let level = level_word(locale, owner);
    tr(
        locale,
        Key::ToolOwner,
        &[
            ("level", Arg::Text(&level)),
            ("id", Arg::Text(&id)),
            ("title", Arg::Text(title)),
        ],
    )
}

/// Claude Code (no project) (layout.md 4.5.1): `claude` with no argument
/// in the home folder, with nothing of Acta's: no folders, focus, plugin,
/// conversation or `ACTA_*` variable.
pub fn free_spec(
    program: &acta_terminal::launch::ClaudeProgram,
    home: PathBuf,
) -> Option<acta_terminal::LaunchSpec> {
    acta_terminal::launch::plain_claude_spec(program, home)
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
    fn the_header_gives_way_in_order_and_never_overlaps() {
        let p = HeaderParts {
            context: 220.,
            switcher: 140.,
            switcher_min: SWITCHER_MIN,
            running: Some(150.),
            waiting: None,
            restart: Some(160.),
        };
        let full = 220. + 140. + 150. + 160. + 3. * HEADER_GAP;
        let at = |w: f32| fit_header(w, p);
        // Room for all: nothing gives way.
        let f = at(full + 10.);
        assert_eq!((f.context, f.switcher), (220., 140.));
        assert!(f.running_text && f.restart_text);
        // 1. The label truncates first, down to its minimum.
        let f = at(full - 100.);
        assert_eq!(f.context, 120.);
        assert!(f.running_text && f.restart_text);
        // 2. Then "(running in ...)" becomes its icon.
        let f = at(140. + 150. + 160. + CONTEXT_MIN + 3. * HEADER_GAP - 1.);
        assert!(!f.running_text && f.restart_text);
        // 3. Then the restart action becomes an icon button.
        let f = at(CONTEXT_MIN + 140. + 2. * ICON_SLOT + 3. * HEADER_GAP + 20.);
        assert!(!f.running_text && !f.restart_text);
        assert_eq!(f.switcher, 140.);
        // 4. Then the switcher's name truncates, then the label goes.
        let f = at(CONTEXT_MIN + 100. + 2. * ICON_SLOT + 3. * HEADER_GAP);
        assert!(f.switcher < 140. && f.switcher >= SWITCHER_MIN);
        let f = at(SWITCHER_MIN + 2. * ICON_SLOT + 2. * HEADER_GAP);
        assert_eq!((f.context, f.switcher), (0., SWITCHER_MIN));
        // At every width the parts take no more than they are given, as
        // long as the icons fit.
        let floor = SWITCHER_MIN + 2. * ICON_SLOT + 2. * HEADER_GAP;
        let mut w = floor;
        while w < full + 50. {
            let f = at(w);
            assert!(f.total(&p) <= w + 0.5, "{w}: {f:?}");
            if f.context > 0. {
                assert!(f.context >= CONTEXT_SHOWN);
            }
            w += 7.;
        }
        // A short label is never stretched; no switcher takes no room.
        let small = HeaderParts {
            context: 40.,
            switcher: 0.,
            switcher_min: 0.,
            running: None,
            waiting: None,
            restart: None,
        };
        assert_eq!(fit_header(500., small).context, 40.);
        assert_eq!(fit_header(30., small).context, 30.);
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
            "Epic E-3 Storage"
        );
        assert_eq!(
            owner_label(Locale::Fr, Owner::Topic(TopicId(42)), "Embed the terminal"),
            "Sujet T-42 Embed the terminal"
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

    #[test]
    fn claude_code_without_a_project_has_nothing_of_acta() {
        let home = PathBuf::from(if cfg!(windows) {
            "C:\\Users\\Alexandre"
        } else {
            "/home/alexandre"
        });
        let exe = ClaudeProgram::Exe(PathBuf::from(if cfg!(windows) {
            "C:\\tools\\claude.exe"
        } else {
            "/usr/bin/claude"
        }));
        let spec = free_spec(&exe, home.clone()).unwrap();
        assert_eq!(spec.cwd, home, "the home folder");
        assert!(spec.args.is_empty(), "plain `claude`: {:?}", spec.args);
        assert!(spec.env.is_empty(), "no ACTA_* variable: {:?}", spec.env);
        // A page session of the same program, for contrast, has them.
        let page = build_claude_launch(
            &exe,
            std::slice::from_ref(&home),
            &focus_of(Owner::Project(ProjectId(1)), ProjectId(1), None),
            &ClaudeOptions {
                plugin_dir: Some(home.clone()),
                ..Default::default()
            },
            |_| true,
        )
        .unwrap();
        assert!(page.spec.args.contains(&"--plugin-dir".to_owned()));
        assert!(page.spec.env.iter().any(|(k, _)| k == "ACTA_PROJECT"));
        // Through a login shell (macOS, Linux), still no Acta argument.
        let shell = free_spec(&ClaudeProgram::LoginShell("/bin/zsh".into()), home).unwrap();
        assert!(
            !shell.args.iter().any(|a| a.starts_with("--")),
            "{:?}",
            shell.args
        );
    }
}
