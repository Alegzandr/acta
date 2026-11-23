//! The Agent browser view (ADR 0010; docs/design/components-app.md 5,
//! Agent browser; layout.md 4.5.1): the browsers of the current page,
//! several per page, numbered, all on the one shared agent profile
//! (`<data dir>/agent-browser`). A browser is opened by the human ("New
//! browser here", the view's `plus`) or by a Claude Code session of the
//! page on its first call; each session drives its own by default and
//! another of the page by number, never one of another page. Browsers of
//! other pages keep running, parked. Windows only.
//!
//! The pipe is served here, for every browser: `acta-browser`'s server
//! asks [`Registry`] (on the pipe threads) for a page's browsers, and a
//! new one is opened on the UI thread through the dock's job channel.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use acta_browser::server::{BrowserOf, Pages, Server};
use acta_core::model::{EpicId, Owner, ProjectId, TopicId};
use async_channel::Sender;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, div, px,
};
use zeroize::Zeroizing;

use crate::browser_panel::{AppHost, BrowserEvent, BrowserPanel, PipeFailure};
use crate::db::Db;
use crate::i18n::{Arg, Key, text, tr};
use crate::icons::Icon;
use crate::layout::{Place, Side};
use crate::settings;
use crate::terminal_dock::Origin;
use crate::theme::Tokens;
use crate::tools_model::{self as tm, Holder, InstanceKey, Instances, WebUse};
use crate::type_scale;
use crate::ui::{self, Variant};

const CLEAR_ATTEMPTS: u32 = 40;
const CLEAR_PAUSE: Duration = Duration::from_millis(250);
/// A browser a session asked for that did not open by then: refused.
const OPEN_WAIT: Duration = Duration::from_secs(10);

/// A page as the pipe names it (`T-42`, `E-3`, `P-1`).
pub fn parse_page(page: &str) -> Option<Owner> {
    TopicId::parse(page)
        .map(Owner::Topic)
        .or_else(|| EpicId::parse(page).map(Owner::Epic))
        .or_else(|| ProjectId::parse(page).map(Owner::Project))
}

/// What the pipe threads ask of the UI thread.
enum DockJob {
    Open {
        owner: Owner,
        opener: String,
        launch: Option<String>,
        reply: mpsc::Sender<Result<u8, String>>,
    },
    PipeFailed(PipeFailure),
}

struct Entry {
    host: Arc<AppHost>,
    /// The session that opened it; `None` when the human did.
    opener: Option<String>,
}

/// The browsers by page and number, as the pipe threads see them.
pub struct Registry {
    browsers: Mutex<HashMap<(Owner, u8), Entry>>,
    jobs: Sender<DockJob>,
    /// The page of each Acta terminal session, by launch token.
    launches: crate::sessions_view::LaunchPages,
}

impl Pages for Registry {
    type Host = AppHost;

    fn browsers(&self, page: &str) -> Vec<BrowserOf> {
        let Some(owner) = parse_page(page) else {
            return Vec::new();
        };
        let map = self.browsers.lock().unwrap_or_else(|p| p.into_inner());
        let mut list: Vec<BrowserOf> = map
            .iter()
            .filter(|((o, _), _)| *o == owner)
            .map(|((_, n), e)| BrowserOf {
                n: *n,
                opener: e.opener.clone(),
            })
            .collect();
        list.sort_by_key(|b| b.n);
        list
    }

    fn host(&self, page: &str, n: u8) -> Option<Arc<AppHost>> {
        let owner = parse_page(page)?;
        let map = self.browsers.lock().unwrap_or_else(|p| p.into_inner());
        map.get(&(owner, n)).map(|e| e.host.clone())
    }

    fn open(&self, page: &str, opener: &str, launch: Option<&str>) -> Result<u8, String> {
        let owner = parse_page(page).ok_or(acta_browser::server::NO_PAGE)?;
        let (tx, rx) = mpsc::channel();
        let job = DockJob::Open {
            owner,
            opener: opener.to_owned(),
            launch: launch.map(str::to_owned),
            reply: tx,
        };
        if self.jobs.send_blocking(job).is_err() {
            return Err("Acta closed; call again to reopen it, or ask the user".into());
        }
        rx.recv_timeout(OPEN_WAIT)
            .unwrap_or_else(|_| Err("the agent browser did not open within 10 s".into()))
    }

    fn page_of_launch(&self, launch: &str) -> Option<String> {
        let map = self.launches.0.lock().unwrap_or_else(|p| p.into_inner());
        map.get(launch).map(|owner| owner.to_string())
    }

    fn driver(&self, page: &str, n: u8, session: Option<&str>, connected: bool) {
        if let Some(host) = self.host(page, n) {
            host.driver(session, connected);
        }
    }

    fn busy(&self, page: &str, n: u8, busy: bool) {
        if let Some(host) = self.host(page, n) {
            host.busy(busy);
        }
    }
}

pub struct BrowserDock {
    panels: Instances<Entity<BrowserPanel>>,
    /// The launch token of the session that opened each, to name it after
    /// that session ("Browser for Claude 2").
    launches: HashMap<InstanceKey, Option<String>>,
    registry: Arc<Registry>,
    current: Option<Origin>,
    focus: FocusHandle,
    shown: bool,
    overlay: bool,
    rearranging: bool,
    place: Place,
    can_split: bool,
    pipe: Option<PipeFailure>,
    clearing: bool,
    subs: HashMap<InstanceKey, Subscription>,
    _jobs: Task<()>,
    _pipe: Task<()>,
    clear_task: Task<()>,
}

impl EventEmitter<BrowserEvent> for BrowserDock {}

impl Focusable for BrowserDock {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.shown_panel() {
            Some(panel) => panel.read(cx).focus_handle(cx),
            None => self.focus.clone(),
        }
    }
}

impl BrowserDock {
    pub fn new(db: Db, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (jobs, jobs_rx) = async_channel::unbounded();
        let registry = Arc::new(Registry {
            browsers: Mutex::new(HashMap::new()),
            jobs,
            launches: crate::sessions_view::launch_pages(cx),
        });
        let listen = cx.spawn_in(window, async move |dock, cx| {
            while let Ok(job) = jobs_rx.recv().await {
                if dock
                    .update_in(cx, |dock, window, cx| dock.on_job(job, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let pipe = start_pipe(db, registry.clone(), cx);
        Self {
            panels: Instances::new(tm::BROWSER_MAX),
            launches: HashMap::new(),
            registry,
            current: None,
            focus: cx.focus_handle(),
            shown: false,
            overlay: false,
            rearranging: false,
            place: Place::Side(Side::Right),
            can_split: false,
            pipe: None,
            clearing: false,
            subs: HashMap::new(),
            _jobs: listen,
            _pipe: pipe,
            clear_task: Task::ready(()),
        }
    }

    fn on_job(&mut self, job: DockJob, window: &mut Window, cx: &mut Context<Self>) {
        match job {
            DockJob::Open {
                owner,
                opener,
                launch,
                reply,
            } => {
                let opened = self.open(owner, Some(opener), launch, window, cx);
                let _ = reply.send(opened.map(|k| k.n).ok_or_else(|| {
                    tr(
                        settings::locale(cx),
                        Key::ToolLimit,
                        &[("n", Arg::Count(tm::BROWSER_MAX as u64))],
                    )
                }));
            }
            DockJob::PipeFailed(f) => {
                crate::log::line(&format!("browser: the pipe is not served: {f:?}"));
                self.pipe = Some(f);
                for (_, panel) in self.panels.iter() {
                    panel.update(cx, |p, cx| p.set_pipe(Some(f), cx));
                }
                cx.notify();
            }
        }
    }

    /// A new browser of `owner`'s page; `None` at the limit.
    fn open(
        &mut self,
        owner: Owner,
        opener: Option<String>,
        launch: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<InstanceKey> {
        let panel = cx.new(|cx| BrowserPanel::new(window, cx));
        let pipe = self.pipe;
        let clearing = self.clearing;
        panel.update(cx, |p, cx| {
            p.set_pipe(pipe, cx);
            p.set_clearing(clearing, cx);
        });
        // Opened by a session: it stays where it is in view; the human's
        // own becomes the one shown.
        let selected = self.panels.selected(Holder::Page(owner));
        let key = self.panels.open(Holder::Page(owner), panel.clone()).ok()?;
        if opener.is_some()
            && let Some(selected) = selected
        {
            self.panels.select(selected);
        }
        let host = panel.read(cx).host();
        self.registry
            .browsers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert((owner, key.n), Entry { host, opener });
        self.launches.insert(key, launch);
        let sub = cx.subscribe(&panel, |_, _, event: &BrowserEvent, cx| {
            cx.emit(match event {
                BrowserEvent::View(a) => BrowserEvent::View(*a),
                BrowserEvent::Clear => BrowserEvent::Clear,
                BrowserEvent::ClearFailed => BrowserEvent::ClearFailed,
                BrowserEvent::Kept(k) => BrowserEvent::Kept(*k),
            })
        });
        self.subs.insert(key, sub);
        self.sync(window, cx);
        cx.notify();
        Some(key)
    }

    /// "New browser here" (the view's `plus`, the palette): opened by the
    /// human, "Browser 1", shown.
    pub fn open_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = self.current.as_ref().map(|o| o.owner) else {
            return;
        };
        if self.open(owner, None, None, window, cx).is_some()
            && let Some(panel) = self.shown_panel()
        {
            let handle = panel.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
    }

    pub fn can_open(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|o| !self.panels.at_limit(Holder::Page(o.owner)))
    }

    /// The instance's `x`: its web view closes and it leaves the page; a
    /// session that drove it gets a new one on its next call.
    fn close(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Holder::Page(owner) = key.holder else {
            return;
        };
        self.registry
            .browsers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&(owner, key.n));
        self.launches.remove(&key);
        self.subs.remove(&key);
        if let Some(panel) = self.panels.close(key) {
            panel.update(cx, |p, cx| p.close_web(window, cx));
        }
        self.sync(window, cx);
        cx.notify();
    }

    fn shown_key(&self) -> Option<InstanceKey> {
        let owner = self.current.as_ref()?.owner;
        self.panels.selected(Holder::Page(owner))
    }

    fn shown_panel(&self) -> Option<&Entity<BrowserPanel>> {
        self.panels.get(self.shown_key()?)
    }

    /// The page the shell shows now (layout.md 4.5.1).
    pub fn set_current(
        &mut self,
        origin: Option<Origin>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current == origin {
            return;
        }
        self.current = origin;
        self.sync(window, cx);
        cx.notify();
    }

    /// Every browser in view or parked: the current page's selected one
    /// when the view is on screen, every other one parked (it keeps
    /// rendering for the agent's screenshots). One closed while idle comes
    /// back at its address when it is shown again.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.shown_key().filter(|_| self.shown);
        let (overlay, rearranging) = (self.overlay, self.rearranging);
        let panels: Vec<(InstanceKey, Entity<BrowserPanel>)> =
            self.panels.iter().map(|(k, p)| (k, p.clone())).collect();
        for (key, panel) in panels {
            let on = shown == Some(key);
            panel.update(cx, |p, cx| {
                p.set_covered(overlay, window, cx);
                p.set_rearranging(rearranging, window, cx);
                p.set_shown(on, window, cx);
                if on {
                    p.reopen(window, cx);
                }
            });
        }
    }

    pub fn set_shown(&mut self, shown: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.shown != shown {
            self.shown = shown;
            self.sync(window, cx);
        }
    }

    pub fn set_covered(&mut self, covered: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay != covered {
            self.overlay = covered;
            self.sync(window, cx);
        }
    }

    pub fn set_rearranging(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.rearranging = on;
        self.sync(window, cx);
        cx.notify();
    }

    pub fn release_keyboard(&self, cx: &App) {
        if let Some(panel) = self.shown_panel() {
            panel.read(cx).release_keyboard(cx);
        }
    }

    /// The menu of the browser in view is open: an overlay for the shell.
    pub fn menu_open(&self, cx: &App) -> bool {
        self.shown_panel().is_some_and(|p| p.read(cx).menu_open())
    }

    /// A browser of this view works (a call, a handoff): the tab's dot.
    pub fn busy(&self, cx: &App) -> bool {
        self.panels.iter().any(|(_, p)| p.read(cx).busy())
    }

    pub fn open_devtools(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = self.shown_panel().cloned() {
            panel.update(cx, |p, cx| p.open_devtools(cx));
        }
    }

    pub fn has_page(&self, cx: &App) -> bool {
        self.shown_panel().is_some_and(|p| p.read(cx).has_page())
    }

    pub fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = self.current.as_ref().map(|o| o.owner) else {
            return;
        };
        self.panels.step(Holder::Page(owner), forward);
        self.sync(window, cx);
        cx.notify();
    }

    /// The open web views, for the shell's 10-minute and 6-view rules.
    pub fn web_uses(&self, cx: &App) -> Vec<WebUse<InstanceKey>> {
        let shown = self.shown_key().filter(|_| self.shown);
        self.panels
            .iter()
            .map(|(key, panel)| {
                let (open, last_used, busy) = panel.read(cx).web_use();
                WebUse {
                    key,
                    open,
                    on_screen: shown == Some(key),
                    last_used,
                    busy,
                }
            })
            .collect()
    }

    /// Closes the browser's web view (its address kept).
    pub fn close_web(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(panel) = self.panels.get(key).cloned() {
            crate::log::line("browser: an idle web view was closed");
            panel.update(cx, |p, cx| p.close_web(window, cx));
        }
    }

    /// "Clear browser data", confirmed: every browser's web view closes and
    /// the shared profile is deleted, retrying while its processes let go
    /// of the files.
    pub fn clear_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panels: Vec<Entity<BrowserPanel>> =
            self.panels.iter().map(|(_, p)| p.clone()).collect();
        for panel in &panels {
            panel.update(cx, |p, cx| {
                p.close_web(window, cx);
                p.set_clearing(true, cx);
            });
        }
        let Some(dir) = crate::browser_host::profile_dir() else {
            return;
        };
        self.clearing = true;
        self.clear_task = cx.spawn(async move |dock, cx| {
            let mut removed = false;
            for _ in 0..CLEAR_ATTEMPTS {
                let target = dir.clone();
                let result = cx
                    .background_spawn(async move { std::fs::remove_dir_all(&target) })
                    .await;
                removed = match result {
                    Ok(()) => true,
                    Err(err) => err.kind() == std::io::ErrorKind::NotFound,
                };
                if removed {
                    break;
                }
                cx.background_executor().timer(CLEAR_PAUSE).await;
            }
            crate::log::line(if removed {
                "browser: profile deleted"
            } else {
                "browser: the profile could not be deleted"
            });
            let _ = dock.update(cx, |dock, cx| {
                dock.clearing = false;
                for (_, panel) in dock.panels.iter() {
                    panel.update(cx, |p, cx| p.set_clearing(false, cx));
                }
                if !removed {
                    cx.emit(BrowserEvent::ClearFailed);
                }
                cx.notify();
            });
        });
    }

    /// The terminal instance number of the session with `token`, when it
    /// runs in Acta's terminal.
    fn claude_number(token: &str, cx: &App) -> Option<u8> {
        cx.try_global::<crate::sessions_view::TerminalTokens>()?
            .0
            .iter()
            .find(|a| a.token == token)
            .map(|a| a.key.n)
    }

    /// The header's context: the owner, then who drives the browser in
    /// view ("driven by Claude 2", else "driven by T-42").
    pub fn header_context(&self, cx: &App) -> Option<AnyElement> {
        let origin = self.current.as_ref()?;
        let t = cx.global::<Tokens>();
        let locale = settings::locale(cx);
        let mut line = crate::terminal_dock::owner_label(locale, origin.owner, &origin.name);
        if let Some(key) = self.shown_key()
            && let Some(panel) = self.panels.get(key)
        {
            let claude = self
                .launches
                .get(&key)
                .cloned()
                .flatten()
                .and_then(|tok| Self::claude_number(&tok, cx));
            let driver = panel.read(cx).driver_text(locale).map(|d| match claude {
                Some(n) => tr(
                    locale,
                    Key::ToolDrivenBy,
                    &[("n", Arg::Count(u64::from(n)))],
                ),
                None => d,
            });
            if let Some(driver) = driver {
                line.push_str(" \u{b7} ");
                line.push_str(&driver);
            }
        }
        Some(
            type_scale::SMALL
                .el()
                .min_w_0()
                .truncate()
                .text_color(t.text_tertiary)
                .child(line)
                .into_any_element(),
        )
    }

    /// `plus` (New browser here), then the buttons of the browser in view.
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
        let can = self.can_open();
        let tip = if can {
            text(locale, Key::ToolBrowserNew)
        } else {
            tr(
                locale,
                Key::ToolLimit,
                &[("n", Arg::Count(tm::BROWSER_MAX as u64))],
            )
        };
        let own = self
            .shown_panel()
            .cloned()
            .map(|panel| panel.update(cx, |p, cx| p.header_buttons(place, can_split, cx)));
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .when(self.current.is_some(), |this| {
                this.child(
                    ui::ghost_icon_button("browser-new", Icon::Plus, 24., false, !can, true, &t)
                        .tooltip(ui::tip(tip))
                        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation()
                        })
                        .when(can, |this| {
                            this.on_click(
                                cx.listener(|dock, _, window, cx| dock.open_new(window, cx)),
                            )
                        }),
                )
            })
            .children(own)
            .into_any_element()
    }

    /// The instance strip: from two browsers on the page.
    fn strip(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let owner = self.current.as_ref()?.owner;
        let keys = self.panels.keys_of(Holder::Page(owner));
        if keys.len() < 2 {
            return None;
        }
        let locale = settings::locale(cx);
        let shown = self.shown_key();
        let tabs: Vec<AnyElement> = keys
            .into_iter()
            .map(|key| {
                let active = shown == Some(key);
                let id = format!("browser-inst-{}", key.n);
                let claude = self
                    .launches
                    .get(&key)
                    .cloned()
                    .flatten()
                    .and_then(|tok| Self::claude_number(&tok, cx));
                let label = match claude {
                    Some(n) => tr(
                        locale,
                        Key::ToolBrowserFor,
                        &[("n", Arg::Count(u64::from(n)))],
                    ),
                    None => tr(
                        locale,
                        Key::ToolBrowserN,
                        &[("n", Arg::Count(u64::from(key.n)))],
                    ),
                };
                crate::tool_strip::tab(id.clone(), None, label, active, t)
                    .on_click(cx.listener(move |dock, _, window, cx| {
                        dock.panels.select(key);
                        dock.sync(window, cx);
                        cx.notify();
                    }))
                    .child(
                        crate::tool_strip::close(
                            format!("browser-inst-close-{}", key.n),
                            id,
                            active,
                            t,
                        )
                        .on_click(cx.listener(
                            move |dock, _, window, cx| {
                                cx.stop_propagation();
                                dock.close(key, window, cx)
                            },
                        )),
                    )
                    .into_any_element()
            })
            .collect();
        Some(crate::tool_strip::strip(tabs, t))
    }
}

impl Render for BrowserDock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let strip = self.strip(&t, cx);
        let body: AnyElement = match (self.current.is_some(), self.shown_panel()) {
            (_, Some(panel)) => panel.clone().into_any_element(),
            // A page with no browser: what the browser is, and "New
            // browser here".
            (true, None) => div()
                .flex()
                .flex_col()
                .p(px(16.))
                .max_w(px(480. + 32.))
                .child(
                    type_scale::HEADING
                        .el()
                        .text_color(t.text_primary)
                        .child(text(locale, Key::BrowserIdleTitle)),
                )
                .child(
                    type_scale::READING
                        .el()
                        .mt(px(8.))
                        .text_color(t.text_secondary)
                        .child(text(locale, Key::BrowserIdleBody)),
                )
                .child(
                    div().mt(px(24.)).flex().child(
                        ui::button(
                            "browser-open-new",
                            text(locale, Key::ToolBrowserNew),
                            Variant::Primary,
                            false,
                            &t,
                        )
                        .on_click(cx.listener(|dock, _, window, cx| dock.open_new(window, cx))),
                    ),
                )
                .into_any_element(),
            // A page that owns nothing has no browser.
            (false, None) => div().into_any_element(),
        };
        div()
            .id("browser-dock")
            .key_context("ToolView")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg_sunken)
            .children(strip)
            .child(div().flex().flex_col().flex_1().min_h_0().child(body))
    }
}

/// Derives the pipe key from the database key off the UI thread, then
/// serves the pipe on its own threads for the life of the app.
fn start_pipe(db: Db, registry: Arc<Registry>, cx: &mut Context<BrowserDock>) -> Task<()> {
    cx.background_spawn(async move {
        let (key, data_dir) = {
            let store = db.lock();
            (
                store.derive_secret(acta_browser::auth::KEY_LABEL),
                store.data_dir().to_path_buf(),
            )
        };
        let sid = match acta_browser::pipe::user_sid() {
            Ok(sid) => sid,
            Err(e) => {
                crate::log::line(&format!("browser: no user id: {:?}", e.kind()));
                let _ = registry
                    .jobs
                    .send_blocking(DockJob::PipeFailed(PipeFailure::Other));
                return;
            }
        };
        let name = acta_browser::pipe::pipe_name(&sid, &key, &data_dir);
        let spawned = std::thread::Builder::new()
            .name("acta-browser-pipe".into())
            .spawn(move || serve_pipe(&name, &sid, key, registry));
        if let Err(e) = spawned {
            crate::log::line(&format!("browser: pipe thread: {:?}", e.kind()));
        }
    })
}

/// Serves the pipe until the app exits: one thread per connection, the
/// leases and the operations in `acta_browser::server`.
fn serve_pipe(name: &str, sid: &str, key: Zeroizing<[u8; 32]>, registry: Arc<Registry>) {
    let failed = |f: PipeFailure| {
        let _ = registry.jobs.send_blocking(DockJob::PipeFailed(f));
    };
    let mut pipe = match acta_browser::pipe::Server::new(name, sid) {
        Ok(p) => p,
        Err(e) => {
            crate::log::line(&format!("browser: pipe: {:?}", e.kind()));
            failed(PipeFailure::Other);
            return;
        }
    };
    let key = Arc::new(key);
    let server = Arc::new(Server::new(registry.clone()));
    let mut first = true;
    loop {
        let accepted = match pipe.accept() {
            Ok(a) => a,
            Err(e) => {
                // The first instance failing means the name is taken.
                crate::log::line(&format!("browser: pipe not served: {:?}", e.kind()));
                failed(if first {
                    PipeFailure::Taken
                } else {
                    PipeFailure::Other
                });
                return;
            }
        };
        if first {
            // The user's SID is not logged.
            let dacl = accepted.dacl.replace(sid, "<user>");
            crate::log::line(&format!("browser: pipe served, DACL {dacl}"));
        }
        first = false;
        let (server, key) = (server.clone(), key.clone());
        let spawned = std::thread::Builder::new()
            .name("acta-browser-conn".into())
            .spawn(move || {
                let Ok(writer) = accepted.file.try_clone() else {
                    return;
                };
                let conn = acta_browser::wire::Conn::new(accepted.file, writer);
                if let Err(why) = server.serve(conn, &key) {
                    crate::log::line(&format!("browser: a client was refused: {why}"));
                }
            });
        if let Err(e) = spawned {
            crate::log::line(&format!("browser: connection thread: {:?}", e.kind()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_named_as_the_pipe_names_it() {
        assert_eq!(parse_page("T-42"), Some(Owner::Topic(TopicId(42))));
        assert_eq!(parse_page("E-3"), Some(Owner::Epic(EpicId(3))));
        assert_eq!(parse_page("P-1"), Some(Owner::Project(ProjectId(1))));
        assert_eq!(parse_page("K-7"), None);
        assert_eq!(parse_page(""), None);
    }
}
