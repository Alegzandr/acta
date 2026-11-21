//! The terminal tool window (docs/design/layout.md 4.5; components-folders.md
//! 5, Terminal launch; screens.md 8.4, 8.11): one session per project,
//! started in the primary effective folder of the page it was opened from,
//! with the other folders passed along and the focus in `ACTA_*`.
//! Navigation never restarts or moves a session; "Restart terminal here"
//! does, with the active page's folders.

use std::collections::HashMap;
use std::path::PathBuf;

use acta_core::model::{Author, Owner, ProjectId};
use acta_terminal::{Focus, LaunchError, TerminalEvent, TerminalTheme, TerminalView};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, PathPromptOptions, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};

use crate::i18n::{Arg, Key, KeyName, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::paths_view;
use crate::settings;
use crate::theme::{Appearance, Mode, RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};
use crate::workspace::{ToastKind, Workspace};

/// docs/design/layout.md 4.5: the right dock is 648 wide (80 columns).
pub const WIDTH: f32 = 648.;
/// The header of a tool window.
const HEADER: f32 = 28.;

/// Where a launch comes from: the page's record and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub owner: Owner,
    pub project: ProjectId,
    pub name: String,
}

enum Pane {
    /// Folders and the program are being resolved (no message before 400 ms).
    Starting,
    Running {
        view: Entity<TerminalView>,
        origin: Origin,
        _sub: Subscription,
    },
    /// No folder anywhere up the chain.
    Ask { origin: Origin, save: bool },
    /// The primary folder is not on this computer.
    Missing {
        origin: Origin,
        path: PathBuf,
        from: Owner,
    },
    /// The `claude` command was not found.
    NotFound { origin: Origin },
}

pub enum DockEvent {
    /// Hide was asked from the header.
    Hide,
    /// "Restart here": the shell restarts with the active page's origin
    /// (after the confirmation when a process runs).
    RestartHere,
}

pub struct TerminalDock {
    ws: Entity<Workspace>,
    panes: HashMap<ProjectId, Pane>,
    /// A folder chosen in the prompt and not saved: used until Acta quits.
    unsaved: HashMap<ProjectId, PathBuf>,
    pub project: Option<ProjectId>,
    /// The page shown now, set by the shell, to mark a mismatch.
    pub active: Option<Origin>,
    focus: FocusHandle,
    text_size: f32,
    mode: Option<Mode>,
}

impl EventEmitter<DockEvent> for TerminalDock {}

impl Focusable for TerminalDock {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.project.and_then(|p| self.panes.get(&p)) {
            Some(Pane::Running { view, .. }) => view.read(cx).focus_handle(cx),
            _ => self.focus.clone(),
        }
    }
}

fn focus_of(owner: Owner, project: ProjectId, epic: Option<acta_core::model::EpicId>) -> Focus {
    match owner {
        Owner::Project(id) => Focus {
            project: id.to_string(),
            epic: None,
            topic: None,
        },
        Owner::Epic(id) => Focus {
            project: project.to_string(),
            epic: Some(id.to_string()),
            topic: None,
        },
        Owner::Topic(id) => Focus {
            project: project.to_string(),
            epic: epic.map(|e| e.to_string()),
            topic: Some(id.to_string()),
        },
    }
}

impl TerminalDock {
    pub fn new(ws: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let text_size = settings::current(cx)
            .terminal_text_size
            .map(f32::from)
            .unwrap_or(acta_terminal::view::DEFAULT_TEXT_SIZE);
        Self {
            ws,
            panes: HashMap::new(),
            unsaved: HashMap::new(),
            project: None,
            active: None,
            focus: cx.focus_handle(),
            text_size,
            mode: None,
        }
    }

    pub fn has_session(&self, project: ProjectId) -> bool {
        self.panes.contains_key(&project)
    }

    /// Whether a process runs in the project's session (the restart
    /// confirmation of interaction.md 6.4 asks only then).
    pub fn is_running(&self, project: ProjectId, cx: &App) -> bool {
        matches!(self.panes.get(&project), Some(Pane::Running { view, .. }) if view.read(cx).is_running())
    }

    /// Show the session of `origin.project`, starting it from `origin` if
    /// there is none.
    pub fn show(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        self.project = Some(origin.project);
        if !self.panes.contains_key(&origin.project) {
            self.launch(origin, window, cx);
        }
        cx.notify();
    }

    /// End the project's session and start it from `origin`.
    pub fn restart(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        self.project = Some(origin.project);
        self.launch(origin, window, cx);
    }

    fn launch(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        let project = origin.project;
        let owner = origin.owner;
        let unsaved = self.unsaved.get(&project).cloned();
        let ws = self.ws.read(cx);
        let task = ws.db.run(cx, move |store| {
            let effective = store.effective_folders(owner)?;
            let epic = match owner {
                Owner::Topic(id) => store.topic(id)?.epic,
                _ => None,
            };
            let mut folders: Vec<(PathBuf, Owner)> = effective
                .folders
                .iter()
                .map(|f| (PathBuf::from(&f.path), f.from))
                .collect();
            if folders.is_empty()
                && let Some(path) = unsaved
            {
                folders.push((path, Owner::Project(project)));
            }
            let paths: Vec<PathBuf> = folders.iter().map(|(p, _)| p.clone()).collect();
            let focus = focus_of(owner, project, epic);
            let result = acta_terminal::launch_spec_for_claude(&paths, &focus);
            let primary_from = folders.first().map(|(_, from)| *from);
            Ok((result, primary_from))
        });
        // The previous session (if any) ends when its view is dropped.
        let old = self.panes.insert(project, Pane::Starting);
        drop(old);
        cx.notify();
        cx.spawn_in(window, async move |dock, cx| {
            let Ok((result, primary_from)) = task.await else {
                return;
            };
            let _ = dock.update_in(cx, |dock, window, cx| {
                let pane = match result {
                    Ok(launch) => {
                        if !launch.left_out.is_empty() {
                            let locale = settings::locale(cx);
                            for folder in &launch.left_out {
                                let name =
                                    paths_view::last_segment(&folder.to_string_lossy()).to_owned();
                                let message = tr(
                                    locale,
                                    Key::TerminalLeftOut,
                                    &[("folder", Arg::Text(&name))],
                                );
                                dock.ws
                                    .update(cx, |ws, cx| ws.toast(message, ToastKind::Error, cx));
                            }
                        }
                        dock.running(origin, launch.spec, window, cx)
                    }
                    Err(LaunchError::NoFolder) => Pane::Ask { origin, save: true },
                    Err(LaunchError::PrimaryMissing(path)) => Pane::Missing {
                        origin,
                        path,
                        from: primary_from.unwrap_or(owner),
                    },
                    Err(LaunchError::ClaudeNotFound) => Pane::NotFound { origin },
                };
                dock.panes.insert(project, pane);
                cx.notify();
            });
        })
        .detach();
    }

    fn running(
        &mut self,
        origin: Origin,
        spec: acta_terminal::LaunchSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Pane {
        let mode = cx.global::<Appearance>().mode;
        let size = self.text_size;
        let locale = settings::locale(cx);
        let macos = cfg!(target_os = "macos");
        let ended = tr(
            locale,
            Key::TerminalEnded,
            &[("keys", Arg::Text(KeyName::Enter.label(locale, macos)))],
        );
        let view = cx.new(|cx| {
            let mut view = TerminalView::spawn(spec, cx);
            view.set_theme(theme_for(mode), cx);
            view.set_text_size(size, cx);
            view.set_ended_message(ended.into(), cx);
            view
        });
        let project = origin.project;
        let restart_origin = origin.clone();
        let sub = cx.subscribe_in(
            &view,
            window,
            move |dock, _, event: &TerminalEvent, window, cx| {
                match event {
                    // Restarting uses the folders of the page open at that
                    // moment; the dock restarts from where it was started when
                    // it does not know the page.
                    TerminalEvent::RestartRequested => {
                        dock.restart(restart_origin.clone(), window, cx)
                    }
                    TerminalEvent::TextSizeChanged(size) => {
                        dock.text_size = *size;
                        let stored = size.round() as u8;
                        settings::change(cx, |prefs| prefs.terminal_text_size = Some(stored));
                    }
                    _ => {}
                }
            },
        );
        let _ = project;
        view.read(cx).focus_handle(cx).focus(window, cx);
        Pane::Running {
            view,
            origin,
            _sub: sub,
        }
    }

    pub fn change_text_size(&mut self, by: f32, cx: &mut Context<Self>) {
        for pane in self.panes.values() {
            if let Pane::Running { view, .. } = pane {
                view.update(cx, |view, cx| {
                    let size = view.text_size() + by;
                    view.set_text_size(size, cx)
                });
            }
        }
    }

    fn choose_folder(
        &mut self,
        origin: Origin,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn_in(window, async move |dock, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.pop() else { return };
            let _ = dock.update_in(cx, |dock, window, cx| {
                let project = origin.project;
                if save {
                    let task = dock.ws.update(cx, |ws, cx| {
                        ws.write(cx, Key::ErrorSave, move |store| {
                            store.add_folder(Owner::Project(project), &path, Author::Human)
                        })
                    });
                    cx.spawn_in(window, async move |dock, cx| {
                        if task.await.is_some() {
                            let _ = dock
                                .update_in(cx, |dock, window, cx| dock.launch(origin, window, cx));
                        }
                    })
                    .detach();
                } else {
                    dock.unsaved.insert(project, path);
                    dock.launch(origin, window, cx);
                }
            });
        })
        .detach();
    }

    fn locate(
        &mut self,
        origin: Origin,
        path: PathBuf,
        from: Owner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = settings::locale(cx);
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(text(locale, Key::FoldersPicker).into()),
        });
        cx.spawn_in(window, async move |dock, cx| {
            let Ok(Ok(Some(mut paths))) = chosen.await else {
                return;
            };
            let Some(new) = paths.pop() else { return };
            let _ = dock.update_in(cx, |dock, window, cx| {
                let old = path.to_string_lossy().into_owned();
                let task = dock.ws.update(cx, |ws, cx| {
                    ws.write(cx, Key::ErrorSave, move |store| {
                        let folder = store
                            .folders(from)?
                            .into_iter()
                            .find(|f| f.path == old)
                            .ok_or_else(|| acta_core::Error::NotFound("folder".into()))?;
                        store.set_folder_path(folder.id, &new, Author::Human)
                    })
                });
                cx.spawn_in(window, async move |dock, cx| {
                    if task.await.is_some() {
                        let _ =
                            dock.update_in(cx, |dock, window, cx| dock.launch(origin, window, cx));
                    }
                })
                .detach();
            });
        })
        .detach();
    }

    /// The header's context: "on topic Embed the terminal".
    fn context(origin: &Origin, locale: Locale) -> String {
        let key = match origin.owner {
            Owner::Topic(_) => Key::TerminalOnTopic,
            Owner::Epic(_) => Key::TerminalOnEpic,
            Owner::Project(_) => Key::TerminalOnProject,
        };
        tr(locale, key, &[("name", Arg::Text(&origin.name))])
    }

    fn render_dock(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let active = self.active.clone();
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let mode = cx.global::<Appearance>().mode;
        if self.mode != Some(mode) {
            self.mode = Some(mode);
            for pane in self.panes.values() {
                if let Pane::Running { view, .. } = pane {
                    view.update(cx, |view, cx| view.set_theme(theme_for(mode), cx));
                }
            }
        }
        let pane = self.project.and_then(|p| self.panes.get(&p));
        let origin = match pane {
            Some(Pane::Running { origin, .. })
            | Some(Pane::Ask { origin, .. })
            | Some(Pane::Missing { origin, .. })
            | Some(Pane::NotFound { origin }) => Some(origin.clone()),
            _ => None,
        };
        let mismatch = match (&origin, &active) {
            (Some(started), Some(active)) => started.owner != active.owner,
            _ => false,
        };
        let header = div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .h(px(HEADER))
            .px(px(8.))
            .bg(t.bg_sunken)
            .border_b_1()
            .border_color(t.border_subtle)
            .child(ui::label(text(locale, Key::TerminalTitle), &t))
            .children(origin.as_ref().map(|o| {
                type_scale::SMALL
                    .el()
                    .min_w_0()
                    .truncate()
                    .text_color(if mismatch {
                        t.text_primary
                    } else {
                        t.text_tertiary
                    })
                    .child(Self::context(o, locale))
            }))
            .when(mismatch, |this| {
                this.child(
                    ui::button(
                        "terminal-restart-text",
                        text(locale, Key::TerminalRestartHere),
                        Variant::Ghost,
                        false,
                        &t,
                    )
                    .h(px(24.))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DockEvent::RestartHere))),
                )
            })
            .child(div().flex_1())
            .child(
                ui::ghost_icon_button(
                    "terminal-restart",
                    Icon::RotateCw,
                    24.,
                    false,
                    false,
                    true,
                    &t,
                )
                .on_click(cx.listener(|_, _, _, cx| cx.emit(DockEvent::RestartHere))),
            )
            .child(
                ui::ghost_icon_button("terminal-hide", Icon::Minus, 24., false, false, true, &t)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DockEvent::Hide))),
            );
        let surface = match mode {
            Mode::Dark => t.bg_sunken,
            Mode::Light => t.bg_raised,
        };
        let body: AnyElement = match pane {
            Some(Pane::Running { view, .. }) => div()
                .flex_1()
                .min_h_0()
                .child(view.clone())
                .into_any_element(),
            Some(Pane::Ask { origin, save }) => {
                let origin = origin.clone();
                let save = *save;
                let project_name = self.project_name(origin.project, cx);
                let sentence = match origin.owner {
                    Owner::Topic(_) => tr(
                        locale,
                        Key::TerminalAskTopic,
                        &[("name", Arg::Text(&origin.name))],
                    ),
                    Owner::Epic(_) => tr(
                        locale,
                        Key::TerminalAskEpic,
                        &[("name", Arg::Text(&origin.name))],
                    ),
                    Owner::Project(_) => tr(
                        locale,
                        Key::TerminalAskProject,
                        &[("name", Arg::Text(&origin.name))],
                    ),
                };
                let project = origin.project;
                message(&t, surface)
                    .child(
                        type_scale::HEADING
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::TerminalAskTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_secondary)
                            .child(sentence),
                    )
                    .child(
                        div()
                            .id("terminal-save")
                            .mt(px(16.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .on_click(cx.listener(move |dock, _, _, cx| {
                                if let Some(Pane::Ask { save, .. }) = dock.panes.get_mut(&project) {
                                    *save = !*save;
                                }
                                cx.notify();
                            }))
                            .child(checkbox(save, &t))
                            .child(type_scale::UI.el().text_color(t.text_primary).child(tr(
                                locale,
                                Key::TerminalAskSave,
                                &[("name", Arg::Text(&project_name))],
                            ))),
                    )
                    .child(
                        type_scale::SMALL
                            .el()
                            .mt(px(4.))
                            .pl(px(24.))
                            .text_color(t.text_tertiary)
                            .child(text(locale, Key::TerminalAskUnsaved)),
                    )
                    .child(
                        div().mt(px(24.)).flex().child(
                            ui::button(
                                "terminal-choose",
                                text(locale, Key::TerminalAskChoose),
                                Variant::Primary,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |dock, _, window, cx| {
                                    dock.choose_folder(origin.clone(), save, window, cx)
                                },
                            )),
                        ),
                    )
                    .into_any_element()
            }
            Some(Pane::Missing { origin, path, from }) => {
                let (origin, path, from) = (origin.clone(), path.clone(), *from);
                let level_name = self.owner_name(from, cx);
                let body_key = Key::TerminalMissingBody;
                let retry_origin = origin.clone();
                message(&t, surface)
                    .child(
                        type_scale::HEADING
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::TerminalMissingTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_secondary)
                            .child(tr(locale, body_key, &[("name", Arg::Text(&level_name))])),
                    )
                    .child(
                        type_scale::CODE
                            .el()
                            .mt(px(8.))
                            .text_color(t.text_primary)
                            .child(path.to_string_lossy().into_owned()),
                    )
                    .child(
                        div()
                            .mt(px(24.))
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(
                                ui::button(
                                    "terminal-locate",
                                    text(locale, Key::TerminalMissingLocate),
                                    Variant::Primary,
                                    false,
                                    &t,
                                )
                                .on_click(cx.listener(
                                    move |dock, _, window, cx| {
                                        dock.locate(origin.clone(), path.clone(), from, window, cx)
                                    },
                                )),
                            )
                            .child(
                                ui::button(
                                    "terminal-retry",
                                    text(locale, Key::CommonTryAgain),
                                    Variant::Secondary,
                                    false,
                                    &t,
                                )
                                .on_click(cx.listener(
                                    move |dock, _, window, cx| {
                                        dock.launch(retry_origin.clone(), window, cx)
                                    },
                                )),
                            ),
                    )
                    .into_any_element()
            }
            Some(Pane::NotFound { origin }) => {
                let origin = origin.clone();
                message(&t, surface)
                    .child(
                        type_scale::HEADING
                            .el()
                            .text_color(t.text_primary)
                            .child(text(locale, Key::StatusbarNotFound)),
                    )
                    .child(
                        div().mt(px(24.)).flex().child(
                            ui::button(
                                "terminal-retry",
                                text(locale, Key::CommonTryAgain),
                                Variant::Secondary,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |dock, _, window, cx| dock.launch(origin.clone(), window, cx),
                            )),
                        ),
                    )
                    .into_any_element()
            }
            Some(Pane::Starting) | None => div().flex_1().bg(surface).into_any_element(),
        };
        div()
            .id("terminal-dock")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(WIDTH))
            .min_w(px(400.))
            .h_full()
            .bg(surface)
            .border_l_1()
            .border_color(t.border_subtle)
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn project_name(&self, id: ProjectId, cx: &App) -> String {
        self.ws
            .read(cx)
            .projects
            .iter()
            .find(|t| t.project.id == id)
            .map(|t| t.project.title.clone())
            .unwrap_or_default()
    }

    fn owner_name(&self, owner: Owner, cx: &App) -> String {
        let ws = self.ws.read(cx);
        for tree in &ws.projects {
            match owner {
                Owner::Project(id) if tree.project.id == id => return tree.project.title.clone(),
                Owner::Epic(id) => {
                    if let Some((e, _)) = tree.epics.iter().find(|(e, _)| e.id == id) {
                        return e.title.clone();
                    }
                }
                Owner::Topic(id) => {
                    for t in tree
                        .loose_topics
                        .iter()
                        .chain(tree.epics.iter().flat_map(|(_, t)| t))
                    {
                        if t.id == id {
                            return t.title.clone();
                        }
                    }
                }
                _ => {}
            }
        }
        String::new()
    }
}

fn theme_for(mode: Mode) -> TerminalTheme {
    match mode {
        Mode::Dark => TerminalTheme::dark(),
        Mode::Light => TerminalTheme::light(),
    }
}

/// The empty-state message on the terminal's surface: padding 12, text at
/// most 480 wide (screens.md 8.11).
fn message(t: &Tokens, surface: gpui_kit::Hsla) -> gpui_kit::Div {
    let _ = t;
    div()
        .flex_1()
        .flex()
        .flex_col()
        .p(px(12.))
        .max_w(px(480. + 24.))
        .bg(surface)
}

fn checkbox(checked: bool, t: &Tokens) -> gpui_kit::Div {
    div()
        .size(px(16.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RADIUS_SM))
        .when(checked, |this| {
            this.bg(t.accent)
                .child(icon(Icon::Check, 12., t.text_on_accent))
        })
        .when(!checked, |this| {
            this.border_1().border_color(t.border_strong)
        })
}

impl Render for TerminalDock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_dock(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{EpicId, TopicId};

    #[test]
    fn the_focus_names_every_level_of_the_page() {
        let f = focus_of(Owner::Topic(TopicId(42)), ProjectId(1), Some(EpicId(3)));
        assert_eq!(f.project, "P-1");
        assert_eq!(f.epic.as_deref(), Some("E-3"));
        assert_eq!(f.topic.as_deref(), Some("T-42"));
        let f = focus_of(Owner::Project(ProjectId(2)), ProjectId(2), None);
        assert_eq!((f.epic, f.topic), (None, None));
    }
}
