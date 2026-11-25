//! Acta, the GUI executable. Velopack's main exe: the only process that
//! installs, updates or uninstalls anything (ADR 0004).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod attach;
mod attach_ui;
#[cfg(windows)]
mod browser_dock;
#[cfg(windows)]
mod browser_host;
#[cfg(windows)]
mod browser_panel;
mod capture;
#[cfg(windows)]
mod chat;
#[cfg(windows)]
mod chat_panel;
#[cfg(windows)]
mod chat_windows;
mod claude_setup;
mod cli_export;
mod commands;
mod contact_page;
mod contact_picker;
mod contacts_model;
mod contacts_page;
mod db;
#[cfg(any(target_os = "linux", test))]
mod desktop_entry;
mod dev_seed;
mod dialogs;
mod diff;
mod due;
mod due_picker;
mod first_run;
mod fold;
mod fold_ui;
mod folders_model;
mod folders_view;
mod fonts;
mod i18n;
mod icons;
mod inbox_page;
mod kb_model;
mod kb_page;
mod kbq_model;
mod kbq_ui;
mod layout;
mod layout_frame;
mod log;
mod markdown_view;
mod menu;
mod palette;
mod palette_model;
mod paths_view;
mod preferences;
mod question_view;
mod questions_model;
mod questions_page;
mod ready;
mod ready_model;
mod record;
mod record_page;
mod route;
mod sessions;
mod sessions_view;
mod settings;
mod settings_page;
mod shell;
mod shortcuts_page;
mod specimen;
mod table;
mod terminal_dock;
mod theme;
mod titlebar;
mod tool_strip;
mod tools_model;
mod trash_page;
mod tree;
mod type_scale;
mod ui;
#[cfg(any(windows, test))]
mod uninstall;
mod update;
mod update_source;
mod updater_window;
mod user_path;
mod voice;
mod voice_worker;
#[cfg(any(windows, target_os = "linux"))]
mod vulkan_shim;
#[cfg(windows)]
mod webview_build;
mod when;
mod workspace;
mod zoom;

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{AnyWindowHandle, App, AppContext};
use velopack::{UpdateManager, VelopackApp};

use acta_core::paths::Os;
use cli_export::Layout;
use shell::{Page, Shell, Start};
use update::{LaunchState, LaunchStep};
use updater_window::UpdaterWindow;

/// How this run was asked to start. Everything but the defaults is a
/// development or test aid, not a user-facing option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Options {
    /// `--specimen` or `ACTA_SPECIMEN=1`: the type specimen window.
    specimen: bool,
    /// `--open=settings`: the window starts on the Settings page.
    /// `--sidebar`: the window starts with the sidebar shown.
    start: Start,
    /// `--no-focus`: windows open without taking the foreground, for
    /// captures on a machine someone is using.
    no_focus: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>, specimen_env: bool) -> Self {
        let mut options = Options {
            specimen: specimen_env,
            ..Options::default()
        };
        for arg in args {
            match arg.as_str() {
                "--specimen" => options.specimen = true,
                "--open=settings" => options.start.page = Page::Settings,
                "--sidebar" => options.start.sidebar = true,
                "--no-focus" => options.no_focus = true,
                _ => {}
            }
        }
        options
    }
}

/// The working directory Acta was started in, before it left it.
static LAUNCH_DIR: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();

/// ADR 0008, "Live files": on Windows no folder can be renamed or deleted
/// while it, or a folder inside it, is some process's working directory.
/// Started from a terminal in a project, Acta (and every process it starts
/// without a directory of its own) would hold that project for as long as
/// it runs; it moves to its own folder instead.
fn leave_launch_dir() {
    let _ = LAUNCH_DIR.set(std::env::current_dir().ok());
    if let Some(own) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        && std::env::set_current_dir(&own).is_err()
    {
        log::line("could not leave the launch directory");
    }
}

/// The directory Acta was started in (the dev seed's project folder).
pub fn launch_dir() -> Option<std::path::PathBuf> {
    LAUNCH_DIR.get().cloned().flatten()
}

fn main() {
    // The dictation worker (ADR 0014, 5): before anything else, so it never
    // opens a window, takes the single-instance lock or looks for updates.
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new(acta_core::voice::protocol::WORKER_ARG))
    {
        std::process::exit(voice_worker::run());
    }
    // SAFETY: first thing in `main` after the worker check, before any
    // thread exists.
    unsafe { acta_terminal::launch::scrub_claude_session() };
    let launched = Instant::now();
    log::rotate();
    let restarted_by_updater = velopack_bootstrap();

    // `Some` only when this executable was installed by Velopack. From a
    // build directory (cargo run, CI) nothing below exports, edits the PATH
    // or looks for updates.
    let installed = velopack_manager();
    if installed.is_none() {
        log::line("not a Velopack install; no export, no updates");
    }

    let options = Options::parse(
        std::env::args().skip(1),
        std::env::var_os("ACTA_SPECIMEN").is_some_and(|value| value == "1"),
    );
    leave_launch_dir();
    let mut pending = None;

    if let Some(manager) = installed {
        // Repairs the export if a hook was missed; cheap when up to date.
        let portable = manager.get_is_portable();
        let spawned = std::thread::Builder::new()
            .name("acta-integration".into())
            .spawn(move || {
                export_cli("start", portable);
                integrate_with_desktop();
            });
        if let Err(err) = spawned {
            log::line(&format!(
                "export: could not start the thread: {:?}",
                err.kind()
            ));
        }
        if !options.specimen {
            // The wait for the update decision runs beside the UI, so the
            // updater window can show while it lasts. An apply is handed
            // to the UI thread, which ends the process with it.
            let state = Arc::new(LaunchState::default());
            let waiting = Arc::clone(&state);
            let spawned = std::thread::Builder::new()
                .name("acta-launch".into())
                .spawn(move || {
                    // Ready however this ends, a panic included.
                    let _ready = update::ReadyOnDrop(Arc::clone(&waiting));
                    update::run_at_launch(manager, restarted_by_updater, waiting);
                });
            match spawned {
                Ok(_) => pending = Some(state),
                Err(err) => log::line(&format!(
                    "update: could not start the launch wait: {:?}",
                    err.kind()
                )),
            }
        }
    }

    // The agent browser's "Acta runs here" mark (ADR 0010), held until exit.
    #[cfg(windows)]
    let _app_mutex = browser_host::hold_app_mutex();
    run_app(options, pending, launched);
}

/// The update manager of this install, or `None` when the executable was not
/// installed by Velopack. Creating it reads local files only.
fn velopack_manager() -> Option<UpdateManager> {
    UpdateManager::new(
        update_source::LatestRelease::new(acta_core::REPO_URL),
        None,
        None,
    )
    .ok()
}

/// Whether this install exports the CLI and edits the PATH.
///
/// A portable Windows install (an unpacked `Portable.zip`) does not: it has
/// no uninstaller, so nothing would ever take the export and the PATH entry
/// away again. Velopack reports every macOS and Linux install as portable
/// (`UpdateManager::get_is_portable`); there the export is the only way the
/// CLI reaches the user, so the flag is ignored.
fn exports_cli(os: Os, portable: bool) -> bool {
    !(os == Os::Windows && portable)
}

/// Must run before anything else: Velopack starts this executable with hook
/// arguments at install, update and uninstall, and expects it to exit fast.
/// It also applies an update that a previous run downloaded.
///
/// Returns whether Velopack's updater started this launch.
fn velopack_bootstrap() -> bool {
    // Before `run`, which removes the variable.
    let restarted = restarted_by_updater();
    // Velopack's startup apply shows its updater's native dialog; Acta
    // applies a pending package itself, silently (update.rs).
    let app = VelopackApp::build().set_auto_apply_on_startup(false);
    // Velopack only has these hooks on Windows. Elsewhere the export is done
    // by the check at every start, and there is no uninstall step to hook.
    #[cfg(windows)]
    let app = app
        .on_after_install_fast_callback(|_| export_cli_from_hook("after-install"))
        .on_after_update_fast_callback(|_| export_cli_from_hook("after-update"))
        .on_before_uninstall_fast_callback(|_| match Layout::locate() {
            Some(layout) => {
                if uninstall::run(&layout, &user_path::USER_PATH) {
                    // Last: PATH, processes and files are already dealt with.
                    user_path::announce_environment_change();
                }
            }
            None => log::line("uninstall: user directories not found; nothing removed"),
        });
    let mut app = app;
    app.run();
    restarted
}

/// Whether Velopack's updater started this process: it sets
/// `VELOPACK_RESTART` when it restarts the app after applying an update, and
/// also after failing to apply one. Only meaningful before
/// `VelopackApp::run`, which removes the variable; hence private to
/// [`velopack_bootstrap`].
fn restarted_by_updater() -> bool {
    std::env::var_os("VELOPACK_RESTART").is_some()
}

/// The export as run from a Velopack hook, which has no manager at hand.
#[cfg(windows)]
fn export_cli_from_hook(when: &str) {
    match velopack_manager() {
        Some(manager) => export_cli(when, manager.get_is_portable()),
        None => log::line(&format!("export ({when}): not a Velopack install")),
    }
}

/// Export the CLI and put it on the PATH. Every failure is logged and
/// swallowed: none of this may keep the window from opening.
fn export_cli(when: &str, portable: bool) {
    if !exports_cli(Os::CURRENT, portable) {
        log::line(&format!(
            "export ({when}): portable install; CLI not exported, PATH not edited"
        ));
        return;
    }
    let Some(layout) = Layout::locate() else {
        log::line(&format!("export ({when}): user directories not found"));
        return;
    };
    match cli_export::export(&layout, acta_core::VERSION, log::now_ms()) {
        Ok(outcome) => log::line(&format!(
            "export ({when}): {:?}, {} old file(s) still in use",
            outcome.decision, outcome.still_locked
        )),
        Err(err) => {
            log::line(&format!("export ({when}): failed: {:?}", err.kind()));
            return;
        }
    }
    match user_path::publish(&layout) {
        Ok(published) => {
            log::line(&format!("export ({when}): {}", published.note));
            if published.environment_changed {
                // Last: nothing waits behind the broadcast.
                user_path::announce_environment_change();
            }
        }
        Err(err) => log::line(&format!(
            "export ({when}): PATH not updated: {:?}",
            err.kind()
        )),
    }
}

/// Linux: give the AppImage its launcher entry and icon (ADR 0004). Only an
/// installed build gets here, and only one started as an AppImage has a
/// stable file to point the entry at.
fn integrate_with_desktop() {
    #[cfg(target_os = "linux")]
    {
        use std::path::PathBuf;

        let appimage = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute());
        let data_home = acta_core::paths::xdg_data_home(&acta_core::paths::BaseDirs::from_env());
        let (Some(appimage), Some(data_home)) = (appimage, data_home) else {
            log::line("desktop entry: not an AppImage, or no data directory; skipped");
            return;
        };
        match desktop_entry::install(&data_home, &appimage) {
            Ok(true) => log::line("desktop entry: written"),
            Ok(false) => log::line("desktop entry: already in place"),
            Err(err) => log::line(&format!("desktop entry: failed: {:?}", err.kind())),
        }
    }
}

fn run_app(options: Options, pending: Option<Arc<LaunchState>>, launched: Instant) {
    gpui_kit::application()
        .with_assets(icons::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            fonts::register(cx);
            settings::init(cx);
            voice::init(cx);
            due::start(cx);
            shell::bind_keys(cx);
            // App-level quit asks first while an install runs (ADR 0013).
            cx.on_action(|_: &shell::QuitActa, cx| shell::request_quit(cx));
            if cfg!(target_os = "macos") {
                let quit = i18n::text(settings::locale(cx), i18n::Key::QuitConfirm);
                cx.set_menus([gpui_kit::Menu::new(acta_core::APP_NAME)
                    .items([gpui_kit::MenuItem::action(quit, shell::QuitActa)])]);
                cx.bind_keys([gpui_kit::KeyBinding::new("cmd-q", shell::QuitActa, None)]);
            }

            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            match pending {
                None => open_main(options, cx),
                Some(state) => {
                    cx.spawn(async move |cx| wait_for_launch(options, state, launched, cx).await)
                        .detach();
                }
            }
            cx.activate(!options.no_focus);
        });
}

/// Tick until the update decision; show the updater window if the main
/// window is late, and close it in the frame the main window opens. A
/// posted apply runs here, on the UI thread, with every window closed and
/// the preferences written first.
async fn wait_for_launch(
    options: Options,
    state: Arc<LaunchState>,
    launched: Instant,
    cx: &mut gpui_kit::AsyncApp,
) {
    const TICK: Duration = Duration::from_millis(16);
    let mut updater: Option<AnyWindowHandle> = None;
    let mut updater_shown = false;
    let mut shown_status = None;
    loop {
        // Ready is read before the apply: a worker posts its apply before
        // the launch becomes ready, so a ready launch never hides one.
        let ready = state.is_ready();
        let apply = state.take_apply();
        match update::launch_step(launched.elapsed(), ready, apply.is_some(), updater_shown) {
            LaunchStep::Wait => {
                let status = state.status();
                if let Some(handle) = updater
                    && shown_status != Some(status)
                {
                    // Repaint only when the status line or the bar change.
                    shown_status = Some(status);
                    let _ = handle.update(cx, |_, window, _| window.refresh());
                }
            }
            LaunchStep::Apply => {
                let Some(apply) = apply else { continue };
                cx.update(|cx| {
                    apply_at_launch(
                        apply,
                        &mut GpuiLaunch {
                            options,
                            updater,
                            cx,
                        },
                    );
                });
                return;
            }
            LaunchStep::ShowUpdaterWindow => {
                updater_shown = true;
                let state = Arc::clone(&state);
                updater = cx.update(|cx| {
                    let window_options = updater_window::window_options(!options.no_focus, cx);
                    let opened = cx.open_window(window_options, |window, cx| {
                        cx.new(|cx| UpdaterWindow::new(state, window, cx))
                    });
                    match opened {
                        Ok(handle) => Some(handle.into()),
                        Err(err) => {
                            // Keep waiting without it; never try again.
                            log::line(&format!("could not open the updater window: {err}"));
                            None
                        }
                    }
                });
            }
            LaunchStep::OpenMainWindow => {
                cx.update(|cx| {
                    open_main(options, cx);
                    if let Some(handle) = updater {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                });
                return;
            }
        }
        cx.background_executor().timer(TICK).await;
    }
}

/// What [`apply_at_launch`] does to the app, so its order can be tested
/// without a display.
trait Launch {
    fn flush_preferences(&mut self);
    fn open_main_window(&mut self);
    fn close_updater_window(&mut self);
}

struct GpuiLaunch<'a> {
    options: Options,
    updater: Option<AnyWindowHandle>,
    cx: &'a mut App,
}

impl Launch for GpuiLaunch<'_> {
    fn flush_preferences(&mut self) {
        settings::flush(self.cx);
    }

    fn open_main_window(&mut self) {
        open_main(self.options, self.cx);
    }

    fn close_updater_window(&mut self) {
        if let Some(handle) = self.updater.take() {
            let _ = handle.update(self.cx, |_, window, _| window.remove_window());
        }
    }
}

/// Run a posted apply on the UI thread: write the preferences, then apply,
/// which ends the process on success. No window is closed first: closing
/// the last one queues GPUI's quit, which a failed apply could not take
/// back. On failure (Velopack applies the package at the next launch) the
/// main window opens first and the updater window closes after it, as when
/// no update is applied, so the app is never without a window.
fn apply_at_launch(apply: update::Apply, launch: &mut impl Launch) {
    launch.flush_preferences();
    if let Err(err) = apply() {
        log::line(&format!("update: {err}; opening without it"));
        launch.open_main_window();
        launch.close_updater_window();
    }
}

/// Open the main window (or the specimen). Without a window there is
/// nothing to show an error in, so a failure is logged and the app quits.
fn open_main(options: Options, cx: &mut App) {
    let focus = !options.no_focus;
    let opened = if options.specimen {
        let window = shell::window_options(specimen::WINDOW_SIZE, focus, cx);
        gpui_kit::open_window(window, cx, |window, cx| {
            let view = cx.new(|_| specimen::Specimen);
            cx.new(|cx| Shell::specimen(view.into(), window, cx))
        })
    } else {
        let window = shell::window_options(shell::DEFAULT_SIZE, focus, cx);
        gpui_kit::open_window(window, cx, |window, cx| {
            cx.new(|cx| Shell::workbench(options.start, window, cx))
        })
    };
    if let Err(err) = opened {
        log::line(&format!("could not open the window: {err}"));
        cx.quit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The windows of a launch with the updater window showing, and every
    /// call in order. The app quits when its last window closes.
    struct FakeLaunch {
        windows: usize,
        quit: bool,
        calls: Vec<&'static str>,
    }

    impl Launch for FakeLaunch {
        fn flush_preferences(&mut self) {
            self.calls.push("flush");
        }

        fn open_main_window(&mut self) {
            self.calls.push("open main");
            self.windows += 1;
        }

        fn close_updater_window(&mut self) {
            self.calls.push("close updater");
            self.windows -= 1;
            self.quit |= self.windows == 0;
        }
    }

    fn launch_with_updater() -> FakeLaunch {
        FakeLaunch {
            windows: 1,
            quit: false,
            calls: Vec::new(),
        }
    }

    #[test]
    fn a_failed_apply_opens_the_main_window_and_never_quits() {
        let mut launch = launch_with_updater();
        apply_at_launch(
            Box::new(|| Err("apply failed: other error".into())),
            &mut launch,
        );
        assert_eq!(launch.calls, ["flush", "open main", "close updater"]);
        assert_eq!(launch.windows, 1, "the main window is open");
        assert!(!launch.quit);
    }

    #[test]
    fn preferences_are_written_before_the_apply_and_no_window_is_closed() {
        let mut launch = launch_with_updater();
        // A real apply exits the process; this one records that it ran.
        apply_at_launch(Box::new(|| Ok(())), &mut launch);
        assert_eq!(launch.calls, ["flush"]);
        assert_eq!(launch.windows, 1);
        assert!(!launch.quit);
    }

    #[test]
    fn options_default_to_the_product_and_read_the_aids() {
        let parse =
            |args: &[&str], env| Options::parse(args.iter().map(|arg| arg.to_string()), env);
        assert_eq!(parse(&[], false), Options::default());
        assert_eq!(Options::default().start.page, Page::FirstRun);
        assert!(!Options::default().start.sidebar);
        assert!(parse(&[], true).specimen);
        let all = parse(
            &["--open=settings", "--sidebar", "--no-focus", "--x"],
            false,
        );
        assert_eq!(all.start.page, Page::Settings);
        assert!(all.start.sidebar && all.no_focus && !all.specimen);
    }

    #[test]
    fn only_a_portable_windows_install_skips_the_export() {
        assert!(exports_cli(Os::Windows, false));
        assert!(!exports_cli(Os::Windows, true));
        // Velopack calls every macOS and Linux install portable.
        assert!(exports_cli(Os::MacOs, true));
        assert!(exports_cli(Os::Linux, true));
    }
}
