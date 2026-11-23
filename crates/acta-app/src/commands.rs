//! Every command the palette lists (docs/design/interaction.md 6.1: "a
//! feature without a palette command is not shipped"), with its label from
//! the catalogue and its direct shortcut, shown so the palette teaches the
//! keyboard.

use acta_core::model::Status;
use acta_core::store::OverviewFilter;

use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::layout::{Place, Side, ViewAction, ViewId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    GoTopic,
    SwitchProject,
    NewTopic,
    NewEpic,
    NewProject,
    /// "Projects: Collapse all" (layout.md 4.2.1).
    TreeCollapseAll,
    Capture,
    GoInbox,
    GoKnowledge,
    NewDocument,
    NewMemory,
    ReviewMemories,
    /// The open document's "Go to heading...", "Save as file...",
    /// "Move to..." (components-knowledge.md 5).
    GoHeading,
    SaveFile,
    KbMoveTo,
    SetStatus(Status),
    /// "Set due date..." and "Clear due date" (interaction.md 6.1): the
    /// focused tree row's record, else the open project, epic or topic.
    SetDue,
    ClearDue,
    /// The Overview's lists (components-records.md 5): "Show blocked
    /// topics" and the others, on the active page's project, or its epic.
    Show(OverviewFilter),
    /// "Overview: Show more" / "Overview: Show less".
    OverviewMore,
    OverviewLess,
    /// Questions and Report to (interaction.md 6.1).
    QuestionNew,
    QuestionsOpen,
    ShowQuestionsLate,
    ReportSet,
    ReportClear,
    Reported,
    Unreported,
    /// On the focused question row.
    Question(QuestionCommand),
    /// Contacts and the profile (interaction.md 6.1).
    ContactsOpen,
    ContactGo,
    ContactNew,
    /// On the open contact page.
    Contact(ContactCommand),
    ProfileName,
    ProfileJobTitle,
    Back,
    Forward,
    Undo,
    Redo,
    ToggleSidebar,
    ToggleRight,
    ToggleBottom,
    ToggleTerminal,
    // The chat panel exists on Windows only (ADR 0009); elsewhere the one
    // chat command opens claude.ai in the browser.
    #[cfg(windows)]
    ToggleChat,
    ChatBrowser,
    /// "{view}: Show", "{view}: Move to left sidebar" and the rest
    /// (workbench.md 4.8.7).
    View(ViewAction, ViewId),
    SplitRight,
    SplitDown,
    TabNextGroup,
    TabPreviousGroup,
    JoinGroups,
    NextArea,
    PreviousArea,
    Zoom,
    GrowArea,
    ShrinkArea,
    LayoutUndo,
    LayoutReset,
    #[cfg(windows)]
    ChatSignOut,
    RestartTerminal,
    TerminalLarger,
    TerminalSmaller,
    SetFolder,
    AddFolder,
    ShowFolders,
    /// "Switch active folder..." (interaction.md 6.1): the palette lists
    /// the folders the page sees.
    FolderSwitch,
    /// "Set commit mode..." for the page's active folder.
    CommitMode,
    /// "Folder settings..." of the page's active folder (its own).
    FolderSettings,
    /// The active folder's Pull, Fetch and branch selector (ADR 0011
    /// addendum).
    FolderPull,
    FolderFetch,
    FolderBranch,
    /// On the focused folder row of the list (interaction.md 6.1).
    Folder(FolderCommand),
    /// "Attach files..." on the editor that has (or last had) the focus,
    /// "Save file as..." on the focused file (components-attachments.md 5).
    Attach,
    AttachmentSave,
    Export,
    Import,
    Trash,
    Snapshot,
    Settings,
    Shortcuts,
    CloseTab,
    CloseOtherTabs,
    ReopenTab,
    Theme(crate::theme::ThemeSetting),
    TextSize(crate::type_scale::TextSize),
    ReadingWidth(crate::ui::ReadingWidth),
    /// Zoom (layout.md 4.9): in, out, back to 100%, or straight to a step.
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ZoomTo(u16),
    /// The trash page's toolbar (components-app.md 5, Trash page).
    TrashRestore,
    TrashDelete,
    TrashEmpty,
    Language(Locale),
    /// The Claude Code settings (components-app.md 5): each one's value.
    Claude(ClaudeSetting, bool),
    /// Dictation (components-voice.md 5): "Dictate" on the editor that has
    /// (or last had) the focus, "Stop dictating", "Voice settings".
    Dictate,
    DictateStop,
    VoiceSettings,
}

/// What the palette does to the focused question.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QuestionCommand {
    Answer,
    Edit,
    Date,
    ClearDate,
    Drop,
    Reopen,
    Move,
    Delete,
}

/// What the palette does to the focused folder of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FolderCommand {
    Active,
    Settings,
    Up,
    Down,
    Other,
    Locate,
    Copy,
    Show,
    /// "Remove folder from this topic" (or epic, or project).
    Remove(FolderLevel),
}

/// The level a page's own folders belong to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FolderLevel {
    #[default]
    Topic,
    Epic,
    Project,
}

/// The focused folder row: its own level's or inherited, active, not
/// found, and where it can move.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusedFolder {
    pub own: bool,
    pub active: bool,
    pub missing: bool,
    pub up: bool,
    pub down: bool,
    pub level: FolderLevel,
}

/// What the palette does to the open contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContactCommand {
    Rename,
    JobTitle,
    Company,
    Address,
    Move,
    Merge,
    Delete,
}

/// The focused question row: open, and whether it has an answer date.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusedQuestion {
    pub open: bool,
    pub dated: bool,
}

/// The Claude Code settings the palette can set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClaudeSetting {
    /// Continue each page's last conversation.
    Resume,
    /// Flash the taskbar when a session waits (Windows).
    Flash,
    /// Plan usage in the status bar.
    Usage,
}

/// Where a view is, for its palette commands (4.8.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewState {
    /// Its area; Inbox and Knowledge while they are pages say the page
    /// area. `None` when hidden.
    pub place: Option<Place>,
    pub visible: bool,
    /// "Split from group" applies.
    pub can_split: bool,
}

/// What the palette needs to know about the page to offer page commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub has_project: bool,
    pub on_topic: bool,
    pub on_record: bool,
    /// A document or memory is open in its reading view.
    pub on_doc: bool,
    pub can_back: bool,
    pub can_forward: bool,
    /// More than one tab is open; a closed tab can be reopened.
    pub tabs: bool,
    pub closed_tab: bool,
    /// Each view of this build, by `ViewId as usize`.
    pub views: [Option<ViewState>; ViewId::ALL.len()],
    /// The view keyboard focus is in: its commands rank first.
    pub focused_view: Option<ViewId>,
    /// The page groups, and whether "Split right" / "Split down" apply.
    pub groups: usize,
    pub can_split_right: bool,
    pub can_split_down: bool,
    /// A layout change can be undone.
    pub layout_undo: bool,
    /// The due date commands' target exists, and whether it has a date.
    pub due: Option<bool>,
    /// The page shows an Overview (a project or an epic), and whether its
    /// "More" (or "Needs attention") is open.
    pub overview: Option<bool>,
    /// The working-folder commands the open page offers.
    pub folders: FolderCommands,
    /// The open topic or epic: whether it has a person, and is reported.
    pub report: Option<(bool, bool)>,
    /// A question row has the keyboard.
    pub question: Option<FocusedQuestion>,
    /// A folder row of the list has the keyboard.
    pub folder: Option<FocusedFolder>,
    /// An editor that takes files has (or last had) the focus.
    pub attach: bool,
    /// A file of a strip has the keyboard.
    pub attachment: bool,
    /// A contact page is open.
    pub on_contact: bool,
    /// An editor that offers dictation has, or last had, the focus.
    pub dictate: bool,
    /// A take is running.
    pub dictating: bool,
    /// The applied zoom and the largest step the work area holds
    /// (layout.md 4.9); 0 when unknown (read as 100 and 200).
    pub zoom: u16,
    pub zoom_max: u16,
    /// The trash page is open: whether rows are selected, and whether
    /// the trash has anything.
    pub trash: Option<(bool, bool)>,
}

/// What the open page's folders allow (interaction.md 6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FolderCommands {
    /// The page sees a folder that exists: "Switch active folder...".
    pub switch: bool,
    /// Its active folder is its own and in git: "Set commit mode...".
    pub commit: bool,
    /// Its active folder is its own: "Folder settings...".
    pub settings: bool,
    /// Its active folder is in git: Pull, Fetch, Switch branch.
    pub git: bool,
}

/// The Overview's lists the palette opens, in the order of 6.1.
pub const SHOW: [OverviewFilter; 6] = [
    OverviewFilter::Blocked,
    OverviewFilter::Idle,
    OverviewFilter::Done,
    OverviewFilter::Decided,
    OverviewFilter::Created,
    OverviewFilter::StaleMemories,
];

/// The commands on a focused folder, in the order of interaction.md 6.1:
/// any folder can be made active (when found and not already), copied
/// and shown; an own folder also has its settings, moves, another folder
/// and removal; a folder not found has Locate.
pub fn folder_commands(f: FocusedFolder) -> Vec<FolderCommand> {
    use FolderCommand as F;
    let mut out = Vec::new();
    if !f.active && !f.missing {
        out.push(F::Active);
    }
    if f.own {
        out.push(F::Settings);
        if f.up {
            out.push(F::Up);
        }
        if f.down {
            out.push(F::Down);
        }
        out.push(F::Other);
    }
    if f.missing {
        out.push(F::Locate);
    }
    out.extend([F::Copy, F::Show]);
    if f.own {
        out.push(F::Remove(f.level));
    }
    out
}

/// The palette commands of one view, in the order of 4.8.7.
fn view_commands(view: ViewId, state: ViewState) -> Vec<Command> {
    let mut out = vec![Command::View(
        if state.visible {
            ViewAction::Hide
        } else {
            ViewAction::Show
        },
        view,
    )];
    for place in [
        Place::Side(Side::Left),
        Place::Side(Side::Right),
        Place::Side(Side::Bottom),
        Place::Page,
    ] {
        if state.place != Some(place) {
            out.push(Command::View(ViewAction::MoveTo(place), view));
        }
    }
    if state.can_split {
        out.push(Command::View(ViewAction::SplitOff, view));
    }
    out
}

impl Command {
    /// The commands available now, in the order they are listed.
    pub fn available(ctx: Context) -> Vec<Command> {
        use crate::theme::ThemeSetting;
        let mut out = vec![Command::GoTopic, Command::SwitchProject];
        if ctx.has_project {
            out.extend([
                Command::NewTopic,
                Command::NewEpic,
                Command::Capture,
                Command::GoInbox,
                Command::GoKnowledge,
                Command::ReviewMemories,
            ]);
        }
        if ctx.on_topic {
            out.extend(Status::ALL.into_iter().map(Command::SetStatus));
        }
        if let Some(dated) = ctx.due {
            out.push(Command::SetDue);
            if dated {
                out.push(Command::ClearDue);
            }
        }
        if ctx.has_project {
            out.extend(SHOW.map(Command::Show));
            out.push(Command::ShowQuestionsLate);
        }
        if ctx.on_record {
            out.push(Command::QuestionNew);
        }
        if ctx.has_project {
            out.push(Command::QuestionsOpen);
        }
        if let Some((person, reported)) = ctx.report {
            out.push(Command::ReportSet);
            if person {
                out.push(Command::ReportClear);
                out.push(if reported {
                    Command::Unreported
                } else {
                    Command::Reported
                });
            }
        }
        if let Some(q) = ctx.question {
            use QuestionCommand as Q;
            if q.open {
                out.extend([Q::Answer, Q::Edit, Q::Date].map(Command::Question));
                if q.dated {
                    out.push(Command::Question(Q::ClearDate));
                }
                out.extend([Q::Drop, Q::Move].map(Command::Question));
            } else {
                out.push(Command::Question(Q::Reopen));
            }
            out.push(Command::Question(Q::Delete));
        }
        out.extend([
            Command::ContactsOpen,
            Command::ContactGo,
            Command::ContactNew,
        ]);
        if ctx.on_contact {
            use ContactCommand as C;
            out.extend(
                [
                    C::Rename,
                    C::JobTitle,
                    C::Company,
                    C::Address,
                    C::Move,
                    C::Merge,
                    C::Delete,
                ]
                .map(Command::Contact),
            );
        }
        out.extend([Command::ProfileName, Command::ProfileJobTitle]);
        if ctx.dictate {
            out.push(Command::Dictate);
        }
        if ctx.dictating {
            out.push(Command::DictateStop);
        }
        match ctx.overview {
            Some(false) => out.push(Command::OverviewMore),
            Some(true) => out.push(Command::OverviewLess),
            None => {}
        }
        if ctx.on_record {
            out.extend([
                Command::NewDocument,
                Command::NewMemory,
                Command::ShowFolders,
                Command::AddFolder,
            ]);
            let f = ctx.folders;
            out.extend(
                [
                    (f.switch, Command::FolderSwitch),
                    (f.settings, Command::FolderSettings),
                    (f.commit, Command::CommitMode),
                    (f.git, Command::FolderPull),
                    (f.git, Command::FolderFetch),
                    (f.git, Command::FolderBranch),
                ]
                .into_iter()
                .filter_map(|(on, c)| on.then_some(c)),
            );
        }
        if let Some(f) = ctx.folder {
            out.extend(folder_commands(f).into_iter().map(Command::Folder));
        }
        if ctx.attach {
            out.push(Command::Attach);
        }
        if ctx.attachment {
            out.push(Command::AttachmentSave);
        }
        if ctx.has_project {
            out.extend([
                Command::SetFolder,
                Command::ToggleTerminal,
                Command::RestartTerminal,
                Command::TerminalLarger,
                Command::TerminalSmaller,
            ]);
        }
        // components-app.md 5: on macOS and Linux there is no panel, only
        // the link.
        #[cfg(windows)]
        out.push(Command::ToggleChat);
        out.push(Command::ChatBrowser);
        #[cfg(windows)]
        out.push(Command::ChatSignOut);
        if ctx.on_doc {
            out.extend([Command::GoHeading, Command::SaveFile, Command::KbMoveTo]);
        }
        out.push(Command::NewProject);
        if ctx.has_project {
            out.push(Command::TreeCollapseAll);
        }
        if ctx.can_back {
            out.push(Command::Back);
        }
        if ctx.can_forward {
            out.push(Command::Forward);
        }
        if ctx.tabs {
            out.extend([Command::CloseTab, Command::CloseOtherTabs]);
        }
        if ctx.closed_tab {
            out.push(Command::ReopenTab);
        }
        out.push(Command::Undo);
        out.push(Command::Redo);
        // The workbench layout (4.8): the focused view's commands first,
        // then the areas, every other view, the page groups, recovery.
        if let Some(focused) = ctx.focused_view
            && let Some(state) = ctx.views[focused as usize]
        {
            out.extend(view_commands(focused, state));
        }
        out.extend([
            Command::ToggleSidebar,
            Command::ToggleRight,
            Command::ToggleBottom,
        ]);
        for view in ViewId::ALL {
            if Some(view) != ctx.focused_view
                && let Some(state) = ctx.views[view as usize]
            {
                out.extend(view_commands(view, state));
            }
        }
        if ctx.can_split_right {
            out.push(Command::SplitRight);
        }
        if ctx.can_split_down {
            out.push(Command::SplitDown);
        }
        if ctx.groups > 1 {
            out.extend([
                Command::TabNextGroup,
                Command::TabPreviousGroup,
                Command::JoinGroups,
            ]);
        }
        out.extend([
            Command::NextArea,
            Command::PreviousArea,
            Command::Zoom,
            Command::GrowArea,
            Command::ShrinkArea,
        ]);
        if ctx.layout_undo {
            out.push(Command::LayoutUndo);
        }
        out.push(Command::LayoutReset);
        out.extend([Command::Export, Command::Import, Command::Trash]);
        if let Some((selected, any)) = ctx.trash {
            if selected {
                out.extend([Command::TrashRestore, Command::TrashDelete]);
            }
            if any {
                out.push(Command::TrashEmpty);
            }
        }
        out.extend([
            Command::Snapshot,
            Command::Settings,
            Command::VoiceSettings,
            Command::Shortcuts,
            Command::Theme(ThemeSetting::System),
            Command::Theme(ThemeSetting::Light),
            Command::Theme(ThemeSetting::Dark),
        ]);
        out.extend(crate::type_scale::TextSize::ALL.map(Command::TextSize));
        out.extend(crate::ui::ReadingWidth::ALL.map(Command::ReadingWidth));
        out.extend(zoom_commands(ctx.zoom, ctx.zoom_max));
        out.extend([Command::Language(Locale::En), Command::Language(Locale::Fr)]);
        for setting in [ClaudeSetting::Resume, ClaudeSetting::Usage] {
            out.extend([
                Command::Claude(setting, true),
                Command::Claude(setting, false),
            ]);
        }
        if cfg!(windows) {
            out.extend([
                Command::Claude(ClaudeSetting::Flash, true),
                Command::Claude(ClaudeSetting::Flash, false),
            ]);
        }
        out
    }

    pub fn label(self, locale: Locale) -> String {
        use crate::theme::ThemeSetting;
        let key = match self {
            Command::GoTopic => Key::CmdGoTopic,
            Command::SwitchProject => Key::CmdSwitchProject,
            Command::NewTopic => Key::CmdNewTopic,
            Command::NewEpic => Key::CmdNewEpic,
            Command::NewProject => Key::CmdNewProject,
            Command::TreeCollapseAll => Key::CmdTreeCollapseAll,
            Command::Capture => Key::CmdCapture,
            Command::GoInbox => Key::CmdGoInbox,
            Command::GoKnowledge => Key::CmdKbGo,
            Command::NewDocument => Key::KbNewDocument,
            Command::NewMemory => Key::KbNewMemory,
            Command::ReviewMemories => Key::CmdKbReview,
            Command::GoHeading => Key::CmdGoHeading,
            Command::SaveFile => Key::CmdSaveFile,
            Command::KbMoveTo => Key::KbMoveTo,
            Command::Back => Key::CmdBack,
            Command::Forward => Key::CmdForward,
            Command::Undo => Key::CmdUndo,
            Command::Redo => Key::CmdRedo,
            Command::ToggleSidebar => Key::CmdToggleSidebar,
            Command::ToggleRight => Key::CmdToggleRight,
            Command::ToggleBottom => Key::CmdToggleBottom,
            Command::ToggleTerminal => Key::CmdToggleTerminal,
            #[cfg(windows)]
            Command::ToggleChat => Key::CmdToggleChat,
            Command::SplitRight => Key::CmdSplitRight,
            Command::SplitDown => Key::CmdSplitDown,
            Command::TabNextGroup => Key::CmdTabNextGroup,
            Command::TabPreviousGroup => Key::CmdTabPrevGroup,
            Command::JoinGroups => Key::CmdJoinGroups,
            Command::NextArea => Key::CmdNextArea,
            Command::PreviousArea => Key::CmdPrevArea,
            Command::Zoom => Key::CmdZoom,
            Command::GrowArea => Key::CmdGrowArea,
            Command::ShrinkArea => Key::CmdShrinkArea,
            Command::LayoutUndo => Key::CmdLayoutUndo,
            Command::LayoutReset => Key::CmdLayoutReset,
            Command::View(action, view) => {
                let key = match action {
                    ViewAction::Show => Key::CmdViewShow,
                    ViewAction::Hide => Key::CmdViewHide,
                    ViewAction::MoveTo(Place::Side(Side::Left)) => Key::CmdViewLeft,
                    ViewAction::MoveTo(Place::Side(Side::Right)) => Key::CmdViewRight,
                    ViewAction::MoveTo(Place::Side(Side::Bottom)) => Key::CmdViewBottom,
                    ViewAction::MoveTo(Place::Page) => Key::CmdViewPage,
                    ViewAction::SplitOff => Key::CmdViewSplitOff,
                };
                let name = text(locale, crate::shell::view_name(view));
                return tr(locale, key, &[("view", Arg::Text(&name))]);
            }
            Command::ChatBrowser => Key::ChatBrowser,
            #[cfg(windows)]
            Command::ChatSignOut => Key::ChatSignout,
            Command::RestartTerminal => Key::CmdRestartTerminal,
            Command::TerminalLarger => Key::CmdTerminalLarger,
            Command::TerminalSmaller => Key::CmdTerminalSmaller,
            Command::SetFolder => Key::FoldersSet,
            Command::AddFolder => Key::CmdFoldersAdd,
            Command::ShowFolders => Key::CmdFoldersShow,
            Command::FolderSwitch => Key::CmdFolderSwitch,
            Command::CommitMode => Key::CmdCommitMode,
            Command::FolderSettings => Key::CmdFolderSettings,
            Command::FolderPull => Key::CmdFolderPull,
            Command::FolderFetch => Key::CmdFolderFetch,
            Command::FolderBranch => Key::CmdFolderBranch,
            Command::Folder(f) => match f {
                FolderCommand::Active => Key::CmdFolderActive,
                FolderCommand::Settings => Key::CmdFolderSettings,
                FolderCommand::Up => Key::CmdFolderUp,
                FolderCommand::Down => Key::CmdFolderDown,
                FolderCommand::Other => Key::FoldersChooseOther,
                FolderCommand::Locate => Key::CmdFolderLocate,
                FolderCommand::Copy => Key::CmdFolderCopy,
                FolderCommand::Show if cfg!(target_os = "macos") => Key::CmdFolderShowM,
                FolderCommand::Show if cfg!(windows) => Key::CmdFolderShowW,
                FolderCommand::Show => Key::CmdFolderShowL,
                FolderCommand::Remove(FolderLevel::Topic) => Key::CmdFolderRemoveTopic,
                FolderCommand::Remove(FolderLevel::Epic) => Key::CmdFolderRemoveEpic,
                FolderCommand::Remove(FolderLevel::Project) => Key::CmdFolderRemoveProject,
            },
            Command::Attach => Key::CmdAttach,
            Command::AttachmentSave => Key::CmdAttachmentSave,
            Command::Export => Key::CmdExport,
            Command::Import => Key::CmdImport,
            Command::Trash => Key::CmdTrash,
            Command::TrashRestore => Key::CmdTrashRestore,
            Command::TrashDelete => Key::CmdTrashDelete,
            Command::TrashEmpty => Key::CmdTrashEmpty,
            Command::ZoomIn => Key::CmdZoomIn,
            Command::ZoomOut => Key::CmdZoomOut,
            Command::ZoomReset => Key::CmdZoomReset,
            Command::Snapshot => Key::CmdSnapshot,
            Command::Settings => Key::CmdSettings,
            Command::Shortcuts => Key::CmdShortcuts,
            Command::Dictate => Key::CmdDictate,
            Command::DictateStop => Key::CmdDictateStop,
            Command::VoiceSettings => Key::CmdVoiceSettings,
            Command::Claude(ClaudeSetting::Resume, true) => Key::CmdResumeOn,
            Command::Claude(ClaudeSetting::Resume, false) => Key::CmdResumeOff,
            Command::Claude(ClaudeSetting::Flash, true) => Key::CmdFlashOn,
            Command::Claude(ClaudeSetting::Flash, false) => Key::CmdFlashOff,
            Command::Claude(ClaudeSetting::Usage, true) => Key::CmdUsageOn,
            Command::Claude(ClaudeSetting::Usage, false) => Key::CmdUsageOff,
            Command::CloseTab => Key::TabClose,
            Command::CloseOtherTabs => Key::TabCloseOthers,
            Command::ReopenTab => Key::TabReopen,
            Command::SetStatus(status) => {
                let name = text(locale, crate::ui::status_key(status));
                return tr(locale, Key::CmdSetStatus, &[("status", Arg::Text(&name))]);
            }
            Command::SetDue => Key::CmdDueSet,
            Command::ClearDue => Key::CmdDueClear,
            Command::Show(filter) => match filter {
                OverviewFilter::Blocked => Key::CmdShowBlocked,
                OverviewFilter::Idle => Key::CmdShowIdle,
                OverviewFilter::Done => Key::CmdShowDoneWeek,
                OverviewFilter::Decided => Key::CmdShowDecidedWeek,
                OverviewFilter::Created => Key::CmdShowCreatedWeek,
                OverviewFilter::StaleMemories => Key::CmdShowStaleMemories,
                // Overdue keeps the table's segment, the inbox "Go to
                // inbox": never listed (SHOW).
                OverviewFilter::Overdue | OverviewFilter::Inbox => Key::CmdGoInbox,
            },
            Command::OverviewMore => Key::CmdOverviewMore,
            Command::OverviewLess => Key::CmdOverviewLess,
            Command::QuestionNew => Key::CmdQuestionNew,
            Command::QuestionsOpen => Key::CmdQuestionsOpen,
            Command::ShowQuestionsLate => Key::CmdShowQuestionsLate,
            Command::ReportSet => Key::CmdReportSet,
            Command::ReportClear => Key::CmdReportClear,
            Command::Reported => Key::CmdReported,
            Command::Unreported => Key::CmdUnreported,
            Command::Question(q) => match q {
                QuestionCommand::Answer => Key::CmdQuestionAnswer,
                QuestionCommand::Edit => Key::CmdQuestionEdit,
                QuestionCommand::Date => Key::CmdQuestionDate,
                QuestionCommand::ClearDate => Key::CmdQuestionDateClear,
                QuestionCommand::Drop => Key::CmdQuestionDrop,
                QuestionCommand::Reopen => Key::CmdQuestionReopen,
                QuestionCommand::Move => Key::CmdQuestionMove,
                QuestionCommand::Delete => Key::CmdQuestionDelete,
            },
            Command::ContactsOpen => Key::CmdContactsOpen,
            Command::ContactGo => Key::CmdContactGo,
            Command::ContactNew => Key::CmdContactNew,
            Command::Contact(c) => match c {
                ContactCommand::Rename => Key::CmdContactRename,
                ContactCommand::JobTitle => Key::CmdContactJobTitle,
                ContactCommand::Company => Key::CmdContactCompany,
                ContactCommand::Address => Key::CmdContactAddress,
                ContactCommand::Move => Key::CmdContactMove,
                ContactCommand::Merge => Key::CmdContactMerge,
                ContactCommand::Delete => Key::CmdContactDelete,
            },
            Command::ProfileName => Key::CmdProfileName,
            Command::ProfileJobTitle => Key::CmdProfileJobTitle,
            Command::Theme(setting) => {
                let value = text(
                    locale,
                    match setting {
                        ThemeSetting::System => Key::SettingsThemeSystem,
                        ThemeSetting::Light => Key::SettingsThemeLight,
                        ThemeSetting::Dark => Key::SettingsThemeDark,
                    },
                );
                return tr(locale, Key::CmdTheme, &[("value", Arg::Text(&value))]);
            }
            Command::TextSize(size) => {
                let value = text(locale, text_size_key(size));
                return tr(locale, Key::CmdTextSize, &[("value", Arg::Text(&value))]);
            }
            Command::ReadingWidth(width) => {
                let value = text(locale, reading_width_key(width));
                return tr(
                    locale,
                    Key::CmdReadingWidth,
                    &[("value", Arg::Text(&value))],
                );
            }
            Command::ZoomTo(step) => {
                let value = crate::i18n::percent(locale, u64::from(step));
                return tr(locale, Key::CmdZoomTo, &[("value", Arg::Text(&value))]);
            }
            Command::Language(target) => {
                return tr(
                    locale,
                    Key::CmdLanguage,
                    &[("value", Arg::Text(target.native_name()))],
                );
            }
        };
        text(locale, key)
    }

    /// The direct shortcut, as kbd keys for this platform and language.
    pub fn shortcut(self, locale: Locale, macos: bool) -> Vec<&'static str> {
        let m = KeyName::Mod.label(locale, macos);
        let shift = KeyName::Shift.label(locale, macos);
        let alt = KeyName::Alt.label(locale, macos);
        match self {
            Command::GoTopic => vec![m, "P"],
            Command::SwitchProject => vec![m, "O"],
            Command::NewTopic => vec![m, "T"],
            Command::Capture => vec![m, "N"],
            Command::GoInbox => vec![m, "I"],
            Command::GoKnowledge => vec![m, "L"],
            Command::NewDocument => vec![m, shift, "D"],
            Command::NewMemory => vec![m, shift, "M"],
            Command::ToggleSidebar => vec![m, "B"],
            Command::ToggleRight => vec![m, shift, "B"],
            Command::ToggleBottom => vec![m, "J"],
            Command::SplitRight => vec![m, "\\"],
            Command::NextArea => vec!["F6"],
            Command::PreviousArea => vec![shift, "F6"],
            Command::Zoom => vec![m, shift, KeyName::Enter.label(locale, macos)],
            Command::ToggleTerminal => vec![KeyName::Ctrl.label(locale, macos), "`"],
            #[cfg(windows)]
            Command::ToggleChat => vec![m, shift, "H"],
            // macOS and Linux: Mod+Shift+H opens claude.ai in the browser.
            #[cfg(not(windows))]
            Command::ChatBrowser => vec![m, shift, "H"],
            Command::Settings => vec![m, ","],
            Command::Shortcuts => vec![m, "/"],
            Command::Dictate | Command::DictateStop => vec![m, shift, "Space"],
            Command::CloseTab => vec![m, "W"],
            // components-app.md 5, Settings: never Ctrl+- (Claude Code's
            // undo); Ctrl+Alt on Windows and Linux, Cmd+Shift on macOS.
            Command::TerminalLarger if macos => vec![m, shift, "="],
            Command::TerminalSmaller if macos => vec![m, shift, "-"],
            Command::TerminalLarger => vec![KeyName::Ctrl.label(locale, macos), alt, "="],
            Command::TerminalSmaller => vec![KeyName::Ctrl.label(locale, macos), alt, "-"],
            Command::ZoomIn => vec![m, "="],
            Command::ZoomOut => vec![m, "-"],
            Command::ZoomReset => vec![m, "0"],
            Command::TrashRestore => vec!["R"],
            Command::TrashDelete => vec![KeyName::Delete.label(locale, macos)],
            Command::Undo => vec![m, "Z"],
            Command::Redo => vec![m, shift, "Z"],
            Command::GoHeading => vec![m, shift, "O"],
            Command::Back if macos => vec![m, "["],
            Command::Forward if macos => vec![m, "]"],
            Command::Back => vec![alt, "\u{2190}"],
            Command::Forward => vec![alt, "\u{2192}"],
            Command::SetStatus(_) => vec!["S"],
            Command::SetDue => vec!["D"],
            Command::QuestionNew => vec!["Q"],
            Command::ReportSet => vec!["R"],
            Command::Question(QuestionCommand::Answer) => vec!["A"],
            Command::Question(QuestionCommand::Edit) => vec!["E"],
            Command::Question(QuestionCommand::Date) => vec!["D"],
            Command::Question(QuestionCommand::Move) => vec!["M"],
            Command::Question(QuestionCommand::Delete) => {
                vec![KeyName::Delete.label(locale, macos)]
            }
            Command::Folder(FolderCommand::Active) => vec!["Space"],
            Command::Folder(FolderCommand::Settings) => vec!["F2"],
            Command::Folder(FolderCommand::Up) => vec![alt, "\u{2191}"],
            Command::Folder(FolderCommand::Down) => vec![alt, "\u{2193}"],
            Command::Folder(FolderCommand::Other) => vec!["E"],
            Command::Folder(FolderCommand::Copy) => vec![m, "C"],
            Command::Folder(FolderCommand::Remove(_)) => {
                vec![KeyName::Delete.label(locale, macos)]
            }
            Command::Attach => vec![m, shift, "A"],
            Command::Contact(ContactCommand::Rename) => vec!["F2"],
            Command::Contact(ContactCommand::Move) => vec!["M"],
            _ => vec![],
        }
    }
}

/// The zoom commands of layout.md 4.9: "Zoom out" not at the floor,
/// "Zoom in" not at the largest step that fits, "Reset zoom" not at 100%,
/// and "Zoom: 150%" for each step that fits.
pub fn zoom_commands(applied: u16, max: u16) -> Vec<Command> {
    use crate::zoom::{DEFAULT, STEPS};
    let applied = if applied == 0 { DEFAULT } else { applied };
    let max = if max == 0 {
        STEPS[STEPS.len() - 1]
    } else {
        max
    };
    let mut out = Vec::new();
    if applied < max {
        out.push(Command::ZoomIn);
    }
    if applied > STEPS[0] {
        out.push(Command::ZoomOut);
    }
    if applied != DEFAULT {
        out.push(Command::ZoomReset);
    }
    out.extend(
        STEPS
            .iter()
            .filter(|s| **s <= max)
            .map(|s| Command::ZoomTo(*s)),
    );
    out
}

/// The name of a Text size value, in Settings and in the palette.
pub fn text_size_key(size: crate::type_scale::TextSize) -> Key {
    use crate::type_scale::TextSize;
    match size {
        TextSize::Small => Key::SettingsTextSizeSmall,
        TextSize::Default => Key::SettingsTextSizeDefault,
        TextSize::Large => Key::SettingsTextSizeLarge,
    }
}

/// The name of a Reading width value, in Settings and in the palette.
pub fn reading_width_key(width: crate::ui::ReadingWidth) -> Key {
    use crate::ui::ReadingWidth;
    match width {
        ReadingWidth::Narrow => Key::SettingsReadingWidthNarrow,
        ReadingWidth::Default => Key::SettingsReadingWidthDefault,
        ReadingWidth::Wide => Key::SettingsReadingWidthWide,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictation_commands_follow_the_editor_and_the_take() {
        let none = Command::available(Context::default());
        assert!(!none.contains(&Command::Dictate));
        assert!(!none.contains(&Command::DictateStop));
        assert!(none.contains(&Command::VoiceSettings));
        let editor = Command::available(Context {
            dictate: true,
            ..Context::default()
        });
        assert!(editor.contains(&Command::Dictate));
        let take = Command::available(Context {
            dictate: true,
            dictating: true,
            ..Context::default()
        });
        assert!(take.contains(&Command::DictateStop));
        assert_eq!(Command::Dictate.label(Locale::Fr), "Dicter");
        assert_eq!(
            Command::Dictate.shortcut(Locale::En, false),
            ["Ctrl", "Shift", "Space"]
        );
        assert_eq!(Command::VoiceSettings.label(Locale::En), "Voice settings");
    }

    #[test]
    fn a_focused_folder_offers_what_its_keys_do() {
        let inherited = FocusedFolder {
            own: false,
            ..FocusedFolder::default()
        };
        assert_eq!(
            folder_commands(inherited),
            [
                FolderCommand::Active,
                FolderCommand::Copy,
                FolderCommand::Show
            ]
        );
        let own = FocusedFolder {
            own: true,
            active: true,
            up: true,
            level: FolderLevel::Epic,
            ..FocusedFolder::default()
        };
        assert_eq!(
            folder_commands(own),
            [
                FolderCommand::Settings,
                FolderCommand::Up,
                FolderCommand::Other,
                FolderCommand::Copy,
                FolderCommand::Show,
                FolderCommand::Remove(FolderLevel::Epic),
            ]
        );
        let missing = FocusedFolder {
            missing: true,
            ..FocusedFolder::default()
        };
        assert!(folder_commands(missing).contains(&FolderCommand::Locate));
        assert!(!folder_commands(missing).contains(&FolderCommand::Active));
        let listed = Command::available(Context {
            on_record: true,
            folder: Some(own),
            ..Context::default()
        });
        assert!(listed.contains(&Command::Folder(FolderCommand::Remove(FolderLevel::Epic))));
        assert_eq!(
            Command::Folder(FolderCommand::Remove(FolderLevel::Epic)).label(Locale::En),
            "Remove folder from this epic"
        );
        assert!(
            !Command::available(Context::default())
                .iter()
                .any(|c| matches!(c, Command::Folder(_)))
        );
    }

    #[test]
    fn attachment_commands_follow_the_editor_and_the_file() {
        let none = Command::available(Context::default());
        assert!(!none.contains(&Command::Attach));
        assert!(!none.contains(&Command::AttachmentSave));
        let both = Command::available(Context {
            attach: true,
            attachment: true,
            ..Context::default()
        });
        assert!(both.contains(&Command::Attach));
        assert!(both.contains(&Command::AttachmentSave));
        assert_eq!(Command::Attach.label(Locale::En), "Attach files\u{2026}");
        assert_eq!(
            Command::AttachmentSave.label(Locale::En),
            "Save file as\u{2026}"
        );
        assert_eq!(
            Command::Attach.shortcut(Locale::En, false),
            ["Ctrl", "Shift", "A"]
        );
    }

    #[test]
    fn page_commands_appear_only_where_they_act() {
        let none = Command::available(Context::default());
        assert!(!none.contains(&Command::NewTopic));
        assert!(none.contains(&Command::NewProject));
        assert!(!none.contains(&Command::Back));
        let topic = Command::available(Context {
            has_project: true,
            on_topic: true,
            on_record: true,
            can_back: true,
            can_forward: false,
            ..Context::default()
        });
        assert!(topic.contains(&Command::SetStatus(Status::Blocked)));
        assert!(topic.contains(&Command::Back));
        assert!(topic.contains(&Command::TreeCollapseAll));
        assert!(
            !none.contains(&Command::TreeCollapseAll),
            "no project, no tree"
        );
        assert_eq!(
            Command::TreeCollapseAll.label(Locale::En),
            "Projects: Collapse all"
        );
        assert!(!topic.contains(&Command::Forward));
    }

    #[test]
    fn due_and_overview_commands_appear_only_with_their_target() {
        let page = |due, overview| {
            Command::available(Context {
                has_project: true,
                on_record: true,
                due,
                overview,
                ..Context::default()
            })
        };
        let undated = page(Some(false), None);
        assert!(undated.contains(&Command::SetDue));
        assert!(!undated.contains(&Command::ClearDue), "nothing to clear");
        assert!(page(Some(true), None).contains(&Command::ClearDue));
        let none = Command::available(Context::default());
        assert!(!none.contains(&Command::SetDue));
        assert!(!none.contains(&Command::Show(OverviewFilter::Blocked)));
        let project = page(Some(false), Some(false));
        assert!(project.contains(&Command::Show(OverviewFilter::StaleMemories)));
        assert!(project.contains(&Command::OverviewMore));
        assert!(!project.contains(&Command::OverviewLess));
        assert!(page(None, Some(true)).contains(&Command::OverviewLess));
        assert_eq!(Command::SetDue.label(Locale::En), "Set due date\u{2026}");
        assert_eq!(
            Command::ClearDue.label(Locale::Fr),
            "Effacer l\u{2019}\u{e9}ch\u{e9}ance"
        );
        assert_eq!(
            Command::Show(OverviewFilter::Idle).label(Locale::En),
            "Show topics with no activity"
        );
        assert_eq!(
            Command::OverviewMore.label(Locale::Fr),
            "Vue d\u{2019}ensemble\u{a0}: afficher plus"
        );
        assert_eq!(Command::SetDue.shortcut(Locale::En, false), ["D"]);
    }

    #[test]
    fn labels_come_from_the_catalogue_in_both_languages() {
        assert_eq!(
            Command::SetStatus(Status::Blocked).label(Locale::En),
            "Set status: Blocked"
        );
        assert_eq!(
            Command::Theme(crate::theme::ThemeSetting::Dark).label(Locale::En),
            "Theme: Dark"
        );
        for command in Command::available(Context {
            has_project: true,
            on_topic: true,
            on_record: true,
            can_back: true,
            can_forward: true,
            due: Some(true),
            overview: Some(false),
            ..Context::default()
        }) {
            for locale in Locale::ALL {
                let label = command.label(locale);
                assert!(!label.is_empty() && !label.contains('{'), "{command:?}");
            }
        }
    }

    #[test]
    fn question_report_and_contact_commands_follow_their_target() {
        let page = |report, question, on_contact| {
            Command::available(Context {
                has_project: true,
                on_record: true,
                report,
                question,
                on_contact,
                ..Context::default()
            })
        };
        let plain = page(None, None, false);
        assert!(plain.contains(&Command::QuestionNew));
        assert!(plain.contains(&Command::ContactGo));
        assert!(
            !plain.contains(&Command::ReportSet),
            "a project has no Report to"
        );
        let unset = page(Some((false, false)), None, false);
        assert!(unset.contains(&Command::ReportSet));
        assert!(!unset.contains(&Command::ReportClear));
        let set = page(Some((true, false)), None, false);
        assert!(set.contains(&Command::Reported) && !set.contains(&Command::Unreported));
        let open = page(
            None,
            Some(FocusedQuestion {
                open: true,
                dated: false,
            }),
            false,
        );
        assert!(open.contains(&Command::Question(QuestionCommand::Answer)));
        assert!(!open.contains(&Command::Question(QuestionCommand::ClearDate)));
        assert!(!open.contains(&Command::Question(QuestionCommand::Reopen)));
        let closed = page(None, Some(FocusedQuestion::default()), false);
        assert!(closed.contains(&Command::Question(QuestionCommand::Reopen)));
        assert!(!closed.contains(&Command::Question(QuestionCommand::Answer)));
        assert!(page(None, None, true).contains(&Command::Contact(ContactCommand::Merge)));
        assert!(!plain.contains(&Command::Contact(ContactCommand::Merge)));
        assert_eq!(
            Command::ContactGo.label(Locale::En),
            "Go to contact\u{2026}"
        );
        assert_eq!(
            Command::Question(QuestionCommand::Answer).shortcut(Locale::En, false),
            ["A"]
        );
    }

    #[test]
    fn shortcuts_name_the_platform_keys() {
        assert_eq!(Command::NewTopic.shortcut(Locale::En, false), ["Ctrl", "T"]);
        assert_eq!(
            Command::NewDocument.shortcut(Locale::Fr, false),
            ["Ctrl", "Maj", "D"]
        );
        assert_eq!(Command::Back.shortcut(Locale::En, true), ["\u{2318}", "["]);
    }

    #[test]
    fn every_platform_has_the_browser_link() {
        let commands = Command::available(Context::default());
        assert!(commands.contains(&Command::ChatBrowser));
        assert_eq!(
            Command::ChatBrowser.label(Locale::En),
            "Open claude.ai in your browser"
        );
    }

    #[test]
    fn view_commands_follow_where_each_view_is() {
        let mut views = [None; ViewId::ALL.len()];
        views[ViewId::Terminal as usize] = Some(ViewState {
            place: Some(Place::Side(Side::Right)),
            visible: true,
            can_split: true,
        });
        views[ViewId::Projects as usize] = Some(ViewState {
            place: None,
            visible: false,
            can_split: false,
        });
        let commands = Command::available(Context {
            views,
            focused_view: Some(ViewId::Terminal),
            ..Context::default()
        });
        let terminal = |a| Command::View(a, ViewId::Terminal);
        assert!(commands.contains(&terminal(ViewAction::Hide)));
        assert!(!commands.contains(&terminal(ViewAction::Show)));
        assert!(!commands.contains(&terminal(ViewAction::MoveTo(Place::Side(Side::Right)))));
        assert!(commands.contains(&terminal(ViewAction::MoveTo(Place::Side(Side::Bottom)))));
        assert!(commands.contains(&terminal(ViewAction::SplitOff)));
        assert!(commands.contains(&Command::View(ViewAction::Show, ViewId::Projects)));
        // The focused view's commands rank before the others.
        let first_terminal = commands
            .iter()
            .position(|c| matches!(c, Command::View(_, ViewId::Terminal)));
        let first_projects = commands
            .iter()
            .position(|c| matches!(c, Command::View(_, ViewId::Projects)));
        assert!(first_terminal < first_projects);
        assert!(commands.contains(&Command::LayoutReset));
        assert!(!commands.contains(&Command::JoinGroups), "one group");
        assert_eq!(
            terminal(ViewAction::MoveTo(Place::Side(Side::Bottom))).label(Locale::Fr),
            "Terminal\u{a0}: déplacer dans le panneau du bas"
        );
        assert_eq!(
            Command::View(ViewAction::Show, ViewId::Projects).label(Locale::En),
            "Projects: Show"
        );
        assert_eq!(
            Command::ToggleRight.shortcut(Locale::En, false),
            ["Ctrl", "Shift", "B"]
        );
        assert_eq!(
            Command::SplitRight.shortcut(Locale::En, true),
            ["\u{2318}", "\\"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_chat_commands_exist_on_windows() {
        let right = Command::available(Context::default());
        assert!(right.contains(&Command::ToggleChat));
        assert!(right.contains(&Command::ChatSignOut));
        assert_eq!(
            Command::ToggleChat.label(Locale::Fr),
            "Afficher ou masquer le chat"
        );
        assert_eq!(
            Command::ToggleChat.shortcut(Locale::En, false),
            ["Ctrl", "Shift", "H"]
        );
    }

    #[test]
    fn zoom_commands_hide_what_does_nothing() {
        let c = zoom_commands(100, 150);
        assert!(c.contains(&Command::ZoomIn));
        assert!(c.contains(&Command::ZoomOut));
        assert!(!c.contains(&Command::ZoomReset));
        assert!(!c.contains(&Command::ZoomTo(175)));
        assert!(c.contains(&Command::ZoomTo(150)));
        let c = zoom_commands(150, 150);
        assert!(!c.contains(&Command::ZoomIn));
        assert!(c.contains(&Command::ZoomReset));
        let c = zoom_commands(90, 200);
        assert!(!c.contains(&Command::ZoomOut));
        assert_eq!(
            Command::ZoomTo(150).label(Locale::Fr),
            "Zoom\u{a0}: 150\u{202f}%"
        );
    }
}
