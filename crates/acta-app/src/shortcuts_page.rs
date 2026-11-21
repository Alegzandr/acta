//! The Keyboard shortcuts page (docs/design/components-app.md 5): `Mod+/`,
//! the palette, or About in Settings. A filter input, focused on open,
//! then the shortcuts grouped under `type.label` headings, rows of 28 with
//! the keys as kbd hints. Read-only: no rebinding in v1.
//!
//! Rows are the palette commands that have a direct key, with the labels
//! and keys the palette shows. The Lists and tree and Editors groups of
//! 6.1 have no catalogue labels per key yet and are left out.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, div, px,
};

use crate::commands::Command;
use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::record_page::PageEvent;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;

/// The groups of the page, in order, with their commands.
fn groups() -> Vec<(Key, Vec<Command>)> {
    vec![
        (
            Key::ShortcutsGroupGeneral,
            vec![
                Command::GoTopic,
                Command::SwitchProject,
                Command::NewTopic,
                Command::AddEntry,
                Command::Capture,
                Command::GoInbox,
                Command::GoKnowledge,
                Command::NewDocument,
                Command::NewMemory,
                Command::Back,
                Command::Forward,
                Command::Undo,
                Command::Settings,
                Command::Shortcuts,
            ],
        ),
        (
            Key::ShortcutsGroupTabs,
            vec![Command::ToggleSidebar, Command::ToggleTerminal],
        ),
        (Key::ShortcutsGroupTerminal, terminal_and_chat()),
    ]
}

fn terminal_and_chat() -> Vec<Command> {
    let mut out = vec![Command::TerminalLarger, Command::TerminalSmaller];
    #[cfg(windows)]
    out.push(Command::ToggleChat);
    #[cfg(not(windows))]
    out.push(Command::ChatBrowser);
    out
}

/// The rows of `group` whose label matches `query` (case-insensitive,
/// anywhere in the label).
pub fn matching(commands: &[Command], query: &str, locale: Locale) -> Vec<Command> {
    let query = query.trim().to_lowercase();
    commands
        .iter()
        .copied()
        .filter(|c| query.is_empty() || c.label(locale).to_lowercase().contains(&query))
        .collect()
}

pub struct ShortcutsPage {
    filter: Entity<InputState>,
    _sub: Subscription,
}

impl EventEmitter<PageEvent> for ShortcutsPage {}

impl ShortcutsPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let locale = settings::locale(cx);
        let filter = cx
            .new(|cx| InputState::new(window, cx).placeholder(text(locale, Key::ShortcutsFilter)));
        // The filter is focused on open.
        filter.update(cx, |state, cx| state.focus(window, cx));
        let sub = cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                cx.notify();
            }
        });
        Self { filter, _sub: sub }
    }
}

impl Render for ShortcutsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let query = self.filter.read(cx).value().to_string();
        let mut column = ui::column()
            .child(
                type_scale::TITLE
                    .el()
                    .text_color(t.text_primary)
                    .child(text(locale, Key::CmdShortcuts)),
            )
            .child(
                div()
                    .mt(px(16.))
                    .h(px(28.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS_SM))
                    .bg(t.bg_raised)
                    .border_1()
                    .border_color(t.border_strong)
                    .child(
                        Input::new(&self.filter)
                            .appearance(false)
                            .px(px(0.))
                            .text_size(px(type_scale::UI.size)),
                    ),
            );
        let mut any = false;
        for (group, commands) in groups() {
            let rows = matching(&commands, &query, locale);
            if rows.is_empty() {
                continue;
            }
            any = true;
            column = column.child(
                type_scale::LABEL
                    .el()
                    .mt(px(24.))
                    .mb(px(4.))
                    .text_color(t.text_secondary)
                    .child(text(locale, group)),
            );
            for command in rows {
                column = column.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .h(px(28.))
                        .child(
                            type_scale::UI
                                .el()
                                .text_color(t.text_primary)
                                .child(command.label(locale)),
                        )
                        .child(ui::kbd(&command.shortcut(locale, macos), &t)),
                );
            }
        }
        if !any {
            column = column.child(
                type_scale::READING
                    .el()
                    .mt(px(24.))
                    .text_color(t.text_secondary)
                    .child(tr(
                        locale,
                        Key::ShortcutsNone,
                        &[("query", Arg::Text(query.trim()))],
                    )),
            );
        }
        ui::page_scroll("shortcuts-page").child(column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_command_has_a_key_and_the_filter_matches_labels() {
        for (_, commands) in groups() {
            for command in commands {
                assert!(
                    !command.shortcut(Locale::En, false).is_empty(),
                    "{command:?} has no key"
                );
            }
        }
        let all: Vec<Command> = groups().into_iter().flat_map(|(_, c)| c).collect();
        let hits = matching(&all, "TERMINAL", Locale::En);
        assert!(hits.contains(&Command::ToggleTerminal));
        assert!(!hits.contains(&Command::NewTopic));
        assert!(matching(&all, "zzz", Locale::En).is_empty());
    }
}
