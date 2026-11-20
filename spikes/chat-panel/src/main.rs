// Spike for ADR 0009: a GPUI window with a plain GPUI pane on the left and an embedded
// system web view (gpui-webview on wry: WebView2 on Windows) docked right, resizable,
// hideable, with a persistent profile directory.
//
// Driven from outside for repeatable captures: every 100 ms the app reads one command from
// the file named by SPIKE_CMD (width <px> | toggle | overlay | palette | popup | focus-web |
// focus-gpui | load <url> | quit) and appends what happens to the file named by SPIKE_LOG.
// "overlay" draws a GPUI overlay and leaves the web view alone (shows the z-order problem);
// "palette" draws the same overlay and hides the web view while it is open (the remedy).
// Ctrl+Shift+K is bound to a GPUI action that only logs: it shows whether an app shortcut
// still reaches GPUI while the web view has the keyboard.
// SPIKE_URL (default https://claude.ai) and SPIKE_PROFILE (web view data directory, default
// a per-user data directory, see `default_profile`) set the page and where its cookies and
// storage live. If the web view cannot be created the window still opens and says why.

mod allow;

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_webview::WebView;

use allow::{Decision, classify};

// DESIGN.md 2.1, dark.
const BG_SUNKEN: u32 = 0x141210;
const BG_BASE: u32 = 0x1a1816;
const BG_OVERLAY: u32 = 0x292622;
const BORDER_SUBTLE: u32 = 0x2b2824;
const BORDER_DEFAULT: u32 = 0x38342f;
const TEXT_PRIMARY: u32 = 0xe9e3d8;
const TEXT_SECONDARY: u32 = 0xb5ad9f;
const ACCENT: u32 = 0x72b5a2;

// DESIGN.md 4.2: the right dock is 648 wide by default, 400 to 60% of the window.
const PANEL_DEFAULT: f32 = 648.;
const PANEL_MIN: f32 = 400.;

actions!(spike, [PaletteKey]);

static START: OnceLock<Instant> = OnceLock::new();

fn log(message: &str) {
    let elapsed = START.get_or_init(Instant::now).elapsed().as_millis();
    let line = format!("{elapsed:>6} ms  {message}\n");
    print!("{line}");
    if let Some(path) = std::env::var_os("SPIKE_LOG")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

struct Shell {
    input: Entity<InputState>,
    // None when the system web view could not be created (see `web_error`).
    webview: Option<Entity<WebView>>,
    web_error: Option<String>,
    panel_width: f32,
    panel_visible: bool,
    dragging: bool,
    overlay: bool,
    // The overlay was opened as "palette": the web view is hidden until it closes.
    web_hidden_for_overlay: bool,
    popup: Option<AnyWindowHandle>,
    // wry borrows the context while building; keep it for the life of the web view.
    _web_context: Box<wry::WebContext>,
}

struct PopupNote;

impl Render for PopupNote {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(BG_OVERLAY))
            .border_1()
            .border_color(rgb(ACCENT))
            .text_color(rgb(TEXT_PRIMARY))
            .child("Separate GPUI popup window above the web view")
    }
}

impl Shell {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = std::env::var("SPIKE_URL").unwrap_or_else(|_| "https://claude.ai".into());
        let profile = std::env::var_os("SPIKE_PROFILE")
            .map(PathBuf::from)
            .or_else(default_profile);
        // Without a directory wry lets WebView2 create "<exe>.WebView2" next to the
        // executable, which for Acta would be inside the Velopack install root.
        log(&format!("profile directory: {profile:?}"));
        // The spike's local test page is not on the allow-list; name its host explicitly.
        let extra_host = url::Url::parse(&url)
            .ok()
            .filter(|u| u.scheme() == "http")
            .and_then(|u| u.host_str().map(str::to_owned));

        let mut web_context = Box::new(wry::WebContext::new(profile));
        let builder = wry::WebViewBuilder::new_with_web_context(&mut web_context)
            .with_url(&url)
            .with_background_color((0x14, 0x12, 0x10, 0xff))
            .with_devtools(false)
            .with_navigation_handler(move |target| {
                let decision = classify(&target, extra_host.as_deref());
                log(&format!(
                    "navigation {decision:?}: {}",
                    origin_only(&target)
                ));
                decision == Decision::InPanel
            })
            .with_new_window_req_handler(|target, _| {
                // Never open a second native window from the page; a real build hands
                // the URL to the system browser here.
                log(&format!(
                    "new-window request denied: {}",
                    origin_only(&target)
                ));
                wry::NewWindowResponse::Deny
            })
            .with_download_started_handler(|source, _| {
                log(&format!("download refused: {}", origin_only(&source)));
                false
            })
            .with_document_title_changed_handler(|title| log(&format!("title: {title}")))
            .with_on_page_load_handler(|event, target| {
                let phase = match event {
                    wry::PageLoadEvent::Started => "started",
                    wry::PageLoadEvent::Finished => "finished",
                };
                log(&format!("page load {phase}: {}", origin_only(&target)));
            });
        let (webview, web_error) = match WebView::build(builder, window, cx) {
            Ok(webview) => {
                log(&format!(
                    "web view built, runtime {}",
                    wry::webview_version().unwrap_or_default()
                ));
                (Some(cx.new(|_| webview)), None)
            }
            Err(error) => {
                log(&format!("web view unavailable: {error}"));
                (None, Some(error.to_string()))
            }
        };

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("GPUI input"));
        cx.subscribe(&input, |_, input, event: &InputEvent, cx| match event {
            InputEvent::Change => log(&format!("gpui input value: {}", input.read(cx).value())),
            InputEvent::Focus => log("gpui input focused"),
            InputEvent::Blur => log("gpui input blurred"),
            InputEvent::PressEnter { .. } => {}
        })
        .detach();

        if let Some(command_file) = std::env::var_os("SPIKE_CMD").map(PathBuf::from) {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let path = command_file.clone();
                    let command = cx
                        .background_spawn(async move {
                            let text = std::fs::read_to_string(&path).ok()?;
                            std::fs::remove_file(&path).ok()?;
                            Some(text.trim().to_owned())
                        })
                        .await;
                    let Some(command) = command else { continue };
                    if this
                        .update_in(cx, |this, window, cx| this.run(&command, window, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }

        Self {
            input,
            webview,
            web_error,
            panel_width: PANEL_DEFAULT,
            panel_visible: true,
            dragging: false,
            overlay: false,
            web_hidden_for_overlay: false,
            popup: None,
            _web_context: web_context,
        }
    }

    fn run(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        log(&format!("command: {command}"));
        let (name, argument) = command.split_once(' ').unwrap_or((command, ""));
        match name {
            "width" => {
                if let Ok(width) = argument.parse::<f32>() {
                    self.set_panel_width(width, window);
                }
            }
            "toggle" => {
                self.panel_visible = !self.panel_visible;
                self.sync_web_visibility(cx);
            }
            "overlay" => self.overlay = !self.overlay,
            "palette" => {
                self.overlay = !self.overlay;
                self.web_hidden_for_overlay = self.overlay;
                self.sync_web_visibility(cx);
                if self.overlay {
                    // A real palette focuses its own input; the GPUI input stands in.
                    self.input.focus_handle(cx).focus(window, cx);
                }
            }
            "popup" => self.toggle_popup(window, cx),
            "focus-web" => {
                if let Some(webview) = &self.webview {
                    let result = webview.read(cx).raw().focus();
                    log(&format!("web view focus requested: {result:?}"));
                }
            }
            "focus-gpui" => {
                if let Some(webview) = &self.webview {
                    let result = webview.read(cx).raw().focus_parent();
                    log(&format!("focus returned to GPUI: {result:?}"));
                }
                self.input.focus_handle(cx).focus(window, cx);
            }
            "load" => {
                if let Some(webview) = &self.webview {
                    webview.update(cx, |webview, _| webview.load_url(argument));
                }
            }
            "quit" => cx.quit(),
            _ => log("unknown command"),
        }
        cx.notify();
    }

    /// The native view is not part of GPUI's scene: when its element stops being painted
    /// it stays where it was, so it has to be hidden and shown explicitly. Hiding also
    /// hands the keyboard back to the GPUI window (`WebView::set_visible`).
    fn sync_web_visibility(&mut self, cx: &mut Context<Self>) {
        let visible = self.panel_visible && !self.web_hidden_for_overlay;
        if let Some(webview) = &self.webview {
            webview.update(cx, |webview, _| {
                if let Err(error) = webview.set_visible(visible) {
                    log(&format!("web view visibility change failed: {error}"));
                }
            });
        }
        log(&format!("web view visible: {visible}"));
    }

    fn set_panel_width(&mut self, width: f32, window: &Window) {
        let max = f32::from(window.viewport_size().width) * 0.6;
        self.panel_width = width.clamp(PANEL_MIN, max.max(PANEL_MIN));
    }

    fn toggle_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(popup) = self.popup.take() {
            let _ = popup.update(cx, |_, window, _| window.remove_window());
            return;
        }
        let parent = window.bounds();
        let popup_size = size(px(520.), px(120.));
        let origin = point(
            parent.origin.x + (parent.size.width - popup_size.width) / 2.,
            parent.origin.y + px(300.),
        );
        let options = WindowOptions {
            kind: WindowKind::PopUp,
            titlebar: None,
            focus: false,
            is_movable: false,
            is_resizable: false,
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin,
                size: popup_size,
            })),
            ..Default::default()
        };
        match cx.open_window(options, |_, cx| cx.new(|_| PopupNote)) {
            Ok(handle) => self.popup = Some(handle.into()),
            Err(error) => log(&format!("popup window failed: {error:#}")),
        }
    }
}

/// A per-user data directory that is not next to the executable, so the profile survives
/// updates that replace the install directory. Acta would use its own data directory.
fn default_profile() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    } else {
        match std::env::var_os("XDG_DATA_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(std::env::var_os("HOME")?).join(".local/share"),
        }
    };
    Some(base.join("ActaSpikeChatPanel").join("webview"))
}

/// Log scheme, host and path only: query strings can carry tokens.
fn origin_only(raw: &str) -> String {
    match url::Url::parse(raw) {
        Ok(parsed) => format!(
            "{}://{}{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            parsed.path()
        ),
        Err(_) => "(unparsable)".into(),
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let state = format!(
            "Panel {} at {} px. Overlay {}. Popup {}.",
            if self.panel_visible {
                "shown"
            } else {
                "hidden"
            },
            self.panel_width,
            if self.overlay { "on" } else { "off" },
            if self.popup.is_some() {
                "open"
            } else {
                "closed"
            },
        );

        let left = div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .bg(rgb(BG_BASE))
            .child("Acta - chat panel spike (ADR 0009)")
            .child(
                div()
                    .text_color(rgb(TEXT_SECONDARY))
                    .child("Left pane: plain GPUI content. Right pane: system web view."),
            )
            .child(div().w(px(320.)).child(Input::new(&self.input)))
            .child(div().text_color(rgb(TEXT_SECONDARY)).child(state));

        let handle = div()
            .id("split-handle")
            .w(px(6.))
            .h_full()
            .flex_none()
            .bg(rgb(if self.dragging { ACCENT } else { BORDER_SUBTLE }))
            .cursor_col_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dragging = true;
                    log("split drag started");
                    cx.notify();
                }),
            );

        let panel = div()
            .w(px(self.panel_width))
            .h_full()
            .flex_none()
            .bg(rgb(BG_SUNKEN))
            .text_color(rgb(TEXT_SECONDARY))
            .map(|panel| match (&self.webview, &self.web_error) {
                (Some(webview), _) if !self.web_hidden_for_overlay => panel.child(webview.clone()),
                (Some(_), _) => panel,
                (None, error) => panel.p_8().child(format!(
                    "The system web view is not available: {}",
                    error.as_deref().unwrap_or("unknown error")
                )),
            });

        let overlay = self.overlay.then(|| {
            deferred(
                div()
                    .absolute()
                    .top(px(120.))
                    .left((viewport.width - px(520.)) / 2.)
                    .w(px(520.))
                    .h(px(120.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(BG_OVERLAY))
                    .border_1()
                    .border_color(rgb(BORDER_DEFAULT))
                    .rounded(px(8.))
                    .child("GPUI overlay drawn in the same window (palette stand-in)"),
            )
            .with_priority(1)
        });

        div()
            .relative()
            .size_full()
            .flex()
            .bg(rgb(BG_BASE))
            .text_color(rgb(TEXT_PRIMARY))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.dragging {
                    let width = f32::from(window.viewport_size().width - event.position.x);
                    this.set_panel_width(width, window);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.dragging {
                        this.dragging = false;
                        log(&format!("split drag ended at {} px", this.panel_width));
                        cx.notify();
                    }
                }),
            )
            .child(left)
            .when(self.panel_visible, |root| root.child(handle).child(panel))
            .children(overlay)
    }
}

fn main() {
    START.get_or_init(Instant::now);
    log("process start");

    // gpui-webview's own example sets this: with DirectComposition the GPUI swap chain
    // covers the WebView2 child window. It must be set before the platform starts.
    // SPIKE_KEEP_DCOMP=1 leaves it on, to observe that problem.
    #[cfg(target_os = "windows")]
    if std::env::var_os("SPIKE_KEEP_DCOMP").is_none() {
        // SAFETY: no other thread exists yet.
        unsafe { std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true") };
    }

    // WebKitGTK embeds only into X11 windows (gpui-webview README).
    #[cfg(target_os = "linux")]
    let app = gpui_kit::platform::linux(WindowingModes::X11);
    #[cfg(not(target_os = "linux"))]
    let app = gpui_kit::application();

    app.run(|cx| {
        gpui_kit::init(cx);
        cx.bind_keys([KeyBinding::new("ctrl-shift-k", PaletteKey, None)]);
        cx.on_action(|_: &PaletteKey, _| log("gpui action: palette key received"));
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::update(cx, |theme| {
            theme.radius = px(4.);
            theme.colors.background = rgb(BG_BASE).into();
            theme.colors.foreground = rgb(TEXT_PRIMARY).into();
            theme.colors.input = rgb(BORDER_DEFAULT).into();
            theme.colors.border = rgb(BORDER_DEFAULT).into();
            theme.colors.ring = rgb(ACCENT).into();
            theme.colors.caret = rgb(ACCENT).into();
        });

        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("Acta chat panel spike".into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.), px(800.)),
                cx,
            ))),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            let shell = cx.new(|cx| Shell::new(window, cx));
            log("window open");
            shell
        })
        .expect("failed to open window");

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
