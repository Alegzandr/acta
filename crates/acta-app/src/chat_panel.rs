//! The Chat view (ADR 0009; docs/design/components-app.md 5, Chat panel;
//! layout.md 4.5.1; screens.md 8.13, 8.36; workbench.md 4.8): claude.ai in
//! the system web view, in whatever area the layout puts it. Windows only;
//! macOS and Linux open claude.ai in the browser instead (ADR 0009,
//! changelog 2026-11-21).
//!
//! Each chat belongs to the page it was opened from, several per page,
//! numbered; the view shows the current page's. Every chat uses the one
//! shared profile (`<data dir>/webview`), so one sign-in serves them all.
//! A chat remembers the claude.ai address it was last on, encrypted with
//! its page's local values, and reopens there.
//!
//! A web view is created the first time the user asks for it and lives on
//! while the panel is hidden, until it has been off screen and unused for
//! 10 minutes or more than 6 web views are open (the shell decides, from
//! [`ChatPanel::web_uses`]). It is a native view above everything GPUI
//! draws, so it is hidden whenever it is not the surface: another chat or
//! page, panel closed, an overlay open, any state but the live page. Acta
//! never reads from or writes to the page: no script of its own, no IPC
//! handler, no custom protocol, no devtools, the stock user agent. (wry
//! itself always adds a `window.ipc.postMessage` shim to every page; with
//! no handler attached, what the page posts is dropped.) Only the address
//! of the page, which the web view reports, is read.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::zoom::px;
use acta_core::model::Owner;
use async_channel::{Receiver, Sender};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, AnchoredPositionMode, AnyElement, App, AppContext as _, Context, Entity, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Task, Window, anchored, deferred, div, point,
};

use crate::chat::{self, Built, ChatState, Decision, Effect, Kept, Reason, Surface};
use crate::db::Db;
use crate::i18n::{Arg, Key, text, tr};
use crate::icons::Icon;
use crate::layout::{Place, Side, ViewAction};
use crate::menu;
use crate::settings;
use crate::terminal_dock::Origin;
use crate::theme::{RADIUS_MD, Tokens};
use crate::tools_model::{self as tm, Holder, InstanceKey, Instances, WebUse};
use crate::type_scale;
use crate::ui::{self, Variant};

/// components-app.md 5: the starting line appears after 400 ms.
const STARTING_LINE: Duration = Duration::from_millis(400);
/// components.md 5, Tooltip: after 500 ms of hover; neighbours instantly
/// until the pointer rests elsewhere for 1 s.
const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
const TOOLTIP_WARM: Duration = Duration::from_secs(1);
/// Deleting the profile: the web view's processes take a moment to let go
/// of its files.
const CLEAR_ATTEMPTS: u32 = 40;
const CLEAR_PAUSE: Duration = Duration::from_millis(250);

/// What the web view's handlers report; they run outside GPUI.
#[derive(Clone, Debug)]
pub enum Signal {
    /// A navigation or new window to send to the system browser.
    OpenInBrowser(String),
    /// A key Acta keeps was pressed while the page had the keyboard.
    Key(Kept),
    /// The page took the keyboard (a click into it).
    Focused,
    LoadFailed(Reason),
}

pub enum ChatEvent {
    /// A kept key, to run as if GPUI had received it.
    Kept(Kept),
    /// An item of the view menu in the chat's own menu (4.8.4).
    View(ViewAction),
    /// "Sign out and clear chat data..." from the header menu: the shell
    /// asks for confirmation.
    SignOut,
    /// The profile could not be deleted; it is tried again at next launch.
    ClearFailed,
    /// A web view was created: the shell applies the 6-view limit.
    Opened,
}

/// A header button with a tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tip {
    New,
    Browser,
    Reload,
    Menu,
}

/// The live web view and what it needs to stay alive. Fields drop in
/// order: the event handlers first, the context last.
struct Web {
    _hooks: Option<crate::chat_windows::Hooks>,
    view: Entity<gpui_webview::WebView>,
    _context: Box<wry::WebContext>,
}

/// One chat of a page.
struct Chat {
    state: ChatState,
    web: Option<Web>,
    /// The claude.ai address it was last on: where it reopens.
    address: Option<String>,
    /// Last on screen or used, for the 10-minute rule.
    last_used: Instant,
    /// Bumped by every build and every drop: a build that lands for an
    /// older one is discarded.
    generation: u64,
    signals: Sender<Signal>,
    _listen: Task<()>,
    starting_task: Task<()>,
    build_task: Task<()>,
    /// The web view being created (crate::webview_build).
    web_task: Task<()>,
}

/// Where a page's chats are kept: their numbers and the address each was
/// last on, in the encrypted database (data.md 11, request 27).
fn list_key(owner: Owner) -> String {
    format!("chat.instances.{owner}")
}

/// The stored list: `[[1, "https://claude.ai/chat/..."], [2, null]]`.
pub fn encode_list(list: &[(u8, Option<String>)]) -> String {
    serde_json::to_string(list).unwrap_or_else(|_| "[]".into())
}

/// The stored list read back; anything unreadable, or an address that
/// would not stay in the panel, is dropped.
pub fn decode_list(raw: &str, max: usize) -> Vec<(u8, Option<String>)> {
    let list: Vec<(u8, Option<String>)> = serde_json::from_str(raw).unwrap_or_default();
    let mut out: Vec<(u8, Option<String>)> = Vec::new();
    for (n, address) in list {
        if n == 0 || out.iter().any(|(m, _)| *m == n) || out.len() >= max {
            continue;
        }
        let address = address.filter(|a| chat::classify(a) == Decision::InPanel);
        out.push((n, address));
    }
    out
}

pub struct ChatPanel {
    db: Db,
    chats: Instances<Chat>,
    /// The current page (layout.md 4.5.1), set by the shell.
    current: Option<Origin>,
    /// Pages whose stored chats were read.
    loaded: HashSet<Owner>,
    /// The user has opened claude.ai here before: showing a chat starts
    /// its web view without asking again.
    consented: bool,
    focus: FocusHandle,
    /// The view is on screen.
    shown: bool,
    /// Covered by an overlay, and hidden for a layout drag (4.8.9): the
    /// web view hides for either, with the line of the one that applies.
    overlay: bool,
    rearranging: bool,
    /// Where the view is and whether it can split, for its menu.
    place: Place,
    can_split: bool,
    /// The sign-in help notice is shown.
    help: bool,
    /// The header's menu is open (it would fall under the web view).
    menu: bool,
    menu_anchor: crate::menu::AnchorBounds,
    /// The profile is being deleted; a start waits for it.
    clearing: bool,
    build_after_clear: Vec<InstanceKey>,
    /// The hovered header button, and whether its tooltip shows yet.
    tip: Option<(Tip, bool)>,
    /// Until when a neighbour's tooltip shows without delay.
    tips_warm_until: Option<Instant>,
    clear_task: Task<()>,
    tip_task: Task<()>,
    _load: Task<()>,
    _activation: gpui_kit::Subscription,
}

impl EventEmitter<ChatEvent> for ChatPanel {}

impl Focusable for ChatPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// `<data dir>/webview`: outside the Velopack root, so updates keep the
/// sign-in and uninstall leaves it alone (ADR 0009, decision 3). `None`
/// when the data directory is unknown: then no web view is created, since
/// WebView2 would otherwise put its profile next to the executable. One
/// profile for every chat of every page.
fn profile_dir() -> Option<std::path::PathBuf> {
    use acta_core::paths::{BaseDirs, Os, data_dir};
    Some(data_dir(Os::CURRENT, &BaseDirs::from_env())?.join("webview"))
}

impl ChatPanel {
    pub fn new(db: Db, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = settings::current(cx);
        let clear_pending = prefs.chat_clear_pending;
        // After the window was in the background, the keyboard comes back to
        // GPUI; give it to the page again when the panel had it.
        let activation = cx.observe_window_activation(window, |panel, window, cx| {
            if window.is_window_active() && panel.focus.is_focused(window) {
                panel.focus_web(cx);
            }
        });
        let mut panel = Self {
            db,
            chats: Instances::new(tm::CHAT_MAX),
            current: None,
            loaded: HashSet::new(),
            consented: prefs.chat_started,
            focus: cx.focus_handle(),
            shown: false,
            overlay: false,
            rearranging: false,
            place: Place::Side(Side::Right),
            can_split: false,
            help: false,
            menu: false,
            menu_anchor: Default::default(),
            clearing: false,
            build_after_clear: Vec::new(),
            tip: None,
            tips_warm_until: None,
            clear_task: Task::ready(()),
            tip_task: Task::ready(()),
            _load: Task::ready(()),
            _activation: activation,
        };
        // A sign-out whose deletion did not finish: before any web view.
        if clear_pending {
            panel.clear_profile(window, cx);
        }
        panel
    }

    fn new_chat(&self) -> (Chat, Receiver<Signal>) {
        let (signals, receiver) = async_channel::unbounded();
        (
            Chat {
                state: ChatState::new(self.consented),
                web: None,
                address: None,
                last_used: Instant::now(),
                generation: 0,
                signals,
                _listen: Task::ready(()),
                starting_task: Task::ready(()),
                build_task: Task::ready(()),
                web_task: Task::ready(()),
            },
            receiver,
        )
    }

    /// Adds a chat for `owner` under the lowest free number, listening to
    /// its web view's signals; `None` at the limit.
    fn add_chat(
        &mut self,
        owner: Owner,
        address: Option<String>,
        number: Option<u8>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<InstanceKey> {
        let (mut chat, receiver) = self.new_chat();
        chat.address = address;
        let holder = Holder::Page(owner);
        // A stored chat keeps its number.
        let key = match number {
            Some(n) => self.chats.open_at(holder, n, chat).ok()?,
            None => self.chats.open(holder, chat).ok()?,
        };
        let listen = cx.spawn_in(window, async move |panel, cx| {
            while let Ok(signal) = receiver.recv().await {
                if panel
                    .update_in(cx, |panel, window, cx| {
                        panel.on_signal(key, signal, window, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        if let Some(chat) = self.chats.get_mut(key) {
            chat._listen = listen;
        }
        Some(key)
    }

    fn on_signal(
        &mut self,
        key: InstanceKey,
        signal: Signal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match signal {
            Signal::OpenInBrowser(url) => {
                crate::log::line(&format!(
                    "chat: opened in the browser: {}",
                    chat::for_log(&url)
                ));
                cx.open_url(&url);
            }
            Signal::Key(kept) => cx.emit(ChatEvent::Kept(kept)),
            Signal::Focused => window.focus(&self.focus, cx),
            Signal::LoadFailed(reason) => {
                crate::log::line(&format!("chat: the page did not load: {reason:?}"));
                if let Some(chat) = self.chats.get_mut(key) {
                    chat.state.load_failed(reason);
                }
                self.sync(window, cx);
                cx.notify();
            }
        }
    }

    /// The chat in view: the current page's selected one.
    fn shown_key(&self) -> Option<InstanceKey> {
        let owner = self.current.as_ref()?.owner;
        self.chats.selected(Holder::Page(owner))
    }

    /// The page the shell shows now (layout.md 4.5.1): its chats, read
    /// from the database the first time; nothing else is touched.
    pub fn set_current(
        &mut self,
        origin: Option<Origin>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current == origin {
            return;
        }
        if let Some(key) = self.shown_key() {
            self.read_address(key, cx);
        }
        let owner = origin.as_ref().map(|o| o.owner);
        self.current = origin;
        self.menu = false;
        if let Some(owner) = owner
            && self.loaded.insert(owner)
        {
            self.load(owner, window, cx);
        }
        self.sync(window, cx);
        cx.notify();
    }

    /// The page's stored chats, off the UI thread.
    fn load(&mut self, owner: Owner, window: &mut Window, cx: &mut Context<Self>) {
        let key = list_key(owner);
        let task = self.db.run(cx, move |store| store.local_value(&key));
        self._load = cx.spawn_in(window, async move |panel, cx| {
            let Ok(Some(raw)) = task.await else { return };
            let _ = panel.update_in(cx, |panel, window, cx| {
                for (n, address) in decode_list(&raw, tm::CHAT_MAX) {
                    let key = InstanceKey {
                        holder: Holder::Page(owner),
                        n,
                    };
                    if !panel.chats.contains(key) {
                        panel.add_chat(owner, address, Some(n), window, cx);
                    }
                }
                panel.sync(window, cx);
                cx.notify();
            });
        });
    }

    /// Writes `owner`'s chats and their addresses (encrypted, local).
    fn save(&self, owner: Owner, cx: &App) {
        let list: Vec<(u8, Option<String>)> = self
            .chats
            .keys_of(Holder::Page(owner))
            .into_iter()
            .filter_map(|k| Some((k.n, self.chats.get(k)?.address.clone())))
            .collect();
        let value = (!list.is_empty()).then(|| encode_list(&list));
        let key = list_key(owner);
        self.db
            .run(cx, move |store| {
                store.set_local_value(&key, value.as_deref())
            })
            .detach();
    }

    /// The address the chat's page is on now, kept when it stays in the
    /// panel (a claude.ai page).
    fn read_address(&mut self, key: InstanceKey, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.get_mut(key) else {
            return;
        };
        let Some(web) = &chat.web else { return };
        let Ok(url) = web.view.read(cx).raw().url() else {
            return;
        };
        if chat::classify(&url) != Decision::InPanel || chat.address.as_deref() == Some(&url) {
            return;
        }
        chat.address = Some(url);
        if let Holder::Page(owner) = key.holder {
            self.save(owner, cx);
        }
    }

    /// The header menu is open: the shell counts it as an overlay.
    pub fn menu_open(&self) -> bool {
        self.menu
    }

    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shown = true;
        self.sync(window, cx);
        cx.notify();
    }

    pub fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shown = false;
        self.menu = false;
        self.tip = None;
        self.sync(window, cx);
        cx.notify();
    }

    /// Give the keyboard to the panel, and to the page when it is live.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.focus_web(cx);
    }

    /// Called by the shell while it renders: whether an overlay that can
    /// overlap the panel is open. Hiding returns the keyboard to GPUI.
    /// When the overlay closes and focus came back to the panel, the page
    /// gets the keyboard again.
    pub fn set_covered(&mut self, covered: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay != covered {
            self.overlay = covered;
            self.sync(window, cx);
        }
    }

    /// From the start of a view or tab drag to its drop or cancel, the web
    /// view is hidden with its own line (workbench.md 4.8.9).
    pub fn set_rearranging(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.rearranging = on;
        self.sync(window, cx);
        cx.notify();
    }

    /// Every chat's state and native view follow what is in view: the
    /// current page's selected chat when the view is on screen; every
    /// other one is hidden (it keeps running).
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.shown_key().filter(|_| self.shown);
        let covered = self.overlay || self.rearranging;
        let keys: Vec<InstanceKey> = self.chats.iter().map(|(k, _)| k).collect();
        let mut effects = Vec::new();
        for key in keys {
            let want = shown == Some(key);
            let Some(chat) = self.chats.get_mut(key) else {
                continue;
            };
            chat.state.set_covered(covered);
            if want && !chat.state.shown {
                effects.push((key, chat.state.show()));
            } else if !want && chat.state.shown {
                chat.state.hide();
            }
            if want {
                chat.last_used = Instant::now();
            }
            if let Some(web) = &chat.web {
                let visible = chat.state.web_visible();
                let view = web.view.read(cx);
                if view.visible() != visible
                    && let Err(err) = view.set_visible(visible)
                {
                    crate::log::line(&format!("chat: visibility change failed: {err}"));
                }
            }
        }
        for (key, effect) in effects {
            self.apply(key, effect, window, cx);
        }
        if shown.is_some() && !covered && self.focus.is_focused(window) {
            self.focus_web(cx);
        }
    }

    /// "Sign out and clear chat data", confirmed: every chat of every page
    /// goes back to Not started and the shared profile is deleted.
    pub fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.help = false;
        self.menu = false;
        self.consented = false;
        settings::change(cx, |prefs| prefs.chat_started = false);
        for (_, chat) in self.chats.iter_mut() {
            let _ = chat.state.sign_out();
            chat.web_task = Task::ready(());
            chat.generation += 1;
            if let Some(web) = chat.web.take() {
                let _ = web.view.read(cx).set_visible(false);
                drop(web);
            }
        }
        self.clear_profile(window, cx);
        self.sync(window, cx);
        cx.notify();
    }

    /// The GPUI window takes the keyboard back from the page before a kept
    /// key runs.
    pub fn release_keyboard(&self, cx: &App) {
        if let Some(web) = self
            .shown_key()
            .and_then(|k| self.chats.get(k))
            .and_then(|c| c.web.as_ref())
        {
            let _ = web.view.read(cx).raw().focus_parent();
        }
    }

    fn focus_web(&self, cx: &App) {
        if let Some(chat) = self.shown_key().and_then(|k| self.chats.get(k))
            && chat.state.web_visible()
            && let Some(web) = &chat.web
        {
            let _ = web.view.read(cx).raw().focus();
        }
    }

    /// Whether the current page can open another chat.
    pub fn can_open(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|o| !self.chats.at_limit(Holder::Page(o.owner)))
    }

    /// "Open claude.ai here" and "New chat here": a new chat of the
    /// current page on claude.ai's new-conversation page.
    pub fn open_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = self.current.as_ref().map(|o| o.owner) else {
            return;
        };
        let Some(key) = self.add_chat(owner, None, None, window, cx) else {
            return;
        };
        self.save(owner, cx);
        self.start(key, window, cx);
        self.sync(window, cx);
        cx.notify();
    }

    fn start(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.get_mut(key) else {
            return;
        };
        let effect = chat.state.start();
        self.apply(key, effect, window, cx);
        cx.notify();
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.shown_key() else { return };
        let Some(chat) = self.chats.get_mut(key) else {
            return;
        };
        let effect = chat.state.retry();
        self.apply(key, effect, window, cx);
        self.sync(window, cx);
        cx.notify();
    }

    /// The instance's `x`: its web view closes and it leaves the page.
    fn close_chat(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(chat) = self.chats.close(key)
            && let Some(web) = &chat.web
        {
            let _ = web.view.read(cx).set_visible(false);
        }
        if let Holder::Page(owner) = key.holder {
            self.save(owner, cx);
        }
        self.sync(window, cx);
        cx.notify();
    }

    fn select(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(current) = self.shown_key()
            && current != key
        {
            self.read_address(current, cx);
        }
        self.chats.select(key);
        self.sync(window, cx);
        cx.notify();
    }

    /// The open web views, for the shell's 10-minute and 6-view rules
    /// (layout.md 4.5.1, Resources).
    pub fn web_uses(&self) -> Vec<WebUse<InstanceKey>> {
        let shown = self.shown_key().filter(|_| self.shown);
        self.chats
            .iter()
            .map(|(key, chat)| WebUse {
                key,
                open: chat.web.is_some(),
                on_screen: shown == Some(key),
                last_used: chat.last_used,
                busy: chat.state.phase == chat::Phase::Starting,
            })
            .collect()
    }

    /// Closes the chat's web view, keeping its address: it reopens there
    /// when its page comes back.
    pub fn close_web(&mut self, key: InstanceKey, cx: &mut Context<Self>) {
        self.read_address(key, cx);
        let Some(chat) = self.chats.get_mut(key) else {
            return;
        };
        chat.web_task = Task::ready(());
        chat.generation += 1;
        if let Some(web) = chat.web.take() {
            let _ = web.view.read(cx).set_visible(false);
            drop(web);
            crate::log::line("chat: an idle web view was closed");
        }
        chat.state.idle_closed();
        cx.notify();
    }

    fn apply(
        &mut self,
        key: InstanceKey,
        effect: Effect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match effect {
            Effect::None => {}
            Effect::Build => {
                self.consented = true;
                settings::change(cx, |prefs| prefs.chat_started = true);
                for (_, chat) in self.chats.iter_mut() {
                    chat.state.consented = true;
                }
                if self.clearing {
                    self.build_after_clear.push(key);
                } else {
                    self.schedule_build(key, window, cx);
                }
            }
            Effect::Reload => {
                if let Some(chat) = self.chats.get(key)
                    && let Some(web) = &chat.web
                {
                    let raw = web.view.read(cx).raw();
                    let url = raw
                        .url()
                        .ok()
                        .filter(|u| chat::classify(u) == Decision::InPanel)
                        .unwrap_or_else(|| chat::HOME.to_owned());
                    let _ = raw.load_url(&url);
                }
            }
            // Signing out is the panel's, for every chat at once.
            Effect::DropAndClear => {}
        }
    }

    /// The starting surface paints first; the web view is created a few
    /// frames later (creating it blocks the UI thread for 200 to 300 ms).
    fn schedule_build(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let starting = cx.spawn(async move |panel, cx| {
            cx.background_executor().timer(STARTING_LINE).await;
            let _ = panel.update(cx, |panel, cx| {
                if let Some(chat) = panel.chats.get_mut(key) {
                    chat.state.starting_elapsed();
                }
                cx.notify();
            });
        });
        let build = cx.spawn_in(window, async move |panel, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let _ = panel.update_in(cx, |panel, window, cx| panel.build(key, window, cx));
        });
        if let Some(chat) = self.chats.get_mut(key) {
            chat.starting_task = starting;
            chat.build_task = build;
        }
    }

    fn build(&mut self, key: InstanceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.get_mut(key) else {
            return;
        };
        if chat.web.is_some() {
            chat.state.built(Built::Ok);
            return;
        }
        if let Err(err) = wry::webview_version() {
            crate::log::line(&format!("chat: no WebView2 runtime: {err}"));
            chat.state.built(Built::RuntimeMissing);
            return;
        }
        let Some(profile) = profile_dir() else {
            crate::log::line("chat: no data directory; the web view is not created");
            chat.state.built(Built::Failed);
            return;
        };
        let to_browser = chat.signals.clone();
        let new_window = chat.signals.clone();
        // Where it was last, else claude.ai's new conversation.
        let url = chat
            .address
            .clone()
            .filter(|a| chat::classify(a) == Decision::InPanel)
            .unwrap_or_else(|| chat::HOME.to_owned());
        let surface = gpui_kit::Rgba::from(cx.global::<Tokens>().bg_sunken);
        let channel = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
        let background = (
            channel(surface.r),
            channel(surface.g),
            channel(surface.b),
            255,
        );
        // Created after this update, outside it (crate::webview_build).
        chat.generation += 1;
        let generation = chat.generation;
        chat.web_task = crate::webview_build::build_later(
            window,
            cx,
            move |parent| {
                let mut context = Box::new(wry::WebContext::new(Some(profile)));
                let started = Instant::now();
                let view = crate::webview_build::build(
                    chat_builder(&mut context, &url, background, to_browser, new_window),
                    parent,
                )?;
                crate::log::line(&format!(
                    "chat: web view created in {} ms",
                    started.elapsed().as_millis()
                ));
                Ok((view, context))
            },
            move |panel, built, window, cx| {
                let current = panel.chats.get(key).is_some_and(|c| {
                    c.state.phase == chat::Phase::Starting && c.generation == generation
                });
                if !current {
                    // Signed out, closed, cleared or failed meanwhile: not kept.
                    if let Ok((view, _context)) = built {
                        let _ = view.set_visible(false);
                    }
                    return;
                }
                panel.built(key, built, window, cx);
                panel.sync(window, cx);
                cx.emit(ChatEvent::Opened);
                cx.notify();
            },
        );
    }

    /// The web view created, or not.
    fn built(
        &mut self,
        key: InstanceKey,
        built: wry::Result<(wry::WebView, Box<wry::WebContext>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let help_dismissed = settings::current(cx).chat_help_dismissed;
        let Some(chat) = self.chats.get_mut(key) else {
            return;
        };
        match built {
            Ok((view, context)) => {
                let view = gpui_webview::WebView::new(view, window, cx);
                // The view starts hidden; `sync` shows the one in view.
                let _ = view.set_visible(false);
                let hooks =
                    match crate::chat_windows::install(view.raw(), chat.signals.clone(), false) {
                        Ok(hooks) => Some(hooks),
                        Err(err) => {
                            // Without them the kept keys do not reach Acta while
                            // the page has the keyboard; the pointer still works.
                            crate::log::line(&format!("chat: WebView2 events not hooked: {err}"));
                            None
                        }
                    };
                chat.web = Some(Web {
                    _hooks: hooks,
                    view: cx.new(|_| view),
                    _context: context,
                });
                chat.state.built(Built::Ok);
                if !help_dismissed {
                    self.help = true;
                }
            }
            Err(err) => {
                crate::log::line(&format!("chat: the web view could not be created: {err}"));
                chat.state.built(Built::Failed);
            }
        }
    }

    /// Delete the profile, retrying while the web view's processes still
    /// hold its files. `chat_clear_pending` stays set until it is gone, so
    /// a deletion that never finished runs again at the next launch.
    fn clear_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dir) = profile_dir() else { return };
        settings::change(cx, |prefs| prefs.chat_clear_pending = true);
        self.clearing = true;
        self.clear_task = cx.spawn_in(window, async move |panel, cx| {
            let mut removed = false;
            for _ in 0..CLEAR_ATTEMPTS {
                let target = dir.clone();
                let result = cx
                    .background_spawn(async move { std::fs::remove_dir_all(&target) })
                    .await;
                match result {
                    Ok(()) => removed = true,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => removed = true,
                    Err(_) => {}
                }
                if removed {
                    break;
                }
                cx.background_executor().timer(CLEAR_PAUSE).await;
            }
            crate::log::line(if removed {
                "chat: profile deleted"
            } else {
                "chat: the profile could not be deleted; tried again at the next launch"
            });
            let _ = panel.update_in(cx, |panel, window, cx| {
                panel.clearing = false;
                if removed {
                    settings::change(cx, |prefs| prefs.chat_clear_pending = false);
                } else {
                    cx.emit(ChatEvent::ClearFailed);
                }
                for key in std::mem::take(&mut panel.build_after_clear) {
                    panel.schedule_build(key, window, cx);
                }
            });
        });
    }

    /// The header's context: the owner first, then "claude.ai".
    pub fn header_context(&self, cx: &App) -> Option<AnyElement> {
        let origin = self.current.as_ref()?;
        let t = cx.global::<Tokens>();
        let locale = settings::locale(cx);
        let owner = crate::terminal_dock::owner_label(locale, origin.owner, &origin.name);
        Some(
            type_scale::SMALL
                .el()
                .min_w_0()
                .truncate()
                .text_color(t.text_tertiary)
                .child(format!("{owner} \u{b7} claude.ai"))
                .into_any_element(),
        )
    }

    /// The instance strip (layout.md 4.5.1): from two chats on the page.
    fn strip(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let owner = self.current.as_ref()?.owner;
        let keys = self.chats.keys_of(Holder::Page(owner));
        if keys.len() < 2 {
            return None;
        }
        let locale = settings::locale(cx);
        let shown = self.shown_key();
        let tabs: Vec<AnyElement> = keys
            .into_iter()
            .map(|key| {
                let active = shown == Some(key);
                let id = format!("chat-inst-{}", key.n);
                let label = tr(
                    locale,
                    Key::ToolChatN,
                    &[("n", Arg::Count(u64::from(key.n)))],
                );
                crate::tool_strip::tab(id.clone(), None, label, active, t)
                    .on_click(
                        cx.listener(move |panel, _, window, cx| panel.select(key, window, cx)),
                    )
                    .child(
                        crate::tool_strip::close(
                            format!("chat-inst-close-{}", key.n),
                            id,
                            active,
                            t,
                        )
                        .on_click(cx.listener(
                            move |panel, _, window, cx| {
                                cx.stop_propagation();
                                panel.close_chat(key, window, cx)
                            },
                        )),
                    )
                    .into_any_element()
            })
            .collect();
        Some(crate::tool_strip::strip(tabs, t))
    }

    /// `Ctrl+PageDown` / `Ctrl+PageUp` from the view (not from the page,
    /// which keeps its keys).
    pub fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = self.current.as_ref().map(|o| o.owner) else {
            return;
        };
        if let Some(current) = self.shown_key() {
            self.read_address(current, cx);
        }
        self.chats.step(Holder::Page(owner), forward);
        self.sync(window, cx);
        cx.notify();
    }

    fn hover(&mut self, tip: Tip, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            if self.tip.is_some_and(|(t, shown)| t == tip && shown) {
                self.tips_warm_until = Some(Instant::now() + TOOLTIP_WARM);
            }
            if self.tip.is_some_and(|(t, _)| t == tip) {
                self.tip = None;
                self.tip_task = Task::ready(());
            }
            cx.notify();
            return;
        }
        let warm = self
            .tips_warm_until
            .is_some_and(|until| Instant::now() < until);
        self.tip = Some((tip, warm));
        if !warm {
            self.tip_task = cx.spawn(async move |panel, cx| {
                cx.background_executor().timer(TOOLTIP_DELAY).await;
                let _ = panel.update(cx, |panel, cx| {
                    if let Some((t, shown)) = &mut panel.tip
                        && *t == tip
                    {
                        *shown = true;
                        cx.notify();
                    }
                });
            });
        }
        cx.notify();
    }

    fn hover_listener(
        tip: Tip,
        cx: &mut Context<Self>,
    ) -> impl Fn(&bool, &mut Window, &mut App) + 'static + use<> {
        let panel = cx.entity().downgrade();
        move |hovered: &bool, _: &mut Window, cx: &mut App| {
            let _ = panel.update(cx, |panel, cx| panel.hover(tip, *hovered, cx));
        }
    }

    /// The tooltip of a header button: above it, right-aligned, 6 away,
    /// never down into the web view (components-app.md 5).
    fn tooltip(&self, tip: Tip, t: &Tokens, cx: &App) -> Option<AnyElement> {
        if self.tip != Some((tip, true)) || self.menu {
            return None;
        }
        let locale = settings::locale(cx);
        let (label, keys) = match tip {
            Tip::New if !self.can_open() => (
                tr(
                    locale,
                    Key::ToolLimit,
                    &[("n", Arg::Count(tm::CHAT_MAX as u64))],
                ),
                None,
            ),
            Tip::New => (text(locale, Key::ToolChatNew), None),
            Tip::Browser => (text(locale, Key::ChatOpenBrowser), None),
            Tip::Reload => (text(locale, Key::ChatReload), None),
            Tip::Menu => (text(locale, Key::ChatMore), None::<[&str; 3]>),
        };
        Some(
            deferred(
                anchored()
                    .position_mode(AnchoredPositionMode::Local)
                    .anchor(Anchor::BottomRight)
                    .position(point(px(24.), px(-6.)))
                    .child(
                        type_scale::SMALL
                            .el()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .px(px(8.))
                            .py(px(4.))
                            .whitespace_nowrap()
                            .bg(t.bg_overlay)
                            .border(px(1.))
                            .border_color(t.border_default)
                            .rounded(px(RADIUS_MD))
                            .shadow(vec![t.shadow_overlay()])
                            .text_color(t.text_primary)
                            .child(label)
                            .children(keys.map(|keys| ui::kbd(&keys, t))),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    fn render_menu(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let locale = settings::locale(cx);
        let close = |panel: &mut ChatPanel, cx: &mut Context<ChatPanel>| {
            panel.menu = false;
            cx.notify();
        };
        let mut panel = menu::panel("chat-menu-panel", t)
            .on_mouse_down_out(cx.listener(move |panel, _, _, cx| close(panel, cx)))
            .child(
                menu::item(
                    "chat-menu-help",
                    None,
                    text(locale, Key::ChatHelp),
                    None,
                    false,
                    t,
                )
                .on_click(cx.listener(move |panel, _, _, cx| {
                    close(panel, cx);
                    panel.help = true;
                })),
            )
            .child(
                menu::item(
                    "chat-menu-signout",
                    None,
                    text(locale, Key::ChatSignout),
                    None,
                    false,
                    t,
                )
                .on_click(cx.listener(move |panel, _, _, cx| {
                    close(panel, cx);
                    cx.emit(ChatEvent::SignOut);
                })),
            )
            .child(menu::separator(t));
        // The view menu (workbench.md 4.8.4): the current area's item
        // disabled with a check.
        let moves = [
            (Place::Side(Side::Left), Key::ViewMoveLeft),
            (Place::Side(Side::Right), Key::ViewMoveRight),
            (Place::Side(Side::Bottom), Key::ViewMoveBottom),
            (Place::Page, Key::ViewMovePage),
        ];
        for (i, (place, key)) in moves.into_iter().enumerate() {
            let here = self.place == place;
            panel = panel.child(if here {
                menu::disabled_item(
                    ("chat-menu-move", i),
                    text(locale, key),
                    Some(menu::check(t)),
                    t,
                )
            } else {
                menu::item(
                    ("chat-menu-move", i),
                    None,
                    text(locale, key),
                    None,
                    false,
                    t,
                )
                .on_click(cx.listener(move |panel, _, _, cx| {
                    close(panel, cx);
                    cx.emit(ChatEvent::View(ViewAction::MoveTo(place)));
                }))
            });
        }
        let split = text(locale, Key::ViewSplitOff);
        panel = panel.child(if self.can_split {
            menu::item("chat-menu-split", None, split, None, false, t).on_click(cx.listener(
                move |panel, _, _, cx| {
                    close(panel, cx);
                    cx.emit(ChatEvent::View(ViewAction::SplitOff));
                },
            ))
        } else {
            menu::disabled_item("chat-menu-split", split, None, t)
        });
        panel = panel.child(menu::separator(t)).child(
            menu::item(
                "chat-menu-hide",
                None,
                text(locale, Key::ViewHide),
                None,
                false,
                t,
            )
            .on_click(cx.listener(move |panel, _, _, cx| {
                close(panel, cx);
                cx.emit(ChatEvent::View(ViewAction::Hide));
            })),
        );
        self.menu_anchor
            .below(panel, crate::menu::Align::End)
            .unwrap_or_else(|| div().into_any_element())
    }

    /// The chat's own header buttons, wherever its area header or view
    /// strip is (workbench.md 4.8.4): New chat here, Open in browser,
    /// Reload, and its menu, which holds the view menu after its own items.
    pub fn header_buttons(
        &mut self,
        place: Place,
        can_split: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.place = place;
        self.can_split = can_split;
        let t = cx.global::<Tokens>().clone();
        let can_reload = self
            .shown_key()
            .and_then(|k| self.chats.get(k))
            .is_some_and(|c| c.state.has_web);
        let can_new = self.can_open();
        let menu = self.menu.then(|| self.render_menu(&t, cx));
        let tip = |this: &Self, tip: Tip, cx: &App| this.tooltip(tip, &t, cx);
        let stop =
            |_: &gpui_kit::MouseDownEvent, _: &mut Window, cx: &mut App| cx.stop_propagation();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .when(self.current.is_some(), |this| {
                this.child(
                    ui::ghost_icon_button("chat-new", Icon::Plus, 24., false, !can_new, true, &t)
                        .on_hover(Self::hover_listener(Tip::New, cx))
                        .on_mouse_down(gpui_kit::MouseButton::Left, stop)
                        .when(can_new, |this| {
                            this.on_click(
                                cx.listener(|panel, _, window, cx| panel.open_new(window, cx)),
                            )
                        })
                        .children(tip(self, Tip::New, cx)),
                )
            })
            .child(
                ui::ghost_icon_button(
                    "chat-browser",
                    Icon::ExternalLink,
                    24.,
                    false,
                    false,
                    true,
                    &t,
                )
                .on_hover(Self::hover_listener(Tip::Browser, cx))
                .on_mouse_down(gpui_kit::MouseButton::Left, stop)
                .on_click(|_, _, cx| cx.open_url(chat::HOME))
                .children(tip(self, Tip::Browser, cx)),
            )
            .child(
                ui::ghost_icon_button(
                    "chat-reload",
                    Icon::RotateCw,
                    24.,
                    false,
                    !can_reload,
                    true,
                    &t,
                )
                .on_hover(Self::hover_listener(Tip::Reload, cx))
                .on_mouse_down(gpui_kit::MouseButton::Left, stop)
                .when(can_reload, |this| {
                    this.on_click(cx.listener(|panel, _, window, cx| panel.retry(window, cx)))
                })
                .children(tip(self, Tip::Reload, cx)),
            )
            .child(
                div()
                    .relative()
                    .child(self.menu_anchor.measure())
                    .child(
                        ui::ghost_icon_button(
                            "chat-menu",
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
                        .children(tip(self, Tip::Menu, cx)),
                    )
                    .children(menu),
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
        let browser = |id: &'static str, key: Key, variant: Variant| {
            ui::button(id, text(locale, key), variant, false, &t)
                .on_click(|_, _, cx| cx.open_url(chat::HOME))
        };
        let buttons = || div().mt(px(24.)).flex().flex_row().flex_wrap().gap(px(8.));
        // A page that owns nothing has no chat (layout.md 4.5.1).
        if self.current.is_none() || !self.shown {
            return div().into_any_element();
        }
        let surface = match self.shown_key().and_then(|k| self.chats.get(k)) {
            Some(chat) => chat.state.surface(),
            // The page has no chat yet: Not started, whose button opens
            // Chat 1 of this page.
            None => Surface::NotStarted,
        };
        match surface {
            Surface::Closed => div().into_any_element(),
            Surface::NotStarted => empty()
                .child(heading(Key::ChatStartTitle))
                .child(reading(Key::ChatStartBody))
                .child(
                    buttons()
                        .child(
                            ui::button(
                                "chat-start",
                                text(locale, Key::ChatStartHere),
                                Variant::Primary,
                                false,
                                &t,
                            )
                            // What it costs is read once: the tooltip.
                            .tooltip(ui::tip(text(locale, Key::ChatStartMemory)))
                            .on_click(cx.listener(
                                |panel, _, window, cx| match panel.shown_key() {
                                    Some(key) => panel.start(key, window, cx),
                                    None => panel.open_new(window, cx),
                                },
                            )),
                        )
                        .child(browser(
                            "chat-start-browser",
                            Key::ChatStartBrowser,
                            Variant::Ghost,
                        )),
                )
                .into_any_element(),
            Surface::Starting { line } => empty()
                .when(line, |this| this.child(small(Key::ChatStarting)))
                .into_any_element(),
            Surface::Covered => empty()
                .child(small(if self.rearranging {
                    Key::LayoutWebviewHidden
                } else {
                    Key::ChatHidden
                }))
                .into_any_element(),
            Surface::RuntimeMissing => empty()
                .child(heading(Key::ChatRuntimeTitle))
                .child(reading(Key::ChatRuntimeBody))
                .child(
                    buttons()
                        .child(browser(
                            "chat-runtime-browser",
                            Key::ChatBrowser,
                            Variant::Primary,
                        ))
                        .child(
                            ui::button(
                                "chat-runtime-get",
                                text(locale, Key::ChatRuntimeGet),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(|_, _, cx| cx.open_url(chat::WEBVIEW2_DOWNLOAD)),
                        )
                        .child(
                            ui::button(
                                "chat-runtime-retry",
                                text(locale, Key::CommonTryAgain),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|panel, _, window, cx| panel.retry(window, cx))),
                        ),
                )
                .into_any_element(),
            Surface::Failed(reason) => empty()
                .child(heading(Key::ChatFailedTitle))
                .when(reason == Reason::Offline, |this| {
                    this.child(reading(Key::ChatFailedOffline))
                })
                .child(
                    buttons()
                        .child(
                            ui::button(
                                "chat-failed-retry",
                                text(locale, Key::CommonTryAgain),
                                Variant::Primary,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(|panel, _, window, cx| panel.retry(window, cx))),
                        )
                        .child(browser(
                            "chat-failed-browser",
                            Key::ChatBrowser,
                            Variant::Ghost,
                        )),
                )
                .into_any_element(),
            Surface::Live => {
                let help = self.help.then(|| {
                    div().p(px(8.)).child(
                        ui::notice(
                            Some((Icon::Info, t.text_secondary)),
                            text(locale, Key::ChatSignin),
                            &t,
                        )
                        .child(
                            ui::button(
                                "chat-help-ok",
                                text(locale, Key::CommonGotIt),
                                Variant::Ghost,
                                false,
                                &t,
                            )
                            .on_click(cx.listener(
                                |panel, _, _, cx| {
                                    panel.help = false;
                                    settings::change(cx, |prefs| prefs.chat_help_dismissed = true);
                                    cx.notify();
                                },
                            )),
                        ),
                    )
                });
                let page = self
                    .shown_key()
                    .and_then(|k| self.chats.get(k))
                    .and_then(|c| c.web.as_ref())
                    .map(|web| web.view.clone().into_any_element());
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .children(help)
                    .child(div().flex_1().min_h_0().children(page))
                    .into_any_element()
            }
        }
    }
}

impl Render for ChatPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let strip = self.strip(&t, cx);
        let body = self.body(cx);
        div()
            .id("chat-dock")
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

/// The chat's web view: claude.ai at `url`, the allow-list, new windows
/// and downloads to the system browser.
fn chat_builder<'a>(
    context: &'a mut wry::WebContext,
    url: &str,
    background: (u8, u8, u8, u8),
    to_browser: Sender<Signal>,
    new_window: Sender<Signal>,
) -> wry::WebViewBuilder<'a> {
    wry::WebViewBuilder::new_with_web_context(context)
        .with_url(url)
        .with_background_color(background)
        .with_devtools(false)
        .with_navigation_handler(move |url| match chat::classify(&url) {
            Decision::InPanel => true,
            Decision::Browser => {
                let _ = to_browser.try_send(Signal::OpenInBrowser(url));
                false
            }
            Decision::Block => {
                crate::log::line(&format!(
                    "chat: navigation dropped: {}",
                    chat::for_log(&url)
                ));
                false
            }
        })
        .with_new_window_req_handler(move |url, _| {
            if chat::new_window(&url) == Decision::Browser {
                let _ = new_window.try_send(Signal::OpenInBrowser(url));
            }
            wry::NewWindowResponse::Deny
        })
        .with_download_started_handler(|_, _| false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_keeps_its_chats_and_their_addresses() {
        let list = vec![
            (1, Some("https://claude.ai/chat/0b6c".to_owned())),
            (3, None),
        ];
        let raw = encode_list(&list);
        assert_eq!(decode_list(&raw, tm::CHAT_MAX), list);
        // Only claude.ai addresses come back; numbers stay unique and
        // within the limit; anything unreadable is nothing.
        let raw = r#"[[1,"https://evil.example/"],[1,null],[0,null],[2,"https://claude.ai/new"],[3,null],[4,null]]"#;
        assert_eq!(
            decode_list(raw, tm::CHAT_MAX),
            [
                (1, None),
                (2, Some("https://claude.ai/new".to_owned())),
                (3, None)
            ]
        );
        assert!(decode_list("not json", tm::CHAT_MAX).is_empty());
    }

    #[test]
    fn every_chat_shares_one_profile() {
        // One user data folder for every chat of every page: one cookie
        // store, one sign-in (layout.md 4.5.1). The agent browser's is
        // another folder (ADR 0010, decision 2).
        let chat = profile_dir();
        assert!(chat.is_some_and(|p| p.ends_with("webview")));
        assert_ne!(profile_dir(), crate::browser_host::profile_dir());
    }
}
