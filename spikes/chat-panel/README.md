# Spike: claude.ai chat panel (ADR 0009)

Throwaway. A GPUI window (gpui-kit 0.10.0) with a GPUI pane on the left and the system web view docked
right through `gpui-webview` 0.10.0 (wry: WebView2 / WKWebView / WebKitGTK). Not a workspace member, own
lockfile. Run on Windows 11 only; macOS and Linux were not built.

## Run

```
cargo run --release                      # loads https://claude.ai with a per-user profile directory
cargo test                               # navigation allow-list (src/allow.rs)
```

Environment:

| Variable | Meaning |
|---|---|
| `SPIKE_URL` | Page to load (default `https://claude.ai`). An `http` host named here is allowed in the panel. |
| `SPIKE_PROFILE` | Web view data directory. Default: `%LocalAppData%\ActaSpikeChatPanel\webview` (or the macOS / XDG equivalent). |
| `SPIKE_CMD` | File polled every 100 ms for one command; the file is deleted once read. |
| `SPIKE_LOG` | File that receives the event log (also printed). URLs are logged without query strings. |
| `SPIKE_KEEP_DCOMP` | Windows: do not set `GPUI_DISABLE_DIRECT_COMPOSITION`. |

Commands: `width <px>`, `toggle`, `overlay` (GPUI overlay, web view untouched), `palette` (same overlay,
web view hidden while open), `popup` (separate GPUI popup window), `focus-web`, `focus-gpui`, `load <url>`,
`quit`. The split handle can also be dragged.

Persistence test, no account involved:

```
python test/server.py test server.log    # serves test/persist.html on 127.0.0.1:8777, logs the user agent
SPIKE_URL=http://127.0.0.1:8777/persist.html SPIKE_PROFILE=<dir> cargo run --release
```

The page sets a cookie and a localStorage value when they are missing and writes what it found into its
title, which the spike logs: `persist fresh ...` on the first run, `persist kept cookie=... ls=...` after.

Do not sign in to anything from this spike.

## What it proved (Windows 11, WebView2 runtime 157.0.4341.60)

- The web view is a child window of the GPUI window and follows the panel: 648 px by default, 420 after
  `width 420`, clamped to 60% of the window, hidden and shown by `toggle`.
- A GPUI overlay in the same window is drawn under the web view. Hiding the web view while the overlay
  is open fixes it and returns the keyboard to GPUI.
- Cookies and localStorage survive a restart when the profile directory is set.
- `https://claude.ai` redirects to `/login` and renders (Google, Apple and email sign-in offered) with the
  stock WebView2 user agent. No sign-in was attempted.
- Navigations to hosts off the allow-list are refused in the panel (the spike only logs them).
- With no usable WebView2 runtime the web view fails with an error and the window still opens.
- Memory: see ADR 0009.

Not proved here: typing into the page and app shortcuts while the page has the keyboard, a real sign-in,
anything on macOS or Linux. See `docs/decisions/0009-claude-chat-panel.md`.
