//! The pieces of the working folders shared by the record pages and the
//! terminal header (docs/design/components-folders.md 5; layout.md 4.5):
//! the switcher, its menu, and the git state.

use std::rc::Rc;

use crate::zoom::px;
use acta_core::model::FolderId;
use gpui_kit::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Window, div,
};

use crate::folders_model::{self, GitLine};
use crate::i18n::{Arg, Key, Locale, number, text, tr};
use crate::icons::{Icon, icon};
use crate::menu;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;

/// The switcher (the compact folder switcher): a ghost button 24 high,
/// padding 0 x 4, `radius.sm`: the folder's icon 16, its name, then
/// `chevron-down` 12. `missing`: the active folder was not found
/// (`folder-x` in `warning`, the name in `text.secondary`). The caller
/// adds the tooltip (the full path) and the click. `in_header`: the 28
/// header of the terminal, where it sits among `type.small` and
/// `type.label` text, so its name is `type.small` and its icon 14 (QA
/// 2026-11-22: it read larger than the view tabs beside it).
pub fn switcher(
    id: impl Into<SharedString>,
    glyph: Icon,
    name: &str,
    missing: bool,
    in_header: bool,
    t: &Tokens,
) -> Stateful<Div> {
    let hover = t.fill_hover;
    let role = if in_header {
        type_scale::SMALL
    } else {
        type_scale::UI
    };
    let el = role.el().id(id.into());
    ui::focusable(el, t, false)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .h(px(24.))
        .px(px(4.))
        .min_w_0()
        .rounded(px(RADIUS_SM))
        .hover(move |s| s.bg(hover))
        .child(icon(
            glyph,
            if in_header { 14. } else { 16. },
            if missing { t.warning } else { t.text_secondary },
        ))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(if missing {
                    t.text_secondary
                } else {
                    t.text_primary
                })
                .child(name.to_owned()),
        )
        .child(icon(Icon::ChevronDown, 12., t.text_tertiary))
}

/// One folder of the switcher's menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchEntry {
    pub id: FolderId,
    pub icon: Icon,
    pub name: String,
    pub active: bool,
    pub missing: bool,
}

/// A group of the menu under its label ("This topic", "From epic
/// Terminal").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchGroup {
    pub label: String,
    pub entries: Vec<SwitchEntry>,
}

/// The entries of the menu in order, for keyboard stepping: the folders
/// that can be chosen, then "Edit folders..." and "Add folder...".
pub fn choosable(groups: &[SwitchGroup]) -> Vec<Option<FolderId>> {
    groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .filter(|e| !e.missing)
        .map(|e| Some(e.id))
        .chain([None, None])
        .collect()
}

/// Type-ahead in the switcher's menu (components.md 5, Context menu): the
/// next folder after `from` (an index of [`choosable`]) whose name starts
/// with `typed`, ignoring case, wrapping around; `None` when no folder
/// does.
pub fn type_ahead(groups: &[SwitchGroup], from: usize, typed: &str) -> Option<usize> {
    let typed = typed.to_lowercase();
    if typed.is_empty() {
        return None;
    }
    let names: Vec<String> = groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .filter(|e| !e.missing)
        .map(|e| e.name.to_lowercase())
        .collect();
    let n = names.len();
    (1..=n)
        .map(|step| (from + step) % n)
        .find(|&i| names[i].starts_with(&typed))
}

type Act<V> = Rc<dyn Fn(&mut V, &mut Window, &mut Context<V>)>;
type Choose<V> = Rc<dyn Fn(&mut V, FolderId, &mut Window, &mut Context<V>)>;

/// What the switcher's menu does.
pub struct SwitchActions<V> {
    pub choose: Choose<V>,
    pub edit: Act<V>,
    pub add: Act<V>,
    pub close: Act<V>,
}

/// The switcher's menu (screens.md 8.11): every folder the page sees,
/// grouped by origin, "Active" on the active one, "Not found" (not
/// choosable) on a folder that was not found; then "Edit folders..." and
/// "Add folder...". Hangs under the switcher, measured by `anchor`.
/// `highlight` indexes [`choosable`].
#[allow(clippy::too_many_arguments)]
pub fn switcher_menu<V: 'static>(
    id: &'static str,
    anchor: &menu::AnchorBounds,
    groups: &[SwitchGroup],
    highlight: Option<usize>,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<V>,
    actions: SwitchActions<V>,
) -> AnyElement {
    let mut panel = menu::panel(id, t);
    let close = actions.close.clone();
    panel = panel.on_mouse_down_out(cx.listener(move |v, _, window, cx| close(v, window, cx)));
    let mut n = 0usize;
    for (g, group) in groups.iter().enumerate() {
        panel = panel.child(
            type_scale::SMALL
                .el()
                .flex()
                .items_center()
                .h(px(24.))
                .px(px(8.))
                .text_color(t.text_tertiary)
                .child(group.label.clone()),
        );
        for (i, entry) in group.entries.iter().enumerate() {
            let item_id = SharedString::from(format!("{id}-{g}-{i}"));
            if entry.missing {
                panel = panel.child(
                    type_scale::MENU
                        .el()
                        .id(item_id)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .h(px(28.))
                        .px(px(8.))
                        .text_color(t.text_disabled)
                        .child(icon(Icon::FolderX, 16., t.text_disabled))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(entry.name.clone()),
                        )
                        .child(
                            type_scale::SMALL
                                .el()
                                .child(text(locale, Key::FoldersNotFoundShort)),
                        ),
                );
                continue;
            }
            let trailing = entry
                .active
                .then(|| menu::hint(text(locale, Key::FoldersActive), t));
            let choose = actions.choose.clone();
            let folder = entry.id;
            panel = panel.child(
                menu::item(
                    item_id,
                    Some(icon(entry.icon, 16., t.text_secondary).into_any_element()),
                    entry.name.clone(),
                    trailing,
                    highlight == Some(n),
                    t,
                )
                .on_click(cx.listener(move |v, _, window, cx| choose(v, folder, window, cx))),
            );
            n += 1;
        }
    }
    let (edit, add) = (actions.edit.clone(), actions.add.clone());
    panel = panel
        .child(menu::separator(t))
        .child(
            menu::item(
                SharedString::from(format!("{id}-edit")),
                None,
                text(locale, Key::FoldersEdit),
                None,
                highlight == Some(n),
                t,
            )
            .on_click(cx.listener(move |v, _, window, cx| edit(v, window, cx))),
        )
        .child(
            menu::item(
                SharedString::from(format!("{id}-add")),
                None,
                text(locale, Key::FoldersAdd),
                None,
                highlight == Some(n + 1),
                t,
            )
            .on_click(cx.listener(move |v, _, window, cx| add(v, window, cx))),
        );
    anchor
        .below(panel, menu::Align::Start)
        .unwrap_or_else(|| div().into_any_element())
}

/// The git state (ADR 0011, 2): `git-branch` 12 and the branch, then
/// " · 3 changed" and " · behind 2", then `git-commit-horizontal` 12 for
/// an automatic commit mode, all `type.small` `text.tertiary`. `branch` is
/// the branch's element (a plain label, or the branch selector's button).
pub fn git_state(
    id: impl Into<SharedString>,
    line: &GitLine,
    branch: AnyElement,
    locale: Locale,
    t: &Tokens,
) -> Stateful<Div> {
    let mut el = type_scale::SMALL
        .el()
        .id(id.into())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .min_w_0()
        .text_color(t.text_tertiary)
        .font_features(type_scale::tabular())
        .child(icon(Icon::GitBranch, 12., t.text_tertiary))
        .child(branch);
    if line.changed > 0 {
        el = el.child(div().flex_shrink_0().child(format!(
            "\u{b7} {}",
            tr(
                locale,
                Key::FoldersGitChanged,
                &[("n", Arg::Count(line.changed as u64))]
            )
        )));
    }
    if line.behind > 0 {
        el = el.child(div().flex_shrink_0().child(format!(
            "\u{b7} {}",
            tr(
                locale,
                Key::FoldersGitBehind,
                &[("n", Arg::Count(line.behind as u64))]
            )
        )));
    }
    if let Some(key) = line.auto.and_then(folders_model::auto_tip) {
        el = el.child(
            div()
                .id("git-auto")
                .flex_shrink_0()
                .ml(px(2.))
                .child(icon(Icon::GitCommitHorizontal, 12., t.text_tertiary))
                .tooltip(ui::tip(text(locale, key))),
        );
    }
    el
}

/// "+3", the count of the other folders a page sees (`tnum`).
pub fn count(locale: Locale, n: usize, t: &Tokens) -> Div {
    type_scale::SMALL
        .el()
        .flex_shrink_0()
        .text_color(t.text_tertiary)
        .font_features(type_scale::tabular())
        .child(format!("+{}", number(locale, n as u64)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, name: &str, missing: bool) -> SwitchEntry {
        SwitchEntry {
            id: FolderId(id),
            icon: Icon::FolderOpen,
            name: name.to_owned(),
            active: false,
            missing,
        }
    }

    #[test]
    fn type_ahead_finds_the_next_name_and_wraps() {
        let groups = vec![
            SwitchGroup {
                label: "This topic".into(),
                entries: vec![entry(1, "acta", false), entry(2, "Docs", false)],
            },
            SwitchGroup {
                label: "From project Acta".into(),
                entries: vec![entry(3, "gone", true), entry(4, "acta-docs", false)],
            },
        ];
        // The menu's indexes skip folders not found ("gone").
        assert_eq!(type_ahead(&groups, 0, "a"), Some(2));
        assert_eq!(type_ahead(&groups, 2, "a"), Some(0));
        assert_eq!(type_ahead(&groups, 0, "D"), Some(1));
        assert_eq!(type_ahead(&groups, 0, "g"), None);
        assert_eq!(type_ahead(&groups, 0, ""), None);
    }
}
