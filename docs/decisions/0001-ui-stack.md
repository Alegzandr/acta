# 0001 - UI stack

Status: accepted (macOS and Linux still to be proven in CI during M1; binary layout for the CLI is settled in ADR 0004)
Date: 2026-11-20

## Context
Hard constraint: Rust + GPUI on Windows, macOS, Linux. Facts checked on 2026-11-20:

- **Official `gpui` on crates.io is stale**: 0.2.2, published 2025-10-22, Apache-2.0. Zed's `main` still says
  `version = "0.2.2"` while the code moved on (platform code split into `gpui_platform`, `gpui_windows`,
  `gpui_linux`, `gpui_macos`, `gpui_wgpu`; the README now starts apps with `gpui_platform::application()`).
  `gpui_platform` is not on crates.io, so the documented official path only works as a git dependency.
  Zed pins Rust 1.98.1 in `rust-toolchain.toml`; no crate declares `rust-version`.
- **`gpui-pre` snapshots**: Longbridge republishes Zed's GPUI crates weekly as `gpui-pre-*` (Apache-2.0, Zed
  notices intact). 0.3.12 = `zed@d2a4fb6ded7c` (2026-11-08); 0.3.13 (2026-11-16) exists but nothing pins it yet.
  Single crates.io owner (`huacnlee`), not Zed.
- **Components**: `longbridge/gpui-component` was renamed `longbridge/gpui-kit` (21k stars, pushed today,
  weekly releases, Apache-2.0). `gpui-kit` 0.10.0 (2026-11-16) is a facade over `gpui-base` (unstyled behavior),
  `gpui-component` (styled, 75+ components) and `gpui-kit-assets` (Lucide icons). It pins `gpui-pre =0.3.12`
  exactly, under the renamed package, so **it cannot be combined with a git or official `gpui`**.
- **What it has that we need**: `Input`, `Textarea` (multi-line, `auto_grow(min, max)`, undo, IME), `Editor`
  (rope-based, optional tree-sitter incl. markdown), `list`, `virtual_list`, `popover`, `command` (palette),
  `resizable`, `dock`, `title_bar`, `window_border` (Linux CSD), `scroll`, `menu`, `tooltip`, `dialog`.
- **Theming**: one global `Theme` with ~135 colour tokens (`ThemeColor`), radius, shadow, focus ring, font
  family/size, mono font; edited at runtime with `Theme::update`, or loaded from a JSON theme file.
- **Backends in the pinned snapshot** (read from source, not run except Windows):
  Windows = Win32 + Direct3D 11 + DirectWrite, IME via IMM, Mica/blur backdrops, PerMonitorV2 manifest.
  Linux = wgpu (Vulkan) for both X11 and Wayland, IME via XIM and text-input-v3, blur only on KDE Wayland.
  macOS = Metal, needs `font-kit` (enabled by gpui-kit) or no glyphs render.
- **Open upstream issues that touch us**: Windows IME composition cannot be cancelled on focus loss
  (zed#67180) and candidate window misplaced (#64850); X11 IME dies after an XIM error (#60578); Wayland
  blurry at fractional scale on some compositors (#65032); GNOME CSD window width drifts on restore (#65132).
- Dependency licenses for the Windows target (460 packages): MIT/Apache/BSD/Zlib/Unicode-3.0, one MPL-2.0
  (`option-ext`), one CC0. No GPL.

## Decision
1. Depend on **`gpui-kit = "=0.10.0"`** from crates.io, default features, and nothing else for UI. It brings
   `gpui-pre =0.3.12`. Import GPUI as `gpui_kit::*`. Commit `Cargo.lock`; build with `--locked`.
2. Pin the toolchain in `rust-toolchain.toml` to the current stable release (1.100.0). `gpui-pre`'s macOS
   backend calls `map_or_default`, stable since Rust 1.98.0. Bump the pin when a dependency needs it, and fix
   new clippy lints rather than allowing them.
3. Use gpui-component's behavior-heavy pieces (inputs, textarea/editor, virtual list, popover, command,
   resizable) and **own the look**: the design tokens (`docs/design/color.md` 2.1) are written into `Theme` at startup from one module.
   Where a styled component cannot be made to match, build ours on `gpui-base` primitives or plain `div`s;
   no component is used with its default look unreviewed.
4. Notes and timeline entries use `Textarea`; revisit `Editor` + `tree-sitter-markdown` in M5.
5. Bundle fonts with `include_bytes!` + `cx.text_system().add_fonts`.
6. Windows: `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`; icon through
   `winresource` in `build.rs` with **no manifest of our own** (GPUI embeds one).
7. Upgrades are deliberate: bump `gpui-kit` on a branch, read its release notes, run all three OS jobs.

## Alternatives
- Official `gpui` 0.2.2 from crates.io: 13 months old, pre-split API, no component library tracks it.
- `gpui` as a pinned git rev of zed: official source, but incompatible with gpui-kit, so we would write every
  input, IME path and virtual list ourselves; also a very large git fetch in CI.
- `gpui-fast` feature (Longbridge's GPUI fork): further from upstream, five weeks old. Not now.
- `gpui-base` only, all visuals ours: viable fallback if theming proves too shallow; costs weeks up front.
- Community forks (`gpui-ce`, `open-gpui`): small user base, no advantage for us.

## Consequences
- We depend on Longbridge's cadence and on one crates.io owner for the GPUI snapshot. Mitigation: exact pins,
  lockfile, `cargo vendor` before 1.0 if this worries us. Exit path: `gpui-base`-only, or a zed git rev.
- 0.x everywhere and weekly releases: expect API breaks on each bump. The `Theme` struct still carries a
  "legacy tokens" surface, so theming code may need rework on upgrade.
- Zed already requires Rust 1.98 and `gpui-pre` 0.3.12 does too on macOS; a future snapshot may raise our
  minimum toolchain again.
- Cost measured on Windows (Ryzen 7 9850X3D, 16 threads): clean `--release` build 1m47s, 460 crates,
  exe 25.4 MB (thin LTO, stripped), 74 MB private memory idle, `target/` 2.5 GB. Hosted CI runners will be
  several times slower: cache with `Swatinem/rust-cache`, set `CARGO_PROFILE_DEV_DEBUG=0`, keep one feature
  set per job (a second feature set recompiles GPUI), use `mold` on Linux.
- Windows builds need `fxc.exe` (Windows SDK, present with VS Build Tools and on `windows-latest`);
  runtime needs Direct3D 11. Linux runtime needs a Vulkan driver.
- Revisit when Zed resumes publishing `gpui` + `gpui_platform`, or if gpui-kit stops tracking upstream.
- CI builds on all three systems through `ci.yml`; a non-blocking smoke run under Xvfb runs on Linux
  (`scripts/ci/linux-deps.sh`, ADR 0004). This replaces the spike CI job first proposed here. macOS needs no offline `metal` compiler: gpui-kit
  enables `runtime_shaders`.

## Changelog
- 2026-11-20: accepted; toolchain pin raised from 1.97.1 (the spike's) to 1.100.0 after the first macOS CI
  run failed on `map_or_default`.

## Evidence
Spike: `spikes/gpui-window` (standalone, own lockfile). Run on Windows 11, rustc 1.97.1 MSVC:
- `cargo build --release` from an empty `target/`: 1m47s (crates already downloaded), no warnings.
  `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass (clippy from cold: 43s).
- Launched the exe: process alive after 6s, `MainWindowTitle` = `Acta`, responding; then killed.
  A `PrintWindow` capture showed the custom background, the text line in bundled IBM Plex Sans, a
  gpui-component `Input` with our border/placeholder colours, and the embedded icon in the title bar.
- PE header subsystem = 2 (GUI), so no console window.

Not verified:
- macOS and Linux: nothing built or run by the spike.
- Typing, IME, HiDPI/multi-monitor, custom title bar, blur, window state restore: read in source only.
- Theming depth: eight colour tokens, radius and font were overridden on one component. Whether every
  component can reach the design system's fidelity is unproven.
- `Textarea`, `Editor`, virtual list, command palette: present in the crate, not exercised.
- Binary size and build time on CI runners; size with fat LTO or `opt-level = "s"`.

Sources: https://crates.io/crates/gpui · https://crates.io/crates/gpui-pre · https://crates.io/crates/gpui-kit ·
https://github.com/zed-industries/zed/tree/main/crates/gpui · https://github.com/longbridge/gpui-kit ·
https://gpui-kit.com/docs/getting-started · https://www.gpui.rs
