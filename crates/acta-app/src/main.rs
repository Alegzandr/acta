//! Acta, the GUI executable. Velopack's main exe: the only process that
//! installs, updates or uninstalls anything (ADR 0004).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli_export;
#[cfg(any(target_os = "linux", test))]
mod desktop_entry;
mod first_run;
mod fonts;
mod log;
mod shell;
mod specimen;
mod theme;
mod titlebar;
mod type_scale;
#[cfg(any(windows, test))]
mod uninstall;
mod update;
mod user_path;

use gpui_kit::{App, AppContext, Render};
use velopack::sources::GithubSource;
use velopack::{UpdateManager, VelopackApp};

use acta_core::paths::Os;
use cli_export::Layout;
use shell::Shell;

fn main() {
    log::rotate();
    // Set by Velopack's updater when it starts the app after applying, or
    // after failing to apply. `VelopackApp::run` removes it, so read it first.
    let restarted_by_updater = std::env::var_os("VELOPACK_RESTART").is_some();
    velopack_bootstrap();

    // `Some` only when this executable was installed by Velopack. From a
    // build directory (cargo run, CI) nothing below exports, edits the PATH
    // or looks for updates.
    let installed = velopack_manager();
    if installed.is_none() {
        log::line("not a Velopack install; no export, no updates");
    }

    // Development aid: `Acta --specimen`, or `ACTA_SPECIMEN=1`.
    let specimen = std::env::args().skip(1).any(|arg| arg == "--specimen")
        || std::env::var_os("ACTA_SPECIMEN").is_some_and(|value| value == "1");

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
            log::line(&format!("export: could not start the thread: {err}"));
        }
        if !specimen {
            // Returns when the window may open; exits instead if it applies.
            update::run_at_launch(manager, restarted_by_updater);
        }
    }

    run_app(specimen);
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
fn velopack_bootstrap() {
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

fn run_app(specimen: bool) {
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        fonts::register(cx);
        theme::install(theme::Tokens::dark(), cx);

        if specimen {
            open(specimen::WINDOW_SIZE, cx, |_| specimen::Specimen);
        } else {
            open(shell::DEFAULT_SIZE, cx, |_| first_run::FirstRun);
        }

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

/// Open an Acta window around `content`. Without a window there is nothing
/// to show an error in, so a failure is logged and the app quits.
fn open<V: Render>(
    default_size: (f32, f32),
    cx: &mut App,
    content: impl FnOnce(&mut gpui_kit::Context<V>) -> V + 'static,
) {
    let options = shell::window_options(default_size, cx);
    let opened = gpui_kit::open_window(options, cx, |_, cx| {
        let content = cx.new(content);
        cx.new(|cx| Shell::new(content.into(), cx))
    });
    if let Err(err) = opened {
        log::line(&format!("could not open the window: {err}"));
        cx.quit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_portable_windows_install_skips_the_export() {
        assert!(exports_cli(Os::Windows, false));
        assert!(!exports_cli(Os::Windows, true));
        // Velopack calls every macOS and Linux install portable.
        assert!(exports_cli(Os::MacOs, true));
        assert!(exports_cli(Os::Linux, true));
    }
}
