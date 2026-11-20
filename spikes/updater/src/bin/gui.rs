//! Stand-in for the Acta GUI: no console window, owns install hooks and updates.
#![windows_subsystem = "windows"]

use acta_spike::{export_cli, log, remove_cli, set_on_path, DEFAULT_FEED, VERSION};
use std::time::{Duration, Instant};
use velopack::{sources::HttpSource, UpdateCheck, UpdateManager, VelopackApp};

fn main() {
    // Must run first: handles --veloapp-* hook invocations and exits.
    VelopackApp::build()
        .on_after_install_fast_callback(|v| {
            log("gui-hook", &format!("after-install {v}: {:?} path={:?}", export_cli(), set_on_path(true)));
        })
        .on_after_update_fast_callback(|v| {
            log("gui-hook", &format!("after-update {v}: {:?}", export_cli()));
        })
        .on_before_uninstall_fast_callback(|v| {
            log("gui-hook", &format!("before-uninstall {v}: path={:?} cli={:?}", set_on_path(false), remove_cli()));
        })
        .on_first_run(|v| log("gui", &format!("first run of {v}")))
        .on_restarted(|v| log("gui", &format!("restarted by updater as {v}")))
        .run();

    log("gui", &format!("started from {:?}", std::env::current_exe().ok()));
    // Idempotent: also repairs the export if a hook was missed.
    log("gui", &format!("export: {:?}", export_cli()));

    if std::env::args().any(|a| a == "--print") {
        println!("gui stdout v{VERSION}");
    }

    let feed = std::env::var("ACTA_SPIKE_FEED").unwrap_or_else(|_| DEFAULT_FEED.to_string());
    let t = Instant::now();
    match update(&feed) {
        Ok(msg) => log("gui", &format!("update: {msg} ({} ms)", t.elapsed().as_millis())),
        Err(e) => log("gui", &format!("update failed, continuing: {e} ({} ms)", t.elapsed().as_millis())),
    }

    // Pretend to be a running app for a while.
    let linger: u64 = std::env::var("ACTA_SPIKE_LINGER").ok().and_then(|s| s.parse().ok()).unwrap_or(15);
    std::thread::sleep(Duration::from_secs(linger));
    log("gui", "exiting normally");
}

fn update(feed: &str) -> Result<String, velopack::Error> {
    let um = UpdateManager::new(HttpSource::new(feed), None, None)?;
    match um.check_for_updates()? {
        UpdateCheck::UpdateAvailable(info) => {
            log("gui", &format!("update available: {} ({})", info.TargetFullRelease.Version, info.TargetFullRelease.FileName));
            um.download_updates(&info, None)?;
            log("gui", "downloaded, applying and restarting");
            um.apply_updates_and_restart(&*info)?; // exits the process
            Ok("unreachable".into())
        }
        UpdateCheck::NoUpdateAvailable => Ok("up to date".into()),
        UpdateCheck::RemoteIsEmpty => Ok("remote feed is empty".into()),
    }
}
