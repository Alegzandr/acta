//! The workbench window (docs/design/layout.md 4.2): titlebar, the tree
//! sidebar, the page area, the docks (terminal and chat) and the status
//! bar, plus the overlays (palette, quick capture, dialogs, toast).
//!
//! The store opens in the background at launch. A database whose key is
//! lost shows the key-loss recovery screen (screens.md 8.9) and is never
//! touched; with no project yet the first-run page asks for a folder
//! (8.5). Tabs are M5: one page per window, with back and forward history.
//!
//! Docks (layout.md 4.5): the terminal lives in the right dock; the chat in
//! the right or the bottom one (Windows; macOS and Linux open claude.ai in
//! the browser, ADR 0009). One tool window per dock: showing one in the
//! right dock hides the other there, and both go on running.

use std::path::PathBuf;
use std::time::Duration;

use acta_core::model::{Author, EpicId, KbKind, Owner, ProjectId, Record, Status, TopicId};
use acta_core::store::ExportFile;
use gpui_kit::component::window_border;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext, Bounds, Context, DragMoveEvent, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyBinding, ParentElement, PathPromptOptions,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task, TitlebarOptions,
    Window, WindowBounds, WindowDecorations, WindowOptions, actions, deferred, div, point, px,
    size,
};

use crate::capture::{Capture, CaptureEvent};
#[cfg(windows)]
use crate::chat::Kept;
#[cfg(windows)]
use crate::chat_panel::{ChatEvent, ChatPanel, Dock};
use crate::claude_setup;
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
use crate::panes;
use crate::record_page::{EditTitle, OpenStatusMenu, PageEvent, RecordPage};
use crate::route::{History, Route};
use crate::settings;
use crate::settings_page::SettingsPage;
use crate::terminal_dock::{DockEvent, Origin, TerminalDock};
use crate::theme::{self, RADIUS_LG, RADIUS_SM, Tokens};
use crate::titlebar::{self, Crumb};
use crate::trash_page::TrashPage;
use crate::tree::{self, Folds, Row};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{Toast, ToastKind, Workspace, WorkspaceEvent};

/// claude.ai: the chat panel's page, and what "Open claude.ai in your
/// browser" opens.
pub const CLAUDE_HOME: &str = "https://claude.ai/";

/// docs/design/layout.md 4.2: default 1440 x 900, minimum 880 x 560.
pub const DEFAULT_SIZE: (f32, f32) = (1440., 900.);
pub const MIN_SIZE: (f32, f32) = (880., 560.);
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
        /// `Mod+Shift+H`, palette "Toggle chat" (macOS and Linux: claude.ai
        /// in the browser).
        ToggleChat,
        /// `Mod+4` (Windows only).
        FocusChat,
        /// "Open claude.ai in your browser".
        OpenChatInBrowser,
        /// "Sign out and clear chat data...".
        ChatSignOut,
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
        /// Settings: "Export...".
        ExportData,
        /// Settings: "Import...".
        ImportData,
        /// Settings: "Restore from snapshot...".
        RestoreSnapshot,
        /// "Open trash".
        OpenTrash,
        /// `Esc` in the sidebar shown over the page: dismiss it.
        DismissSidebar,
        /// `Mod+/`, palette "Keyboard shortcuts".
        OpenShortcuts,
        /// Settings: "Remove from Claude Code" (confirmed, 6.4).
        RemoveClaudeSetup,
        /// `Mod+W`, palette "Close tab" (layout.md 4.4).
        CloseTab,
        /// `Ctrl+Tab` / `Ctrl+Shift+Tab`.
        NextTab,
        PreviousTab,
        /// `Mod+Enter` on a tree row: open it in a new tab.
        TreeOpenTab,
    ]
);

/// The active project as the status bar reports it (layout.md 4.6).
#[derive(Clone, Debug)]
struct ProjectFacts {
    project: ProjectId,
    has_folder: bool,
    /// The topic Claude Code wrote to last, and when.
    last: Option<(String, i64)>,
}

/// What a resize handle drags (docs/design/components.md 5, Resize handle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    Sidebar,
    Right,
    Bottom,
}

/// The value of a resize drag.
struct ResizeDrag(Handle);

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
    // interaction.md 6.1: on Windows and Linux every Ctrl+letter goes to
    // the focused terminal (readline keys), so app keys stop at it; on
    // macOS Acta keeps every Cmd shortcut there except undo and redo
    // (`plain` below) and the terminal's own copy, paste and text size.
    // While the chat's page has the keyboard (Windows) GPUI receives no
    // keys; the ones Acta keeps are routed from WebView2 (chat_windows.rs).
    let app = if mac { None } else { Some("!Terminal") };
    let plain = Some("!Terminal && !Input");
    let mut out = vec![
        KeyBinding::new(m("cmd-b", "ctrl-b"), ToggleSidebar, app),
        KeyBinding::new(m("cmd-,", "ctrl-,"), OpenSettings, app),
        KeyBinding::new(m("cmd-/", "ctrl-/"), OpenShortcuts, app),
        // Mod+W only outside editors until drafts survive a closed tab.
        KeyBinding::new(m("cmd-w", "ctrl-w"), CloseTab, plain),
        // interaction.md 6.1: Ctrl+Tab is kept from the terminal too (its
        // key routing hands it to the app).
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
        // The terminal text size from anywhere (settings 5); inside the
        // terminal its own key routing does it.
        KeyBinding::new(
            m("cmd-shift-=", "ctrl-alt-="),
            acta_terminal::view::IncreaseTextSize,
            Some("!Terminal"),
        ),
        KeyBinding::new(
            m("cmd-shift--", "ctrl-alt--"),
            acta_terminal::view::DecreaseTextSize,
            Some("!Terminal"),
        ),
        KeyBinding::new(m("cmd-enter", "ctrl-enter"), TreeOpenTab, Some("Tree")),
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
        KeyBinding::new(m("cmd-shift-h", "ctrl-shift-h"), ToggleChat, None),
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
        KeyBinding::new("escape", DismissSidebar, Some("Tree")),
    ];
    // interaction.md 6.1: only Windows has a chat panel to focus.
    if cfg!(windows) {
        out.push(KeyBinding::new(m("cmd-4", "ctrl-4"), FocusChat, None));
    }
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
    Shortcuts(Entity<crate::shortcuts_page::ShortcutsPage>),
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
            PageView::Shortcuts(v) => v.clone().into(),
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
    /// The active tab's history (layout.md 4.4).
    history: History,
    /// Every tab's history, in order; the active one's entry is refreshed
    /// from `history` whenever tabs change.
    tabs: Vec<History>,
    active_tab: usize,
    /// Closed tabs, most recent last ("Reopen closed tab").
    closed_tabs: Vec<History>,
    page: PageView,
    settings: Entity<SettingsPage>,
    folds: Folds,
    cursor: usize,
    tree_focus: FocusHandle,
    page_focus: FocusHandle,
    active_project: Option<ProjectId>,
    /// Where the next "New topic" goes when it was asked for from a tree
    /// row's `plus` (that project, and that epic if any).
    create_in: Option<(ProjectId, Option<EpicId>)>,
    recent: Vec<TopicId>,
    overlay: Option<Overlay>,
    /// Where focus returns when the overlay closes.
    return_focus: Option<FocusHandle>,
    capture_draft: String,
    /// After a reorder, the row the keyboard cursor follows once the tree
    /// reloads.
    pending_cursor: Option<Route>,
    toast: Option<Toast>,
    toast_timer: Task<()>,
    dock: Entity<TerminalDock>,
    dock_visible: bool,
    #[cfg(windows)]
    chat: Entity<ChatPanel>,
    _subs: Vec<Subscription>,
    _page_sub: Option<Subscription>,
}

enum Content {
    /// The store is opening (never shown for long; nothing is drawn).
    Opening,
    /// The database exists and no key Acta has opens it (8.9).
    KeyLoss {
        db: PathBuf,
        /// A key was found and does not open the file (B5 of the M7
        /// review): the body says the file may be damaged.
        unreadable: bool,
        /// The name the file would be kept under, fixed when the screen
        /// opens so that what it says is what "Start empty" does.
        kept: String,
        /// The import preview, before anything is set aside.
        dialog: Option<Entity<Dialog>>,
    },
    /// The database could not be opened for another reason (or there is no
    /// data directory): nothing is set aside, only Try again is offered.
    OpenFailed {
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
    /// Pane sizes (layout.md 4.2, 4.5), from the preferences and the
    /// resize handles.
    sizes: panes::Sizes,
    /// The handle being dragged; its size goes to the preferences once
    /// the drag ends.
    resizing: Option<Handle>,
    /// Whether the sidebar fitted beside the page in the last frame
    /// (layout.md 4.2, narrow behaviour).
    sidebar_fits: bool,
    /// `Mod+B` while the sidebar is auto-hidden: it shows over the page.
    sidebar_overlay: bool,
    /// What the status bar says about the active project (layout.md 4.6):
    /// read off the UI thread when the project or the data changes.
    facts: Option<ProjectFacts>,
    facts_key: Option<(ProjectId, u64)>,
    _facts: Task<()>,
    _activation: Subscription,
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
        let sizes = panes::Sizes::from_prefs(settings::current(cx));
        // layout.md 4.6: the setup is checked again when the window
        // regains focus.
        let activation = cx.observe_window_activation(window, |shell, window, cx| {
            if window.is_window_active() && matches!(shell.content, Content::Workbench(_)) {
                claude_setup::refresh_if_stale(cx);
            }
        });
        Self {
            content,
            start,
            sidebar_visible,
            sizes,
            resizing: None,
            sidebar_fits: true,
            sidebar_overlay: false,
            facts: None,
            facts_key: None,
            _facts: Task::ready(()),
            _activation: activation,
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
                    Opened::KeyLoss { db, unreadable } => shell.content = key_loss(db, unreadable),
                    Opened::Failed { db } => shell.content = Content::OpenFailed { db },
                }
                cx.notify();
            });
        });
    }

    fn start_workbench(&mut self, db: Db, window: &mut Window, cx: &mut Context<Self>) {
        let ws = cx.new(|cx| Workspace::new(db, cx));
        let dock = cx.new(|cx| TerminalDock::new(ws.clone(), cx));
        #[cfg(windows)]
        let chat = cx.new(|cx| ChatPanel::new(window, cx));
        let prefs = settings::current(cx).clone();
        let folds = Folds {
            collapsed: prefs.collapsed.iter().cloned().collect(),
            closed_shown: prefs.closed_shown.iter().cloned().collect(),
        };
        let settings_page = cx.new(|cx| SettingsPage::new(window, cx));
        settings_page.update(cx, |page, cx| page.attach(ws.clone(), cx));
        #[allow(unused_mut)]
        let mut subs = vec![
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
        #[cfg(windows)]
        subs.push(cx.subscribe_in(&chat, window, Self::on_chat));
        let first_run = cx.new(|_| FirstRun);
        let first_run_sub = Self::watch_first_run(&first_run, window, cx);
        let bench = Bench {
            ws,
            history: History::default(),
            tabs: vec![History::default()],
            active_tab: 0,
            closed_tabs: Vec::new(),
            page: PageView::FirstRun(first_run),
            settings: settings_page,
            folds,
            cursor: 0,
            tree_focus: cx.focus_handle(),
            page_focus: cx.focus_handle(),
            active_project: None,
            create_in: None,
            recent: Vec::new(),
            overlay: None,
            return_focus: None,
            capture_draft: String::new(),
            pending_cursor: None,
            toast: None,
            toast_timer: Task::ready(()),
            dock,
            dock_visible: false,
            #[cfg(windows)]
            chat,
            _subs: subs,
            _page_sub: Some(first_run_sub),
        };
        self.content = Content::Workbench(Box::new(bench));
        // layout.md 4.6: the setup is checked at launch.
        claude_setup::refresh(cx);
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
            // layout.md 4.4: tabs are restored at launch.
            let prefs = settings::current(cx).clone();
            // Keys are paired with their index first, so a key that no
            // longer parses does not shift which tab was active.
            let pairs: Vec<(usize, History)> = prefs
                .tabs
                .iter()
                .enumerate()
                .filter_map(|(i, k)| Route::parse(k).map(|r| (i, History::at(r))))
                .collect();
            let active = pairs
                .iter()
                .position(|(i, _)| *i == prefs.active_tab)
                .unwrap_or(0);
            let restored: Vec<History> = pairs.into_iter().map(|(_, h)| h).collect();
            if restored.len() >= 2 && has_projects && start.page != Page::Settings {
                bench.history = restored[active].clone();
                bench.tabs = restored;
                bench.active_tab = active;
            }
            let saved = bench
                .history
                .current()
                .or_else(|| prefs.page.as_deref().and_then(Route::parse));
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
        self.follow_cursor(cx);
        self.clamp_cursor(cx);
    }

    /// Put the keyboard cursor back on the row that moved.
    fn follow_cursor(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let Some(bench) = self.bench() else { return };
        if let Some(route) = bench.pending_cursor.take()
            && let Some(at) = rows.iter().position(|r| r.route() == Some(route))
        {
            bench.cursor = at;
        }
    }

    /// Debug builds: `ACTA_DEV_OVERLAY=palette|capture|terminal|chat|chat-palette|export|snapshots`
    /// opens that surface at launch, so captures need no input.
    fn dev_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !cfg!(debug_assertions) {
            return;
        }
        match std::env::var("ACTA_DEV_OVERLAY").as_deref() {
            Ok("palette") => self.open_palette(None, None, window, cx),
            Ok("capture") => self.open_capture(window, cx),
            Ok("terminal") => self.toggle_terminal(window, cx),
            // Shown without taking the keyboard, for captures.
            #[cfg(windows)]
            Ok("chat") => self.show_chat(false, window, cx),
            // The chat under the palette: the placeholder instead of the page.
            #[cfg(windows)]
            Ok("chat-palette") => {
                self.show_chat(false, window, cx);
                self.open_palette(None, None, window, cx);
            }
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

    /// Whether the sidebar is on screen, beside the page or over it.
    pub fn sidebar_visible(&self) -> bool {
        if self.sidebar_fits {
            self.sidebar_visible
        } else {
            self.sidebar_overlay
        }
    }

    /// `Mod+B`: show or hide the sidebar. While it is auto-hidden because
    /// the window is too narrow, it shows over the page instead (4.2).
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.content, Content::Workbench(_)) {
            return;
        }
        if self.sidebar_fits {
            self.sidebar_visible = !self.sidebar_visible;
            let hidden = !self.sidebar_visible;
            settings::change(cx, |prefs| prefs.sidebar_hidden = hidden);
        } else {
            self.sidebar_overlay = !self.sidebar_overlay;
        }
        cx.notify();
    }

    /// Dismiss the sidebar shown over the page and return to the page.
    fn dismiss_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_overlay {
            self.sidebar_overlay = false;
            self.focus_page(window, cx);
            cx.notify();
        }
    }

    // ---- pane sizes ----

    /// A resize handle moved to `position` (window coordinates).
    fn resize(
        &mut self,
        handle: Handle,
        position: gpui_kit::Point<gpui_kit::Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let viewport = window.viewport_size();
        let (x, y) = (f32::from(position.x), f32::from(position.y));
        match handle {
            Handle::Sidebar => self.sizes.sidebar = panes::sidebar_width(x),
            Handle::Right => {
                let width = (f32::from(viewport.width) - x).max(panes::DOCK_MIN);
                if self.terminal_visible() {
                    self.sizes.terminal = width;
                } else {
                    self.sizes.chat = width;
                }
            }
            Handle::Bottom => {
                self.sizes.bottom =
                    (f32::from(viewport.height) - STATUS_BAR_HEIGHT - y).max(panes::BOTTOM_MIN)
            }
        }
        self.resizing = Some(handle);
        cx.notify();
    }

    /// Double-click on a handle: the default size again.
    fn reset_size(&mut self, handle: Handle, cx: &mut Context<Self>) {
        let d = panes::Sizes::default();
        match handle {
            Handle::Sidebar => self.sizes.sidebar = d.sidebar,
            Handle::Right if self.terminal_visible() => self.sizes.terminal = d.terminal,
            Handle::Right => self.sizes.chat = d.chat,
            Handle::Bottom => self.sizes.bottom = d.bottom,
        }
        self.save_sizes(cx);
    }

    /// "Widen sidebar", "Narrow terminal" and the like: steps of 40.
    fn step_size(&mut self, handle: Handle, by: f32, cx: &mut Context<Self>) {
        match handle {
            Handle::Sidebar => {
                self.sizes.sidebar = panes::sidebar_width(self.sizes.sidebar + by);
                if !self.sidebar_visible {
                    self.toggle_sidebar(cx);
                }
            }
            Handle::Right | Handle::Bottom => {
                self.sizes.terminal = (self.sizes.terminal + by).max(panes::DOCK_MIN)
            }
        }
        self.save_sizes(cx);
    }

    fn save_sizes(&mut self, cx: &mut Context<Self>) {
        self.resizing = None;
        let sizes = self.sizes;
        settings::change(cx, |prefs| sizes.to_prefs(prefs));
        cx.notify();
    }

    /// The titlebar shows only the name and the window controls: first
    /// run, key loss, open failed (screens.md 8.5, 8.9).
    pub fn bare_titlebar(&self) -> bool {
        match &self.content {
            Content::Workbench(bench) => matches!(bench.page, PageView::FirstRun(_)),
            Content::Specimen(_) => false,
            _ => true,
        }
    }

    pub fn terminal_visible(&self) -> bool {
        match &self.content {
            Content::Workbench(bench) => bench.dock_visible,
            _ => false,
        }
    }

    #[cfg(windows)]
    pub fn chat_visible(&self, cx: &App) -> bool {
        match &self.content {
            Content::Workbench(bench) => bench.chat.read(cx).is_shown(),
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
        // Opening something dismisses the sidebar shown over the page.
        self.sidebar_overlay = false;
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
            Route::Shortcuts => {
                let page = cx.new(|cx| crate::shortcuts_page::ShortcutsPage::new(window, cx));
                (PageView::Shortcuts(page), None)
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
            Route::Kb(_) | Route::Trash | Route::Settings | Route::Shortcuts => None,
        };
        if let Some(project) = project {
            bench.active_project = Some(project);
            bench.folds.expand(&project.to_string());
        } else if bench.active_project.is_none() {
            // A page of no project opened first (Settings, Trash at
            // launch): the first project in the tree, so the status bar
            // and the project commands have one (layout.md 4.6).
            bench.active_project = projects.first().map(|t| t.project.id);
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
                if let Some(bench) = self.bench() {
                    bench.active_project = Some(project);
                }
                // The breadcrumb and the tree now know the entry's owner.
                cx.notify();
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
        bench.tabs[bench.active_tab] = bench.history.clone();
        let (tabs, active_tab) = if bench.tabs.len() >= 2 {
            (
                bench
                    .tabs
                    .iter()
                    .filter_map(|h| h.current().map(Route::key))
                    .collect::<Vec<_>>(),
                bench.active_tab,
            )
        } else {
            (Vec::new(), 0)
        };
        let current = settings::current(cx);
        if current.page == page
            && current.collapsed == collapsed
            && current.closed_shown == closed
            && current.tabs == tabs
            && current.active_tab == active_tab
        {
            return;
        }
        settings::change(cx, |prefs| {
            prefs.page = page;
            prefs.collapsed = collapsed;
            prefs.closed_shown = closed;
            prefs.tabs = tabs;
            prefs.active_tab = active_tab;
        });
    }

    // ---- tabs (layout.md 4.4) ----

    /// Open `route` in a new tab, or focus the tab that already shows it.
    fn open_in_tab(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.tabs[bench.active_tab] = bench.history.clone();
        if let Some(at) = bench.tabs.iter().position(|h| h.current() == Some(route)) {
            self.switch_tab(at, window, cx);
            return;
        }
        bench.tabs.push(History::at(route));
        let at = bench.tabs.len() - 1;
        self.switch_tab(at, window, cx);
    }

    fn switch_tab(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if at >= bench.tabs.len() {
            return;
        }
        bench.tabs[bench.active_tab] = bench.history.clone();
        bench.active_tab = at;
        bench.history = bench.tabs[at].clone();
        if let Some(route) = bench.history.current() {
            self.show(route, window, cx);
        }
        cx.notify();
    }

    /// Close tab `at`; the last remaining tab cannot be closed.
    fn close_tab(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if bench.tabs.len() <= 1 || at >= bench.tabs.len() {
            return;
        }
        bench.tabs[bench.active_tab] = bench.history.clone();
        let closed = bench.tabs.remove(at);
        bench.closed_tabs.push(closed);
        let was_active = at == bench.active_tab;
        if at < bench.active_tab || bench.active_tab >= bench.tabs.len() {
            bench.active_tab = bench.active_tab.saturating_sub(1);
        }
        bench.history = bench.tabs[bench.active_tab].clone();
        if was_active && let Some(route) = bench.history.current() {
            self.show(route, window, cx);
        } else {
            self.save_layout(cx);
        }
        cx.notify();
    }

    fn close_other_tabs(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.tabs[bench.active_tab] = bench.history.clone();
        let keep = bench.tabs.remove(bench.active_tab);
        bench.closed_tabs.append(&mut bench.tabs);
        bench.tabs = vec![keep];
        bench.active_tab = 0;
        self.save_layout(cx);
        cx.notify();
    }

    fn reopen_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let Some(history) = bench.closed_tabs.pop() else {
            return;
        };
        bench.tabs[bench.active_tab] = bench.history.clone();
        bench.tabs.push(history);
        let at = bench.tabs.len() - 1;
        self.switch_tab(at, window, cx);
    }

    /// `Ctrl+Tab` / `Ctrl+Shift+Tab`, wrapping.
    fn cycle_tab(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let n = bench.tabs.len();
        if n < 2 {
            return;
        }
        let at = if forward {
            (bench.active_tab + 1) % n
        } else {
            (bench.active_tab + n - 1) % n
        };
        self.switch_tab(at, window, cx);
    }

    /// A tab's icon and title (layout.md 4.4).
    fn tab_label(&self, route: Route, locale: Locale, cx: &App) -> (Icon, Option<Status>, String) {
        let Content::Workbench(bench) = &self.content else {
            return (Icon::FileText, None, String::new());
        };
        let projects = &bench.ws.read(cx).projects;
        match route {
            Route::Topic(id) => {
                let topic = projects
                    .iter()
                    .flat_map(|p| {
                        p.loose_topics
                            .iter()
                            .chain(p.epics.iter().flat_map(|(_, t)| t))
                    })
                    .find(|t| t.id == id);
                match topic {
                    Some(t) => (Icon::Circle, Some(t.status), t.title.clone()),
                    None => (Icon::Circle, None, id.to_string()),
                }
            }
            Route::Epic(id) => (
                Icon::Layers,
                None,
                tree::locate_epic(projects, id)
                    .and_then(|l| l.epic.map(|e| e.1))
                    .unwrap_or_else(|| id.to_string()),
            ),
            Route::Project(id) => (
                Icon::Folder,
                None,
                tree::locate_project(projects, id)
                    .and_then(|l| l.project.map(|p| p.1))
                    .unwrap_or_else(|| id.to_string()),
            ),
            Route::Kb(id) => {
                let title = match &bench.page {
                    PageView::Doc(page) if bench.history.current() == Some(route) => {
                        page.read(cx).located().and_then(|(_, t)| t)
                    }
                    _ => None,
                };
                (
                    Icon::FileText,
                    None,
                    title.unwrap_or_else(|| id.to_string()),
                )
            }
            Route::Inbox(_) => (Icon::Inbox, None, text(locale, Key::TreeInbox)),
            Route::Knowledge(_) => (Icon::Library, None, text(locale, Key::TreeKnowledge)),
            Route::Trash => (Icon::Trash2, None, text(locale, Key::TreeTrash)),
            Route::Settings => (Icon::Settings, None, text(locale, Key::TreeSettings)),
            Route::Shortcuts => (Icon::Settings, None, text(locale, Key::CmdShortcuts)),
        }
    }

    /// The tab bar (layout.md 4.4): 32 high, `bg.sunken`, only with two or
    /// more tabs, across the page area.
    fn tab_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let (routes, active) = match &self.content {
            Content::Workbench(bench) if bench.tabs.len() >= 2 => {
                let mut tabs = bench.tabs.clone();
                tabs[bench.active_tab] = bench.history.clone();
                (
                    tabs.iter().map(|h| h.current()).collect::<Vec<_>>(),
                    bench.active_tab,
                )
            }
            _ => return None,
        };
        let mut bar = div()
            .id("tab-bar")
            .flex()
            .flex_row()
            .flex_shrink_0()
            .h(px(32.))
            .w_full()
            .overflow_x_scroll()
            .bg(t.bg_sunken)
            .border_b_1()
            .border_color(t.border_subtle);
        for (i, route) in routes.into_iter().enumerate() {
            let Some(route) = route else { continue };
            let (glyph, status, title) = self.tab_label(route, locale, cx);
            let is_active = i == active;
            let group = SharedString::from(format!("tab-{i}"));
            let hover = t.fill_hover;
            let leading = match status {
                Some(status) => ui::status_glyph(status, 14., &t),
                None => icon(glyph, 14., t.text_secondary),
            };
            let close = ui::ghost_icon_button(
                format!("tab-close-{i}"),
                Icon::X,
                16.,
                false,
                false,
                true,
                &t,
            )
            .when(!is_active, |this| {
                this.invisible().group_hover(group.clone(), |s| s.visible())
            })
            .on_click(cx.listener(move |shell, _, window, cx| {
                cx.stop_propagation();
                shell.close_tab(i, window, cx)
            }));
            bar = bar.child(
                ui::focusable(div().id(("tab", i)), &t, false)
                    .group(group)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h_full()
                    .px(px(12.))
                    .min_w(px(120.))
                    .max_w(px(220.))
                    .flex_shrink(1.)
                    .when(is_active, |this| this.bg(t.bg_base))
                    .when(!is_active, |this| this.hover(move |s| s.bg(hover)))
                    .on_click(
                        cx.listener(move |shell, _, window, cx| shell.switch_tab(i, window, cx)),
                    )
                    .on_mouse_down(
                        gpui_kit::MouseButton::Middle,
                        cx.listener(move |shell, _, window, cx| shell.close_tab(i, window, cx)),
                    )
                    .child(leading)
                    .child(
                        type_scale::UI
                            .el()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(if is_active {
                                t.text_primary
                            } else {
                                t.text_secondary
                            })
                            .child(title),
                    )
                    .child(close),
            );
        }
        Some(bar.into_any_element())
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
        let context = self.command_context(cx);
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

    fn command_context(&self, cx: &App) -> commands::Context {
        // Only the chat (Windows) needs the app here.
        #[cfg(not(windows))]
        let _ = cx;
        match &self.content {
            Content::Workbench(bench) => commands::Context {
                has_project: bench.active_project.is_some(),
                on_topic: matches!(bench.history.current(), Some(Route::Topic(_))),
                on_record: matches!(bench.page, PageView::Record(_)),
                can_back: bench.history.can_go_back(),
                can_forward: bench.history.can_go_forward(),
                tabs: bench.tabs.len() > 1,
                closed_tab: !bench.closed_tabs.is_empty(),
                #[cfg(windows)]
                chat_bottom: bench.chat.read(cx).dock == Dock::Bottom,
            },
            _ => commands::Context::default(),
        }
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Content::KeyLoss { dialog, .. } = &mut self.content {
            *dialog = None;
            cx.notify();
            return;
        }
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
            PaletteEvent::Dismiss => {
                if let Some(bench) = self.bench() {
                    bench.create_in = None;
                }
                self.close_overlay(window, cx)
            }
            PaletteEvent::Run(target, new_tab) => {
                let (target, new_tab) = (target.clone(), *new_tab);
                self.close_overlay(window, cx);
                let route = match target {
                    Target::Topic(id, _) => Route::Topic(id),
                    Target::Project(id) => Route::Project(id),
                    Target::Kb(id, _) => Route::Kb(id),
                    Target::Command(command) => return self.run_command(command, window, cx),
                };
                if new_tab {
                    self.open_in_tab(route, window, cx);
                } else {
                    self.navigate(route, true, window, cx);
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
        let mut epic = self.current_epic(cx);
        let Some(bench) = self.bench() else { return };
        let mut project = bench.active_project;
        if let Some((p, e)) = bench.create_in.take() {
            project = Some(p);
            epic = e;
        }
        let Some(project) = project else {
            return;
        };
        let ws = bench.ws.clone();
        // Undoable: undo sends the new record to the trash.
        let task = ws.update(cx, |ws, cx| {
            ws.write_undoable(cx, Some(project), None, move |store| {
                let (record, route) = match kind {
                    CreateKind::Topic => {
                        let id = store.create_topic(
                            acta_core::store::NewTopic {
                                project,
                                epic,
                                title,
                                description: String::new(),
                                status: Status::Planned,
                            },
                            Author::Human,
                        )?;
                        (Record::Topic(id), Route::Topic(id))
                    }
                    CreateKind::Epic => {
                        let id = store.create_epic(project, &title, "", Author::Human)?;
                        (Record::Epic(id), Route::Epic(id))
                    }
                };
                Ok((crate::workspace::Undo::Delete(record), route))
            })
        });
        cx.spawn_in(window, async move |shell, cx| {
            if let Ok(route) = task.await {
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
                if let Some(Route::Topic(id)) = self.bench().and_then(|b| b.history.current())
                    && let Some(bench) = self.bench()
                {
                    // The previous status is read in the write itself.
                    bench.ws.update(cx, |ws, cx| ws.set_status(id, status, cx));
                }
            }
            Command::Back => self.back(window, cx),
            Command::Forward => self.forward(window, cx),
            Command::Undo => self.undo(cx),
            Command::ToggleSidebar => self.toggle_sidebar(cx),
            Command::WidenSidebar => self.step_size(Handle::Sidebar, panes::STEP, cx),
            Command::NarrowSidebar => self.step_size(Handle::Sidebar, -panes::STEP, cx),
            Command::WidenTerminal => self.step_size(Handle::Right, panes::STEP, cx),
            Command::NarrowTerminal => self.step_size(Handle::Right, -panes::STEP, cx),
            Command::ToggleTerminal => self.toggle_terminal(window, cx),
            #[cfg(windows)]
            Command::ToggleChat => self.toggle_chat(window, cx),
            #[cfg(windows)]
            Command::ChatBottom => self.move_chat(Dock::Bottom, window, cx),
            #[cfg(windows)]
            Command::ChatRight => self.move_chat(Dock::Right, window, cx),
            Command::ChatBrowser => cx.open_url(CLAUDE_HOME),
            #[cfg(windows)]
            Command::ChatSignOut => self.open_dialog(DialogKind::ChatSignOut, window, cx),
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
            Command::Shortcuts => self.navigate(Route::Shortcuts, true, window, cx),
            Command::CloseTab => {
                if let Some(at) = self.bench().map(|b| b.active_tab) {
                    self.close_tab(at, window, cx)
                }
            }
            Command::CloseOtherTabs => self.close_other_tabs(cx),
            Command::ReopenTab => self.reopen_tab(window, cx),
            Command::Theme(setting) => settings::set_theme(setting, cx),
            Command::TextSize(size) => settings::set_text_size(size, cx),
            Command::ReadingWidth(width) => settings::set_reading_width(width, cx),
            Command::Language(locale) => settings::set_language(locale, cx),
        }
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
        let ws = self.bench().map(|bench| bench.ws.clone());
        let return_focus = window.focused(cx);
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
                DialogEvent::RemoveSetup => {
                    shell.close_overlay(window, cx);
                    claude_setup::run_remove(cx);
                }
                #[cfg(windows)]
                DialogEvent::ChatSignOut => {
                    shell.close_overlay(window, cx);
                    if let Some(bench) = shell.bench() {
                        let chat = bench.chat.clone();
                        chat.update(cx, |chat, cx| chat.sign_out(window, cx));
                    }
                }
                DialogEvent::Recover(file) => {
                    let file = std::sync::Arc::clone(file);
                    shell.close_overlay(window, cx);
                    shell.start_empty(Some(file), window, cx);
                }
            },
        );
        match &mut self.content {
            Content::Workbench(bench) => {
                bench.return_focus = return_focus;
                bench._subs.push(sub);
                bench.overlay = Some(Overlay::Dialog(dialog));
            }
            Content::KeyLoss { dialog: slot, .. } => {
                sub.detach();
                *slot = Some(dialog);
            }
            _ => {}
        }
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
    /// "Import...": the system open dialog, then the preview. On the
    /// key-loss screen (no database open) the preview shows what an empty
    /// database would get, and nothing is set aside before Import.
    pub fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let db = self.bench().map(|bench| bench.ws.read(cx).db.clone());
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
            let recovery = db.is_none();
            let parsed = cx
                .background_spawn(async move {
                    // The file is read before the store is taken.
                    let text = std::fs::read_to_string(&path).map_err(acta_core::Error::Io)?;
                    let file = ExportFile::from_json(&text)?;
                    let report = match db {
                        Some(db) => db.lock().import(&file, true)?,
                        None => {
                            if file.version > acta_core::store::FORMAT_VERSION {
                                return Err(acta_core::Error::FormatTooNew {
                                    found: file.version,
                                    supported: acta_core::store::FORMAT_VERSION,
                                });
                            }
                            crate::dialogs::report_into_empty(&file)
                        }
                    };
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
                        recovery,
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
        // The list and each file's size, read off the UI thread.
        let task = bench.ws.read(cx).db.run(cx, |store| {
            let mut list = store.snapshots()?;
            list.sort_by_key(|s| std::cmp::Reverse(s.taken_at));
            let sizes = list
                .iter()
                .map(|s| std::fs::metadata(&s.path).map(|m| m.len()).unwrap_or(0))
                .collect::<Vec<u64>>();
            Ok((list, sizes))
        });
        cx.spawn_in(window, async move |shell, cx| {
            if let Ok((list, sizes)) = task.await {
                let _ = shell.update_in(cx, |shell, window, cx| {
                    shell.open_dialog(
                        DialogKind::Snapshots {
                            list,
                            sizes,
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
    /// folder, which becomes its primary working folder (8.5). Undoable.
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
                ws.write_undoable(cx, None, None, move |store| {
                    let id = store.create_project(&name, "", Author::Human)?;
                    store.add_folder(Owner::Project(id), &path, Author::Human)?;
                    Ok((crate::workspace::Undo::Delete(Record::Project(id)), id))
                })
            });
            if let Ok(id) = task.await {
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
        #[cfg(windows)]
        hide_right_chat(bench, cx);
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
        #[cfg(windows)]
        hide_right_chat(bench, cx);
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

    // ---- chat ----

    #[cfg(windows)]
    fn on_chat(
        &mut self,
        _: &Entity<ChatPanel>,
        event: &ChatEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ChatEvent::Hide => self.hide_chat(window, cx),
            ChatEvent::Move(dock) => self.move_chat(*dock, window, cx),
            ChatEvent::SignOut => self.open_dialog(DialogKind::ChatSignOut, window, cx),
            ChatEvent::Kept(kept) => self.run_kept(*kept, window, cx),
            ChatEvent::ClearFailed => {
                let message = text(settings::locale(cx), Key::ChatClearFailed);
                if let Some(bench) = self.bench() {
                    bench
                        .ws
                        .update(cx, |ws, cx| ws.toast(message, ToastKind::Error, cx));
                }
            }
        }
    }

    #[cfg(windows)]
    /// A key Acta keeps, pressed while the page had the keyboard
    /// (interaction.md 6.1). Keys of features that do not exist yet (tabs,
    /// pane cycling, zoom) are kept from the page and do nothing.
    fn run_kept(&mut self, kept: Kept, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let chat = bench.chat.clone();
        let action: Box<dyn gpui_kit::Action> = match kept {
            Kept::Palette => Box::new(OpenPalette),
            Kept::ToggleChat => Box::new(ToggleChat),
            Kept::ToggleTerminal => Box::new(ToggleTerminal),
            Kept::Focus(1) => Box::new(FocusSidebar),
            Kept::Focus(2) => Box::new(FocusPage),
            Kept::Focus(3) => Box::new(FocusTerminal),
            Kept::NextTab => Box::new(NextTab),
            Kept::PreviousTab => Box::new(PreviousTab),
            Kept::Focus(_) | Kept::NextPane | Kept::PreviousPane | Kept::ZoomToolWindow => return,
        };
        // The keyboard comes back to the GPUI window before the action
        // runs, so that what it focuses receives the next keys.
        chat.read(cx).release_keyboard(cx);
        window.dispatch_action(action, cx);
    }

    #[cfg(windows)]
    /// Show and focus the chat; if it has focus, hide it (layout.md 4.5).
    fn toggle_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let chat = bench.chat.clone();
        let focused = chat.read(cx).focus_handle(cx).contains_focused(window, cx);
        if chat.read(cx).is_shown() && focused {
            self.hide_chat(window, cx);
        } else {
            self.show_chat(true, window, cx);
        }
    }

    #[cfg(windows)]
    fn show_chat(&mut self, focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let chat = bench.chat.clone();
        if chat.read(cx).dock == Dock::Right {
            bench.dock_visible = false;
        }
        chat.update(cx, |chat, cx| {
            chat.show(window, cx);
            if focus && chat.is_shown() {
                chat.focus(window, cx);
            }
        });
        cx.notify();
    }

    #[cfg(windows)]
    fn hide_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let chat = bench.chat.clone();
        chat.update(cx, |chat, cx| chat.hide(cx));
        self.focus_page(window, cx);
        cx.notify();
    }

    #[cfg(windows)]
    /// `Mod+4`: focus the chat, showing it first if needed.
    fn focus_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let chat = bench.chat.clone();
        if chat.read(cx).is_shown() {
            chat.update(cx, |chat, cx| chat.focus(window, cx));
        } else {
            self.show_chat(true, window, cx);
        }
    }

    #[cfg(windows)]
    /// "Move chat to bottom" / "Move chat to right": the chat shows in its
    /// new dock and takes the right dock from the terminal if it goes there.
    fn move_chat(&mut self, dock: Dock, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let chat = bench.chat.clone();
        chat.update(cx, |chat, cx| chat.move_to(dock, cx));
        self.show_chat(true, window, cx);
    }

    #[cfg(windows)]
    /// Whether a GPUI layer that can overlap the chat is open
    /// (components-app.md 5: palette, quick capture, dialog, menu).
    fn chat_covered(&mut self, cx: &mut Context<Self>) -> bool {
        let sidebar_over = self.sidebar_overlay && !self.sidebar_fits;
        let Some(bench) = self.bench() else {
            return false;
        };
        let page_menu = match &bench.page {
            PageView::Record(page) => page.read(cx).menu_open(),
            PageView::Knowledge(page) => page.read(cx).menu_open(),
            PageView::Settings(page) => page.read(cx).menu_open(),
            PageView::Doc(page) => page.read(cx).menu_open(),
            _ => false,
        };
        sidebar_over || bench.overlay.is_some() || page_menu || bench.chat.read(cx).menu_open()
    }

    /// macOS and Linux: no panel, claude.ai opens in the browser
    /// (components-app.md 5, Chat panel).
    #[cfg(not(windows))]
    fn toggle_chat(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        cx.open_url(CLAUDE_HOME);
    }

    #[cfg(not(windows))]
    fn focus_chat(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    #[cfg(windows)]
    fn ask_chat_sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_dialog(DialogKind::ChatSignOut, window, cx);
    }

    #[cfg(not(windows))]
    fn ask_chat_sign_out(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    /// The chat for the right dock and for the bottom dock, with the web
    /// view hidden while an overlay is open.
    #[cfg(windows)]
    fn chat_docks(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> (Option<AnyElement>, Option<AnyElement>) {
        let covered = self.chat_covered(cx);
        let Some(bench) = self.bench() else {
            return (None, None);
        };
        let chat = bench.chat.clone();
        chat.update(cx, |chat, cx| chat.set_covered(covered, window, cx));
        let (shown, dock) = {
            let chat = chat.read(cx);
            (chat.is_shown(), chat.dock)
        };
        if !shown {
            return (None, None);
        }
        match dock {
            Dock::Right => (Some(chat.into_any_element()), None),
            Dock::Bottom => (None, Some(chat.into_any_element())),
        }
    }

    #[cfg(not(windows))]
    fn chat_docks(
        &mut self,
        _: &Window,
        _: &mut Context<Self>,
    ) -> (Option<AnyElement>, Option<AnyElement>) {
        (None, None)
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

    /// `Mod+Z`: the active project's stack (pages of no project, such as
    /// Settings, keep the project of the page before them).
    fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.toast = None;
        let project = bench.active_project;
        bench.ws.update(cx, |ws, cx| ws.undo_active(project, cx));
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
                if let Some(record) = tree::row_record(&row) {
                    let projects = bench.ws.read(cx).projects.clone();
                    let project = row_project(&projects, &row);
                    bench.ws.update(cx, |ws, cx| ws.delete(record, project, cx));
                }
            }
            TreeKey::MoveUp | TreeKey::MoveDown => {
                let up = key == TreeKey::MoveUp;
                let projects = bench.ws.read(cx).projects.clone();
                let Some(record) = tree::row_record(&row) else {
                    return;
                };
                // The siblings shown, in order: same kind and depth, inside
                // the same parent row.
                let start = tree::parent_index(&rows, at).map_or(0, |p| p + 1);
                let same = |r: &Row| match (r, &row) {
                    (Row::Topic { .. }, Row::Topic { .. })
                    | (Row::Epic { .. }, Row::Epic { .. })
                    | (Row::Project { .. }, Row::Project { .. }) => r.depth() == row.depth(),
                    _ => false,
                };
                let visible: Vec<Record> = rows[start..]
                    .iter()
                    .take_while(|r| r.depth() >= row.depth())
                    .filter(|r| same(r))
                    .filter_map(tree::row_record)
                    .collect();
                let stored = tree::stored_siblings(&projects, &row);
                if let Some(target) = tree::move_target(&stored, &visible, &record, up) {
                    let project = row_project(&projects, &row);
                    bench.pending_cursor = row.route();
                    bench
                        .ws
                        .update(cx, |ws, cx| ws.reorder(record, target, project, cx));
                }
            }
        }
        cx.notify();
    }

    fn sidebar(&mut self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let tokens = cx.global::<Tokens>().clone();
        let t = &tokens;
        let locale = settings::locale(cx);
        let rows = self.rows(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let mut current = bench.history.current();
        // A document or memory selects its owner's row (components-knowledge.md 5).
        if let (Some(Route::Kb(_)), PageView::Doc(page)) = (current, &bench.page)
            && let Some((owner, _)) = page.read(cx).located()
        {
            current = Some(match owner {
                Owner::Project(id) => Route::Project(id),
                Owner::Epic(id) => Route::Epic(id),
                Owner::Topic(id) => Route::Topic(id),
            });
        }
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
            .w(px(width))
            .h_full()
            .bg(t.bg_sunken)
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
                    .on_action(cx.listener(|s, _: &DismissSidebar, w, cx| s.dismiss_sidebar(w, cx)))
                    .on_action(cx.listener(|s, _: &TreeOpenTab, w, cx| {
                        let rows = s.rows(cx);
                        let route = s
                            .bench()
                            .and_then(|b| rows.get(b.cursor))
                            .and_then(Row::route);
                        if let Some(route) = route {
                            s.open_in_tab(route, w, cx);
                        }
                    }))
                    .children(items),
            )
            .child(pinned)
            .into_any_element()
    }

    /// Read again what the status bar says about the active project when
    /// the project or the data changed.
    fn refresh_facts(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let Some(project) = bench.active_project else {
            return;
        };
        let ws = bench.ws.read(cx);
        let key = (project, ws.revision);
        if self.facts_key == Some(key) {
            return;
        }
        let task = ws.db.run(cx, move |store| {
            let has_folder = !store.folders(Owner::Project(project))?.is_empty();
            let last = store
                .last_claude_write(project)?
                .map(|(_, title, at)| (title, at));
            Ok(ProjectFacts {
                project,
                has_folder,
                last,
            })
        });
        self.facts_key = Some(key);
        self._facts = cx.spawn(async move |shell, cx| {
            if let Ok(facts) = task.await {
                let _ = shell.update(cx, |shell, cx| {
                    shell.facts = Some(facts);
                    cx.notify();
                });
            }
        });
    }

    /// "Set working folder..." from the status bar: the folder picker, then
    /// the folder becomes the active project's primary one. Undoable.
    fn set_project_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let Some(project) = bench.active_project else {
            return;
        };
        let ws = bench.ws.clone();
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.pop() else { return };
            ws.update(cx, |ws, cx| {
                ws.undoable(cx, Some(project), None, move |store| {
                    let id = store.add_folder(Owner::Project(project), &path, Author::Human)?;
                    store.reorder(Record::Folder(id), 0, Author::Human)?;
                    Ok((crate::workspace::Undo::Delete(Record::Folder(id)), ()))
                });
            });
        })
        .detach();
    }

    fn status_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        self.refresh_facts(cx);
        let tokens = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let version = tr(
            locale,
            Key::StatusbarVersion,
            &[("version", Arg::Text(acta_core::VERSION))],
        );
        let active = self.bench().and_then(|bench| bench.active_project);
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
        let facts = self.facts.clone().filter(|f| Some(f.project) == active);
        let check = claude_setup::state(cx).and_then(|s| s.check);
        let status = claude_setup::status(check, facts.as_ref().map(|f| f.has_folder));
        let (glyph, glyph_color, key) = match status {
            claude_setup::Status::NotSetUp => (
                Icon::CircleDashed,
                tokens.text_tertiary,
                Key::StatusbarNotSetUp,
            ),
            claude_setup::Status::NotFound => (
                Icon::CircleSlash,
                tokens.text_tertiary,
                Key::StatusbarNotFound,
            ),
            claude_setup::Status::NoFolder => {
                (Icon::Circle, tokens.text_secondary, Key::StatusbarNoFolder)
            }
            claude_setup::Status::Ready => (
                Icon::CircleCheck,
                tokens.text_secondary,
                Key::StatusbarReady,
            ),
        };
        let last_wrote = (status == claude_setup::Status::Ready)
            .then(|| facts.and_then(|f| f.last))
            .flatten()
            .map(|(topic, at)| {
                let when = crate::when::long_alone(locale, at);
                tr(
                    locale,
                    Key::StatusbarLastWrote,
                    &[("when", Arg::Text(&when)), ("topic", Arg::Text(&topic))],
                )
            });
        let claude_item = ui::bar_item("status-claude", &tokens)
            .child(icon(glyph, 14., glyph_color))
            .child(type_scale::SMALL.el().child(text(locale, key)))
            .when_some(last_wrote, |this, tip| this.tooltip(ui::tip(tip)))
            .on_click(cx.listener(move |shell, _, window, cx| {
                if status == claude_setup::Status::NoFolder {
                    shell.set_project_folder(window, cx);
                } else {
                    shell.open_settings(window, cx);
                }
            }));
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
                    .child(claude_item),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h_full()
                    .when(settings::updated_this_session(cx), |this| {
                        // components.md 5, Update indicator: "Updated to
                        // 0.4.1" for the first session on a new version,
                        // opening the release notes; no dot.
                        this.child(
                            ui::bar_item("status-updated", &tokens)
                                .child(type_scale::SMALL.el().child(tr(
                                    locale,
                                    Key::UpdateDone,
                                    &[("version", Arg::Text(acta_core::VERSION))],
                                )))
                                .on_click(|_, _, cx| cx.open_url(crate::settings_page::RELEASES)),
                        )
                    })
                    .child(
                        ui::bar_item("status-version", &tokens)
                            .child(
                                type_scale::SMALL
                                    .el()
                                    .font_features(type_scale::tabular())
                                    .child(version),
                            )
                            .on_click(
                                cx.listener(|shell, _, window, cx| shell.open_settings(window, cx)),
                            ),
                    ),
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
            // A document or memory: its owner's place as links, then the
            // entry itself (layout.md 4.3).
            (Some(Route::Kb(_)), PageView::Doc(page)) => {
                let Some((owner, title)) = page.read(cx).located() else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                match owner {
                    Owner::Project(id) => out.extend(project_crumb(id)),
                    Owner::Epic(id) => {
                        if let Some(at) = tree::locate_epic(&projects, id) {
                            out.extend(at.project.map(|(p, n)| crumb(n, Some(Route::Project(p)))));
                            out.extend(at.epic.map(|(e, n)| crumb(n, Some(Route::Epic(e)))));
                        }
                    }
                    Owner::Topic(id) => {
                        if let Some(at) = tree::locate_topic(&projects, id) {
                            out.extend(at.project.map(|(p, n)| crumb(n, Some(Route::Project(p)))));
                            out.extend(at.epic.map(|(e, n)| crumb(n, Some(Route::Epic(e)))));
                            out.extend(at.topic.map(|(t, n)| crumb(n, Some(Route::Topic(t)))));
                        }
                    }
                }
                out.extend(title.map(|t| crumb(t, None)));
                out
            }
            (Some(Route::Kb(_)), _) => Vec::new(),
            (Some(Route::Trash), _) => vec![crumb(text(locale, Key::TreeTrash), None)],
            (Some(Route::Settings), _) => vec![crumb(text(locale, Key::TreeSettings), None)],
            (Some(Route::Shortcuts), _) => vec![crumb(text(locale, Key::CmdShortcuts), None)],
        }
    }

    pub fn open_crumb(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(route, true, window, cx);
    }

    // ---- key loss ----

    fn key_loss(
        &mut self,
        db: PathBuf,
        unreadable: bool,
        kept: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
        let path = db.to_string_lossy().into_owned();
        // macOS keeps the key in a file in the data folder (ADR 0003), so
        // its copy names the file, not a keychain.
        let body = if unreadable {
            text(locale, Key::KeylossBodyUnreadable)
        } else if cfg!(target_os = "macos") {
            text(locale, Key::KeylossBodyM)
        } else {
            tr(
                locale,
                Key::KeylossBody,
                &[("keychain", Arg::Text(&keychain))],
            )
        };
        ui::page_scroll("key-loss")
            .child(
                div()
                    .w_full()
                    .max_w(px(ui::column_max()))
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
                            .child(body),
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
                                .on_click(
                                    cx.listener(|shell, _, window, cx| shell.import(window, cx)),
                                ),
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
                                |shell, _, window, cx| shell.start_empty(None, window, cx),
                            )),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// "Start empty", and the confirmed key-loss import: the unreadable
    /// file is renamed and kept, never deleted, a new database is created
    /// next to it, and `file` (if any) is imported into it.
    fn start_empty(
        &mut self,
        file: Option<std::sync::Arc<ExportFile>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(config) = db::config() else { return };
        let dir = config.data_dir.clone();
        let kept = match &self.content {
            Content::KeyLoss { kept, .. } => kept.clone(),
            _ => acta_core::db::unreadable_name(acta_core::time::now_ms()),
        };
        let opening = cx.background_spawn(async move {
            if let Err(err) = acta_core::db::set_aside_unreadable(&dir, &kept) {
                crate::log::line(&format!(
                    "key loss: could not set the file aside: {:?}",
                    err.kind()
                ));
                return (
                    db::Opened::Failed {
                        db: Some(dir.join(acta_core::store::DB_FILE)),
                    },
                    None,
                );
            }
            let mut opened = db::open(Some(config));
            let mut imported = None;
            if let (Opened::Ready(store), Some(file)) = (&mut opened, file) {
                let report = store.import(&file, false);
                let paths: Vec<String> = file.folders.iter().map(|f| f.path.clone()).collect();
                imported = Some((report.ok(), paths));
            }
            (opened, imported)
        });
        self._open = cx.spawn_in(window, async move |shell, cx| {
            let (opened, imported) = opening.await;
            let missing = match &imported {
                Some((_, paths)) => {
                    let paths = paths.clone();
                    cx.background_executor()
                        .spawn(async move { crate::workspace::missing_paths(paths).len() as u64 })
                        .await
                }
                None => 0,
            };
            let _ = shell.update_in(cx, |shell, window, cx| {
                match opened {
                    Opened::Ready(store) => {
                        shell.start_workbench(Db::new(store), window, cx);
                        let locale = settings::locale(cx);
                        if let Some((report, _)) = imported {
                            let toast = match report {
                                Some(report) => Toast {
                                    message: crate::dialogs::import_done(locale, &report, missing),
                                    kind: ToastKind::Done,
                                },
                                None => Toast {
                                    message: text(locale, Key::ErrorSave),
                                    kind: ToastKind::Error,
                                },
                            };
                            shell.show_toast(toast, cx);
                        }
                    }
                    Opened::KeyLoss { db, unreadable } => shell.content = key_loss(db, unreadable),
                    Opened::Failed { db } => shell.content = Content::OpenFailed { db },
                }
                cx.notify();
            });
        });
    }

    /// The screen when the database cannot be opened and the key is not the
    /// reason: what happened, nothing changed, Try again.
    fn open_failed(&mut self, db: Option<PathBuf>, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let body = if db.is_some() {
            text(locale, Key::OpenfailBody)
        } else {
            text(locale, Key::OpenfailNodir)
        };
        ui::page_scroll("open-failed")
            .child(
                div()
                    .w_full()
                    .max_w(px(ui::column_max()))
                    .pt(px(96.))
                    .flex()
                    .flex_col()
                    .child(
                        type_scale::TITLE
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::OpenfailTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(12.))
                            .text_color(t.text_secondary)
                            .child(body),
                    )
                    .children(db.map(|db| {
                        type_scale::CODE
                            .el()
                            .mt(px(12.))
                            .text_color(t.text_primary)
                            .child(db.to_string_lossy().into_owned())
                    }))
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(
                                ui::button(
                                    "openfail-retry",
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
                            )
                            .children(crate::log::path().map(|log| {
                                ui::button(
                                    "openfail-log",
                                    text(locale, Key::OpenfailShowLog),
                                    Variant::Ghost,
                                    false,
                                    &t,
                                )
                                .on_click(move |_, _, cx| cx.reveal_path(log))
                            })),
                    ),
            )
            .into_any_element()
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
                .rounded(px(RADIUS_LG))
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

    /// The workbench between the titlebar and the status bar: sidebar,
    /// page, docks and their resize handles, sized so that panes give way
    /// sidebar first (layout.md 4.2, narrow behaviour).
    fn workbench_body(
        &mut self,
        outlined: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let viewport = window.viewport_size();
        let frame = if outlined { 2. } else { 0. };
        let width = f32::from(viewport.width) - frame;
        let body_height = f32::from(viewport.height) - frame - titlebar::HEIGHT - STATUS_BAR_HEIGHT;
        let first_run = matches!(self.bench().map(|b| &b.page), Some(PageView::FirstRun(_)));
        let bulk = self.bulk_notice(cx);
        let toast = self.toast_element(cx);
        let tab_bar = self.tab_bar(cx);
        let (right_chat, bottom) = self.chat_docks(window, cx);
        let terminal = self.terminal_visible();
        let right_wanted = if terminal {
            Some(self.sizes.terminal)
        } else if right_chat.is_some() {
            Some(self.sizes.chat)
        } else {
            None
        };
        let dock_width = right_wanted.map(|w| panes::dock_width(w, width));
        let sidebar_width = self.sizes.sidebar;
        let fits = first_run || panes::sidebar_fits(sidebar_width, dock_width.unwrap_or(0.), width);
        if fits != self.sidebar_fits {
            self.sidebar_fits = fits;
            if fits {
                self.sidebar_overlay = false;
            }
        }
        let inline_sidebar = (self.sidebar_visible && fits && !first_run)
            .then(|| self.sidebar(sidebar_width, window, cx));
        let overlay_sidebar = (self.sidebar_overlay && !fits && !first_run).then(|| {
            let el = self.sidebar(panes::SIDEBAR_DEFAULT, window, cx);
            deferred(
                div()
                    .id("sidebar-overlay")
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .border_r_1()
                    .border_color(t.border_default)
                    .shadow(vec![t.shadow_overlay()])
                    .on_mouse_down_out(
                        cx.listener(|shell, _, window, cx| shell.dismiss_sidebar(window, cx)),
                    )
                    .child(el),
            )
            .with_priority(1)
            .into_any_element()
        });
        let resizing = self.resizing;
        let bench = self.bench().expect("workbench");
        let page_view = bench.page.view();
        let page_focus = bench.page_focus.clone();
        let right: Option<AnyElement> = if bench.dock_visible {
            Some(bench.dock.clone().into_any_element())
        } else {
            right_chat
        };
        let page = div()
            .id("page-area")
            .track_focus(&page_focus)
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .children(tab_bar)
            .children(bulk)
            .child(div().flex_1().min_h_0().child(page_view))
            .children(toast);
        // The bottom dock sits under the page area only.
        let bottom = bottom.map(|dock| {
            let height = panes::bottom_height(self.sizes.bottom, body_height);
            div()
                .flex()
                .flex_col()
                .w_full()
                .flex_shrink_0()
                .child(resize_handle(Handle::Bottom, resizing, &t, cx))
                .child(div().h(px(height)).w_full().child(dock))
        });
        let column = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(panes::PAGE_MIN.min(width)))
            .h_full()
            .child(page)
            .children(bottom);
        let right = right.map(|dock| {
            let width = dock_width.unwrap_or(panes::DOCK_DEFAULT);
            div()
                .flex()
                .flex_row()
                .h_full()
                .flex_shrink_0()
                .child(resize_handle(Handle::Right, resizing, &t, cx))
                .child(div().w(px(width)).h_full().child(dock))
        });
        let sidebar = inline_sidebar.map(|el| {
            div()
                .flex()
                .flex_row()
                .h_full()
                .flex_shrink_0()
                .child(el)
                .child(resize_handle(Handle::Sidebar, resizing, &t, cx))
        });
        div()
            .relative()
            .flex()
            .flex_row()
            .flex_1()
            .min_h_0()
            .w_full()
            .children(sidebar)
            .child(column)
            .children(right)
            .children(overlay_sidebar)
            .into_any_element()
    }
    fn bulk_notice(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let bench = self.bench()?;
        let offer = bench.ws.read(cx).bulk.clone()?;
        let n = offer.changes.len() as u64;
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
                                let project = bench.active_project;
                                bench.ws.update(cx, |ws, cx| ws.apply_bulk(project, cx));
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

/// The key-loss screen for `db`, with the name the file would be kept
/// under fixed now (screens.md 8.9).
fn key_loss(db: PathBuf, unreadable: bool) -> Content {
    Content::KeyLoss {
        db,
        unreadable,
        kept: acta_core::db::unreadable_name(acta_core::time::now_ms()),
        dialog: None,
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

/// The project of a tree row.
fn row_project(projects: &[acta_core::model::ProjectTree], row: &Row) -> Option<ProjectId> {
    match row {
        Row::Project { id, .. } => Some(*id),
        Row::Epic { project, .. } => Some(*project),
        Row::Topic { id, .. } => {
            tree::locate_topic(projects, *id).and_then(|l| l.project.map(|p| p.0))
        }
        Row::Inbox { project, .. } | Row::Knowledge { project, .. } => Some(*project),
        Row::Closed { .. } => None,
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
        .when(cursor, |this| this.shadow(ui::ring_shadow(t, false)));
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
    // components.md 5, list / tree row: hover on a project or epic row
    // reveals a compact ghost `plus`, "New topic" there.
    let plus = |project: ProjectId, epic: Option<EpicId>, cx: &mut Context<Shell>| {
        div()
            .invisible()
            .group_hover(id.clone(), |s| s.visible())
            .child(
                ui::ghost_icon_button(
                    format!("tree-plus-{index}"),
                    Icon::Plus,
                    24.,
                    false,
                    false,
                    true,
                    t,
                )
                .tooltip(ui::tip(text(locale, Key::TopicNew)))
                .on_click(cx.listener(move |shell, _, window, cx| {
                    cx.stop_propagation();
                    if let Some(bench) = shell.bench() {
                        bench.create_in = Some((project, epic));
                    }
                    shell.open_palette(None, Some(CreateKind::Topic), window, cx);
                })),
            )
    };
    el = match row {
        Row::Project {
            title: name,
            expanded,
            id: pid,
        } => el
            .child(chevron(*expanded, pid.to_string(), cx))
            .child(title(type_scale::UI_STRONG, name.clone(), t.text_primary))
            .child(plus(*pid, None, cx)),
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
            project,
        } => el
            .child(chevron(*expanded, eid.to_string(), cx))
            .child(title(type_scale::LABEL, name.clone(), rest))
            .child(plus(*project, Some(*eid), cx)),
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
        // Mod+click or a middle click opens the row in a new tab (4.4).
        el = el
            .on_click(
                cx.listener(move |shell, event: &gpui_kit::ClickEvent, window, cx| {
                    if let Some(bench) = shell.bench() {
                        bench.cursor = index;
                    }
                    if event.modifiers().secondary(cx) {
                        shell.open_in_tab(route, window, cx)
                    } else {
                        shell.navigate(route, true, window, cx)
                    }
                }),
            )
            .on_mouse_down(
                gpui_kit::MouseButton::Middle,
                cx.listener(move |shell, _, window, cx| shell.open_in_tab(route, window, cx)),
            );
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
            Content::KeyLoss {
                db,
                unreadable,
                kept,
                dialog,
            } => {
                let (db, unreadable, kept, dialog) =
                    (db.clone(), *unreadable, kept.clone(), dialog.clone());
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(self.key_loss(db, unreadable, kept, cx))
                    .children(dialog.map(|d| deferred(d).into_any_element()))
                    .into_any_element()
            }
            Content::OpenFailed { db } => {
                let db = db.clone();
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.open_failed(db, cx))
                    .into_any_element()
            }
            Content::Specimen(view) => div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(view.clone())
                .into_any_element(),
            Content::Workbench(_) => self.workbench_body(outlined, window, cx),
        };
        // A drag that ended: its size goes to the preferences.
        if self.resizing.is_some() && !cx.has_active_drag() {
            self.save_sizes(cx);
        }
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
            .on_action(cx.listener(|shell, _: &OpenShortcuts, window, cx| {
                shell.navigate(Route::Shortcuts, true, window, cx)
            }))
            .on_action(
                cx.listener(|s, _: &CloseTab, w, cx| s.run_command(Command::CloseTab, w, cx)),
            )
            .on_action(cx.listener(|s, _: &RemoveClaudeSetup, w, cx| {
                s.open_dialog(DialogKind::RemoveSetup, w, cx)
            }))
            .on_action(cx.listener(|s, _: &NextTab, w, cx| s.cycle_tab(true, w, cx)))
            .on_action(cx.listener(|s, _: &PreviousTab, w, cx| s.cycle_tab(false, w, cx)))
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_action(cx.listener(|s, _: &OpenPalette, w, cx| s.open_palette(None, None, w, cx)))
            .on_action(cx.listener(|s, _: &GoToTopic, w, cx| {
                s.open_palette(Some(Group::Topics), None, w, cx)
            }))
            .on_action(cx.listener(|s, _: &SwitchProject, w, cx| {
                // First run (8.5): Mod+O is "Choose a folder".
                if s.bare_titlebar() && matches!(s.content, Content::Workbench(_)) {
                    s.new_project(w, cx);
                } else {
                    s.open_palette(Some(Group::Projects), None, w, cx)
                }
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
            .on_action(cx.listener(|s, _: &ExportData, w, cx| s.open_export(w, cx)))
            .on_action(cx.listener(|s, _: &ImportData, w, cx| s.import(w, cx)))
            .on_action(cx.listener(|s, _: &RestoreSnapshot, w, cx| s.open_snapshots(w, cx)))
            .on_action(cx.listener(|s, _: &OpenTrash, w, cx| s.navigate(Route::Trash, true, w, cx)))
            .on_action(cx.listener(|s, _: &ToggleTerminal, w, cx| s.toggle_terminal(w, cx)))
            .on_action(cx.listener(|s, _: &ToggleChat, w, cx| s.toggle_chat(w, cx)))
            .on_action(cx.listener(|s, _: &FocusChat, w, cx| s.focus_chat(w, cx)))
            .on_action(|_: &OpenChatInBrowser, _, cx| cx.open_url(CLAUDE_HOME))
            .on_action(cx.listener(|s, _: &ChatSignOut, w, cx| s.ask_chat_sign_out(w, cx)))
            .on_action(cx.listener(|s, _: &FocusSidebar, w, cx| {
                if let Some(bench) = s.bench() {
                    let handle = bench.tree_focus.clone();
                    if s.sidebar_fits {
                        s.sidebar_visible = true;
                    } else {
                        s.sidebar_overlay = true;
                    }
                    w.focus(&handle, cx);
                    cx.notify();
                }
            }))
            .on_drag_move(cx.listener(|s, e: &DragMoveEvent<ResizeDrag>, w, cx| {
                let handle = e.drag(cx).0;
                s.resize(handle, e.event.position, w, cx)
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

/// A resize handle (docs/design/components.md 5): the 1px `border.subtle`
/// divider between two panes, with an 8 hit area that lies over the pane
/// painted before it, so the pane after it never covers the hit area.
/// Hovered or dragged, the line becomes 2px `accent`; double-click restores
/// the default size.
fn resize_handle(
    handle: Handle,
    resizing: Option<Handle>,
    t: &Tokens,
    cx: &mut Context<Shell>,
) -> gpui_kit::Div {
    let vertical = handle != Handle::Bottom;
    let id: &'static str = match handle {
        Handle::Sidebar => "resize-sidebar",
        Handle::Right => "resize-right",
        Handle::Bottom => "resize-bottom",
    };
    let accent = t.accent;
    let reach = panes::HANDLE_HIT - 1.;
    let line = div()
        .absolute()
        .map(|this| {
            if vertical {
                this.top_0().bottom_0().right_0().w(px(2.))
            } else {
                this.left_0().right_0().bottom_0().h(px(2.))
            }
        })
        .when(resizing == Some(handle), |this| this.bg(accent))
        .group_hover(id, move |s| s.bg(accent));
    let hit = div()
        .id(id)
        .group(id)
        .absolute()
        .map(|this| {
            if vertical {
                this.top_0()
                    .bottom_0()
                    .left(px(-reach))
                    .w(px(panes::HANDLE_HIT))
                    .cursor(gpui_kit::CursorStyle::ResizeLeftRight)
            } else {
                this.left_0()
                    .right_0()
                    .top(px(-reach))
                    .h(px(panes::HANDLE_HIT))
                    .cursor(gpui_kit::CursorStyle::ResizeUpDown)
            }
        })
        .on_drag(ResizeDrag(handle), |_, _, _, cx| {
            cx.new(|_| gpui_kit::EmptyView)
        })
        .on_click(
            cx.listener(move |shell, event: &gpui_kit::ClickEvent, _, cx| {
                if event.click_count() >= 2 {
                    shell.reset_size(handle, cx);
                }
            }),
        )
        .child(line);
    div()
        .relative()
        .flex_shrink_0()
        .map(|this| {
            if vertical {
                this.w(px(1.)).h_full()
            } else {
                this.h(px(1.)).w_full()
            }
        })
        .bg(t.border_subtle)
        .child(hit)
}

/// A chat in the right dock gives way to the terminal shown there.
#[cfg(windows)]
fn hide_right_chat(bench: &mut Bench, cx: &mut Context<Shell>) {
    let chat = bench.chat.clone();
    if chat.read(cx).dock == Dock::Right {
        chat.update(cx, |chat, cx| chat.hide(cx));
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
