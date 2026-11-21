//! The chat tool window (ADR 0009; docs/design/components-app.md 5, Chat
//! panel; screens.md 8.13): claude.ai in the system web view, in the right
//! or the bottom dock.
//!
//! The web view is created the first time the user asks for it and lives on
//! while the panel is hidden. It is a native view above everything GPUI
//! draws, so it is hidden whenever the panel is not the surface: panel
//! closed, an overlay open, any state but the live page. The page is never
//! scripted, read or styled: no initialization script, no IPC, no custom
//! protocol, no devtools, the stock user agent.
//!
//! On Linux there is no panel (no web view is linked): the entity exists so
//! the shell has one code path, and showing it opens claude.ai in the
//! browser.

use async_channel::{Receiver, Sender};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Task, Window, anchored, deferred, div, point, px,
};

use crate::chat::{self, ChatState, Effect, Kept, Reason, Surface};
use crate::i18n::{Key, text};
use crate::icons::Icon;
use crate::menu;
use crate::settings;
use crate::theme::Tokens;
use crate::type_scale;
use crate::ui::{self, Variant};

/// docs/design/layout.md 4.5: the right dock is 648 wide, the bottom one
/// 320 high.
const WIDTH: f32 = 648.;
const HEIGHT: f32 = 320.;
const HEADER: f32 = 28.;
/// components-app.md 5: the starting line appears after 400 ms.
const STARTING_LINE: std::time::Duration = std::time::Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dock {
    Right,
    Bottom,
}

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
    /// Hide was asked from the header.
    Hide,
    /// A kept key, to run as if GPUI had received it.
    Kept(Kept),
    /// "Move chat to bottom" / "Move chat to right" from the header menu.
    Move(Dock),
    /// "Sign out and clear chat data..." from the header menu: the shell
    /// asks for confirmation.
    SignOut,
}

/// The live web view and what it needs to stay alive. Fields drop in
/// order: the event handlers first, the context last.
#[cfg(any(windows, target_os = "macos"))]
struct Web {
    #[cfg(windows)]
    _hooks: Option<crate::chat_windows::Hooks>,
    view: gpui_kit::Entity<gpui_webview::WebView>,
    _context: Box<wry::WebContext>,
}

pub struct ChatPanel {
    state: ChatState,
    pub dock: Dock,
    focus: FocusHandle,
    /// The sign-in help notice is shown.
    help: bool,
    /// The header's menu is open (it would fall under the web view).
    menu: bool,
    signals: Sender<Signal>,
    /// The profile is being deleted; a start waits for it.
    clearing: bool,
    build_after_clear: bool,
    #[cfg(any(windows, target_os = "macos"))]
    web: Option<Web>,
    /// macOS keeps the site data in the system's store, not in the profile
    /// directory: a sign-out without a web view clears it on the next start.
    #[cfg(target_os = "macos")]
    clear_on_build: bool,
    _tasks: Vec<Task<()>>,
    _activation: Option<gpui_kit::Subscription>,
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
/// WebView2 would otherwise put its profile next to the executable.
pub fn profile_dir() -> Option<std::path::PathBuf> {
    use acta_core::paths::{BaseDirs, Os, data_dir};
    Some(data_dir(Os::CURRENT, &BaseDirs::from_env())?.join("webview"))
}

impl ChatPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = settings::current(cx);
        let panel = cfg!(any(windows, target_os = "macos"));
        let state = ChatState::new(panel, prefs.chat_started);
        let dock = if prefs.chat_bottom {
            Dock::Bottom
        } else {
            Dock::Right
        };
        let (signals, receiver) = async_channel::unbounded();
        let task = Self::listen(receiver, window, cx);
        // After the window was in the background, the keyboard comes back to
        // GPUI; give it to the page again when the panel had it.
        let activation = cx.observe_window_activation(window, |panel, window, cx| {
            if window.is_window_active() && panel.focus.is_focused(window) {
                panel.focus_web(cx);
            }
        });
        Self {
            state,
            dock,
            focus: cx.focus_handle(),
            help: false,
            menu: false,
            signals,
            clearing: false,
            build_after_clear: false,
            #[cfg(any(windows, target_os = "macos"))]
            web: None,
            #[cfg(target_os = "macos")]
            clear_on_build: false,
            _tasks: vec![task],
            _activation: Some(activation),
        }
    }

    fn listen(receiver: Receiver<Signal>, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |panel, cx| {
            while let Ok(signal) = receiver.recv().await {
                if panel
                    .update_in(cx, |panel, window, cx| panel.on_signal(signal, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    fn on_signal(&mut self, signal: Signal, window: &mut Window, cx: &mut Context<Self>) {
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
                self.state.load_failed(reason);
                self.sync(cx);
                cx.notify();
            }
        }
    }

    pub fn is_shown(&self) -> bool {
        self.state.shown && self.state.panel
    }

    /// The header menu is open: the shell counts it as an overlay.
    pub fn menu_open(&self) -> bool {
        self.menu
    }

    /// Show the panel (on Linux: open claude.ai in the browser).
    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.state.show();
        self.apply(effect, window, cx);
        self.sync(cx);
        cx.notify();
    }

    pub fn hide(&mut self, cx: &mut Context<Self>) {
        self.state.hide();
        self.menu = false;
        self.sync(cx);
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
    pub fn set_covered(&mut self, covered: bool, window: &Window, cx: &mut Context<Self>) {
        if self.state.covered != covered {
            self.state.set_covered(covered);
            self.sync(cx);
            if !covered && self.focus.is_focused(window) {
                self.focus_web(cx);
            }
        }
    }

    pub fn move_to(&mut self, dock: Dock, cx: &mut Context<Self>) {
        self.dock = dock;
        settings::change(cx, |prefs| prefs.chat_bottom = dock == Dock::Bottom);
        cx.notify();
    }

    /// "Sign out and clear chat data", confirmed.
    pub fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.state.sign_out();
        self.help = false;
        self.menu = false;
        settings::change(cx, |prefs| prefs.chat_started = false);
        self.apply(effect, window, cx);
        self.sync(cx);
        cx.notify();
    }

    /// The page takes the keyboard back from GPUI before a kept key runs.
    pub fn release_keyboard(&self, cx: &App) {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some(web) = &self.web {
            let _ = web.view.read(cx).raw().focus_parent();
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = cx;
    }

    fn focus_web(&self, cx: &App) {
        #[cfg(any(windows, target_os = "macos"))]
        if self.state.web_visible()
            && let Some(web) = &self.web
        {
            let _ = web.view.read(cx).raw().focus();
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = cx;
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.state.start();
        self.apply(effect, window, cx);
        cx.notify();
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.state.retry();
        self.apply(effect, window, cx);
        self.sync(cx);
        cx.notify();
    }

    fn apply(&mut self, effect: Effect, window: &mut Window, cx: &mut Context<Self>) {
        match effect {
            Effect::None => {}
            Effect::OpenInBrowser(url) => cx.open_url(url),
            Effect::Build => {
                if self.state.consented {
                    settings::change(cx, |prefs| prefs.chat_started = true);
                }
                if self.clearing {
                    self.build_after_clear = true;
                } else {
                    self.schedule_build(window, cx);
                }
            }
            Effect::Reload =>
            {
                #[cfg(any(windows, target_os = "macos"))]
                if let Some(web) = &self.web {
                    let _ = web.view.read(cx).raw().load_url(chat::HOME);
                }
            }
            Effect::DropAndClear => self.drop_and_clear(window, cx),
        }
    }

    /// The starting surface paints first; the web view is created a few
    /// frames later (creating it blocks the UI thread for 200 to 300 ms).
    fn schedule_build(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line = cx.spawn(async move |panel, cx| {
            cx.background_executor().timer(STARTING_LINE).await;
            let _ = panel.update(cx, |panel, cx| {
                panel.state.starting_elapsed();
                cx.notify();
            });
        });
        let build = cx.spawn_in(window, async move |panel, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(50))
                .await;
            let _ = panel.update_in(cx, |panel, window, cx| {
                panel.build(window, cx);
                panel.sync(cx);
                cx.notify();
            });
        });
        self._tasks.push(line);
        self._tasks.push(build);
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn build(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use chat::{Built, Decision};

        if self.web.is_some() {
            self.state.built(Built::Ok);
            return;
        }
        #[cfg(windows)]
        if let Err(err) = wry::webview_version() {
            crate::log::line(&format!("chat: no WebView2 runtime: {err}"));
            self.state.built(Built::RuntimeMissing);
            return;
        }
        let Some(profile) = profile_dir() else {
            crate::log::line("chat: no data directory; the web view is not created");
            self.state.built(Built::Failed);
            return;
        };
        #[cfg(target_os = "macos")]
        let clear_first = std::mem::take(&mut self.clear_on_build);
        #[cfg(not(target_os = "macos"))]
        let clear_first = false;

        let mut context = Box::new(wry::WebContext::new(Some(profile)));
        let to_browser = self.signals.clone();
        let new_window = self.signals.clone();
        let surface = gpui_kit::Rgba::from(cx.global::<Tokens>().bg_sunken);
        let channel = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
        let mut builder = wry::WebViewBuilder::new_with_web_context(&mut context)
            .with_background_color((
                channel(surface.r),
                channel(surface.g),
                channel(surface.b),
                255,
            ))
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
            .with_download_started_handler(|_, _| false);
        if !clear_first {
            builder = builder.with_url(chat::HOME);
        }
        let started = std::time::Instant::now();
        match gpui_webview::WebView::build(builder, window, cx) {
            Ok(view) => {
                crate::log::line(&format!(
                    "chat: web view created in {} ms",
                    started.elapsed().as_millis()
                ));
                if clear_first {
                    let _ = view.raw().clear_all_browsing_data();
                    let _ = view.raw().load_url(chat::HOME);
                }
                #[cfg(windows)]
                let hooks = match crate::chat_windows::install(view.raw(), self.signals.clone()) {
                    Ok(hooks) => Some(hooks),
                    Err(err) => {
                        // Without them the kept keys do not reach Acta while
                        // the page has the keyboard; the pointer still works.
                        crate::log::line(&format!("chat: WebView2 events not hooked: {err}"));
                        None
                    }
                };
                self.web = Some(Web {
                    #[cfg(windows)]
                    _hooks: hooks,
                    view: cx.new(|_| view),
                    _context: context,
                });
                self.state.built(Built::Ok);
                if !settings::current(cx).chat_help_dismissed {
                    self.help = true;
                }
            }
            Err(err) => {
                crate::log::line(&format!("chat: the web view could not be created: {err}"));
                self.state.built(Built::Failed);
            }
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn build(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.state.built(chat::Built::Failed);
    }

    /// Destroy the web view, then delete its profile once its processes
    /// have let go of the files.
    fn drop_and_clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some(web) = self.web.take() {
            let _ = web.view.read(cx).raw().clear_all_browsing_data();
            let _ = web.view.read(cx).set_visible(false);
            drop(web);
        } else {
            #[cfg(target_os = "macos")]
            {
                self.clear_on_build = true;
            }
        }
        let Some(dir) = profile_dir() else { return };
        self.clearing = true;
        let task = cx.spawn_in(window, async move |panel, cx| {
            let mut removed = false;
            for _ in 0..40 {
                let target = dir.clone();
                let result = cx
                    .background_spawn(async move { std::fs::remove_dir_all(&target) })
                    .await;
                match result {
                    Ok(()) => {
                        removed = true;
                        break;
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        removed = true;
                        break;
                    }
                    // The web view's processes still hold files.
                    Err(_) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(250))
                            .await
                    }
                }
            }
            crate::log::line(if removed {
                "chat: profile deleted"
            } else {
                "chat: the profile could not be deleted"
            });
            let _ = panel.update_in(cx, |panel, window, cx| {
                panel.clearing = false;
                if std::mem::take(&mut panel.build_after_clear) {
                    panel.schedule_build(window, cx);
                }
            });
        });
        self._tasks.push(task);
    }

    /// The native view's visibility follows the state: the element alone
    /// cannot hide it, since a view that is not painted stays where it was.
    fn sync(&mut self, cx: &mut Context<Self>) {
        #[cfg(any(windows, target_os = "macos"))]
        if let Some(web) = &self.web {
            let visible = self.state.web_visible();
            let view = web.view.read(cx);
            if view.visible() != visible
                && let Err(err) = view.set_visible(visible)
            {
                crate::log::line(&format!("chat: visibility change failed: {err}"));
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = cx;
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = cx.global::<Tokens>().clone();
        let locale = settings::locale(cx);
        let can_reload = self.state.has_web;
        let menu = self.menu.then(|| self.render_menu(&t, cx));
        div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .h(px(HEADER))
            .px(px(8.))
            .bg(t.bg_sunken)
            .border_b_1()
            .border_color(t.border_subtle)
            .child(ui::label(text(locale, Key::ChatTitle), &t))
            .child(
                type_scale::SMALL
                    .el()
                    .text_color(t.text_tertiary)
                    .child("claude.ai"),
            )
            .child(div().flex_1())
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
                .on_click(|_, _, cx| cx.open_url(chat::HOME)),
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
                .when(can_reload, |this| {
                    this.on_click(cx.listener(|panel, _, window, cx| panel.retry(window, cx)))
                }),
            )
            .child(
                div()
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
                        .on_click(cx.listener(|panel, _, _, cx| {
                            panel.menu = !panel.menu;
                            cx.notify();
                        })),
                    )
                    .children(menu),
            )
            .child(
                ui::ghost_icon_button("chat-hide", Icon::Minus, 24., false, false, true, &t)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ChatEvent::Hide))),
            )
    }

    fn render_menu(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let locale = settings::locale(cx);
        let (move_label, target) = match self.dock {
            Dock::Right => (Key::CmdChatBottom, Dock::Bottom),
            Dock::Bottom => (Key::CmdChatRight, Dock::Right),
        };
        let close = |panel: &mut ChatPanel, cx: &mut Context<ChatPanel>| {
            panel.menu = false;
            cx.notify();
        };
        deferred(
            anchored().offset(point(px(-176.), px(28.))).child(
                menu::panel("chat-menu-panel", t)
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
                            "chat-menu-move",
                            None,
                            text(locale, move_label),
                            None,
                            false,
                            t,
                        )
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            close(panel, cx);
                            cx.emit(ChatEvent::Move(target));
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
                    ),
            ),
        )
        .with_priority(1)
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
        match self.state.surface() {
            Surface::Closed => div().into_any_element(),
            Surface::NotStarted => empty()
                .child(heading(Key::ChatStartTitle))
                .child(reading(Key::ChatStartBody))
                .child(small(Key::ChatStartMemory).mt(px(8.)))
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
                            .on_click(cx.listener(|panel, _, window, cx| panel.start(window, cx))),
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
            Surface::Covered => empty().child(small(Key::ChatHidden)).into_any_element(),
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
                let page = self.live_view();
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

    fn live_view(&self) -> Option<AnyElement> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            self.web
                .as_ref()
                .map(|web| web.view.clone().into_any_element())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            None
        }
    }
}

impl Render for ChatPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        let header = self.header(cx);
        let body = self.body(cx);
        div()
            .id("chat-dock")
            .track_focus(&self.focus)
            // Keys that reach GPUI while the page is live are the page's,
            // except the kept ones (shell.rs bindings, interaction.md 6.1).
            .when(self.state.web_visible(), |this| this.key_context("ChatWeb"))
            .flex()
            .flex_col()
            .flex_shrink_0()
            .bg(t.bg_sunken)
            .border_color(t.border_subtle)
            .map(|this| match self.dock {
                Dock::Right => this.w(px(WIDTH)).min_w(px(400.)).h_full().border_l_1(),
                Dock::Bottom => this.h(px(HEIGHT)).min_h(px(160.)).w_full().border_t_1(),
            })
            .child(header)
            .child(div().flex().flex_col().flex_1().min_h_0().child(body))
    }
}
