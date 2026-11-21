# Spike: agent browser (ADR 0010)

Throwaway. Not a workspace member, own lockfile. Windows 11 only. Proves that a process standing in for
`acta mcp` can drive an agent web view inside a running app, in-process through WebView2's
`CallDevToolsProtocolMethod` and `GetDevToolsProtocolEventReceiver`, over a per-user named pipe with a
mutual HMAC handshake. No DevTools port is opened and DevTools stay disabled.

## Run

```
cargo test --release                     # policy, rendering, handshake (portable parts)
cargo run --release                      # = demo: test server + host child process + client
```

`demo` starts a loopback HTTP server with the test pages (`src/http.rs`), spawns itself as `host` (the
app's stand-in) with a random secret in `SPIKE_SECRET` (environment, never arguments), then runs the
client (`acta mcp`'s stand-in, `src/client.rs`) and prints each step with PASS/FAIL, size against its
budget and time. `SPIKE_DIR` holds the two profiles and the screenshots (default
`%TEMP%\acta-spike-agent-browser`).

The host window is a `WS_POPUP` with `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`, placed to the right of every
monitor and shown with `SW_SHOWNOACTIVATE`; it answers `WM_MOUSEACTIVATE` with `MA_NOACTIVATE`. It never
takes focus and nobody sees it. No synthetic OS input: clicks and typing go through CDP `Input.*`, which
are delivered inside the renderer.

## Layout

| File | Role |
|---|---|
| `src/policy.rs` | Navigation policy (schemes, never-automated sites, loopback-only for the spike) and the secret-field rule. |
| `src/render.rs` | Accessibility snapshot to compact lines with `e<n>` refs; the `[web page <nonce>]` fence; budgets. |
| `src/ops.rs` | `navigate`, `snapshot`, `click`, `type`, `screenshot`, `console` over a `Cdp` trait, plus probes. |
| `src/auth.rs` | Key derivation and the mutual challenge-response. |
| `src/pipe.rs` | Named pipe: owner-only DACL, network denied, remote clients rejected, first instance; client with `SECURITY_IDENTIFICATION`. |
| `src/host.rs` | Window, agent web view (own profile), chat stand-in web view (other profile), UI-thread CDP bridge, pipe server. |
| `src/client.rs` | The scripted run. |

## What it proved (Windows 11 26300, WebView2 runtime 157, release build)

See `docs/decisions/0010-agent-browser.md`, Evidence. In short: wrong key refused; navigate, accessibility
snapshot, click (`isTrusted` true), type, console and JPEG screenshot all work through in-process CDP with
DevTools disabled; password fields refused; file:, claude.ai, off-list hosts, downloads and pop-ups blocked;
the agent view's CDP sees neither the chat profile's cookies nor its page; screenshots fail on a hidden
view but work on a "parked" one; the host window was never foreground.
