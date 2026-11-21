# 0010 - Agent browser (browser tools for Claude Code inside Acta)

Status: accepted (2026-11-21); built in M8 on Windows (see Implementation below).
Date: 2026-11-21

## Context
Claude Code often needs a browser: open a local dev server, read the page, click, fill a form, look at a
screenshot, read the console. Today that is "Claude in Chrome", an extension driving the user's own
Chrome. The user wants Acta's embedded web view to serve that purpose, so the work stays in one place.
Product principle (from the user): the user logs in as few times as possible, and Claude Code (on a
subscription, not the API) gets what it needs as directly as possible: terminal, chat, browser, embedded.
Facts checked on 2026-11-21; "read" means read in a source, "run" means observed in the spike.

**Claude in Chrome (CiC)**, code.claude.com/docs/en/chrome and support.claude.com articles 12012173,
12902428, 12902446 (read):
- Chrome and Edge (the Claude Code page also names Brave, Arc, Vivaldi, Opera; the support article says
  other Chromium browsers are not supported). Needs extension 1.0.36+, a direct Anthropic plan (Pro, Max,
  Ultra, Team, Enterprise), `/login`; off with an API key, Bedrock, Vertex, Foundry or ZDR. In beta.
- Off by default: `claude --chrome` or "Enabled by default" in `/chrome`, which "increases context usage
  since the browser tools and their instructions are always loaded". A project's settings cannot turn it on.
- Connects through a native messaging host registered on first use (Windows: an HKCU registry key) and a
  named pipe; the error table also names `bridge.claudeusercontent.com`.
- Per-site permissions (allow once, always on this site, decline); still asks before downloads,
  sensitive input and authorizations; login pages and CAPTCHAs pause for the user; uploads capped at
  10 MB and refuse credential-named files. Anthropic's safeguards (classifiers, action screening) apply.
- Tool set, read from the schemas of the extension's MCP server loaded in this session
  (`mcp__claude-in-chrome__*`): `navigate(url|back|forward, tabId)`, `read_page(tabId, filter
  all|interactive, depth=15, ref_id, max_chars=50000)` (accessibility tree with `ref_n` refs), `find(query)`
  (natural language, up to 20 elements), `computer(action: left_click|type|key|screenshot|scroll|zoom|hover|
  ..., coordinate|ref, scale)`, `form_input(ref, value)`, `get_page_text`, `read_console_messages(pattern,
  onlyErrors, limit=100)`, `read_network_requests(urlPattern, limit=100)`, `javascript_tool`,
  `tabs_context_mcp`, `tabs_create_mcp`, `tabs_close_mcp`, `browser_batch`, `gif_creator`, `file_upload`,
  `upload_image`, `resize_window`, browser selection tools. Default outputs are large (50,000 chars for
  `read_page`, 100 console or network items).
- Prompt injection: Anthropic reports attack success falling from 23.6% to 11.2% with mitigations at
  launch (claude.com/blog/claude-for-chrome, 2025-08), 1.4% for Opus 4.5 against an adaptive attacker
  (anthropic.com/research/prompt-injection-defenses, 2025-11), under 0.08% for Opus 4.8 now (support
  article 12902428), "still non-zero".

**Comparable open tools** (read): Playwright MCP (github.com/microsoft/playwright-mcp): `browser_snapshot`
(accessibility snapshot, elements addressed by a `target` ref), `browser_click(target)`,
`browser_type(target, text, submit)`, `browser_fill_form`, `browser_take_screenshot(scale)`,
`browser_console_messages`, `browser_network_requests`, `browser_tabs`, `browser_evaluate`; connects to
an existing browser with `--cdp-endpoint` or an extension; `--allowed-origins` "does not serve as a
security boundary"; persistent profile by default, `--isolated` in memory. Chrome DevTools MCP
(github.com/ChromeDevTools/chrome-devtools-mcp): `take_snapshot` (a11y tree, `uid` per element),
`click(uid)`, `fill`, `take_screenshot(format, quality, uid)`, `list_console_messages`/
`list_network_requests` with `pageIdx`/`pageSize`, `evaluate_script`; attaches with `--browserUrl`,
`--wsEndpoint` or `--autoConnect`; warns that it "exposes content of the browser instance to the MCP
clients". Both converge on the same shape: snapshot with short refs, act by ref, screenshots optional.

**WebView2** (learn.microsoft.com, SDK 1.0.4314.35 pages, read):
- `ICoreWebView2::CallDevToolsProtocolMethod(method, paramsJson, handler)` and
  `GetDevToolsProtocolEventReceiver(event)` speak CDP to the view's top-level page in-process; iframes from
  other origins and workers are separate targets (`CallDevToolsProtocolMethodForSession`). The domains are
  Chromium's, not documented by the WebView2 team.
- `--remote-debugging-port` is a browser flag ("Apps in production shouldn't use WebView2 browser flags").
  Microsoft documents attaching chrome-devtools-mcp to any WebView2 app by writing
  `HKCU\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments` `"<app>.exe" =
  "--remote-debugging-port=0"`: any process of the user can turn on an unauthenticated CDP port for Acta.
- One user data folder is one browser process; several profiles (`ProfileName`) can share it. Downloads
  can be cancelled (`DownloadStarting`), navigations cancelled (`NavigationStarting`), new windows handled
  (`NewWindowRequested`; the popup blocker is off in WebView2).

**Named pipes** (learn.microsoft.com, read): with no security attributes the default DACL gives Everyone
and anonymous read access; "If the server service is running, all named pipes are accessible remotely ...
deny access to NT AUTHORITY\NETWORK"; `PIPE_REJECT_REMOTE_CLIENTS` rejects remote clients;
`FILE_FLAG_FIRST_PIPE_INSTANCE` makes creation fail if the name already exists (Kestrel uses it against
pipe squatting).

**Acta today.** ADR 0005: `acta mcp` (stdio, plain-text results, budgets, a random-nonce data fence around
stored text) shipped as a local plugin with one allow rule. ADR 0009: the chat panel is WebView2 through
`gpui-webview`/`lb-wry` on Windows, profile `<data dir>/webview`, and Acta never reads from or writes to
that page. CLAUDE.md: no plaintext user data or key material on disk, in logs or in process arguments,
with the chat profile as the one exception.

## Decision
**Build an embedded agent browser on Windows, driven in-process through WebView2's DevTools Protocol
methods, reached by `acta mcp browser` over an authenticated per-user named pipe. Keep it strictly apart
from the claude.ai profile. Claude in Chrome stays usable alongside; Acta neither requires nor blocks it.**

1. **What the user sees.** A "Browser" tool window beside "Chat" in the same dock (ADR 0009 placement and
   overlay rules). Header: the address (read-only for the agent's page, editable by the human), the
   session driving it (focus topic id), **Stop**, and a menu with "Clear browser data". An activity line
   shows the agent's last action ("clicked e19 button Greet"); the element acted on is highlighted for
   300 ms with CDP `Overlay.highlightNode` (no script in the page). The human can click and type in the
   page at any time; the agent's next snapshot sees the result. Agent activity never takes focus and never
   opens the dock by itself: when the Browser tab is not visible, its header tab shows a busy dot.
2. **Two profiles, two user data folders.** Chat: `<data dir>/webview` (ADR 0009, unchanged). Agent:
   `<data dir>/agent-browser`, a separate WebView2 environment, hence a separate browser process. Never one
   folder with two `ProfileName`s: one browser process means CDP's `Target` domain and process-wide calls
   (`Network.getAllCookies`) could reach the chat page. The agent profile is persistent (sign in to a site
   once, by hand, and the agent reuses the session), excluded from export and snapshots (ADR 0006),
   deleted by "Clear browser data". The agent web view never opens `claude.ai`, `claude.com` or
   `anthropic.com`: the Claude account is never automatable.
3. **IPC.** The app owns `\\.\pipe\acta-<user SID>-browser`, created with `FILE_FLAG_FIRST_PIPE_INSTANCE`
   (a squatter makes the app report the error instead of joining it), `PIPE_REJECT_REMOTE_CLIENTS`, and the
   DACL `D:P(D;;GA;;;NU)(A;;GA;;;<user SID>)` (network logons denied, only the user admitted, nothing
   inherited). On top, a mutual challenge-response: the app sends a 32-byte nonce, the client answers
   with its own nonce and `HMAC-SHA256(k, "client" || nonces)`, the app answers with
   `HMAC(k, "server" || nonces)`. `k = HMAC(database key, "acta agent-browser ipc v1")`: both processes
   already read the database key from the keychain (ADR 0003), so nothing new is stored, shown, copied or
   passed in arguments, and a process that cannot open Acta's data cannot drive its browser. The client
   opens the pipe with `SECURITY_IDENTIFICATION` (no impersonation). One controlling connection at a time;
   another session's call gets one line, "the browser is in use by another Claude Code session". No TCP
   port, no DevTools port, DevTools UI disabled (`AreDevToolsEnabled = false`).
   - The app refuses to create the agent view, and says why, if the `AdditionalBrowserArguments` policy
     value for Acta's executable or a `DevToolsActivePort` file in either profile exists.
   - macOS and Linux (later): a Unix socket in a 0700 per-user directory (`$XDG_RUNTIME_DIR/acta` or the
     data directory), peer uid checked (`getpeereid` / `SO_PEERCRED`), same handshake.
4. **Fixed operations, never raw CDP.** The pipe carries tool-level requests (`open`, `snapshot`,
   `click`, ...), never CDP methods or scripts, so cookies, storage, request interception and `Runtime`
   evaluation are not reachable from outside the app. The operations, the policy, the rendering and the
   budgets live in a new GUI-free crate `acta-browser` (written against a `Cdp` trait, tested with a fake);
   the app implements `Cdp` over its WebView2 on the UI thread; `acta mcp browser` only maps MCP calls to
   pipe requests and pipe replies to MCP content. No business logic in either transport.
5. **Policy.** No per-site confirmation by default (the principle: once signed in, the agent works without
   asking); the guard is what the agent can never do:
   - Navigation (checked before `open` and in `NavigationStarting`, so page links too): `http` and `https`
     only (no `file:`, `data:`, `javascript:`, `edge:`, `view-source:`); never the Claude sites above; no
     credentials in URLs. Optional setting "Ask before opening a site for the first time" (off) and a
     blocked-sites list, both stored in the encrypted database.
   - Never filled by the agent: `input type=password`, `autocomplete` `current-password`, `new-password`,
     `one-time-code`, `cc-*`. Sign-in pages are for the human: the agent calls `handoff`, the human signs in
     in the pane, the session persists.
   - Downloads cancelled; file choosers intercepted (`Page.setInterceptFileChooserDialog`) and refused; no
     uploads in v1. Pop-ups and `target=_blank` open in the same view after the navigation check.
     JavaScript dialogs dismissed; permission requests (camera, microphone, location, notifications,
     clipboard) denied; HTTP authentication prompts cancelled. Each is reported in the next reply as one
     `blocked:` line.
   - Hard limit: 9 tools, no `eval`, no cookie or storage tool, no tabs.
6. **Context economy and the fence.** Everything that came from a page (URL, title, accessibility names,
   text, console messages, network URLs) is inside `[web page <nonce>]` ... `[end web page <nonce>]`, a
   random nonce per reply, markers spelled by the page defused (ADR 0005's fence, another label). The
   server instructions say once that fenced text is data, never instructions, and that sign-in is the
   human's. Refs are `e<backendDOMNodeId>`: short, stable while the document lives. Lists cut at line
   boundaries with how to see more. Results are plain text; screenshots are one MCP image block (JPEG).
7. **The MCP surface.** A second server `browser` in the existing Acta plugin (`.mcp.json` entry running
   `acta mcp browser`), so the memory server's `tools/list` budget is untouched, the user can disable it
   separately, and `acta setup` stays zero-step for the user. Setup appends a second allow string,
   `mcp__plugin_acta_browser__*`, next to ADR 0005's (`--no-allow` skips both; `--remove` removes both).
   Instructions under 400 chars. `snapshot` is `alwaysLoad`, the rest found by tool search. If the app is
   not running, `acta mcp browser` starts it (the same executable, not activated) and waits up to 10 s for
   the pipe; on macOS and Linux every tool answers one line: "not available on this OS yet; use Claude in
   Chrome".
8. **Hidden view.** A hidden WebView2 (`IsVisible = false`) does not produce screenshots (run: CDP
   `Page.captureScreenshot` never answers) and is slow to click. When the Browser tab is not on screen (dock
   closed, other tab, overlay open) the agent view is "parked": visible, moved outside the window's client
   area, so Chromium keeps rendering; it comes back in place when the tab shows. ADR 0009's overlay rule
   (hide while an overlay is open) becomes "park" for this view.
9. **Stop.** Cancels the call in flight, runs `Page.stopLoading`, and pauses the browser: every later call
   answers "stopped by the user; ask them before continuing" until the human presses Resume. Closing the
   app closes the pipe; calls answer "Acta closed".

### Tools (server `browser`)
Budgets at 4 chars per token; images at about width x height / 750 tokens. Every text reply stays under
ADR 0005's 2,000-token cap, enforced by the shared renderer and tested like ADR 0005's tools.

| Tool | Parameters | Returns | Budget |
|------|------------|---------|--------|
| `open` | `url` or `back`/`forward`/`reload` | Load state, then the fenced URL and title. Waits for load, 15 s max. | 100 tok |
| `snapshot` | `filter?` (all, interactive), `within?` (ref), `wait_for?` (text, 10 s max), `max?` (chars, max 8,000) | Fenced accessibility outline, one element per line: `e19 button "Greet" [focused]`; text nodes quoted; cut with "+N more lines". | 1,500 / 2,000 tok |
| `read` | `within?` (ref), `cursor?` | The page's readable text, fenced, a page at a time with a cursor. | 1,500 tok |
| `click` | `ref`, `double?` | Ack; notes if the point hit another element, and any `blocked:` line. | 60 tok |
| `type` | `ref`, `text?` (max 2,000), `clear?`, `key?` (Enter, Tab, Escape, arrows) | Ack; refusal for secret fields. | 60 tok |
| `screenshot` | `ref?` (scrolls it into view and clips to it), `width=1024` (320 to 1568) | One JPEG (quality 70) and a one-line caption. | ~1,000 image tok at default |
| `console` | `level?` (error, warning, all), `pattern?`, `limit=20` (max 50) | New messages since the last call, fenced, 160 chars each. | 500 tok |
| `network` | `pattern?`, `failed?`, `limit=20` (max 50) | Requests since the last call: method, status, type, size, URL (query cut), fenced. No bodies or headers. | 500 tok |
| `handoff` | `message` (max 200 chars) | Shows the message and a Done button in the Browser pane, waits up to 10 min: `done`, `stopped` or `timeout`. For sign-in, CAPTCHA, or anything the agent must not do. | 20 tok |

Left out: `eval`/JavaScript (revisit after dogfooding, possibly loopback origins only), tabs, uploads,
downloads, cookies and storage, natural-language `find` (the interactive snapshot is cheap), GIF
recording, coordinate clicks (refs only; a page that needs them gets `handoff`), batch calls.

### Logins
A user of Acta signs in: to Claude Code (outside Acta, once per machine); to claude.ai once in the chat
panel (ADR 0009, persistent); to each site the agent needs, once, by hand in the Browser pane, after which
the session persists in the agent profile until the site expires it. Acta itself has no account, the
pipe needs no token, the plugin needs no step. Total: 2 + one per site. It cannot be lower without
handing the agent the chat profile (never) or the user's everyday browser (that is Claude in Chrome).

## Alternatives
- **Keep using Claude in Chrome, build nothing.** Free, maintained by Anthropic with its own injection
  classifiers and site permissions, real Chrome with tabs, GIFs and uploads, works on macOS. Against it:
  outside Acta (another window, an extension, `--chrome` per session or always-loaded tools), large default
  outputs, beta, and it drives the user's everyday browser with every session they have (mail, bank), which
  a prompt injection can reach. The embedded browser is narrower and compact, holds only the sites the user
  signed into for the agent, and lives beside the work. We lose Anthropic's classifiers and own the CDP
  code. Both can be used; the skill says which to prefer: Acta's for local dev servers and sites signed in
  there, Claude in Chrome when the user's own browser session is the point.
- **`--remote-debugging-port` plus Playwright MCP or chrome-devtools-mcp.** No code of ours, but an
  unauthenticated CDP endpoint on loopback for every local process (and pages can probe localhost), full
  CDP including cookies of the whole profile. Rejected; the app actively checks the policy key above.
- **One user data folder, two `ProfileName`s.** One browser process: CDP `Target` and process-wide calls
  can cross. Rejected (decision 2).
- **Script-based automation (`evaluate_script`, init scripts).** Page-visible, `isTrusted` false, the
  page can tamper; also against ADR 0009's no-script rule. CDP `Input.*` gives trusted events.
- **Loopback HTTP/WebSocket server with a token** (or an MCP HTTP server in the app). A port visible to
  every local user and to browser pages (DNS rebinding, CSRF to localhost); a token on disk or in Claude
  Code's config. The pipe has an OS DACL and needs no stored secret.
- **Token file shared by app and CLI.** Key material on disk, against CLAUDE.md; deriving from the
  database key needs nothing new.
- **Raw CDP passthrough over the pipe.** Makes every CDP capability reachable to whatever holds the pipe.
  Rejected (decision 4).
- **Per-site confirmation by default (CiC's default).** Safer against injection, against the user's
  principle; kept as an off-by-default setting.
- **Reusing the chat panel's web view.** The one session that must never be automatable. Rejected.
- **Browser tools in the existing `acta` server.** Covered by the existing wildcard and inflating its
  `tools/list`; a second server keeps the two apart.

## Consequences
- CLAUDE.md's exception for the web view profile must name the agent profile too (lead's change):
  cookies of sites the user signs into for the agent live in `<data dir>/agent-browser`, protected only by
  the user profile's permissions, like any browser profile.
- With the allow rule, a prompt-injected agent can browse any public site and act in sessions the user
  opened in the agent profile, without a prompt. Mitigations are decisions 2, 4, 5, 6 and 9; residual risk
  is stated in the setup text and the docs. Users who want prompts pass `--no-allow` or turn on "Ask
  before opening a site".
- Memory: a second WebView2 environment, about 140 MB private for a local page (ADR 0009's measure), paid
  only once the agent uses the browser. Created on first use, closed after 10 min idle (profile kept).
- CDP is Chromium's and moves; the operations use stable domains (`Page`, `DOM`, `Input`,
  `Accessibility`, `Runtime` events, `Overlay`, `Network` events). Cover them with an end-to-end test like
  the spike on CI's Windows runner (WebView2 is installed there; unverified).
- Cross-origin iframes are separate targets: their content is missing from `snapshot` in v1. Revisit with
  `Target.setAutoAttach` and `CallDevToolsProtocolMethodForSession` if dogfooding needs it.
- New work: crate `acta-browser`; the Browser tool window (product-designer specifies it in
  `docs/design/components-app.md`); `acta mcp browser`; plugin `.mcp.json` and setup changes; skill text.
- macOS and Linux get nothing until ADR 0009's web view exists there. WKWebView and WebKitGTK have no CDP:
  the path would be scripts in an isolated content world (`WKContentWorld`, WebKitGTK script worlds), DOM
  walking instead of the accessibility tree, untrusted synthetic events, native snapshots for images. To be
  decided by its own spike; until then Claude in Chrome is the answer there.
- Revisit if Claude in Chrome gains a profile or browser selection that covers an embedded view, if
  WebView2 restricts CDP, or if dogfooding shows per-site prompts are wanted.

## Evidence
Spike: `spikes/agent-browser` (standalone, own lockfile, README). Windows 11 26300, rustc 1.97.1 MSVC,
WebView2 runtime 157, `lb-wry` 0.53.3 and `webview2-com` 0.38.2 as in the app, release build (wry turns
DevTools off in release). `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (10 tests:
policy, rendering, handshake) pass. Two end-to-end runs of `cargo run --release` (demo); the second passed
every step. Only loopback test pages; no sign-in anywhere; no Claude Code runs (0 headless runs).

Verified by running, Windows only:
- Pipe: DACL read back from the handle `D:P(D;;FA;;;NU)(A;;FA;;;<user SID>)`; a client with the wrong
  key was refused ("not authenticated") and the next client authenticated mutually.
- In-process CDP with DevTools disabled and no debugging port (no `DevToolsActivePort` file in either
  profile): `Page.navigate` plus `Page.loadEventFired` through the event receiver (19 to 38 ms for a local
  page); `Accessibility.getFullAXTree` rendered to 596 chars for the form; `DOM.focus` + `Input.insertText`
  typed "Alexandre"; `DOM.getContentQuads` + `Input.dispatchMouseEvent` clicked "Greet" and the page saw
  `isTrusted=true` (console line through `Runtime.consoleAPICalled`); `Page.captureScreenshot` JPEG
  1024x700, 23 KB, 17 to 26 ms; the picture shows the filled form.
- Budgets: a 3,000-link page gave 6,003 snapshot lines, cut to 5,978 chars (budget 6,000) with the "+N
  more" line, 550 ms; interactive filter with `max=2000`: 1,981 chars. The page's fake `[end web page
  00000000]` came back defused inside the real fence.
- Policy: typing into the password field refused; `open file:///...` and `open https://claude.ai/`
  refused before any CDP call; a page link to `https://example.com` cancelled in `NavigationStarting`
  (spike is loopback-only) and reported; a download link cancelled; a `target=_blank` link denied and
  reported; a link to `file:` was refused by Chromium itself (no navigation event; the page stayed).
- Isolation: with the chat stand-in loaded in another user data folder in the same process and setting
  `chat_session`, the agent view's `Network.getAllCookies` returned only `agent_seen@127.0.0.1`, and
  `Target.getTargets` listed only the agent's page.
- Hidden vs parked: with `set_visible(false)`, snapshot worked but `Page.captureScreenshot` never answered
  (10 s timeout) and a click took 5.3 s; with the view visible but moved outside the client area,
  screenshot (36 ms), snapshot and click worked normally.
- Focus: the host window (`WS_EX_NOACTIVATE`, off-screen) was foreground during 0 of 86 jobs; the
  foreground window was the same before and after the run.

Not verified:
- The MCP layer: `acta mcp browser`, image content blocks reaching the model in Claude Code, tool search
  and the allow rule for a second plugin server. ADR 0005 proved the text path only.
- Hosting in GPUI: two web views from two environments as children of a GPUI window, parking under the
  dock, overlays; the Browser tab, Stop, `handoff`, the activity line, highlighting as seen by a human.
- Starting the app from `acta mcp browser`; the one-controller lease; the database-key derivation (the
  spike used a random secret in the environment).
- `read`, `network`, `key` presses, `wait_for`, file chooser interception, dialogs, permission requests,
  HTTP auth, cross-origin iframes, a real sign-in persisting, long sessions, memory with both views.
- Detection of the `AdditionalBrowserArguments` policy value; another user or a network client being
  refused by the DACL (only one local user was available).
- Image token cost (width x height / 750 is Anthropic's published estimate, not measured here).
- macOS and Linux: nothing built or run.

### Implementation (M8, 2026-11-21)
Crate `acta-browser` (operations over a `Host` trait, policy, fence and budgets, event store, handshake,
server with the lease, Windows pipe and client); the app's Browser view (`browser_panel.rs`,
`browser_host.rs`); `acta mcp browser` (`crates/acta-cli/src/browser.rs`); setup and skill. Deviations
from the text above, all small:
- The pipe name ends with a tag of the data directory, `\\.\pipe\acta-<user SID>-browser-<8 hex
  of SHA-256 of the data dir path>`, so a development or test instance never meets the installed app.
- `open` takes `url` or `go` (`back`, `forward`, `reload`) instead of three booleans.
- The lease is taken over by another session after 5 minutes without a call, so a session that went
  away never holds the browser for good.
- `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` asking for a debugging port refuses the view too.
- The view is created with `with_focused(false)`, and setup takes a one-pixel capture: run, a new view
  made Acta the foreground window when Windows allowed it, and dropped input until its first frame.
- The blocked-sites list exists in the policy (hosts and their subdomains) but has no setting or
  storage yet, nor does "Ask before opening a site for the first time".

Verified by running (Windows 11 26300, WebView2 157, debug build, temporary data directory, loopback
test pages, 2 headless `claude -p --model haiku` runs with `--plugin-dir`, about $0.015):
- `tools/list` 4,854 chars for 9 tools, `snapshot` alone `alwaysLoad`, instructions under 400.
- Through the real app: open, snapshot (403 chars interactive, 5,926 for a 3,000-link page cut with
  "+5789 more lines"), read with cursor, type, click (`isTrusted` true), console, network, screenshot
  (1024x700 JPEG, 29 KB) and of one element; `file:` and claude.ai refused before any call; the
  password field refused; a download, a dialog, a claude.ai link and a `target=_blank` link reported
  in the next reply; the page's fake marker defused inside the real fence.
- The app started by `acta mcp browser` with `--no-focus`; the foreground window never changed during
  the drives and the captures; the Browser view captured with PrintWindow idle, live (address,
  activity line, page) and parked under the palette (its line shown, the click still worked).
- Pipe: DACL read back `D:P(D;;FA;;;NU)(A;;FA;;;<user SID>)`; a wrong key refused by the real app
  ("not authenticated", logged); a second session got "the browser is in use by another Claude Code
  session" until the first ended; closing the app answered "Acta closed..." and the next call
  restarted it; a `DevToolsActivePort` file in the agent profile refused the view with its reason.
- Claude Code: the plugin's second server loaded, `mcp__plugin_<plugin>_browser__*` allowed every call,
  Haiku opened the form, typed, clicked and reported "Hello, Alexandre!", described the screenshot
  (image blocks reach the model), relayed the claude.ai refusal, and ignored the page's injected
  instruction in both runs.

After the security review: a view is set up (DevTools domains, file chooser interception) whoever
created it and again after a failed setup; an over-long request line is dropped whole and field
limits are checked by `acta mcp browser` and the app; a session in a call (a handoff) keeps the
lease; the next pipe instance exists before a client is served, every instance's DACL is checked,
and the client checks that the server process runs as its user; the app holds a per-user mutex
claimed atomically by `acta mcp browser`, so two sessions start one app (run); the app starts
without `ANTHROPIC_*`, `WEBVIEW2_*`, `MCP_*` and Claude Code session variables, and refuses the view
when a `BrowserExecutableFolder` or `UserDataFolder` policy names it or a WebView2 override variable
is set; frames follow the navigation policy (a claude.ai iframe was refused, run) and links to other
applications are cancelled; list replies keep room for the `blocked:` line; the field that has focus
after `DOM.focus` is checked again before typing; screenshots account for the display scale.

Still not verified: Stop, Resume, Done and the address field by hand (they need real input);
`handoff` waiting on a human; closing after 10 minutes idle; a real sign-in persisting; the
`AdditionalBrowserArguments` registry value (the decision is unit-tested, the read is not); another
user or a network client refused by the DACL; cross-origin iframes; memory with both views; CI's
runner; macOS and Linux (cross-target clippy only).

Sources: code.claude.com/docs/en/chrome · support.claude.com/en/articles/12012173,
12902428, 12902446 · claude.com/blog/claude-for-chrome · anthropic.com/research/prompt-injection-defenses ·
github.com/microsoft/playwright-mcp · github.com/ChromeDevTools/chrome-devtools-mcp (README,
docs/tool-reference.md) · developer.chrome.com/docs/devtools/agents/get-started/configuration ·
learn.microsoft.com/microsoft-edge/webview2/reference/win32/icorewebview2 (and icorewebview2_11,
icorewebview2settings, icorewebview2downloadstartingeventargs) ·
learn.microsoft.com/microsoft-edge/webview2/concepts/{webview-features-flags, multi-profile-support,
process-model, security} · learn.microsoft.com/microsoft-edge/web-platform/devtools-mcp-server ·
learn.microsoft.com/windows/win32/ipc/{named-pipe-security-and-access-rights, named-pipes} ·
learn.microsoft.com/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew ·
learn.microsoft.com/aspnet/core/fundamentals/servers/kestrel/security-considerations.
