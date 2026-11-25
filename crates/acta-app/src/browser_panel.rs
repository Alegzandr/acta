//! One agent browser (ADR 0010; docs/design/components-app.md 5, Agent
//! browser; layout.md 4.5.1): a WebView2 on the shared agent profile,
//! driven over the authenticated pipe by `acta mcp browser`. A page can
//! have several; the Agent browser view (browser_dock.rs) shows the
//! current page's. Windows only; elsewhere the MCP tools point to Claude
//! in Chrome.
//!
//! The pipe threads run the operations of `acta-browser` against
//! [`AppHost`], which hands every DevTools call to this browser on the UI
//! thread through a job channel. The web view is created on the first
//! call (or when the human opens an address), lives on while it is not in
//! view, and is "parked" (visible, outside the window's client area) then,
//! so Chromium keeps rendering screenshots (ADR 0010, decision 8). The
//! shell closes it after 10 minutes off screen and unused, keeping its
//! address; its profile stays.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::zoom::px;
use acta_browser::events::{Events, Notice};
use acta_browser::ops::{Activity, Handoff, Host, STOPPED};
use acta_browser::policy::{Policy, origin_of};
use async_channel::{Receiver, Sender};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, anchored, deferred, div, point,
};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use wry::WebViewExtWindows as _;

use crate::browser_host::{self, DebugPort};
use crate::chat::Kept;
use crate::i18n::{Arg, Key, text, tr};
use crate::icons::Icon;
use crate::layout::{Place, Side, ViewAction};
use crate::menu;
use crate::settings;
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui::{self, Variant};

/// A DevTools call that has not answered by then has failed.
const CDP_WAIT: Duration = Duration::from_secs(10);
/// The element acted on stays highlighted this long.
const HIGHLIGHT: Duration = Duration::from_millis(300);
/// The size a web view takes when it was never on screen.
const PARKED_SIZE: (f64, f64) = (1024., 700.);
/// The web view behind a page that paints no background: white, as in
/// any browser (RGBA).
const PAGE_BACKGROUND: (u8, u8, u8, u8) = (255, 255, 255, 255);

pub enum BrowserEvent {
    /// An item of the view menu in the browser's own menu.
    View(ViewAction),
    /// "Clear browser data..." from the menu: the shell asks first.
    Clear,
    /// The profile could not be deleted.
    ClearFailed,
    /// A key Acta keeps, pressed while the page had the keyboard.
    Kept(Kept),
}

/// Why the pipe is not served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipeFailure {
    /// Another process (a second Acta, or a squatter) holds the name.
    Taken,
    Other,
}

/// What the pipe threads and the web view's handlers ask of the view.
enum Job {
    Ensure(mpsc::Sender<Result<bool, String>>),
    Cdp {
        method: String,
        params: String,
        reply: mpsc::Sender<Result<String, String>>,
    },
    Activity(Activity),
    Highlight(i64),
    Handoff(String, mpsc::Sender<Handoff>),
    HandoffEnd,
    Driver(Option<String>, bool),
    Busy(bool),
    /// A pop-up or `target=_blank` link the policy allows: opened here.
    OpenHere(String),
    /// The page finished loading this URL (for the address field).
    Loaded(String),
    /// The page took the keyboard, or a kept key was pressed in it.
    Focused,
    Key(Kept),
}

/// What the view and the pipe threads share.
struct Shared {
    events: Arc<Events>,
    stopped: AtomicBool,
    /// The current view had its DevTools setup (ops::setup); cleared when a
    /// view is created or closed, so a view the human opened, or a setup
    /// that failed, is set up on the next call.
    set_up: AtomicBool,
    policy: Mutex<Policy>,
}

/// The operations' view of the app (`acta_browser::ops::Host`).
pub struct AppHost {
    jobs: Sender<Job>,
    shared: Arc<Shared>,
}

impl AppHost {
    fn post(&self, job: Job) -> bool {
        self.jobs.send_blocking(job).is_ok()
    }

    /// Waits for `rx`, giving up after `limit` or, when `stoppable`, as soon
    /// as the human presses Stop.
    fn wait<T>(&self, rx: &mpsc::Receiver<T>, limit: Duration, stoppable: bool) -> Option<T> {
        let end = Instant::now() + limit;
        loop {
            let now = Instant::now();
            if now >= end || (stoppable && self.stopped()) {
                return None;
            }
            match rx.recv_timeout((end - now).min(Duration::from_millis(100))) {
                Ok(v) => return Some(v),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

const CLOSED: &str = "Acta closed; call again to reopen it, or ask the user";

impl Host for AppHost {
    fn ensure_view(&self) -> Result<bool, String> {
        let (tx, rx) = mpsc::channel();
        if !self.post(Job::Ensure(tx)) {
            return Err(CLOSED.into());
        }
        self.wait(&rx, Duration::from_secs(30), false)
            .unwrap_or_else(|| Err("the agent browser did not start within 30 s".into()))
    }

    fn set_up(&self) {
        self.shared.set_up.store(true, Ordering::SeqCst);
    }

    fn cdp(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        let (tx, rx) = mpsc::channel();
        let job = Job::Cdp {
            method: method.into(),
            params: params.to_string(),
            reply: tx,
        };
        if !self.post(job) {
            return Err(CLOSED.into());
        }
        match self.wait(&rx, CDP_WAIT, true) {
            Some(Ok(json)) => serde_json::from_str(&json).map_err(|e| format!("{method}: {e}")),
            Some(Err(e)) => Err(e),
            None if self.stopped() => Err(STOPPED.into()),
            None => Err(format!("{method}: no answer in 10 s")),
        }
    }

    fn events(&self) -> &Events {
        &self.shared.events
    }

    fn policy(&self) -> Policy {
        self.shared
            .policy
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn activity(&self, what: Activity) {
        self.post(Job::Activity(what));
    }

    fn highlight(&self, backend: i64) {
        self.post(Job::Highlight(backend));
    }

    fn handoff(&self, message: &str, timeout: Duration) -> Handoff {
        let (tx, rx) = mpsc::channel();
        if !self.post(Job::Handoff(message.into(), tx)) {
            return Handoff::Stopped;
        }
        let outcome = self.wait(&rx, timeout, true);
        if outcome.is_none() {
            self.post(Job::HandoffEnd);
        }
        match outcome {
            Some(o) => o,
            None if self.stopped() => Handoff::Stopped,
            None => Handoff::Timeout,
        }
    }

    fn stopped(&self) -> bool {
        self.shared.stopped.load(Ordering::SeqCst)
    }
}

impl AppHost {
    /// Who drives this browser, as the header shows it.
    pub fn driver(&self, session: Option<&str>, connected: bool) {
        self.post(Job::Driver(session.map(str::to_string), connected));
    }

    /// A call is running.
    pub fn busy(&self, busy: bool) {
        self.post(Job::Busy(busy));
    }
}

/// Where the view stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// No web view; the view explains, the address field can start one.
    Idle,
    Starting,
    Live,
    RuntimeMissing,
    Refused(DebugPort),
    Failed,
}

/// What waits for the web view to be created.
type AfterBuild = Box<
    dyn FnOnce(&mut BrowserPanel, Result<bool, String>, &mut Window, &mut Context<BrowserPanel>),
>;

/// The live web view and what keeps it alive. Fields drop in order.
struct Web {
    _hooks: Option<browser_host::Hooks>,
    _keys: Option<crate::chat_windows::Hooks>,
    webview: ICoreWebView2,
    view: Entity<gpui_webview::WebView>,
    _context: Box<wry::WebContext>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tip {
    Stop,
    Menu,
}

pub struct BrowserPanel {
    phase: Phase,
    focus: FocusHandle,
    /// On screen (visible in the layout, not folded away).
    shown: bool,
    overlay: bool,
    rearranging: bool,
    place: Place,
    can_split: bool,
    menu: bool,
    menu_anchor: crate::menu::AnchorBounds,
    tip: Option<Tip>,
    web: Option<Web>,
    shared: Arc<Shared>,
    jobs: Sender<Job>,
    /// Kept keys and focus from the page (chat_windows.rs reports them as
    /// the chat's signals).
    signals: Sender<crate::chat_panel::Signal>,
    activity: Option<Activity>,
    driver: Option<String>,
    connected: bool,
    busy: bool,
    handoff: Option<(String, mpsc::Sender<Handoff>)>,
    pipe: Option<PipeFailure>,
    address: Entity<InputState>,
    /// The last page the agent or the human loaded.
    url: String,
    clearing: bool,
    last_used: Instant,
    host: Arc<AppHost>,
    _jobs_task: Task<()>,
    _signals_task: Task<()>,
    /// The web view being created (crate::webview_build), and what waits
    /// for it.
    web_task: Task<()>,
    waiting: Vec<AfterBuild>,
    /// Bumped by every build and every close: a build that lands for an
    /// older one is discarded.
    build_generation: u64,
    _subs: Vec<Subscription>,
}

impl EventEmitter<BrowserEvent> for BrowserPanel {}

impl Focusable for BrowserPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl BrowserPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (jobs, jobs_rx) = async_channel::unbounded();
        let (signals, signals_rx) = async_channel::unbounded();
        let shared = Arc::new(Shared {
            events: Arc::new(Events::default()),
            stopped: AtomicBool::new(false),
            set_up: AtomicBool::new(false),
            policy: Mutex::new(Policy::default()),
        });
        let locale = settings::locale(cx);
        let address =
            cx.new(|cx| InputState::new(window, cx).placeholder(text(locale, Key::BrowserAddress)));
        let sub = cx.subscribe_in(
            &address,
            window,
            |panel, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    panel.open_address(window, cx);
                }
            },
        );
        let host = Arc::new(AppHost {
            jobs: jobs.clone(),
            shared: shared.clone(),
        });
        Self {
            phase: Phase::Idle,
            focus: cx.focus_handle(),
            shown: false,
            overlay: false,
            rearranging: false,
            place: Place::Side(Side::Right),
            can_split: false,
            menu: false,
            menu_anchor: Default::default(),
            tip: None,
            web: None,
            shared,
            jobs,
            signals,
            activity: None,
            driver: None,
            connected: false,
            busy: false,
            handoff: None,
            pipe: None,
            address,
            url: String::new(),
            clearing: false,
            last_used: Instant::now(),
            host,
            _jobs_task: Self::listen(jobs_rx, window, cx),
            _signals_task: Self::listen_signals(signals_rx, window, cx),
            web_task: Task::ready(()),
            waiting: Vec::new(),
            build_generation: 0,
            _subs: vec![sub],
        }
    }

    /// What the pipe threads drive.
    pub fn host(&self) -> Arc<AppHost> {
        self.host.clone()
    }

    /// The pipe is not served (shown as a notice).
    pub fn set_pipe(&mut self, failure: Option<PipeFailure>, cx: &mut Context<Self>) {
        self.pipe = failure;
        cx.notify();
    }

    /// The profile is being deleted: no web view meanwhile.
    pub fn set_clearing(&mut self, on: bool, cx: &mut Context<Self>) {
        self.clearing = on;
        cx.notify();
    }

    /// Shown again after an idle close: the web view comes back at its
    /// address.
    pub fn reopen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.web.is_some() || self.phase != Phase::Idle || self.url.is_empty() {
            return;
        }
        let url = self.url.clone();
        self.ensure_then(window, cx, move |panel, created, _, cx| {
            if created.is_ok()
                && let Some(web) = &panel.web
            {
                let _ = web.view.read(cx).raw().load_url(&url);
            }
        });
    }

    /// For the 10-minute rule: whether a web view exists, when it was last
    /// used, and whether a call or a handoff holds it.
    pub fn web_use(&self) -> (bool, Instant, bool) {
        (
            self.web.is_some(),
            self.last_used,
            self.busy || self.handoff.is_some() || self.phase == Phase::Starting,
        )
    }

    fn listen(rx: Receiver<Job>, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |panel, cx| {
            while let Ok(job) = rx.recv().await {
                if panel
                    .update_in(cx, |panel, window, cx| panel.on_job(job, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn listen_signals(
        rx: Receiver<crate::chat_panel::Signal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn_in(window, async move |panel, cx| {
            while let Ok(signal) = rx.recv().await {
                let job = match signal {
                    crate::chat_panel::Signal::Key(kept) => Job::Key(kept),
                    crate::chat_panel::Signal::Focused => Job::Focused,
                    _ => continue,
                };
                if panel
                    .update_in(cx, |panel, window, cx| panel.on_job(job, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn on_job(&mut self, job: Job, window: &mut Window, cx: &mut Context<Self>) {
        self.last_used = Instant::now();
        match job {
            Job::Ensure(reply) => self.ensure_then(window, cx, move |panel, created, _, _| {
                let ready = created.map(|_| !panel.shared.set_up.load(Ordering::SeqCst));
                let _ = reply.send(ready);
            }),
            Job::Cdp {
                method,
                params,
                reply,
            } => match &self.web {
                Some(web) => {
                    browser_host::call(&web.webview, &method, &params, move |r| {
                        let _ = reply.send(r);
                    });
                }
                None => {
                    let _ = reply.send(Err("the agent browser is closed".into()));
                }
            },
            Job::Activity(a) => {
                self.activity = Some(a);
                cx.notify();
            }
            Job::Highlight(b) => self.highlight(b, cx),
            Job::Handoff(message, reply) => {
                if let Some((_, old)) = self.handoff.replace((message, reply)) {
                    let _ = old.send(Handoff::Stopped);
                }
                cx.notify();
            }
            Job::HandoffEnd => {
                self.handoff = None;
                cx.notify();
            }
            Job::Driver(session, connected) => {
                self.driver = session;
                self.connected = connected;
                cx.notify();
            }
            Job::Busy(busy) => {
                self.busy = busy;
                cx.notify();
            }
            Job::OpenHere(url) => {
                if let Some(web) = &self.web {
                    let _ = web.view.read(cx).raw().load_url(&url);
                }
            }
            Job::Loaded(url) => {
                self.url = url.clone();
                if !self.address.read(cx).focus_handle(cx).is_focused(window) {
                    let shown = if url == "about:blank" {
                        String::new()
                    } else {
                        url
                    };
                    self.address
                        .update(cx, |a, cx| a.set_value(shown, window, cx));
                }
                cx.notify();
            }
            Job::Focused => window.focus(&self.focus, cx),
            Job::Key(crate::chat::Kept::DevTools) => self.open_devtools(cx),
            Job::Key(kept) => cx.emit(BrowserEvent::Kept(kept)),
        }
    }

    /// The element acted on, outlined for 300 ms (CDP `Overlay`, no script
    /// in the page).
    fn highlight(&mut self, backend: i64, cx: &mut Context<Self>) {
        let Some(web) = &self.web else { return };
        let params = serde_json::json!({
            "backendNodeId": backend,
            "highlightConfig": {
                "borderColor": {"r": 70, "g": 120, "b": 230, "a": 0.9},
                "contentColor": {"r": 70, "g": 120, "b": 230, "a": 0.15},
            },
        });
        browser_host::call(
            &web.webview,
            "Overlay.highlightNode",
            &params.to_string(),
            |_| {},
        );
        cx.spawn(async move |panel, cx| {
            cx.background_executor().timer(HIGHLIGHT).await;
            let _ = panel.update(cx, |panel, _| {
                if let Some(web) = &panel.web {
                    browser_host::call(&web.webview, "Overlay.hideHighlight", "{}", |_| {});
                }
            });
        })
        .detach();
    }

    /// Creates the web view if needed, then `then` with whether it was
    /// created now. The web view is created after this update, outside it
    /// (crate::webview_build); calls meanwhile wait for the same one.
    fn ensure_then(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<bool, String>, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if self.web.is_some() {
            return then(self, Ok(false), window, cx);
        }
        if self.clearing {
            let busy = "the browser's data is being cleared; try again in a moment";
            return then(self, Err(busy.into()), window, cx);
        }
        self.waiting.push(Box::new(then));
        if self.phase == Phase::Starting {
            return;
        }
        self.phase = Phase::Starting;
        if let Err(e) = self.build(window, cx) {
            self.answer_waiting(Err(e), window, cx);
        }
        self.sync(window, cx);
        cx.notify();
    }

    /// Everything waiting for the web view gets `result`.
    fn answer_waiting(
        &mut self,
        result: Result<bool, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for then in std::mem::take(&mut self.waiting) {
            then(self, result.clone(), window, cx);
        }
    }

    /// Checks what can refuse the browser, then starts creating the web
    /// view; [`Self::built`] takes it from there.
    fn build(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let refused = |panel: &mut Self, phase: Phase, line: &str| {
            panel.phase = phase;
            Err(line.to_string())
        };
        let Some(profile) = browser_host::profile_dir() else {
            return refused(
                self,
                Phase::Failed,
                "Acta has no data directory; the browser cannot start",
            );
        };
        if let Some(data_dir) = profile.parent()
            && let Some(why) = browser_host::debug_port(data_dir)
        {
            crate::log::line(&format!(
                "browser: refused, a debugging port is configured ({why:?})"
            ));
            return refused(
                self,
                Phase::Refused(why),
                "Acta did not start its browser: a debugging port or another WebView2 override is configured for Acta; the user must remove it",
            );
        }
        if let Err(err) = wry::webview_version() {
            crate::log::line(&format!("browser: no WebView2 runtime: {err}"));
            return refused(
                self,
                Phase::RuntimeMissing,
                "the WebView2 runtime is not installed; ask the user to install it",
            );
        }
        let shared = self.shared.clone();
        let downloads = self.shared.clone();
        let popups = self.shared.clone();
        let popup_jobs = self.jobs.clone();
        let loads = self.jobs.clone();
        let builder = crate::webview_build::builder(move |context| {
            wry::WebViewBuilder::new_with_web_context(context)
                .with_url("about:blank")
                // A browser's default page background, whatever Acta's theme:
                // a page that sets none is black on white for the human and in
                // the agent's screenshots, as in any browser.
                .with_background_color(PAGE_BACKGROUND)
                // The DevTools window, for the human only (ADR 0010, 3,
                // amended): allowed from creation, before the first
                // navigation, since a later settings change applies only
                // from the next one. An in-process window, never a port.
                .with_devtools(true)
                // Never take the keyboard on creation: created on an agent's
                // call, it must not activate Acta's window (run: a new view
                // made Acta the foreground window when it could).
                .with_focused(false)
                .with_navigation_handler(move |url| {
                    let policy = shared.policy.lock().map(|p| p.clone()).unwrap_or_default();
                    match policy.check(&url) {
                        Ok(_) => true,
                        Err(_) => {
                            shared.events.notice(Notice::Navigation(origin_of(&url)));
                            false
                        }
                    }
                })
                .with_download_started_handler(move |url, _| {
                    downloads.events.notice(Notice::Download(origin_of(&url)));
                    false
                })
                .with_new_window_req_handler(move |url, _| {
                    let policy = popups.policy.lock().map(|p| p.clone()).unwrap_or_default();
                    if policy.check(&url).is_ok() {
                        popups.events.notice(Notice::PopupHere(origin_of(&url)));
                        let _ = popup_jobs.try_send(Job::OpenHere(url));
                    } else {
                        popups.events.notice(Notice::Popup(origin_of(&url)));
                    }
                    wry::NewWindowResponse::Deny
                })
                .with_on_page_load_handler(move |event, url| {
                    if let wry::PageLoadEvent::Finished = event {
                        let _ = loads.try_send(Job::Loaded(url));
                    }
                })
        });
        self.build_generation += 1;
        let generation = self.build_generation;
        self.web_task = crate::webview_build::build_later(
            window,
            cx,
            move |parent| {
                let mut context = Box::new(wry::WebContext::new(Some(profile)));
                let started = Instant::now();
                let view = crate::webview_build::build(builder(&mut context), parent)?;
                crate::log::line(&format!(
                    "browser: web view created in {} ms",
                    started.elapsed().as_millis()
                ));
                Ok((view, context))
            },
            move |panel, built, window, cx| {
                if !panel.awaits(generation) {
                    // Closed (or closed and started again) meanwhile.
                    discard(built);
                    return;
                }
                let result = panel.built(built, window, cx).map(|_| true);
                panel.answer_waiting(result, window, cx);
                panel.sync(window, cx);
                cx.notify();
            },
        );
        Ok(())
    }

    /// Whether the build of `generation` is the one awaited.
    fn awaits(&self, generation: u64) -> bool {
        awaited(self.phase, self.build_generation, generation)
    }

    /// The web view created, or not.
    fn built(
        &mut self,
        built: wry::Result<(wry::WebView, Box<wry::WebContext>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match built {
            Ok((view, context)) => {
                let view = gpui_webview::WebView::new(view, window, cx);
                let webview = view.raw().webview();
                let policy = self.shared.clone();
                let allowed = Arc::new(move |url: &str| {
                    policy
                        .policy
                        .lock()
                        .map(|p| p.check(url).is_ok())
                        .unwrap_or(false)
                });
                let hooks = browser_host::install(&webview, self.shared.events.clone(), allowed)
                    .map_err(|e| crate::log::line(&format!("browser: events not hooked: {e}")))
                    .ok();
                let keys = crate::chat_windows::install(view.raw(), self.signals.clone(), true)
                    .map_err(|e| crate::log::line(&format!("browser: keys not hooked: {e}")))
                    .ok();
                if hooks.is_none() {
                    // Without them dialogs, permissions and HTTP sign-in
                    // prompts would reach the page: no browser then.
                    self.phase = Phase::Failed;
                    return Err("the agent browser could not be set up".into());
                }
                self.shared.events.reset();
                self.shared.set_up.store(false, Ordering::SeqCst);
                self.web = Some(Web {
                    _hooks: hooks,
                    _keys: keys,
                    webview,
                    view: cx.new(|_| view),
                    _context: context,
                });
                self.phase = Phase::Live;
                let _ = window;
                Ok(())
            }
            Err(err) => {
                crate::log::line(&format!(
                    "browser: the web view could not be created: {err}"
                ));
                self.phase = Phase::Failed;
                Err("the agent browser could not be created".into())
            }
        }
    }

    /// Closes the web view, or gives up the one being created: what
    /// waited for it is told so.
    pub fn close_web(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.web_task = Task::ready(());
        self.build_generation += 1;
        if self.phase == Phase::Starting {
            self.answer_waiting(Err(CLOSED.into()), window, cx);
        }
        if let Some(web) = self.web.take() {
            let _ = web.view.read(cx).set_visible(false);
            drop(web);
        }
        self.shared.events.reset();
        self.shared.set_up.store(false, Ordering::SeqCst);
        self.phase = Phase::Idle;
        self.activity = None;
        cx.notify();
    }

    /// The header menu is open: the shell counts it as an overlay.
    pub fn menu_open(&self) -> bool {
        self.menu
    }

    /// A call is running: the view's tab shows a dot when it is not on
    /// screen (ADR 0010, decision 1).
    pub fn busy(&self) -> bool {
        self.busy || self.handoff.is_some()
    }

    pub fn set_shown(&mut self, shown: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.shown != shown {
            self.shown = shown;
            if !shown {
                self.menu = false;
                self.tip = None;
            }
            self.sync(window, cx);
            cx.notify();
        }
    }

    pub fn set_covered(&mut self, covered: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay != covered {
            self.overlay = covered;
            self.sync(window, cx);
            cx.notify();
        }
    }

    pub fn set_rearranging(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.rearranging = on;
        self.sync(window, cx);
        cx.notify();
    }

    pub fn release_keyboard(&self, cx: &App) {
        if let Some(web) = &self.web {
            let _ = web.view.read(cx).raw().focus_parent();
        }
    }

    /// Whether the page is drawn in place this frame.
    fn in_place(&self) -> bool {
        self.shown && !self.overlay && !self.rearranging && self.phase == Phase::Live
    }

    /// Visible in place, or parked: visible but outside the window's client
    /// area, at its last size, so Chromium keeps rendering (decision 8).
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let in_place = self.in_place();
        let Some(web) = &self.web else { return };
        let view = web.view.read(cx);
        if !view.visible()
            && let Err(err) = view.set_visible(true)
        {
            crate::log::line(&format!("browser: visibility change failed: {err}"));
        }
        if in_place {
            // The element sets the bounds when it paints.
            return;
        }
        let b = view.bounds();
        let (w, h) = if b.size.width > px(0.) && b.size.height > px(0.) {
            (f64::from(b.size.width), f64::from(b.size.height))
        } else {
            PARKED_SIZE
        };
        let x = f64::from(window.viewport_size().width) + 64.;
        let rect = wry::Rect {
            position: wry::dpi::LogicalPosition::new(x, 0.).into(),
            size: wry::dpi::LogicalSize::new(w, h).into(),
        };
        if let Err(err) = view.set_bounds(rect) {
            crate::log::line(&format!("browser: parking failed: {err}"));
        }
    }

    /// The human typed an address and pressed Enter.
    fn open_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.address.read(cx).value().trim().to_string();
        if raw.is_empty() {
            return;
        }
        let url = if raw.contains("://") || raw == "about:blank" {
            raw
        } else if raw.starts_with("localhost") || raw.starts_with("127.") {
            format!("http://{raw}")
        } else {
            format!("https://{raw}")
        };
        let policy = self
            .shared
            .policy
            .lock()
            .map(|p| p.clone())
            .unwrap_or_default();
        if policy.check(&url).is_err() {
            // The field keeps what was typed; nothing is opened.
            return;
        }
        self.ensure_then(window, cx, move |panel, created, window, cx| {
            if created.is_err() {
                return;
            }
            if let Some(web) = &panel.web {
                let _ = web.view.read(cx).raw().load_url(&url);
            }
            window.focus(&panel.focus, cx);
        });
    }

    /// "Open developer tools" (`F12`, the view menu, the palette): the
    /// DevTools window of the page in view, for the human only; no pipe
    /// operation opens or reads it (components-app.md 5; ADR 0010, 3).
    pub fn open_devtools(&mut self, cx: &mut Context<Self>) {
        if let Some(web) = &self.web {
            browser_host::open_devtools(&web.webview);
        }
        cx.notify();
    }

    /// Whether a page is open to inspect.
    pub fn has_page(&self) -> bool {
        self.web.is_some()
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.shared.stopped.store(true, Ordering::SeqCst);
        if let Some((_, reply)) = self.handoff.take() {
            let _ = reply.send(Handoff::Stopped);
        }
        if let Some(web) = &self.web {
            browser_host::call(&web.webview, "Page.stopLoading", "{}", |_| {});
        }
        crate::log::line("browser: stopped by the user");
        cx.notify();
    }

    fn resume(&mut self, cx: &mut Context<Self>) {
        self.shared.stopped.store(false, Ordering::SeqCst);
        cx.notify();
    }

    fn done(&mut self, cx: &mut Context<Self>) {
        if let Some((_, reply)) = self.handoff.take() {
            let _ = reply.send(Handoff::Done);
        }
        cx.notify();
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.phase = Phase::Idle;
        self.ensure_then(window, cx, |_, _, _, _| {});
    }

    fn activity_text(&self, locale: crate::i18n::Locale) -> Option<String> {
        let (key, target) = match self.activity.as_ref()? {
            Activity::Opened(o) => (Key::BrowserActOpened, Some(o.as_str())),
            Activity::Reloaded => (Key::BrowserActReloaded, None),
            Activity::Back => (Key::BrowserActBack, None),
            Activity::Forward => (Key::BrowserActForward, None),
            Activity::Clicked(t) => (Key::BrowserActClicked, Some(t.as_str())),
            Activity::Typed(t) => (Key::BrowserActTyped, Some(t.as_str())),
            Activity::Screenshot => (Key::BrowserActScreenshot, None),
            Activity::Waiting => (Key::BrowserActWaiting, None),
        };
        Some(match target {
            Some(t) => tr(locale, key, &[("target", Arg::Text(t))]),
            None => text(locale, key),
        })
    }

    /// Who drives the browser, for the header: "driven by T-42", or
    /// `None` while no session is connected.
    pub fn driver_text(&self, locale: crate::i18n::Locale) -> Option<String> {
        match (&self.driver, self.connected) {
            (Some(session), true) => Some(tr(
                locale,
                Key::BrowserDriver,
                &[("session", Arg::Text(session))],
            )),
            (None, true) => Some(text(locale, Key::BrowserDriverUnnamed)),
            _ => None,
        }
    }

    /// Stop or Resume, then the menu holding "Clear browser data..." and the
    /// view menu (workbench.md 4.8.4).
    pub fn header_buttons(
        &mut self,
        place: Place,
        can_split: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.place = place;
        self.can_split = can_split;
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let stopped = self.shared.stopped.load(Ordering::SeqCst);
        let menu = self.menu.then(|| self.render_menu(&t, cx));
        let stop =
            |_: &gpui_kit::MouseDownEvent, _: &mut Window, cx: &mut App| cx.stop_propagation();
        let tip = |this: &Self, which: Tip, label: String| {
            (this.tip == Some(which) && !this.menu).then(|| {
                deferred(
                    anchored()
                        .anchor(gpui_kit::Anchor::BottomRight)
                        .position_mode(gpui_kit::AnchoredPositionMode::Local)
                        .position(point(px(24.), px(-6.)))
                        .child(
                            type_scale::SMALL
                                .el()
                                .px(px(8.))
                                .py(px(4.))
                                .whitespace_nowrap()
                                .bg(t.bg_overlay)
                                .border(px(1.))
                                .border_color(t.border_default)
                                .rounded(px(crate::theme::RADIUS_MD))
                                .shadow(vec![t.shadow_overlay()])
                                .text_color(t.text_primary)
                                .child(label),
                        ),
                )
                .with_priority(2)
            })
        };
        let (icon, label) = if stopped {
            (Icon::Play, text(locale, Key::BrowserResume))
        } else {
            (Icon::Square, text(locale, Key::BrowserStop))
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                ui::ghost_icon_button("browser-stop", icon, 24., stopped, false, true, &t)
                    .on_hover(Self::hover_listener(Tip::Stop, cx))
                    .on_mouse_down(gpui_kit::MouseButton::Left, stop)
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        if stopped {
                            panel.resume(cx)
                        } else {
                            panel.stop(cx)
                        }
                    }))
                    .children(tip(self, Tip::Stop, label)),
            )
            .child(
                div()
                    .relative()
                    .child(self.menu_anchor.measure())
                    .child(
                        ui::ghost_icon_button(
                            "browser-menu",
                            Icon::Ellipsis,
                            24.,
                            self.menu,
                            false,
                            true,
                            &t,
                        )
                        .on_hover(Self::hover_listener(Tip::Menu, cx))
                        .on_mouse_down(gpui_kit::MouseButton::Left, stop)
                        .on_click(cx.listener(|panel, _, _, cx| {
                            panel.menu = !panel.menu;
                            cx.notify();
                        }))
                        .children(tip(
                            self,
                            Tip::Menu,
                            text(locale, Key::ChatMore),
                        )),
                    )
                    .children(menu),
            )
            .into_any_element()
    }

    fn hover_listener(
        which: Tip,
        cx: &mut Context<Self>,
    ) -> impl Fn(&bool, &mut Window, &mut App) + 'static + use<> {
        let panel = cx.entity().downgrade();
        move |hovered: &bool, _: &mut Window, cx: &mut App| {
            let _ = panel.update(cx, |panel, cx| {
                panel.tip = if *hovered { Some(which) } else { None };
                cx.notify();
            });
        }
    }

    fn render_menu(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let locale = settings::locale(cx);
        let close = |panel: &mut BrowserPanel, cx: &mut Context<BrowserPanel>| {
            panel.menu = false;
            cx.notify();
        };
        let live = self.web.is_some();
        let devtools = text(locale, Key::BrowserDevtools);
        let mut panel = menu::panel("browser-menu-panel", t)
            .on_mouse_down_out(cx.listener(move |panel, _, _, cx| close(panel, cx)))
            .child(if live {
                menu::item(
                    "browser-menu-devtools",
                    None,
                    devtools,
                    Some(crate::ui::kbd(&["F12"], t).into_any_element()),
                    false,
                    t,
                )
                .on_click(cx.listener(move |panel, _, _, cx| {
                    close(panel, cx);
                    panel.open_devtools(cx);
                }))
            } else {
                menu::disabled_item("browser-menu-devtools", devtools, None, t)
            })
            .child(
                menu::item(
                    "browser-menu-clear",
                    None,
                    text(locale, Key::BrowserClear),
                    None,
                    false,
                    t,
                )
                .on_click(cx.listener(move |panel, _, _, cx| {
                    close(panel, cx);
                    cx.emit(BrowserEvent::Clear);
                })),
            )
            .child(menu::separator(t));
        let moves = [
            (Place::Side(Side::Left), Key::ViewMoveLeft),
            (Place::Side(Side::Right), Key::ViewMoveRight),
            (Place::Side(Side::Bottom), Key::ViewMoveBottom),
            (Place::Page, Key::ViewMovePage),
        ];
        for (i, (place, key)) in moves.into_iter().enumerate() {
            panel = panel.child(if self.place == place {
                menu::disabled_item(
                    ("browser-menu-move", i),
                    text(locale, key),
                    Some(menu::check(t)),
                    t,
                )
            } else {
                menu::item(
                    ("browser-menu-move", i),
                    None,
                    text(locale, key),
                    None,
                    false,
                    t,
                )
                .on_click(cx.listener(move |panel, _, _, cx| {
                    close(panel, cx);
                    cx.emit(BrowserEvent::View(ViewAction::MoveTo(place)));
                }))
            });
        }
        let split = text(locale, Key::ViewSplitOff);
        panel = panel.child(if self.can_split {
            menu::item("browser-menu-split", None, split, None, false, t).on_click(cx.listener(
                move |panel, _, _, cx| {
                    close(panel, cx);
                    cx.emit(BrowserEvent::View(ViewAction::SplitOff));
                },
            ))
        } else {
            menu::disabled_item("browser-menu-split", split, None, t)
        });
        panel = panel.child(menu::separator(t)).child(
            menu::item(
                "browser-menu-hide",
                None,
                text(locale, Key::ViewHide),
                None,
                false,
                t,
            )
            .on_click(cx.listener(move |panel, _, _, cx| {
                close(panel, cx);
                cx.emit(BrowserEvent::View(ViewAction::Hide));
            })),
        );
        self.menu_anchor
            .below(panel, crate::menu::Align::End)
            .unwrap_or_else(|| div().into_any_element())
    }

    /// The address field, 28 high, under the header.
    fn address_row(&self, t: &Tokens) -> AnyElement {
        div()
            .flex_shrink_0()
            .px(px(8.))
            .py(px(4.))
            .child(
                div()
                    .h(px(28.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS_SM))
                    .bg(t.bg_raised)
                    .border(px(1.))
                    .border_color(t.border_strong)
                    .child(
                        Input::new(&self.address)
                            .appearance(false)
                            .px(px(0.))
                            .text_size(px(type_scale::UI.size)),
                    ),
            )
            .into_any_element()
    }

    fn body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let empty = || div().flex().flex_col().p(px(16.)).max_w(px(480. + 32.));
        let heading = |key: Key| {
            type_scale::HEADING
                .el()
                .text_color(t.text_primary)
                .child(text(locale, key))
        };
        let reading = |key: Key| {
            type_scale::READING
                .el()
                .mt(px(8.))
                .text_color(t.text_secondary)
                .child(text(locale, key))
        };
        let small = |key: Key| {
            type_scale::SMALL
                .el()
                .text_color(t.text_tertiary)
                .child(text(locale, key))
        };
        let buttons = || div().mt(px(24.)).flex().flex_row().flex_wrap().gap(px(8.));
        let retry = |id: &'static str, variant: Variant, cx: &mut Context<Self>| {
            ui::button(id, text(locale, Key::CommonTryAgain), variant, false, &t)
                .on_click(cx.listener(|panel, _, window, cx| panel.retry(window, cx)))
        };
        if (self.overlay || self.rearranging) && self.phase == Phase::Live {
            return empty()
                .child(small(if self.rearranging {
                    Key::LayoutWebviewHidden
                } else {
                    Key::BrowserHidden
                }))
                .into_any_element();
        }
        let stopped = self.shared.stopped.load(Ordering::SeqCst);
        let notices = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(8.))
            .when_some(self.pipe, |this, f| {
                this.pt(px(8.)).child(ui::notice(
                    Some((Icon::TriangleAlert, t.warning)),
                    text(
                        locale,
                        match f {
                            PipeFailure::Taken => Key::BrowserPipeTaken,
                            PipeFailure::Other => Key::BrowserPipeFailed,
                        },
                    ),
                    &t,
                ))
            })
            .when(stopped, |this| {
                this.pt(px(8.)).child(
                    ui::notice(
                        Some((Icon::Square, t.text_secondary)),
                        text(locale, Key::BrowserStopped),
                        &t,
                    )
                    .child(
                        ui::button(
                            "browser-resume",
                            text(locale, Key::BrowserResume),
                            Variant::Ghost,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(|panel, _, _, cx| panel.resume(cx))),
                    ),
                )
            })
            .when_some(
                self.handoff.as_ref().map(|(m, _)| m.clone()),
                |this, message| {
                    this.pt(px(8.)).child(
                        ui::notice(
                            Some((Icon::Info, t.text_secondary)),
                            tr(
                                locale,
                                Key::BrowserHandoff,
                                &[("message", Arg::Text(&message))],
                            ),
                            &t,
                        )
                        .child(
                            ui::button(
                                "browser-done",
                                text(locale, Key::BrowserDone),
                                Variant::Primary,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|panel, _, _, cx| panel.done(cx))),
                        ),
                    )
                },
            );
        let activity = self.activity_text(locale).map(|line| {
            type_scale::SMALL
                .el()
                .flex_shrink_0()
                .h(px(24.))
                .px(px(12.))
                .flex()
                .items_center()
                .min_w_0()
                .truncate()
                .text_color(t.text_secondary)
                .child(line)
        });
        let content = match self.phase {
            Phase::Idle => empty()
                .child(heading(Key::BrowserIdleTitle))
                .child(reading(Key::BrowserIdleBody))
                .into_any_element(),
            Phase::Starting => empty()
                .child(small(Key::BrowserStarting))
                .into_any_element(),
            Phase::RuntimeMissing => empty()
                .child(heading(Key::BrowserRuntimeTitle))
                .child(reading(Key::BrowserRuntimeBody))
                .child(
                    buttons()
                        .child(
                            ui::button(
                                "browser-runtime-get",
                                text(locale, Key::ChatRuntimeGet),
                                Variant::Primary,
                                false,
                                &t,
                            )
                            .on_click(|_, _, cx| cx.open_url(crate::chat::WEBVIEW2_DOWNLOAD)),
                        )
                        .child(retry("browser-runtime-retry", Variant::Ghost, cx)),
                )
                .into_any_element(),
            Phase::Refused(_) => empty()
                .child(heading(Key::BrowserRefusedTitle))
                .child(reading(Key::BrowserRefusedBody))
                .child(buttons().child(retry("browser-refused-retry", Variant::Secondary, cx)))
                .into_any_element(),
            Phase::Failed => empty()
                .child(heading(Key::BrowserFailedTitle))
                .child(buttons().child(retry("browser-failed-retry", Variant::Primary, cx)))
                .into_any_element(),
            Phase::Live => {
                let page = self
                    .web
                    .as_ref()
                    .filter(|_| self.in_place())
                    .map(|web| web.view.clone().into_any_element());
                div().flex_1().min_h_0().children(page).into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(self.address_row(&t))
            .children(activity)
            .child(notices)
            .child(div().flex().flex_col().flex_1().min_h_0().child(content))
            .into_any_element()
    }
}

impl Render for BrowserPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let _ = window;
        let body = self.body(cx);
        div()
            .id("browser-view")
            .key_context("ToolView")
            .track_focus(&self.focus)
            // `F12` while the view (its address field) has the keyboard;
            // from the page it comes through the kept keys.
            .on_key_down(cx.listener(|panel, e: &gpui_kit::KeyDownEvent, _, cx| {
                let m = &e.keystroke.modifiers;
                if e.keystroke.key == "f12" && !m.control && !m.alt && !m.shift && !m.platform {
                    panel.open_devtools(cx);
                    cx.stop_propagation();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg_sunken)
            .child(body)
    }
}

/// A web view that landed for a build given up: hidden, then dropped.
fn discard(built: wry::Result<(wry::WebView, Box<wry::WebContext>)>) {
    if let Ok((view, _context)) = built {
        let _ = view.set_visible(false);
    }
}

/// A build lands only while the panel still starts that very build: a
/// close (idle, cleared data) or a newer build gives it up.
fn awaited(phase: Phase, current: u64, generation: u64) -> bool {
    phase == Phase::Starting && current == generation
}

#[cfg(test)]
mod build_tests {
    use super::*;

    #[test]
    fn a_build_given_up_is_not_kept() {
        assert!(awaited(Phase::Starting, 3, 3));
        // Closed while it was being created: the close bumped the count.
        assert!(!awaited(Phase::Idle, 4, 3));
        // Closed and started again: the newer build is the one awaited.
        assert!(!awaited(Phase::Starting, 5, 3));
        assert!(!awaited(Phase::Live, 3, 3));
    }
}
