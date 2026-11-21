//! The workbench window (docs/design/layout.md 4.2, workbench.md 4.8):
//! titlebar, the four areas (left sidebar, page area, bottom panel, right
//! sidebar) with the views and page tabs in their groups, the status bar,
//! and the overlays (palette, quick capture, dialogs, toast). The layout
//! itself is `layout.rs`; drawing and moving it is `shell_layout.rs`.
//!
//! The store opens in the background at launch. A database whose key is
//! lost shows the key-loss recovery screen (screens.md 8.9) and is never
//! touched; with no project yet the first-run page asks for a folder
//! (8.5).
//!
//! Page tabs: the focused group's page is "checked out" (`history`, `page`
//! in [`Bench`]); every other tab waits in `slots` with its history, and
//! with its view while some group shows it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use acta_core::model::{Author, EpicId, KbId, KbKind, Owner, ProjectId, Record, Status, TopicId};
use acta_core::store::ExportFile;
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::component::window_border;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext, Bounds, Context, DragMoveEvent, Entity, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, KeyDownEvent,
    ParentElement, PathPromptOptions, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Task, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions,
    actions, deferred, div, point, px, size,
};

#[cfg(windows)]
use crate::browser_panel::{BrowserEvent, BrowserPanel};
use crate::capture::{Capture, CaptureEvent};
#[cfg(windows)]
use crate::chat::Kept;
#[cfg(windows)]
use crate::chat_panel::{ChatEvent, ChatPanel};
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
use crate::layout::{Direction, Layout, Place, Side, Tab, TabId, ViewAction, ViewId};
use crate::palette::{CreateKind, Palette, PaletteEvent, Setup};
use crate::palette_model::{Group, Target};
use crate::record_page::{EditTitle, OpenStatusMenu, PageEvent, RecordPage};
use crate::route::{History, Route};
use crate::settings;
use crate::settings_page::SettingsPage;
use crate::terminal_dock::{DockEvent, Origin, TerminalDock};
use crate::theme::{self, RADIUS_LG, RADIUS_SM, Tokens};
use crate::titlebar::{self, Crumb};
use crate::trash_page::TrashPage;
use crate::tree::{self, Folds, MenuItem, MenuKind, Row, RowKey};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{Toast, ToastKind, Workspace, WorkspaceEvent};

#[path = "shell_layout.rs"]
mod layout_ui;

use layout_ui::{CHAT, LayoutUi, ResizeDrag};
pub(crate) use layout_ui::{
    LayoutMenu, area_keys, busy_dot, view_icon, view_keys, view_name, views,
};

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
/// Deferred layers paint by priority: the sidebar over the page 1, the
/// layout toast 2, menus 3, a drag 4, the palette and dialogs above all.
const OVERLAY_PRIORITY: usize = 10;
/// Type-ahead in the tree: keys typed within this of each other extend
/// the same search.
const TYPE_AHEAD_PAUSE: Duration = Duration::from_secs(1);

actions!(
    acta,
    [
        /// `Mod+B`, palette "Toggle left sidebar".
        ToggleSidebar,
        /// `Mod+Shift+B`, palette "Toggle right sidebar".
        ToggleRight,
        /// `Mod+J`, palette "Toggle bottom panel".
        ToggleBottom,
        /// `Mod+\\`, palette "Split right".
        SplitRight,
        /// `F6` / `Shift+F6`: the next or previous area (6.2).
        NextArea,
        PreviousArea,
        /// `Mod+Shift+Enter`: maximize the focused area and back.
        ZoomArea,
        /// `Esc` during a drag: cancel it (4.8.7).
        CancelDrag,
        /// Settings: "Reset layout" (4.8.13).
        ResetLayout,
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
        /// `Mod+Shift+Z`, also `Ctrl+Y` on Windows and Linux.
        Redo,
        /// `Mod+Shift+O`: the open document's headings.
        GoToHeading,
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

/// The command "File with Claude Code" runs (the plugin's triage skill).
const TRIAGE_COMMAND: &str = "/acta:triage";

/// The active project as the status bar reports it (layout.md 4.6).
#[derive(Clone, Debug)]
struct ProjectFacts {
    project: ProjectId,
    has_folder: bool,
    /// The topic Claude Code wrote to last, and when.
    last: Option<(String, i64)>,
}

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
        // interaction.md 6.1: the terminal and the chat keep Mod+Shift+B;
        // Ctrl+J is Claude Code's newline and Ctrl+\\ the shell's quit.
        KeyBinding::new(m("cmd-shift-b", "ctrl-shift-b"), ToggleRight, None),
        KeyBinding::new(m("cmd-j", "ctrl-j"), ToggleBottom, app),
        KeyBinding::new(m("cmd-\\", "ctrl-\\"), SplitRight, app),
        KeyBinding::new("f6", NextArea, None),
        KeyBinding::new("shift-f6", PreviousArea, None),
        KeyBinding::new(m("cmd-shift-enter", "ctrl-shift-enter"), ZoomArea, None),
        KeyBinding::new("escape", CancelDrag, Some("Dragging")),
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
        KeyBinding::new(m("cmd-shift-z", "ctrl-shift-z"), Redo, plain),
        KeyBinding::new(m("cmd-shift-o", "ctrl-shift-o"), GoToHeading, app),
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
        KeyBinding::new("down", TreeDown, Some("Tree")),
        KeyBinding::new("left", TreeLeft, Some("Tree")),
        KeyBinding::new("right", TreeRight, Some("Tree")),
        KeyBinding::new("enter", TreeOpen, Some("Tree")),
        KeyBinding::new("delete", TreeDelete, Some("Tree")),
        KeyBinding::new("alt-up", TreeMoveUp, Some("Tree")),
        KeyBinding::new("alt-down", TreeMoveDown, Some("Tree")),
        KeyBinding::new("home", TreeHome, Some("Tree")),
        KeyBinding::new("end", TreeEnd, Some("Tree")),
        KeyBinding::new("escape", DismissSidebar, Some("Tree")),
        // 4.8.10: `Esc` dismisses the sidebar shown over the page wherever
        // the keyboard is, except where `Esc` means something (a terminal,
        // a field, a dialog, a menu: deeper contexts, or key handlers that
        // run when the sidebar is not shown).
        KeyBinding::new("escape", DismissSidebar, Some("Workbench && !Terminal")),
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
        out.push(KeyBinding::new("ctrl-y", Redo, plain));
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

/// A menu of a tree row, with its highlighted item.
#[derive(Clone, Copy, Debug)]
struct TreeMenu {
    row: RowKey,
    kind: MenuKind,
    highlight: usize,
}

/// A tree row being renamed in place.
#[derive(Clone)]
struct Rename {
    key: RowKey,
    state: Entity<InputState>,
    /// The new title is being written.
    saving: bool,
}

enum Overlay {
    Palette(Entity<Palette>),
    Capture(Entity<Capture>),
    Dialog(Entity<Dialog>),
}

/// A page tab that is not the checked-out one: its history, and its view
/// while a group shows it.
struct PageSlot {
    history: History,
    view: Option<PageView>,
    sub: Option<Subscription>,
}

struct Bench {
    ws: Entity<Workspace>,
    /// The layout of the window (workbench.md 4.8) and its undo steps.
    layout: Layout,
    layout_undo: crate::layout::UndoLog,
    ui: LayoutUi,
    /// The checked-out page tab: the focused group's page (or the last
    /// page it showed, under a view tab), with its history and view.
    current: TabId,
    history: History,
    page: PageView,
    /// Every other page tab.
    slots: HashMap<TabId, PageSlot>,
    next_tab: u64,
    /// Closed tabs, most recent last ("Reopen closed tab").
    closed_tabs: Vec<History>,
    /// Page tabs a layout change closed (an Inbox page that became the
    /// view), for the undo of that change.
    retired: HashMap<TabId, History>,
    /// The titles of the documents and memories open in tabs, read off the
    /// UI thread, for the tabs that are not active.
    kb_titles: std::collections::HashMap<KbId, String>,
    _kb_titles: Task<()>,
    settings: Entity<SettingsPage>,
    /// The page shown where a tab has nothing yet.
    first_run: Entity<FirstRun>,
    folds: Folds,
    cursor: tree::Cursor,
    /// The tree's open menu (`Shift+F10`, `S`, `M`).
    tree_menu: Option<TreeMenu>,
    /// The row being renamed in place.
    rename: Option<Rename>,
    /// Type-ahead: what was typed, and when the last key came.
    find: (String, Option<Instant>),
    tree_focus: FocusHandle,
    /// One focus handle per page group (at most three, 4.8.8).
    group_focus: [FocusHandle; 3],
    /// The containers of the Inbox and the Knowledge views.
    inbox_focus: FocusHandle,
    knowledge_focus: FocusHandle,
    inbox_view: Option<Entity<InboxPage>>,
    knowledge_view: Option<Entity<KnowledgePage>>,
    view_subs: HashMap<ViewId, Subscription>,
    active_project: Option<ProjectId>,
    /// Where the next "New topic" goes when it was asked for from a tree
    /// row's `plus` (that project, and that epic if any).
    create_in: Option<(ProjectId, Option<EpicId>)>,
    /// What the open picker does with the row chosen.
    on_pick: Option<crate::palette::OnPick>,
    /// "File with Claude Code" found Claude Code mid-task: the restart
    /// confirmation, when accepted, starts a session on the triage.
    pending_triage: Option<Origin>,
    recent: Vec<TopicId>,
    overlay: Option<Overlay>,
    /// Where focus returns when the overlay closes.
    return_focus: Option<FocusHandle>,
    capture_draft: String,
    toast: Option<Toast>,
    toast_timer: Task<()>,
    dock: Entity<TerminalDock>,
    #[cfg(windows)]
    chat: Entity<ChatPanel>,
    #[cfg(windows)]
    browser: Entity<BrowserPanel>,
    _subs: Vec<Subscription>,
    _page_sub: Option<Subscription>,
}

impl Bench {
    fn new_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab);
        self.next_tab += 1;
        id
    }

    /// The tab showing `route`, if any.
    fn find_tab(&self, route: Route) -> Option<TabId> {
        if self.history.current() == Some(route) {
            return Some(self.current);
        }
        self.slots
            .iter()
            .find(|(_, s)| s.history.current() == Some(route))
            .map(|(id, _)| *id)
    }
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
        // layout.md 4.6: the setup is checked again when the window
        // regains focus. A drag ends with the window's focus (4.8.7).
        let activation = cx.observe_window_activation(window, |shell, window, cx| {
            if !window.is_window_active() {
                shell.cancel_drag(window, cx);
            }
            if window.is_window_active() && matches!(shell.content, Content::Workbench(_)) {
                claude_setup::refresh_if_stale(cx);
            }
        });
        Self {
            content,
            start,
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
        #[cfg(windows)]
        let browser = {
            let db = db.clone();
            cx.new(|cx| BrowserPanel::new(db, window, cx))
        };
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
                    DockEvent::RestartHere => shell.restart_terminal_here(window, cx),
                },
            ),
        ];
        #[cfg(windows)]
        subs.push(cx.subscribe_in(&chat, window, Self::on_chat));
        #[cfg(windows)]
        subs.push(cx.subscribe_in(&browser, window, Self::on_browser));
        let first_run = cx.new(|_| FirstRun);
        let first_run_sub = Self::watch_first_run(&first_run, window, cx);
        let bench = Bench {
            ws,
            // The real layout is read with the first tree (`after_reload`).
            layout: Layout::defaults(vec![TabId(0)], CHAT),
            layout_undo: crate::layout::UndoLog::default(),
            ui: LayoutUi::new(),
            current: TabId(0),
            history: History::default(),
            page: PageView::FirstRun(first_run.clone()),
            slots: HashMap::new(),
            next_tab: 1,
            closed_tabs: Vec::new(),
            retired: HashMap::new(),
            kb_titles: Default::default(),
            _kb_titles: Task::ready(()),
            settings: settings_page,
            first_run,
            folds,
            cursor: tree::Cursor::default(),
            tree_menu: None,
            rename: None,
            find: (String::new(), None),
            tree_focus: cx.focus_handle(),
            group_focus: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            inbox_focus: cx.focus_handle(),
            knowledge_focus: cx.focus_handle(),
            inbox_view: None,
            knowledge_view: None,
            view_subs: HashMap::new(),
            active_project: None,
            create_in: None,
            on_pick: None,
            pending_triage: None,
            recent: Vec::new(),
            overlay: None,
            return_focus: None,
            capture_draft: String::new(),
            toast: None,
            toast_timer: Task::ready(()),
            dock,
            #[cfg(windows)]
            chat,
            #[cfg(windows)]
            browser,
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
        let started = bench.history.current().is_some() || !bench.slots.is_empty();
        if !started {
            self.restore_layout(has_projects, window, cx);
            let Some(bench) = self.bench() else { return };
            let route = if start.page == Page::Settings {
                Some(Route::Settings)
            } else if has_projects {
                bench
                    .history
                    .current()
                    .or(first_project.map(Route::Project))
            } else {
                None
            };
            match route {
                Some(route) => self.navigate(route, false, window, cx),
                None => self.show_first_run(window, cx),
            }
            self.after_layout(window, cx);
            self.dev_overlay(window, cx);
            return;
        }
        // Titles may have changed: read them again.
        bench.kb_titles.clear();
        if !has_projects && !matches!(bench.page, PageView::Settings(_) | PageView::Trash(_)) {
            self.show_first_run(window, cx);
        } else if has_projects
            && on_first_run
            && let Some(id) = first_project
        {
            self.navigate(Route::Inbox(id), true, window, cx);
        }
        self.refresh_kb_titles(cx);
        self.mark_shown_seen(cx);
    }

    /// The layout of `preferences.json` (4.8.12): every page tab whose key
    /// still parses gets a slot, the layout is repaired against them, and
    /// the focused group's page is checked out. With no project, or no tab,
    /// one empty tab.
    fn restore_layout(&mut self, has_projects: bool, window: &mut Window, cx: &mut Context<Self>) {
        let saved = settings::current(cx).layout.clone();
        let start_sidebar = self.start.sidebar;
        let Some(bench) = self.bench() else { return };
        let (mut layout, pages) = match saved {
            Some(saved) => (saved.layout, saved.pages),
            None => (Layout::defaults(Vec::new(), CHAT), Vec::new()),
        };
        bench.slots.clear();
        if has_projects {
            for (id, key) in pages {
                if let Some(route) = Route::parse(&key) {
                    bench.slots.insert(
                        id,
                        PageSlot {
                            history: History::at(route),
                            view: None,
                            sub: None,
                        },
                    );
                    bench.next_tab = bench.next_tab.max(id.0 + 1);
                }
            }
        }
        if bench.slots.is_empty() {
            let id = bench.new_tab_id();
            bench.slots.insert(
                id,
                PageSlot {
                    history: History::default(),
                    view: None,
                    sub: None,
                },
            );
        }
        let live: Vec<TabId> = bench.slots.keys().copied().collect();
        layout.repair(&live, CHAT);
        if start_sidebar {
            layout.left.shown = true;
        }
        let first = layout.focused_page().unwrap_or(live[0]);
        bench.layout = layout;
        // The placeholder tab of `start_workbench` is gone; `first` is
        // checked out without storing it.
        let slot = bench.slots.remove(&first).expect("a live tab");
        bench.current = first;
        bench.history = slot.history;
        let _ = (window, cx);
    }

    /// Debug builds: `ACTA_DEV_OVERLAY=palette|capture|terminal|chat|chat-palette|browser|browser-palette|export|snapshots`
    /// opens that surface at launch, so captures need no input.
    fn dev_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !cfg!(debug_assertions) {
            return;
        }
        match std::env::var("ACTA_DEV_OVERLAY").as_deref() {
            Ok("palette") => self.open_palette(None, None, window, cx),
            Ok("capture") => self.open_capture(window, cx),
            Ok("terminal") => self.view_action(ViewId::Terminal, ViewAction::Show, window, cx),
            // Shown without taking the keyboard, for captures.
            #[cfg(windows)]
            Ok("chat") => {
                self.change_layout(None, window, cx, |l| l.show_view(ViewId::Chat).is_some());
            }
            // The chat under the palette: the placeholder instead of the page.
            #[cfg(windows)]
            Ok("chat-palette") => {
                self.change_layout(None, window, cx, |l| l.show_view(ViewId::Chat).is_some());
                self.open_palette(None, None, window, cx);
            }
            // The agent browser shown without the keyboard, for captures;
            // `-palette` under the palette (parked, with its line).
            #[cfg(windows)]
            Ok("browser") => {
                self.change_layout(None, window, cx, |l| {
                    l.show_view(ViewId::AgentBrowser).is_some()
                });
            }
            #[cfg(windows)]
            Ok("browser-palette") => {
                self.change_layout(None, window, cx, |l| {
                    l.show_view(ViewId::AgentBrowser).is_some()
                });
                self.open_palette(None, None, window, cx);
            }
            Ok("layout-menu") => self.open_menu(LayoutMenu::Layout, 1000., 40., cx),
            // "File with Claude Code" against a live Claude Code: the
            // terminal starts, then after 30 s the triage runs (typed into
            // an idle session), then after 60 s more `/exit`, if idle. The
            // terminal's screen goes to stderr at each step.
            #[cfg(debug_assertions)]
            Ok("triage-check") => self.dev_triage_check(window, cx),
            Ok("split") => {
                self.change_layout(None, window, cx, |l| {
                    l.show_view(ViewId::Terminal);
                    l.move_tab(
                        Tab::View(ViewId::Terminal),
                        crate::layout::Target::Area(Side::Bottom),
                    )
                    .is_ok()
                });
            }
            // 8.19: a second page split to the right, Inbox as a view.
            Ok("groups") => {
                let project = self.bench().and_then(|b| b.active_project);
                if let Some(project) = project {
                    self.open_in_tab(Route::Settings, window, cx);
                    self.split_page(Direction::Row, window, cx);
                    self.open_in_tab(Route::Inbox(project), window, cx);
                    if let Some(tab) = self.bench().map(|b| b.current) {
                        self.change_layout(None, window, cx, |l| {
                            l.page_to_view(
                                tab,
                                ViewId::Inbox,
                                crate::layout::Target::Area(Side::Left),
                            )
                            .is_ok()
                        });
                    }
                }
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

    /// `Mod+B`: show or hide the left sidebar; while it is auto-hidden in a
    /// narrow window it shows over the page instead (4.8.10).
    pub fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.content, Content::Workbench(_)) {
            self.toggle_area(Side::Left, window, cx);
        }
    }

    /// Dismiss the sidebar shown over the page and return to the page.
    fn dismiss_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if bench.ui.left_overlay {
            bench.ui.left_overlay = false;
            self.focus_page(window, cx);
            cx.notify();
        }
    }

    /// Whether the left sidebar is on screen, beside the page or over it.
    pub fn sidebar_visible(&self, window: &Window) -> bool {
        let Content::Workbench(bench) = &self.content else {
            return false;
        };
        let (w, h) = layout_ui::body_size(window);
        let f = crate::layout_frame::frame(&bench.layout, w, h);
        f.left.is_some() || (f.left_auto_hidden && bench.ui.left_overlay)
    }

    /// Whether a side area is shown (the titlebar's area toggles).
    pub fn area_shown(&self, side: Side, window: &Window) -> bool {
        let Content::Workbench(bench) = &self.content else {
            return false;
        };
        match side {
            Side::Left => self.sidebar_visible(window),
            _ => bench.layout.side(side).shown,
        }
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

    /// Whether a view's titlebar toggle is selected (4.8.5): visible in the
    /// layout; for Inbox and Knowledge while they are pages, the focused
    /// group's active tab shows that page.
    pub fn view_selected(&self, view: ViewId, window: &Window) -> bool {
        let Content::Workbench(bench) = &self.content else {
            return false;
        };
        if view.is_page_kind()
            && !bench.layout.is_placed(view)
            && !bench.layout.last.contains_key(&view)
        {
            let showing =
                matches!(bench.layout.focused_tab(), Some(Tab::Page(id)) if id == bench.current);
            return showing
                && matches!(
                    (view, bench.history.current()),
                    (ViewId::Inbox, Some(Route::Inbox(_)))
                        | (ViewId::Knowledge, Some(Route::Knowledge(_)))
                );
        }
        // Selected only while drawn: a view in a folded group, an
        // auto-hidden sidebar or under a maximized group is not (4.8.5).
        self.view_drawn(view, window)
    }

    /// The titlebar is a workbench's (toggles shown).
    pub fn is_workbench(&self) -> bool {
        matches!(self.content, Content::Workbench(_))
    }

    pub fn can_go_back(&self) -> bool {
        matches!(&self.content, Content::Workbench(b) if b.history.can_go_back())
    }

    pub fn can_go_forward(&self) -> bool {
        matches!(&self.content, Content::Workbench(b) if b.history.can_go_forward())
    }

    // ---- navigation ----

    /// Open `route` in the focused group's tab (`push` records a history
    /// step). A page open in another tab is focused there instead: a page
    /// is open once (4.4).
    pub fn navigate(
        &mut self,
        route: Route,
        push: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        if let Some(other) = bench.find_tab(route)
            && other != bench.current
            && let Some(spot) = bench.layout.locate(Tab::Page(other))
        {
            bench.layout.activate(spot);
            self.after_layout(window, cx);
            self.focus_page(window, cx);
            return;
        }
        let Some(bench) = self.bench() else { return };
        // Navigation goes to the focused group's page tab (4.8.8).
        if let Some(spot) = bench.layout.locate(Tab::Page(bench.current))
            && bench.layout.focused_tab() != Some(Tab::Page(bench.current))
        {
            bench.layout.activate(spot);
        }
        if push {
            bench.history.open(route);
        } else {
            bench.history.replace(route);
        }
        self.show(route, window, cx);
        self.mark_shown_seen(cx);
    }

    /// The pages on screen (the active tab of every page group) lose their
    /// unread dot and get their read marker (data.md request 1).
    pub(super) fn mark_shown_seen(&mut self, cx: &mut Context<Self>) {
        let Content::Workbench(bench) = &self.content else {
            return;
        };
        let shown: Vec<Record> = bench
            .layout
            .page
            .groups
            .iter()
            .filter_map(|g| match g.active_tab()? {
                Tab::Page(id) if id == bench.current => bench.history.current(),
                Tab::Page(id) => bench.slots.get(&id)?.history.current(),
                Tab::View(_) => None,
            })
            .filter_map(|route| match route {
                Route::Topic(id) => Some(Record::Topic(id)),
                Route::Kb(id) => Some(Record::Kb(id)),
                _ => None,
            })
            .collect();
        let ws = bench.ws.clone();
        ws.update(cx, |ws, cx| {
            for record in shown {
                ws.mark_seen(record, cx);
            }
        });
    }

    /// The view of a page, its events tied to the tab that shows it.
    fn make_view(
        &mut self,
        route: Route,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (PageView, Option<Subscription>) {
        let Some(bench) = self.bench() else {
            return (PageView::FirstRun(cx.new(|_| FirstRun)), None);
        };
        let ws = bench.ws.clone();
        let settings_page = bench.settings.clone();
        match route {
            Route::Project(id) => record(ws, Owner::Project(id), tab, window, cx),
            Route::Epic(id) => record(ws, Owner::Epic(id), tab, window, cx),
            Route::Topic(id) => record(ws, Owner::Topic(id), tab, window, cx),
            Route::Inbox(id) => {
                let page = cx.new(|cx| InboxPage::new(ws, id, window, cx));
                let sub = Self::page_events(&page, tab, window, cx);
                (PageView::Inbox(page), Some(sub))
            }
            Route::Knowledge(id) => {
                let page = cx.new(|cx| KnowledgePage::new(ws, id, Filters::default(), window, cx));
                let sub = Self::page_events(&page, tab, window, cx);
                (PageView::Knowledge(page), Some(sub))
            }
            Route::Kb(id) => {
                let page = cx.new(|cx| DocPage::open(ws, id, cx));
                let sub = Self::page_events(&page, tab, window, cx);
                (PageView::Doc(page), Some(sub))
            }
            Route::Trash => {
                let page = cx.new(|cx| TrashPage::new(ws, cx));
                let sub = Self::page_events(&page, tab, window, cx);
                (PageView::Trash(page), Some(sub))
            }
            Route::Settings => (PageView::Settings(settings_page), None),
            Route::Shortcuts => {
                let page = cx.new(|cx| crate::shortcuts_page::ShortcutsPage::new(window, cx));
                (PageView::Shortcuts(page), None)
            }
        }
    }

    /// A page's events, for the tab `tab` that shows it.
    fn page_events<P: EventEmitter<PageEvent>>(
        page: &Entity<P>,
        tab: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            page,
            window,
            move |shell, _, event: &PageEvent, window, cx| shell.on_page(tab, event, window, cx),
        )
    }

    /// Show `route` in the checked-out tab.
    fn show(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.show_route(route, true, window, cx);
    }

    /// Show `route`; `focus` moves the keyboard to the page (not when the
    /// tree previews a row with `Space`).
    fn show_route(
        &mut self,
        route: Route,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.bench().map(|b| b.current) else {
            return;
        };
        let (page, sub) = self.make_view(route, tab, window, cx);
        let Some(bench) = self.bench() else { return };
        bench.page = page;
        bench._page_sub = sub;
        self.follow_route(route, focus, window, cx);
    }

    /// What follows the checked-out page: the active project and its tree
    /// node unfolded (4.2, 4.6), recent topics, the terminal's origin, the
    /// saved layout, and focus when `focus`.
    fn follow_route(
        &mut self,
        route: Route,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        // Opening something dismisses the sidebar shown over the page.
        bench.ui.left_overlay = false;
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
        self.sync_page_kind_views(window, cx);
        self.save_layout(cx);
        if focus {
            self.focus_page(window, cx);
        }
        cx.notify();
    }

    fn on_page(
        &mut self,
        tab: TabId,
        event: &PageEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A page in another group: that group takes focus first.
        let current = self.bench().map(|b| b.current);
        if current != Some(tab)
            && matches!(event, PageEvent::Open(_) | PageEvent::NewKb(_))
            && let Some(bench) = self.bench()
            && let Some(spot) = bench.layout.locate(Tab::Page(tab))
        {
            bench.layout.activate(spot);
            self.after_layout(window, cx);
        }
        let current = self.bench().map(|b| b.current);
        match event {
            PageEvent::Open(route) => self.navigate(*route, true, window, cx),
            PageEvent::Saved(route) => {
                if let Some(bench) = self.bench() {
                    if current == Some(tab) {
                        bench.history.open(*route);
                    } else if let Some(slot) = bench.slots.get_mut(&tab) {
                        slot.history.open(*route);
                    }
                }
                self.save_layout(cx);
                cx.notify();
            }
            PageEvent::NewKb(draft) => self.new_kb_draft(*draft, window, cx),
            PageEvent::Pick(picker, on_pick) => {
                self.open_picker(picker.clone(), on_pick.clone(), window, cx)
            }
            PageEvent::Triage(project) => self.file_with_claude(*project, window, cx),
            PageEvent::Located(project) => {
                let project = *project;
                if current == Some(tab)
                    && let Some(bench) = self.bench()
                {
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
        let tab = bench.current;
        let page = cx.new(|cx| DocPage::create(ws, draft, window, cx));
        let sub = Self::page_events(&page, tab, window, cx);
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

    /// Read the titles of the documents and memories open in tabs.
    fn refresh_kb_titles(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let ids: Vec<KbId> = std::iter::once(&bench.history)
            .chain(bench.slots.values().map(|slot| &slot.history))
            .filter_map(|h| match h.current() {
                Some(Route::Kb(id)) => Some(id),
                _ => None,
            })
            .collect();
        if ids.is_empty() || ids.iter().all(|id| bench.kb_titles.contains_key(id)) {
            return;
        }
        let task = bench.ws.read(cx).db.run(cx, move |store| {
            Ok(ids
                .into_iter()
                .filter_map(|id| store.kb_entry(id).ok().map(|e| (id, e.title)))
                .collect::<Vec<_>>())
        });
        bench._kb_titles = cx.spawn(async move |shell, cx| {
            let Ok(titles) = task.await else { return };
            let _ = shell.update(cx, |shell, cx| {
                if let Some(bench) = shell.bench() {
                    bench.kb_titles.extend(titles);
                }
                cx.notify();
            });
        });
    }
    /// Write the layout (page tabs as route keys) and the tree's folds to
    /// `preferences.json` (ids only, 4.7).
    fn save_layout(&mut self, cx: &mut Context<Self>) {
        self.refresh_kb_titles(cx);
        let Some(bench) = self.bench() else { return };
        let mut collapsed: Vec<String> = bench.folds.collapsed.iter().cloned().collect();
        let mut closed: Vec<String> = bench.folds.closed_shown.iter().cloned().collect();
        collapsed.sort();
        closed.sort();
        let pages: Vec<(TabId, String)> = bench
            .layout
            .page_tabs()
            .into_iter()
            .filter_map(|id| {
                let route = if id == bench.current {
                    bench.history.current()
                } else {
                    bench.slots.get(&id).and_then(|s| s.history.current())
                };
                route.map(|r| (id, r.key()))
            })
            .collect();
        // The first-run page has no tab worth keeping.
        let layout = (!pages.is_empty()).then(|| crate::layout::Saved {
            layout: bench.layout.clone(),
            pages,
        });
        let current = settings::current(cx);
        if current.collapsed == collapsed
            && current.closed_shown == closed
            && current.layout == layout
        {
            return;
        }
        settings::change(cx, |prefs| {
            prefs.collapsed = collapsed;
            prefs.closed_shown = closed;
            if layout.is_some() {
                prefs.layout = layout;
            }
        });
    }

    // ---- tabs (layout.md 4.4, workbench.md 4.8.8) ----

    /// Open `route` in a new tab of the focused group, or focus the tab
    /// that already shows it.
    fn open_in_tab(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if let Some(at) = bench.find_tab(route) {
            if let Some(spot) = bench.layout.locate(Tab::Page(at)) {
                bench.layout.activate(spot);
            }
            self.after_layout(window, cx);
            self.focus_page(window, cx);
            return;
        }
        self.open_tab(History::at(route), window, cx);
    }

    /// A new page tab holding `history`, after the focused group's active
    /// tab. Opening a tab is not a layout step: closing it undoes it.
    fn open_tab(&mut self, history: History, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let id = bench.new_tab_id();
        bench.slots.insert(
            id,
            PageSlot {
                history,
                view: None,
                sub: None,
            },
        );
        bench.layout.add_page(id);
        self.after_layout(window, cx);
        self.focus_page(window, cx);
    }

    /// Close page tab `id`; the last page of the window cannot be closed.
    fn close_page(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if bench.layout.close_page(id).is_err() {
            return;
        }
        let history = if id == bench.current {
            Some(bench.history.clone())
        } else {
            bench.slots.get(&id).map(|s| s.history.clone())
        };
        bench.closed_tabs.extend(history);
        self.after_layout(window, cx);
        self.focus_page(window, cx);
    }

    /// "Close other tabs": every page but the checked-out one.
    fn close_other_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let others: Vec<TabId> = bench
            .layout
            .page_tabs()
            .into_iter()
            .filter(|id| *id != bench.current)
            .collect();
        for id in others {
            if bench.layout.close_page(id).is_ok()
                && let Some(slot) = bench.slots.get(&id)
            {
                bench.closed_tabs.push(slot.history.clone());
            }
        }
        self.after_layout(window, cx);
    }

    fn reopen_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let Some(history) = bench.closed_tabs.pop() else {
            return;
        };
        self.open_tab(history, window, cx);
    }

    /// `Ctrl+Tab` / `Ctrl+Shift+Tab`, wrapping. From the terminal or the
    /// chat the keyboard stays where it is (`focus` false).
    fn cycle_tab(
        &mut self,
        forward: bool,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        let group = bench.layout.focused;
        let g = &bench.layout.page.groups[group];
        let n = g.tabs.len();
        if n < 2 {
            return;
        }
        let at = if forward {
            (g.active + 1) % n
        } else {
            (g.active + n - 1) % n
        };
        if focus {
            self.activate_tab(Place::Page, group, at, window, cx);
        } else {
            bench.layout.activate(crate::layout::Spot {
                place: Place::Page,
                group,
                index: at,
            });
            self.after_layout(window, cx);
        }
    }

    /// A tab's icon and title (layout.md 4.4). The active tab shows a
    /// draft as "New document" or "New memory".
    fn tab_label(
        &self,
        route: Route,
        active: bool,
        locale: Locale,
        cx: &App,
    ) -> (Icon, Option<Status>, String) {
        let Content::Workbench(bench) = &self.content else {
            return (Icon::FileText, None, String::new());
        };
        if active
            && let PageView::Doc(page) = &bench.page
            && let Some(kind) = page.read(cx).draft_kind()
        {
            return (Icon::FileText, None, draft_label(locale, kind));
        }
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
                let doc = |view: &PageView| match view {
                    PageView::Doc(page) => page.read(cx).located().and_then(|(_, t)| t),
                    _ => None,
                };
                // Tabs without a view: the titles read for them (`kb_titles`).
                let title = if bench.history.current() == Some(route) {
                    doc(&bench.page)
                } else {
                    bench
                        .slots
                        .values()
                        .find(|s| s.history.current() == Some(route))
                        .and_then(|s| s.view.as_ref())
                        .and_then(doc)
                }
                .or_else(|| bench.kb_titles.get(&id).cloned());
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

    /// Focus the focused group: its view, or its page's own handle, else
    /// the group. Pages that focus their own editor on open (inbox, editor)
    /// keep it.
    fn focus_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let group = bench.layout.focused.min(2);
        if let Some(Tab::View(view)) = bench.layout.focused_tab() {
            return self.focus_view(view, window, cx);
        }
        let handle = match &bench.page {
            PageView::Record(page) => page.read(cx).focus_handle(cx),
            PageView::Doc(page) if page.read(cx).editing() => return,
            PageView::Doc(page) => page.read(cx).focus_handle(cx),
            PageView::Inbox(_) => return,
            _ => bench.group_focus[group].clone(),
        };
        window.focus(&handle, cx);
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
        self.open_palette_with(scope, create, None, window, cx);
    }

    /// The palette as a picker a page asked for (a destination, a heading).
    pub(super) fn open_picker(
        &mut self,
        picker: crate::palette::Picker,
        on_pick: crate::palette::OnPick,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_palette_with(None, None, Some(picker), window, cx);
        if let Some(bench) = self.bench() {
            bench.on_pick = Some(on_pick);
        }
    }

    fn open_palette_with(
        &mut self,
        scope: Option<Group>,
        create: Option<CreateKind>,
        picker: Option<crate::palette::Picker>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let context = self.command_context(window, cx);
        let Some(bench) = self.bench() else { return };
        bench.on_pick = None;
        let ws = bench.ws.read(cx);
        let setup = Setup {
            scope,
            create,
            picker,
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

    fn command_context(&self, window: &Window, cx: &App) -> commands::Context {
        let Content::Workbench(bench) = &self.content else {
            return commands::Context::default();
        };
        let mut states = [None; ViewId::ALL.len()];
        for view in views() {
            let place = bench.layout.locate(Tab::View(view)).map(|s| s.place);
            states[view as usize] = Some(commands::ViewState {
                place: place.or(if view.is_page_kind() {
                    Some(Place::Page)
                } else {
                    None
                }),
                visible: bench.layout.is_visible(view),
                can_split: bench.layout.can_split_off(view),
            });
        }
        let focused_view = views().find(|v| {
            bench.layout.is_visible(*v)
                && self
                    .view_focus(*v, cx)
                    .is_some_and(|h| h.contains_focused(window, cx))
        });
        commands::Context {
            has_project: bench.active_project.is_some(),
            on_topic: matches!(bench.history.current(), Some(Route::Topic(_))),
            on_record: matches!(bench.page, PageView::Record(_)),
            on_doc: matches!(bench.history.current(), Some(Route::Kb(_))),
            can_back: bench.history.can_go_back(),
            can_forward: bench.history.can_go_forward(),
            tabs: bench.layout.page_tabs().len() > 1,
            closed_tab: !bench.closed_tabs.is_empty(),
            views: states,
            focused_view,
            groups: bench.layout.page.groups.len(),
            can_split_right: self.can_split_page(Direction::Row, window),
            can_split_down: self.can_split_page(Direction::Column, window),
            layout_undo: !bench.layout_undo.is_empty(),
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
                    bench.on_pick = None;
                }
                self.close_overlay(window, cx)
            }
            PaletteEvent::Picked(index) => {
                let index = *index;
                let on_pick = self.bench().and_then(|b| b.on_pick.take());
                self.close_overlay(window, cx);
                if let Some(on_pick) = on_pick {
                    (on_pick.0)(index, window, cx);
                }
            }
            PaletteEvent::Run(target, new_tab) => {
                let (target, new_tab) = (target.clone(), *new_tab);
                self.close_overlay(window, cx);
                let route = match target {
                    Target::Topic(id, _) => Route::Topic(id),
                    Target::Project(id) => Route::Project(id),
                    Target::Kb(id, _) => Route::Kb(id),
                    Target::Command(command) => return self.run_command(command, window, cx),
                    Target::Choice(_) => return,
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
            Command::GoHeading | Command::SaveFile | Command::KbMoveTo => {
                if let Some(bench) = self.bench()
                    && let PageView::Doc(page) = &bench.page
                {
                    page.update(cx, |page, cx| match command {
                        Command::GoHeading => page.go_to_heading(cx),
                        Command::SaveFile => page.save_as_file(cx),
                        _ => page.move_to(cx),
                    });
                }
            }
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
                let tab = self.bench().map_or(TabId(0), |b| b.current);
                let page = cx.new(|cx| KnowledgePage::new(ws, project, filters, window, cx));
                let sub = Self::page_events(&page, tab, window, cx);
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
            Command::Undo => self.undo(window, cx),
            Command::Redo => self.redo(window, cx),
            Command::ToggleSidebar => self.toggle_sidebar(window, cx),
            Command::ToggleRight => self.toggle_area(Side::Right, window, cx),
            Command::ToggleBottom => self.toggle_area(Side::Bottom, window, cx),
            Command::ToggleTerminal => self.toggle_terminal(window, cx),
            #[cfg(windows)]
            Command::ToggleChat => self.toggle_chat(window, cx),
            Command::View(action, view) => self.view_action(view, action, window, cx),
            Command::SplitRight => self.split_page(Direction::Row, window, cx),
            Command::SplitDown => self.split_page(Direction::Column, window, cx),
            Command::TabNextGroup => self.move_tab_to_group(1, window, cx),
            Command::TabPreviousGroup => self.move_tab_to_group(-1, window, cx),
            Command::JoinGroups => self.join_groups(window, cx),
            Command::NextArea => self.cycle_area(true, window, cx),
            Command::PreviousArea => self.cycle_area(false, window, cx),
            Command::Zoom => self.toggle_zoom(window, cx),
            Command::GrowArea => self.step_area(true, window, cx),
            Command::ShrinkArea => self.step_area(false, window, cx),
            Command::LayoutUndo => self.undo_layout(window, cx),
            Command::LayoutReset => self.reset_layout(window, cx),
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
                // The focused group's active tab: a page closes, a view
                // tab hides its view (4.8.8).
                if let Some(tab) = self.bench().and_then(|b| b.layout.focused_tab()) {
                    self.close_tab_of(tab, window, cx)
                }
            }
            Command::CloseOtherTabs => self.close_other_tabs(window, cx),
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
                DialogEvent::Close => {
                    if let Some(bench) = shell.bench() {
                        bench.pending_triage = None;
                    }
                    shell.close_overlay(window, cx)
                }
                DialogEvent::RestartTerminal => {
                    shell.close_overlay(window, cx);
                    match shell.bench().and_then(|b| b.pending_triage.take()) {
                        Some(origin) => shell.start_triage(origin, window, cx),
                        None => shell.do_restart_terminal(window, cx),
                    }
                }
                DialogEvent::RemoveSetup => {
                    shell.close_overlay(window, cx);
                    claude_setup::run_remove(cx);
                }
                #[cfg(windows)]
                DialogEvent::BrowserClear => {
                    shell.close_overlay(window, cx);
                    if let Some(bench) = shell.bench() {
                        let browser = bench.browser.clone();
                        browser.update(cx, |b, cx| b.clear_data(window, cx));
                    }
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

    /// ``Ctrl+` ``: the terminal's toggle (4.8.5).
    fn terminal_focused(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        self.bench().is_some_and(|bench| {
            bench
                .dock
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
        })
    }

    fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_view(ViewId::Terminal, window, cx);
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
        self.change_layout(None, window, cx, |l| {
            l.show_view(ViewId::Terminal).is_some()
        });
        let Some(bench) = self.bench() else { return };
        bench.ui.terminal_shown = true;
        bench
            .dock
            .update(cx, |dock, cx| dock.restart(origin, window, cx));
        cx.notify();
    }

    /// "File with Claude Code" (screens.md 8.2): the terminal shows, and
    /// the triage command runs in the project's session, typed there only
    /// when Claude Code is idle at its prompt. Mid-task, nothing is typed:
    /// the restart confirmation asks before a new session starts on it.
    fn file_with_claude(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        let projects = bench.ws.read(cx).projects.clone();
        let Some(name) = tree::locate_project(&projects, project).and_then(|l| l.project) else {
            return;
        };
        let origin = Origin {
            owner: Owner::Project(project),
            project,
            name: name.1,
        };
        self.show_terminal_for_command(window, cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        let sent = dock.update(cx, |dock, cx| {
            dock.send(origin.clone(), TRIAGE_COMMAND, window, cx)
        });
        crate::log::line(&format!("triage: {sent:?}"));
        if sent == crate::terminal_dock::Send::Busy {
            if let Some(bench) = self.bench() {
                bench.pending_triage = Some(origin);
            }
            self.open_dialog(DialogKind::RestartTerminal, window, cx);
        }
    }

    fn start_triage(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        self.show_terminal_for_command(window, cx);
        let Some(bench) = self.bench() else { return };
        bench.dock.update(cx, |dock, cx| {
            dock.restart_on(origin, TRIAGE_COMMAND, window, cx)
        });
        cx.notify();
    }

    /// Show the Terminal view without starting a session from the page:
    /// the command decides which session runs.
    fn show_terminal_for_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench.ui.terminal_shown = true;
        }
        self.change_layout(None, window, cx, |l| {
            l.show_view(ViewId::Terminal).is_some()
        });
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
            ChatEvent::View(action) => self.view_action(ViewId::Chat, *action, window, cx),
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
    fn on_browser(
        &mut self,
        browser: &Entity<BrowserPanel>,
        event: &BrowserEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            BrowserEvent::View(action) => {
                self.view_action(ViewId::AgentBrowser, *action, window, cx)
            }
            BrowserEvent::Clear => self.open_dialog(DialogKind::BrowserClear, window, cx),
            BrowserEvent::Kept(kept) => {
                browser.read(cx).release_keyboard(cx);
                self.run_kept(*kept, window, cx)
            }
            BrowserEvent::ClearFailed => {
                let message = text(settings::locale(cx), Key::BrowserClearFailed);
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
    /// (interaction.md 6.1).
    fn run_kept(&mut self, kept: Kept, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let chat = bench.chat.clone();
        let action: Box<dyn gpui_kit::Action> = match kept {
            Kept::Palette => Box::new(OpenPalette),
            Kept::ToggleChat => Box::new(ToggleChat),
            Kept::ToggleTerminal => Box::new(ToggleTerminal),
            Kept::ToggleRight => Box::new(ToggleRight),
            Kept::Focus(1) => Box::new(FocusSidebar),
            Kept::Focus(2) => Box::new(FocusPage),
            Kept::Focus(3) => Box::new(FocusTerminal),
            Kept::Focus(4) => Box::new(FocusChat),
            // The chat keeps the keyboard: the next Mod+W is the page's
            // own, not Acta's Close tab.
            Kept::NextTab | Kept::PreviousTab => {
                self.cycle_tab(kept == Kept::NextTab, false, window, cx);
                return;
            }
            Kept::NextPane => Box::new(NextArea),
            Kept::PreviousPane => Box::new(PreviousArea),
            Kept::ZoomToolWindow => Box::new(ZoomArea),
            Kept::Focus(_) => return,
        };
        // The keyboard comes back to the GPUI window before the action
        // runs, so that what it focuses receives the next keys.
        chat.read(cx).release_keyboard(cx);
        window.dispatch_action(action, cx);
    }

    /// `Mod+Shift+H`: the chat's toggle (4.8.5); macOS and Linux open
    /// claude.ai in the browser (components-app.md 5, Chat panel).
    fn toggle_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if CHAT {
            self.toggle_view(ViewId::Chat, window, cx);
        } else {
            cx.open_url(CLAUDE_HOME);
        }
    }

    /// `Mod+4`: focus the chat wherever it is, showing it first if needed.
    fn focus_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if CHAT {
            self.view_action(ViewId::Chat, ViewAction::Show, window, cx);
        }
    }

    #[cfg(windows)]
    /// Whether a GPUI layer that can overlap a web view (the chat or the
    /// agent browser) is open (components-app.md 5: palette, quick
    /// capture, dialog, menu, the sidebar over the page).
    fn web_covered(&mut self, view: ViewId, cx: &mut Context<Self>) -> bool {
        let Some(bench) = self.bench() else {
            return false;
        };
        let page_menu = |view: &PageView| match view {
            PageView::Record(page) => page.read(cx).menu_open(),
            PageView::Knowledge(page) => page.read(cx).menu_open(),
            PageView::Settings(page) => page.read(cx).menu_open(),
            PageView::Doc(page) => page.read(cx).menu_open(),
            _ => false,
        };
        let any_page_menu = page_menu(&bench.page)
            || bench
                .slots
                .values()
                .filter_map(|s| s.view.as_ref())
                .any(page_menu)
            || bench
                .knowledge_view
                .as_ref()
                .is_some_and(|v| v.read(cx).menu_open());
        let place = bench.layout.locate(Tab::View(view)).map(|s| s.place);
        // The move toast sits at the bottom left of the page area, over the
        // sidebar shown over the page.
        let toast_over = bench.toast.is_some()
            && (place == Some(Place::Page)
                || (bench.ui.left_overlay && place == Some(Place::Side(Side::Left))));
        // The sidebar shown over the page covers what is under it, and
        // hosts what is in it (workbench.md 4.8.10).
        let left_over = bench.ui.left_overlay && place != Some(Place::Side(Side::Left));
        left_over
            || bench.ui.menu.is_some()
            || bench.tree_menu.is_some()
            || toast_over
            || bench.overlay.is_some()
            || any_page_menu
            || bench.chat.read(cx).menu_open()
            || bench.browser.read(cx).menu_open()
    }

    #[cfg(windows)]
    fn ask_chat_sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_dialog(DialogKind::ChatSignOut, window, cx);
    }

    #[cfg(not(windows))]
    fn ask_chat_sign_out(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    // ---- toast and undo ----

    fn show_toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        bench.toast = Some(toast);
        self.arm_toast_timer(cx);
        cx.notify();
    }

    /// The toast goes 6 s after it shows, or after the pointer leaves it
    /// (components.md 5, Toast); an error stays until dismissed.
    fn arm_toast_timer(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let stays = bench
            .toast
            .as_ref()
            .is_none_or(|t| t.kind == ToastKind::Error);
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
    }

    /// The toast is paused while the pointer is on it: a slow reader or a
    /// pointer on its way to Undo never sees it go.
    fn hover_toast(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            if let Some(bench) = self.bench() {
                bench.toast_timer = Task::ready(());
            }
        } else {
            self.arm_toast_timer(cx);
        }
    }

    /// `Mod+Z`: the newest undo step of the window, a layout change
    /// (4.8.11, rule 3) or a write of the active project (pages of no
    /// project, such as Settings, keep the project of the page before them).
    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let project = bench.active_project;
        let write = bench.ws.read(cx).newest_undo(project);
        let layout = bench.layout_undo.newest();
        if crate::layout::layout_step_first(layout, write) {
            return self.undo_layout(window, cx);
        }
        bench.toast = None;
        bench.ws.update(cx, |ws, cx| ws.undo_active(project, cx));
        cx.notify();
    }

    #[cfg(debug_assertions)]
    fn dev_triage_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.view_action(ViewId::Terminal, ViewAction::Show, window, cx);
        cx.spawn_in(window, async move |shell, cx| {
            let dump = |shell: &mut Shell, step: &str, cx: &mut Context<Shell>| {
                if let Some(bench) = shell.bench()
                    && let Some(project) = bench.active_project
                {
                    let screen = bench.dock.read(cx).dev_screen(project, cx);
                    eprintln!("[triage-check] {step}\n{}", screen.unwrap_or_default());
                }
            };
            cx.background_executor()
                .timer(Duration::from_secs(30))
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                dump(shell, "before", cx);
                if let Some(project) = shell.bench().and_then(|b| b.active_project) {
                    shell.file_with_claude(project, window, cx);
                }
            });
            cx.background_executor()
                .timer(Duration::from_secs(60))
                .await;
            let _ = shell.update_in(cx, |shell, _, cx| {
                dump(shell, "after triage", cx);
                if let Some(bench) = shell.bench()
                    && let Some(project) = bench.active_project
                {
                    let typed = bench
                        .dock
                        .update(cx, |dock, cx| dock.dev_type_if_idle(project, "/exit", cx));
                    eprintln!("[triage-check] /exit typed: {typed}");
                }
            });
            cx.background_executor()
                .timer(Duration::from_secs(10))
                .await;
            let _ = shell.update_in(cx, |shell, _, cx| dump(shell, "after exit", cx));
        })
        .detach();
    }

    /// `Mod+Shift+Z` (`Ctrl+Y` on Windows and Linux): the newest undone
    /// step of the window, layout or write, by the same rule as `Mod+Z`.
    fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let project = bench.active_project;
        let write = bench.ws.read(cx).newest_redo(project);
        let layout = bench.layout_undo.newest_redo();
        if crate::layout::layout_step_first(layout, write) {
            return self.redo_layout(window, cx);
        }
        bench.toast = None;
        bench.ws.update(cx, |ws, cx| ws.redo_active(project, cx));
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

    fn toggle_fold(&mut self, key: String, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench.folds.toggle(key);
        }
        self.save_layout(cx);
        cx.notify();
    }

    fn tree_key(&mut self, key: TreeKey, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let Some(bench) = self.bench() else { return };
        if rows.is_empty() {
            return;
        }
        // An open menu takes the arrows, Enter and Esc. A menu whose row
        // went away (deleted by another process) closes.
        if bench.tree_menu.is_some() && self.menu_items(&rows, cx).is_empty() {
            self.close_tree_menu(cx);
        }
        let Some(bench) = self.bench() else { return };
        if bench.tree_menu.is_some() {
            match key {
                TreeKey::Up | TreeKey::Down => {
                    let n = self.menu_items(&rows, cx).len();
                    if let Some(menu) = self.bench().and_then(|b| b.tree_menu.as_mut()) {
                        menu.highlight = if key == TreeKey::Up {
                            menu.highlight.saturating_sub(1)
                        } else {
                            (menu.highlight + 1).min(n.saturating_sub(1))
                        };
                    }
                }
                TreeKey::Open | TreeKey::Right => {
                    let at = self.bench().and_then(|b| b.tree_menu).map(|m| m.highlight);
                    if let Some(at) = at {
                        self.choose_menu_item(at, window, cx);
                    }
                }
                _ => {}
            }
            cx.notify();
            return;
        }
        let at = bench.cursor.index(&rows);
        let row = rows[at].clone();
        match key {
            TreeKey::Up => bench.cursor.set(&rows, at.saturating_sub(1)),
            TreeKey::Down => bench.cursor.set(&rows, at + 1),
            TreeKey::Home => bench.cursor.set(&rows, 0),
            TreeKey::End => bench.cursor.set(&rows, rows.len() - 1),
            TreeKey::Left => match (row.fold_key(), row.is_expanded()) {
                (Some(k), Some(true)) => self.toggle_fold(k, cx),
                _ => {
                    if let Some(parent) = tree::parent_index(&rows, at) {
                        bench.cursor.set(&rows, parent);
                    }
                }
            },
            TreeKey::Right => match (row.fold_key(), row.is_expanded()) {
                (Some(k), Some(false)) => self.toggle_fold(k, cx),
                (Some(_), Some(true)) => bench.cursor.set(&rows, at + 1),
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
            TreeKey::Delete => self.delete_row(&row, cx),
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
                    // The cursor is on the record, so it follows it there.
                    let project = row_project(&projects, &row);
                    bench
                        .ws
                        .update(cx, |ws, cx| ws.reorder(record, target, project, cx));
                }
            }
        }
        cx.notify();
    }

    fn delete_row(&mut self, row: &Row, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        if let Some(record) = tree::row_record(row) {
            let projects = bench.ws.read(cx).projects.clone();
            let project = row_project(&projects, row);
            bench.ws.update(cx, |ws, cx| ws.delete(record, project, cx));
        }
    }

    /// Keys the tree reads itself (interaction.md 6.1): the letter keys,
    /// type-ahead, `F2`, `Shift+F10` or the menu key, and `1` to `6` in the
    /// status menu.
    fn tree_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        if m.control || m.alt || m.platform || m.function {
            return;
        }
        let rows = self.rows(cx);
        let Some(bench) = self.bench() else { return };
        if rows.is_empty() || bench.rename.is_some() {
            return;
        }
        let at = bench.cursor.index(&rows);
        let row = rows[at].clone();
        if (keystroke.key == "f10" && m.shift) || keystroke.key == "menu" {
            self.open_tree_menu(MenuKind::Row, &rows, cx);
            cx.stop_propagation();
            return;
        }
        if keystroke.key == "f2" {
            self.start_rename(&row, window, cx);
            cx.stop_propagation();
            return;
        }
        // In the status menu, 1 to 6 pick a status.
        if let Some(menu) = bench.tree_menu {
            if menu.kind == MenuKind::Status
                && let Some(n) = keystroke
                    .key
                    .parse::<usize>()
                    .ok()
                    .filter(|n| (1..=6).contains(n))
            {
                self.choose_menu_item(n - 1, window, cx);
                cx.stop_propagation();
            }
            return;
        }
        let Some(ch) = keystroke
            .key_char
            .as_deref()
            .filter(|c| c.chars().count() == 1)
            .and_then(|c| c.chars().next())
            .filter(|c| !c.is_control())
        else {
            return;
        };
        let typing = bench.find.1.is_some_and(|t| t.elapsed() < TYPE_AHEAD_PAUSE);
        let buffer = if typing {
            bench.find.0.clone()
        } else {
            String::new()
        };
        match tree::letter(&buffer, ch) {
            tree::Letter::Up => self.tree_key(TreeKey::Up, window, cx),
            tree::Letter::Down => self.tree_key(TreeKey::Down, window, cx),
            tree::Letter::Status => self.open_tree_menu(MenuKind::Status, &rows, cx),
            tree::Letter::Move => self.open_tree_menu(MenuKind::Epics, &rows, cx),
            tree::Letter::Rename => self.start_rename(&row, window, cx),
            tree::Letter::Preview => {
                if let Some(route) = row.route() {
                    if let Some(bench) = self.bench() {
                        bench.history.open(route);
                    }
                    self.show_route(route, false, window, cx);
                }
            }
            tree::Letter::Find(query) => {
                let from = if query.chars().count() == 1 {
                    at + 1
                } else {
                    at
                };
                if let Some(bench) = self.bench() {
                    if let Some(hit) = tree::type_ahead(&rows, from, &query) {
                        bench.cursor.set(&rows, hit);
                    }
                    bench.find = (query, Some(Instant::now()));
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// The items of the open tree menu.
    fn menu_items(&mut self, rows: &[Row], cx: &mut Context<Self>) -> Vec<MenuItem> {
        let no_epic = text(settings::locale(cx), Key::TreeNoEpic);
        let Some(bench) = self.bench() else {
            return Vec::new();
        };
        let Some(menu) = bench.tree_menu else {
            return Vec::new();
        };
        let Some(row) = rows.iter().find(|r| r.key() == menu.row) else {
            return Vec::new();
        };
        let projects = &bench.ws.read(cx).projects;
        tree::menu_items(projects, row, menu.kind, &no_epic)
    }

    /// Open a menu of the cursor's row, if it has one.
    fn open_tree_menu(&mut self, kind: MenuKind, rows: &[Row], cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let row = &rows[bench.cursor.index(rows)];
        let highlight = match row {
            Row::Topic { status, .. } if kind == MenuKind::Status => {
                Status::ALL.iter().position(|s| s == status).unwrap_or(0)
            }
            _ => 0,
        };
        bench.tree_menu = Some(TreeMenu {
            row: row.key(),
            kind,
            highlight,
        });
        if self.menu_items(rows, cx).is_empty()
            && let Some(bench) = self.bench()
        {
            bench.tree_menu = None;
        }
        cx.notify();
    }

    fn close_tree_menu(&mut self, cx: &mut Context<Self>) -> bool {
        let closed = self.bench().and_then(|b| b.tree_menu.take()).is_some();
        if closed {
            cx.notify();
        }
        closed
    }

    fn choose_menu_item(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let items = self.menu_items(&rows, cx);
        let Some(bench) = self.bench() else { return };
        let Some(menu) = bench.tree_menu else { return };
        let (Some(item), Some(row)) = (
            items.get(at).cloned(),
            rows.iter().find(|r| r.key() == menu.row).cloned(),
        ) else {
            return;
        };
        bench.tree_menu = None;
        let ws = bench.ws.clone();
        match item {
            MenuItem::Open => {
                if let Some(route) = row.route() {
                    self.navigate(route, true, window, cx);
                }
            }
            MenuItem::OpenInNewTab => {
                if let Some(route) = row.route() {
                    self.open_in_tab(route, window, cx);
                }
            }
            MenuItem::Rename => self.start_rename(&row, window, cx),
            MenuItem::StatusMenu => self.open_tree_menu(MenuKind::Status, &rows, cx),
            MenuItem::MoveMenu => self.open_tree_menu(MenuKind::Epics, &rows, cx),
            MenuItem::Status(status) => {
                if let Row::Topic { id, .. } = row {
                    ws.update(cx, |ws, cx| ws.set_status(id, status, cx));
                }
            }
            MenuItem::MoveTo(epic, _) => {
                if let Row::Topic { id, .. } = row {
                    ws.update(cx, |ws, cx| ws.move_topic(id, epic, cx));
                }
            }
            MenuItem::Delete => self.delete_row(&row, cx),
        }
        cx.notify();
    }

    /// `E` / `F2`: the row's title becomes an inline edit (components.md 5).
    fn start_rename(&mut self, row: &Row, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(title), Some(_)) = (row.title(), owner_of(row)) else {
            return;
        };
        let key = row.key();
        let state = cx.new(|cx| InputState::new(window, cx).default_value(title.to_owned()));
        state.update(cx, |state, cx| state.focus(window, cx));
        let sub = cx.subscribe_in(
            &state,
            window,
            |shell, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => shell.commit_rename(window, cx),
                _ => {}
            },
        );
        let Some(bench) = self.bench() else { return };
        bench._subs.push(sub);
        bench.tree_menu = None;
        bench.rename = Some(Rename {
            key,
            state,
            saving: false,
        });
        cx.notify();
    }

    /// Save the inline edit; it stays with the typed text until the write
    /// succeeds (interaction.md 6.4).
    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let Some(bench) = self.bench() else { return };
        let Some(Rename { key, state, saving }) = bench.rename.clone() else {
            return;
        };
        if saving {
            return;
        }
        let new = state.read(cx).value().trim().to_owned();
        let row = rows.iter().find(|r| r.key() == key);
        let unchanged = row.and_then(Row::title).is_none_or(|old| old == new);
        let owner = row.and_then(owner_of);
        let (Some(owner), false, false) = (owner, new.is_empty(), unchanged) else {
            self.cancel_rename(window, cx);
            return;
        };
        if let Some(rename) = bench.rename.as_mut() {
            rename.saving = true;
        }
        let task = bench.ws.update(cx, |ws, cx| ws.rename(owner, new, cx));
        cx.spawn_in(window, async move |shell, cx| {
            let saved = task.await.is_ok();
            let _ = shell.update_in(cx, |shell, window, cx| {
                if saved {
                    shell.cancel_rename(window, cx);
                } else if let Some(rename) = shell.bench().and_then(|b| b.rename.as_mut()) {
                    rename.saving = false;
                }
            });
        })
        .detach();
    }

    /// Close the inline edit. The keyboard goes back to the tree if the
    /// edit had it or nothing has it (the field gave it up on `Enter`): a
    /// click elsewhere that ended it keeps its focus.
    fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let nowhere = window.focused(cx).is_none();
        let Some(bench) = self.bench() else { return };
        if let Some(rename) = bench.rename.take()
            && (nowhere || rename.state.focus_handle(cx).is_focused(window))
        {
            let handle = bench.tree_focus.clone();
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    /// The route whose row the tree selects: the open page, or for a
    /// document or memory its owner (components-knowledge.md 5).
    fn tree_route(&mut self, cx: &mut Context<Self>) -> Option<Route> {
        let bench = self.bench()?;
        let current = bench.history.current();
        if let PageView::Doc(page) = &bench.page
            && let Some((owner, _)) = page.read(cx).located()
        {
            return Some(match owner {
                Owner::Project(id) => Route::Project(id),
                Owner::Epic(id) => Route::Epic(id),
                Owner::Topic(id) => Route::Topic(id),
            });
        }
        current
    }

    /// The Projects view (4.8.3): the tree, with Trash and Settings pinned
    /// at its bottom; it fills its group.
    fn projects_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let tokens = cx.global::<Tokens>().clone();
        let t = &tokens;
        let locale = settings::locale(cx);
        let rows = self.rows(cx);
        let current = self.tree_route(cx);
        let Some(bench) = self.bench() else {
            return div().into_any_element();
        };
        let cursor = bench.cursor.index(&rows);
        let tree_focus = bench.tree_focus.clone();
        // components.md 5: the keyboard cursor is a focus ring, which
        // pointer input never draws (interaction.md 6.2).
        let ring = tree_focus.is_focused(window) && window.last_input_was_keyboard();
        let menu = bench.tree_menu;
        let renaming = bench.rename.as_ref().map(|r| (r.key, r.state.clone()));
        let menu_items = self.menu_items(&rows, cx);
        let unread: Vec<bool> = match self.bench() {
            Some(bench) => {
                let ws = bench.ws.read(cx);
                rows.iter()
                    .map(|row| tree::row_unread(row, &ws.projects, &ws.unread))
                    .collect()
            }
            None => vec![false; rows.len()],
        };
        let items: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let key = row.key();
                let menu = menu.filter(|m| m.row == key).map(|m| {
                    let status = match row {
                        Row::Topic { status, .. } => Some(*status),
                        _ => None,
                    };
                    tree_menu(&menu_items, m, status, locale, t, cx)
                });
                let edit = renaming
                    .as_ref()
                    .filter(|(k, _)| *k == key)
                    .map(|(_, state)| state.clone());
                let cursor = ring && i == cursor && edit.is_none();
                tree_row(
                    row, i, current, unread[i], cursor, menu, edit, locale, t, cx,
                )
            })
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
            .size_full()
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
                    .on_action(cx.listener(|s, _: &DismissSidebar, w, cx| {
                        if !s.close_tree_menu(cx) {
                            s.dismiss_sidebar(w, cx)
                        }
                    }))
                    .on_action(cx.listener(|s, _: &TreeOpenTab, w, cx| {
                        let rows = s.rows(cx);
                        let route = s
                            .bench()
                            .and_then(|b| rows.get(b.cursor.index(&rows)))
                            .and_then(Row::route);
                        if let Some(route) = route {
                            s.open_in_tab(route, w, cx);
                        }
                    }))
                    .on_action(cx.listener(|s, _: &Escape, w, cx| s.cancel_rename(w, cx)))
                    .on_key_down(cx.listener(Self::tree_key_down))
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
            // A document or memory: its owner's place as links, then the
            // entry itself (layout.md 4.3).
            // A draft too, whose history step is still the page it was started
            // from: "New document" ends it.
            (Some(_), PageView::Doc(page)) => {
                let page = page.read(cx);
                let Some((owner, title)) = page.located() else {
                    return Vec::new();
                };
                let title = title.or_else(|| page.draft_kind().map(|k| draft_label(locale, k)));
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
    /// no scrim; dialogs draw their own scrim. Above every other layer (the
    /// sidebar over the page, the toast, menus, a drag).
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
            .with_priority(OVERLAY_PRIORITY)
            .into_any_element()
        };
        Some(match bench.overlay.as_ref()? {
            Overlay::Palette(palette) => top(palette.clone().into_any_element()),
            Overlay::Capture(capture) => top(capture.clone().into_any_element()),
            Overlay::Dialog(dialog) => deferred(dialog.clone())
                .with_priority(OVERLAY_PRIORITY)
                .into_any_element(),
        })
    }

    fn toast_element(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let bench = self.bench()?;
        let toast = bench.toast.clone()?;
        if matches!(
            toast.kind,
            ToastKind::Layout | ToastKind::LayoutReset | ToastKind::LayoutNoRoom
        ) {
            return Some(self.layout_toast_element(toast, &t, locale, cx));
        }
        let glyph = match toast.kind {
            ToastKind::Error => Some(icon(Icon::TriangleAlert, 16., t.danger)),
            ToastKind::Done => Some(icon(Icon::Check, 16., t.success)),
            _ => None,
        };
        Some(
            div()
                .id("toast")
                .on_hover(
                    cx.listener(|shell, hovered: &bool, _, cx| shell.hover_toast(*hovered, cx)),
                )
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
                        .on_click(cx.listener(|shell, _, window, cx| shell.undo(window, cx))),
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

    /// The move toast (4.8.11): the message on its own line, wrapping to
    /// two at most, then Undo, Reset layout and the `x` on a line under it.
    /// "Layout reset." has Undo only; the no-room toast its one action. It
    /// is drawn above the sidebar shown over the page, which would
    /// otherwise cover it.
    fn layout_toast_element(
        &mut self,
        toast: Toast,
        t: &Tokens,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let macos = cfg!(target_os = "macos");
        let kind = toast.kind;
        let undo = kind != ToastKind::LayoutNoRoom;
        let reset = kind == ToastKind::Layout;
        let room = kind == ToastKind::LayoutNoRoom;
        let element = div()
            .id("layout-toast")
            .occlude()
            .on_hover(cx.listener(|shell, hovered: &bool, _, cx| shell.hover_toast(*hovered, cx)))
            .absolute()
            .left(px(16.))
            .bottom(px(16.))
            .max_w(px(360.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(12.))
            .py(px(8.))
            .bg(t.bg_overlay)
            .border_1()
            .border_color(t.border_default)
            .rounded(px(RADIUS_LG))
            .shadow(vec![t.shadow_overlay()])
            .child(
                type_scale::UI
                    .el()
                    .text_color(t.text_primary)
                    .line_clamp(2)
                    .child(toast.message.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .when(undo, |this| {
                        this.child(
                            ui::button(
                                "toast-layout-undo",
                                text(locale, Key::CommonUndo),
                                Variant::Ghost,
                                false,
                                t,
                            )
                            .child(ui::kbd(
                                &[crate::i18n::KeyName::Mod.label(locale, macos), "Z"],
                                t,
                            ))
                            .on_click(
                                cx.listener(|shell, _, window, cx| shell.undo_layout(window, cx)),
                            ),
                        )
                    })
                    .when(reset, |this| {
                        this.child(
                            ui::button(
                                "toast-layout-reset",
                                text(locale, Key::CmdLayoutReset),
                                Variant::Ghost,
                                false,
                                t,
                            )
                            .on_click(
                                cx.listener(|shell, _, window, cx| shell.reset_layout(window, cx)),
                            ),
                        )
                    })
                    .when(room, |this| {
                        this.child(
                            ui::button(
                                "toast-layout-room",
                                text(locale, Key::LayoutNoRoomHideRight),
                                Variant::Ghost,
                                false,
                                t,
                            )
                            .on_click(cx.listener(
                                |shell, _, window, cx| shell.hide_right_and_move(window, cx),
                            )),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        ui::ghost_icon_button("toast-close", Icon::X, 24., false, false, true, t)
                            .on_click(cx.listener(|shell, _, _, cx| {
                                if let Some(bench) = shell.bench() {
                                    bench.toast = None;
                                }
                                cx.notify();
                            })),
                    ),
            );
        deferred(element).with_priority(2).into_any_element()
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
    let seed = std::env::var("ACTA_DEV_SEED").unwrap_or_default();
    if cfg!(debug_assertions)
        && (seed == "1" || seed == "bare")
        && let Ok(dir) = std::env::current_dir()
        && let Err(err) = if seed == "bare" {
            crate::dev_seed::bare(store, &dir)
        } else {
            crate::dev_seed::seed(store, &dir)
        }
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

/// "New document" / "New memory": a draft's place in the tab and the
/// breadcrumb.
fn draft_label(locale: Locale, kind: KbKind) -> String {
    text(
        locale,
        match kind {
            KbKind::Document => Key::KbNewDocument,
            KbKind::Memory => Key::KbNewMemory,
        },
    )
}

/// The record a tree row renames.
fn owner_of(row: &Row) -> Option<Owner> {
    match row {
        Row::Project { id, .. } => Some(Owner::Project(*id)),
        Row::Epic { id, .. } => Some(Owner::Epic(*id)),
        Row::Topic { id, .. } => Some(Owner::Topic(*id)),
        _ => None,
    }
}

fn record(
    ws: Entity<Workspace>,
    owner: Owner,
    tab: TabId,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (PageView, Option<Subscription>) {
    let page = cx.new(|cx| RecordPage::new(ws, owner, window, cx));
    let sub = Shell::page_events(&page, tab, window, cx);
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
#[allow(clippy::too_many_arguments)]
fn tree_row(
    row: &Row,
    index: usize,
    current: Option<Route>,
    unread: bool,
    cursor: bool,
    menu: Option<AnyElement>,
    edit: Option<Entity<InputState>>,
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
    // Renaming: the title becomes an inline edit (components.md 5).
    let title = |role: type_scale::TypeRole, label: String, color| match &edit {
        Some(state) => role
            .el()
            .flex_1()
            .min_w_0()
            .text_color(primary)
            .child(Input::new(state).appearance(false))
            .into_any_element(),
        None => role
            .el()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_color(color)
            .group_hover(id.clone(), move |s| s.text_color(primary))
            .child(label)
            .into_any_element(),
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
            .child(title(type_scale::UI_STRONG, name.clone(), rest))
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
            // The 14 glyph sits in a 16 slot, so topic titles line up
            // with the Inbox and Knowledge titles above them.
            el.child(icon_slot(ui::status_glyph(*status, 14., t)))
                .child(title(
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
            // Under the titles of its group, not under their glyphs.
            el.on_click(cx.listener(move |shell, _, _, cx| shell.toggle_fold(key.clone(), cx)))
                .child(div().w(px(16.)).flex_shrink_0())
                .child(type_scale::SMALL.el().text_color(t.text_tertiary).child(tr(
                    locale,
                    Key::TreeClosed,
                    &[("n", Arg::Count(*n as u64))],
                )))
        }
    };
    // The trailing slot: the unread dot (components.md 5, list / tree row).
    if unread {
        el = el.child(busy_dot(t));
    }
    if let Some(route) = row.route() {
        // Mod+click or a middle click opens the row in a new tab (4.4).
        el = el
            .on_click(
                cx.listener(move |shell, event: &gpui_kit::ClickEvent, window, cx| {
                    let rows = shell.rows(cx);
                    if let Some(bench) = shell.bench() {
                        bench.cursor.set(&rows, index);
                        bench.tree_menu = None;
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
            )
            // A right click opens the row's menu (components.md 5).
            .on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(move |shell, _, window, cx| {
                    let rows = shell.rows(cx);
                    let Some(bench) = shell.bench() else { return };
                    bench.cursor.set(&rows, index);
                    let handle = bench.tree_focus.clone();
                    window.focus(&handle, cx);
                    shell.open_tree_menu(MenuKind::Row, &rows, cx);
                }),
            );
    }
    el.children(menu.map(|menu| div().absolute().top_0().left(px(8. + indent)).child(menu)))
        .into_any_element()
}

/// The open menu of a tree row (components.md 5, Context menu): the row
/// menu, the status menu (`S`, then `1` to `6`) or the epics (`M`).
fn tree_menu(
    items: &[MenuItem],
    menu: TreeMenu,
    status: Option<Status>,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let mut panel =
        crate::menu::panel("tree-menu", t).on_mouse_down_out(cx.listener(|shell, _, _, cx| {
            shell.close_tree_menu(cx);
        }));
    for (i, item) in items.iter().enumerate() {
        let hint = |s: &str| Some(crate::menu::hint(s.to_owned(), t));
        let (leading, label, trailing): (Option<AnyElement>, String, Option<AnyElement>) =
            match item {
                MenuItem::Open => (None, text(locale, Key::CommonOpen), None),
                MenuItem::OpenInNewTab => (None, text(locale, Key::TabOpenNew), None),
                MenuItem::Rename => (None, text(locale, Key::TreeRename), hint("E")),
                MenuItem::StatusMenu => (None, text(locale, Key::TreeSetStatus), hint("S")),
                MenuItem::MoveMenu => (None, text(locale, Key::TreeMoveEpic), hint("M")),
                MenuItem::Status(s) => (
                    Some(ui::status_glyph(*s, 16., t).into_any_element()),
                    text(locale, ui::status_key(*s)),
                    if status == Some(*s) {
                        Some(crate::menu::check(t))
                    } else {
                        hint(&(i + 1).to_string())
                    },
                ),
                MenuItem::MoveTo(_, name) => (None, name.clone(), None),
                MenuItem::Delete => (None, text(locale, Key::CommonDelete), None),
            };
        let danger = *item == MenuItem::Delete;
        if danger {
            panel = panel.child(crate::menu::separator(t));
        }
        panel = panel.child(
            crate::menu::item(
                ("tree-menu-item", i),
                leading,
                label,
                trailing,
                i == menu.highlight,
                t,
            )
            .when(danger, |this| this.text_color(t.danger))
            .on_click(cx.listener(move |shell, _, window, cx| {
                cx.stop_propagation();
                shell.choose_menu_item(i, window, cx)
            })),
        );
    }
    deferred(
        gpui_kit::anchored()
            .offset(point(px(0.), px(28.)))
            .child(panel),
    )
    .with_priority(1)
    .into_any_element()
}

/// A 16 slot centring a smaller glyph, so rows with 14 and 16 icons keep
/// one title column.
fn icon_slot(glyph: impl IntoElement) -> gpui_kit::Div {
    div()
        .size(px(16.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph)
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
        // A resize drag that ended: one undo step, saved (4.8.6).
        let resizing = matches!(&self.content, Content::Workbench(b) if b.ui.resize.is_some());
        if resizing && !cx.has_active_drag() {
            self.resize_ended(window, cx);
        }
        let dragging = matches!(&self.content, Content::Workbench(b) if b.ui.dragging());
        let status_bar = is_bench.then(|| self.status_bar(cx));
        let overlay = self.overlay_element();

        let frame = type_scale::UI
            .el()
            .id("shell")
            .key_context("Workbench")
            .track_focus(&self.focus)
            .when(dragging, |this| this.key_context("Dragging"))
            // 4.8.7: `Esc` cancels a drag before it means anything else,
            // whatever has focus.
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" && s.cancel_drag(w, cx) {
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|s, _: &CancelDrag, w, cx| {
                s.cancel_drag(w, cx);
            }))
            .on_action(cx.listener(|s, _: &DismissSidebar, w, cx| {
                let over = s.bench().is_some_and(|b| {
                    b.ui.left_overlay && b.overlay.is_none() && b.ui.menu.is_none()
                });
                if over {
                    s.dismiss_sidebar(w, cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|s, _: &ToggleSidebar, w, cx| s.toggle_sidebar(w, cx)))
            .on_action(cx.listener(|s, _: &ResetLayout, w, cx| s.reset_layout(w, cx)))
            .on_action(cx.listener(|s, _: &ToggleRight, w, cx| s.toggle_area(Side::Right, w, cx)))
            .on_action(cx.listener(|s, _: &ToggleBottom, w, cx| s.toggle_area(Side::Bottom, w, cx)))
            .on_action(cx.listener(|s, _: &SplitRight, w, cx| s.split_page(Direction::Row, w, cx)))
            .on_action(cx.listener(|s, _: &NextArea, w, cx| s.cycle_area(true, w, cx)))
            .on_action(cx.listener(|s, _: &PreviousArea, w, cx| s.cycle_area(false, w, cx)))
            .on_action(cx.listener(|s, _: &ZoomArea, w, cx| s.toggle_zoom(w, cx)))
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
            .on_action(cx.listener(|s, _: &NextTab, w, cx| {
                let focus = !s.terminal_focused(w, cx);
                s.cycle_tab(true, focus, w, cx)
            }))
            .on_action(cx.listener(|s, _: &PreviousTab, w, cx| {
                let focus = !s.terminal_focused(w, cx);
                s.cycle_tab(false, focus, w, cx)
            }))
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
            .on_action(cx.listener(|s, _: &GoInbox, w, cx| s.toggle_view(ViewId::Inbox, w, cx)))
            .on_action(
                cx.listener(|s, _: &GoKnowledge, w, cx| s.toggle_view(ViewId::Knowledge, w, cx)),
            )
            .on_action(
                cx.listener(|s, _: &NewDocument, w, cx| s.run_command(Command::NewDocument, w, cx)),
            )
            .on_action(
                cx.listener(|s, _: &NewMemory, w, cx| s.run_command(Command::NewMemory, w, cx)),
            )
            .on_action(cx.listener(|s, _: &Back, w, cx| s.back(w, cx)))
            .on_action(cx.listener(|s, _: &Forward, w, cx| s.forward(w, cx)))
            .on_action(cx.listener(|s, _: &Undo, w, cx| s.undo(w, cx)))
            .on_action(cx.listener(|s, _: &Redo, w, cx| s.redo(w, cx)))
            .on_action(
                cx.listener(|s, _: &GoToHeading, w, cx| s.run_command(Command::GoHeading, w, cx)),
            )
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
                // `Mod+1`: Projects wherever it is, shown if hidden, the
                // cursor on the open page's row; in a narrow window the
                // left sidebar shows over the page.
                if s.is_workbench() {
                    let rows = s.rows(cx);
                    let current = s.tree_route(cx);
                    if let Some(bench) = s.bench()
                        && let Some(route) = current
                    {
                        bench.cursor.put(&rows, route);
                    }
                    s.view_action(ViewId::Projects, ViewAction::Show, w, cx);
                    let over = s.left_auto_hidden(w)
                        && s.bench().is_some_and(|b| {
                            b.layout
                                .locate(Tab::View(ViewId::Projects))
                                .map(|p| p.place)
                                == Some(Place::Side(Side::Left))
                        });
                    if over && let Some(bench) = s.bench() {
                        bench.ui.left_overlay = true;
                    }
                    cx.notify();
                }
            }))
            .on_drag_move(cx.listener(|s, e: &DragMoveEvent<ResizeDrag>, _, cx| {
                let handle = e.drag(cx).0;
                let p = e.event.position;
                layout_ui::on_resize_drag(s, handle, f32::from(p.x), f32::from(p.y), cx)
            }))
            .on_action(cx.listener(|s, _: &FocusPage, w, cx| s.focus_page(w, cx)))
            .on_action(cx.listener(|s, _: &FocusTerminal, w, cx| {
                if s.is_workbench() {
                    s.view_action(ViewId::Terminal, ViewAction::Show, w, cx)
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

    /// The action GPUI dispatches for `key` with focus in the innermost of
    /// `contexts` (outermost first), as the window resolves it.
    fn action_for(key: &'static str, contexts: &[&str]) -> Option<Box<dyn gpui_kit::Action>> {
        let keymap = gpui_kit::Keymap::new(key_bindings());
        let stack: Vec<gpui_kit::KeyContext> = contexts
            .iter()
            .map(|c| gpui_kit::KeyContext::parse(c).expect("a context"))
            .collect();
        let keystroke = gpui_kit::Keystroke::parse(key).expect("a keystroke");
        let (bindings, pending) = keymap.bindings_for_input(&[keystroke], &stack);
        assert!(!pending, "{key} waits for more keys");
        bindings.first().map(|b| b.action().boxed_clone())
    }

    fn is<A: gpui_kit::Action + Default>(action: Option<Box<dyn gpui_kit::Action>>) -> bool {
        action.is_some_and(|a| a.partial_eq(&A::default()))
    }

    #[test]
    fn alt_up_and_alt_down_reorder_in_the_tree() {
        let tree = ["Workbench", "Tree"];
        assert!(is::<TreeMoveUp>(action_for("alt-up", &tree)));
        assert!(is::<TreeMoveDown>(action_for("alt-down", &tree)));
        assert!(is::<TreeUp>(action_for("up", &tree)));
        // Outside the tree they mean nothing.
        assert!(action_for("alt-up", &["Workbench"]).is_none());
    }

    #[test]
    fn escape_dismisses_the_sidebar_over_the_page_except_in_a_terminal() {
        assert!(is::<DismissSidebar>(action_for("escape", &["Workbench"])));
        assert!(is::<DismissSidebar>(action_for(
            "escape",
            &["Workbench", "RecordPage"]
        )));
        assert!(action_for("escape", &["Workbench", "Terminal"]).is_none());
        assert!(is::<crate::dialogs::CancelDialog>(action_for(
            "escape",
            &["Workbench", "Dialog"]
        )));
    }
}
