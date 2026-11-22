//! The Keyboard shortcuts page (docs/design/components-app.md 5): `Mod+/`,
//! the palette, or About in Settings. A filter input, focused on open,
//! then the shortcuts of interaction.md 6.1 grouped under `type.label`
//! headings, rows of 28 with the keys as kbd hints. Read-only: no
//! rebinding in v1.
//!
//! Rows are the palette commands with a direct key, with the labels and
//! keys the palette shows, and the keys that have no command (tabs, focus,
//! lists and the tree, editors). Keys that do nothing yet are left out
//! (entry types in the composer), except the area keys the spec lists as
//! reserved.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, div, px,
};

use crate::commands::Command;
use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::record_page::PageEvent;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;

/// One row: the action and its key combinations, alternatives in order
/// ("Ctrl+K", also "Ctrl+Shift+K").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub keys: Vec<Vec<&'static str>>,
}

fn command(c: Command, locale: Locale, macos: bool) -> Row {
    Row {
        label: c.label(locale),
        keys: vec![c.shortcut(locale, macos)],
    }
}

fn row(locale: Locale, key: Key, keys: Vec<Vec<&'static str>>) -> Row {
    Row {
        label: text(locale, key),
        keys,
    }
}

/// The groups of the page, in order, with their rows.
pub fn groups(locale: Locale, macos: bool) -> Vec<(Key, Vec<Row>)> {
    let m = KeyName::Mod.label(locale, macos);
    let ctrl = KeyName::Ctrl.label(locale, macos);
    let shift = KeyName::Shift.label(locale, macos);
    let alt = KeyName::Alt.label(locale, macos);
    let enter = KeyName::Enter.label(locale, macos);
    let esc = KeyName::Esc.label(locale, macos);
    let tab = KeyName::Tab.label(locale, macos);
    let delete = KeyName::Delete.label(locale, macos);
    let (up, down, left, right) = ("\u{2191}", "\u{2193}", "\u{2190}", "\u{2192}");
    let c = |c: Command| command(c, locale, macos);
    let r = |key: Key, keys: Vec<Vec<&'static str>>| row(locale, key, keys);

    let mut general = vec![r(
        Key::ShortcutsPalette,
        vec![vec![m, "K"], vec![m, shift, "K"]],
    )];
    general.extend(
        [
            Command::GoTopic,
            Command::SwitchProject,
            Command::NewTopic,
            Command::AddEntry,
            Command::Capture,
            Command::GoInbox,
            Command::GoKnowledge,
            Command::NewDocument,
            Command::NewMemory,
            Command::GoHeading,
            Command::Back,
            Command::Forward,
            Command::Undo,
        ]
        .map(c),
    );
    // interaction.md 6.1: Ctrl+Y as well on Windows and Linux.
    let mut redo = c(Command::Redo);
    if !macos {
        redo.keys.push(vec![ctrl, "Y"]);
    }
    general.push(redo);
    general.extend([Command::Settings, Command::Shortcuts].map(c));

    let mut tabs = vec![
        r(Key::TabOpenNew, vec![vec![m, enter]]),
        c(Command::CloseTab),
        r(Key::ShortcutsNextTab, vec![vec![ctrl, tab]]),
        r(Key::ShortcutsPreviousTab, vec![vec![ctrl, shift, tab]]),
        c(Command::ToggleSidebar),
        c(Command::ToggleRight),
        c(Command::ToggleBottom),
        c(Command::SplitRight),
        Row {
            label: Command::ToggleTerminal.label(locale),
            keys: vec![vec![ctrl, "`"], vec![m, shift, "J"]],
        },
        r(Key::ShortcutsFocusProjects, vec![vec![m, "1"]]),
        r(Key::ShortcutsFocusPage, vec![vec![m, "2"]]),
        r(Key::ShortcutsFocusTerminal, vec![vec![m, "3"]]),
    ];
    // interaction.md 6.1: only the Windows panel can take focus.
    if cfg!(windows) {
        tabs.push(r(Key::ShortcutsFocusChat, vec![vec![m, "4"]]));
    }
    tabs.extend([
        r(Key::ShortcutsNextArea, vec![vec!["F6"]]),
        r(Key::ShortcutsPreviousArea, vec![vec![shift, "F6"]]),
        r(Key::ShortcutsMaximize, vec![vec![m, shift, enter]]),
    ]);

    let lists = vec![
        r(
            Key::ShortcutsListMove,
            vec![vec![up], vec![down], vec!["K"], vec!["J"]],
        ),
        r(Key::ShortcutsListEnds, vec![vec!["Home"], vec!["End"]]),
        r(Key::ShortcutsListFold, vec![vec![left], vec![right]]),
        r(Key::CommonOpen, vec![vec![enter]]),
        r(Key::ShortcutsListPreview, vec![vec!["Space"]]),
        r(Key::TabOpenNew, vec![vec![m, enter]]),
        r(Key::TreeRename, vec![vec!["E"], vec!["F2"]]),
        r(Key::ShortcutsListStatus, vec![vec!["S"]]),
        c(Command::SetDue),
        r(Key::TreeMoveEpic, vec![vec!["M"]]),
        r(
            Key::ShortcutsListReorder,
            vec![vec![alt, up], vec![alt, down]],
        ),
        r(
            Key::CommonDelete,
            if macos {
                vec![vec![delete], vec![m, delete]]
            } else {
                vec![vec![delete]]
            },
        ),
        r(Key::ShortcutsListMenu, vec![vec![shift, "F10"]]),
    ];

    let editors = vec![
        r(Key::ShortcutsEditorSave, vec![vec![m, enter]]),
        r(Key::ShortcutsEditorCancel, vec![vec![esc]]),
    ];

    let mut terminal = vec![c(Command::TerminalLarger), c(Command::TerminalSmaller)];
    #[cfg(windows)]
    terminal.push(c(Command::ToggleChat));
    #[cfg(not(windows))]
    terminal.push(c(Command::ChatBrowser));

    vec![
        (Key::ShortcutsGroupGeneral, general),
        (Key::ShortcutsGroupTabs, tabs),
        (Key::ShortcutsGroupLists, lists),
        (Key::ShortcutsGroupEditors, editors),
        (Key::ShortcutsGroupTerminal, terminal),
    ]
}

/// The rows whose label matches `query` (case-insensitive, anywhere in
/// the label).
pub fn matching(rows: &[Row], query: &str) -> Vec<Row> {
    let query = query.trim().to_lowercase();
    rows.iter()
        .filter(|r| query.is_empty() || r.label.to_lowercase().contains(&query))
        .cloned()
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
        for (group, rows) in groups(locale, macos) {
            let rows = matching(&rows, &query);
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
            for row in rows {
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
                                .child(row.label),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.))
                                .children(row.keys.iter().map(|keys| ui::kbd(keys, &t))),
                        ),
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
    fn every_row_has_keys_and_the_filter_matches_labels() {
        for locale in Locale::ALL {
            for macos in [false, true] {
                for (_, rows) in groups(locale, macos) {
                    for row in rows {
                        assert!(
                            !row.keys.is_empty() && row.keys.iter().all(|k| !k.is_empty()),
                            "{} has no key",
                            row.label
                        );
                    }
                }
            }
        }
        let all: Vec<Row> = groups(Locale::En, false)
            .into_iter()
            .flat_map(|(_, r)| r)
            .collect();
        let hits = matching(&all, "TERMINAL");
        assert!(hits.iter().any(|r| r.label == "Toggle terminal"));
        assert!(!hits.iter().any(|r| r.label == "New topic"));
        assert!(matching(&all, "zzz").is_empty());
    }

    #[test]
    fn the_page_lists_the_keys_of_6_1() {
        // The palette with both keys, the terminal's second key, tabs,
        // focus, and the groups that had no rows.
        let groups = groups(Locale::En, false);
        let find = |label: &str| {
            groups
                .iter()
                .flat_map(|(_, r)| r)
                .find(|r| r.label == label)
                .cloned()
                .unwrap_or_else(|| panic!("{label} missing"))
        };
        assert_eq!(
            find("Command palette").keys,
            [vec!["Ctrl", "K"], vec!["Ctrl", "Shift", "K"]]
        );
        assert!(
            find("Toggle terminal")
                .keys
                .contains(&vec!["Ctrl", "Shift", "J"])
        );
        assert_eq!(find("Close tab").keys, [vec!["Ctrl", "W"]]);
        assert_eq!(
            find("Redo").keys,
            [vec!["Ctrl", "Shift", "Z"], vec!["Ctrl", "Y"]]
        );
        assert_eq!(find("Next tab").keys, [vec!["Ctrl", "Tab"]]);
        assert_eq!(
            find("Save the editor in focus").keys,
            [vec!["Ctrl", "Enter"]]
        );
        assert_eq!(find("Focus Projects").keys, [vec!["Ctrl", "1"]]);
        assert!(
            find("Open in new tab")
                .keys
                .contains(&vec!["Ctrl", "Enter"])
        );
        let names: Vec<Key> = groups.iter().map(|(k, _)| *k).collect();
        assert!(names.contains(&Key::ShortcutsGroupLists));
        assert!(names.contains(&Key::ShortcutsGroupEditors));
    }
}
