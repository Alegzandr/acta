//! Acta, the GUI executable. Velopack's main exe: the only process that
//! installs, updates or uninstalls anything (ADR 0004).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli_export;
#[cfg(any(target_os = "linux", test))]
mod desktop_entry;
mod first_run;
mod fonts;
#[allow(
    dead_code,
    reason = "the whole catalogue of DESIGN.md 10.4 is loaded now; screens reference keys as they arrive"
)]
mod i18n;
mod icons;
mod log;
mod preferences;
mod settings;
mod settings_page;
mod shell;
mod specimen;
mod theme;
mod titlebar;
mod type_scale;
mod ui;
#[cfg(any(windows, test))]
mod uninstall;
mod update;
mod updater_window;
mod user_path;

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{AnyWindowHandle, App, AppContext};
use velopack::sources::GithubSource;
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

fn main() {
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
            // updater window can show while it lasts. The process exits
            // from the worker instead if the update is applied.
            let state = Arc::new(LaunchState::default());
            let waiting = Arc::clone(&state);
            let spawned = std::thread::Builder::new()
                .name("acta-launch".into())
                .spawn(move || {
                    update::run_at_launch(manager, restarted_by_updater, Arc::clone(&waiting));
                    waiting.set_ready();
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

    run_app(options, pending, launched);
}

/// The update manager of this install, or `None` when the executable was not
/// installed by Velopack. Creating it reads local files only.
fn velopack_manager() -> Option<UpdateManager> {
    UpdateManager::new(
        GithubSource::new(acta_core::REPO_URL, None, false),
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
    let app = VelopackApp::build();
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
            shell::bind_keys(cx);

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
/// window is late, and close it in the frame the main window opens.
async fn wait_for_launch(
    options: Options,
    state: Arc<LaunchState>,
    launched: Instant,
    cx: &mut gpui_kit::AsyncApp,
) {
    const TICK: Duration = Duration::from_millis(16);
    let mut updater: Option<AnyWindowHandle> = None;
    loop {
        match update::launch_step(launched.elapsed(), state.is_ready(), updater.is_some()) {
            LaunchStep::Wait => {
                if let Some(handle) = updater {
                    // The status line and the bar follow the worker.
                    let _ = handle.update(cx, |_, window, _| window.refresh());
                }
            }
            LaunchStep::ShowUpdaterWindow => {
                let state = Arc::clone(&state);
                updater = cx.update(|cx| {
                    let window = updater_window::window_options(!options.no_focus, cx);
                    match cx.open_window(window, |_, cx| cx.new(|_| UpdaterWindow::new(state))) {
                        Ok(handle) => Some(handle.into()),
                        Err(err) => {
                            log::line(&format!("could not open the updater window: {err}"));
                            // Keep waiting without it; never try again.
                            None
                        }
                    }
                });
                if updater.is_none() {
                    break;
                }
            }
            LaunchStep::OpenMainWindow => break,
        }
        cx.background_executor().timer(TICK).await;
    }
    // Without an updater window (it failed to open) the wait goes on here.
    while !state.is_ready() {
        cx.background_executor().timer(TICK).await;
    }
    cx.update(|cx| {
        open_main(options, cx);
        if let Some(handle) = updater {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    });
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
