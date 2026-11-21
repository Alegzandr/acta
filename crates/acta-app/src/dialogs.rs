//! Dialogs (docs/design/components.md 5, Dialog; components-app.md 5): the
//! export dialog, the import preview, snapshot restore, and the terminal
//! restart confirmation. Centred, over the scrim, focus trapped, `Esc`
//! cancels and focus starts on the safe button.

use std::path::PathBuf;
use std::sync::Arc;

use acta_core::model::ProjectId;
use acta_core::snapshots::{Snapshot, SnapshotKind};
use acta_core::store::{ExportFile, ImportReport};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::i18n::{Arg, Key, Locale, number, text, tr};
use crate::icons::Icon;
use crate::settings;
use crate::theme::{RADIUS_LG, RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{ToastKind, Workspace};

gpui_kit::actions!(acta_dialog, [CancelDialog]);

pub enum Kind {
    Export {
        project: Option<(ProjectId, String)>,
        this_project: bool,
    },
    Import {
        file_name: String,
        file: Arc<ExportFile>,
        report: ImportReport,
    },
    /// The file could not be imported: one notice and Close.
    ImportRefused {
        file_name: String,
        message: String,
    },
    Snapshots {
        list: Vec<Snapshot>,
        selected: Option<usize>,
    },
    RestartTerminal,
}

pub enum DialogEvent {
    Close,
    /// The terminal restart was confirmed.
    RestartTerminal,
}

pub struct Dialog {
    ws: Entity<Workspace>,
    pub kind: Kind,
    focus: FocusHandle,
}

impl EventEmitter<DialogEvent> for Dialog {}

impl Focusable for Dialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// `acta-2026-11-21.acta.json`, the export's default name.
pub fn export_name(now_ms: i64) -> String {
    let day = acta_core::time::day_number(now_ms);
    let (y, m, d) = acta_core::time::civil_from_days(day);
    format!(
        "acta-{y:04}-{m:02}-{d:02}.{}",
        acta_core::store::FILE_EXTENSION
    )
}

/// The label of a snapshot kind (components-app.md 5, Snapshot restore).
pub fn snapshot_label(locale: Locale, kind: SnapshotKind) -> String {
    match kind {
        SnapshotKind::Daily => text(locale, Key::SnapshotDaily),
        SnapshotKind::Import => text(locale, Key::SnapshotBeforeImport),
        SnapshotKind::Restore => text(locale, Key::SnapshotBeforeRestore),
        SnapshotKind::Migration => tr(
            locale,
            Key::SnapshotBeforeUpdate,
            &[("version", Arg::Text(acta_core::VERSION))],
        ),
        // Acta takes no manual snapshot itself and the catalogue has no word
        // for one; one made by the CLI reads as a daily one.
        SnapshotKind::Manual => text(locale, Key::SnapshotDaily),
    }
}

impl Dialog {
    pub fn new(
        ws: Entity<Workspace>,
        kind: Kind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self { ws, kind, focus }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(DialogEvent::Close);
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        let Kind::Export {
            project,
            this_project,
        } = &self.kind
        else {
            return;
        };
        let scope = if *this_project {
            project.as_ref().map(|p| p.0)
        } else {
            None
        };
        let name = export_name(acta_core::time::now_ms());
        let dir = crate::paths_view::home()
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let chosen = cx.prompt_for_new_path(&dir, Some(&name));
        let ws = self.ws.clone();
        cx.emit(DialogEvent::Close);
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let task = ws.update(cx, |ws, cx| {
                ws.write(cx, Key::ErrorSave, move |store| {
                    let file = store.export(scope)?;
                    let count = file.projects.len() as u64;
                    let json = file.to_json()?;
                    std::fs::write(&path, json)?;
                    store.record_export()?;
                    Ok(count)
                })
            });
            if let Some(count) = task.await {
                ws.update(cx, |ws, cx| {
                    let locale = settings::locale(cx);
                    let message = tr(
                        locale,
                        Key::ExportDone,
                        &[("n", Arg::Count(count)), ("file", Arg::Text(&file_name))],
                    );
                    ws.toast(message, ToastKind::Done, cx);
                });
            }
        })
        .detach();
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let Kind::Import { file, .. } = &self.kind else {
            return;
        };
        let file = Arc::clone(file);
        let ws = self.ws.clone();
        cx.emit(DialogEvent::Close);
        cx.spawn(async move |_, cx| {
            let task = ws.update(cx, |ws, cx| {
                ws.write(cx, Key::ErrorSave, move |store| {
                    let report = store.import(&file, false)?;
                    let missing = file
                        .folders
                        .iter()
                        .filter(|f| !std::path::Path::new(&f.path).is_dir())
                        .count() as u64;
                    Ok((report, missing))
                })
            });
            if let Some((report, missing)) = task.await {
                ws.update(cx, |ws, cx| {
                    let locale = settings::locale(cx);
                    let total = report.total();
                    let created = tr(
                        locale,
                        Key::ImportCountCreated,
                        &[("n", Arg::Count(total.created.max(0) as u64))],
                    );
                    let updated = tr(
                        locale,
                        Key::ImportCountUpdated,
                        &[("n", Arg::Count(total.updated.max(0) as u64))],
                    );
                    let mut message = tr(
                        locale,
                        Key::ImportDone,
                        &[
                            ("created", Arg::Text(&created)),
                            ("updated", Arg::Text(&updated)),
                        ],
                    );
                    if missing > 0 {
                        message.push(' ');
                        message.push_str(&tr(
                            locale,
                            Key::ImportFolders,
                            &[("n", Arg::Count(missing))],
                        ));
                    }
                    ws.toast(message, ToastKind::Done, cx);
                });
            }
        })
        .detach();
    }

    fn restore_snapshot(&mut self, cx: &mut Context<Self>) {
        let Kind::Snapshots {
            list,
            selected: Some(at),
        } = &self.kind
        else {
            return;
        };
        let Some(snapshot) = list.get(*at).cloned() else {
            return;
        };
        let task = self.ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| {
                store.restore_snapshot(&snapshot.path)
            })
        });
        cx.spawn(async move |_, cx| {
            if task.await.is_some() {
                cx.update(|cx| {
                    crate::settings::flush(cx);
                    cx.restart();
                });
            }
        })
        .detach();
    }

    fn body(
        &mut self,
        locale: Locale,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> (String, AnyElement, Vec<AnyElement>, f32) {
        match &self.kind {
            Kind::Export {
                project,
                this_project,
            } => {
                let this_project = *this_project;
                let scope = project.as_ref().map(|_| {
                    ui::segmented(t)
                        .child(
                            ui::segment(
                                "export-all",
                                text(locale, Key::ExportScopeAll),
                                !this_project,
                                t,
                            )
                            .on_click(cx.listener(|d, _, _, cx| {
                                if let Kind::Export { this_project, .. } = &mut d.kind {
                                    *this_project = false;
                                }
                                cx.notify();
                            })),
                        )
                        .child(
                            ui::segment(
                                "export-project",
                                text(locale, Key::ExportScopeProject),
                                this_project,
                                t,
                            )
                            .on_click(cx.listener(|d, _, _, cx| {
                                if let Kind::Export { this_project, .. } = &mut d.kind {
                                    *this_project = true;
                                }
                                cx.notify();
                            })),
                        )
                });
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .children(scope)
                    .child(ui::notice(
                        Some((Icon::TriangleAlert, t.warning)),
                        text(locale, Key::ExportWarning),
                        t,
                    ))
                    .into_any_element();
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "export-choose",
                        text(locale, Key::ExportChoose),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|d, _, _, cx| d.export(cx)))
                    .into_any_element(),
                ];
                (text(locale, Key::ExportTitle), body, buttons, 440.)
            }
            Kind::Import {
                file_name,
                file,
                report,
            } => {
                let title = tr(locale, Key::ImportTitle, &[("file", Arg::Text(file_name))]);
                let date = when::date(locale, file.exported_at);
                let projects = tr(
                    locale,
                    Key::CountProjects,
                    &[("n", Arg::Count(file.projects.len() as u64))],
                );
                let version = file.version.to_string();
                let meta = tr(
                    locale,
                    Key::ImportMeta,
                    &[
                        ("date", Arg::Text(&date)),
                        ("n", Arg::Text(&version)),
                        ("projects", Arg::Text(&projects)),
                    ],
                );
                let cell = |n: i64, t: &Tokens| {
                    type_scale::UI
                        .el()
                        .w(px(80.))
                        .text_right()
                        .font_features(type_scale::tabular())
                        .text_color(if n == 0 {
                            t.text_tertiary
                        } else {
                            t.text_primary
                        })
                        .child(number(locale, n.max(0) as u64))
                };
                let rows = [
                    (Key::ImportKindProjects, report.projects),
                    (Key::ImportKindEpics, report.epics),
                    (Key::ImportKindTopics, report.topics),
                    (Key::ImportKindEntries, report.entries),
                    (Key::ImportKindNotes, report.notes),
                    (Key::SectionKnowledge, report.knowledge),
                ];
                let header = type_scale::SMALL
                    .el()
                    .flex()
                    .flex_row()
                    .h(px(28.))
                    .items_center()
                    .text_color(t.text_tertiary)
                    .child(div().flex_1())
                    .child(
                        div()
                            .w(px(80.))
                            .text_right()
                            .child(text(locale, Key::ImportCreated)),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .text_right()
                            .child(text(locale, Key::ImportUpdated)),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .text_right()
                            .child(text(locale, Key::ImportSkipped)),
                    );
                let table = div().flex().flex_col().mt(px(16.)).child(header).children(
                    rows.into_iter().map(|(key, counts)| {
                        div()
                            .flex()
                            .flex_row()
                            .h(px(28.))
                            .items_center()
                            .child(
                                type_scale::UI
                                    .el()
                                    .flex_1()
                                    .text_color(t.text_primary)
                                    .child(text(locale, key)),
                            )
                            .child(cell(counts.created, t))
                            .child(cell(counts.updated, t))
                            .child(cell(counts.skipped, t))
                    }),
                );
                let body = div()
                    .flex()
                    .flex_col()
                    .child(
                        type_scale::SMALL
                            .el()
                            .text_color(t.text_tertiary)
                            .child(meta),
                    )
                    .child(table)
                    .child(
                        type_scale::UI
                            .el()
                            .mt(px(16.))
                            .text_color(t.text_secondary)
                            .child(text(locale, Key::ImportSafe)),
                    )
                    .into_any_element();
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "import-confirm",
                        text(locale, Key::ImportConfirm),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|d, _, _, cx| d.import(cx)))
                    .into_any_element(),
                ];
                (title, body, buttons, 520.)
            }
            Kind::ImportRefused { file_name, message } => {
                let title = tr(locale, Key::ImportTitle, &[("file", Arg::Text(file_name))]);
                let body = ui::notice(Some((Icon::Info, t.text_secondary)), message.clone(), t)
                    .into_any_element();
                let buttons = vec![
                    ui::button(
                        "dialog-close",
                        text(locale, Key::CommonClose),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                ];
                (title, body, buttons, 520.)
            }
            Kind::Snapshots { list, selected } => {
                let selected = *selected;
                let rows = list.iter().enumerate().map(|(i, s)| {
                    let is = selected == Some(i);
                    div()
                        .id(("snapshot", i))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.))
                        .h(px(32.))
                        .px(px(8.))
                        .rounded(px(RADIUS_SM))
                        .when(is, |this| this.bg(t.fill_selected))
                        .hover({
                            let hover = t.fill_hover;
                            move |st| st.bg(hover)
                        })
                        .on_click(cx.listener(move |d, _, _, cx| {
                            if let Kind::Snapshots { selected, .. } = &mut d.kind {
                                *selected = Some(i);
                            }
                            cx.notify();
                        }))
                        .child(
                            type_scale::UI
                                .el()
                                .font_features(type_scale::tabular())
                                .text_color(t.text_primary)
                                .child(when::datetime(locale, s.taken_at)),
                        )
                        .child(
                            type_scale::UI
                                .el()
                                .text_color(t.text_secondary)
                                .child(snapshot_label(locale, s.kind)),
                        )
                });
                let note = selected.and_then(|i| list.get(i)).map(|s| {
                    type_scale::UI
                        .el()
                        .mt(px(16.))
                        .text_color(t.text_secondary)
                        .child(tr(
                            locale,
                            Key::SnapshotBody,
                            &[("datetime", Arg::Text(&when::datetime(locale, s.taken_at)))],
                        ))
                });
                let body = div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("snapshot-list")
                            .max_h(px(320.))
                            .overflow_y_scroll()
                            .children(rows),
                    )
                    .children(note)
                    .into_any_element();
                let danger = selected.is_some();
                let confirm = ui::button(
                    "snapshot-confirm",
                    text(locale, Key::SnapshotConfirm),
                    Variant::Primary,
                    !danger,
                    t,
                )
                .when(danger, |this| this.bg(t.danger))
                .on_click(cx.listener(|d, _, _, cx| d.restore_snapshot(cx)));
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    confirm.into_any_element(),
                ];
                (text(locale, Key::CmdSnapshot), body, buttons, 520.)
            }
            Kind::RestartTerminal => {
                let body = type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::TerminalRestartBody))
                    .into_any_element();
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "restart-confirm",
                        text(locale, Key::TerminalRestartConfirm),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DialogEvent::RestartTerminal)))
                    .into_any_element(),
                ];
                (text(locale, Key::TerminalRestartTitle), body, buttons, 440.)
            }
        }
    }
}

impl Render for Dialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let (title, body, mut buttons, width) = self.body(locale, &t, cx);
        // The confirming action is last, first on Windows (platform order).
        if cfg!(windows) {
            buttons.reverse();
        }
        div()
            .id("dialog-scrim")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(t.scrim)
            .occlude()
            .child(
                div()
                    .id("dialog")
                    .track_focus(&self.focus)
                    .key_context("Dialog")
                    .on_action(cx.listener(|d, _: &CancelDialog, _, cx| d.close(cx)))
                    .w(px(width))
                    .flex()
                    .flex_col()
                    .p(px(24.))
                    .bg(t.bg_overlay)
                    .border_1()
                    .border_color(t.border_default)
                    .rounded(px(RADIUS_LG))
                    .shadow(vec![t.shadow_overlay()])
                    .child(
                        type_scale::HEADING
                            .el()
                            .text_color(t.text_primary)
                            .child(title),
                    )
                    .child(div().mt(px(8.)).child(body))
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.))
                            .children(buttons),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_is_named_after_the_day() {
        // 2026-11-20T12:00:00Z
        assert_eq!(export_name(1_795_176_000_000), "acta-2026-11-20.acta.json");
    }

    #[test]
    fn every_snapshot_kind_has_a_label() {
        for locale in Locale::ALL {
            for kind in [
                SnapshotKind::Daily,
                SnapshotKind::Import,
                SnapshotKind::Restore,
                SnapshotKind::Migration,
                SnapshotKind::Manual,
            ] {
                assert!(!snapshot_label(locale, kind).is_empty());
            }
        }
    }
}
