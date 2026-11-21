//! Every command the palette lists (docs/design/interaction.md 6.1: "a
//! feature without a palette command is not shipped"), with its label from
//! the catalogue and its direct shortcut, shown so the palette teaches the
//! keyboard.

use acta_core::model::Status;

use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};

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
    ToggleTerminal,
    ToggleChat,
    ChatBottom,
    ChatRight,
    ChatBrowser,
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
    Theme(crate::theme::ThemeSetting),
    Language(Locale),
}

/// What the palette needs to know about the page to offer page commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub has_project: bool,
    pub on_topic: bool,
    pub on_record: bool,
    pub can_back: bool,
    pub can_forward: bool,
    /// The chat panel is in the bottom dock.
    pub chat_bottom: bool,
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
        // components-app.md 5: on Linux there is no panel, only the link.
        if cfg!(any(windows, target_os = "macos")) {
            out.push(Command::ToggleChat);
            out.push(if ctx.chat_bottom {
                Command::ChatRight
            } else {
                Command::ChatBottom
            });
        }
        out.push(Command::ChatBrowser);
        if cfg!(any(windows, target_os = "macos")) {
            out.push(Command::ChatSignOut);
        }
        out.push(Command::NewProject);
        if ctx.can_back {
            out.push(Command::Back);
        }
        if ctx.can_forward {
            out.push(Command::Forward);
        }
        out.extend([
            Command::Undo,
            Command::ToggleSidebar,
            Command::Export,
            Command::Import,
            Command::Trash,
            Command::Snapshot,
            Command::Settings,
            Command::Theme(ThemeSetting::System),
            Command::Theme(ThemeSetting::Light),
            Command::Theme(ThemeSetting::Dark),
            Command::Language(Locale::En),
            Command::Language(Locale::Fr),
        ]);
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
            Command::ToggleTerminal => Key::CmdToggleTerminal,
            Command::ToggleChat => Key::CmdToggleChat,
            Command::ChatBottom => Key::CmdChatBottom,
            Command::ChatRight => Key::CmdChatRight,
            Command::ChatBrowser => Key::ChatBrowser,
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
            Command::ToggleTerminal => vec![KeyName::Ctrl.label(locale, macos), "`"],
            Command::ToggleChat => vec![m, shift, "H"],
            // Linux: Mod+Shift+H opens claude.ai in the browser.
            Command::ChatBrowser if cfg!(target_os = "linux") => vec![m, shift, "H"],
            Command::Settings => vec![m, ","],
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
            chat_bottom: false,
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
            chat_bottom: false,
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
        assert_eq!(
            Command::ToggleChat.shortcut(Locale::En, true),
            ["\u{2318}", "\u{21e7}", "H"]
        );
    }

    #[test]
    fn the_chat_commands_follow_the_dock_and_the_platform() {
        let right = Command::available(Context::default());
        let bottom = Command::available(Context {
            chat_bottom: true,
            ..Context::default()
        });
        assert!(right.contains(&Command::ChatBrowser));
        let panel = cfg!(any(windows, target_os = "macos"));
        assert_eq!(right.contains(&Command::ToggleChat), panel);
        assert_eq!(right.contains(&Command::ChatBottom), panel);
        assert!(!right.contains(&Command::ChatRight));
        assert_eq!(bottom.contains(&Command::ChatRight), panel);
        assert_eq!(right.contains(&Command::ChatSignOut), panel);
        assert_eq!(
            Command::ChatBrowser.label(Locale::En),
            "Open claude.ai in your browser"
        );
        assert_eq!(
            Command::ToggleChat.label(Locale::Fr),
            "Afficher ou masquer le chat"
        );
    }
}
