# 0009 - Claude chat panel (embedded claude.ai)

Status: accepted with conditions (M5 opens with the keyboard handoff check, a sign-in by the user and a macOS build; a failed keyboard check stops the milestone)
Date: 2026-11-20

## Context
M5 wants claude.ai as a side panel next to the work, signed in once and staying signed in, at zero cost and
without shipping a browser engine. Facts checked on 2026-11-20; "read" means read in source or documentation,
"run" means observed on Windows 11 (see Evidence).

- **Hosting.** gpui-kit ships no web view element, but its repository publishes `gpui-webview` 0.10.0
  (Apache-2.0, "experimental", formerly `gpui-wry`), pinned to gpui-kit 0.10.0. It wraps `lb-wry` 0.53.3
  (Longbridge's republished wry: WebView2, WKWebView, WebKitGTK). `WebView::build` calls
  `build_as_child(window)` with the GPUI window's raw handle, and a `WebViewElement` sets the native view's
  bounds in `prepaint` every frame, so the view follows GPUI layout. Cost on Windows: exe 26.0 MB against
  25.4 MB without it (ADR 0001), lockfile 904 packages against 803.
- **Z-order.** The native view is not part of GPUI's scene. gpui-webview's README: on gpui-pre "the native
  view ... covers GPUI content in its bounds, including popovers, dialogs, menus and tooltips", cannot be
  clipped, and overlays above the page exist only with the `gpui-fast` backend on macOS and Linux (rejected
  in ADR 0001) and "Not yet" on Windows. Run: confirmed, and hiding the view while an overlay is open works.
- **Windows rendering switch.** The spike sets `GPUI_DISABLE_DIRECT_COMPOSITION=true` before the platform
  starts (taken from the upstream example). Read in gpui-pre-windows 0.3.12: it replaces the composition
  swap chain by an HWND swap chain and drops `WS_EX_NOREDIRECTIONBITMAP`, for every window of the process.
- **Profile.** WebView2 keeps cookies and storage in a user data folder; the default is `<exe>.WebView2`
  next to the executable (Microsoft Learn, "Manage user data folders"), which for Acta is inside the
  Velopack root that updates rename and uninstall deletes (ADR 0004). wry takes the folder from
  `WebContext::new(Some(dir))`. WebKitGTK: same call sets `base_data_directory` and a `cookies` file (read).
  WKWebView ignores the directory: data goes to the default store keyed by the bundle identifier, or to a
  store named by `with_data_store_identifier` on macOS 14+ (read).
- **Runtime.** WebView2 Evergreen is part of Windows 11 and was pushed to Windows 10 1803+ in 2022;
  Microsoft still advises checking, since it can be absent on clean Windows 10, Server and LTSC. The
  bootstrapper is about 2 MB and installs per user without elevation (Microsoft Learn, "Distribute your app
  and the WebView2 Runtime"). macOS always has WKWebView.
- **Linux.** lb-wry links `webkit2gtk` 2.0.1 with feature `v2_40`, i.e. the system `libwebkit2gtk-4.1`
  (2.40 or later), plus GTK 3 and libsoup 3. gpui-webview embeds it only in an X11 window: the app must
  start with `WindowingModes::X11` (XWayland in a Wayland session), `WebView::build` fails on Wayland, and
  GTK's main loop is pumped from GPUI every 16 ms (read). Velopack packs our files into an AppImage and
  bundles no system libraries; Tauri's AppImage, which does bundle WebKitGTK, grows "from the 2-6 MB range to
  70+ MB" (v2.tauri.app/distribute/appimage). A binary linked against WebKitGTK does not start where the
  library is missing.
- **claude.ai in an embedded view.** Run: `https://claude.ai` redirects to `/login` and renders with the
  stock WebView2 user agent (`... Chrome/157.0.0.0 Safari/537.36 Edg/157.0.0.0`, no web view marker); no
  challenge page appeared. The page offers Google, Apple and email.
- **Sign-in.** Google refuses its sign-in in embedded views: "Google Account sign-ins from all embedded
  frameworks will be blocked starting on January 4, 2021" (CEF named), browsers "must identify itself
  clearly in the User-Agent" and must not use another browser's string (Google Developers Blog, 2020-08);
  "Embedded webviews implementing or extending WKWebView" do not comply and get `disallowed_useragent`
  (same blog, 2021-06). WebView2 is not named anywhere; reports for it are mixed. Email sign-in sends a link;
  opened on the requesting browser it signs that browser in, opened elsewhere "clicking it will generate a
  verification code" to "enter ... on the original device where you requested the login email"
  (support.claude.com article 13189465). The mail link opens in the default browser, so the panel gets the code.
- **Terms.** Nothing was found that forbids showing claude.ai, unmodified, in an embedded browser on the
  user's own machine with the user's own account. What the texts do forbid, and what the panel must
  therefore never do: Consumer Terms (effective 2025-10-08) section 3, "to access the Services through
  automated or non-human means, whether through a bot, script, or otherwise", "To crawl, scrape, or otherwise
  harvest data or information from our Services", "bypassing any of our systems or protective measures";
  section 13, no use of "our name, logos, or other trademarks" without permission. Usage Policy (effective
  2026-11-12), "Do Not Abuse Our Platform": "Resell, proxy, or otherwise provide access to Claude through
  unauthorized means", which covers routing requests through consumer subscriptions or misrepresenting the
  product or client being used. This is a reading by an engineer, not legal advice, and Anthropic can block
  embedded views at any time.

## Decision
**Go with limits.** Build the panel on Windows and macOS with the system web view; Linux gets a link
until an X11-only, WebKitGTK-linked build is judged acceptable. (Amended 2026-11-21: macOS gets the link
too for now; see Changelog.)

1. `gpui-webview = "=0.10.0"` and `lb-wry = "=0.53.3"`, in the GUI crate only and only for Windows and
   macOS targets. The domain, storage, CLI and MCP crates never see them. Bumped together with gpui-kit.
2. The panel is a plain browser surface: no initialization scripts, no `evaluate_script`, no IPC handler,
   no custom protocol, no request interception, no cookie API, no devtools, stock user agent. Acta never
   reads from or writes to the page. This is a rule for review, not a setting.
3. Profile directory: `<Acta data dir>/webview` (Windows `%LocalAppData%\ActaData\webview`, ADR 0003's
   base, outside the Velopack root). macOS: default data store, tied to the bundle identifier. The profile
   is not encrypted by Acta, is excluded from export and snapshots (ADR 0006), and "Sign out and clear chat
   data" deletes it.
4. The web view is created on first opening of the panel, not at startup, and kept alive while hidden so
   the conversation survives toggling. If creation fails the panel shows one line and the link in 7.
5. Overlays: while any GPUI floating layer that can overlap the panel is open (palette, quick capture,
   dialog, menu, sidebar overlay), the web view is hidden (`WebView::set_visible(false)`, which also
   returns the keyboard to GPUI) and the panel shows `bg.sunken`; it comes back when the layer closes.
   Tooltips and toasts are placed so that they never reach into the panel.
6. Navigation allow-list, as in `spikes/chat-panel/src/allow.rs`: `https` on `claude.ai` and its
   subdomains, plus exactly `accounts.google.com` and `appleid.apple.com`. Anything else `http(s)` opens in
   the system browser, other schemes are dropped, new-window requests and downloads are refused. The list
   is to be corrected after the first real sign-in.
7. Sign-in: email is the supported path (link opens in the system browser, code typed into the panel).
   The Google and Apple buttons are left to work or fail on their own; the user agent is never changed to
   make them pass. The panel always offers "Open claude.ai in your browser".
8. Windows: set `GPUI_DISABLE_DIRECT_COMPOSITION` only if the M5 work shows it is needed on screen.
9. The panel is labelled as a view of claude.ai, without Anthropic's logo, and Acta's documentation says
   it is the website in the system web view, not an Anthropic product.

## Alternatives
- System browser window positioned beside Acta: no engine and no sign-in problem, but moving another
  application's window needs per-OS window management, is impossible on Wayland, and breaks on every
  maximise, snap and virtual desktop. Rejected as a panel; kept as the plain link below.
- Plain link ("Open claude.ai in your browser"): free, always works, no integration. This is the Linux
  behaviour and the fallback everywhere.
- CEF / bundled Chromium: identical on three OSes and overlays could be composited, but 100 MB or more per
  platform, our own security updates, and Google names CEF as blocked for sign-in. Rejected.
- `gpui-fast` backend for overlays above the page: not on Windows, and ADR 0001 rejected the fork.
- Web view in a separate top-level window glued to the main window: fixes z-order, costs focus, taskbar
  and move/resize glitches. Not needed once the view is hidden under overlays.
- Chat through the API in native GPUI: a different product, needs an API key and money. Out of scope.

## Consequences
- M5 holds for Windows and, pending proof, macOS. It must start with three checks the spike could not
  make: keyboard (typing in the page; `Ctrl+Shift+K` and the panel toggle while the page has focus, which
  WebView2 does not forward to the host: the route is `ICoreWebView2Controller::AcceleratorKeyPressed`
  through `WebViewExtWindows::controller()`), a first sign-in by the user on each OS, and a macOS build.
- If the keyboard route fails, the panel can still be closed with the pointer but breaks DESIGN.md
  principle 4 (keyboard first); that would be a reason to stop.
- Memory is the price: about 360 to 430 MB private in 9 WebView2 processes for the login page alone, more
  with a conversation open, against 89 MB for the host. Hiding the panel does not free it. Paid only by
  users who open the panel.
- `WebView::build` creates the WebView2 environment synchronously on the UI thread: 180 to 340 ms measured.
  Acceptable once, on first open; move to wry's async builder if it shows.
- Linux users get a link, not a panel. Revisit when gpui-webview supports Wayland or when forcing X11 for
  the whole app is acceptable; the cost then is `libwebkit2gtk-4.1-0` as a hard runtime dependency of the
  AppImage and `libwebkit2gtk-4.1-dev` on the ubuntu-22.04 runner.
- Windows without the WebView2 runtime: panel shows the fallback line; Acta itself is unaffected. No
  bootstrapper is bundled.
- A second place where account cookies live on disk, protected only by the user profile's permissions,
  like any browser profile. CLAUDE.md states it as the one exception to "no plaintext user data on disk".
- We depend on a site we do not control: a bot check, a user-agent rule or a terms change can end the
  feature without notice. The panel must fail to the link, never to a blank pane.
- The panel's design (dock placement beside the terminal, toggle and shortcut, placeholder under overlays,
  failure and sign-in help states, copy) is in `docs/design/components-app.md` (Chat panel) and
  `docs/design/screens.md` 8.13.
- macOS ad-hoc signing (ADR 0004) gives a new code identity per build; whether WKWebView's data store and
  cookies survive an update under those conditions is unknown.

## Changelog
- 2026-11-20: proposed as "go with limits"; accepted with the conditions in the status line.
- 2026-11-20: the panel's design was specified; CLAUDE.md records the web view profile exception.
- 2026-11-21: macOS gets the link, like Linux, until wry tells main-frame navigations apart: on WKWebView
  its navigation handler runs for every frame with no main-frame flag, so the allow-list would refuse (and
  send to the browser) the page's own third-party frames. The panel ships on Windows only; the WKWebView
  path is not compiled. Decision 2 holds for what Acta adds; wry itself always adds one initialization
  script, a `window.ipc.postMessage` shim, to every page, and with no IPC handler attached what the page
  posts is dropped, so the page gains no capability from it. "Sign out and clear" that cannot delete the
  profile (files still held) leaves `chat_clear_pending` in preferences and deletes it at the next launch,
  before any web view; if that fails too the user is told.

## Evidence
Spike: `spikes/chat-panel` (standalone, own lockfile, README with the commands). Windows 11 26300, rustc
1.97.1 MSVC, WebView2 runtime 157.0.4341.60, release build. `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo test` (6 allow-list tests) pass. No sign-in was
attempted and no credentials were entered anywhere. Window geometry was read with `EnumChildWindows`,
pictures with `PrintWindow`.

Verified by running, Windows only:
- Hosting and resize: child window `WRY_WEBVIEW` at x=640 w=648 in a 1280 window; w=420 after `width 420`;
  w=768 after `width 2000` (60% clamp); `visible=False` after `toggle` and back.
- Z-order: a `deferred` GPUI overlay crossing the split is drawn under the web view. With the web view
  hidden while the overlay is open, the overlay is whole and the GPUI input takes focus; the page receives
  `blur`. The view returns at the same bounds afterwards.
- Focus calls: `focus()` gives the page a `focus` event, `focus_parent()` and hiding give it `blur`. With
  GPUI focused, a typed letter reached the GPUI input and `Ctrl+Shift+K` reached the GPUI action.
- Persistence: a local page (`test/persist.html` on 127.0.0.1) reported `persist fresh cookie=null ls=null`
  on the first run and `persist kept cookie=1795207542270 ls=1795207542270` after quitting and restarting
  with the same profile directory.
- claude.ai: `/` then `/login`, title and login form rendered at 648 and at 400 px wide; profile directory
  39.6 MB afterwards.
- Allow-list live: `load https://example.com/` and `load http://claude.ai/` were refused and the page
  stayed; `https://accounts.google.com/` was allowed (left immediately, nothing entered).
- Memory (private / working set): local test page 6 WebView2 processes, 141 / 317 MB; claude.ai login page
  9 processes, 431 / 715 MB after 12 s, 358 / 688 MB after 30 s hidden, 379 / 691 MB shown again. Host
  process 89 / 73 MB.
- Missing runtime, simulated with `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` pointing at an empty folder:
  `WebView::build` returned `HRESULT(0x80070002)` after 6 ms, the window opened and showed the message.
- Shutdown: after `quit` and after `WM_CLOSE`, the host and all its `msedgewebview2.exe` processes were gone.

Not verified:
- Keyboard while the page has focus (typing in the page, app shortcuts, IME). Synthetic keys need the
  foreground window and the machine was in use; the attempt was stopped. One observation from it: after the
  window lost and regained activation, focus was on the GPUI input, not on the page.
- Any sign-in: Google, Apple, email code, session lifetime, whether more hosts are needed, and whether
  WebView2 passes Google's check. Needs the user.
- Whether `GPUI_DISABLE_DIRECT_COMPOSITION` is needed: a `PrintWindow` capture with DirectComposition left
  on looked the same, which says nothing about the screen.
- A separate GPUI popup window above the web view (opened and closed without error, not looked at).
- Memory with a signed-in conversation, long sessions, several windows.
- A real machine without WebView2; Windows 10.
- macOS: nothing built or run. WKWebView as a child of the GPUI view, overlay hiding, data store
  persistence across restarts and ad-hoc signed updates, sign-in.
- Linux: nothing built or run. X11-only embedding, package names and versions, AppImage behaviour.
- Profile survival across a Velopack update (follows from the directory being outside the root; not run).

Sources: https://github.com/longbridge/gpui-kit/tree/main/crates/webview ·
https://learn.microsoft.com/microsoft-edge/webview2/concepts/user-data-folder ·
https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution ·
https://developers.googleblog.com/2020/08/guidance-for-our-effort-to-block-less-secure-browser-and-apps.html ·
https://developers.googleblog.com/upcoming-security-changes-to-googles-oauth-20-authorization-endpoint-in-embedded-webviews/ ·
https://support.claude.com/en/articles/13189465-logging-in-to-your-claude-account ·
https://www.anthropic.com/legal/consumer-terms · https://www.anthropic.com/legal/aup ·
https://v2.tauri.app/distribute/appimage/
