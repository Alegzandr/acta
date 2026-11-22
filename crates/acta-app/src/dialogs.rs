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
use crate::theme::{RADIUS_SM, RADIUS_XL, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::when;
use crate::workspace::{ToastKind, Workspace};

gpui_kit::actions!(acta_dialog, [CancelDialog]);

/// A confirmation asked by a page (interaction.md 6.4): the title names
/// the object, the body the consequence, the button repeats the verb.
#[derive(Clone, Debug, PartialEq)]
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub confirm: String,
    /// Run once confirmed (the index is unused).
    pub on_confirm: crate::palette::OnPick,
}

// One dialog lives at a time: the import report's size does not matter.
#[allow(clippy::large_enum_variant)]
pub enum Kind {
    Export {
        project: Option<(ProjectId, String)>,
        this_project: bool,
    },
    Import {
        file_name: String,
        file: Arc<ExportFile>,
        report: ImportReport,
        /// From the key-loss screen: confirming sets the unreadable
        /// database aside and imports into a new one.
        recovery: bool,
    },
    /// The file could not be imported: one notice and Close.
    ImportRefused {
        file_name: String,
        message: String,
    },
    Snapshots {
        list: Vec<Snapshot>,
        /// Each snapshot's file size in bytes, in list order.
        sizes: Vec<u64>,
        selected: Option<usize>,
    },
    RestartTerminal,
    /// A page's own confirmation (a git action where a session works).
    Confirm(Confirm),
    /// "Remove Acta from Claude Code?" (Settings, Claude Code).
    RemoveSetup,
    /// "Install Git?" (Ready for Claude Code, ADR 0013): where it comes
    /// from, the scope, who updates it, and the exact command with Copy.
    ReadyInstall {
        tool: acta_core::prereqs::Tool,
        plan: acta_core::prereqs::Plan,
        update: bool,
        platform: acta_core::prereqs::Platform,
    },
    /// "Quit Acta?" while an install runs (it has no cancel).
    QuitInstalling {
        tool: acta_core::prereqs::Tool,
    },
    /// "Set up Claude Code?" (Settings, Claude Code): the open project
    /// and how many of its folders setup touches.
    Setup {
        project: Option<(acta_core::model::ProjectId, String)>,
        folders: usize,
    },
    /// "Dictate with your voice" (components-voice.md 5, First use).
    VoiceSetup,
    #[cfg(windows)]
    /// "Sign out and clear chat data?" (components-app.md 5, Chat panel).
    ChatSignOut,
    #[cfg(windows)]
    /// "Clear the browser's data?" (components-app.md 5, Agent browser).
    BrowserClear,
}

pub enum DialogEvent {
    Close,
    /// Key-loss import confirmed: start a new database from this file.
    Recover(Arc<ExportFile>),
    /// The terminal restart was confirmed.
    RestartTerminal,
    /// A page's confirmation was confirmed: run its action.
    Confirmed(crate::palette::OnPick),
    /// Removing Acta from Claude Code was confirmed.
    RemoveSetup,
    /// Setup was confirmed, linking that project's folders.
    Setup(Option<acta_core::model::ProjectId>),
    /// An install of the Ready block was confirmed.
    ReadyRun {
        tool: acta_core::prereqs::Tool,
        plan: acta_core::prereqs::Plan,
        update: bool,
    },
    /// Quitting while an install runs was confirmed.
    Quit,
    #[cfg(windows)]
    /// Deleting the chat panel's profile was confirmed.
    ChatSignOut,
    #[cfg(windows)]
    /// Deleting the agent browser's profile was confirmed.
    BrowserClear,
}

pub struct Dialog {
    /// `None` on the key-loss screen, where no database is open.
    ws: Option<Entity<Workspace>>,
    pub kind: Kind,
    focus: FocusHandle,
    /// Cancel (or Close): the safe button, focused when the dialog opens.
    safe: FocusHandle,
    /// The install confirmation's "Details" are unfolded.
    details_open: bool,
}

impl EventEmitter<DialogEvent> for Dialog {}

impl Focusable for Dialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// A file size in megabytes with one decimal, in the language's number
/// form: "12.4 MB", "12,4 Mo" (copy.md 10.2).
pub fn size_mb(locale: Locale, bytes: u64) -> String {
    let tenths = (bytes as f64 / 104_857.6).round() as u64;
    let whole = number(locale, tenths / 10);
    let sep = if locale == Locale::Fr { ',' } else { '.' };
    let size = format!("{whole}{sep}{}", tenths % 10);
    tr(locale, Key::SnapshotSize, &[("size", Arg::Text(&size))])
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

/// The result line of an import (components-app.md 5, Import).
pub fn import_done(locale: Locale, report: &ImportReport, missing_folders: u64) -> String {
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
    if missing_folders > 0 {
        message.push(' ');
        message.push_str(&tr(
            locale,
            Key::ImportFolders,
            &[("n", Arg::Count(missing_folders))],
        ));
    }
    if report.folders_refused > 0 {
        message.push(' ');
        message.push_str(&tr(
            locale,
            Key::ImportNetworkFolders,
            &[("n", Arg::Count(report.folders_refused as u64))],
        ));
    }
    message
}

/// The result line of a key-loss recovery: [`import_done`], and, when the
/// file had folders that committed automatically, that they are Manual
/// now (commit modes never come from a file, ADR 0011).
pub fn recovery_done(
    locale: Locale,
    report: &ImportReport,
    missing_folders: u64,
    had_auto: bool,
) -> String {
    let mut message = import_done(locale, report, missing_folders);
    if had_auto {
        message.push(' ');
        message.push_str(&text(locale, Key::ImportRecoveredManual));
    }
    message
}

/// The refusal of a file too large to import, with its size.
pub fn import_too_large(locale: Locale, size: u64, limit: u64) -> String {
    let limit = tr(
        locale,
        Key::SnapshotSize,
        &[("size", Arg::Text(&number(locale, limit / (1024 * 1024))))],
    );
    tr(
        locale,
        Key::ImportTooLarge,
        &[
            ("size", Arg::Text(&size_mb(locale, size))),
            ("limit", Arg::Text(&limit)),
        ],
    )
}

/// The folder paths of an imported file that may be checked for
/// existence: those the import kept (never one that could name another
/// computer).
pub fn probe_safe_paths(file: &ExportFile) -> Vec<String> {
    file.folders
        .iter()
        .map(|f| f.path.clone())
        .filter(|p| acta_core::folder_path::is_probe_safe(p))
        .collect()
}

/// What importing `file` into an empty database does: every record is
/// created (the key-loss preview, before any database exists).
pub fn report_into_empty(file: &ExportFile) -> ImportReport {
    let created = |n: usize| acta_core::store::ImportCounts {
        created: n as i64,
        updated: 0,
        skipped: 0,
    };
    ImportReport {
        projects: created(file.projects.len()),
        epics: created(file.epics.len()),
        topics: created(file.topics.len()),
        entries: created(file.entries.len()),
        notes: created(file.notes.len()),
        knowledge: created(file.knowledge.len()),
        revisions: created(file.knowledge.iter().map(|k| k.revisions.len()).sum()),
        folders: created(probe_safe_paths(file).len()),
        folders_refused: (file.folders.len() - probe_safe_paths(file).len()) as i64,
        questions: created(file.questions.len()),
        contacts: created(file.contacts.len()),
        lifted: 0,
        attachments: created(file.attachments.len()),
        files_not_in_export: Vec::new(),
    }
}

/// Write `bytes` to `path` without ever leaving half a file, readable by
/// the user only (the export is plaintext): see
/// [`acta_core::fsutil::write_private_atomic`].
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    acta_core::fsutil::write_private_atomic(path, bytes)
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
        ws: Option<Entity<Workspace>>,
        kind: Kind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        // components.md 5, Dialog: focus starts on the safe button.
        let safe = cx.focus_handle().tab_index(0).tab_stop(true);
        window.focus(&safe, cx);
        Self {
            ws,
            kind,
            focus,
            safe,
            details_open: false,
        }
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
        let Some(ws) = self.ws.clone() else { return };
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
        cx.emit(DialogEvent::Close);
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Read under the store, write the file without it.
            let read = ws.update(cx, |ws, cx| {
                ws.db.run(cx, move |store| {
                    let file = store.export(scope)?;
                    Ok((file.projects.len() as u64, file.to_json()?))
                })
            });
            let Ok((count, json)) = read.await else {
                ws.update(cx, |ws, cx| ws.toast_error(Key::ErrorSave, cx));
                return;
            };
            let written = cx
                .background_executor()
                .spawn(async move { write_atomic(&path, json.as_bytes()) })
                .await;
            if written.is_err() {
                ws.update(cx, |ws, cx| ws.toast_error(Key::ErrorSave, cx));
                return;
            }
            ws.update(cx, |ws, cx| {
                ws.write_detached(cx, Key::ErrorSave, |store| store.record_export());
                let locale = settings::locale(cx);
                let message = tr(
                    locale,
                    Key::ExportDone,
                    &[("n", Arg::Count(count)), ("file", Arg::Text(&file_name))],
                );
                ws.toast(message, ToastKind::Done, cx);
            });
        })
        .detach();
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let Kind::Import { file, recovery, .. } = &self.kind else {
            return;
        };
        let file = Arc::clone(file);
        if *recovery {
            // Key loss: the unreadable database is set aside only now.
            cx.emit(DialogEvent::Recover(file));
            return;
        }
        let Some(ws) = self.ws.clone() else { return };
        cx.emit(DialogEvent::Close);
        cx.spawn(async move |_, cx| {
            let task = ws.update(cx, |ws, cx| {
                let file = Arc::clone(&file);
                ws.write(cx, Key::ErrorSave, move |store| store.import(&file, false))
            });
            let Ok(report) = task.await else { return };
            // Existence is checked off the store.
            let paths = probe_safe_paths(&file);
            let missing = cx
                .background_executor()
                .spawn(async move { crate::workspace::missing_paths(paths).len() as u64 })
                .await;
            ws.update(cx, |ws, cx| {
                let message = import_done(settings::locale(cx), &report, missing);
                ws.toast(message, ToastKind::Done, cx);
            });
        })
        .detach();
    }

    fn restore_snapshot(&mut self, cx: &mut Context<Self>) {
        let Kind::Snapshots {
            list,
            selected: Some(at),
            ..
        } = &self.kind
        else {
            return;
        };
        let Some(snapshot) = list.get(*at).cloned() else {
            return;
        };
        let Some(ws) = self.ws.clone() else { return };
        let task = ws.update(cx, |ws, cx| {
            ws.write(cx, Key::ErrorSave, move |store| {
                store.restore_snapshot(&snapshot.path)
            })
        });
        cx.spawn(async move |_, cx| {
            if task.await.is_ok() {
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
                    .track_focus(&self.safe)
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
                ..
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
                    .track_focus(&self.safe)
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
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                ];
                (title, body, buttons, 520.)
            }
            Kind::Snapshots {
                list,
                sizes,
                selected,
            } => {
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
                                .flex_1()
                                .text_color(t.text_secondary)
                                .child(snapshot_label(locale, s.kind)),
                        )
                        .child(
                            type_scale::SMALL
                                .el()
                                .font_features(type_scale::tabular())
                                .text_color(t.text_tertiary)
                                .child(size_mb(locale, sizes.get(i).copied().unwrap_or(0))),
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
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    confirm.into_any_element(),
                ];
                (text(locale, Key::SnapshotTitle), body, buttons, 520.)
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
                    .track_focus(&self.safe)
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
            Kind::VoiceSetup => crate::voice::setup::body(&self.safe, locale, t, cx),
            Kind::Confirm(c) => {
                let body = type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(c.body.clone())
                    .into_any_element();
                let on_confirm = c.on_confirm.clone();
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "confirm-confirm",
                        c.confirm.clone(),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(DialogEvent::Confirmed(on_confirm.clone()))
                    }))
                    .into_any_element(),
                ];
                (c.title.clone(), body, buttons, 440.)
            }
            Kind::Setup { project, folders } => {
                let mut sentence = text(locale, Key::SettingsSetupBody);
                if let Some((_, name)) = project.as_ref().filter(|_| *folders > 0) {
                    sentence.push(' ');
                    sentence.push_str(&crate::i18n::tr(
                        locale,
                        Key::SettingsSetupFolders,
                        &[
                            ("n", crate::i18n::Arg::Count(*folders as u64)),
                            ("project", crate::i18n::Arg::Text(name)),
                        ],
                    ));
                }
                let body = type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(sentence)
                    .into_any_element();
                let id = project.as_ref().map(|(id, _)| *id);
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "setup-confirm",
                        text(locale, Key::SettingsSetupConfirm),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(DialogEvent::Setup(id))))
                    .into_any_element(),
                ];
                (text(locale, Key::SettingsSetupTitle), body, buttons, 440.)
            }
            Kind::ReadyInstall {
                tool,
                plan,
                update,
                platform,
            } => {
                let me = cx.entity().downgrade();
                let toggle = move |_: &mut Window, cx: &mut App| {
                    let _ = me.update(cx, |d, cx| {
                        d.details_open = !d.details_open;
                        cx.notify();
                    });
                };
                let (title, body, confirm) = crate::ready::dialog_parts(
                    *tool,
                    plan,
                    *update,
                    platform,
                    self.details_open,
                    toggle,
                    locale,
                    t,
                );
                // A command Acta does not run (npm) has Close only.
                let safe = if confirm.is_some() {
                    Key::CommonCancel
                } else {
                    Key::CommonClose
                };
                let mut buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, safe),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                ];
                if let Some(confirm) = confirm {
                    let (tool, plan, update) = (*tool, plan.clone(), *update);
                    buttons.push(
                        ui::button("ready-confirm", confirm, Variant::Primary, false, t)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(DialogEvent::ReadyRun {
                                    tool,
                                    plan: plan.clone(),
                                    update,
                                })
                            }))
                            .into_any_element(),
                    );
                }
                (title, body, buttons, 440.)
            }
            Kind::QuitInstalling { tool } => {
                let body = type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(tr(
                        locale,
                        Key::QuitInstalling,
                        &[("name", Arg::Text(tool.display_name()))],
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
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "quit-confirm",
                        text(locale, Key::QuitConfirm),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DialogEvent::Quit)))
                    .into_any_element(),
                ];
                (text(locale, Key::QuitTitle), body, buttons, 440.)
            }
            Kind::RemoveSetup => {
                let body = type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::SettingsRemoveBody))
                    .into_any_element();
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "remove-setup-confirm",
                        text(locale, Key::SettingsRemoveConfirm),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DialogEvent::RemoveSetup)))
                    .into_any_element(),
                ];
                (text(locale, Key::SettingsRemoveTitle), body, buttons, 440.)
            }
            #[cfg(windows)]
            Kind::ChatSignOut => {
                let body = type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::ChatSignoutBody))
                    .into_any_element();
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "chat-signout-confirm",
                        text(locale, Key::ChatSignoutConfirm),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DialogEvent::ChatSignOut)))
                    .into_any_element(),
                ];
                (text(locale, Key::ChatSignoutTitle), body, buttons, 440.)
            }
            #[cfg(windows)]
            Kind::BrowserClear => {
                let body = type_scale::UI
                    .el()
                    .text_color(t.text_secondary)
                    .child(text(locale, Key::BrowserClearBody))
                    .into_any_element();
                let buttons = vec![
                    ui::button(
                        "dialog-cancel",
                        text(locale, Key::CommonCancel),
                        Variant::Secondary,
                        false,
                        t,
                    )
                    .track_focus(&self.safe)
                    .on_click(cx.listener(|d, _, _, cx| d.close(cx)))
                    .into_any_element(),
                    ui::button(
                        "browser-clear-confirm",
                        text(locale, Key::BrowserClearConfirm),
                        Variant::Primary,
                        false,
                        t,
                    )
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DialogEvent::BrowserClear)))
                    .into_any_element(),
                ];
                (text(locale, Key::BrowserClearTitle), body, buttons, 440.)
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
                    .rounded(px(RADIUS_XL))
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
    fn an_export_is_written_whole_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("acta-2026-11-21.acta.json");
        std::fs::write(&path, "old").unwrap();
        write_atomic(&path, b"{\"format\":\"acta\"}").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"format\":\"acta\"}"
        );
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 1);
        assert!(write_atomic(&dir.path().join("missing").join("x.json"), b"x").is_err());
    }

    #[test]
    fn an_import_into_an_empty_database_creates_everything() {
        let mut temp = crate::db::test_support::temp_store();
        let p = temp
            .store
            .create_project("Acta", "", acta_core::model::Author::Human)
            .unwrap();
        temp.store
            .add_note(p, "Alexandre's note", acta_core::model::Author::Human)
            .unwrap();
        let file = temp.store.export(None).unwrap();
        let report = report_into_empty(&file);
        assert_eq!(report.projects.created, 1);
        assert_eq!(report.notes.created, 1);
        let empty = crate::db::test_support::temp_store();
        let mut store = empty.store;
        let real = store.import(&file, true).unwrap();
        assert_eq!(real.projects, report.projects);
        assert_eq!(real.notes, report.notes);
    }

    #[test]
    fn network_folders_are_counted_and_never_probed() {
        let mut temp = crate::db::test_support::temp_store();
        temp.store
            .create_project("Acta", "", acta_core::model::Author::Human)
            .unwrap();
        let mut file = temp.store.export(None).unwrap();
        let project = file.projects[0].stamp.uuid.clone();
        let folder = |path: &str, uuid: &str| acta_core::store::XFolder {
            stamp: acta_core::store::XStamp {
                uuid: uuid.into(),
                created_at: 1,
                updated_at: 1,
                updated_by: acta_core::model::Author::Human,
                deleted_at: None,
                deleted_by: None,
            },
            project: project.clone(),
            epic: None,
            topic: None,
            path: path.into(),
            position: 0,
            local: true,
            name: String::new(),
            icon: String::new(),
            commit_mode: Default::default(),
            commit_as_user: None,
            commit_on_default: false,
        };
        file.folders = vec![
            folder(r"C:\Users\Alexandre\acta", "f1"),
            folder(r"\\10.0.0.5\share", "f2"),
        ];
        assert_eq!(probe_safe_paths(&file), vec![r"C:\Users\Alexandre\acta"]);
        let report = report_into_empty(&file);
        assert_eq!((report.folders.created, report.folders_refused), (1, 1));
        let message = import_done(Locale::En, &report, 0);
        assert!(
            message.ends_with(
                "1 working folder on a network path was left out. \
                 Add it again from its project, epic or topic."
            ),
            "{message}"
        );
        // Key-loss recovery says commit modes are Manual again, only when
        // the file had automatic ones.
        assert_eq!(recovery_done(Locale::En, &report, 0, false), message);
        assert!(recovery_done(Locale::En, &report, 0, true).ends_with(
            "Commit modes come back as Manual: set them again in each folder\u{2019}s settings."
        ));
        assert_eq!(
            import_too_large(Locale::En, 600 * 1024 * 1024, 512 * 1024 * 1024),
            "This file is too large to import (600.0 MB). The limit is 512 MB. \
             Export one project at a time, then import each file."
        );
    }

    #[test]
    fn sizes_read_in_megabytes_in_each_language() {
        assert_eq!(size_mb(Locale::En, 13_002_342), "12.4 MB");
        assert_eq!(size_mb(Locale::Fr, 13_002_342), "12,4 Mo");
        assert_eq!(size_mb(Locale::En, 0), "0.0 MB");
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
