//! Update at launch (ADR 0004). Never prompts, never runs on the UI thread.
//!
//! A worker thread checks the feed and downloads. The main thread waits for
//! it, before any window exists, for a bounded time:
//! - nothing new, or the check failed: open the window;
//! - an update arrived within the budgets: apply it and relaunch, no window;
//! - too slow: open the window; the worker keeps downloading and Velopack
//!   applies the package at the next launch (`VelopackApp::run`).
//!
//! [`Gate`] settles the race between "the wait timed out" and "the download
//! just finished", so the app can never be restarted under a visible window.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use velopack::{UpdateCheck, UpdateManager};

use crate::log;

/// How long the launch waits for the feed to answer (ADR 0004).
pub const CHECK_BUDGET: Duration = Duration::from_secs(2);
/// How long the launch then waits for the download. The ADR only says "slow";
/// this is long enough for a delta on an ordinary connection.
pub const DOWNLOAD_BUDGET: Duration = Duration::from_secs(5);

const UNDECIDED: u8 = 0;
const WINDOW: u8 = 1;
const APPLY: u8 = 2;

/// One-shot decision shared by the launch (wants to open the window) and the
/// worker (wants to apply and restart). Whoever claims first wins.
#[derive(Default)]
pub struct Gate(AtomicU8);

impl Gate {
    fn claim(&self, side: u8) -> bool {
        self.0
            .compare_exchange(UNDECIDED, side, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// The launch stops waiting. False if the worker is already applying.
    pub fn claim_window(&self) -> bool {
        self.claim(WINDOW)
    }

    /// The worker may restart the app. False once the window is opening.
    pub fn claim_apply(&self) -> bool {
        self.claim(APPLY)
    }
}

/// Progress reports from the worker to the waiting launch.
#[derive(Debug, PartialEq, Eq)]
pub enum Progress {
    /// The feed answered and has a newer version; the download started.
    Downloading,
    /// Nothing will be applied during this launch (up to date, or failed).
    Finished,
}

/// Wait for the worker within the budgets. Returns when the window should
/// open; if the worker applies the update the process exits instead.
pub fn wait_for_launch_decision(
    progress: &Receiver<Progress>,
    gate: &Gate,
    check_budget: Duration,
    download_budget: Duration,
) {
    let mut deadline = Instant::now() + check_budget;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match progress.recv_timeout(remaining) {
            Ok(Progress::Downloading) => deadline = Instant::now() + download_budget,
            Ok(Progress::Finished) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                if gate.claim_window() {
                    log::line("update: budget spent, opening; it continues in the background");
                    return;
                }
                // The worker claimed first and is applying: the process is
                // about to exit. Only a failed apply gets past this line.
                let _ = progress.recv();
                return;
            }
        }
    }
}

/// What went wrong, for the log. Velopack's messages embed file paths (which
/// contain the user name), so only the kind of error is kept.
pub fn error_kind(err: &velopack::Error) -> String {
    use velopack::Error;
    match err {
        Error::Io(io) => format!("io error ({:?})", io.kind()),
        Error::FileNotFound(_) => "file not found".into(),
        Error::ChecksumInvalid(..) => "checksum mismatch".into(),
        Error::SizeInvalid(..) => "size mismatch".into(),
        Error::Zip(_) => "archive error".into(),
        Error::Network(_) => "network error".into(),
        Error::Json(_) => "malformed feed".into(),
        Error::Semver(_) => "malformed version".into(),
        Error::InvalidPackage(_) => "invalid package".into(),
        Error::NotInstalled(_) => "not installed".into(),
        Error::NotSupported(_) => "not supported".into(),
        _ => "other error".into(),
    }
}

/// Check, download and (if still allowed) apply. Runs on the worker thread.
fn work(manager: &UpdateManager, gate: &Gate, progress: &Sender<Progress>) -> Result<(), String> {
    let started = Instant::now();
    let check = manager
        .check_for_updates()
        .map_err(|err| format!("check failed: {}", error_kind(&err)))?;
    let info = match check {
        UpdateCheck::UpdateAvailable(info) => info,
        UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => {
            log::line(&format!(
                "update: up to date ({} ms)",
                started.elapsed().as_millis()
            ));
            return Ok(());
        }
    };
    let version = info.TargetFullRelease.Version.clone();
    log::line(&format!("update: {version} available, downloading"));
    let _ = progress.send(Progress::Downloading);

    manager
        .download_updates(&info, None)
        .map_err(|err| format!("download failed: {}", error_kind(&err)))?;
    log::line(&format!(
        "update: {version} downloaded ({} ms)",
        started.elapsed().as_millis()
    ));

    if !gate.claim_apply() {
        log::line("update: window already open, it will be applied at next launch");
        return Ok(());
    }
    // Exits the process on success.
    manager
        .apply_updates_and_restart(&*info)
        .map_err(|err| format!("apply failed: {}", error_kind(&err)))
}

/// Start the worker and wait for the launch decision.
pub fn run_at_launch(manager: UpdateManager) {
    let gate = Arc::new(Gate::default());
    let (sender, receiver) = mpsc::channel();

    let worker_gate = Arc::clone(&gate);
    let spawned = thread::Builder::new()
        .name("acta-update".into())
        .spawn(move || {
            if let Err(err) = work(&manager, &worker_gate, &sender) {
                log::line(&format!("update: {err}; retrying at next launch"));
            }
            let _ = sender.send(Progress::Finished);
        });
    if let Err(err) = spawned {
        log::line(&format!("update: could not start the worker: {err}"));
        return;
    }
    wait_for_launch_decision(&receiver, &gate, CHECK_BUDGET, DOWNLOAD_BUDGET);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORT: Duration = Duration::from_millis(40);
    const LONG: Duration = Duration::from_secs(5);

    #[test]
    fn logged_error_kinds_carry_no_path() {
        let path = std::path::PathBuf::from("Users")
            .join("Alexandre")
            .join("x.nupkg");
        let err = velopack::Error::FileNotFound(path);
        assert_eq!(error_kind(&err), "file not found");
        let err = velopack::Error::NotInstalled("C:/Users/Alexandre/Update.exe".into());
        assert!(!error_kind(&err).contains("Alexandre"));
    }

    #[test]
    fn gate_is_won_once() {
        let gate = Gate::default();
        assert!(gate.claim_window());
        assert!(!gate.claim_apply());
        assert!(!gate.claim_window());

        let gate = Gate::default();
        assert!(gate.claim_apply());
        assert!(!gate.claim_window());
    }

    #[test]
    fn up_to_date_opens_immediately() {
        let (sender, receiver) = mpsc::channel();
        let gate = Gate::default();
        sender.send(Progress::Finished).unwrap();

        let started = Instant::now();
        wait_for_launch_decision(&receiver, &gate, LONG, LONG);

        assert!(started.elapsed() < Duration::from_secs(1));
        // Nothing was claimed: a late download could not exist anyway.
        assert!(gate.claim_window());
    }

    #[test]
    fn worker_that_died_opens_immediately() {
        let (sender, receiver) = mpsc::channel::<Progress>();
        drop(sender);
        let started = Instant::now();
        wait_for_launch_decision(&receiver, &Gate::default(), LONG, LONG);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn silent_feed_times_out_and_forbids_a_later_apply() {
        let (_sender, receiver) = mpsc::channel::<Progress>();
        let gate = Gate::default();

        let started = Instant::now();
        wait_for_launch_decision(&receiver, &gate, SHORT, LONG);

        assert!(started.elapsed() >= SHORT);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!gate.claim_apply(), "the window is open; no restart");
    }

    #[test]
    fn download_gets_its_own_budget_after_the_check() {
        let (sender, receiver) = mpsc::channel();
        let gate = Gate::default();
        sender.send(Progress::Downloading).unwrap();

        let started = Instant::now();
        wait_for_launch_decision(&receiver, &gate, LONG, SHORT);

        assert!(started.elapsed() >= SHORT);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!gate.claim_apply());
    }

    #[test]
    fn launch_waits_while_the_worker_applies() {
        let (sender, receiver) = mpsc::channel();
        let gate = Arc::new(Gate::default());
        assert!(gate.claim_apply());

        // Stands in for an apply that fails after a while; a successful one
        // exits the process and never reports.
        let worker = thread::spawn(move || {
            thread::sleep(SHORT * 3);
            sender.send(Progress::Finished).unwrap();
        });

        let started = Instant::now();
        wait_for_launch_decision(&receiver, &gate, SHORT, SHORT);

        assert!(started.elapsed() >= SHORT * 3);
        worker.join().unwrap();
    }
}
