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

use crate::zoom::{self, px};
use acta_core::model::{Author, EpicId, KbId, KbKind, Owner, ProjectId, Record, Status, TopicId};
use acta_core::store::ExportFile;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::window_border;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext, Bounds, Context, DragMoveEvent, Entity, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, KeyDownEvent,
    ParentElement, PathPromptOptions, Render, StatefulInteractiveElement, Styled, Subscription,
    Task, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions, actions,
    deferred, div, point, size,
};

#[cfg(windows)]
use crate::browser_dock::BrowserDock;
#[cfg(windows)]
use crate::browser_panel::BrowserEvent;
use crate::capture::{Capture, CaptureEvent};
#[cfg(windows)]
use crate::chat::Kept;
#[cfg(windows)]
use crate::chat_panel::{ChatEvent, ChatPanel};
use crate::claude_setup;
use crate::commands::{self, Command};
use crate::db::{self, Db, Opened};
use crate::dialogs::{Dialog, DialogEvent, Kind as DialogKind};
use crate::due_picker::{AnchorBounds, DuePicker, DuePickerEvent};
use crate::first_run::{FirstRun, FirstRunEvent};
use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::inbox_page::InboxPage;
use crate::kb_model::Filters;
use crate::kb_page::{DocPage, Draft, KnowledgePage};
use crate::layout::{Direction, Layout, Place, Side, Tab, TabId, ViewAction, ViewId};
use crate::menu::Align;
use crate::palette::{CreateKind, Palette, PaletteEvent, Setup};
use crate::palette_model::{Group, Target};
use crate::record_page::{EditTitle, OpenDuePicker, OpenStatusMenu, PageEvent, RecordPage};
use crate::route::{History, Route};
use crate::settings;
use crate::settings_page::SettingsPage;
use crate::terminal_dock::{DockEvent, Origin, TerminalDock};
use crate::theme::{self, RADIUS_LG, Tokens};
use crate::titlebar::{self, Crumb};
use crate::tools_model::{Holder, InstanceKey};
use crate::trash_page::TrashPage;
use crate::tree::{self, Folds, MenuItem, MenuKind, Row, RowKey};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{Toast, ToastKind, Workspace, WorkspaceEvent};

#[path = "shell_layout.rs"]
mod layout_ui;
#[path = "shell_tree.rs"]
mod tree_ui;

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

/// A toast about a failure stays until dismissed (components.md 5, Toast):
/// an error, and "Not enough room", which also carries the way out.
fn toast_stays(kind: ToastKind) -> bool {
    matches!(kind, ToastKind::Error | ToastKind::LayoutNoRoom)
}
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
        /// ``Ctrl+Shift+` ``: a new Claude Code session on the current page
        /// (layout.md 4.5.1).
        StartClaudeHere,
        /// `Ctrl+Alt+J` (`Cmd+Option+J`): Claude Code (no project).
        ToggleFreeClaude,
        /// `Ctrl+PageDown` / `Ctrl+PageUp` in a tool view: the next or
        /// previous instance.
        NextInstance,
        PreviousInstance,
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
        /// Settings: "Reset Acta..." (confirmed by typing a word).
        ResetData,
        /// "Open trash".
        OpenTrash,
        /// `Esc` in the sidebar shown over the page: dismiss it.
        DismissSidebar,
        /// `Mod+/`, palette "Keyboard shortcuts".
        OpenShortcuts,
        /// `Mod+=`, `Mod++`: zoom in (layout.md 4.9).
        ZoomIn,
        /// `Mod+-`: zoom out.
        ZoomOut,
        /// `Mod+0`: back to 100%.
        ZoomReset,
        /// `Mod+Shift+[` / `Mod+Shift+]`: collapse or expand every section
        /// of the active page (components-records.md 5, Section fold).
        CollapseSections,
        ExpandSections,
        /// Settings: "Remove from Claude Code" (confirmed, 6.4).
        RemoveClaudeSetup,
        /// Settings: "Set up Claude Code" (confirmed, with what it touches).
        SetUpClaude,
        /// Quit Acta (macOS: Cmd+Q and the app menu): asks first while an
        /// install runs (ADR 0013).
        QuitActa,
        /// `Mod+W`, palette "Close tab" (layout.md 4.4).
        CloseTab,
        /// `Ctrl+Tab` / `Ctrl+Shift+Tab`.
        NextTab,
        PreviousTab,
        /// `Mod+Enter` on a tree row: open it in a new tab.
        TreeOpenTab,
    ]
);

/// The page of a record that owns tools.
fn owner_route(owner: Owner) -> Route {
    match owner {
        Owner::Project(id) => Route::Project(id),
        Owner::Epic(id) => Route::Epic(id),
        Owner::Topic(id) => Route::Topic(id),
    }
}

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

/// Quit Acta, unless an install runs and has not been confirmed: then the
/// main window asks "Quit Acta?" first, whose Quit closes it (and the app
/// with its last window). Every app-level quit goes through here.
pub fn request_quit(cx: &mut App) {
    if let Some(tool) = crate::ready::must_ask_quit(cx) {
        for handle in cx.windows() {
            if let Some(shell) = handle.downcast::<Shell>() {
                let asked = shell.update(cx, |shell, window, cx| {
                    shell.open_dialog(DialogKind::QuitInstalling { tool }, window, cx)
                });
                if asked.is_ok() {
                    return;
                }
            }
        }
    }
    cx.quit();
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
        // The backtick matched by position on every platform (6.1).
        KeyBinding::new("ctrl-shift-`", StartClaudeHere, None),
        KeyBinding::new(m("cmd-alt-j", "ctrl-alt-j"), ToggleFreeClaude, None),
        // In a tool view only: passed through everywhere else (4.5.1).
        KeyBinding::new("ctrl-pagedown", NextInstance, Some("Terminal")),
        KeyBinding::new("ctrl-pageup", PreviousInstance, Some("Terminal")),
        KeyBinding::new("ctrl-pagedown", NextInstance, Some("ToolView")),
        KeyBinding::new("ctrl-pageup", PreviousInstance, Some("ToolView")),
        KeyBinding::new(m("cmd-1", "ctrl-1"), FocusSidebar, None),
        KeyBinding::new(m("cmd-2", "ctrl-2"), FocusPage, None),
        KeyBinding::new(m("cmd-3", "ctrl-3"), FocusTerminal, None),
        KeyBinding::new(m("cmd-shift-h", "ctrl-shift-h"), ToggleChat, None),
        KeyBinding::new("s", OpenStatusMenu, Some("RecordPage && !Input")),
        KeyBinding::new("d", OpenDuePicker, Some("RecordPage && !Input")),
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
    for (keys, key, context) in crate::zoom::bindings(mac) {
        out.push(match key {
            crate::zoom::ZoomKey::In => KeyBinding::new(keys, ZoomIn, context),
            crate::zoom::ZoomKey::Out => KeyBinding::new(keys, ZoomOut, context),
            crate::zoom::ZoomKey::Reset => KeyBinding::new(keys, ZoomReset, context),
        });
    }
    // The brackets by position (6.1): GPUI on Windows reports the shifted
    // key (`{`) with Shift consumed, macOS the bracket with Shift.
    for keys in [m("cmd-shift-[", "ctrl-shift-["), m("cmd-{", "ctrl-{")] {
        out.push(KeyBinding::new(keys, CollapseSections, app));
    }
    for keys in [m("cmd-shift-]", "ctrl-shift-]"), m("cmd-}", "ctrl-}")] {
        out.push(KeyBinding::new(keys, ExpandSections, app));
    }
    out.extend(crate::due_picker::bindings());
    out.extend(crate::record_page::questions::bindings());
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
    Questions(Entity<crate::questions_page::QuestionsPage>),
    Contacts(Entity<crate::contacts_page::ContactsPage>),
    Contact(Entity<crate::contact_page::ContactPage>),
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
            PageView::Questions(v) => v.clone().into(),
            PageView::Contacts(v) => v.clone().into(),
            PageView::Contact(v) => v.clone().into(),
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

/// The due date picker opened on a tree row (`D`, "Set due date...").
struct TreeDue {
    row: RowKey,
    owner: Owner,
    picker: Entity<DuePicker>,
    anchor: AnchorBounds,
    _sub: Subscription,
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
    /// The file viewer (components-attachments.md 5, Viewer).
    Viewer(Entity<crate::attach_ui::Viewer>),
    /// A file's menu: View, Save as..., Remove.
    FileMenu(Entity<crate::attach_ui::FileMenu>),
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
    /// The tree's rows, scroll and pending reveal (4.2.1).
    tree: tree_ui::TreeState,
    /// The tree's open menu (`Shift+F10`, `S`, `M`).
    tree_menu: Option<TreeMenu>,
    /// Where the tree's menu hangs: its row's title.
    tree_menu_anchor: crate::menu::AnchorBounds,
    /// The due date picker on a tree row (`D`).
    tree_due: Option<TreeDue>,
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
    /// confirmation, when accepted, restarts that session on the triage.
    pending_triage: Option<InstanceKey>,
    /// Restart or "Restart in ..." waiting for the restart confirmation:
    /// the instance (of the free dock when its holder is free).
    pending_restart: Option<InstanceKey>,
    recent: Vec<TopicId>,
    overlay: Option<Overlay>,
    /// Where focus returns when the overlay closes.
    return_focus: Option<FocusHandle>,
    capture_draft: String,
    /// Quick capture's files, kept with its draft when it closes.
    capture_tray: Option<Entity<crate::attach_ui::Tray>>,
    /// Strips, menus, the viewer and Save as (components-attachments.md 5).
    files: Entity<crate::attach_ui::Files>,
    /// The Questions page opened next starts on this segment (the
    /// Overview's late answers open it on Overdue).
    questions_filter: Option<crate::questions_model::Filter>,
    toast: Option<Toast>,
    toast_timer: Task<()>,
    dock: Entity<TerminalDock>,
    /// Claude Code (no project) (layout.md 4.5.1).
    free_dock: Entity<TerminalDock>,
    sessions: Entity<crate::sessions_view::SessionsView>,
    #[cfg(windows)]
    chat: Entity<ChatPanel>,
    #[cfg(windows)]
    browser: Entity<BrowserDock>,
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
    /// The zoom this window shows and its width in effective px, as of
    /// its last frame (layout.md 4.9).
    zoom_applied: u16,
    effective_width: f32,
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
        // An install has no cancel (ADR 0013): quitting while one runs
        // asks first.
        let me = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let Some(tool) = crate::ready::must_ask_quit(cx) else {
                return true;
            };
            let _ = me.update(cx, |shell, cx| {
                shell.open_dialog(DialogKind::QuitInstalling { tool }, window, cx)
            });
            false
        });
        // layout.md 4.6: the setup is checked again when the window
        // regains focus. A drag ends with the window's focus (4.8.7).
        let activation = cx.observe_window_activation(window, |shell, window, cx| {
            if !window.is_window_active() {
                shell.cancel_drag(window, cx);
            }
            if window.is_window_active()
                && let Content::Workbench(bench) = &shell.content
            {
                claude_setup::refresh_if_stale(cx);
                // The Ready rows only while their block is shown (10 s apart).
                if matches!(bench.page, PageView::Settings(_) | PageView::FirstRun(_)) {
                    crate::ready::check_if_stale(cx);
                }
                // ADR 0008, "Live files": focus is the fallback for a
                // change the folder monitor could not prove on its own.
                bench.ws.update(cx, |ws, cx| ws.check_folders_again(cx));
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
            zoom_applied: crate::zoom::DEFAULT,
            effective_width: 0.,
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
            cx.new(|cx| BrowserDock::new(db, window, cx))
        };
        // Session presence (components-app.md 5, Claude Code sessions), the
        // store for setup's folder links, and this build's plugin files
        // for the sessions the terminal starts (ADR 0008 amendment).
        crate::sessions::start(db.clone(), cx);
        // Dictation's chosen microphone is a local value (layout.md 4.7).
        crate::voice::set_db(db.clone(), cx);
        // Ready for Claude Code (ADR 0013): the Claude Code and sign-in
        // rows once at launch, for the status bar.
        crate::ready::init(db.clone(), cx);
        crate::ready::launch_check(cx);
        crate::sessions::watch_usage(cx);
        cx.background_executor()
            .spawn(async { claude_setup::write_plugin_files() })
            .detach();
        #[cfg(windows)]
        let chat_db = db.clone();
        let ws = cx.new(|cx| Workspace::new(db, cx));
        if db::RESET_FAILED.swap(false, std::sync::atomic::Ordering::Relaxed) {
            ws.update(cx, |ws, cx| {
                let message = text(settings::locale(cx), Key::ResetFailed);
                ws.toast(message, ToastKind::Error, cx);
            });
        }
        let files = cx.new(|_| crate::attach_ui::Files::new(ws.clone()));
        cx.set_global(crate::attach_ui::FilesHub(files.clone()));
        let dock = cx.new(|cx| TerminalDock::new(ws.clone(), false, cx));
        let free_dock = cx.new(|cx| TerminalDock::new(ws.clone(), true, cx));
        let sessions = cx.new(|cx| crate::sessions_view::SessionsView::new(ws.clone(), cx));
        #[cfg(windows)]
        let chat = cx.new(|cx| ChatPanel::new(chat_db, window, cx));
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
            cx.subscribe_in(&dock, window, Self::on_dock),
            cx.subscribe_in(&free_dock, window, Self::on_dock),
            cx.subscribe_in(&sessions, window, Self::on_sessions),
            cx.subscribe_in(&files, window, Self::on_files),
            cx.subscribe_in(
                &settings_page,
                window,
                |shell, _, request: &crate::ready::Request, window, cx| {
                    shell.on_ready(request.clone(), window, cx)
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
            tree: tree_ui::TreeState::default(),
            tree_menu: None,
            tree_menu_anchor: Default::default(),
            tree_due: None,
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
            pending_restart: None,
            recent: Vec::new(),
            overlay: None,
            return_focus: None,
            capture_draft: String::new(),
            capture_tray: None,
            files,
            questions_filter: None,
            toast: None,
            toast_timer: Task::ready(()),
            dock,
            free_dock,
            sessions,
            #[cfg(windows)]
            chat,
            #[cfg(windows)]
            browser,
            _subs: subs,
            _page_sub: Some(first_run_sub),
        };
        self.content = Content::Workbench(Box::new(bench));
        // layout.md 4.5.1, Resources: idle web views close after 10 minutes.
        #[cfg(windows)]
        cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(60))
                    .await;
                if shell
                    .update_in(cx, |shell, window, cx| {
                        shell.close_idle_web_views(window, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
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

    /// A file asks for the viewer or its menu: an overlay, with focus back
    /// to the file when it closes.
    fn on_files(
        &mut self,
        _: &Entity<crate::attach_ui::Files>,
        event: &crate::attach_ui::FilesEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::attach_ui::{FileMenu, FilesEvent, OverlayEvent, Viewer};
        let return_focus = window.focused(cx);
        let Some(bench) = self.bench() else { return };
        // A menu's View hands over to the viewer: focus still goes back to
        // the file the menu was opened on.
        let keep = match &bench.overlay {
            Some(Overlay::FileMenu(_)) => bench.return_focus.clone(),
            _ => None,
        };
        let (overlay, sub) = match event {
            FilesEvent::View(list, at) => {
                let (list, at) = (list.clone(), *at);
                let viewer = cx.new(|cx| Viewer::new(list, at, window, cx));
                let sub =
                    cx.subscribe_in(&viewer, window, |shell, _, _: &OverlayEvent, window, cx| {
                        shell.close_overlay(window, cx)
                    });
                (Overlay::Viewer(viewer), sub)
            }
            FilesEvent::Menu(target, at) => {
                let (target, at) = (target.clone(), *at);
                let menu = cx.new(|cx| FileMenu::new(target, at, window, cx));
                let sub = cx.subscribe_in(
                    &menu,
                    window,
                    |shell, menu, _: &OverlayEvent, window, cx| {
                        // A choice that opened the viewer replaced the menu.
                        let open = shell.bench().is_some_and(
                            |b| matches!(&b.overlay, Some(Overlay::FileMenu(m)) if m == menu),
                        );
                        if open {
                            shell.close_overlay(window, cx)
                        }
                    },
                );
                (Overlay::FileMenu(menu), sub)
            }
        };
        let Some(bench) = self.bench() else { return };
        bench.return_focus = keep.or(return_focus);
        bench._subs.push(sub);
        bench.overlay = Some(overlay);
        cx.notify();
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

    #[cfg(debug_assertions)]
    /// The memory-quality surfaces of `dev_overlay`.
    fn dev_kbq(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        use crate::kb_model::Chip;
        let project = self.bench().and_then(|b| b.active_project);
        let page = self.bench().map(|b| match &b.page {
            PageView::Doc(p) => (Some(p.clone()), None),
            PageView::Knowledge(p) => (None, Some(p.clone())),
            _ => (None, None),
        });
        let (doc, kb) = page.unwrap_or((None, None));
        match which {
            "kbq-to-check" | "kbq-to-check-all" | "kbq-inactive" | "kbq-to-check-verify" => {
                let Some(project) = project else { return };
                let (chip, kind) = match which {
                    "kbq-inactive" => (Chip::Inactive, None),
                    "kbq-to-check-all" => (Chip::ToCheck, None),
                    _ => (Chip::ToCheck, Some(KbKind::Memory)),
                };
                self.open_kb_chip(project, chip, kind, window, cx);
                if which == "kbq-to-check-verify" {
                    cx.spawn_in(window, async move |shell, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(1500))
                            .await;
                        let _ = shell.update_in(cx, |shell, _, cx| {
                            if let Some(bench) = shell.bench()
                                && let PageView::Knowledge(p) = &bench.page
                            {
                                p.update(cx, |p, cx| p.dev_verify_first(cx));
                            }
                        });
                    })
                    .detach();
                }
            }
            "kbq-unfold" => {
                if let Some(kb) = kb {
                    kb.update(cx, |p, cx| p.dev_unfold_first(cx));
                }
            }
            "kbq-history" => {
                if let Some(doc) = doc {
                    doc.update(cx, |p, cx| p.dev_history(cx));
                }
            }
            "kbq-merge" | "kbq-merge-undo" => {
                if let Some(doc) = doc {
                    doc.update(cx, |p, cx| p.dev_merge(cx));
                }
            }
            _ => {}
        }
    }

    #[cfg(debug_assertions)]
    /// The due date and Overview surfaces of `dev_overlay`.
    fn dev_due(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        let input = std::env::var("ACTA_DEV_DUE_INPUT").unwrap_or_default();
        match which {
            "due-tree" => {
                let rows = self.rows(cx);
                let route = self.tree_route(cx);
                if let Some(bench) = self.bench()
                    && !rows.is_empty()
                {
                    if let Some(route) = route {
                        bench.cursor.put(&rows, route);
                    }
                    let row = rows[bench.cursor.index(&rows)].clone();
                    self.open_tree_due(&row, window, cx);
                }
            }
            "due-row" => self.with_record(
                cx,
                |page, window, cx| page.dev_due_on_first_row(window, cx),
                window,
            ),
            "chip-blocked" | "chip-idle" | "chip-stale" => {
                use acta_core::store::OverviewFilter;
                let filter = match which {
                    "chip-idle" => OverviewFilter::Idle,
                    "chip-stale" => OverviewFilter::StaleMemories,
                    _ => OverviewFilter::Blocked,
                };
                self.with_record(cx, |page, _, cx| page.show_filter(filter, cx), window);
            }
            _ => {
                let enter = which == "due-picker-enter";
                self.with_record(
                    cx,
                    |page, window, cx| {
                        page.open_due_picker(false, window, cx);
                        page.dev_type_due(&input, enter, window, cx);
                    },
                    window,
                );
            }
        }
    }

    /// Debug builds, dictation (voice/dev.rs): once the page has loaded,
    /// `ACTA_DEV_VOICE=dialog-*` opens the first-use dialog in that state,
    /// `settings` opens Settings > Voice, and `fake` (or `fake-capture`,
    /// in the quick capture) starts a take in the last editor focused,
    /// fed by generated audio.
    #[cfg(debug_assertions)]
    fn dev_voice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(which) =
            crate::voice::get(cx).and_then(|d| d.read(cx).dev.which().map(str::to_owned))
        else {
            return;
        };
        if which == "fake-capture" {
            self.open_capture(window, cx);
        }
        cx.spawn_in(window, async move |shell, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2000))
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                if which.starts_with("dialog-") {
                    shell.open_dialog(DialogKind::VoiceSetup, window, cx);
                } else if which == "settings" {
                    shell.open_voice_settings(window, cx);
                } else if which.starts_with("fake")
                    && let Some(d) = crate::voice::get(cx)
                {
                    d.update(cx, |d, cx| d.dictate_target(window, cx));
                }
            });
        })
        .detach();
    }

    /// Debug builds' captures of drags and in-place editors (see the
    /// `tree-drag` arm of [`Shell::dev_overlay`]).
    #[cfg(debug_assertions)]
    fn dev_drag_and_pages(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        match which {
            "tree-drag" | "tree-drag-onto" => self.dev_tree_drag(which == "tree-drag-onto", cx),
            "qpage-edit" => {
                if let Some(bench) = self.bench()
                    && let PageView::Questions(page) = &bench.page
                {
                    let page = page.clone();
                    page.update(cx, |page, cx| page.dev_edit_first(window, cx));
                }
            }
            "inbox-filter" => {
                if let Some(bench) = self.bench()
                    && let PageView::Inbox(page) = &bench.page
                {
                    let page = page.clone();
                    page.update(cx, |page, cx| page.dev_open_filter(window, cx));
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// Debug builds: `ACTA_DEV_OVERLAY=palette|capture|terminal|chat|chat-palette|browser|browser-palette|export|snapshots`
    /// opens that surface at launch, so captures need no input.
    fn dev_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !cfg!(debug_assertions) {
            return;
        }
        // `ACTA_DEV_ROUTE=E-2` (a route key) opens that page first.
        if let Ok(key) = std::env::var("ACTA_DEV_ROUTE")
            && let Some(route) = Route::parse(&key)
        {
            self.navigate(route, true, window, cx);
        }
        #[cfg(debug_assertions)]
        self.dev_voice(window, cx);
        match std::env::var("ACTA_DEV_OVERLAY").as_deref() {
            Ok("palette") => {
                self.open_palette(None, None, window, cx);
                // `ACTA_DEV_QUERY`: typed into it (the ranking of 8.3).
                #[cfg(debug_assertions)]
                if let Ok(query) = std::env::var("ACTA_DEV_QUERY")
                    && let Some(bench) = self.bench()
                    && let Some(Overlay::Palette(palette)) = &bench.overlay
                {
                    palette.update(cx, |p, cx| p.dev_type(&query, window, cx));
                }
            }
            // Due dates and the Overview (components-records.md 5), once
            // the page has loaded: the picker, with `ACTA_DEV_DUE_INPUT`
            // typed into it (`due-picker-enter` also presses Enter); the
            // picker on the tree's cursor row; the table with a chip.
            #[cfg(debug_assertions)]
            Ok(
                which @ ("due-picker" | "due-picker-enter" | "due-tree" | "chip-blocked"
                | "chip-idle" | "chip-stale" | "due-row"),
            ) => {
                let which = which.to_owned();
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ =
                        shell.update_in(cx, |shell, window, cx| shell.dev_due(&which, window, cx));
                })
                .detach();
            }
            // Working folders (screens.md 8.11), once the page has loaded:
            // the list unfolded, the switcher open, the settings form of
            // the active folder, the branch selector.
            #[cfg(debug_assertions)]
            Ok(
                which @ ("folders-open" | "folders-switcher" | "folders-form" | "folders-branch"),
            ) => {
                let which = which.to_owned();
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.with_record(
                            cx,
                            |page, window, cx| page.dev_folders(&which, window, cx),
                            window,
                        )
                    });
                })
                .detach();
            }
            // The terminal header after the session's page changed: its
            // active folder ("Restart in ...") or the commit mode of the
            // folder the session runs in ("Restart here").
            #[cfg(debug_assertions)]
            Ok(which @ ("terminal-switch" | "terminal-commit")) => {
                let which = which.to_owned();
                self.view_action(ViewId::Terminal, ViewAction::Show, window, cx);
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(8000))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.with_record(
                            cx,
                            |page, window, cx| page.dev_folders(&which, window, cx),
                            window,
                        )
                    });
                })
                .detach();
            }
            // Ready for Claude Code (ADR 0013), with `ACTA_DEV_READY`: the
            // install confirmation of Git (`-update`: of Claude Code), the
            // quit confirmation during an install, and a one-off session
            // running `where git` (prints, starts nothing else).
            #[cfg(debug_assertions)]
            Ok(
                which @ ("ready-dialog"
                | "ready-dialog-update"
                | "ready-quit"
                | "ready-oneoff"
                | "ready-reveal"),
            ) => self.dev_ready(which, window, cx),
            Ok("capture") => self.open_capture(window, cx),
            Ok("terminal") => self.view_action(ViewId::Terminal, ViewAction::Show, window, cx),
            // Claude Code (no project), shown with nothing started.
            Ok("free-claude") => {
                self.change_layout(None, window, cx, |l| {
                    l.show_view(ViewId::FreeClaude).is_some()
                });
            }
            // Two browsers opened by the human on the current page, once it
            // has loaded: the instance strip (layout.md 4.5.1).
            #[cfg(windows)]
            Ok("browsers") => {
                self.change_layout(None, window, cx, |l| {
                    l.show_view(ViewId::AgentBrowser).is_some()
                });
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.sync_tool_owner(window, cx);
                        if let Some(bench) = shell.bench() {
                            let browser = bench.browser.clone();
                            browser.update(cx, |b, cx| {
                                b.open_new(window, cx);
                                b.open_new(window, cx);
                            });
                        }
                    });
                })
                .detach();
            }
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
            Ok("layout-menu") => {
                self.open_menu_under(LayoutMenu::Layout, "titlebar-layout", Align::End, cx)
            }
            // Menus hung from their buttons (components.md 5), once the
            // page has loaded: the status menu, the first entry's menu,
            // the Settings language menu.
            #[cfg(debug_assertions)]
            Ok(which @ ("status-menu" | "entry-menu" | "settings-language")) => {
                let which = which.to_owned();
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        if which == "settings-language" {
                            if let Some(bench) = shell.bench()
                                && let PageView::Settings(page) = &bench.page
                            {
                                page.update(cx, |page, cx| page.dev_open_language(cx));
                            }
                        } else {
                            shell.with_record(
                                cx,
                                |page, window, cx| page.dev_menu(&which, window, cx),
                                window,
                            )
                        }
                    });
                })
                .detach();
            }
            // "File with Claude Code" against a live Claude Code: the
            // terminal starts, then after 30 s the triage runs (typed into
            // an idle session), then after 60 s more `/exit`, if idle. The
            // terminal's screen goes to stderr at each step.
            #[cfg(debug_assertions)]
            Ok("triage-check") => self.dev_triage_check(window, cx),
            // The same against a pending permission question: the prompt
            // in `ACTA_DEV_CHECK_PROMPT` is typed first; the triage must
            // then find Claude Code busy and type nothing; the question is
            // declined with Esc, never approved, then `/exit`.
            #[cfg(debug_assertions)]
            Ok("permission-check") => self.dev_permission_check(window, cx),
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
            // Questions, Report to and contacts (screens.md 8.21 to 8.30),
            // once the page has loaded: the composer with the picker typed
            // into (`ACTA_DEV_QUERY`), the answer editor of the first
            // question, the Report to popover, the closed questions
            // unfolded, a contact's move refused.
            #[cfg(debug_assertions)]
            Ok(
                which @ ("q-composer" | "q-answer" | "q-closed" | "q-menu" | "q-scroll"
                | "report-popover" | "contact-move" | "q-drag" | "q-refused" | "blocks"),
            ) => {
                let which = which.to_owned();
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        if which == "contact-move" {
                            if let Some(bench) = shell.bench()
                                && let PageView::Contact(page) = &bench.page
                            {
                                let page = page.clone();
                                page.update(cx, |page, cx| {
                                    page.run(crate::commands::ContactCommand::Move, window, cx)
                                });
                            }
                        } else {
                            let query = std::env::var("ACTA_DEV_QUERY").unwrap_or_default();
                            shell.with_record(
                                cx,
                                |page, window, cx| page.dev_questions(&which, &query, window, cx),
                                window,
                            )
                        }
                    });
                })
                .detach();
            }
            // Dragging tree rows (components.md 5): a topic held over the
            // lower half of another topic's row, or over its epic's row;
            // the Questions page with its first open question edited in
            // place; the inbox's Report to filter open with its search.
            #[cfg(debug_assertions)]
            Ok(which @ ("tree-drag" | "tree-drag-onto" | "qpage-edit" | "inbox-filter")) => {
                let which = which.to_owned();
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.dev_drag_and_pages(&which, window, cx)
                    });
                })
                .detach();
            }
            // The palette's refusal: Enter on the highlighted (refused) row
            // of a picker opened by `contact-move`, after it loaded.
            #[cfg(debug_assertions)]
            Ok("contact-refused") => {
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        if let Some(bench) = shell.bench()
                            && let PageView::Contact(page) = &bench.page
                        {
                            let page = page.clone();
                            page.update(cx, |page, cx| {
                                page.run(crate::commands::ContactCommand::Move, window, cx)
                            });
                        }
                    });
                    cx.background_executor()
                        .timer(Duration::from_millis(1000))
                        .await;
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        let query = std::env::var("ACTA_DEV_QUERY").unwrap_or_default();
                        if let Some(bench) = shell.bench()
                            && let Some(Overlay::Palette(palette)) = &bench.overlay
                        {
                            palette.update(cx, |p, cx| p.dev_enter(&query, window, cx));
                        }
                    });
                })
                .detach();
            }
            Ok("export") => self.open_export(window, cx),
            Ok("snapshots") => self.open_snapshots(window, cx),
            // Memory quality (components-knowledge.md 5), once the page
            // has loaded: the knowledge page with a chip, an unfolded
            // memory, the history, a merge from the notice (and its undo).
            #[cfg(debug_assertions)]
            Ok(which) if which.starts_with("kbq-") => {
                let which = which.to_owned();
                cx.spawn_in(window, async move |shell, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1500))
                        .await;
                    let _ =
                        shell.update_in(cx, |shell, window, cx| shell.dev_kbq(&which, window, cx));
                    if which.ends_with("-undo") {
                        cx.background_executor()
                            .timer(Duration::from_millis(4000))
                            .await;
                        let _ = shell.update_in(cx, |shell, _, cx| {
                            if let Some(bench) = shell.bench() {
                                let (ws, project) = (bench.ws.clone(), bench.active_project);
                                ws.update(cx, |ws, cx| ws.undo_active(project, cx));
                            }
                        });
                        // Then `Esc`, as the person would: the editor
                        // closes on the text the store now holds.
                        cx.background_executor()
                            .timer(Duration::from_millis(1500))
                            .await;
                        let _ = shell.update_in(cx, |shell, window, cx| {
                            if let Some(bench) = shell.bench()
                                && let PageView::Doc(p) = &bench.page
                            {
                                p.update(cx, |p, cx| p.dev_cancel(window, cx));
                            }
                        });
                    }
                })
                .detach();
            }
            // Files (components-attachments.md 5): see `dev_attach`.
            #[cfg(debug_assertions)]
            Ok(which) if which.starts_with("attach-") => {
                let which = which.to_owned();
                self.dev_attach(&which, window, cx)
            }
            _ => {}
        }
    }

    /// Debug builds, `ACTA_DEV_OVERLAY=attach-*`, once the page has loaded:
    /// quick capture with a pasted image (the paste hook called with a
    /// clipboard holding only an image) and the files of `ACTA_DEV_FILES`
    /// (`;`-separated) dropped through GPUI's own file drop events
    /// (`attach-capture`; `attach-drop-hover` stops while they are over
    /// it); the viewer on the first or second image or on a text file;
    /// a file's menu; a note, a description or an entry being edited.
    /// The real clipboard and file manager are never used.
    #[cfg(debug_assertions)]
    fn dev_attach(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::{ExternalPaths, FileDropEvent, PlatformInput, point};
        let which = which.to_owned();
        let files: Vec<std::path::PathBuf> = std::env::var("ACTA_DEV_FILES")
            .unwrap_or_default()
            .split(';')
            .filter(|p| !p.is_empty())
            .map(std::path::PathBuf::from)
            .collect();
        if matches!(which.as_str(), "attach-capture" | "attach-drop-hover") {
            self.open_capture(window, cx);
        }
        cx.spawn_in(window, async move |shell, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            let capture = |shell: &mut Shell| match shell.bench().map(|b| b.overlay.as_ref()) {
                Some(Some(Overlay::Capture(c))) => Some(c.clone()),
                _ => None,
            };
            match which.as_str() {
                "attach-capture" | "attach-drop-hover" => {
                    if which == "attach-capture" {
                        let _ = shell.update_in(cx, |shell, window, cx| {
                            if let Some(c) = capture(shell) {
                                c.update(cx, |c, cx| c.dev_fill(window, cx));
                            }
                        });
                    }
                    // Over the capture's editor: 96 from the top, centred.
                    let _ = cx.update(|window, cx| {
                        let at = point(window.viewport_size().width / 2., px(150.));
                        let paths = ExternalPaths(files.into_iter().collect());
                        window.dispatch_event(
                            PlatformInput::FileDrop(FileDropEvent::Entered {
                                position: at,
                                paths,
                            }),
                            cx,
                        );
                        window.dispatch_event(
                            PlatformInput::FileDrop(FileDropEvent::Pending { position: at }),
                            cx,
                        );
                        if which == "attach-capture" {
                            window.dispatch_event(
                                PlatformInput::FileDrop(FileDropEvent::Submit { position: at }),
                                cx,
                            );
                        }
                    });
                }
                "attach-edit" | "attach-filed" => {
                    let filed = which == "attach-filed";
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        if let Some(bench) = shell.bench()
                            && let PageView::Inbox(page) = &bench.page
                        {
                            let page = page.clone();
                            page.update(cx, |p, cx| {
                                if filed {
                                    p.dev_show_filed(cx)
                                } else {
                                    p.dev_edit_first(window, cx)
                                }
                            });
                        }
                    });
                }
                "attach-settings" => {
                    let _ = shell.update_in(cx, |shell, _, cx| {
                        if let Some(bench) = shell.bench()
                            && let PageView::Settings(page) = &bench.page
                        {
                            let page = page.clone();
                            page.update(cx, |p, cx| p.dev_scroll_end(cx));
                        }
                    });
                }
                "attach-description" | "attach-entry" => {
                    let entry = which == "attach-entry";
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.with_record(
                            cx,
                            |page, window, cx| page.dev_files_editor(entry, window, cx),
                            window,
                        )
                    });
                }
                _ => {
                    let _ = shell.update_in(cx, |shell, _, cx| {
                        if let Some(bench) = shell.bench() {
                            let files = bench.files.clone();
                            files.update(cx, |f, cx| f.dev_open(&which, cx));
                        }
                    });
                }
            }
        })
        .detach();
    }

    fn show_first_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // First run shows the Ready block while something is missing.
        crate::ready::check_if_stale(cx);
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
                FirstRunEvent::Ready(request) => shell.on_ready(request.clone(), window, cx),
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
        if bench.ui.left_overlay || bench.ui.right_overlay {
            bench.ui.left_overlay = false;
            bench.ui.right_overlay = false;
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
            Route::Questions(scope) => {
                let filter = self
                    .bench()
                    .and_then(|b| b.questions_filter.take())
                    .unwrap_or_default();
                let page =
                    cx.new(|cx| crate::questions_page::QuestionsPage::new(ws, scope, filter, cx));
                let sub = Self::page_events(&page, tab, window, cx);
                (PageView::Questions(page), Some(sub))
            }
            Route::Contacts => {
                let active = self.bench().and_then(|b| b.active_project);
                let page =
                    cx.new(|cx| crate::contacts_page::ContactsPage::new(ws, active, window, cx));
                let sub = Self::page_events(&page, tab, window, cx);
                (PageView::Contacts(page), Some(sub))
            }
            Route::Contact(id) => {
                let page = cx.new(|cx| crate::contact_page::ContactPage::new(ws, id, cx));
                let sub = Self::page_events(&page, tab, window, cx);
                (PageView::Contact(page), Some(sub))
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
        // Settings opens: every Ready row is checked (ADR 0013).
        if route == Route::Settings {
            crate::ready::check_if_stale(cx);
        }
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
        bench.ui.right_overlay = false;
        let projects = bench.ws.read(cx).projects.clone();
        let project = match route {
            Route::Project(id) | Route::Inbox(id) | Route::Knowledge(id) => Some(id),
            Route::Topic(id) => {
                tree::locate_topic(&projects, id).and_then(|l| l.project.map(|p| p.0))
            }
            Route::Epic(id) => {
                tree::locate_epic(&projects, id).and_then(|l| l.project.map(|p| p.0))
            }
            Route::Questions(owner) => bench.ws.read(cx).project_of(owner),
            Route::Kb(_)
            | Route::Trash
            | Route::Settings
            | Route::Shortcuts
            | Route::Contacts
            | Route::Contact(_) => None,
        };
        if let Some(project) = project {
            bench.active_project = Some(project);
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
        self.sync_tool_owner(window, cx);
        // Opening a page by any route reveals its row (layout.md 4.2.1).
        self.reveal_in_tree(false, cx);
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
            && matches!(
                event,
                PageEvent::Open(_) | PageEvent::NewKb(_) | PageEvent::OpenEntry(..)
            )
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
                // The breadcrumb and the tree now know the entry's owner,
                // whose row the tree reveals.
                if current == Some(tab) {
                    self.reveal_in_tree(false, cx);
                }
                cx.notify();
            }
            PageEvent::ShowTerminal(owner) => self.show_page_sessions(*owner, window, cx),
            PageEvent::Confirm(confirm) => {
                self.open_dialog(DialogKind::Confirm(confirm.clone()), window, cx)
            }
            PageEvent::KbChip(project, chip, kind) => {
                self.open_kb_chip(*project, *chip, *kind, window, cx)
            }
            PageEvent::EditKb(id) => self.edit_kb(*id, window, cx),
            PageEvent::LateAnswers(scope) => self.open_late_answers(*scope, window, cx),
            PageEvent::OpenEntry(topic, entry) => self.open_entry(*topic, *entry, window, cx),
        }
    }

    /// "Go to contact...": the palette lists the contacts the active
    /// project sees, nearest level first, with their details; `Enter`
    /// opens the contact page.
    fn pick_contact(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let Some(project) = bench.active_project else {
            return;
        };
        let ws = bench.ws.clone();
        let place = acta_core::model::Place {
            project,
            epic: None,
        };
        let task = ws
            .read(cx)
            .db
            .run(cx, move |store| store.contacts_visible(place));
        cx.spawn_in(window, async move |shell, cx| {
            let Ok(list) = task.await else { return };
            let _ = shell.update_in(cx, |shell, window, cx| {
                let locale = settings::locale(cx);
                let projects = ws.read(cx).projects.clone();
                let choices = list
                    .iter()
                    .map(|c| crate::palette::Choice {
                        title: c.contact.name.clone(),
                        detail: [
                            crate::contacts_model::details(&c.contact),
                            crate::contacts_model::level_name(locale, c.contact.level, &projects),
                        ]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" \u{b7} "),
                        lead: crate::palette::Lead::Icon(Icon::User),
                        trailing: String::new(),
                        refusal: None,
                    })
                    .collect();
                let ids: Vec<acta_core::model::ContactId> =
                    list.iter().map(|c| c.contact.id).collect();
                let me = cx.entity().downgrade();
                let on_pick = crate::palette::OnPick::new(move |i, window, cx| {
                    if let Some(id) = ids.get(i).copied() {
                        let _ = me.update(cx, |shell, cx| {
                            shell.navigate(Route::Contact(id), true, window, cx)
                        });
                    }
                });
                shell.open_picker(
                    crate::palette::Picker {
                        label: text(locale, Key::CmdContactGo),
                        choices,
                    },
                    on_pick,
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    /// The Overview's late answers: the Questions page with Overdue.
    /// "Logged as a decision": the topic, with that entry focused.
    pub(super) fn open_entry(
        &mut self,
        topic: TopicId,
        entry: acta_core::model::EntryId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(Route::Topic(topic), true, window, cx);
        if let Some(bench) = self.bench()
            && let PageView::Record(page) = &bench.page
            && page.read(cx).topic() == Some(topic)
        {
            let page = page.clone();
            page.update(cx, |page, cx| page.focus_entry(entry, window, cx));
        }
    }

    pub(super) fn open_late_answers(
        &mut self,
        scope: Owner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bench) = self.bench() {
            bench.questions_filter = Some(crate::questions_model::Filter::Overdue);
        }
        self.navigate(Route::Questions(scope), true, window, cx);
    }

    /// The knowledge page with a chip (components-knowledge.md 5): "To
    /// check" from the Overview's memory line (Kind: Memories) or the
    /// palette (every kind), "Superseded and retired" from the palette.
    pub(super) fn open_kb_chip(
        &mut self,
        project: ProjectId,
        chip: crate::kb_model::Chip,
        kind: Option<KbKind>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bench) = self.bench() {
            bench.history.open(Route::Knowledge(project));
        }
        let Some(bench) = self.bench() else { return };
        let (ws, tab) = (bench.ws.clone(), bench.current);
        let page = cx.new(|cx| KnowledgePage::new_chip(ws, project, chip, kind, window, cx));
        let sub = Self::page_events(&page, tab, window, cx);
        if let Some(bench) = self.bench() {
            bench.page = PageView::Knowledge(page);
            bench._page_sub = Some(sub);
        }
        cx.notify();
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

    /// "Voice settings": Settings, scrolled to Voice.
    fn open_voice_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings(window, cx);
        if let Some(bench) = self.bench() {
            bench.settings.update(cx, |page, cx| page.reveal_voice(cx));
        }
    }

    /// The first-use dialog of dictation; a download that ended before is
    /// not shown again.
    fn open_voice_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = crate::voice::get(cx) {
            d.update(cx, |d, _| {
                if d.download.is_none() {
                    d.finished = None;
                }
            });
        }
        self.open_dialog(DialogKind::VoiceSetup, window, cx);
    }

    /// "Delete Whisper Large turbo?" (6.4): deleting is undone only by
    /// downloading again.
    fn confirm_delete_model(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = crate::voice::get(cx) else {
            return;
        };
        let Some(model) = d.update(cx, |d, _| d.to_delete.take()) else {
            return;
        };
        let locale = settings::locale(cx);
        let name = crate::voice::view::model_name(model);
        let size = crate::voice::view::size(locale, model.file().size);
        let confirm = crate::dialogs::Confirm {
            title: crate::i18n::tr(
                locale,
                Key::VoiceModelDeleteTitle,
                &[("model", crate::i18n::Arg::Text(name))],
            ),
            body: crate::i18n::tr(
                locale,
                Key::VoiceModelDeleteBody,
                &[("size", crate::i18n::Arg::Text(&size))],
            ),
            confirm: crate::i18n::text(locale, Key::CommonDelete),
            on_confirm: crate::palette::OnPick::new(move |_, _, cx| {
                d.update(cx, |d, cx| d.delete_model(model, cx));
            }),
        };
        self.open_dialog(DialogKind::Confirm(confirm), window, cx);
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
        // The Questions sections' folds share the list (record pages write
        // them): they are kept, never dropped by the tree's.
        closed.extend(
            settings::current(cx)
                .closed_shown
                .iter()
                .filter(|k| k.starts_with("questions:"))
                .cloned(),
        );
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
            Route::Questions(_) => (
                Icon::CircleQuestionMark,
                None,
                text(locale, Key::SectionQuestions),
            ),
            Route::Contacts => (Icon::Users, None, text(locale, Key::ContactsTitle)),
            Route::Contact(id) => {
                let name = |view: &PageView| match view {
                    PageView::Contact(page) => page.read(cx).title(),
                    _ => None,
                };
                let title = if bench.history.current() == Some(route) {
                    name(&bench.page)
                } else {
                    bench
                        .slots
                        .values()
                        .find(|s| s.history.current() == Some(route))
                        .and_then(|s| s.view.as_ref())
                        .and_then(name)
                };
                (Icon::User, None, title.unwrap_or_else(|| id.to_string()))
            }
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
    /// The record that owns the current page's tools, with its project and
    /// title (layout.md 4.5.1): `None` on a page that owns nothing.
    fn origin(&mut self, cx: &mut Context<Self>) -> Option<Origin> {
        let bench = self.bench()?;
        let route = bench.history.current()?;
        let kb_owner = match (&bench.page, route) {
            (PageView::Doc(page), Route::Kb(_)) => page.read(cx).located().map(|(o, _)| o),
            _ => None,
        };
        let owner = crate::tools_model::owner_of(route, kb_owner)?;
        let ws = bench.ws.read(cx);
        let projects = &ws.projects;
        let (project, name) = match owner {
            Owner::Project(id) => (id, tree::locate_project(projects, id)?.project?.1),
            Owner::Epic(id) => {
                let l = tree::locate_epic(projects, id)?;
                (l.project?.0, l.epic?.1)
            }
            Owner::Topic(id) => {
                let l = tree::locate_topic(projects, id)?;
                (l.project?.0, l.topic?.1)
            }
        };
        Some(Origin {
            owner,
            project,
            name,
        })
    }

    /// The Terminal view follows the current page (layout.md 4.5.1): set
    /// on navigation and each frame (a document's owner arrives after it
    /// loads); a no-op while nothing changed.
    fn sync_tool_owner(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let origin = self.origin(cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        if dock.read(cx).current() != origin.as_ref() {
            dock.update(cx, |dock, cx| dock.set_current(origin.clone(), cx));
        }
        #[cfg(windows)]
        {
            let chat = bench.chat.clone();
            chat.update(cx, |chat, cx| chat.set_current(origin.clone(), window, cx));
            let browser = bench.browser.clone();
            browser.update(cx, |b, cx| b.set_current(origin, window, cx));
        }
        #[cfg(not(windows))]
        let _ = window;
    }

    /// The 10-minute and 6-view rules over every web view, chats and
    /// browsers together (layout.md 4.5.1, Resources): run each minute and
    /// whenever a web view opens.
    #[cfg(windows)]
    fn close_idle_web_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::tools_model::{WebUse, web_to_close};
        #[derive(Clone, Copy)]
        enum Web {
            Chat(InstanceKey),
            Browser(InstanceKey),
        }
        let Some(bench) = self.bench() else { return };
        let (chat, browser) = (bench.chat.clone(), bench.browser.clone());
        let tag = |f: fn(InstanceKey) -> Web, u: WebUse<InstanceKey>| WebUse {
            key: f(u.key),
            open: u.open,
            on_screen: u.on_screen,
            last_used: u.last_used,
            busy: u.busy,
        };
        let uses: Vec<WebUse<Web>> = chat
            .read(cx)
            .web_uses()
            .into_iter()
            .map(|u| tag(Web::Chat, u))
            .chain(
                browser
                    .read(cx)
                    .web_uses(cx)
                    .into_iter()
                    .map(|u| tag(Web::Browser, u)),
            )
            .collect();
        for web in web_to_close(&uses, std::time::Instant::now()) {
            match web {
                Web::Chat(key) => chat.update(cx, |c, cx| c.close_web(key, cx)),
                Web::Browser(key) => browser.update(cx, |b, cx| b.close_web(key, window, cx)),
            }
        }
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
        let mut context = self.command_context(window, cx);
        context.due = self.due_target(window, cx).map(|(_, _, due)| due.is_some());
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
        let tools = {
            let dock = bench.dock.read(cx);
            let free = bench.free_dock.read(cx);
            commands::ToolCommands {
                claude_start: dock.current().is_some() && !dock.at_limit(),
                claude_restart: dock.shown().is_some(),
                free_new: !free.at_limit(),
                #[cfg(windows)]
                browser_devtools: bench.layout.is_visible(ViewId::AgentBrowser)
                    && bench.browser.read(cx).has_page(cx),
                #[cfg(not(windows))]
                browser_devtools: false,
            }
        };
        commands::Context {
            tools,
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
            due: None,
            overview: match &bench.page {
                PageView::Record(page) => page.read(cx).overview_state(cx),
                _ => None,
            },
            folds: match &bench.page {
                PageView::Record(page) => page.read(cx).fold_commands(cx),
                PageView::Contact(page) => page.read(cx).fold_commands(cx),
                _ => (false, false),
            },
            folders: match &bench.page {
                PageView::Record(page) => page.read(cx).folder_commands(),
                _ => commands::FolderCommands::default(),
            },
            report: match &bench.page {
                PageView::Record(page) => page.read(cx).report_context(),
                _ => None,
            },
            question: match &bench.page {
                PageView::Record(page) => page.read(cx).question_context(window),
                _ => None,
            },
            folder: match &bench.page {
                PageView::Record(page) => page.read(cx).focused_folder_commands(window),
                _ => None,
            },
            attach: crate::attach_ui::target_tray(cx).is_some(),
            attachment: crate::attach_ui::focused_file(window, cx).is_some(),
            on_contact: matches!(bench.page, PageView::Contact(_)),
            dictate: crate::voice::get(cx).is_some_and(|d| {
                let d = d.read(cx);
                d.target()
                    .is_some_and(|e| d.phase(e, cx) != crate::voice::view::Phase::Off)
            }),
            dictating: crate::voice::get(cx).is_some_and(|d| d.read(cx).running()),
            zoom: self.zoom_applied,
            zoom_max: self.zoom_max(window, cx),
            trash: match &bench.page {
                PageView::Trash(page) => Some(page.read(cx).command_state()),
                _ => None,
            },
            kb: self.kb_target(window, cx),
            kb_scanning: bench.ws.read(cx).kb_scanning,
        }
    }

    /// The knowledge entry the memory-quality commands act on: the focused
    /// row, else the open entry (interaction.md 6.1).
    fn kb_target(&self, window: &Window, cx: &App) -> Option<commands::FocusedKb> {
        let Content::Workbench(bench) = &self.content else {
            return None;
        };
        match &bench.page {
            PageView::Knowledge(page) => page.read(cx).kb_context(window),
            PageView::Record(page) => page.read(cx).kb_context(window),
            PageView::Doc(page) => page.read(cx).kb_context(),
            _ => None,
        }
    }

    /// Run a memory-quality command on [`Self::kb_target`].
    fn kb_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let Some(k) = self.kb_target(window, cx) else {
            return;
        };
        let Some(bench) = self.bench() else { return };
        let ws = bench.ws.clone();
        let page = match &bench.page {
            PageView::Knowledge(p) => Some(PageView::Knowledge(p.clone())),
            PageView::Record(p) => Some(PageView::Record(p.clone())),
            PageView::Doc(p) => Some(PageView::Doc(p.clone())),
            _ => None,
        };
        let owner = Some(k.owner);
        let kind = if k.memory {
            KbKind::Memory
        } else {
            KbKind::Document
        };
        match command {
            // The page hides the mark at once; the write follows.
            Command::KbVerify => match page {
                Some(PageView::Knowledge(p)) => p.update(cx, |p, cx| p.verify_row(k.id, cx)),
                Some(PageView::Record(p)) => p.update(cx, |p, cx| p.verify_kb(k.id, cx)),
                Some(PageView::Doc(p)) => p.update(cx, |p, cx| p.verify(cx)),
                _ => crate::kbq_ui::verify(&ws, k.id, owner, cx),
            },
            Command::KbRetire => crate::kbq_ui::retire(&ws, k.id, Some(k.rev), kind, owner, cx),
            Command::KbMakeLive => crate::kbq_ui::make_live(&ws, k.id, owner, cx),
            Command::KbKeepBoth(a, b) => {
                let pair = if k.conflict == Some(b) {
                    acta_core::model::KbPairKind::Conflict
                } else {
                    acta_core::model::KbPairKind::Duplicate
                };
                crate::kbq_ui::keep_both(&ws, a, b, pair, owner, cx)
            }
            Command::KbReplace(old) => crate::kbq_ui::replace(&ws, k.id, old, owner, cx),
            Command::KbMerge => {
                let task = crate::kbq_ui::merge_candidates(&ws, k.id, cx);
                let locale = settings::locale(cx);
                cx.spawn_in(window, async move |shell, cx| {
                    let Ok(candidates) = task.await else {
                        return;
                    };
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        let weak = cx.entity().downgrade();
                        let on_merged: crate::kbq_ui::OnMerged =
                            std::rc::Rc::new(move |id, window, cx| {
                                let _ = weak.update(cx, |s, cx| s.edit_kb(id, window, cx));
                            });
                        let (picker, on_pick) = crate::kbq_ui::merge_picker(
                            locale, &ws, k.id, owner, candidates, on_merged,
                        );
                        shell.open_picker(picker, on_pick, window, cx);
                    });
                })
                .detach();
            }
            _ => {}
        }
    }

    /// A merge's survivor opens in its editor so the person can tidy the
    /// joined text (components-knowledge.md 5, Merge).
    pub(super) fn edit_kb(&mut self, id: KbId, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Route::Kb(id), true, window, cx);
        if let Some(bench) = self.bench()
            && let PageView::Doc(page) = &bench.page
            && page.read(cx).id == Some(id)
        {
            page.update(cx, |page, cx| page.edit_when_loaded(cx));
        }
    }

    /// The largest step the window's monitor work area holds (4.9).
    fn zoom_max(&self, window: &Window, cx: &App) -> u16 {
        match window.display(cx) {
            Some(display) => {
                let area = display.visible_bounds().size;
                zoom::largest_fitting(f32::from(area.width), f32::from(area.height))
            }
            None => zoom::STEPS[zoom::STEPS.len() - 1],
        }
    }

    /// `Mod+=`: the next step the work area holds, growing the window to
    /// its minimum at that step; nothing at the largest (4.9).
    pub fn zoom_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (w, h) = match window.display(cx) {
            Some(display) => {
                let area = display.visible_bounds().size;
                (f32::from(area.width), f32::from(area.height))
            }
            None => (f32::MAX, f32::MAX),
        };
        if let Some(step) = zoom::zoom_in(self.zoom_applied.max(zoom::STEPS[0]), w, h) {
            self.zoom_to(step, window, cx);
        }
    }

    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        if let Some(step) = zoom::zoom_out(self.zoom_applied.max(zoom::STEPS[0])) {
            settings::set_zoom(step, cx);
        }
    }

    /// Choose `step`, growing the window when it is under that step's
    /// minimum (never past its monitor's work area).
    pub fn zoom_to(&mut self, step: u16, window: &mut Window, cx: &mut Context<Self>) {
        let viewport = window.viewport_size();
        let size = (f32::from(viewport.width), f32::from(viewport.height));
        let work = window
            .display(cx)
            .map(|d| d.visible_bounds().size)
            .map_or(size, |a| (f32::from(a.width), f32::from(a.height)));
        if !window.is_maximized()
            && !window.is_fullscreen()
            && let Some((w, h)) = zoom::grown(step, size, work)
        {
            window.resize(gpui_kit::size(gpui_kit::px(w), gpui_kit::px(h)));
        }
        settings::set_zoom(step, cx);
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
                    Target::Epic(id) => Route::Epic(id),
                    Target::Kb(id, _) => Route::Kb(id),
                    Target::Command(command) => return self.run_command(command, window, cx),
                    Target::Choice(_) => return,
                    Target::Found(route, _) => route,
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
                    // "New contact" from the palette: at the active
                    // project's level (components-contacts.md 5).
                    CreateKind::Contact => {
                        let id = store.create_contact(
                            &title,
                            acta_core::model::ContactLevel::Project(project),
                            Author::Human,
                        )?;
                        (Record::Contact(id), Route::Contact(id))
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
            Command::TreeCollapseAll => self.collapse_all(cx),
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
            Command::SetDue => match self.due_target(window, cx) {
                Some((true, _, _)) => {
                    let rows = self.rows(cx);
                    if let Some(bench) = self.bench() {
                        let row = rows[bench.cursor.index(&rows)].clone();
                        self.open_tree_due(&row, window, cx);
                    }
                }
                Some((false, _, _)) => self.with_record(
                    cx,
                    |page, window, cx| page.open_due_here(true, window, cx),
                    window,
                ),
                None => {}
            },
            Command::ClearDue => {
                if let Some((_, owner, _)) = self.due_target(window, cx)
                    && let Some(bench) = self.bench()
                {
                    bench.ws.update(cx, |ws, cx| ws.set_due(owner, None, cx));
                }
            }
            // "Show knowledge to check": every kind (the Overview's line
            // adds Kind: Memories).
            Command::Show(acta_core::store::OverviewFilter::StaleMemories) => {
                if let Some(project) = self.bench().and_then(|b| b.active_project) {
                    self.open_kb_chip(project, crate::kb_model::Chip::ToCheck, None, window, cx);
                }
            }
            Command::Show(filter) => self.show_overview_list(filter, window, cx),
            Command::ShowInactive => {
                if let Some(project) = self.bench().and_then(|b| b.active_project) {
                    self.open_kb_chip(project, crate::kb_model::Chip::Inactive, None, window, cx);
                }
            }
            Command::KbFindDuplicates => {
                if let Some(bench) = self.bench()
                    && let Some(project) = bench.active_project
                {
                    let ws = bench.ws.clone();
                    crate::kbq_ui::find_duplicates(&ws, project, cx);
                }
            }
            Command::KbVerify
            | Command::KbRetire
            | Command::KbMakeLive
            | Command::KbMerge
            | Command::KbKeepBoth(..)
            | Command::KbReplace(_) => self.kb_command(command, window, cx),
            Command::QuestionNew => {
                self.with_record(cx, |page, window, cx| page.new_question(window, cx), window)
            }
            Command::QuestionsOpen => {
                let scope = match self.bench().and_then(|b| b.history.current()) {
                    Some(Route::Epic(e)) => Some(Owner::Epic(e)),
                    Some(Route::Topic(t)) => Some(Owner::Topic(t)),
                    _ => self
                        .bench()
                        .and_then(|b| b.active_project)
                        .map(Owner::Project),
                };
                if let Some(scope) = scope {
                    self.navigate(Route::Questions(scope), true, window, cx);
                }
            }
            Command::ShowQuestionsLate => {
                let scope = match self.bench().and_then(|b| b.history.current()) {
                    Some(Route::Epic(e)) => Some(Owner::Epic(e)),
                    _ => self
                        .bench()
                        .and_then(|b| b.active_project)
                        .map(Owner::Project),
                };
                if let Some(scope) = scope {
                    self.open_late_answers(scope, window, cx);
                }
            }
            Command::ReportSet => self.with_record(
                cx,
                |page, window, cx| page.open_report_to(window, cx),
                window,
            ),
            Command::ReportClear => self.with_record(
                cx,
                |page, _, cx| page.set_report(crate::contact_picker::Value::None, cx),
                window,
            ),
            Command::Reported | Command::Unreported => {
                let on = command == Command::Reported;
                self.with_record(cx, |page, _, cx| page.set_reported(on, cx), window)
            }
            Command::Question(q) => self.with_record(
                cx,
                |page, window, cx| page.run_question(q, window, cx),
                window,
            ),
            Command::Folder(f) => self.with_record(
                cx,
                |page, window, cx| page.run_folder_command(f, window, cx),
                window,
            ),
            Command::Attach => crate::attach_ui::attach_to_target(window, cx),
            Command::AttachmentSave => crate::attach_ui::save_focused(window, cx),
            Command::ContactsOpen => self.navigate(Route::Contacts, true, window, cx),
            Command::ContactGo => self.pick_contact(window, cx),
            Command::ContactNew => self.open_palette(None, Some(CreateKind::Contact), window, cx),
            Command::Contact(c) => {
                if let Some(bench) = self.bench()
                    && let PageView::Contact(page) = &bench.page
                {
                    let page = page.clone();
                    page.update(cx, |page, cx| page.run(c, window, cx));
                }
            }
            Command::ProfileName | Command::ProfileJobTitle => {
                let name = command == Command::ProfileName;
                self.open_settings(window, cx);
                if let Some(bench) = self.bench() {
                    let settings = bench.settings.clone();
                    settings.update(cx, |page, cx| page.focus_profile(name, window, cx));
                }
            }
            Command::SectionsCollapse => self.fold_all(true, window, cx),
            Command::SectionsExpand => self.fold_all(false, window, cx),
            Command::OverviewMore | Command::OverviewLess => {
                let open = command == Command::OverviewMore;
                self.with_record(cx, |page, _, cx| page.set_overview_open(open, cx), window)
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
            Command::Dictate => {
                if let Some(d) = crate::voice::get(cx) {
                    d.update(cx, |d, cx| d.dictate_target(window, cx));
                }
            }
            Command::DictateStop => {
                if let Some(d) = crate::voice::get(cx) {
                    d.update(cx, |d, cx| {
                        d.request_stop(acta_core::voice::take::StopReason::Asked, cx)
                    });
                }
            }
            Command::VoiceSettings => self.open_voice_settings(window, cx),
            Command::ChatBrowser => cx.open_url(CLAUDE_HOME),
            #[cfg(windows)]
            Command::ChatSignOut => self.open_dialog(DialogKind::ChatSignOut, window, cx),
            Command::ClaudeRestart => self.restart_shown(window, cx),
            Command::ClaudeStart => self.start_claude_here(window, cx),
            Command::FreeNew => self.new_free_claude(window, cx),
            #[cfg(windows)]
            Command::BrowserDevtools => self.open_browser_devtools(window, cx),
            Command::TerminalLarger => self.terminal_size(1., cx),
            Command::TerminalSmaller => self.terminal_size(-1., cx),
            Command::SetFolder => self.with_record(cx, |page, _, cx| page.set_folder(cx), window),
            Command::AddFolder => self.with_record(cx, |page, _, cx| page.add_folder(cx), window),
            Command::ShowFolders => {
                self.with_record(cx, |page, _, cx| page.show_folders(cx), window)
            }
            Command::FolderSwitch => {
                self.with_record(cx, |page, _, cx| page.pick_active_folder(cx), window)
            }
            Command::CommitMode => {
                self.with_record(cx, |page, _, cx| page.pick_commit_mode(cx), window)
            }
            Command::FolderSettings => self.with_record(
                cx,
                |page, window, cx| {
                    if let Some(id) = page.settings_target() {
                        page.open_folder_settings(id, window, cx)
                    }
                },
                window,
            ),
            Command::FolderPull => self.with_record(
                cx,
                |page, _, cx| {
                    if let Some(id) = page.git_target() {
                        page.pull(id, cx)
                    }
                },
                window,
            ),
            Command::FolderFetch => self.with_record(
                cx,
                |page, _, cx| {
                    if let Some(id) = page.git_target() {
                        page.fetch(id, cx)
                    }
                },
                window,
            ),
            Command::FolderBranch => self.with_record(
                cx,
                |page, _, cx| {
                    if let Some(id) = page.git_target() {
                        page.show_folders(cx);
                        page.open_branch_menu(id, cx)
                    }
                },
                window,
            ),
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
            Command::ZoomIn => self.zoom_in(window, cx),
            Command::ZoomOut => self.zoom_out(cx),
            Command::ZoomReset => settings::set_zoom(zoom::DEFAULT, cx),
            Command::ZoomTo(step) => self.zoom_to(step, window, cx),
            Command::TrashRestore | Command::TrashDelete | Command::TrashEmpty => {
                let page = match self.bench().map(|b| &b.page) {
                    Some(PageView::Trash(page)) => Some(page.clone()),
                    _ => None,
                };
                if let Some(page) = page {
                    page.update(cx, |page, cx| page.command(command, window, cx));
                }
            }
            Command::ReadingWidth(width) => settings::set_reading_width(width, cx),
            Command::Language(locale) => settings::set_language(locale, cx),
            Command::Claude(setting, on) => {
                use crate::commands::ClaudeSetting;
                settings::change(cx, |p| match setting {
                    ClaudeSetting::Resume => p.new_conversations = !on,
                    ClaudeSetting::Flash => p.flash_waiting = on,
                    ClaudeSetting::Usage => p.hide_usage = !on,
                });
                cx.refresh_windows();
            }
        }
    }

    /// A palette command of the Overview: on the open project or epic page,
    /// else on the active project's page, opened for it.
    fn show_overview_list(
        &mut self,
        filter: acta_core::store::OverviewFilter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let on_scope = self.bench().is_some_and(|b| {
            matches!(
                b.history.current(),
                Some(Route::Project(_)) | Some(Route::Epic(_))
            )
        });
        if !on_scope {
            let Some(project) = self.bench().and_then(|b| b.active_project) else {
                return;
            };
            self.navigate(Route::Project(project), true, window, cx);
        }
        self.with_record(cx, |page, _, cx| page.show_filter(filter, cx), window);
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

    /// "Collapse all sections" / "Expand all sections" on the active page.
    fn fold_all(&mut self, folded: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        match &bench.page {
            PageView::Record(page) => {
                page.update(cx, |page, cx| page.set_all_folds(folded, window, cx))
            }
            PageView::Contact(page) => {
                page.update(cx, |page, cx| page.set_all_folds(folded, window, cx))
            }
            _ => {}
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
        let tray = bench.capture_tray.take();
        let capture = cx.new(|cx| Capture::new(ws, project, draft, tray, window, cx));
        let sub = cx.subscribe_in(
            &capture,
            window,
            |shell, _, event: &CaptureEvent, window, cx| match event {
                CaptureEvent::Close(draft, tray) => {
                    if let Some(bench) = shell.bench() {
                        bench.capture_draft = draft.clone();
                        bench.capture_tray = tray.clone();
                    }
                    shell.close_overlay(window, cx);
                }
                // The note was refused for its contact: the capture comes
                // back with the text and the files, unless another overlay
                // took the screen meanwhile (the draft then waits).
                CaptureEvent::Refused { body, name, tray } => {
                    let Some(bench) = shell.bench() else { return };
                    bench.capture_draft = body.clone();
                    bench.capture_tray = Some(tray.clone());
                    if bench.overlay.is_some() {
                        return;
                    }
                    shell.open_capture(window, cx);
                    let capture = shell.bench().and_then(|b| match &b.overlay {
                        Some(Overlay::Capture(c)) => Some(c.clone()),
                        _ => None,
                    });
                    if let Some(capture) = capture {
                        capture.update(cx, |c, cx| c.refused(name, window, cx));
                    }
                }
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
                        bench.pending_restart = None;
                    }
                    shell.close_overlay(window, cx)
                }
                DialogEvent::ResetHere(options) => {
                    let options = *options;
                    shell.close_overlay(window, cx);
                    shell.reset_here(options, window, cx);
                }
                DialogEvent::BackUpFirst => {
                    shell.close_overlay(window, cx);
                    shell.open_export(window, cx);
                }
                DialogEvent::RestartTerminal => {
                    shell.close_overlay(window, cx);
                    let restart = shell.bench().and_then(|b| b.pending_restart.take());
                    match shell.bench().and_then(|b| b.pending_triage.take()) {
                        Some(key) => shell.start_triage(key, window, cx),
                        None => {
                            if let Some(key) = restart {
                                shell.do_restart(key, window, cx)
                            }
                        }
                    }
                }
                DialogEvent::RemoveSetup => {
                    shell.close_overlay(window, cx);
                    claude_setup::run_remove(cx);
                }
                DialogEvent::Confirmed(on_confirm) => {
                    let on_confirm = on_confirm.clone();
                    shell.close_overlay(window, cx);
                    (on_confirm.0)(0, window, cx);
                }
                DialogEvent::Setup(project) => {
                    shell.close_overlay(window, cx);
                    claude_setup::run_install(*project, cx);
                }
                DialogEvent::ReadyRun { tool, plan, update } => {
                    shell.close_overlay(window, cx);
                    shell.run_ready(*tool, plan.clone(), *update, window, cx);
                }
                DialogEvent::Quit => {
                    shell.close_overlay(window, cx);
                    crate::ready::confirm_quit(cx);
                    // The last window closing quits the app (main.rs).
                    window.remove_window();
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
                include_files: true,
                files: None,
            },
            window,
            cx,
        );
    }

    /// "Import...": the system open dialog, then the backup's passphrase
    /// (dialogs.rs), then the preview. On the key-loss screen (no database
    /// open) the preview shows what an empty database would get, and
    /// nothing is set aside before Import. A file that is not a backup, or
    /// is too large, is refused before the passphrase is asked.
    pub fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            let checked = cx
                .background_spawn({
                    let path = path.clone();
                    async move { acta_core::store::check_backup_file(&path) }
                })
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                let locale = settings::locale(cx);
                let kind = match checked {
                    Ok(()) => DialogKind::ImportPassphrase {
                        path,
                        file_name,
                        error: None,
                        opening: false,
                    },
                    Err(err) => DialogKind::ImportRefused {
                        file_name,
                        message: crate::dialogs::import_refusal(locale, err),
                    },
                };
                shell.open_dialog(kind, window, cx);
            });
        })
        .detach();
    }

    /// "Reset Acta...": the confirmation, with whether voice models are
    /// on this computer (only then is deleting them offered).
    pub fn open_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let has_models = db::config().is_some_and(|c| {
            acta_core::voice::models::models_dir(&c.data_dir)
                .read_dir()
                .is_ok_and(|mut d| d.next().is_some())
        });
        self.open_dialog(
            DialogKind::Reset {
                sign_out: false,
                voice_models: false,
                has_models,
                counts: None,
                unreachable: None,
            },
            window,
            cx,
        );
    }

    /// The key-loss screen's reset: erased now, before anything opens the
    /// database, then a new one opens on first run.
    fn reset_here(
        &mut self,
        options: acta_core::reset::ResetOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(config) = db::config() else { return };
        let erasing = cx.background_spawn(async move {
            acta_core::reset::wipe_here(&config.data_dir, &config.keys, options)
        });
        self._open = cx.spawn_in(window, async move |shell, cx| {
            let wiped = erasing.await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                match wiped {
                    Ok(_) => {
                        crate::log::line("reset: data erased from the key-loss screen");
                        settings::change(cx, |prefs| {
                            crate::preferences::forget_records(prefs, options.sign_out)
                        });
                        shell.content = Content::Opening;
                        shell.open_store(window, cx);
                    }
                    Err(err) => crate::log::line(&format!("reset: not done: {err}")),
                }
                cx.notify();
            });
        });
    }

    /// "Reset Acta..." on the key-loss screen: the same confirmation,
    /// without counts (nothing can be read).
    fn open_reset_here(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let has_models = db::config().is_some_and(|c| {
            acta_core::voice::models::models_dir(&c.data_dir)
                .read_dir()
                .is_ok_and(|mut d| d.next().is_some())
        });
        self.open_dialog(
            DialogKind::Reset {
                sign_out: false,
                voice_models: false,
                has_models,
                counts: None,
                unreachable: None,
            },
            window,
            cx,
        );
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

    /// "Set up Claude Code": a confirmation that says how many folders of
    /// the open project setup will touch.
    fn confirm_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let project = bench.active_project;
        let ws = bench.ws.clone();
        let name =
            project.and_then(|p| crate::sessions_view::title_of(ws.read(cx), Owner::Project(p)));
        let count = ws.read(cx).db.run(cx, move |store| {
            let Some(p) = project else { return Ok(0) };
            let mut paths: Vec<String> = store
                .all_folders()?
                .into_iter()
                .filter(|f| f.project == p)
                .map(|f| f.path)
                .collect();
            paths.sort();
            paths.dedup();
            Ok(paths.len())
        });
        cx.spawn_in(window, async move |shell, cx| {
            let folders = count.await.unwrap_or(0);
            let _ = shell.update_in(cx, |shell, window, cx| {
                let kind = DialogKind::Setup {
                    project: project.zip(name),
                    folders,
                };
                shell.open_dialog(kind, window, cx);
            });
        })
        .detach();
    }

    /// Development captures of the Ready block's dialogs and terminal.
    #[cfg(debug_assertions)]
    fn dev_ready(&mut self, which: &str, window: &mut Window, cx: &mut Context<Self>) {
        use acta_core::prereqs::commands::{install_plan, update_plan};
        use acta_core::prereqs::{Choice, Kind, Tool};
        let Some(platform) = crate::ready::platform(cx) else {
            return;
        };
        match which {
            "ready-dialog" | "ready-dialog-update" => {
                let update = which == "ready-dialog-update";
                let (tool, choice) = if update {
                    (
                        Tool::Claude,
                        update_plan(Tool::Claude, Kind::Native, &platform),
                    )
                } else {
                    (Tool::Git, install_plan(Tool::Git, &platform))
                };
                if let Choice::Run(plan) = choice {
                    self.on_ready(
                        crate::ready::Request::Confirm { tool, plan, update },
                        window,
                        cx,
                    );
                }
            }
            // Settings scrolled to the block, as the status bar opens it.
            "ready-reveal" => {
                self.open_settings(window, cx);
                if let Some(bench) = self.bench() {
                    let page = bench.settings.clone();
                    page.update(cx, |page, cx| page.reveal_ready(cx));
                }
            }
            "ready-quit" => {
                self.open_dialog(DialogKind::QuitInstalling { tool: Tool::Git }, window, cx)
            }
            "ready-oneoff" => {
                let argv: Vec<std::ffi::OsString> = vec!["where".into(), "git".into()];
                let title = "where git".to_owned();
                let spec = cx
                    .background_executor()
                    .spawn(async move { crate::ready::one_off_spec(&argv) });
                cx.spawn_in(window, async move |shell, cx| {
                    let Some(spec) = spec.await else { return };
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.open_one_off(crate::ready_model::Row::Git, title, spec, window, cx)
                    });
                })
                .detach();
            }
            _ => {}
        }
    }

    /// A button of the Ready for Claude Code block (ADR 0013).
    pub(crate) fn on_ready(
        &mut self,
        request: crate::ready::Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::ready::Request;
        match request {
            Request::Confirm { tool, plan, update } => {
                let Some(platform) = crate::ready::platform(cx) else {
                    return;
                };
                let kind = DialogKind::ReadyInstall {
                    tool,
                    plan,
                    update,
                    platform,
                };
                self.open_dialog(kind, window, cx);
            }
            Request::SignIn => self.ready_sign_in(window, cx),
            Request::ShowTerminal => self.show_one_off(window, cx),
        }
    }

    /// A confirmed install: a child with its log, a one-off session of
    /// the terminal (Linux root installs), or Apple's own window.
    fn run_ready(
        &mut self,
        tool: acta_core::prereqs::Tool,
        plan: acta_core::prereqs::Plan,
        update: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use acta_core::prereqs::RunsIn;
        match plan.runs_in {
            RunsIn::Child => crate::ready::start(tool, &plan, update, cx),
            RunsIn::AppleDialog => crate::ready::start_apple(&plan, cx),
            RunsIn::Terminal => {
                let row = crate::ready_model::Row::of(tool);
                if crate::ready::is_dev(cx) {
                    // Never a real install from a development state.
                    crate::ready::wait_terminal(row, cx);
                    return;
                }
                let title = plan.display();
                let argv = plan.argv.clone();
                let spec = cx
                    .background_executor()
                    .spawn(async move { crate::ready::one_off_spec(&argv) });
                cx.spawn_in(window, async move |shell, cx| {
                    let Some(spec) = spec.await else { return };
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.open_one_off(row, title, spec, window, cx)
                    });
                })
                .detach();
            }
            RunsIn::Copy => {}
        }
    }

    /// "Sign in...": Claude Code alone in a one-off session, in the open
    /// project's folder or the home folder; its own first run signs in.
    fn ready_sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = crate::ready_model::Row::SignIn;
        if crate::ready::is_dev(cx) {
            crate::ready::wait_terminal(row, cx);
            return;
        }
        let Some(bench) = self.bench() else { return };
        let project = bench.active_project;
        let folder = bench.ws.read(cx).db.run(cx, move |store| {
            let Some(p) = project else { return Ok(None) };
            Ok(store
                .folders(Owner::Project(p))?
                .first()
                .map(|f| std::path::PathBuf::from(&f.path)))
        });
        cx.spawn_in(window, async move |shell, cx| {
            let cwd = folder.await.ok().flatten();
            let spec = cx
                .background_executor()
                .spawn(async move { crate::ready::sign_in_spec(cwd) })
                .await;
            let Some(spec) = spec else { return };
            let _ = shell.update_in(cx, |shell, window, cx| {
                shell.open_one_off(
                    row,
                    acta_core::prereqs::Tool::Claude.display_name().to_owned(),
                    spec,
                    window,
                    cx,
                )
            });
        })
        .detach();
    }

    /// Start a one-off session for `row` in the terminal and show it.
    fn open_one_off(
        &mut self,
        row: crate::ready_model::Row,
        title: String,
        spec: acta_terminal::LaunchSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::ready::wait_terminal(row, cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        dock.update(cx, |dock, cx| {
            dock.open_one_off(row, title, spec, window, cx)
        });
        self.show_one_off(window, cx);
    }

    /// Show the terminal on its one-off session.
    fn show_one_off(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(None, window, cx, |l| {
            l.show_view(ViewId::Terminal).is_some()
        });
        let Some(bench) = self.bench() else { return };
        // Shown already: showing it starts no session of the project.
        bench.ui.terminal_shown = true;
        let dock = bench.dock.clone();
        dock.update(cx, |dock, cx| dock.select_one_off(cx));
        let focus = dock.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// The Sessions view's actions (components-app.md 5).
    fn on_sessions(
        &mut self,
        _: &Entity<crate::sessions_view::SessionsView>,
        event: &crate::sessions_view::SessionsEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::sessions_view::SessionsEvent;
        match event {
            SessionsEvent::Open(owner) => {
                self.navigate(owner_route(*owner), true, window, cx);
            }
            SessionsEvent::ShowTerminal(key) => self.show_session_terminal(*key, window, cx),
            SessionsEvent::End(key) => {
                if let Some(bench) = self.bench() {
                    bench.dock.update(cx, |dock, cx| dock.end(*key, cx));
                }
            }
        }
    }

    /// What a terminal dock asks of the shell (layout.md 4.5.1).
    fn on_dock(
        &mut self,
        _: &Entity<TerminalDock>,
        event: &DockEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            DockEvent::Restart(key) => self.restart_instance(*key, window, cx),
            DockEvent::EditFolders(origin) => self.open_folders_of(origin.owner, false, window, cx),
            DockEvent::AddFolder(origin) => self.open_folders_of(origin.owner, true, window, cx),
            DockEvent::OneOffEnded(row) => crate::ready::terminal_ended(*row, cx),
            DockEvent::ShowFree => {
                self.view_action(ViewId::FreeClaude, ViewAction::Show, window, cx)
            }
        }
    }

    /// The dock holding `key`: the free one for Claude Code (no project).
    fn dock_of(&mut self, key: InstanceKey) -> Option<Entity<TerminalDock>> {
        let bench = self.bench()?;
        Some(match key.holder {
            Holder::Free => bench.free_dock.clone(),
            Holder::Page(_) => bench.dock.clone(),
        })
    }

    /// "Show terminal" (Sessions view): the session's own page opens in
    /// the active tab and the Terminal view shows that session; the view
    /// never shows a session of another page (layout.md 4.5.1).
    pub(crate) fn show_session_terminal(
        &mut self,
        key: InstanceKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Holder::Page(owner) = key.holder else {
            return;
        };
        self.navigate(owner_route(owner), false, window, cx);
        self.show_terminal_view(window, cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        dock.update(cx, |dock, cx| dock.select(key, cx));
        let focus = dock.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// "Show terminal" of a page line: the page's own session (the one
    /// waiting, else the one used last).
    pub(crate) fn show_page_sessions(
        &mut self,
        owner: Owner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(owner_route(owner), false, window, cx);
        self.show_terminal_view(window, cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        dock.update(cx, |dock, cx| dock.select_page_session(owner, cx));
        let focus = dock.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// Show the Terminal view without starting anything.
    fn show_terminal_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(None, window, cx, |l| {
            l.show_view(ViewId::Terminal).is_some()
        });
        if let Some(bench) = self.bench() {
            bench.ui.terminal_shown = true;
        }
    }

    /// "Start Claude Code here" (``Ctrl+Shift+` ``, the palette): a new
    /// session on the current page, the view shown. On a page that owns
    /// nothing the view shows why.
    fn start_claude_here(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_tool_owner(window, cx);
        self.show_terminal_view(window, cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        dock.update(cx, |dock, cx| {
            dock.start_here(window, cx);
        });
        let focus = dock.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// "New Claude Code (no project)": the view shown and a session
    /// started in it.
    fn new_free_claude(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_layout(None, window, cx, |l| {
            l.show_view(ViewId::FreeClaude).is_some()
        });
        let Some(bench) = self.bench() else { return };
        let dock = bench.free_dock.clone();
        dock.update(cx, |dock, cx| {
            dock.start_here(window, cx);
        });
        let focus = dock.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// `Ctrl+PageDown` / `Ctrl+PageUp`: the next or previous instance of
    /// the tool view that has the keyboard.
    fn step_instance(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        #[cfg(windows)]
        {
            let chat = bench.chat.clone();
            if chat.read(cx).focus_handle(cx).contains_focused(window, cx) {
                chat.update(cx, |chat, cx| chat.step(forward, window, cx));
                return;
            }
            let browser = bench.browser.clone();
            if browser
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
            {
                browser.update(cx, |b, cx| b.step(forward, window, cx));
                return;
            }
        }
        let free = bench.free_dock.clone();
        let dock = if free.read(cx).focus_handle(cx).contains_focused(window, cx) {
            free
        } else {
            bench.dock.clone()
        };
        dock.update(cx, |dock, cx| dock.step(forward, window, cx));
    }

    /// "Restart Claude Code" (palette): the session in view of the
    /// Terminal view, else of Claude Code (no project).
    fn restart_shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench() else { return };
        let key = bench
            .dock
            .read(cx)
            .shown()
            .or_else(|| bench.free_dock.read(cx).shown());
        if let Some(key) = key {
            self.restart_instance(key, window, cx);
        }
    }

    /// Restart and "Restart in ..." (layout.md 4.5.1): the instance starts
    /// again from its own page; confirmed while a process runs (6.4).
    fn restart_instance(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dock) = self.dock_of(key) else {
            return;
        };
        if dock.read(cx).is_running(key, cx) {
            if let Some(bench) = self.bench() {
                bench.pending_restart = Some(key);
            }
            self.open_dialog(DialogKind::RestartTerminal, window, cx);
        } else {
            self.do_restart(key, window, cx);
        }
    }

    fn do_restart(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dock) = self.dock_of(key) else {
            return;
        };
        dock.update(cx, |dock, cx| dock.restart(key, window, cx));
        cx.notify();
    }

    /// "Edit folders..." / "Add folder..." of the terminal header: the
    /// session's page, its folder list unfolded (and the picker).
    fn open_folders_of(
        &mut self,
        owner: Owner,
        add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(owner_route(owner), true, window, cx);
        self.with_record(
            cx,
            |page, _, cx| {
                page.show_folders(cx);
                if add {
                    page.add_folder(cx);
                }
            },
            window,
        );
    }

    /// "File with Claude Code" (screens.md 8.2): the terminal shows, and
    /// the triage command runs in the project page's session, typed there
    /// only when Claude Code is idle at its prompt. Mid-task, nothing is
    /// typed: the restart confirmation asks before that session restarts
    /// on it.
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
        self.show_terminal_view(window, cx);
        let Some(bench) = self.bench() else { return };
        let dock = bench.dock.clone();
        let (sent, key) = dock.update(cx, |dock, cx| {
            dock.send(origin.clone(), TRIAGE_COMMAND, window, cx)
        });
        crate::log::line(&format!("triage: {sent:?}"));
        if sent == crate::terminal_dock::Send::Busy {
            if let Some(bench) = self.bench() {
                bench.pending_triage = key;
            }
            self.open_dialog(DialogKind::RestartTerminal, window, cx);
        }
    }

    fn start_triage(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        self.show_terminal_view(window, cx);
        let Some(bench) = self.bench() else { return };
        bench.dock.update(cx, |dock, cx| {
            dock.restart_on(key, TRIAGE_COMMAND, window, cx)
        });
        cx.notify();
    }

    fn terminal_size(&mut self, by: f32, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            let free = bench.free_dock.clone();
            bench
                .dock
                .update(cx, |dock, cx| dock.change_text_size(by, cx));
            let size = bench.dock.read(cx).text_size();
            free.update(cx, |dock, cx| dock.apply_text_size(size, cx));
        }
    }

    // ---- chat ----

    /// "Agent browser: Open developer tools" (the palette, while the view
    /// is shown): the view's DevTools window.
    #[cfg(windows)]
    fn open_browser_devtools(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(bench) = self.bench() {
            bench.browser.update(cx, |b, cx| b.open_devtools(cx));
        }
    }

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
            ChatEvent::Opened => self.close_idle_web_views(window, cx),
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
        browser: &Entity<BrowserDock>,
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
            Kept::ZoomIn => Box::new(ZoomIn),
            Kept::ZoomOut => Box::new(ZoomOut),
            Kept::ZoomReset => Box::new(ZoomReset),
            // The agent browser handles its own; the chat never keeps it.
            Kept::DevTools | Kept::Focus(_) => return,
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
        let right_over = bench.ui.right_overlay && place != Some(Place::Side(Side::Right));
        left_over
            || right_over
            || bench.ui.menu.is_some()
            || bench.tree_menu.is_some()
            || toast_over
            || bench.overlay.is_some()
            || any_page_menu
            || bench.chat.read(cx).menu_open()
            || bench.browser.read(cx).menu_open(cx)
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
        let stays = bench.toast.as_ref().is_none_or(|t| toast_stays(t.kind));
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
    fn dev_permission_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = std::env::var("ACTA_DEV_CHECK_PROMPT").unwrap_or_default();
        self.start_claude_here(window, cx);
        cx.spawn_in(window, async move |shell, cx| {
            let step = |shell: &mut Shell, name: &str, cx: &mut Context<Shell>, f: &dyn Fn(&mut TerminalDock, ProjectId, &mut Context<TerminalDock>)| {
                let Some(bench) = shell.bench() else { return };
                let Some(project) = bench.active_project else { return };
                let screen = bench.dock.read(cx).dev_screen(project, cx);
                eprintln!("[permission-check] {name}\n{}", screen.unwrap_or_default());
                bench.dock.update(cx, |dock, cx| f(dock, project, cx));
            };
            cx.background_executor()
                .timer(Duration::from_secs(30))
                .await;
            let _ = shell.update_in(cx, |shell, _, cx| {
                step(shell, "start", cx, &|dock, project, cx| {
                    let typed = dock.dev_type_if_idle(project, &prompt, cx);
                    eprintln!("[permission-check] prompt typed: {typed}");
                })
            });
            cx.background_executor()
                .timer(Duration::from_secs(40))
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                step(shell, "question pending", cx, &|_, _, _| {});
                if let Some(project) = shell.bench().and_then(|b| b.active_project) {
                    shell.file_with_claude(project, window, cx);
                }
                // The restart confirmation, if it opened: left unanswered.
                shell.close_overlay(window, cx);
            });
            cx.background_executor()
                .timer(Duration::from_secs(5))
                .await;
            let _ = shell.update_in(cx, |shell, _, cx| {
                step(shell, "declining", cx, &|dock, project, cx| {
                    dock.dev_send_raw(project, b"\x1b", cx)
                })
            });
            cx.background_executor()
                .timer(Duration::from_secs(15))
                .await;
            let _ = shell.update_in(cx, |shell, _, cx| {
                step(shell, "declined", cx, &|dock, project, cx| {
                    let typed = dock.dev_type_if_idle(project, "/exit", cx);
                    eprintln!("[permission-check] /exit typed: {typed}");
                })
            });
            cx.background_executor()
                .timer(Duration::from_secs(10))
                .await;
            let _ = shell.update_in(cx, |shell, _, cx| step(shell, "after exit", cx, &|_, _, _| {}));
        })
        .detach();
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
            TreeKey::Open => match (row.route(), &row) {
                (Some(route), _) => self.navigate(route, true, window, cx),
                (None, Row::NewTopic { project, epic }) => {
                    self.new_topic_in(*project, Some(*epic), window, cx)
                }
                (None, _) => {
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
        // The cursor's row stays in view, never under the sticky row.
        self.reveal_cursor(cx);
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
            tree::Letter::Due => self.open_tree_due(&row, window, cx),
            tree::Letter::Rename => self.start_rename(&row, window, cx),
            tree::Letter::Preview => match (row.route(), &row) {
                (Some(route), _) => {
                    if let Some(bench) = self.bench() {
                        bench.history.open(route);
                    }
                    self.show_route(route, false, window, cx);
                }
                // On a ghost row `Space` does what `Enter` does (6.1).
                (None, Row::NewTopic { project, epic }) => {
                    self.new_topic_in(*project, Some(*epic), window, cx)
                }
                (None, _) => {
                    if let Some(k) = row.fold_key() {
                        self.toggle_fold(k, cx);
                    }
                }
            },
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
                self.reveal_cursor(cx);
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

    /// The record of a tree row the due date commands act on, with its
    /// date as stored.
    fn due_of_row(&self, row: &Row, cx: &App) -> Option<(Owner, Option<acta_core::date::Date>)> {
        let Content::Workbench(bench) = &self.content else {
            return None;
        };
        let projects = &bench.ws.read(cx).projects;
        match row {
            Row::Project { id, .. } => projects
                .iter()
                .find(|t| t.project.id == *id)
                .map(|t| (Owner::Project(*id), t.project.due_on)),
            Row::Epic { id, .. } => projects
                .iter()
                .flat_map(|t| &t.epics)
                .find(|(e, _)| e.id == *id)
                .map(|(e, _)| (Owner::Epic(*id), e.due_on)),
            Row::Topic { id, .. } => projects
                .iter()
                .flat_map(|t| {
                    t.loose_topics
                        .iter()
                        .chain(t.epics.iter().flat_map(|e| &e.1))
                })
                .find(|t| t.id == *id)
                .map(|t| (Owner::Topic(*id), t.due_on)),
            _ => None,
        }
    }

    /// `D` on a project, epic or topic row: the due date picker anchored
    /// to the row's title.
    fn open_tree_due(&mut self, row: &Row, window: &mut Window, cx: &mut Context<Self>) {
        let Some((owner, current)) = self.due_of_row(row, cx) else {
            return;
        };
        let today = crate::due::today(cx);
        let picker = cx.new(|cx| DuePicker::new(today, current, window, cx));
        let sub = cx.subscribe_in(
            &picker,
            window,
            |shell, _, event: &DuePickerEvent, window, cx| {
                let DuePickerEvent::Done(outcome) = *event;
                shell.close_tree_due(outcome, window, cx);
            },
        );
        let Some(bench) = self.bench() else { return };
        bench.tree_menu = None;
        bench.tree_due = Some(TreeDue {
            row: row.key(),
            owner,
            picker,
            anchor: AnchorBounds::default(),
            _sub: sub,
        });
        cx.notify();
    }

    /// The tree's picker closed: write what it set, focus back on the tree.
    fn close_tree_due(
        &mut self,
        outcome: crate::due::Outcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bench) = self.bench() else { return };
        let Some(due) = bench.tree_due.take() else {
            return;
        };
        if let crate::due::Outcome::Set(date) = outcome {
            bench
                .ws
                .update(cx, |ws, cx| ws.set_due(due.owner, date, cx));
        }
        let focus = bench.tree_focus.clone();
        window.focus(&focus, cx);
        cx.notify();
    }

    /// Where "Set due date..." and "Clear due date" act (interaction.md
    /// 6.1): the tree row under the keyboard cursor while the tree has
    /// focus, else the open project, epic or topic page. `true` in the
    /// first field: the tree row.
    fn due_target(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<(bool, Owner, Option<acta_core::date::Date>)> {
        let rows = self.rows(cx);
        let bench = self.bench()?;
        if bench.tree_focus.contains_focused(window, cx) && !rows.is_empty() {
            let row = rows[bench.cursor.index(&rows)].clone();
            if let Some((owner, due)) = self.due_of_row(&row, cx) {
                return Some((true, owner, due));
            }
        }
        let bench = self.bench()?;
        if let PageView::Record(page) = &bench.page
            && let Some((owner, due)) = page.read(cx).due_target(window)
        {
            return Some((false, owner, due));
        }
        None
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
                // Enter saves and gives the keyboard back to the tree at
                // once, so its keys work while the write runs (QA: focus
                // was lost until Mod+1).
                InputEvent::PressEnter { .. } => {
                    shell.commit_rename(window, cx);
                    if let Some(handle) = shell.bench().map(|b| b.tree_focus.clone()) {
                        window.focus(&handle, cx);
                    }
                }
                InputEvent::Blur => shell.commit_rename(window, cx),
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
        let Some(bench) = self.bench() else { return };
        let Some(Rename { key, state, saving }) = bench.rename.clone() else {
            return;
        };
        if saving {
            return;
        }
        let new = state.read(cx).value().trim().to_owned();
        // The row may be folded away or out of the drawn range: the record
        // and its title come from the key.
        let projects = &bench.ws.read(cx).projects;
        let (owner, old) = match key {
            RowKey::Route(Route::Project(id)) => (
                Some(Owner::Project(id)),
                tree::locate_project(projects, id).and_then(|l| l.project.map(|p| p.1)),
            ),
            RowKey::Route(Route::Epic(id)) => (
                Some(Owner::Epic(id)),
                tree::locate_epic(projects, id).and_then(|l| l.epic.map(|e| e.1)),
            ),
            RowKey::Route(Route::Topic(id)) => (
                Some(Owner::Topic(id)),
                tree::locate_topic(projects, id).and_then(|l| l.topic.map(|t| t.1)),
            ),
            _ => (None, None),
        };
        let unchanged = old.is_none_or(|old| old == new);
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
        let status = claude_setup::status(
            crate::ready::readiness(cx),
            check,
            facts.as_ref().map(|f| f.has_folder),
        );
        let (glyph, glyph_color, key) = match status {
            claude_setup::Status::NotInstalled => (
                Icon::CircleSlash,
                tokens.text_tertiary,
                Key::StatusClaudeMissing,
            ),
            claude_setup::Status::SignedOut => (
                Icon::CircleDashed,
                tokens.text_tertiary,
                Key::StatusClaudeSignedOut,
            ),
            claude_setup::Status::NotSetUp => (
                Icon::CircleDashed,
                tokens.text_tertiary,
                Key::StatusbarNotSetUp,
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
        // components-app.md 5, Claude Code sessions: while sessions run, the
        // item counts them by state and shows the Sessions view.
        let live = crate::sessions::live(cx).to_vec();
        let counts = crate::sessions::counts(&live);
        let sessions_text = crate::sessions::status_text(locale, counts);
        // layout.md 4.9: what gives way when the bar is short of room.
        let give = status_give_way(self.effective_width);
        let claude_item = match (sessions_text, crate::sessions::strongest(&live)) {
            (Some(label), Some(state)) => {
                // The bar names the most urgent state; the tooltip lists
                // every state, then the first session's focus and activity.
                let mut lines = crate::sessions::state_counts(locale, counts);
                if let Some(s) = live.first() {
                    let owner = crate::sessions::place(s);
                    let name = self
                        .bench()
                        .and_then(|b| crate::sessions_view::title_of(b.ws.read(cx), owner))
                        .unwrap_or_default();
                    lines.push(crate::terminal_dock::focus_label(locale, owner, &name));
                    lines.push(crate::sessions::detail(locale, s, true));
                }
                let tip = Some(lines.join("\n"));
                ui::bar_item("status-claude", &tokens)
                    .child(icon(
                        Icon::Bot,
                        14.,
                        crate::sessions::state_color(state, &tokens),
                    ))
                    .child(type_scale::SMALL.el().child(label))
                    .when_some(tip, |this, tip| this.tooltip(ui::tip(tip)))
                    .on_click(cx.listener(|shell, _, window, cx| {
                        shell.view_action(ViewId::Sessions, ViewAction::Show, window, cx)
                    }))
            }
            _ => ui::bar_item("status-claude", &tokens)
                .child(icon(glyph, 14., glyph_color))
                .when(!give.claude_words, |this| {
                    this.child(type_scale::SMALL.el().child(text(locale, key)))
                })
                .when_some(
                    if give.claude_words {
                        Some(match last_wrote {
                            Some(tip) => format!("{}\n{tip}", text(locale, key)),
                            None => text(locale, key),
                        })
                    } else {
                        last_wrote
                    },
                    |this, tip| this.tooltip(ui::tip(tip)),
                )
                .on_click(cx.listener(move |shell, _, window, cx| {
                    match status {
                        claude_setup::Status::NoFolder => shell.set_project_folder(window, cx),
                        // A required row is not ready: Settings, at the block.
                        claude_setup::Status::NotInstalled | claude_setup::Status::SignedOut => {
                            shell.open_settings(window, cx);
                            if let Some(bench) = shell.bench() {
                                let page = bench.settings.clone();
                                page.update(cx, |page, cx| page.reveal_ready(cx));
                            }
                        }
                        _ => shell.open_settings(window, cx),
                    }
                })),
        };
        let usage_items = if settings::current(cx).hide_usage || give.usage {
            Vec::new()
        } else {
            usage_items(&tokens, locale, cx)
        };
        let chosen = settings::zoom(cx);
        let applied = self.zoom_applied;
        let zoom_item = (applied != zoom::DEFAULT).then(|| {
            let value = crate::i18n::percent(locale, u64::from(applied));
            let mut tip = text(locale, Key::ZoomStatusTip);
            if applied < chosen {
                tip.push('\n');
                tip.push_str(&tr(
                    locale,
                    Key::ZoomLimited,
                    &[
                        (
                            "chosen",
                            Arg::Text(&crate::i18n::percent(locale, u64::from(chosen))),
                        ),
                        ("applied", Arg::Text(&value)),
                    ],
                ));
            }
            let macos = cfg!(target_os = "macos");
            ui::bar_item("status-zoom", &tokens)
                .child(
                    type_scale::SMALL
                        .el()
                        .font_features(type_scale::tabular())
                        .child(tr(locale, Key::ZoomStatus, &[("value", Arg::Text(&value))])),
                )
                .tooltip(ui::tip_keys(
                    tip,
                    vec![crate::i18n::KeyName::Mod.label(locale, macos), "0"],
                ))
                .on_click(|_, _, cx| settings::set_zoom(zoom::DEFAULT, cx))
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
            .border_t(px(1.))
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
                    .child(claude_item)
                    .children(usage_items),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h_full()
                    .children(zoom_item)
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
                    .when(!give.version, |this| {
                        this.child(
                            ui::bar_item("status-version", &tokens)
                                .child(
                                    type_scale::SMALL
                                        .el()
                                        .font_features(type_scale::tabular())
                                        .child(version),
                                )
                                .on_click(cx.listener(|shell, _, window, cx| {
                                    shell.open_settings(window, cx)
                                })),
                        )
                    }),
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
            (Some(Route::Questions(owner)), _) => {
                let project = bench.ws.read(cx).project_of(owner);
                project
                    .and_then(project_crumb)
                    .into_iter()
                    .chain([crumb(text(locale, Key::SectionQuestions), None)])
                    .collect()
            }
            (Some(Route::Contacts), _) | (Some(Route::Contact(_)), _) => vec![crumb(
                text(locale, Key::ContactsTitle),
                Some(Route::Contacts),
            )],
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
                        div()
                            .mt(px(8.))
                            .ml(px(-12.))
                            .child(
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
                            )
                            .child(
                                ui::button(
                                    "keyloss-reset",
                                    text(locale, Key::SettingsReset),
                                    Variant::Ghost,
                                    false,
                                    &t,
                                )
                                .on_click(cx.listener(
                                    |shell, _, window, cx| shell.open_reset_here(window, cx),
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
                let paths = crate::dialogs::probe_safe_paths(&file);
                // Commit modes never come from a file: a recovered folder
                // that committed automatically is Manual now.
                let had_auto = file
                    .folders
                    .iter()
                    .any(|f| f.commit_mode != acta_core::model::CommitMode::Manual);
                imported = Some((report.ok(), paths, had_auto));
            }
            (opened, imported)
        });
        self._open = cx.spawn_in(window, async move |shell, cx| {
            let (opened, imported) = opening.await;
            let missing = match &imported {
                Some((_, paths, _)) => {
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
                        if let Some((report, _, had_auto)) = imported {
                            let toast = match report {
                                Some(report) => Toast {
                                    message: crate::dialogs::recovery_done(
                                        locale, &report, missing, had_auto,
                                    ),
                                    kind: ToastKind::Done,
                                    action: None,
                                },
                                None => Toast {
                                    message: text(locale, Key::ErrorSave),
                                    kind: ToastKind::Error,
                                    action: None,
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
    /// sidebar over the page, the toast, menus, a drag). Both are modal: a
    /// transparent layer over the window takes every click outside them,
    /// which closes them and reaches nothing behind (the user's report of
    /// 2026-11-22: the app behind stayed clickable under the palette).
    fn overlay_element(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let outside = cx.listener(|shell, _: &gpui_kit::MouseDownEvent, window, cx| {
            shell.overlay_clicked_outside(window, cx)
        });
        let bench = self.bench()?;
        let top = |child: AnyElement| {
            deferred(
                div()
                    .id("overlay-catcher")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .on_any_mouse_down(outside)
                    .pt(px(96.))
                    .flex()
                    .justify_center()
                    .items_start()
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
            Overlay::Viewer(viewer) => deferred(viewer.clone())
                .with_priority(OVERLAY_PRIORITY)
                .into_any_element(),
            Overlay::FileMenu(menu) => deferred(menu.clone())
                .with_priority(OVERLAY_PRIORITY)
                .into_any_element(),
        })
    }

    /// The palette is open: `Tab` never leaves it.
    fn modal_overlay(&mut self) -> bool {
        self.bench()
            .is_some_and(|b| matches!(b.overlay, Some(Overlay::Palette(_))))
    }

    /// A click outside the palette or quick capture: it closes, as `Esc`
    /// does (capture keeps its draft), and the click acts on nothing else.
    fn overlay_clicked_outside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let Some(bench) = self.bench() else { return };
        match &bench.overlay {
            Some(Overlay::Palette(_)) => {
                bench.create_in = None;
                bench.on_pick = None;
                self.close_overlay(window, cx);
            }
            Some(Overlay::Capture(capture)) => {
                let capture = capture.clone();
                capture.update(cx, |c, cx| c.close(cx));
            }
            _ => {}
        }
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
                .border(px(1.))
                .border_color(t.border_default)
                .rounded(px(RADIUS_LG))
                .shadow(vec![t.shadow_overlay()])
                .children(glyph)
                .child(
                    // `min_w_0`: the message wraps inside the toast's 360
                    // instead of running past its background.
                    type_scale::UI
                        .el()
                        .flex_1()
                        .min_w_0()
                        .text_color(t.text_primary)
                        .child(toast.message.clone()),
                )
                .children(toast.action.clone().map(|action| {
                    match action {
                        crate::workspace::ToastAction::Reveal(path) => ui::button(
                            "toast-reveal",
                            text(locale, Key::AttachShowInFolder),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        // The parent folder with the file selected, never the file.
                        .on_click(move |_, _, cx| cx.reveal_path(&path))
                        .into_any_element(),
                        crate::workspace::ToastAction::ShowToCheck(project) => ui::button(
                            "toast-show",
                            text(locale, Key::KbScanShow),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(move |shell, _, window, cx| {
                            if let Some(bench) = shell.bench() {
                                bench.toast = None;
                            }
                            shell.open_kb_chip(
                                project,
                                crate::kb_model::Chip::ToCheck,
                                None,
                                window,
                                cx,
                            );
                        }))
                        .into_any_element(),
                        crate::workspace::ToastAction::RetrySave(file, path) => ui::button(
                            "toast-retry",
                            text(locale, Key::CommonRetry),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if let Some(bench) = shell.bench() {
                                bench.toast = None;
                                let (file, path) = ((*file).clone(), path.clone());
                                bench
                                    .files
                                    .clone()
                                    .update(cx, |files, cx| files.save_to(file, path, cx));
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                    }
                }))
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
            .border(px(1.))
            .border_color(t.border_default)
            .rounded(px(RADIUS_LG))
            .shadow(vec![t.shadow_overlay()])
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(8.))
                    // A failure says so with its glyph (QA 2026-11-22).
                    .when(room, |this| {
                        this.child(div().h(px(20.)).flex().items_center().child(icon(
                            Icon::TriangleAlert,
                            16.,
                            t.warning,
                        )))
                    })
                    .child(
                        type_scale::UI
                            .el()
                            .flex_1()
                            .min_w_0()
                            .text_color(t.text_primary)
                            .line_clamp(2)
                            .child(toast.message.clone()),
                    ),
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

/// Debug builds started with `ACTA_DEV_SEED=1` (or `bare`, or `long`) fill an
/// empty database with sample data, the current directory as the project folder.
fn seed_if_asked(store: &mut acta_core::Store) {
    let seed = std::env::var("ACTA_DEV_SEED").unwrap_or_default();
    if cfg!(debug_assertions)
        && (seed == "1"
            || seed == "bare"
            || seed == "long"
            || seed == "due"
            || seed == "folders"
            || seed == "folders-missing"
            || seed == "questions"
            || seed == "people"
            || seed == "attachments"
            || seed == "quality")
        && let Some(dir) = crate::launch_dir()
        && let Err(err) = match seed.as_str() {
            "bare" => crate::dev_seed::bare(store, &dir),
            "long" => crate::dev_seed::long(store, &dir),
            #[cfg(debug_assertions)]
            "due" => crate::dev_seed::due(store, &dir),
            #[cfg(debug_assertions)]
            "folders" | "folders-missing" => crate::dev_seed::folders(store, &dir),
            #[cfg(debug_assertions)]
            "questions" | "people" => crate::dev_seed::questions(store, &dir),
            #[cfg(debug_assertions)]
            "attachments" => crate::dev_seed::attachments(store, &dir),
            #[cfg(debug_assertions)]
            "quality" => crate::dev_seed::quality(store, &dir),
            _ => crate::dev_seed::seed(store, &dir),
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
        Row::Inbox { project, .. } | Row::Knowledge { project } | Row::NewTopic { project, .. } => {
            Some(*project)
        }
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

/// The plan usage meters of the status bar (components-app.md 5, Plan
/// usage): "5h 42%" and "7d 18%", each with a 24 x 4 bar, `warning` above
/// 80 %, dimmed once its window has reset; nothing until Claude Code first
/// reports it.
fn usage_items(t: &Tokens, locale: Locale, cx: &mut Context<Shell>) -> Vec<AnyElement> {
    let now = acta_core::time::now_ms();
    // Nothing until there is a number (layout.md 4.6): why the bar is
    // empty is the tooltip of the Settings row.
    let Some(usage) = crate::sessions::plan_usage(cx) else {
        return Vec::new();
    };
    let windows = [
        (
            "status-usage-5h",
            usage.five_hour,
            Key::UsageFiveHour,
            Key::UsageFiveHourTip,
        ),
        (
            "status-usage-7d",
            usage.seven_day,
            Key::UsageSevenDay,
            Key::UsageSevenDayTip,
        ),
    ];
    windows
        .into_iter()
        .filter_map(|(id, w, key, tip_key)| {
            let m = crate::sessions::meter(w, now)?;
            let pct = crate::i18n::percent(locale, m.percent as u64);
            let mut tip = format!(
                "{}, {}",
                tr(locale, tip_key, &[("percent", Arg::Text(&pct))]),
                crate::sessions::resets_text(locale, m.resets_at, now)
            );
            if m.stale {
                tip.push('\n');
                tip.push_str(&text(locale, Key::UsageStale));
            }
            let color = if m.stale {
                t.text_disabled
            } else if m.high {
                t.warning
            } else {
                t.text_secondary
            };
            let filled = 24. * f32::from(m.percent.min(100)) / 100.;
            Some(
                ui::bar_item(id, t)
                    .gap(px(4.))
                    .child(
                        type_scale::SMALL
                            .el()
                            .font_features(type_scale::tabular())
                            .when(m.high && !m.stale, |this| this.text_color(t.warning))
                            .when(m.stale, |this| this.text_color(t.text_disabled))
                            .child(tr(locale, key, &[("percent", Arg::Text(&pct))])),
                    )
                    .child(
                        div()
                            .w(px(24.))
                            .h(px(4.))
                            .rounded(px(2.))
                            .bg(t.fill_selected)
                            .child(div().w(px(filled)).h_full().rounded(px(2.)).bg(color)),
                    )
                    .tooltip(ui::tip(tip))
                    .into_any_element(),
            )
        })
        .collect()
}

/// The open menu of a tree row (components.md 5, Context menu): the row
/// menu, the status menu (`S`, then `1` to `6`) or the epics (`M`).
fn tree_menu(
    anchor: &crate::menu::AnchorBounds,
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
    // Over the row's title, one row high: the menu hangs 4 under the row.
    div()
        .size_full()
        .relative()
        .child(anchor.measure())
        .children(anchor.below(panel, crate::menu::Align::Start))
        .into_any_element()
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // layout.md 4.9: the applied zoom, before anything is sized; every
        // view of this window renders after this one, in this frame.
        let viewport = window.viewport_size();
        let (w, h) = (f32::from(viewport.width), f32::from(viewport.height));
        self.zoom_applied = zoom::applied(settings::zoom(cx), w, h);
        zoom::set_factor(zoom::as_factor(self.zoom_applied));
        window.set_rem_size(zoom::rem_size());
        crate::webview_build::set_zoom(zoom::factor());
        self.effective_width = zoom::eff(viewport.width);
        let tokens = cx.global::<Tokens>().clone();
        self.sync_tool_owner(window, cx);
        // A hidden tree reports no pointer leaving it: it is not hovered.
        if !self.view_drawn(ViewId::Projects, window)
            && let Some(bench) = self.bench()
        {
            bench.tree.clear_hover();
        }
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
        let overlay = self.overlay_element(cx);

        let frame = type_scale::UI
            .el()
            .id("shell")
            .key_context("Workbench")
            .track_focus(&self.focus)
            .when(dragging, |this| this.key_context("Dragging"))
            // 4.8.7: `Esc` cancels a drag before it means anything else,
            // whatever has focus.
            .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "escape" && (s.cancel_drag(w, cx) || s.cancel_tree_drag(cx)) {
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|s, _: &CancelDrag, w, cx| {
                s.cancel_drag(w, cx);
            }))
            .on_action(cx.listener(|s, _: &DismissSidebar, w, cx| {
                let over = s.bench().is_some_and(|b| {
                    (b.ui.left_overlay || b.ui.right_overlay)
                        && b.overlay.is_none()
                        && b.ui.menu.is_none()
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
            // Dictation (components-voice.md 5): push-to-talk ends on the
            // key's release, wherever the focus is.
            .capture_key_up(crate::voice::key_up)
            .capture_key_down(crate::voice::any_key_down)
            // One take, one undo step: the window sees an editor's undo
            // and redo first (voice/group.rs).
            .capture_action(|_: &gpui_kit::component::input::Undo, window, cx| {
                crate::voice::undo_key(true, window, cx)
            })
            .capture_action(|_: &gpui_kit::component::input::Redo, window, cx| {
                crate::voice::undo_key(false, window, cx)
            })
            .on_action(
                cx.listener(|s, _: &crate::voice::OpenVoiceSettings, w, cx| {
                    s.open_voice_settings(w, cx)
                }),
            )
            .on_action(
                cx.listener(|s, _: &crate::voice::OpenVoiceSetup, w, cx| s.open_voice_setup(w, cx)),
            )
            .on_action(cx.listener(
                |s, _: &crate::voice::settings_view::ConfirmDeleteModel, w, cx| {
                    s.confirm_delete_model(w, cx)
                },
            ))
            .on_action(cx.listener(|shell, _: &OpenShortcuts, window, cx| {
                shell.navigate(Route::Shortcuts, true, window, cx)
            }))
            .on_action(
                cx.listener(|s, _: &CloseTab, w, cx| s.run_command(Command::CloseTab, w, cx)),
            )
            .on_action(cx.listener(|s, _: &RemoveClaudeSetup, w, cx| {
                s.open_dialog(DialogKind::RemoveSetup, w, cx)
            }))
            .on_action(cx.listener(|s, _: &SetUpClaude, w, cx| s.confirm_setup(w, cx)))
            .on_action(cx.listener(|s, _: &NextTab, w, cx| {
                let focus = !s.terminal_focused(w, cx);
                s.cycle_tab(true, focus, w, cx)
            }))
            .on_action(cx.listener(|s, _: &PreviousTab, w, cx| {
                let focus = !s.terminal_focused(w, cx);
                s.cycle_tab(false, focus, w, cx)
            }))
            // Focus stays in the palette while it is open (it is modal).
            .on_action(cx.listener(|s, _: &FocusNext, window, cx| {
                if !s.modal_overlay() {
                    window.focus_next(cx)
                }
            }))
            .on_action(cx.listener(|s, _: &FocusPrevious, window, cx| {
                if !s.modal_overlay() {
                    window.focus_prev(cx)
                }
            }))
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
            .on_action(cx.listener(|s, _: &ResetData, w, cx| s.open_reset(w, cx)))
            .on_action(cx.listener(|s, _: &OpenTrash, w, cx| s.navigate(Route::Trash, true, w, cx)))
            .on_action(cx.listener(|s, _: &ToggleTerminal, w, cx| s.toggle_terminal(w, cx)))
            .on_action(cx.listener(|s, _: &StartClaudeHere, w, cx| {
                if s.is_workbench() {
                    s.start_claude_here(w, cx)
                }
            }))
            .on_action(cx.listener(|s, _: &ToggleFreeClaude, w, cx| {
                if s.is_workbench() {
                    s.toggle_view(ViewId::FreeClaude, w, cx)
                }
            }))
            .on_action(cx.listener(|s, _: &NextInstance, w, cx| s.step_instance(true, w, cx)))
            .on_action(cx.listener(|s, _: &PreviousInstance, w, cx| s.step_instance(false, w, cx)))
            .on_action(cx.listener(|s, _: &ToggleChat, w, cx| s.toggle_chat(w, cx)))
            .on_action(cx.listener(|s, _: &FocusChat, w, cx| s.focus_chat(w, cx)))
            .on_action(|_: &OpenChatInBrowser, _, cx| cx.open_url(CLAUDE_HOME))
            .on_action(cx.listener(|s, _: &ChatSignOut, w, cx| s.ask_chat_sign_out(w, cx)))
            .on_action(cx.listener(|s, _: &FocusSidebar, w, cx| {
                // `Mod+1`: Projects wherever it is, shown if hidden, the
                // cursor on the open page's row; in a narrow window the
                // left sidebar shows over the page.
                if s.is_workbench() {
                    s.reveal_in_tree(true, cx);
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
                layout_ui::on_resize_drag(s, handle, zoom::eff(p.x), zoom::eff(p.y), cx)
            }))
            .on_action(cx.listener(|s, _: &FocusPage, w, cx| s.focus_page(w, cx)))
            .on_action(cx.listener(|s, _: &CollapseSections, w, cx| s.fold_all(true, w, cx)))
            .on_action(cx.listener(|s, _: &ExpandSections, w, cx| s.fold_all(false, w, cx)))
            .on_action(cx.listener(|s, _: &ZoomIn, w, cx| s.zoom_in(w, cx)))
            .on_action(cx.listener(|s, _: &ZoomOut, _, cx| s.zoom_out(cx)))
            .on_action(cx.listener(|_, _: &ZoomReset, _, cx| settings::set_zoom(zoom::DEFAULT, cx)))
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
                this.border(px(1.)).border_color(tokens.border_subtle)
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

/// What the status bar leaves out at an effective width (layout.md 4.9):
/// the version first, then the plan usage meters, then the Claude Code
/// item's words (its glyph stays). The zoom item never goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GiveWay {
    pub version: bool,
    pub usage: bool,
    pub claude_words: bool,
}

pub fn status_give_way(effective_width: f32) -> GiveWay {
    // 0 is a window not yet laid out: everything shows.
    let w = if effective_width <= 0. {
        f32::MAX
    } else {
        effective_width
    };
    GiveWay {
        version: w < 960.,
        usage: w < 880.,
        claude_words: w < 800.,
    }
}

/// Options of an Acta window of the given default size, centred. `focus`
/// is false only for the test hook that must not take the foreground.
pub fn window_options(default_size: (f32, f32), focus: bool, cx: &App) -> WindowOptions {
    let px = gpui_kit::px;
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
    fn the_status_bar_gives_way_in_order() {
        assert_eq!(status_give_way(1440.), GiveWay::default());
        assert_eq!(status_give_way(0.), GiveWay::default());
        let g = status_give_way(900.);
        assert!(g.version && !g.usage && !g.claude_words);
        let g = status_give_way(860.);
        assert!(g.version && g.usage && !g.claude_words);
        assert!(status_give_way(760.).claude_words);
    }

    #[test]
    fn failures_stay_until_dismissed_and_undo_toasts_go() {
        assert!(toast_stays(ToastKind::Error));
        assert!(toast_stays(ToastKind::LayoutNoRoom));
        assert!(!toast_stays(ToastKind::Layout));
        assert!(!toast_stays(ToastKind::LayoutReset));
    }

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
