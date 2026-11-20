# Acta

Acta is a local-first desktop app that complements Claude Code: a project tracker for the
human and a structured, scoped memory for the agent. It is written in Rust with GPUI and
runs on Windows, macOS and Linux.

## Status

Early. The 0.1.x releases are an empty window whose only job is to prove that the app
installs silently and updates itself on all three systems. The tracker, the MCP server
and the CLI come next; see [PLAN.md](PLAN.md).

## Install

Per user, no wizard, no administrator rights. Acta starts when the install finishes.

Windows 10/11, x64 (PowerShell):

```powershell
irm https://github.com/Alegzandr/acta/releases/latest/download/install.ps1 | iex
```

macOS (Apple silicon or Intel) and Linux x86_64:

```sh
curl -fsSL https://github.com/Alegzandr/acta/releases/latest/download/install.sh | sh
```

| System | Where it goes |
|---|---|
| Windows | `%LocalAppData%\Acta`, Start menu and desktop shortcuts |
| macOS | `~/Applications/Acta.app` |
| Linux | `~/.local/share/acta/Acta.AppImage` |

A specific version: set `ACTA_VERSION=0.1.0` before running either script. Set
`ACTA_NO_LAUNCH=1` to install without starting the app.

On Linux the AppImage uses the system's fontconfig, xkbcommon, X11 or Wayland libraries
and a Vulkan driver; desktop distributions have them.

## Unsigned builds

Acta has no budget, so nothing is signed with a paid certificate: no Windows code-signing
certificate, no Apple Developer ID, no notarization. The install scripts above avoid most
of the consequences. If you download the files with a browser instead, expect this:

- **Windows.** `Acta-win-x64-Setup.exe` triggers SmartScreen ("Windows protected your
  PC"): click **More info**, then **Run anyway**. If Smart App Control is on, it blocks
  unsigned programs with no per-app exception; Acta cannot run until it is turned off
  (Windows Security > App & browser control > Smart App Control), and Windows does not
  let you turn it back on without a reset. Antivirus false positives are possible.
- **macOS.** The app is signed ad hoc (no identity), which Apple silicon requires. A zip
  downloaded with a browser is quarantined and Gatekeeper refuses to open the app. Either
  try to open it once, then go to **System Settings > Privacy & Security** and click
  **Open Anyway**, or remove the quarantine attribute:
  `xattr -dr com.apple.quarantine ~/Applications/Acta.app`.
  `install.sh` downloads with curl, which sets no quarantine attribute, so no warning is
  expected.
- **Linux.** No warning. A manually downloaded AppImage needs `chmod +x`.

What this means for trust: the integrity of an install or an update rests on HTTPS to
GitHub and on the checksums published with each release, not on a publisher signature.

## Updates

Acta checks GitHub Releases each time it starts. If a newer version exists it downloads
it (only the difference when possible), applies it and restarts before the window opens,
without asking. If the check is slow or fails, Acta opens normally and the update is
applied at the next start. Updating uses [Velopack](https://velopack.io) and needs no
administrator rights.

## Uninstall

- **Windows.** Settings > Apps > Installed apps > Acta > Uninstall, or
  `& "$env:LOCALAPPDATA\Acta\Update.exe" --uninstall --silent`.
- **macOS.** Quit Acta and delete `~/Applications/Acta.app`. The `acta` command is a link
  at `~/.local/bin/acta` to `~/Library/Application Support/Acta/bin`; delete both.
- **Linux.** Quit Acta and delete `~/.local/share/acta/Acta.AppImage`,
  `~/.local/share/acta/bin`, the link `~/.local/bin/acta` and Acta's entry in
  `~/.local/share/applications`.

## Verify a download

Each release carries a `SHA256SUMS` file and a GitHub build provenance attestation that
ties every file to the workflow run that built it.

```sh
# checksum (Linux; on macOS use `shasum -a 256 -c`)
sha256sum -c --ignore-missing SHA256SUMS

# provenance, with the GitHub CLI
gh attestation verify Acta-linux-x64.AppImage --repo Alegzandr/acta
```

On Windows: `(Get-FileHash Acta-win-x64-Setup.exe).Hash` and compare it with the line in
`SHA256SUMS`. The install scripts do the checksum comparison themselves.

## Build from source

Install Rust with [rustup](https://rustup.rs); the toolchain version is pinned in
`rust-toolchain.toml` and installed on first use.

- Windows: Visual Studio Build Tools with the Windows SDK.
- macOS: Xcode command line tools.
- Linux (Ubuntu names): `scripts/ci/linux-deps.sh` lists and installs the packages.

```sh
cargo build --release --locked
```

The executables are `target/release/Acta` (the app) and `target/release/acta-cli` (the
command line and MCP server, installed as `acta`). To build the installer and update
packages the way the release workflow does, see [docs/releasing.md](docs/releasing.md).
