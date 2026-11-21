//! The Chat view (ADR 0009; docs/design/components-app.md 5, Chat panel;
//! screens.md 8.13; workbench.md 4.8): claude.ai in the system web view,
//! in whatever area the layout puts it. Windows only; macOS and Linux open claude.ai in the
//! browser instead (ADR 0009, changelog 2026-11-21).
//!
//! The web view is created the first time the user asks for it and lives on
//! while the panel is hidden. It is a native view above everything GPUI
//! draws, so it is hidden whenever the panel is not the surface: panel
//! closed, an overlay open, any state but the live page. Acta never reads
//! from or writes to the page: no script of its own, no IPC handler, no
//! custom protocol, no devtools, the stock user agent. (wry itself always
//! adds a `window.ipc.postMessage` shim to every page; with no handler
//! attached, what the page posts is dropped.)

use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Anchor, AnchoredPositionMode, AnyElement, App, AppContext as _, Context, Entity, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Task, Window, anchored, deferred, div, point, px,
};

use crate::chat::{self, Built, ChatState, Decision, Effect, Kept, Reason, Surface};
use crate::i18n::{Key, text};
use crate::icons::Icon;
use crate::layout::{Place, Side, ViewAction};
use crate::menu;
use crate::settings;
use crate::theme::{RADIUS_MD, Tokens};
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
}

/// A header button with a tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tip {
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

pub struct ChatPanel {
    state: ChatState,
    focus: FocusHandle,
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
    signals: Sender<Signal>,
    web: Option<Web>,
    /// The profile is being deleted; a start waits for it.
    clearing: bool,
    build_after_clear: bool,
    /// The hovered header button, and whether its tooltip shows yet.
    tip: Option<(Tip, bool)>,
    /// Until when a neighbour's tooltip shows without delay.
    tips_warm_until: Option<Instant>,
    // One task per job: a new one replaces (and cancels) the previous.
    _listen: Task<()>,
    starting_task: Task<()>,
    build_task: Task<()>,
    /// The web view being created (crate::webview_build).
    web_task: Task<()>,
    clear_task: Task<()>,
    tip_task: Task<()>,
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
/// WebView2 would otherwise put its profile next to the executable.
fn profile_dir() -> Option<std::path::PathBuf> {
    use acta_core::paths::{BaseDirs, Os, data_dir};
    Some(data_dir(Os::CURRENT, &BaseDirs::from_env())?.join("webview"))
}

impl ChatPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = settings::current(cx);
        let state = ChatState::new(prefs.chat_started);
        let clear_pending = prefs.chat_clear_pending;
        let (signals, receiver) = async_channel::unbounded();
        let listen = Self::listen(receiver, window, cx);
        // After the window was in the background, the keyboard comes back to
        // GPUI; give it to the page again when the panel had it.
        let activation = cx.observe_window_activation(window, |panel, window, cx| {
            if window.is_window_active() && panel.focus.is_focused(window) {
                panel.focus_web(cx);
            }
        });
        let mut panel = Self {
            state,
            focus: cx.focus_handle(),
            overlay: false,
            rearranging: false,
            place: Place::Side(Side::Right),
            can_split: false,
            help: false,
            menu: false,
            signals,
            web: None,
            clearing: false,
            build_after_clear: false,
            tip: None,
            tips_warm_until: None,
            _listen: listen,
            starting_task: Task::ready(()),
            build_task: Task::ready(()),
            web_task: Task::ready(()),
            clear_task: Task::ready(()),
            tip_task: Task::ready(()),
            _activation: activation,
        };
        // A sign-out whose deletion did not finish: before any web view.
        if clear_pending {
            panel.clear_profile(window, cx);
        }
        panel
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

    /// The header menu is open: the shell counts it as an overlay.
    pub fn menu_open(&self) -> bool {
        self.menu
    }

    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let effect = self.state.show();
        self.apply(effect, window, cx);
        self.sync(cx);
        cx.notify();
    }

    pub fn hide(&mut self, cx: &mut Context<Self>) {
        self.state.hide();
        self.menu = false;
        self.tip = None;
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
        self.overlay = covered;
        self.apply_covered(window, cx);
    }

    /// From the start of a view or tab drag to its drop or cancel, the web
    /// view is hidden with its own line (workbench.md 4.8.9).
    pub fn set_rearranging(&mut self, on: bool, window: &Window, cx: &mut Context<Self>) {
        self.rearranging = on;
        self.apply_covered(window, cx);
        cx.notify();
    }

    fn apply_covered(&mut self, window: &Window, cx: &mut Context<Self>) {
        let covered = self.overlay || self.rearranging;
        if self.state.covered != covered {
            self.state.set_covered(covered);
            self.sync(cx);
            if !covered && self.focus.is_focused(window) {
                self.focus_web(cx);
            }
        }
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

    /// The GPUI window takes the keyboard back from the page before a kept
    /// key runs.
    pub fn release_keyboard(&self, cx: &App) {
        if let Some(web) = &self.web {
            let _ = web.view.read(cx).raw().focus_parent();
        }
    }

    fn focus_web(&self, cx: &App) {
        if self.state.web_visible()
            && let Some(web) = &self.web
        {
            let _ = web.view.read(cx).raw().focus();
        }
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
            Effect::Build => {
                settings::change(cx, |prefs| prefs.chat_started = true);
                if self.clearing {
                    self.build_after_clear = true;
                } else {
                    self.schedule_build(window, cx);
                }
            }
            Effect::Reload => {
                if let Some(web) = &self.web {
                    let _ = web.view.read(cx).raw().load_url(chat::HOME);
                }
            }
            Effect::DropAndClear => {
                if let Some(web) = self.web.take() {
                    let _ = web.view.read(cx).set_visible(false);
                    drop(web);
                }
                self.clear_profile(window, cx);
            }
        }
    }

    /// The starting surface paints first; the web view is created a few
    /// frames later (creating it blocks the UI thread for 200 to 300 ms).
    fn schedule_build(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.starting_task = cx.spawn(async move |panel, cx| {
            cx.background_executor().timer(STARTING_LINE).await;
            let _ = panel.update(cx, |panel, cx| {
                panel.state.starting_elapsed();
                cx.notify();
            });
        });
        self.build_task = cx.spawn_in(window, async move |panel, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let _ = panel.update_in(cx, |panel, window, cx| panel.build(window, cx));
        });
    }

    fn build(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.web.is_some() {
            self.state.built(Built::Ok);
            return;
        }
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
        let to_browser = self.signals.clone();
        let new_window = self.signals.clone();
        let surface = gpui_kit::Rgba::from(cx.global::<Tokens>().bg_sunken);
        let channel = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
        let background = (
            channel(surface.r),
            channel(surface.g),
            channel(surface.b),
            255,
        );
        // Created after this update, outside it (crate::webview_build).
        self.web_task = crate::webview_build::build_later(
            window,
            cx,
            move |parent| {
                let mut context = Box::new(wry::WebContext::new(Some(profile)));
                let started = Instant::now();
                let view = crate::webview_build::build(
                    chat_builder(&mut context, background, to_browser, new_window),
                    parent,
                )?;
                crate::log::line(&format!(
                    "chat: web view created in {} ms",
                    started.elapsed().as_millis()
                ));
                Ok((view, context))
            },
            |panel, built, window, cx| {
                panel.built(built, window, cx);
                panel.sync(cx);
                cx.notify();
            },
        );
    }

    /// The web view created, or not.
    fn built(
        &mut self,
        built: wry::Result<(wry::WebView, Box<wry::WebContext>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match built {
            Ok((view, context)) => {
                let view = gpui_webview::WebView::new(view, window, cx);
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
                if std::mem::take(&mut panel.build_after_clear) {
                    panel.schedule_build(window, cx);
                }
            });
        });
    }

    /// The native view's visibility follows the state: the element alone
    /// cannot hide it, since a view that is not painted stays where it was.
    fn sync(&mut self, cx: &mut Context<Self>) {
        if let Some(web) = &self.web {
            let visible = self.state.web_visible();
            let view = web.view.read(cx);
            if view.visible() != visible
                && let Err(err) = view.set_visible(visible)
            {
                crate::log::line(&format!("chat: visibility change failed: {err}"));
            }
        }
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
        let (key, keys) = match tip {
            Tip::Browser => (Key::ChatOpenBrowser, None),
            Tip::Reload => (Key::ChatReload, None),
            Tip::Menu => (Key::ChatMore, None::<[&str; 3]>),
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
                            .border_1()
                            .border_color(t.border_default)
                            .rounded(px(RADIUS_MD))
                            .shadow(vec![t.shadow_overlay()])
                            .text_color(t.text_primary)
                            .child(text(locale, key))
                            .children(keys.map(|keys| ui::kbd(&keys, t))),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    /// The chat's own header buttons, wherever its area header or view
    /// strip is (workbench.md 4.8.4): Open in browser, Reload, and its menu,
    /// which holds the view menu after its own items.
    pub fn header_buttons(
        &mut self,
        place: Place,
        can_split: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.place = place;
        self.can_split = can_split;
        let t = cx.global::<Tokens>().clone();
        let can_reload = self.state.has_web;
        let menu = self.menu.then(|| self.render_menu(&t, cx));
        let tip = |this: &Self, tip: Tip, cx: &App| this.tooltip(tip, &t, cx);
        let stop =
            |_: &gpui_kit::MouseDownEvent, _: &mut Window, cx: &mut App| cx.stop_propagation();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
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
        deferred(anchored().offset(point(px(-176.), px(28.))).child(panel))
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
                    .web
                    .as_ref()
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
        let body = self.body(cx);
        div()
            .id("chat-dock")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg_sunken)
            .child(div().flex().flex_col().flex_1().min_h_0().child(body))
    }
}

/// The chat's web view: claude.ai, the allow-list, new windows and
/// downloads to the system browser.
fn chat_builder<'a>(
    context: &'a mut wry::WebContext,
    background: (u8, u8, u8, u8),
    to_browser: Sender<Signal>,
    new_window: Sender<Signal>,
) -> wry::WebViewBuilder<'a> {
    wry::WebViewBuilder::new_with_web_context(context)
        .with_url(chat::HOME)
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
