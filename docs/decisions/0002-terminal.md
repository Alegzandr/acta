# 0002 - Terminal embedding

Status: accepted, except point 4: bundling ConPTY is deferred to M4 and done only if the system ConPTY misbehaves on a stable Windows build (the bundled path is untested and adds packaging weight)
Date: 2026-11-20

## Context
M4 embeds a terminal that runs Claude Code (`claude`, a TUI using truecolor, alt screen, bracketed paste,
SGR mouse, focus events, the kitty keyboard protocol, synchronized output) in the active project's directory.
Facts checked on 2026-11-20:

- **`alacritty_terminal` 0.26.0** (2026-04-06, Apache-2.0, MSRV 1.85): emulator, grid with scrollback and
  reflow, selection, damage tracking, plus its own PTY module (`tty`: ConPTY on Windows, openpty on Unix) and
  a reader thread (`event_loop`). Upstream is active (last commit 2026-11-18). No semver promise: every 0.x
  release may break the API. https://crates.io/crates/alacritty_terminal
- Zed uses the same crate through a fork (`zed-industries/alacritty` rev `4c129667`), which proves the
  design on all three OSes. https://github.com/zed-industries/zed/blob/main/Cargo.toml
- **Zed licensing**: `crates/terminal` and `crates/terminal_view` are **GPL-3.0-or-later**; `crates/gpui` is
  Apache-2.0. We may study how Zed does it; we must not copy or port code from those two crates.
  Alacritty itself (including its key-to-escape encoding in `alacritty/src/input/keyboard.rs`) is Apache-2.0
  and may be adapted with attribution.
- `portable-pty` 0.9.0 (MIT) last released 2025-02-11. On Windows it opens ConPTY with `INHERIT_CURSOR` and
  `WIN32_INPUT_MODE`, so the host must answer a `CSI 6n` at startup and receives `CSI ?9001h` (both seen in
  the spike). `alacritty_terminal::tty` passes no flags.
- Both PTY layers load a side-by-side `conpty.dll` (+ `OpenConsole.exe`) if present and fall back to the
  system ConPTY. Zed ships the pair from the MIT `Microsoft.Windows.Console.ConPTY` 1.25 package.
  https://github.com/zed-industries/zed/blob/main/script/bundle-windows.ps1
- Existing GPUI terminal crates are all young: `gpui-terminal` 0.1.0 (one release, Dec 2025, no selection or
  scrollback navigation), `gpui_xterm` 0.3.0 (5 stars, tied to `gpui-kit`), `bezel-terminal` and
  `ely-terminal` (tied to GPUI forks), `gpui-libghostty` (alpha, needs Zig, no Windows mentioned).
- Claude Code installs as a native binary on every OS (`~/.local/bin/claude[.exe]`, Homebrew, WinGet, apt);
  the npm package links the same binary but can leave a `.cmd` shim on Windows.
  https://code.claude.com/docs/en/setup
- Claude Code needs no terminal configuration; Shift+Enter works out of the box in terminals implementing
  the kitty keyboard protocol (Claude Code >= 2.1.269). https://code.claude.com/docs/en/terminal-config

## Decision
1. **One dependency for emulation and PTY**: `alacritty_terminal = "0.26.0"` from crates.io, using its
   `tty` module and `EventLoop`. No `portable-pty`, no Zed fork. `Term` is created with
   `kitty_keyboard: true` and 10 000 lines of scrollback.
2. **Write our own GPUI view and input mapping** (roughly 1.5-2k lines) in an `acta_terminal` crate split in
   two: a GPUI-free session module (spawn, resize, input encoding, events; unit-tested) and a GPUI element.
3. **Launch `claude` directly, not through an interactive shell.** Resolve the program once per launch:
   - Windows: search `PATH` (+ `%USERPROFILE%\.local\bin`) for `claude.exe`, then `claude.cmd`; spawn the
     exe directly, a `.cmd` through `%ComSpec% /d /s /c`.
   - macOS/Linux: a GUI app does not inherit the shell's `PATH`, so spawn `$SHELL -l -i -c 'exec claude'`,
     falling back to `~/.local/bin/claude`, `/opt/homebrew/bin`, `/usr/local/bin`.
   - Working directory = project directory. Environment: inherited, plus `TERM=xterm-256color`,
     `COLORTERM=truecolor`, `TERM_PROGRAM=Acta`, `TERM_PROGRAM_VERSION`. No secrets in arguments.
   - When the child exits, keep the last screen and offer "restart"; do not fall back to a shell.
4. **Windows**: ship `conpty.dll` and `OpenConsole.exe` next to `acta.exe` (decided with ADR 0004) so
   behaviour does not depend on the Windows build; the system ConPTY remains the fallback.

### Rendering plan (GPUI)
- A custom `Element`. `prepaint` measures the monospace cell (`em_advance`, line height), derives
  cols x rows from the bounds and, when they change, resizes `Term` first and then the PTY (debounced).
- Under one short `Term` lock, copy the visible cells into a frame snapshot; never shape or paint while
  holding the lock (the reader thread needs it).
- Per row: merge adjacent cells with the same background into one `paint_quad`; merge adjacent cells with
  the same font/colour/flags into one `TextRun` and shape with `shape_line`, positioning each run at
  `column * cell_width` so the grid never drifts. Wide cells take two columns; spacer cells are skipped.
- Draw box-drawing and block characters as quads so Claude Code's borders and logo join seamlessly.
- Then selection quads, underlines/strikeout, hyperlink hover, and the cursor (block, bar, underline,
  hollow when unfocused) on top.
- Input: `on_key_event` maps keystrokes to bytes according to `TermMode` (app cursor, kitty flags);
  `on_mouse_event` either reports SGR mouse to the child or drives local selection (Shift forces local);
  paste is wrapped when `BRACKETED_PASTE` is set; focus in/out is reported when `FOCUS_IN_OUT` is set;
  IME goes through an `InputHandler` anchored at the cursor cell.
- Performance: the reader thread sends one coalesced wake-up per batch; the view calls `notify` at most
  once per frame. Synchronized output (`CSI ?2026`) is already buffered by the parser, so partial frames are
  never visible. Cache shaped lines per row keyed by the row's content hash, using `Term::damage()` to skip
  unchanged rows. Budget: a full 120x40 repaint under 2 ms on the UI thread.

## Alternatives
- `portable-pty` + `alacritty_terminal`: works (spike), but a second dependency, stale releases, and
  ConPTY flags we do not want; we would also rewrite the reader loop Alacritty already provides.
- Zed's `alacritty` fork: unversioned git dependency carrying Zed-specific changes we do not need.
- `wezterm-term`: not published on crates.io; git-only, large dependency tree.
- `vt100` 0.18.1: parser and screen only; no PTY, no reflowing scrollback, no kitty keyboard state.
- `libghostty-vt` 0.2.2 (Rust bindings): promising now that libghostty has a stable C ABI, but needs Zig
  0.17 at build time on all three CI runners and Windows is barely documented. Revisit after M4.
- Depending on an existing GPUI terminal crate: none is mature, most pin a GPUI fork or version we do not
  control; `gpui-terminal` (MIT/Apache) is useful as permissively licensed reading.
- Running a login shell with `claude` typed into it: slower start, shell prompt noise, and exit lands the
  user in a shell we do not want to support.

## Consequences
- We own the view and the key/mouse encoding: the main M4 cost, and the main place for bugs (IME, dead keys,
  AltGr on Windows, Option-as-Meta on macOS, font fallback width for emoji and CJK).
- `alacritty_terminal` upgrades are deliberate, reviewed changes; keep all uses behind the session module.
- The reply path is ours: `Event::PtyWrite`, `ColorRequest` and `TextAreaSizeRequest` must be written back
  to the PTY or Claude Code's start-up queries go unanswered.
- Bundling OpenConsole adds about 1-2 MB and two files to the Windows installer.
- Revisit if `alacritty_terminal` stalls, or once `libghostty-vt` has prebuilt Windows artifacts.

## Evidence
Spike: `spikes/terminal-pty` (`cargo run`, `cargo run -- portable`, `cargo run -- probe`). Headless,
`alacritty_terminal` 0.26.0, run on **Windows 11 build 26300 only**, rustc 1.97.1, system ConPTY (no bundled
`conpty.dll`), `claude` 2.1.329 (native `claude.exe`). Each run uses a fresh temp working directory.

- (a) `cmd.exe` at 120x40: `echo`, `cd` (prints the temp dir, so the working directory is honoured) and
  wide CJK text land in the grid correctly.
- (b) `claude --version`, spawned directly: grid shows `2.1.329 (Claude Code)`, exit status 0 is reported.
- (c) Interactive `claude`, no input sent: recognisable first screen (logo, version, prompt box, status
  line) 0.4-1.2 s after spawn; 427 truecolor cells; the emulator reports alt screen, bracketed paste, focus
  events, SGR mouse and kitty keyboard all enabled, and answers Claude's kitty keyboard query. No trust or
  login dialog appeared on this machine. Dropping the session kills the child within 1 s.
- (d) Resize to 100x30: a shell child reports `100x30`, a 113-character line re-wraps at column 100, and
  Claude Code repaints its full-width rules at exactly 100 columns.
- `probe`: a child writing `CSI ?2026h/l`, `?2004h`, `?1004h`, kitty `CSI >1u`, truecolor SGR and OSC 8
  gets all of them through the system ConPTY unchanged on this build.
- `portable`: the same first screen through `portable-pty` 0.9.0, plus the extra `CSI 6n` and `CSI ?9001h`.

Not proven: macOS and Linux (nothing was run there), older Windows 10/11 ConPTY builds, the `.cmd` shim
path, the bundled OpenConsole, typing/mouse input, behaviour under heavy streaming output, and all GPUI
rendering (the spike has no UI).
