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
//!
//! The wait runs off the UI thread while GPUI is already up: when the main
//! window is not ready [`UPDATER_WINDOW_DELAY`] after launch, the updater
//! window (docs/design/components-app.md 5 and 8.16) shows [`LaunchState`] until the decision,
//! and closes in the frame the main window opens.
//!
//! The worker never applies itself: Velopack's apply ends the process with
//! `exit(0)`, which must not happen on a worker thread while GPUI draws.
//! The worker posts the apply to [`LaunchState`]; the UI thread closes its
//! windows, writes the preferences and runs it.
//!
//! A launch that Velopack's updater itself started (`VELOPACK_RESTART`) never
//! applies. The updater restarts the app after a successful apply and also
//! after a failed one, in which case it starts the old version again with the
//! package still on disk (`apply.rs` in Velopack 1.2.161). Applying again in
//! that launch would loop forever without a window; this way a failed update
//! is retried once per launch by the user, never by the app on its own.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use velopack::{UpdateCheck, UpdateInfo, UpdateManager};

use crate::log;

/// How long the launch waits for the feed to answer (ADR 0004).
pub const CHECK_BUDGET: Duration = Duration::from_secs(2);
/// How long the launch then waits for the download. The ADR only says "slow";
/// this is long enough for a delta on an ordinary connection.
pub const DOWNLOAD_BUDGET: Duration = Duration::from_secs(5);
/// How long the launch waits, once the worker has claimed the apply, for it
/// to post the apply and finish. Nothing slow happens in between (the apply
/// itself runs on the UI thread), so this only bounds a worker that died or
/// hung there; past it the launch is ready and the UI decides.
pub const APPLY_BUDGET: Duration = Duration::from_secs(1);

/// The updater window appears when the main window is not ready this long
/// after launch (docs/design/interaction.md 6.3).
pub const UPDATER_WINDOW_DELAY: Duration = Duration::from_millis(300);

/// What the updater window says (docs/design/components-app.md 5, Updater window).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// "Checking for updates...", no bar.
    Checking,
    /// "Downloading update...", the bar at this percentage.
    Downloading(u8),
    /// "Restarting...", the bar full.
    Restarting,
}

/// Applies a downloaded update and restarts; returns only on failure.
pub type Apply = Box<dyn FnOnce() -> Result<(), String> + Send>;

/// Shared between the update worker, the launch wait and the UI.
pub struct LaunchState {
    status: Mutex<Status>,
    ready: AtomicBool,
    apply: Mutex<Option<Apply>>,
}

impl Default for LaunchState {
    fn default() -> Self {
        Self {
            status: Mutex::new(Status::Checking),
            ready: AtomicBool::new(false),
            apply: Mutex::new(None),
        }
    }
}

/// Marks the launch ready when dropped, so the UI never waits forever on a
/// launch thread that panicked.
pub struct ReadyOnDrop(pub Arc<LaunchState>);

impl Drop for ReadyOnDrop {
    fn drop(&mut self) {
        self.0.set_ready();
    }
}

impl LaunchState {
    pub fn status(&self) -> Status {
        *self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn set_status(&self, status: Status) {
        *self
            .status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = status;
    }

    /// The main window may open.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    /// The worker hands the apply to the UI thread. Posted before the
    /// worker reports, so before the launch is ready.
    fn post_apply(&self, apply: Apply) {
        *self
            .apply
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(apply);
    }

    /// The apply to run on the UI thread, once.
    pub fn take_apply(&self) -> Option<Apply> {
        self.apply
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }
}

/// What the UI does at a tick of the launch, while it waits for the update
/// decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchStep {
    Wait,
    /// Close every window, write the preferences, apply and restart.
    Apply,
    ShowUpdaterWindow,
    /// Open the main window, and close the updater window if it is shown,
    /// in the same frame.
    OpenMainWindow,
}

/// `apply_posted` wins: the worker only posts an apply it claimed, which
/// means no main window may open in this launch.
pub fn launch_step(
    since_launch: Duration,
    ready: bool,
    apply_posted: bool,
    updater_shown: bool,
) -> LaunchStep {
    if apply_posted {
        LaunchStep::Apply
    } else if ready {
        LaunchStep::OpenMainWindow
    } else if !updater_shown && since_launch >= UPDATER_WINDOW_DELAY {
        LaunchStep::ShowUpdaterWindow
    } else {
        LaunchStep::Wait
    }
}

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

/// Wait for the worker within the budgets. Returns when the launch is
/// decided: either the window may open, or the worker has posted an apply
/// for the UI thread (see [`LaunchState::take_apply`]).
pub fn wait_for_launch_decision(
    progress: &Receiver<Progress>,
    gate: &Gate,
    check_budget: Duration,
    download_budget: Duration,
    apply_budget: Duration,
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
                // The worker claimed first: it is posting the apply for the
                // UI thread and finishing, which takes moments. Only a worker
                // that died or hung there gets past this bound.
                if progress.recv_timeout(apply_budget) == Err(RecvTimeoutError::Timeout) {
                    log::line("update: the worker did not hand over the apply in time");
                }
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
        Error::Network(net) => format!("network error ({})", network_kind(net)),
        Error::Json(_) => "malformed feed".into(),
        Error::Semver(_) => "malformed version".into(),
        Error::InvalidPackage(_) => "invalid package".into(),
        Error::NotInstalled(_) => "not installed".into(),
        Error::NotSupported(_) => "not supported".into(),
        _ => "other error".into(),
    }
}

/// The HTTP status or the transport failure behind a network error: enough to
/// tell a rate limit (403, 429) from a dead network, never a URL.
fn network_kind(err: &velopack::NetworkError) -> String {
    use ureq::Error;
    match err {
        velopack::NetworkError::Url(_) => "bad url".into(),
        velopack::NetworkError::Http(http) => match http {
            Error::StatusCode(code) => format!("HTTP {code}"),
            Error::Timeout(_) => "timeout".into(),
            Error::HostNotFound => "host not found".into(),
            Error::ConnectionFailed => "connection failed".into(),
            Error::Io(io) => format!("io {:?}", io.kind()),
            Error::Tls(_) | Error::Pem(_) | Error::Rustls(_) => "tls".into(),
            Error::RedirectFailed | Error::TooManyRedirects => "redirect".into(),
            _ => "other".into(),
        },
    }
}

/// What the worker needs from Velopack. A trait only so that the decisions
/// in [`work`] can be tested without an installed app and a feed.
pub trait Feed {
    type Update;
    /// The newer version and its handle, or `None` when up to date.
    fn check(&self) -> Result<Option<(String, Self::Update)>, String>;
    /// `progress` receives the percentage as data arrives.
    fn download(
        &self,
        update: &Self::Update,
        progress: Box<dyn Fn(u8) + Send>,
    ) -> Result<(), String>;
    /// Exits the process on success.
    fn apply(&self, update: &Self::Update) -> Result<(), String>;
}

impl Feed for UpdateManager {
    type Update = Box<UpdateInfo>;

    fn check(&self) -> Result<Option<(String, Self::Update)>, String> {
        match self.check_for_updates() {
            Ok(UpdateCheck::UpdateAvailable(info)) => {
                Ok(Some((info.TargetFullRelease.Version.clone(), info)))
            }
            Ok(UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty) => Ok(None),
            Err(err) => Err(format!("check failed: {}", error_kind(&err))),
        }
    }

    fn download(
        &self,
        update: &Self::Update,
        progress: Box<dyn Fn(u8) + Send>,
    ) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel::<i16>();
        // Velopack reports on the channel while it downloads; the channel
        // closes when it returns, which ends this thread.
        let forward = thread::Builder::new()
            .name("acta-update-progress".into())
            .spawn(move || {
                for percent in receiver {
                    progress(percent.clamp(0, 100) as u8);
                }
            });
        let result = self
            .download_updates(update, Some(sender))
            .map_err(|err| format!("download failed: {}", error_kind(&err)));
        if let Ok(forward) = forward {
            let _ = forward.join();
        }
        result
    }

    fn apply(&self, update: &Self::Update) -> Result<(), String> {
        self.apply_updates_and_restart(&**update)
            .map_err(|err| format!("apply failed: {}", error_kind(&err)))
    }
}

/// Check, download and, if this launch may still apply, return the update
/// to apply. Runs on the worker thread.
fn work<F: Feed>(
    feed: &F,
    gate: &Gate,
    progress: &Sender<Progress>,
    state: &Arc<LaunchState>,
) -> Result<Option<F::Update>, String> {
    let started = Instant::now();
    let Some((version, update)) = feed.check()? else {
        log::line(&format!(
            "update: up to date ({} ms)",
            started.elapsed().as_millis()
        ));
        return Ok(None);
    };
    log::line(&format!("update: {version} available, downloading"));
    let _ = progress.send(Progress::Downloading);
    state.set_status(Status::Downloading(0));

    let reporting = Arc::clone(state);
    feed.download(
        &update,
        Box::new(move |percent| reporting.set_status(Status::Downloading(percent))),
    )?;
    log::line(&format!(
        "update: {version} downloaded ({} ms)",
        started.elapsed().as_millis()
    ));

    if !gate.claim_apply() {
        log::line("update: not applying in this launch; Velopack applies it at the next one");
        return Ok(None);
    }
    state.set_status(Status::Restarting);
    Ok(Some(update))
}

/// The gate a launch starts with. A launch started by the updater has already
/// decided: the window opens, nothing is applied (see the module comment).
fn launch_gate(restarted_by_updater: bool) -> Gate {
    let gate = Gate::default();
    if restarted_by_updater {
        gate.claim_window();
    }
    gate
}

/// Start the worker and, unless the updater started this launch, wait for the
/// launch decision. Blocks; run it off the UI thread.
pub fn run_at_launch(manager: UpdateManager, restarted_by_updater: bool, state: Arc<LaunchState>) {
    let gate = Arc::new(launch_gate(restarted_by_updater));
    let (sender, receiver) = mpsc::channel();

    let worker_gate = Arc::clone(&gate);
    let spawned = thread::Builder::new()
        .name("acta-update".into())
        .spawn(move || {
            match work(&manager, &worker_gate, &sender, &state) {
                Ok(Some(update)) => state.post_apply(Box::new(move || manager.apply(&update))),
                Ok(None) => {}
                Err(err) => log::line(&format!("update: {err}; retrying at next launch")),
            }
            let _ = sender.send(Progress::Finished);
        });
    if let Err(err) = spawned {
        log::line(&format!("update: could not start the worker: {err}"));
        return;
    }
    if restarted_by_updater {
        log::line("update: started by the updater, opening without applying");
        return;
    }
    wait_for_launch_decision(
        &receiver,
        &gate,
        CHECK_BUDGET,
        DOWNLOAD_BUDGET,
        APPLY_BUDGET,
    );
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

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
    fn network_errors_name_the_status_or_the_transport() {
        let net =
            |e: ureq::Error| velopack::Error::Network(Box::new(velopack::NetworkError::Http(e)));
        assert_eq!(
            error_kind(&net(ureq::Error::StatusCode(403))),
            "network error (HTTP 403)"
        );
        assert_eq!(
            error_kind(&net(ureq::Error::HostNotFound)),
            "network error (host not found)"
        );
        assert_eq!(
            error_kind(&net(ureq::Error::Tls("handshake"))),
            "network error (tls)"
        );
        let io = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "/home/Alexandre");
        assert_eq!(
            error_kind(&net(ureq::Error::Io(io))),
            "network error (io ConnectionReset)"
        );
        let uri = ureq::Error::BadUri("https://example.com/Alexandre".into());
        assert_eq!(error_kind(&net(uri)), "network error (other)");
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
        wait_for_launch_decision(&receiver, &gate, LONG, LONG, LONG);

        assert!(started.elapsed() < Duration::from_secs(1));
        // Nothing was claimed: a late download could not exist anyway.
        assert!(gate.claim_window());
    }

    #[test]
    fn worker_that_died_opens_immediately() {
        let (sender, receiver) = mpsc::channel::<Progress>();
        drop(sender);
        let started = Instant::now();
        wait_for_launch_decision(&receiver, &Gate::default(), LONG, LONG, LONG);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn silent_feed_times_out_and_forbids_a_later_apply() {
        let (_sender, receiver) = mpsc::channel::<Progress>();
        let gate = Gate::default();

        let started = Instant::now();
        wait_for_launch_decision(&receiver, &gate, SHORT, LONG, LONG);

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
        wait_for_launch_decision(&receiver, &gate, LONG, SHORT, LONG);

        assert!(started.elapsed() >= SHORT);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!gate.claim_apply());
    }

    #[test]
    fn launch_waits_while_the_worker_hands_over_the_apply() {
        let (sender, receiver) = mpsc::channel();
        let gate = Arc::new(Gate::default());
        assert!(gate.claim_apply());

        // Stands in for a worker that posts its apply, then finishes.
        let worker = thread::spawn(move || {
            thread::sleep(SHORT * 3);
            sender.send(Progress::Finished).unwrap();
        });

        let started = Instant::now();
        wait_for_launch_decision(&receiver, &gate, SHORT, SHORT, LONG);

        assert!(started.elapsed() >= SHORT * 3);
        worker.join().unwrap();
    }

    #[test]
    fn launch_stops_waiting_for_an_apply_that_never_finishes() {
        let (_sender, receiver) = mpsc::channel::<Progress>();
        let gate = Gate::default();
        assert!(gate.claim_apply());

        let started = Instant::now();
        wait_for_launch_decision(&receiver, &gate, SHORT, SHORT, SHORT * 2);

        assert!(started.elapsed() >= SHORT * 3);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    /// A feed with one newer version, counting what the worker asks of it.
    #[derive(Default)]
    struct FakeFeed {
        up_to_date: bool,
        check_fails: bool,
        download_fails: bool,
        downloads: Cell<u32>,
        applies: Cell<u32>,
    }

    impl Feed for FakeFeed {
        type Update = ();

        fn check(&self) -> Result<Option<(String, ())>, String> {
            if self.check_fails {
                Err("check failed: network error".into())
            } else if self.up_to_date {
                Ok(None)
            } else {
                Ok(Some(("0.1.1".into(), ())))
            }
        }

        fn download(&self, _: &(), progress: Box<dyn Fn(u8) + Send>) -> Result<(), String> {
            self.downloads.set(self.downloads.get() + 1);
            progress(42);
            if self.download_fails {
                Err("download failed: network error".into())
            } else {
                Ok(())
            }
        }

        fn apply(&self, _: &()) -> Result<(), String> {
            self.applies.set(self.applies.get() + 1);
            // The real one exits the process; a return is a failed apply.
            Err("apply failed: other error".into())
        }
    }

    fn reports(receiver: &Receiver<Progress>) -> Vec<Progress> {
        receiver.try_iter().collect()
    }

    #[test]
    fn worker_downloads_then_applies_on_an_ordinary_launch() {
        let feed = FakeFeed::default();
        let gate = launch_gate(false);
        let (sender, receiver) = mpsc::channel();

        let result = work(&feed, &gate, &sender, &Arc::default());

        // The update comes back to be applied on the UI thread.
        assert_eq!(result, Ok(Some(())));
        assert_eq!((feed.downloads.get(), feed.applies.get()), (1, 0));
        assert_eq!(reports(&receiver), [Progress::Downloading]);
        assert!(!gate.claim_window(), "the apply side owns the gate");
    }

    #[test]
    fn the_launch_state_follows_the_worker() {
        let feed = FakeFeed::default();
        let (sender, _receiver) = mpsc::channel();
        let state = Arc::new(LaunchState::default());
        assert_eq!(state.status(), Status::Checking);

        // Claimed by the window: the download is reported, nothing applies.
        let gate = launch_gate(true);
        let _ = work(&feed, &gate, &sender, &state);
        assert_eq!(state.status(), Status::Downloading(42));

        // An ordinary launch: the worker restarts the app after the download.
        let _ = work(&feed, &launch_gate(false), &sender, &state);
        assert_eq!(state.status(), Status::Restarting);
        assert!(!state.is_ready());
        state.set_ready();
        assert!(state.is_ready());
    }

    /// `launch_step` with nothing posted.
    fn launch_step4(since: Duration, ready: bool, shown: bool) -> LaunchStep {
        launch_step(since, ready, false, shown)
    }

    #[test]
    fn a_posted_apply_comes_first_and_runs_once() {
        let ms = Duration::from_millis;
        for (since, ready, shown) in [(0, false, false), (900, true, true), (900, false, true)] {
            assert_eq!(
                launch_step(ms(since), ready, true, shown),
                LaunchStep::Apply
            );
        }
        let state = LaunchState::default();
        assert!(state.take_apply().is_none());
        state.post_apply(Box::new(|| Err("apply failed: other error".into())));
        let apply = state.take_apply().expect("posted");
        assert!(state.take_apply().is_none(), "taken once");
        assert!(apply().is_err());
    }

    #[test]
    fn the_launch_is_ready_even_if_its_thread_panics() {
        let state = Arc::new(LaunchState::default());
        let guarded = Arc::clone(&state);
        let joined = thread::spawn(move || {
            let _ready = ReadyOnDrop(guarded);
            panic!("the launch wait failed");
        })
        .join();
        assert!(joined.is_err());
        assert!(state.is_ready());
    }

    #[test]
    fn the_updater_window_shows_only_when_the_main_window_is_late() {
        let ms = Duration::from_millis;
        assert_eq!(launch_step4(ms(0), false, false), LaunchStep::Wait);
        assert_eq!(launch_step4(ms(299), false, false), LaunchStep::Wait);
        assert_eq!(
            launch_step4(ms(300), false, false),
            LaunchStep::ShowUpdaterWindow
        );
        // Shown once, then the launch waits with it.
        assert_eq!(launch_step4(ms(900), false, true), LaunchStep::Wait);
        // Ready before 300 ms: the main window opens, no updater window ever.
        assert_eq!(
            launch_step4(ms(120), true, false),
            LaunchStep::OpenMainWindow
        );
        // Ready later: the main window opens and the updater window closes.
        assert_eq!(
            launch_step4(ms(4000), true, true),
            LaunchStep::OpenMainWindow
        );
    }

    #[test]
    fn worker_never_applies_in_a_launch_started_by_the_updater() {
        // The loop this prevents: apply fails, the updater restarts the old
        // version, which finds the same package and applies again.
        let feed = FakeFeed::default();
        let gate = launch_gate(true);
        let (sender, receiver) = mpsc::channel();

        let result = work(&feed, &gate, &sender, &Arc::default());

        assert_eq!(result, Ok(None));
        assert_eq!((feed.downloads.get(), feed.applies.get()), (1, 0));
        assert_eq!(reports(&receiver), [Progress::Downloading]);
    }

    #[test]
    fn worker_does_not_apply_once_the_window_is_opening() {
        let feed = FakeFeed::default();
        let gate = launch_gate(false);
        assert!(gate.claim_window());
        let (sender, _receiver) = mpsc::channel();

        assert_eq!(work(&feed, &gate, &sender, &Arc::default()), Ok(None));
        assert_eq!((feed.downloads.get(), feed.applies.get()), (1, 0));
    }

    #[test]
    fn worker_stops_when_up_to_date_or_when_a_step_fails() {
        let (sender, receiver) = mpsc::channel();

        let feed = FakeFeed {
            up_to_date: true,
            ..FakeFeed::default()
        };
        assert_eq!(
            work(&feed, &launch_gate(false), &sender, &Arc::default()),
            Ok(None)
        );
        assert_eq!((feed.downloads.get(), feed.applies.get()), (0, 0));
        assert_eq!(reports(&receiver), []);

        let feed = FakeFeed {
            check_fails: true,
            ..FakeFeed::default()
        };
        assert!(work(&feed, &launch_gate(false), &sender, &Arc::default()).is_err());
        assert_eq!((feed.downloads.get(), feed.applies.get()), (0, 0));

        let feed = FakeFeed {
            download_fails: true,
            ..FakeFeed::default()
        };
        assert!(work(&feed, &launch_gate(false), &sender, &Arc::default()).is_err());
        assert_eq!((feed.downloads.get(), feed.applies.get()), (1, 0));
    }
}
