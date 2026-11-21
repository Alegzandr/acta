//! The chat panel's logic, without GPUI or the web view (ADR 0009;
//! docs/design/components-app.md 5, Chat panel; interaction.md 6.1):
//! where a navigation may go, which keys Acta keeps while the page has the
//! keyboard, and the panel's states.
//!
//! The panel is a plain browser surface: nothing here reads from or writes
//! to the page. URLs are only classified, keys only matched.

use url::Url;

/// The page the panel opens, and the one "Open claude.ai in your browser"
/// opens everywhere.
pub const HOME: &str = "https://claude.ai/";
/// Microsoft's WebView2 download page ("Get WebView2").
pub const WEBVIEW2_DOWNLOAD: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

// ---- navigation ----

/// What happens to a navigation the page starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Load inside the panel.
    InPanel,
    /// Refuse in the panel and hand the URL to the system browser.
    Browser,
    /// Refuse and do nothing (other schemes, unparsable input).
    Block,
}

/// Hosts the page may stay on: the host itself or any subdomain of it.
const PANEL_HOSTS: &[&str] = &["claude.ai"];
/// Sign-in providers, exact hosts only (ADR 0009, decision 6). To be
/// corrected after the first real sign-in.
const SIGN_IN_HOSTS: &[&str] = &["accounts.google.com", "appleid.apple.com"];

/// Where a navigation to `raw` goes.
pub fn classify(raw: &str) -> Decision {
    // Chromium and WebKit report these for the initial document and for
    // blob frames the page creates itself.
    if raw == "about:blank" || raw.starts_with("blob:https://claude.ai/") {
        return Decision::InPanel;
    }
    let Ok(url) = Url::parse(raw) else {
        return Decision::Block;
    };
    match url.scheme() {
        "https" => {}
        "http" => return browser_or_block(raw),
        _ => return Decision::Block,
    }
    let Some(host) = url.host_str() else {
        return Decision::Block;
    };
    let in_panel = PANEL_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
        || SIGN_IN_HOSTS.contains(&host);
    if in_panel {
        Decision::InPanel
    } else {
        browser_or_block(raw)
    }
}

/// The same gate as the terminal's links: only `http(s)` with a host is
/// handed to the system's URL handler.
fn browser_or_block(raw: &str) -> Decision {
    if acta_terminal::url::is_openable(raw) {
        Decision::Browser
    } else {
        Decision::Block
    }
}

/// A new-window request from the page (a `target=_blank` link, a popup):
/// never a second native window; a web URL goes to the system browser.
pub fn new_window(raw: &str) -> Decision {
    if acta_terminal::url::is_openable(raw) {
        Decision::Browser
    } else {
        Decision::Block
    }
}

/// Scheme, host and path only, for the log: query strings carry tokens.
pub fn for_log(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(url) => format!(
            "{}://{}{}",
            url.scheme(),
            url.host_str().unwrap_or(""),
            url.path()
        ),
        Err(_) => "(unparsable)".into(),
    }
}

// ---- keys ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
}

/// A key, as far as the tables of interaction.md 6.1 need to tell keys
/// apart. Letters are the character the layout produces; digits and the
/// backtick are physical positions (the US layout's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    Letter(char),
    Digit(u8),
    Backtick,
    Tab,
    Enter,
    F6,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Cmd on macOS.
    pub cmd: bool,
}

/// A key Acta keeps while the page has the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    Palette,
    ToggleChat,
    ToggleTerminal,
    /// `Mod+1` to `Mod+4`: sidebar, page, terminal, chat.
    Focus(u8),
    NextTab,
    PreviousTab,
    NextPane,
    PreviousPane,
    ZoomToolWindow,
    /// macOS's own `Cmd+Q`, `Cmd+H`, `Cmd+M`: left to the system.
    System,
}

/// What Acta keeps of `key` with `mods` on `platform` while the chat web
/// view has focus (interaction.md 6.1, "Chat web view"). Everything else
/// belongs to the page, `Mod+K` and plain typing included.
pub fn kept(platform: Platform, key: KeyCode, mods: Mods) -> Option<Kept> {
    let Mods {
        ctrl,
        shift,
        alt,
        cmd,
    } = mods;
    // Alt (Option) is never part of a kept key; on Windows Ctrl+Alt is
    // also AltGr, which types characters.
    if alt {
        return None;
    }
    // `Mod`: Ctrl on Windows, Cmd on macOS, and never both.
    let (modifier, other) = match platform {
        Platform::Windows => (ctrl, cmd),
        Platform::MacOs => (cmd, ctrl),
    };
    let bare = !ctrl && !cmd;
    match key {
        KeyCode::F6 if bare => Some(if shift {
            Kept::PreviousPane
        } else {
            Kept::NextPane
        }),
        // Ctrl+Tab on every platform.
        KeyCode::Tab if ctrl && !cmd => Some(if shift {
            Kept::PreviousTab
        } else {
            Kept::NextTab
        }),
        // Ctrl+` on every platform.
        KeyCode::Backtick if ctrl && !cmd && !shift => Some(Kept::ToggleTerminal),
        _ if !modifier || other => None,
        KeyCode::Letter('k') if shift => Some(Kept::Palette),
        KeyCode::Letter('h') if shift => Some(Kept::ToggleChat),
        KeyCode::Letter('j') if shift => Some(Kept::ToggleTerminal),
        KeyCode::Enter if shift => Some(Kept::ZoomToolWindow),
        KeyCode::Digit(n @ 1..=4) if !shift => Some(Kept::Focus(n)),
        KeyCode::Letter('q' | 'h' | 'm') if platform == Platform::MacOs && !shift => {
            Some(Kept::System)
        }
        _ => None,
    }
}

/// A Windows key event as a [`KeyCode`]: letters by virtual key (the
/// layout's character), digits and the backtick by scan code (the
/// physical key).
pub fn windows_key(virtual_key: u32, scan_code: u32) -> KeyCode {
    match (virtual_key, scan_code) {
        (0x41..=0x5a, _) => KeyCode::Letter(char::from(virtual_key as u8).to_ascii_lowercase()),
        (_, 0x02..=0x0a) => KeyCode::Digit((scan_code - 1) as u8),
        (_, 0x0b) => KeyCode::Digit(0),
        (_, 0x29) => KeyCode::Backtick,
        (0x09, _) => KeyCode::Tab,
        (0x0d, _) => KeyCode::Enter,
        (0x75, _) => KeyCode::F6,
        _ => KeyCode::Other,
    }
}

// ---- states ----

/// Why the page could not be shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Unknown,
    Offline,
}

/// The web view's error status of a failed top-level navigation
/// (`COREWEBVIEW2_WEB_ERROR_STATUS`), as what the panel shows. `None` for
/// what is not a failure to show: a navigation the panel refused itself
/// (operation canceled), a download, or an unknown status.
pub fn failure_of(status: i32) -> Option<Reason> {
    match status {
        // DISCONNECTED, HOST_NAME_NOT_RESOLVED.
        11 | 13 => Some(Reason::Offline),
        // Certificate errors (1 to 5), SERVER_UNREACHABLE, TIMEOUT,
        // invalid server response, CONNECTION_ABORTED, CONNECTION_RESET,
        // CANNOT_CONNECT, UNEXPECTED_ERROR.
        1..=10 | 12 | 16 => Some(Reason::Unknown),
        _ => None,
    }
}

/// Where the web view stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nothing created; the panel explains and offers to start.
    NotStarted,
    /// The web view is being created.
    Starting,
    /// The page is in the web view.
    Live,
    /// Windows without the WebView2 runtime.
    RuntimeMissing,
    /// Creation or the page load failed.
    Failed(Reason),
}

/// What the panel draws now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The panel is not shown.
    Closed,
    NotStarted,
    /// The empty surface; `line` once 400 ms have passed.
    Starting {
        line: bool,
    },
    /// The web view fills the panel under the header.
    Live,
    /// The web view is hidden under a GPUI overlay.
    Covered,
    RuntimeMissing,
    Failed(Reason),
}

/// What the GPUI side must do after a transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Create the web view (once the starting surface is on screen).
    Build,
    /// Load the page again in the existing web view.
    Reload,
    /// Open this URL in the system browser instead of showing a panel.
    OpenInBrowser(&'static str),
    /// Destroy the web view and delete its profile.
    DropAndClear,
}

/// How creating the web view ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Built {
    Ok,
    RuntimeMissing,
    Failed,
}

/// The panel's state machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatState {
    /// Whether this platform has a panel at all (not on Linux).
    pub panel: bool,
    pub phase: Phase,
    /// Shown in its dock.
    pub shown: bool,
    /// A GPUI overlay that could overlap the panel is open.
    pub covered: bool,
    /// The web view exists (it lives on while hidden).
    pub has_web: bool,
    /// The "Opening claude.ai..." line is due (400 ms after starting).
    pub starting_line: bool,
    /// The user has opened claude.ai here before: showing the panel starts
    /// the web view without asking again.
    pub consented: bool,
}

impl ChatState {
    pub fn new(panel: bool, consented: bool) -> Self {
        Self {
            panel,
            phase: Phase::NotStarted,
            shown: false,
            covered: false,
            has_web: false,
            starting_line: false,
            consented,
        }
    }

    /// Show the panel. On Linux there is none: claude.ai opens in the
    /// browser. Once the user has started it here, showing starts it.
    pub fn show(&mut self) -> Effect {
        if !self.panel {
            return Effect::OpenInBrowser(HOME);
        }
        self.shown = true;
        if self.phase == Phase::NotStarted && self.consented {
            return self.start();
        }
        Effect::None
    }

    pub fn hide(&mut self) {
        self.shown = false;
    }

    /// "Open claude.ai here".
    pub fn start(&mut self) -> Effect {
        if !self.panel {
            return Effect::OpenInBrowser(HOME);
        }
        if matches!(self.phase, Phase::Starting | Phase::Live) {
            return Effect::None;
        }
        self.consented = true;
        self.phase = Phase::Starting;
        self.starting_line = false;
        Effect::Build
    }

    /// The 400 ms of the starting surface have passed.
    pub fn starting_elapsed(&mut self) {
        if self.phase == Phase::Starting {
            self.starting_line = true;
        }
    }

    pub fn built(&mut self, result: Built) {
        if self.phase != Phase::Starting {
            return;
        }
        self.phase = match result {
            Built::Ok => {
                self.has_web = true;
                Phase::Live
            }
            Built::RuntimeMissing => Phase::RuntimeMissing,
            Built::Failed => Phase::Failed(Reason::Unknown),
        };
    }

    /// The page failed to load (the panel never fails to a blank surface).
    pub fn load_failed(&mut self, reason: Reason) {
        if self.has_web && matches!(self.phase, Phase::Live | Phase::Failed(_)) {
            self.phase = Phase::Failed(reason);
        }
    }

    /// "Try again" (runtime missing or could not open) and "Reload".
    pub fn retry(&mut self) -> Effect {
        match self.phase {
            Phase::Failed(_) | Phase::Live if self.has_web => {
                self.phase = Phase::Live;
                Effect::Reload
            }
            Phase::Failed(_) | Phase::RuntimeMissing => {
                self.phase = Phase::NotStarted;
                self.start()
            }
            _ => Effect::None,
        }
    }

    pub fn set_covered(&mut self, covered: bool) {
        self.covered = covered;
    }

    /// "Sign out and clear chat data", confirmed: back to not started.
    pub fn sign_out(&mut self) -> Effect {
        self.phase = Phase::NotStarted;
        self.consented = false;
        self.starting_line = false;
        // Also without a web view: the profile may exist from an earlier run.
        self.has_web = false;
        Effect::DropAndClear
    }

    /// Whether the native view is to be visible: it covers everything GPUI
    /// draws in its bounds, so only while the page is the surface.
    pub fn web_visible(&self) -> bool {
        self.surface() == Surface::Live
    }

    pub fn surface(&self) -> Surface {
        if !self.panel || !self.shown {
            return Surface::Closed;
        }
        match self.phase {
            Phase::NotStarted => Surface::NotStarted,
            Phase::Starting => Surface::Starting {
                line: self.starting_line,
            },
            Phase::Live if self.covered => Surface::Covered,
            Phase::Live => Surface::Live,
            Phase::RuntimeMissing => Surface::RuntimeMissing,
            Phase::Failed(reason) => Surface::Failed(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- navigation ----

    #[test]
    fn claude_and_its_subdomains_stay_in_the_panel() {
        assert_eq!(classify("https://claude.ai/new"), Decision::InPanel);
        assert_eq!(classify("https://api.claude.ai/x?y=1"), Decision::InPanel);
        assert_eq!(classify("about:blank"), Decision::InPanel);
        assert_eq!(classify("blob:https://claude.ai/1234"), Decision::InPanel);
    }

    #[test]
    fn lookalike_hosts_leave_the_panel() {
        for raw in [
            "https://claude.ai.example.com/login",
            "https://notclaude.ai/",
            "https://user@claude.ai@example.com/",
        ] {
            assert_eq!(classify(raw), Decision::Browser, "{raw}");
        }
    }

    #[test]
    fn plain_http_and_other_sites_go_to_the_system_browser() {
        assert_eq!(classify("http://claude.ai/"), Decision::Browser);
        assert_eq!(
            classify("https://github.com/Alexandre/project"),
            Decision::Browser
        );
        assert_eq!(
            classify("https://support.claude.com/en/articles/1"),
            Decision::Browser
        );
    }

    #[test]
    fn non_web_schemes_are_dropped() {
        for raw in [
            "file:///C:/Users/Alexandre/x",
            "javascript:alert(1)",
            "ms-msdt:/id",
            "mailto:alexandre@example.com",
            "not a url",
            "blob:https://example.com/1",
        ] {
            assert_eq!(classify(raw), Decision::Block, "{raw}");
        }
    }

    #[test]
    fn sign_in_providers_match_exactly() {
        assert_eq!(
            classify("https://accounts.google.com/o/oauth2/v2/auth"),
            Decision::InPanel
        );
        assert_eq!(
            classify("https://appleid.apple.com/auth"),
            Decision::InPanel
        );
        assert_eq!(
            classify("https://evil.accounts.google.com/"),
            Decision::Browser
        );
        assert_eq!(classify("http://accounts.google.com/"), Decision::Browser);
    }

    #[test]
    fn new_windows_only_ever_reach_the_browser() {
        assert_eq!(new_window("https://claude.ai/new"), Decision::Browser);
        assert_eq!(new_window("https://example.com/"), Decision::Browser);
        assert_eq!(new_window("file:///etc/passwd"), Decision::Block);
        assert_eq!(new_window("https://"), Decision::Block);
    }

    #[test]
    fn the_log_never_sees_a_query_string() {
        assert_eq!(
            for_log("https://claude.ai/magic-link?token=secret#x"),
            "https://claude.ai/magic-link"
        );
        assert_eq!(for_log("::"), "(unparsable)");
    }

    // ---- keys ----

    fn ctrl() -> Mods {
        Mods {
            ctrl: true,
            ..Mods::default()
        }
    }
    fn ctrl_shift() -> Mods {
        Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        }
    }
    fn cmd() -> Mods {
        Mods {
            cmd: true,
            ..Mods::default()
        }
    }
    fn cmd_shift() -> Mods {
        Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        }
    }

    #[test]
    fn windows_keeps_exactly_the_listed_keys() {
        use KeyCode::*;
        let w = |key, mods| kept(Platform::Windows, key, mods);
        assert_eq!(w(Letter('k'), ctrl_shift()), Some(Kept::Palette));
        assert_eq!(w(Letter('h'), ctrl_shift()), Some(Kept::ToggleChat));
        assert_eq!(w(Backtick, ctrl()), Some(Kept::ToggleTerminal));
        assert_eq!(w(Letter('j'), ctrl_shift()), Some(Kept::ToggleTerminal));
        for n in 1..=4 {
            assert_eq!(w(Digit(n), ctrl()), Some(Kept::Focus(n)));
        }
        assert_eq!(w(Tab, ctrl()), Some(Kept::NextTab));
        assert_eq!(w(Tab, ctrl_shift()), Some(Kept::PreviousTab));
        assert_eq!(w(F6, Mods::default()), Some(Kept::NextPane));
        let shift = Mods {
            shift: true,
            ..Mods::default()
        };
        assert_eq!(w(F6, shift), Some(Kept::PreviousPane));
        assert_eq!(w(Enter, ctrl_shift()), Some(Kept::ZoomToolWindow));
    }

    #[test]
    fn windows_passes_the_page_its_own_keys() {
        use KeyCode::*;
        let w = |key, mods| kept(Platform::Windows, key, mods);
        // Mod+K is claude.ai's own shortcut; Ctrl+W, Ctrl+N, Ctrl+Enter too.
        for key in [Letter('k'), Letter('w'), Letter('n'), Letter('c'), Enter] {
            assert_eq!(w(key, ctrl()), None, "{key:?}");
        }
        // Plain typing, digits without Ctrl, Ctrl+5, Alt and AltGr.
        assert_eq!(w(Letter('k'), Mods::default()), None);
        assert_eq!(w(Digit(1), Mods::default()), None);
        assert_eq!(w(Digit(5), ctrl()), None);
        let alt_gr = Mods {
            ctrl: true,
            alt: true,
            ..Mods::default()
        };
        assert_eq!(w(Digit(2), alt_gr), None);
        assert_eq!(
            w(
                Letter('k'),
                Mods {
                    alt: true,
                    ..ctrl_shift()
                }
            ),
            None
        );
        // Function keys other than F6, Ctrl+Shift+1.
        assert_eq!(w(Other, Mods::default()), None);
        assert_eq!(w(Digit(1), ctrl_shift()), None);
        // Cmd (the Windows key) is not Mod on Windows.
        assert_eq!(w(Letter('k'), cmd_shift()), None);
    }

    #[test]
    fn macos_keeps_cmd_keys_and_the_ctrl_exceptions() {
        use KeyCode::*;
        let m = |key, mods| kept(Platform::MacOs, key, mods);
        assert_eq!(m(Letter('k'), cmd_shift()), Some(Kept::Palette));
        assert_eq!(m(Letter('h'), cmd_shift()), Some(Kept::ToggleChat));
        assert_eq!(m(Letter('j'), cmd_shift()), Some(Kept::ToggleTerminal));
        assert_eq!(m(Backtick, ctrl()), Some(Kept::ToggleTerminal));
        assert_eq!(m(Digit(4), cmd()), Some(Kept::Focus(4)));
        assert_eq!(m(Tab, ctrl()), Some(Kept::NextTab));
        assert_eq!(m(Enter, cmd_shift()), Some(Kept::ZoomToolWindow));
        for key in [Letter('q'), Letter('h'), Letter('m')] {
            assert_eq!(m(key, cmd()), Some(Kept::System));
        }
        // Cmd+K, Cmd+W, Cmd+N, Cmd+, belong to the page; so does Ctrl+Shift+K.
        for key in [Letter('k'), Letter('w'), Letter('n'), Other] {
            assert_eq!(m(key, cmd()), None, "{key:?}");
        }
        assert_eq!(m(Letter('k'), ctrl_shift()), None);
        assert_eq!(m(Digit(1), ctrl()), None);
    }

    #[test]
    fn windows_keys_follow_the_layout_for_letters_and_the_position_for_digits() {
        assert_eq!(windows_key(0x4b, 0x25), KeyCode::Letter('k'));
        // AZERTY: the digit row yields symbols, but the scan code is the key.
        assert_eq!(windows_key(0xde, 0x02), KeyCode::Digit(1));
        assert_eq!(windows_key(0x34, 0x05), KeyCode::Digit(4));
        assert_eq!(windows_key(0x30, 0x0b), KeyCode::Digit(0));
        assert_eq!(windows_key(0xc0, 0x29), KeyCode::Backtick);
        assert_eq!(windows_key(0xde, 0x29), KeyCode::Backtick);
        assert_eq!(windows_key(0x09, 0x0f), KeyCode::Tab);
        assert_eq!(windows_key(0x0d, 0x1c), KeyCode::Enter);
        assert_eq!(windows_key(0x75, 0x40), KeyCode::F6);
        assert_eq!(windows_key(0x74, 0x3f), KeyCode::Other);
    }

    // ---- states ----

    #[test]
    fn the_panel_starts_closed_and_asks_before_creating_anything() {
        let mut s = ChatState::new(true, false);
        assert_eq!(s.surface(), Surface::Closed);
        assert_eq!(s.show(), Effect::None);
        assert_eq!(s.surface(), Surface::NotStarted);
        assert!(!s.web_visible());
    }

    #[test]
    fn opening_goes_through_starting_to_live() {
        let mut s = ChatState::new(true, false);
        s.show();
        assert_eq!(s.start(), Effect::Build);
        assert_eq!(s.surface(), Surface::Starting { line: false });
        s.starting_elapsed();
        assert_eq!(s.surface(), Surface::Starting { line: true });
        // A second click while it builds does not build twice.
        assert_eq!(s.start(), Effect::None);
        s.built(Built::Ok);
        assert_eq!(s.surface(), Surface::Live);
        assert!(s.web_visible() && s.has_web && s.consented);
    }

    #[test]
    fn once_consented_showing_starts_the_web_view() {
        let mut s = ChatState::new(true, true);
        assert_eq!(s.show(), Effect::Build);
        assert_eq!(s.phase, Phase::Starting);
    }

    #[test]
    fn hiding_keeps_the_web_view_and_showing_brings_it_back() {
        let mut s = ChatState::new(true, true);
        s.show();
        s.built(Built::Ok);
        s.hide();
        assert_eq!(s.surface(), Surface::Closed);
        assert!(!s.web_visible() && s.has_web);
        assert_eq!(s.show(), Effect::None);
        assert!(s.web_visible());
    }

    #[test]
    fn an_overlay_hides_the_web_view_and_shows_the_placeholder() {
        let mut s = ChatState::new(true, true);
        s.show();
        s.built(Built::Ok);
        s.set_covered(true);
        assert_eq!(s.surface(), Surface::Covered);
        assert!(!s.web_visible());
        s.set_covered(false);
        assert!(s.web_visible());
        // Only the live page is covered: the other states are GPUI's own.
        let mut s = ChatState::new(true, false);
        s.show();
        s.set_covered(true);
        assert_eq!(s.surface(), Surface::NotStarted);
    }

    #[test]
    fn a_missing_runtime_offers_to_try_again() {
        let mut s = ChatState::new(true, false);
        s.show();
        s.start();
        s.built(Built::RuntimeMissing);
        assert_eq!(s.surface(), Surface::RuntimeMissing);
        assert!(!s.has_web);
        assert_eq!(s.retry(), Effect::Build);
        assert_eq!(s.phase, Phase::Starting);
    }

    #[test]
    fn a_failed_load_shows_why_and_reload_retries_in_place() {
        let mut s = ChatState::new(true, true);
        s.show();
        s.built(Built::Ok);
        s.load_failed(Reason::Offline);
        assert_eq!(s.surface(), Surface::Failed(Reason::Offline));
        assert!(!s.web_visible(), "never a blank or error page");
        assert_eq!(s.retry(), Effect::Reload);
        assert_eq!(s.surface(), Surface::Live);
        // A creation failure builds again.
        let mut s = ChatState::new(true, true);
        s.show();
        s.built(Built::Failed);
        assert_eq!(s.surface(), Surface::Failed(Reason::Unknown));
        assert_eq!(s.retry(), Effect::Build);
    }

    #[test]
    fn a_load_failure_before_the_web_view_exists_is_ignored() {
        let mut s = ChatState::new(true, true);
        s.show();
        s.load_failed(Reason::Offline);
        assert_eq!(s.phase, Phase::Starting);
    }

    #[test]
    fn signing_out_drops_everything_and_asks_again() {
        let mut s = ChatState::new(true, true);
        s.show();
        s.built(Built::Ok);
        assert_eq!(s.sign_out(), Effect::DropAndClear);
        assert_eq!(s.surface(), Surface::NotStarted);
        assert!(!s.has_web && !s.consented);
        assert_eq!(s.sign_out(), Effect::DropAndClear);
    }

    #[test]
    fn linux_has_no_panel_only_the_link() {
        let mut s = ChatState::new(false, true);
        assert_eq!(s.show(), Effect::OpenInBrowser(HOME));
        assert_eq!(s.start(), Effect::OpenInBrowser(HOME));
        assert_eq!(s.surface(), Surface::Closed);
        assert!(!s.web_visible());
    }

    #[test]
    fn only_connection_failures_are_shown() {
        assert_eq!(failure_of(11), Some(Reason::Offline));
        assert_eq!(failure_of(13), Some(Reason::Offline));
        assert_eq!(failure_of(12), Some(Reason::Unknown));
        assert_eq!(failure_of(7), Some(Reason::Unknown));
        // Refused by the allow-list, unknown, redirect.
        assert_eq!(failure_of(14), None);
        assert_eq!(failure_of(0), None);
        assert_eq!(failure_of(15), None);
    }
}
