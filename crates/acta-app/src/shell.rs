//! The workbench window (docs/design/layout.md 4.2): titlebar, the tree
//! sidebar, the page area, the terminal dock and the status bar, plus the
//! overlays (palette, quick capture, dialogs, toast).
//!
//! The store opens in the background at launch. A database whose key is
//! lost shows the key-loss recovery screen (screens.md 8.9) and is never
//! touched; with no project yet the first-run page asks for a folder
//! (8.5). Tabs and the chat panel are M5: one page per window, with back
//! and forward history.

use std::path::PathBuf;
use std::time::Duration;

use acta_core::model::{Author, EpicId, KbKind, Owner, ProjectId, Record, Status, TopicId};
use acta_core::store::ExportFile;
use gpui_kit::component::window_border;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, PathPromptOptions, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, TitlebarOptions, Window,
    WindowBounds, WindowDecorations, WindowOptions, actions, deferred, div, point, px, size,
};

use crate::capture::{Capture, CaptureEvent};
use crate::commands::{self, Command};
use crate::db::{self, Db, Opened};
use crate::dialogs::{Dialog, DialogEvent, Kind as DialogKind};
use crate::first_run::{FirstRun, FirstRunEvent};
use crate::i18n::{Arg, Key, Locale, short_count, text, tr};
use crate::icons::{Icon, icon};
use crate::inbox_page::InboxPage;
use crate::kb_model::Filters;
use crate::kb_page::{DocPage, Draft, KnowledgePage};
use crate::palette::{CreateKind, Palette, PaletteEvent, Setup};
use crate::palette_model::{Group, Target};
use crate::record_page::{EditTitle, OpenStatusMenu, PageEvent, RecordPage};
use crate::route::{History, Route};
use crate::settings;
use crate::settings_page::SettingsPage;
use crate::terminal_dock::{DockEvent, Origin, TerminalDock};
use crate::theme::{self, RADIUS_MD, RADIUS_SM, Tokens};
use crate::titlebar::{self, Crumb};
use crate::trash_page::TrashPage;
use crate::tree::{self, Folds, Row};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{Toast, ToastKind, Workspace, WorkspaceEvent};

/// docs/design/layout.md 4.2: default 1440 x 900, minimum 880 x 560.
pub const DEFAULT_SIZE: (f32, f32) = (1440., 900.);
pub const MIN_SIZE: (f32, f32) = (880., 560.);
/// docs/design/layout.md 4.2: the sidebar is 260 wide.
const SIDEBAR_WIDTH: f32 = 260.;
/// docs/design/layout.md 4.6: the status bar is 24 high.
const STATUS_BAR_HEIGHT: f32 = 24.;
/// docs/design/components.md 5, Toast: 6 s.
const TOAST_TIME: Duration = Duration::from_secs(6);

actions!(
    acta,
    [
        /// `Mod+B`, palette "Toggle sidebar".
        ToggleSidebar,
        /// `Mod+,`, palette "Open settings".
        OpenSettings,
        /// `Tab`: the next control.
        FocusNext,
        /// `Shift+Tab`: the previous control.
        FocusPrevious,
        /// `Mod+K`, `Mod+Shift+K`.
        OpenPalette,
        /// `Mod+P`: the palette scoped to topics.
        GoToTopic,
        /// `Mod+O`: the palette scoped to projects.
        SwitchProject,
        /// `Mod+T`.
        NewTopic,
        /// `Mod+E`.
        AddEntry,
        /// `Mod+N`.
        CaptureNote,
        /// `Mod+I`.
        GoInbox,
        /// `Mod+L`.
        GoKnowledge,
        /// `Mod+Shift+D`.
        NewDocument,
        /// `Mod+Shift+M`.
        NewMemory,
        /// `Alt+Left` / `Cmd+[`.
        Back,
        /// `Alt+Right` / `Cmd+]`.
        Forward,
        /// `Mod+Z`.
        Undo,
        /// ``Ctrl+` `` or `Mod+Shift+J`.
        ToggleTerminal,
        /// `Mod+1`.
        FocusSidebar,
        /// `Mod+2`.
        FocusPage,
        /// `Mod+3`.
        FocusTerminal,
        TreeUp,
        TreeDown,
        TreeLeft,
        TreeRight,
        TreeOpen,
        TreeDelete,
        TreeMoveUp,
        TreeMoveDown,
        TreeHome,
        TreeEnd,
    ]
);

/// The keys of the workbench (docs/design/interaction.md 6.1). Bindings
/// that must not reach past a focused terminal carry `!Terminal`; the ones
/// the terminal keeps (palette, terminal toggles, pane focus, text size)
/// have no context.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(key_bindings());
}

fn key_bindings() -> Vec<KeyBinding> {
    // `Mod` is Cmd on macOS and Ctrl elsewhere; the pinned GPUI wants
    // static keystrokes, so both spellings are written out.
    let mac = cfg!(target_os = "macos");
    let m = |macos: &'static str, other: &'static str| if mac { macos } else { other };
    let app = Some("!Terminal");
    let plain = Some("!Terminal && !Input");
    let mut out = vec![
        KeyBinding::new(m("cmd-b", "ctrl-b"), ToggleSidebar, app),
        KeyBinding::new(m("cmd-,", "ctrl-,"), OpenSettings, app),
        KeyBinding::new("tab", FocusNext, plain),
        KeyBinding::new("shift-tab", FocusPrevious, plain),
        KeyBinding::new(m("cmd-k", "ctrl-k"), OpenPalette, app),
        KeyBinding::new(m("cmd-shift-k", "ctrl-shift-k"), OpenPalette, None),
        KeyBinding::new(m("cmd-p", "ctrl-p"), GoToTopic, app),
        KeyBinding::new(m("cmd-o", "ctrl-o"), SwitchProject, app),
        KeyBinding::new(m("cmd-t", "ctrl-t"), NewTopic, app),
        KeyBinding::new(m("cmd-e", "ctrl-e"), AddEntry, app),
        KeyBinding::new(m("cmd-n", "ctrl-n"), CaptureNote, app),
        KeyBinding::new(m("cmd-i", "ctrl-i"), GoInbox, app),
        KeyBinding::new(m("cmd-l", "ctrl-l"), GoKnowledge, app),
        KeyBinding::new(m("cmd-shift-d", "ctrl-shift-d"), NewDocument, app),
        KeyBinding::new(m("cmd-shift-m", "ctrl-shift-m"), NewMemory, app),
        KeyBinding::new(m("cmd-z", "ctrl-z"), Undo, plain),
        KeyBinding::new("ctrl-`", ToggleTerminal, None),
        KeyBinding::new(m("cmd-shift-j", "ctrl-shift-j"), ToggleTerminal, None),
        KeyBinding::new(m("cmd-1", "ctrl-1"), FocusSidebar, None),
        KeyBinding::new(m("cmd-2", "ctrl-2"), FocusPage, None),
        KeyBinding::new(m("cmd-3", "ctrl-3"), FocusTerminal, None),
        KeyBinding::new("s", OpenStatusMenu, Some("RecordPage && !Input")),
        KeyBinding::new("e", EditTitle, Some("RecordPage && !Input")),
        KeyBinding::new("f2", EditTitle, Some("RecordPage && !Input")),
        KeyBinding::new("escape", crate::dialogs::CancelDialog, Some("Dialog")),
        KeyBinding::new("up", TreeUp, Some("Tree")),
        KeyBinding::new("k", TreeUp, Some("Tree")),
        KeyBinding::new("down", TreeDown, Some("Tree")),
        KeyBinding::new("j", TreeDown, Some("Tree")),
        KeyBinding::new("left", TreeLeft, Some("Tree")),
        KeyBinding::new("right", TreeRight, Some("Tree")),
        KeyBinding::new("enter", TreeOpen, Some("Tree")),
        KeyBinding::new("delete", TreeDelete, Some("Tree")),
        KeyBinding::new("alt-up", TreeMoveUp, Some("Tree")),
        KeyBinding::new("alt-down", TreeMoveDown, Some("Tree")),
        KeyBinding::new("home", TreeHome, Some("Tree")),
        KeyBinding::new("end", TreeEnd, Some("Tree")),
    ];
    if mac {
        out.push(KeyBinding::new("cmd-[", Back, app));
        out.push(KeyBinding::new("cmd-]", Forward, app));
        out.push(KeyBinding::new("cmd-backspace", TreeDelete, Some("Tree")));
    } else {
        out.push(KeyBinding::new("alt-left", Back, plain));
        out.push(KeyBinding::new("alt-right", Forward, plain));
    }
    out
}

/// What the page area shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    FirstRun,
    Settings,
}

/// How a workbench window starts (development aids aside, the last page).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Start {
    pub page: Page,
    pub sidebar: bool,
}

enum PageView {
    FirstRun(Entity<FirstRun>),
    Settings(Entity<SettingsPage>),
    Record(Entity<RecordPage>),
    Inbox(Entity<InboxPage>),
    Knowledge(Entity<KnowledgePage>),
    Doc(Entity<DocPage>),
    Trash(Entity<TrashPage>),
}

impl PageView {
    fn view(&self) -> AnyView {
        match self {
            PageView::FirstRun(v) => v.clone().into(),
            PageView::Settings(v) => v.clone().into(),
            PageView::Record(v) => v.clone().into(),
            PageView::Inbox(v) => v.clone().into(),
            PageView::Knowledge(v) => v.clone().into(),
            PageView::Doc(v) => v.clone().into(),
            PageView::Trash(v) => v.clone().into(),
        }
    }
}

enum Overlay {
    Palette(Entity<Palette>),
    Capture(Entity<Capture>),
    Dialog(Entity<Dialog>),
}

struct Bench {
    ws: Entity<Workspace>,
    history: History,
    page: PageView,
    settings: Entity<SettingsPage>,
    folds: Folds,
    cursor: usize,
    tree_focus: FocusHandle,
    page_focus: FocusHandle,
    active_project: Option<ProjectId>,
    recent: Vec<TopicId>,
    overlay: Option<Overlay>,
    /// Where focus returns when the overlay closes.
    return_focus: Option<FocusHandle>,
    capture_draft: String,
    toast: Option<Toast>,
    toast_timer: Task<()>,
    dock: Entity<TerminalDock>,
    dock_visible: bool,
    _subs: Vec<Subscription>,
    _page_sub: Option<Subscription>,
}

enum Content {
    /// The store is opening (never shown for long; nothing is drawn).
    Opening,
    KeyLoss {
        db: Option<PathBuf>,
    },
    Workbench(Box<Bench>),
    /// The type specimen, a development aid: no sidebar, no status bar.
    Specimen(AnyView),
}

pub struct Shell {
    content: Content,
    start: Start,
    sidebar_visible: bool,
    focus: FocusHandle,
    /// A press on the titlebar that has not moved yet (macOS and Linux,
    /// where the app starts the window move itself).
    pub titlebar_should_move: bool,
    _appearance: Subscription,
    _open: Task<()>,
}

impl Shell {
    pub fn workbench(start: Start, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut shell = Self::new(Content::Opening, start, window, cx);
        shell.open_store(window, cx);
        shell
    }

    pub fn specimen(view: AnyView, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Content::Specimen(view), Start::default(), window, cx)
    }

    fn new(content: Content, start: Start, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // System follows the OS live (docs/design/color.md 2).
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            theme::os_appearance_changed(window.appearance(), cx);
        });
        theme::os_appearance_changed(window.appearance(), cx);
        let focus = cx.focus_handle();
        // Focus inside the window, so its keys reach the shell. This does
        // not raise or activate the window.
        window.focus(&focus, cx);
        let sidebar_visible = start.sidebar || !settings::current(cx).sidebar_hidden;
        Self {
            content,
            start,
            sidebar_visible,
            focus,
            titlebar_should_move: false,
            _appearance: appearance,
            _open: Task::ready(()),
        }
    }

    fn open_store(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let config = db::config();
        let opening = cx.background_spawn(async move {
            let mut opened = db::open(config);
            if let Opened::Ready(store) = &mut opened {
                seed_if_asked(store);
            }
            opened
        });
        self._open = cx.spawn_in(window, async move |shell, cx| {
            let opened = opening.await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                match opened {
                    Opened::Ready(store) => shell.start_workbench(Db::new(store), window, cx),
                    Opened::KeyLoss { db } => shell.content = Content::KeyLoss { db: Some(db) },
                    Opened::Failed { db } => shell.content = Content::KeyLoss { db },
                }
                cx.notify();
            });
        });
    }

    fn start_workbench(&mut self, db: Db, window: &mut Window, cx: &mut Context<Self>) {
        let ws = cx.new(|cx| Workspace::new(db, cx));
        let dock = cx.new(|cx| TerminalDock::new(ws.clone(), cx));
        let prefs = settings::current(cx).clone();
        let folds = Folds {
            collapsed: prefs.collapsed.iter().cloned().collect(),
            closed_shown: prefs.closed_shown.iter().cloned().collect(),
        };
        let settings_page = cx.new(|cx| SettingsPage::new(window, cx));
        let subs = vec![
            cx.subscribe_in(&ws, window, Self::on_workspace),
            cx.observe(&ws, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &dock,
                window,
                |shell, _, event: &DockEvent, window, cx| match event {
                    DockEvent::Hide => shell.hide_terminal(window, cx),
                    DockEvent::RestartHere => shell.restart_terminal_here(window, cx),
                },
            ),
        ];
        let first_run = cx.new(|_| FirstRun);
        let first_run_sub = Self::watch_first_run(&first_run, window, cx);
        let bench = Bench {
            ws,
            history: History::default(),
            page: PageView::FirstRun(first_run),
            settings: settings_page,
            folds,
            cursor: 0,
            tree_focus: cx.focus_handle(),
            page_focus: cx.focus_handle(),
            active_project: None,
            recent: Vec::new(),
            overlay: None,
            return_focus: None,
            capture_draft: String::new(),
            toast: None,
            toast_timer: Task::ready(()),
            dock,
            dock_visible: false,
            _subs: subs,
            _page_sub: Some(first_run_sub),
        };
        self.content = Content::Workbench(Box::new(bench));
        // The first page waits for the tree (first reload).
    }

    fn bench(&mut self) -> Option<&mut Bench> {
        match &mut self.content {
            Content::Workbench(bench) => Some(bench),
            _ => None,
        }
    }

    fn on_workspace(
        &mut self,
        _: &Entity<Workspace>,
        event: &WorkspaceEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorkspaceEvent::Toast(toast) => self.show_toast(toast.clone(), cx),
            WorkspaceEvent::Reloaded => self.after_reload(window, cx),
        }
    }

    /// The first reload opens the first page; later ones keep the page and
    /// move to first run when the last project is gone (or out of it once
    /// one exists).
    fn after_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.start;
        let Some(bench) = self.bench() else { return };
        let ws = bench.ws.read(cx);
        let has_projects = !ws.projects.is_empty();
        let first_project = ws.projects.first().map(|t| t.project.id);
        let on_first_run = matches!(bench.page, PageView::FirstRun(_));
        let started = bench.history.current().is_some();
        if !started {
            let saved = settings::current(cx).page.as_deref().and_then(Route::parse);
            let route = if start.page == Page::Settings {
                Some(Route::Settings)
            } else if has_projects {
                saved.or(first_project.map(Route::Project))
            } else {
                None
            };
            match route {
                Some(route) => self.navigate(route, false, window, cx),
                None => self.show_first_run(window, cx),
            }
            self.dev_overlay(window, cx);
            return;
        }
        if !has_projects && !matches!(bench.page, PageView::Settings(_) | PageView::Trash(_)) {
            self.show_first_run(window, cx);
        } else if has_projects
            && on_first_run
            && let Some(id) = first_project
        {
            self.navigate(Route::Inbox(id), true, window, cx);
        }
        self.clamp_cursor(cx);
    }

    /// Debug builds: `ACTA_DEV_OVERLAY=palette|capture|terminal|export|snapshots`
    /// opens that surface at launch, so captures need no input.
    fn dev_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !cfg!(debug_assertions) {
            return;
        }
        match std::env::var("ACTA_DEV_OVERLAY").as_deref() {
            Ok("palette") => self.open_palette(None, None, window, cx),
            Ok("capture") => self.open_capture(window, cx),
            Ok("terminal") => self.toggle_terminal(window, cx),
            Ok("export") => self.open_export(window, cx),
            Ok("snapshots") => self.open_snapshots(window, cx),
            _ => {}
        }
    }

    fn show_first_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if matches!(bench.page, PageView::FirstRun(_)) {
            return;
        }
        let page = cx.new(|_| FirstRun);
        let sub = Self::watch_first_run(&page, window, cx);
        let Some(bench) = self.bench() else { return };
        bench.page = PageView::FirstRun(page);
        bench._page_sub = Some(sub);
        cx.notify();
    }

    fn watch_first_run(
        page: &Entity<FirstRun>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            page,
            window,
            |shell, _, event: &FirstRunEvent, window, cx| match event {
                FirstRunEvent::ChooseFolder => shell.new_project(window, cx),
            },
        )
    }

    pub fn sidebar_visible(&self) -> bool {
        self.sidebar_visible
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        if matches!(self.content, Content::Workbench(_)) {
            self.sidebar_visible = !self.sidebar_visible;
            let hidden = !self.sidebar_visible;
            settings::change(cx, |prefs| prefs.sidebar_hidden = hidden);
            cx.notify();
        }
    }

    pub fn terminal_visible(&self) -> bool {
        match &self.content {
            Content::Workbench(bench) => bench.dock_visible,
            _ => false,
        }
    }

    pub fn can_go_back(&self) -> bool {
        matches!(&self.content, Content::Workbench(b) if b.history.can_go_back())
    }

    pub fn can_go_forward(&self) -> bool {
        matches!(&self.content, Content::Workbench(b) if b.history.can_go_forward())
    }

    // ---- navigation ----

    /// Open `route` in the page area (`push` records a history step).
    pub fn navigate(
        &mut self,
        route: Route,
        push: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        if push {
            bench.history.open(route);
        } else {
            bench.history.replace(route);
        }
        self.show(route, window, cx);
    }

    fn show(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let ws = bench.ws.clone();
        let (page, sub): (PageView, Option<Subscription>) = match route {
            Route::Project(id) => record(ws, Owner::Project(id), window, cx),
            Route::Epic(id) => record(ws, Owner::Epic(id), window, cx),
            Route::Topic(id) => record(ws, Owner::Topic(id), window, cx),
            Route::Inbox(id) => {
                let page = cx.new(|cx| InboxPage::new(ws, id, window, cx));
                let sub = cx.subscribe_in(&page, window, Self::on_page);
                (PageView::Inbox(page), Some(sub))
            }
            Route::Knowledge(id) => {
                let page = cx.new(|cx| KnowledgePage::new(ws, id, Filters::default(), window, cx));
                let sub = cx.subscribe_in(&page, window, Self::on_page);
                (PageView::Knowledge(page), Some(sub))
            }
            Route::Kb(id) => {
                let page = cx.new(|cx| DocPage::open(ws, id, cx));
                let sub = cx.subscribe_in(&page, window, Self::on_page);
                (PageView::Doc(page), Some(sub))
            }
            Route::Trash => {
                let page = cx.new(|cx| TrashPage::new(ws, cx));
                let sub = cx.subscribe_in(&page, window, Self::on_page);
                (PageView::Trash(page), Some(sub))
            }
            Route::Settings => {
                let Some(bench) = self.bench() else { return };
                (PageView::Settings(bench.settings.clone()), None)
            }
        };
        let Some(bench) = self.bench() else { return };
        bench.page = page;
        bench._page_sub = sub;
        // The active project, and its tree node unfolded (4.2, 4.6).
        let projects = bench.ws.read(cx).projects.clone();
        let project = match route {
            Route::Project(id) | Route::Inbox(id) | Route::Knowledge(id) => Some(id),
            Route::Topic(id) => {
                tree::locate_topic(&projects, id).and_then(|l| l.project.map(|p| p.0))
            }
            Route::Epic(id) => {
                tree::locate_epic(&projects, id).and_then(|l| l.project.map(|p| p.0))
            }
            Route::Kb(_) | Route::Trash | Route::Settings => None,
        };
        if let Some(project) = project {
            bench.active_project = Some(project);
            bench.folds.expand(&project.to_string());
        }
        if let Route::Topic(id) = route {
            bench.recent.retain(|t| *t != id);
            bench.recent.insert(0, id);
            bench.recent.truncate(5);
        }
        let page_focus = bench.page_focus.clone();
        let dock = bench.dock.clone();
        let origin = self.origin(cx);
        dock.update(cx, |dock, cx| {
            if let Some(origin) = &origin
                && dock.has_session(origin.project)
            {
                dock.project = Some(origin.project);
            }
            dock.active = origin;
            cx.notify();
        });
        self.save_layout(cx);
        self.focus_page(window, cx);
        let _ = page_focus;
        cx.notify();
    }

    fn on_page(
        &mut self,
        _: &Entity<impl Render>,
        event: &PageEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            PageEvent::Open(route) => self.navigate(*route, true, window, cx),
            PageEvent::Replace(route) => {
                if let Some(bench) = self.bench() {
                    bench.history.replace(*route);
                }
                self.save_layout(cx);
            }
            PageEvent::NewKb(draft) => self.new_kb_draft(*draft, window, cx),
            PageEvent::Located(project) => {
                let project = *project;
                if let Some(bench) = self.bench()
                    && bench.active_project != Some(project)
                {
                    bench.active_project = Some(project);
                    cx.notify();
                }
            }
        }
    }

    fn new_kb_draft(&mut self, draft: Draft, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let ws = bench.ws.clone();
        let page = cx.new(|cx| DocPage::create(ws, draft, window, cx));
        let sub = cx.subscribe_in(&page, window, Self::on_page);
        let Some(bench) = self.bench() else { return };
        bench.page = PageView::Doc(page);
        bench._page_sub = Some(sub);
        cx.notify();
    }

    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if let Some(route) = bench.history.back() {
            self.show(route, window, cx);
        }
    }

    fn forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if let Some(route) = bench.history.forward() {
            self.show(route, window, cx);
        }
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Route::Settings, true, window, cx);
    }

    /// Write the page and the tree's folds to `preferences.json` (ids only).
    fn save_layout(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let page = bench.history.current().map(Route::key);
        let mut collapsed: Vec<String> = bench.folds.collapsed.iter().cloned().collect();
        let mut closed: Vec<String> = bench.folds.closed_shown.iter().cloned().collect();
        collapsed.sort();
        closed.sort();
        let current = settings::current(cx);
        if current.page == page && current.collapsed == collapsed && current.closed_shown == closed
        {
            return;
        }
        settings::change(cx, |prefs| {
            prefs.page = page;
            prefs.collapsed = collapsed;
            prefs.closed_shown = closed;
        });
    }

    fn focus_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let handle = match &bench.page {
            PageView::Record(page) => Some(page.read(cx).focus_handle(cx)),
            _ => None,
        };
        let handle = handle.unwrap_or_else(|| bench.page_focus.clone());
        // Pages that focus their own editor on open (inbox, editor) keep it.
        if !matches!(bench.page, PageView::Inbox(_) | PageView::Doc(_)) {
            window.focus(&handle, cx);
        }
    }

    /// The page's record, for the terminal and the folder commands.
    fn origin(&mut self, cx: &mut Context<Self>) -> Option<Origin> {
        let bench = self.bench()?;
        let project = bench.active_project?;
        let projects = bench.ws.read(cx).projects.clone();
        let route = bench.history.current();
        let (owner, name) = match route {
            Some(Route::Topic(id)) => (
                Owner::Topic(id),
                tree::locate_topic(&projects, id)?.topic?.1,
            ),
            Some(Route::Epic(id)) => (Owner::Epic(id), tree::locate_epic(&projects, id)?.epic?.1),
            _ => (
                Owner::Project(project),
                tree::locate_project(&projects, project)?.project?.1,
            ),
        };
        Some(Origin {
            owner,
            project,
            name,
        })
    }

    // ---- overlays ----

    fn open_palette(
        &mut self,
        scope: Option<Group>,
        create: Option<CreateKind>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let context = self.command_context();
        let Some(bench) = self.bench() else { return };
        let ws = bench.ws.read(cx);
        let setup = Setup {
            scope,
            create,
            projects: ws.projects.clone(),
            recent: bench.recent.clone(),
            context,
            db: ws.db.clone(),
        };
        bench.return_focus = window.focused(cx);
        let palette = cx.new(|cx| Palette::new(setup, window, cx));
        let sub = cx.subscribe_in(&palette, window, Self::on_palette);
        let Some(bench) = self.bench() else { return };
        bench._subs.push(sub);
        bench.overlay = Some(Overlay::Palette(palette));
        cx.notify();
    }

    fn command_context(&self) -> commands::Context {
        match &self.content {
            Content::Workbench(bench) => commands::Context {
                has_project: bench.active_project.is_some(),
                on_topic: matches!(bench.history.current(), Some(Route::Topic(_))),
                on_record: matches!(bench.page, PageView::Record(_)),
                can_back: bench.history.can_go_back(),
                can_forward: bench.history.can_go_forward(),
            },
            _ => commands::Context::default(),
        }
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.overlay = None;
        if let Some(handle) = bench.return_focus.take() {
            window.focus(&handle, cx);
        } else {
            self.focus_page(window, cx);
        }
        cx.notify();
    }

    fn on_palette(
        &mut self,
        _: &Entity<Palette>,
        event: &PaletteEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            PaletteEvent::Dismiss => self.close_overlay(window, cx),
            PaletteEvent::Run(target) => {
                let target = target.clone();
                self.close_overlay(window, cx);
                match target {
                    Target::Topic(id, _) => self.navigate(Route::Topic(id), true, window, cx),
                    Target::Project(id) => self.navigate(Route::Project(id), true, window, cx),
                    Target::Kb(id, _) => self.navigate(Route::Kb(id), true, window, cx),
                    Target::Command(command) => self.run_command(command, window, cx),
                }
            }
            PaletteEvent::Create(kind, title) => {
                let (kind, title) = (*kind, title.clone());
                self.close_overlay(window, cx);
                self.create(kind, title, window, cx);
            }
        }
    }

    /// A topic in the current epic (or the active project), or an epic in
    /// the active project; the new page opens.
    fn create(
        &mut self,
        kind: CreateKind,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let epic = self.current_epic(cx);
        let Some(bench) = self.bench() else { return };
        let Some(project) = bench.active_project else {
            return;
        };
        let ws = bench.ws.clone();
        let task = ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| match kind {
                CreateKind::Topic => store
                    .create_topic(
                        acta_core::store::NewTopic {
                            project,
                            epic,
                            title,
                            description: String::new(),
                            status: Status::Planned,
                        },
                        Author::Human,
                    )
                    .map(Route::Topic),
                CreateKind::Epic => store
                    .create_epic(project, &title, "", Author::Human)
                    .map(Route::Epic),
            })
        });
        cx.spawn_in(window, async move |shell, cx| {
            if let Some(route) = task.await {
                let _ = shell.update_in(cx, |shell, window, cx| {
                    shell.navigate(route, true, window, cx)
                });
            }
        })
        .detach();
    }

    fn current_epic(&mut self, cx: &mut Context<Self>) -> Option<EpicId> {
        let bench = self.bench()?;
        match bench.history.current()? {
            Route::Epic(id) => Some(id),
            Route::Topic(id) => {
                let projects = &bench.ws.read(cx).projects;
                tree::locate_topic(projects, id)?.epic.map(|e| e.0)
            }
            _ => None,
        }
    }

    fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::GoTopic => self.open_palette(Some(Group::Topics), None, window, cx),
            Command::SwitchProject => self.open_palette(Some(Group::Projects), None, window, cx),
            Command::NewTopic => self.open_palette(None, Some(CreateKind::Topic), window, cx),
            Command::NewEpic => self.open_palette(None, Some(CreateKind::Epic), window, cx),
            Command::NewProject => self.new_project(window, cx),
            Command::AddEntry => self.with_record(
                cx,
                |page, window, cx| page.focus_composer(window, cx),
                window,
            ),
            Command::Capture => self.open_capture(window, cx),
            Command::GoInbox => self.go_project_page(Route::Inbox, window, cx),
            Command::GoKnowledge => self.go_project_page(Route::Knowledge, window, cx),
            Command::ReviewMemories => {
                let Some(bench) = self.bench() else { return };
                let Some(project) = bench.active_project else {
                    return;
                };
                let ws = bench.ws.clone();
                let filters = Filters {
                    kind: Some(KbKind::Memory),
                    author: Some(Author::Claude),
                    level: None,
                };
                if let Some(bench) = self.bench() {
                    bench.history.open(Route::Knowledge(project));
                }
                let page = cx.new(|cx| KnowledgePage::new(ws, project, filters, window, cx));
                let sub = cx.subscribe_in(&page, window, Self::on_page);
                if let Some(bench) = self.bench() {
                    bench.page = PageView::Knowledge(page);
                    bench._page_sub = Some(sub);
                }
                cx.notify();
            }
            Command::NewDocument => {
                self.with_record(cx, |page, _, cx| page.new_kb(KbKind::Document, cx), window)
            }
            Command::NewMemory => {
                self.with_record(cx, |page, _, cx| page.new_kb(KbKind::Memory, cx), window)
            }
            Command::SetStatus(status) => {
                if let Some(Route::Topic(id)) = self.bench().and_then(|b| b.history.current()) {
                    let old = self.topic_status(id, cx);
                    let Some(bench) = self.bench() else { return };
                    bench.ws.update(cx, |ws, cx| {
                        let undo =
                            crate::workspace::Undo::Status(id, old.unwrap_or(Status::Planned));
                        ws.write_undoable(cx, undo, None, move |store| {
                            store.update_topic(
                                id,
                                acta_core::store::TopicPatch {
                                    status: Some(status),
                                    ..Default::default()
                                },
                                Author::Human,
                            )
                        });
                    });
                }
            }
            Command::Back => self.back(window, cx),
            Command::Forward => self.forward(window, cx),
            Command::Undo => self.undo(cx),
            Command::ToggleSidebar => self.toggle_sidebar(cx),
            Command::ToggleTerminal => self.toggle_terminal(window, cx),
            Command::RestartTerminal => self.restart_terminal_here(window, cx),
            Command::TerminalLarger => self.terminal_size(1., cx),
            Command::TerminalSmaller => self.terminal_size(-1., cx),
            Command::SetFolder => self.with_record(cx, |page, _, cx| page.set_folder(cx), window),
            Command::AddFolder => self.with_record(cx, |page, _, cx| page.add_folder(cx), window),
            Command::ShowFolders => {
                self.with_record(cx, |page, _, cx| page.show_folders(cx), window)
            }
            Command::Export => self.open_export(window, cx),
            Command::Import => self.import(window, cx),
            Command::Trash => self.navigate(Route::Trash, true, window, cx),
            Command::Snapshot => self.open_snapshots(window, cx),
            Command::Settings => self.open_settings(window, cx),
            Command::Theme(setting) => settings::set_theme(setting, cx),
            Command::Language(locale) => settings::set_language(locale, cx),
        }
    }

    fn topic_status(&mut self, id: TopicId, cx: &mut Context<Self>) -> Option<Status> {
        let bench = self.bench()?;
        let projects = &bench.ws.read(cx).projects;
        projects.iter().find_map(|t| {
            t.loose_topics
                .iter()
                .chain(t.epics.iter().flat_map(|(_, x)| x))
                .find(|x| x.id == id)
                .map(|x| x.status)
        })
    }

    fn with_record(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut RecordPage, &mut Window, &mut Context<RecordPage>),
        window: &mut Window,
    ) {
        let Some(bench) = self.bench() else { return };
        if let PageView::Record(page) = &bench.page {
            page.update(cx, |page, cx| f(page, window, cx));
        }
    }

    fn go_project_page(
        &mut self,
        make: fn(ProjectId) -> Route,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.bench().and_then(|b| b.active_project) else {
            return;
        };
        self.navigate(make(project), true, window, cx);
    }

    fn open_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let Some(project) = bench.active_project else {
            return;
        };
        bench.return_focus = window.focused(cx);
        let ws = bench.ws.clone();
        let draft = std::mem::take(&mut bench.capture_draft);
        let capture = cx.new(|cx| Capture::new(ws, project, draft, window, cx));
        let sub = cx.subscribe_in(
            &capture,
            window,
            |shell, _, event: &CaptureEvent, window, cx| {
                let CaptureEvent::Close(draft) = event;
                if let Some(bench) = shell.bench() {
                    bench.capture_draft = draft.clone();
                }
                shell.close_overlay(window, cx);
            },
        );
        let Some(bench) = self.bench() else { return };
        bench._subs.push(sub);
        bench.overlay = Some(Overlay::Capture(capture));
        cx.notify();
    }

    fn open_dialog(&mut self, kind: DialogKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.return_focus = window.focused(cx);
        let ws = bench.ws.clone();
        let dialog = cx.new(|cx| Dialog::new(ws, kind, window, cx));
        let sub = cx.subscribe_in(
            &dialog,
            window,
            |shell, _, event: &DialogEvent, window, cx| match event {
                DialogEvent::Close => shell.close_overlay(window, cx),
                DialogEvent::RestartTerminal => {
                    shell.close_overlay(window, cx);
                    shell.do_restart_terminal(window, cx);
                }
            },
        );
        let Some(bench) = self.bench() else { return };
        bench._subs.push(sub);
        bench.overlay = Some(Overlay::Dialog(dialog));
        cx.notify();
    }

    pub fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let project = bench.active_project.and_then(|id| {
            bench
                .ws
                .read(cx)
                .projects
                .iter()
                .find(|t| t.project.id == id)
                .map(|t| (id, t.project.title.clone()))
        });
        self.open_dialog(
            DialogKind::Export {
                project,
                this_project: false,
            },
            window,
            cx,
        );
    }

    /// "Import...": the system open dialog, then the preview.
    pub fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let db = bench.ws.read(cx).db.clone();
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |shell, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.pop() else { return };
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parsed = cx
                .background_spawn(async move {
                    let text = std::fs::read_to_string(&path).map_err(acta_core::Error::Io)?;
                    let file = ExportFile::from_json(&text)?;
                    let report = db.lock().import(&file, true)?;
                    Ok::<_, acta_core::Error>((file, report))
                })
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                let locale = settings::locale(cx);
                let kind = match parsed {
                    Ok((file, report)) => DialogKind::Import {
                        file_name,
                        file: std::sync::Arc::new(file),
                        report,
                    },
                    Err(acta_core::Error::FormatTooNew { found, .. }) => {
                        DialogKind::ImportRefused {
                            file_name,
                            message: tr(
                                locale,
                                Key::ImportNewer,
                                &[("n", Arg::Text(&found.to_string()))],
                            ),
                        }
                    }
                    Err(_) => DialogKind::ImportRefused {
                        file_name,
                        message: text(locale, Key::ImportNotActa),
                    },
                };
                shell.open_dialog(kind, window, cx);
            });
        })
        .detach();
    }

    pub fn open_snapshots(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let task = bench.ws.read(cx).db.run(cx, |store| store.snapshots());
        cx.spawn_in(window, async move |shell, cx| {
            if let Ok(mut list) = task.await {
                list.sort_by_key(|s| std::cmp::Reverse(s.taken_at));
                let _ = shell.update_in(cx, |shell, window, cx| {
                    shell.open_dialog(
                        DialogKind::Snapshots {
                            list,
                            selected: None,
                        },
                        window,
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    /// "New project...": the folder picker; the project is named after the
    /// folder, which becomes its primary working folder (8.5).
    pub fn new_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let ws = bench.ws.clone();
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn_in(window, async move |shell, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.pop() else { return };
            let name = crate::paths_view::last_segment(&path.to_string_lossy()).to_owned();
            let task = ws.update(cx, |ws, cx| {
                ws.write(cx, Key::ErrorSave, move |store| {
                    let id = store.create_project(&name, "", Author::Human)?;
                    store.add_folder(Owner::Project(id), &path, Author::Human)?;
                    Ok(id)
                })
            });
            if let Some(id) = task.await {
                let _ = shell.update_in(cx, |shell, window, cx| {
                    shell.navigate(Route::Inbox(id), true, window, cx)
                });
            }
        })
        .detach();
    }

    // ---- terminal ----

    fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let origin = self.origin(cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        let focused = dock.read(cx).focus_handle(cx).contains_focused(window, cx);
        if bench.dock_visible && focused {
            self.hide_terminal(window, cx);
            return;
        }
        let Some(origin) = origin else { return };
        bench.dock_visible = true;
        dock.update(cx, |dock, cx| dock.show(origin, window, cx));
        let handle = dock.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        cx.notify();
    }

    fn hide_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench.dock_visible = false;
        }
        self.focus_page(window, cx);
        cx.notify();
    }

    /// "Restart terminal here": confirmed while a process runs (6.4).
    fn restart_terminal_here(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(origin) = self.origin(cx) else {
            return;
        };
        let Some(bench) = self.bench() else { return };
        if bench.dock.read(cx).is_running(origin.project, cx) {
            self.open_dialog(DialogKind::RestartTerminal, window, cx);
        } else {
            self.do_restart_terminal(window, cx);
        }
    }

    fn do_restart_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(origin) = self.origin(cx) else {
            return;
        };
        let Some(bench) = self.bench() else { return };
        bench.dock_visible = true;
        bench
            .dock
            .update(cx, |dock, cx| dock.restart(origin, window, cx));
        cx.notify();
    }

    fn terminal_size(&mut self, by: f32, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench
                .dock
                .update(cx, |dock, cx| dock.change_text_size(by, cx));
        }
    }

    // ---- toast and undo ----

    fn show_toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let stays = toast.kind == ToastKind::Error;
        bench.toast = Some(toast);
        bench.toast_timer = if stays {
            Task::ready(())
        } else {
            cx.spawn(async move |shell, cx| {
                cx.background_executor().timer(TOAST_TIME).await;
                let _ = shell.update(cx, |shell, cx| {
                    if let Some(bench) = shell.bench() {
                        bench.toast = None;
                    }
                    cx.notify();
                });
            })
        };
        cx.notify();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.toast = None;
        bench.ws.update(cx, |ws, cx| ws.undo(cx));
        cx.notify();
    }

    // ---- tree ----

    fn rows(&mut self, cx: &mut Context<Self>) -> Vec<Row> {
        match self.bench() {
            Some(bench) => {
                let projects = &bench.ws.read(cx).projects;
                tree::rows(projects, &bench.folds)
            }
            None => Vec::new(),
        }
    }

    fn clamp_cursor(&mut self, cx: &mut Context<Self>) {
        let len = self.rows(cx).len();
        if let Some(bench) = self.bench() {
            bench.cursor = bench.cursor.min(len.saturating_sub(1));
        }
    }

    fn toggle_fold(&mut self, key: String, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench.folds.toggle(key);
        }
        self.save_layout(cx);
        self.clamp_cursor(cx);
        cx.notify();
    }

    fn tree_key(&mut self, key: TreeKey, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let Some(bench) = self.bench() else { return };
        if rows.is_empty() {
            return;
        }
        let at = bench.cursor.min(rows.len() - 1);
        let row = rows[at].clone();
        match key {
            TreeKey::Up => bench.cursor = at.saturating_sub(1),
            TreeKey::Down => bench.cursor = (at + 1).min(rows.len() - 1),
            TreeKey::Home => bench.cursor = 0,
            TreeKey::End => bench.cursor = rows.len() - 1,
            TreeKey::Left => match (row.fold_key(), row.is_expanded()) {
                (Some(k), Some(true)) => self.toggle_fold(k, cx),
                _ => {
                    if let Some(parent) = tree::parent_index(&rows, at) {
                        bench.cursor = parent;
                    }
                }
            },
            TreeKey::Right => match (row.fold_key(), row.is_expanded()) {
                (Some(k), Some(false)) => self.toggle_fold(k, cx),
                (Some(_), Some(true)) => bench.cursor = (at + 1).min(rows.len() - 1),
                _ => {}
            },
            TreeKey::Open => match row.route() {
                Some(route) => self.navigate(route, true, window, cx),
                None => {
                    if let Some(k) = row.fold_key() {
                        self.toggle_fold(k, cx);
                    }
                }
            },
            TreeKey::Delete => {
                let record = match row {
                    Row::Topic { id, .. } => Some(Record::Topic(id)),
                    Row::Epic { id, .. } => Some(Record::Epic(id)),
                    Row::Project { id, .. } => Some(Record::Project(id)),
                    _ => None,
                };
                if let Some(record) = record {
                    let locale = settings::locale(cx);
                    let message = text(locale, Key::TopicDeleted);
                    bench.ws.update(cx, |ws, cx| {
                        ws.write_undoable(
                            cx,
                            crate::workspace::Undo::Restore(record),
                            Some(message),
                            move |store| store.delete(record, Author::Human),
                        );
                    });
                }
            }
            TreeKey::MoveUp | TreeKey::MoveDown => {
                let up = key == TreeKey::MoveUp;
                let sibling = |r: &Row| match (r, &row) {
                    (Row::Topic { depth: a, .. }, Row::Topic { depth: b, .. }) => a == b,
                    (Row::Epic { .. }, Row::Epic { .. })
                    | (Row::Project { .. }, Row::Project { .. }) => true,
                    _ => false,
                };
                // The index among the siblings above it in its group.
                let start = tree::parent_index(&rows, at).map_or(0, |p| p + 1);
                let index = rows[start..at].iter().filter(|r| sibling(r)).count();
                let target = if up {
                    index.checked_sub(1)
                } else {
                    Some(index + 1)
                };
                let record = match row {
                    Row::Topic { id, .. } => Some(Record::Topic(id)),
                    Row::Epic { id, .. } => Some(Record::Epic(id)),
                    Row::Project { id, .. } => Some(Record::Project(id)),
                    _ => None,
                };
                if let (Some(record), Some(target)) = (record, target) {
                    bench.ws.update(cx, |ws, cx| {
                        ws.write_detached(cx, Key::ErrorSave, move |store| {
                            store.reorder(record, target, Author::Human)
                        });
                    });
                    bench.cursor = if up { at.saturating_sub(1) } else { at + 1 };
                }
            }
        }
        cx.notify();
    }

    fn sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let tokens = cx.global::<Tokens>().clone();
        let t = &tokens;
        let locale = settings::locale(cx);
        let rows = self.rows(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let current = bench.history.current();
        let cursor = bench.cursor;
        let tree_focus = bench.tree_focus.clone();
        let tree_focused = tree_focus.is_focused(window);
        let ws = bench.ws.clone();
        let _ = ws;
        let items: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| tree_row(row, i, current, tree_focused && i == cursor, locale, t, cx))
            .collect();
        let on_trash = current == Some(Route::Trash);
        let on_settings = current == Some(Route::Settings);
        let pinned = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .p(px(8.))
            .border_t_1()
            .border_color(t.border_subtle)
            .child(
                sidebar_row(
                    "sidebar-trash",
                    Icon::Trash2,
                    text(locale, Key::TreeTrash),
                    on_trash,
                    t,
                )
                .on_click(cx.listener(|shell, _, window, cx| {
                    shell.navigate(Route::Trash, true, window, cx)
                })),
            )
            .child(
                sidebar_row(
                    "sidebar-settings",
                    Icon::Settings,
                    text(locale, Key::TreeSettings),
                    on_settings,
                    t,
                )
                .on_click(cx.listener(|shell, _, window, cx| shell.open_settings(window, cx))),
            );
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(t.bg_sunken)
            .border_r_1()
            .border_color(t.border_subtle)
            .child(
                div()
                    .id("tree")
                    .key_context("Tree")
                    .track_focus(&tree_focus)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(8.))
                    .on_action(cx.listener(|s, _: &TreeUp, w, cx| s.tree_key(TreeKey::Up, w, cx)))
                    .on_action(
                        cx.listener(|s, _: &TreeDown, w, cx| s.tree_key(TreeKey::Down, w, cx)),
                    )
                    .on_action(
                        cx.listener(|s, _: &TreeLeft, w, cx| s.tree_key(TreeKey::Left, w, cx)),
                    )
                    .on_action(
                        cx.listener(|s, _: &TreeRight, w, cx| s.tree_key(TreeKey::Right, w, cx)),
                    )
                    .on_action(
                        cx.listener(|s, _: &TreeOpen, w, cx| s.tree_key(TreeKey::Open, w, cx)),
                    )
                    .on_action(
                        cx.listener(|s, _: &TreeDelete, w, cx| s.tree_key(TreeKey::Delete, w, cx)),
                    )
                    .on_action(
                        cx.listener(|s, _: &TreeMoveUp, w, cx| s.tree_key(TreeKey::MoveUp, w, cx)),
                    )
                    .on_action(cx.listener(|s, _: &TreeMoveDown, w, cx| {
                        s.tree_key(TreeKey::MoveDown, w, cx)
                    }))
                    .on_action(
                        cx.listener(|s, _: &TreeHome, w, cx| s.tree_key(TreeKey::Home, w, cx)),
                    )
                    .on_action(cx.listener(|s, _: &TreeEnd, w, cx| s.tree_key(TreeKey::End, w, cx)))
                    .children(items),
            )
            .child(pinned)
            .into_any_element()
    }

    fn status_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tokens = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let version = tr(
            locale,
            Key::StatusbarVersion,
            &[("version", Arg::Text(acta_core::VERSION))],
        );
        let project = self.bench().and_then(|bench| {
            let id = bench.active_project?;
            bench
                .ws
                .read(cx)
                .projects
                .iter()
                .find(|t| t.project.id == id)
                .map(|t| t.project.title.clone())
        });
        div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(STATUS_BAR_HEIGHT))
            .bg(tokens.bg_sunken)
            .border_t_1()
            .border_color(tokens.border_subtle)
            .text_color(tokens.text_tertiary)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h_full()
                    .children(project.map(|name| {
                        ui::bar_item("status-project", &tokens)
                            .child(icon(Icon::Folder, 14., tokens.text_tertiary))
                            .child(type_scale::SMALL.el().child(name))
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.open_palette(Some(Group::Projects), None, window, cx)
                            }))
                    }))
                    .child(
                        // The setup check of ADR 0005 is M3; until then the
                        // status reports setup only (layout.md 4.6).
                        ui::bar_item("status-claude", &tokens)
                            .child(icon(Icon::CircleDashed, 14., tokens.text_tertiary))
                            .child(
                                type_scale::SMALL
                                    .el()
                                    .child(text(locale, Key::StatusbarNotSetUp)),
                            )
                            .on_click(
                                cx.listener(|shell, _, window, cx| shell.open_settings(window, cx)),
                            ),
                    ),
            )
            .child(
                ui::bar_item("status-version", &tokens)
                    .child(
                        type_scale::SMALL
                            .el()
                            .font_features(type_scale::tabular())
                            .child(version),
                    )
                    .on_click(cx.listener(|shell, _, window, cx| shell.open_settings(window, cx))),
            )
    }

    /// The breadcrumb (layout.md 4.3): project and epic as links, then the
    /// page itself.
    fn breadcrumb(&mut self, cx: &mut Context<Self>) -> Vec<Crumb> {
        let locale = settings::locale(cx);
        let Some(bench) = self.bench() else {
            return vec![Crumb {
                label: acta_core::APP_NAME.into(),
                route: None,
            }];
        };
        let projects = bench.ws.read(cx).projects.clone();
        let crumb = |label: String, route: Option<Route>| Crumb {
            label: label.into(),
            route,
        };
        let project_crumb = |id: ProjectId| {
            tree::locate_project(&projects, id)
                .and_then(|l| l.project)
                .map(|(id, name)| crumb(name, Some(Route::Project(id))))
        };
        match (bench.history.current(), &bench.page) {
            (_, PageView::FirstRun(_)) | (None, _) => vec![crumb(acta_core::APP_NAME.into(), None)],
            (Some(Route::Topic(id)), _) => match tree::locate_topic(&projects, id) {
                Some(at) => {
                    let mut out = Vec::new();
                    if let Some((p, name)) = at.project {
                        out.push(crumb(name, Some(Route::Project(p))));
                    }
                    if let Some((e, name)) = at.epic {
                        out.push(crumb(name, Some(Route::Epic(e))));
                    }
                    if let Some((_, name)) = at.topic {
                        out.push(crumb(name, None));
                    }
                    out
                }
                None => vec![],
            },
            (Some(Route::Epic(id)), _) => match tree::locate_epic(&projects, id) {
                Some(at) => {
                    let mut out = Vec::new();
                    if let Some((p, name)) = at.project {
                        out.push(crumb(name, Some(Route::Project(p))));
                    }
                    if let Some((_, name)) = at.epic {
                        out.push(crumb(name, None));
                    }
                    out
                }
                None => vec![],
            },
            (Some(Route::Project(id)), _) => project_crumb(id)
                .map(|mut c| {
                    c.route = None;
                    vec![c]
                })
                .unwrap_or_default(),
            (Some(Route::Inbox(id)), _) => project_crumb(id)
                .into_iter()
                .chain([crumb(text(locale, Key::TreeInbox), None)])
                .collect(),
            (Some(Route::Knowledge(id)), _) => project_crumb(id)
                .into_iter()
                .chain([crumb(text(locale, Key::TreeKnowledge), None)])
                .collect(),
            (Some(Route::Kb(_)), _) => bench
                .active_project
                .and_then(project_crumb)
                .into_iter()
                .chain([crumb(
                    text(locale, Key::TreeKnowledge),
                    bench.active_project.map(Route::Knowledge),
                )])
                .collect(),
            (Some(Route::Trash), _) => vec![crumb(text(locale, Key::TreeTrash), None)],
            (Some(Route::Settings), _) => vec![crumb(text(locale, Key::TreeSettings), None)],
        }
    }

    pub fn open_crumb(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(route, true, window, cx);
    }

    // ---- key loss ----

    fn key_loss(&mut self, db: Option<PathBuf>, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let keychain = text(
            locale,
            if cfg!(windows) {
                Key::KeychainW
            } else if cfg!(target_os = "macos") {
                Key::KeychainM
            } else {
                Key::KeychainL
            },
        );
        let kept = format!(
            "acta.unreadable-{}.db",
            acta_core::time::file_stamp(acta_core::time::now_ms())
        );
        let path = db
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        ui::page_scroll("key-loss")
            .child(
                div()
                    .w_full()
                    .max_w(px(ui::COLUMN_MAX))
                    .pt(px(96.))
                    .flex()
                    .flex_col()
                    .child(
                        type_scale::TITLE
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::KeylossTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(12.))
                            .text_color(t.text_secondary)
                            .child(tr(
                                locale,
                                Key::KeylossBody,
                                &[("keychain", Arg::Text(&keychain))],
                            )),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(12.))
                            .text_color(t.text_primary)
                            .child(text(locale, Key::KeylossUntouched)),
                    )
                    .child(
                        type_scale::CODE
                            .el()
                            .mt(px(4.))
                            .text_color(t.text_primary)
                            .child(path),
                    )
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(
                                ui::button(
                                    "keyloss-import",
                                    text(locale, Key::KeylossImport),
                                    Variant::Primary,
                                    false,
                                    &t,
                                )
                                .on_click(cx.listener(
                                    |shell, _, window, cx| shell.start_empty(true, window, cx),
                                )),
                            )
                            .child(
                                ui::button(
                                    "keyloss-retry",
                                    text(locale, Key::CommonTryAgain),
                                    Variant::Secondary,
                                    false,
                                    &t,
                                )
                                .on_click(cx.listener(
                                    |shell, _, window, cx| {
                                        shell.content = Content::Opening;
                                        shell.open_store(window, cx);
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(24.))
                            .text_color(t.text_secondary)
                            .child(tr(locale, Key::KeylossLimit, &[("file", Arg::Text(&kept))])),
                    )
                    .child(
                        div().mt(px(8.)).ml(px(-12.)).child(
                            ui::button(
                                "keyloss-empty",
                                text(locale, Key::KeylossStartEmpty),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(
                                |shell, _, window, cx| shell.start_empty(false, window, cx),
                            )),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// "Start empty" (and "Import an export...", which starts empty first):
    /// the unreadable file is renamed and kept, never deleted, and a new
    /// database is created next to it.
    fn start_empty(&mut self, then_import: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(config) = db::config() else { return };
        let dir = config.data_dir.clone();
        let opening = cx.background_spawn(async move {
            match acta_core::db::set_aside_unreadable(&dir) {
                Ok(_) => db::open(Some(config)),
                Err(err) => {
                    crate::log::line(&format!(
                        "key loss: could not set the file aside: {:?}",
                        err.kind()
                    ));
                    db::Opened::Failed {
                        db: Some(dir.join(acta_core::store::DB_FILE)),
                    }
                }
            }
        });
        self._open = cx.spawn_in(window, async move |shell, cx| {
            let opened = opening.await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                match opened {
                    Opened::Ready(store) => {
                        shell.start_workbench(Db::new(store), window, cx);
                        if then_import {
                            shell.import(window, cx);
                        }
                    }
                    Opened::KeyLoss { db } => shell.content = Content::KeyLoss { db: Some(db) },
                    Opened::Failed { db } => shell.content = Content::KeyLoss { db },
                }
                cx.notify();
            });
        });
    }

    /// The palette and quick capture: 96 below the window top, centred,
    /// no scrim; dialogs draw their own scrim.
    fn overlay_element(&mut self) -> Option<AnyElement> {
        let bench = self.bench()?;
        let top = |child: AnyElement| {
            deferred(
                div()
                    .absolute()
                    .top(px(96.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(child),
            )
            .into_any_element()
        };
        Some(match bench.overlay.as_ref()? {
            Overlay::Palette(palette) => top(palette.clone().into_any_element()),
            Overlay::Capture(capture) => top(capture.clone().into_any_element()),
            Overlay::Dialog(dialog) => deferred(dialog.clone()).into_any_element(),
        })
    }

    fn toast_element(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let bench = self.bench()?;
        let toast = bench.toast.clone()?;
        let glyph = match toast.kind {
            ToastKind::Error => Some(icon(Icon::TriangleAlert, 16., t.danger)),
            ToastKind::Done => Some(icon(Icon::Check, 16., t.success)),
            ToastKind::Undo => None,
        };
        Some(
            div()
                .absolute()
                .left(px(16.))
                .bottom(px(16.))
                .max_w(px(360.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .bg(t.bg_overlay)
                .border_1()
                .border_color(t.border_default)
                .rounded(px(RADIUS_MD))
                .shadow(vec![t.shadow_overlay()])
                .children(glyph)
                .child(
                    type_scale::UI
                        .el()
                        .flex_1()
                        .text_color(t.text_primary)
                        .child(toast.message.clone()),
                )
                .when(toast.kind == ToastKind::Undo, |this| {
                    this.child(
                        ui::button(
                            "toast-undo",
                            text(locale, Key::CommonUndo),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(|shell, _, _, cx| shell.undo(cx))),
                    )
                })
                .child(
                    ui::ghost_icon_button("toast-close", Icon::X, 24., false, false, true, &t)
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(bench) = shell.bench() {
                                bench.toast = None;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }

    fn bulk_notice(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let bench = self.bench()?;
        let offer = bench.ws.read(cx).bulk.clone()?;
        let n = offer.count as u64;
        Some(
            div()
                .px(px(32.))
                .pt(px(16.))
                .child(
                    ui::notice(
                        Some((Icon::Info, t.text_secondary)),
                        tr(
                            locale,
                            Key::FoldersBulk,
                            &[("n", Arg::Count(n)), ("root", Arg::Text(&offer.old_root))],
                        ),
                        &t,
                    )
                    .child(
                        ui::button(
                            "bulk-apply",
                            tr(locale, Key::FoldersBulkApply, &[("n", Arg::Count(n))]),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(bench) = shell.bench() {
                                bench.ws.update(cx, |ws, cx| ws.apply_bulk(cx));
                            }
                        })),
                    )
                    .child(
                        ui::button(
                            "bulk-dismiss",
                            text(locale, Key::CommonDismiss),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(bench) = shell.bench() {
                                bench.ws.update(cx, |ws, cx| ws.dismiss_bulk(cx));
                            }
                        })),
                    ),
                )
                .into_any_element(),
        )
    }
}

/// Debug builds started with `ACTA_DEV_SEED=1` fill an empty database with
/// sample data, the current directory as the project folder.
fn seed_if_asked(store: &mut acta_core::Store) {
    if cfg!(debug_assertions)
        && std::env::var_os("ACTA_DEV_SEED").is_some_and(|v| v == "1")
        && let Ok(dir) = std::env::current_dir()
        && let Err(err) = crate::dev_seed::seed(store, &dir)
    {
        crate::log::line(&format!(
            "dev seed failed: {}",
            crate::workspace::err_kind(&err)
        ));
    }
}

fn record(
    ws: Entity<Workspace>,
    owner: Owner,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (PageView, Option<Subscription>) {
    let page = cx.new(|cx| RecordPage::new(ws, owner, window, cx));
    let sub = cx.subscribe_in(&page, window, Shell::on_page);
    (PageView::Record(page), Some(sub))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeKey {
    Up,
    Down,
    Left,
    Right,
    Open,
    Delete,
    MoveUp,
    MoveDown,
    Home,
    End,
}

/// One row of the tree (docs/design/components.md 5, list / tree row).
fn tree_row(
    row: &Row,
    index: usize,
    current: Option<Route>,
    cursor: bool,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let selected = row.route().is_some() && row.route() == current;
    let id = SharedString::from(format!("tree-row-{index}"));
    let (hover, primary) = (t.fill_hover, t.text_primary);
    let rest = if selected {
        t.text_primary
    } else {
        t.text_secondary
    };
    let indent = row.depth() as f32 * 16.;
    let mut el = div()
        .id(id.clone())
        .group(id.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .pl(px(8. + indent))
        .pr(px(8.))
        .rounded(px(RADIUS_SM))
        .when(selected, |this| this.bg(t.fill_selected))
        .when(!selected, |this| this.hover(move |s| s.bg(hover)))
        .when(cursor, |this| this.border_2().border_color(t.focus_ring));
    let chevron = |expanded: bool, key: String, cx: &mut Context<Shell>| {
        div()
            .id(SharedString::from(format!("tree-fold-{index}")))
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                shell.toggle_fold(key.clone(), cx);
            }))
            .child(icon(
                if expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                16.,
                t.text_secondary,
            ))
    };
    let title = |role: type_scale::TypeRole, label: String, color| {
        role.el()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_color(color)
            .group_hover(id.clone(), move |s| s.text_color(primary))
            .child(label)
    };
    let count = |n: i64| {
        type_scale::SMALL
            .el()
            .font_features(type_scale::tabular())
            .text_color(if selected {
                t.text_secondary
            } else {
                t.text_tertiary
            })
            .child(short_count(n.max(0) as u64))
    };
    el = match row {
        Row::Project {
            title: name,
            expanded,
            id: pid,
        } => el
            .child(chevron(*expanded, pid.to_string(), cx))
            .child(title(type_scale::UI_STRONG, name.clone(), t.text_primary)),
        Row::Inbox { count: n, .. } => el
            .child(icon(Icon::Inbox, 16., rest))
            .child(title(
                if selected {
                    type_scale::UI_STRONG
                } else {
                    type_scale::UI
                },
                text(locale, Key::TreeInbox),
                rest,
            ))
            .when(*n > 0, |this| this.child(count(*n))),
        Row::Knowledge { count: n, .. } => el
            .child(icon(Icon::Library, 16., rest))
            .child(title(
                if selected {
                    type_scale::UI_STRONG
                } else {
                    type_scale::UI
                },
                text(locale, Key::TreeKnowledge),
                rest,
            ))
            .when(*n > 0, |this| this.child(count(*n))),
        Row::Epic {
            title: name,
            expanded,
            id: eid,
            ..
        } => el
            .child(chevron(*expanded, eid.to_string(), cx))
            .child(title(type_scale::LABEL, name.clone(), rest)),
        Row::Topic {
            title: name,
            status,
            ..
        } => {
            let color = if status.is_open() {
                rest
            } else {
                t.text_tertiary
            };
            el.child(ui::status_glyph(*status, 14., t)).child(title(
                if selected {
                    type_scale::UI_STRONG
                } else {
                    type_scale::UI
                },
                name.clone(),
                color,
            ))
        }
        Row::Closed {
            count: n,
            group,
            shown,
            ..
        } => {
            let key = group.key();
            let _ = shown;
            el.on_click(cx.listener(move |shell, _, _, cx| shell.toggle_fold(key.clone(), cx)))
                .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                    locale,
                    Key::TreeClosed,
                    &[("n", Arg::Count(*n as u64))],
                )))
        }
    };
    if let Some(route) = row.route() {
        el = el.on_click(cx.listener(move |shell, _, window, cx| {
            if let Some(bench) = shell.bench() {
                bench.cursor = index;
            }
            shell.navigate(route, true, window, cx)
        }));
    }
    el.into_any_element()
}

/// A pinned row of the sidebar (docs/design/components.md 5, list / tree row).
fn sidebar_row(
    id: &'static str,
    glyph: Icon,
    label: String,
    selected: bool,
    tokens: &Tokens,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (hover, primary) = (tokens.fill_hover, tokens.text_primary);
    let rest = if selected {
        tokens.text_primary
    } else {
        tokens.text_secondary
    };
    let role = if selected {
        type_scale::UI_STRONG
    } else {
        type_scale::UI
    };
    div()
        .id(id)
        .group(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(8.))
        .rounded(px(RADIUS_SM))
        .when(selected, |this| this.bg(tokens.fill_selected))
        .when(!selected, |this| this.hover(move |style| style.bg(hover)))
        .child(icon(glyph, 16., rest).group_hover(id, move |style| style.text_color(primary)))
        .child(
            role.el()
                .min_w_0()
                .truncate()
                .text_color(rest)
                .group_hover(id, move |style| style.text_color(primary))
                .child(label),
        )
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = cx.global::<Tokens>().clone();
        // Windows: 1px outline when not maximized (docs/design/layout.md 4.3).
        let outlined = cfg!(target_os = "windows") && !window.is_maximized();
        let crumbs = match &self.content {
            Content::Specimen(_) => vec![Crumb {
                label: "Type specimen".into(),
                route: None,
            }],
            Content::Workbench(_) => self.breadcrumb(cx),
            _ => vec![Crumb {
                label: acta_core::APP_NAME.into(),
                route: None,
            }],
        };
        let titlebar = titlebar::render(self, crumbs, window, cx);
        let is_bench = matches!(self.content, Content::Workbench(_));

        let body: AnyElement = match &self.content {
            Content::Opening => div().flex_1().into_any_element(),
            Content::KeyLoss { db } => {
                let db = db.clone();
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.key_loss(db, cx))
                    .into_any_element()
            }
            Content::Specimen(view) => div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(view.clone())
                .into_any_element(),
            Content::Workbench(_) => {
                let first_run =
                    matches!(self.bench().map(|b| &b.page), Some(PageView::FirstRun(_)));
                let sidebar =
                    (self.sidebar_visible && !first_run).then(|| self.sidebar(window, cx));
                let bulk = self.bulk_notice(cx);
                let toast = self.toast_element(cx);
                let bench = self.bench().expect("workbench");
                let page_view = bench.page.view();
                let page_focus = bench.page_focus.clone();
                let dock = bench.dock_visible.then(|| bench.dock.clone());
                let page = div()
                    .id("page-area")
                    .track_focus(&page_focus)
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(420.))
                    .h_full()
                    .children(bulk)
                    .child(div().flex_1().min_h_0().child(page_view))
                    .children(toast);
                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .children(sidebar)
                    .child(page)
                    .children(dock)
                    .into_any_element()
            }
        };
        let status_bar = is_bench.then(|| self.status_bar(cx));
        let overlay = self.overlay_element();

        let frame = type_scale::UI
            .el()
            .id("shell")
            .key_context("Workbench")
            .track_focus(&self.focus)
            .on_action(cx.listener(|shell, _: &ToggleSidebar, _, cx| shell.toggle_sidebar(cx)))
            .on_action(
                cx.listener(|shell, _: &OpenSettings, window, cx| shell.open_settings(window, cx)),
            )
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_action(cx.listener(|s, _: &OpenPalette, w, cx| s.open_palette(None, None, w, cx)))
            .on_action(cx.listener(|s, _: &GoToTopic, w, cx| {
                s.open_palette(Some(Group::Topics), None, w, cx)
            }))
            .on_action(cx.listener(|s, _: &SwitchProject, w, cx| {
                s.open_palette(Some(Group::Projects), None, w, cx)
            }))
            .on_action(
                cx.listener(|s, _: &NewTopic, w, cx| s.run_command(Command::NewTopic, w, cx)),
            )
            .on_action(
                cx.listener(|s, _: &AddEntry, w, cx| s.run_command(Command::AddEntry, w, cx)),
            )
            .on_action(cx.listener(|s, _: &CaptureNote, w, cx| s.open_capture(w, cx)))
            .on_action(cx.listener(|s, _: &GoInbox, w, cx| s.run_command(Command::GoInbox, w, cx)))
            .on_action(
                cx.listener(|s, _: &GoKnowledge, w, cx| s.run_command(Command::GoKnowledge, w, cx)),
            )
            .on_action(
                cx.listener(|s, _: &NewDocument, w, cx| s.run_command(Command::NewDocument, w, cx)),
            )
            .on_action(
                cx.listener(|s, _: &NewMemory, w, cx| s.run_command(Command::NewMemory, w, cx)),
            )
            .on_action(cx.listener(|s, _: &Back, w, cx| s.back(w, cx)))
            .on_action(cx.listener(|s, _: &Forward, w, cx| s.forward(w, cx)))
            .on_action(cx.listener(|s, _: &Undo, _, cx| s.undo(cx)))
            .on_action(cx.listener(|s, _: &ToggleTerminal, w, cx| s.toggle_terminal(w, cx)))
            .on_action(cx.listener(|s, _: &FocusSidebar, w, cx| {
                if let Some(bench) = s.bench() {
                    let handle = bench.tree_focus.clone();
                    s.sidebar_visible = true;
                    w.focus(&handle, cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|s, _: &FocusPage, w, cx| s.focus_page(w, cx)))
            .on_action(cx.listener(|s, _: &FocusTerminal, w, cx| {
                if !s.terminal_visible() {
                    s.toggle_terminal(w, cx);
                } else if let Some(bench) = s.bench() {
                    let handle = bench.dock.read(cx).focus_handle(cx);
                    w.focus(&handle, cx);
                }
            }))
            .on_action(
                cx.listener(|s, _: &acta_terminal::view::IncreaseTextSize, _, cx| {
                    s.terminal_size(1., cx)
                }),
            )
            .on_action(
                cx.listener(|s, _: &acta_terminal::view::DecreaseTextSize, _, cx| {
                    s.terminal_size(-1., cx)
                }),
            )
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(tokens.bg_base)
            .text_color(tokens.text_primary)
            .when(outlined, |this| {
                this.border_1().border_color(tokens.border_subtle)
            })
            .child(titlebar)
            .child(body)
            .children(status_bar)
            .children(overlay);

        if cfg!(target_os = "linux") {
            // Client-side decorations: resize edges and the window shadow.
            window_border().child(frame).into_any_element()
        } else {
            frame.into_any_element()
        }
    }
}

/// Options of an Acta window of the given default size, centred. `focus`
/// is false only for the test hook that must not take the foreground.
pub fn window_options(default_size: (f32, f32), focus: bool, cx: &App) -> WindowOptions {
    let inset = px(titlebar::TRAFFIC_LIGHT_INSET);
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(acta_core::APP_NAME.into()),
            appears_transparent: true,
            traffic_light_position: Some(point(inset, inset)),
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(default_size.0), px(default_size.1)),
            cx,
        ))),
        window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
        window_decorations: Some(WindowDecorations::Client),
        // The titlebar starts window moves itself on macOS (see titlebar.rs).
        app_owns_titlebar_drag: true,
        app_id: Some("acta".into()),
        focus,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_bindings_parse() {
        // `KeyBinding::new` panics on a keystroke the pinned GPUI rejects.
        assert!(key_bindings().len() > 30);
    }
}
