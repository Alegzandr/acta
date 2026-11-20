//! Acta, the GUI executable. Velopack's main exe: the only process that
//! installs, updates or uninstalls anything (ADR 0004).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli_export;
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
    let installed = match UpdateManager::new(
        GithubSource::new(acta_core::REPO_URL, None, false),
        None,
        None,
    ) {
        Ok(manager) => Some(manager),
        Err(err) => {
            log::line(&format!(
                "not a Velopack install ({}); no export, no updates",
                update::error_kind(&err)
            ));
            None
        }
    };

    // Development aid: `Acta --specimen`, or `ACTA_SPECIMEN=1`.
    let specimen = std::env::args().skip(1).any(|arg| arg == "--specimen")
        || std::env::var_os("ACTA_SPECIMEN").is_some_and(|value| value == "1");

    if let Some(manager) = installed {
        // Repairs the export if a hook was missed; cheap when up to date.
        let spawned = std::thread::Builder::new()
            .name("acta-cli-export".into())
            .spawn(|| export_cli("start"));
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

/// Must run before anything else: Velopack starts this executable with hook
/// arguments at install, update and uninstall, and expects it to exit fast.
/// It also applies an update that a previous run downloaded.
fn velopack_bootstrap() {
    let app = VelopackApp::build();
    // Velopack only has these hooks on Windows. Elsewhere the export is done
    // by the check at every start, and there is no uninstall step to hook.
    #[cfg(windows)]
    let app = app
        .on_after_install_fast_callback(|_| export_cli("after-install"))
        .on_after_update_fast_callback(|_| export_cli("after-update"))
        .on_before_uninstall_fast_callback(|_| match Layout::locate() {
            Some(layout) => uninstall::run(&layout),
            None => log::line("uninstall: user directories not found; nothing removed"),
        });
    let mut app = app;
    app.run();
}

/// Export the CLI and put it on the PATH. Every failure is logged and
/// swallowed: none of this may keep the window from opening.
fn export_cli(when: &str) {
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
            log::line(&format!("export ({when}): failed: {err}"));
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
        Err(err) => log::line(&format!("export ({when}): PATH not updated: {err}")),
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
