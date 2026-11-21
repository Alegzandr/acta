//! Every command the palette lists (docs/design/interaction.md 6.1: "a
//! feature without a palette command is not shipped"), with its label from
//! the catalogue and its direct shortcut, shown so the palette teaches the
//! keyboard.

use acta_core::model::Status;

use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::layout::{Place, Side, ViewAction, ViewId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    GoTopic,
    SwitchProject,
    NewTopic,
    NewEpic,
    NewProject,
    AddEntry,
    Capture,
    GoInbox,
    GoKnowledge,
    NewDocument,
    NewMemory,
    ReviewMemories,
    SetStatus(Status),
    Back,
    Forward,
    Undo,
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
    Language(Locale),
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
            out.push(Command::AddEntry);
            out.extend(Status::ALL.into_iter().map(Command::SetStatus));
        }
        if ctx.on_record {
            out.extend([
                Command::NewDocument,
                Command::NewMemory,
                Command::ShowFolders,
                Command::AddFolder,
            ]);
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
        out.push(Command::NewProject);
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
        out.extend([
            Command::Export,
            Command::Import,
            Command::Trash,
            Command::Snapshot,
            Command::Settings,
            Command::Shortcuts,
            Command::Theme(ThemeSetting::System),
            Command::Theme(ThemeSetting::Light),
            Command::Theme(ThemeSetting::Dark),
        ]);
        out.extend(crate::type_scale::TextSize::ALL.map(Command::TextSize));
        out.extend(crate::ui::ReadingWidth::ALL.map(Command::ReadingWidth));
        out.extend([Command::Language(Locale::En), Command::Language(Locale::Fr)]);
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
            Command::AddEntry => Key::CmdAddEntry,
            Command::Capture => Key::CmdCapture,
            Command::GoInbox => Key::CmdGoInbox,
            Command::GoKnowledge => Key::CmdKbGo,
            Command::NewDocument => Key::KbNewDocument,
            Command::NewMemory => Key::KbNewMemory,
            Command::ReviewMemories => Key::CmdKbReview,
            Command::Back => Key::CmdBack,
            Command::Forward => Key::CmdForward,
            Command::Undo => Key::CmdUndo,
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
            Command::Export => Key::CmdExport,
            Command::Import => Key::CmdImport,
            Command::Trash => Key::CmdTrash,
            Command::Snapshot => Key::CmdSnapshot,
            Command::Settings => Key::CmdSettings,
            Command::Shortcuts => Key::CmdShortcuts,
            Command::CloseTab => Key::TabClose,
            Command::CloseOtherTabs => Key::TabCloseOthers,
            Command::ReopenTab => Key::TabReopen,
            Command::SetStatus(status) => {
                let name = text(locale, crate::ui::status_key(status));
                return tr(locale, Key::CmdSetStatus, &[("status", Arg::Text(&name))]);
            }
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
            Command::AddEntry => vec![m, "E"],
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
            Command::CloseTab => vec![m, "W"],
            // components-app.md 5, Settings: never Ctrl+- (Claude Code's
            // undo); Ctrl+Alt on Windows and Linux, Cmd+Shift on macOS.
            Command::TerminalLarger if macos => vec![m, shift, "="],
            Command::TerminalSmaller if macos => vec![m, shift, "-"],
            Command::TerminalLarger => vec![KeyName::Ctrl.label(locale, macos), alt, "="],
            Command::TerminalSmaller => vec![KeyName::Ctrl.label(locale, macos), alt, "-"],
            Command::Undo => vec![m, "Z"],
            Command::Back if macos => vec![m, "["],
            Command::Forward if macos => vec![m, "]"],
            Command::Back => vec![alt, "\u{2190}"],
            Command::Forward => vec![alt, "\u{2192}"],
            Command::SetStatus(_) => vec!["S"],
            _ => vec![],
        }
    }
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
        assert!(!topic.contains(&Command::Forward));
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
            ..Context::default()
        }) {
            for locale in Locale::ALL {
                let label = command.label(locale);
                assert!(!label.is_empty() && !label.contains('{'), "{command:?}");
            }
        }
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
            "Terminal\u{202f}: déplacer dans le panneau du bas"
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
}
