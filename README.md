# Acta

Acta is a local-first desktop app that complements Claude Code: a project tracker for the
human and a structured, scoped memory for the agent. It is written in Rust with GPUI and
runs on Windows, macOS and Linux.

## Status

Early. The 0.1.x releases are an empty window whose only job is to prove that the app
installs silently and updates itself on all three systems. The tracker, the MCP server
and the CLI come next; see [PLAN.md](PLAN.md).

**No release has been published yet.** The commands below work from the first release
on. Until then this page describes how installing and updating are designed to behave;
[what has been verified](#what-has-been-verified) says how much of it has been seen
working.

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

The `acta` command: on Windows the installer puts it on your user PATH. On macOS and
Linux Acta links it as `~/.local/bin/acta` the first time it starts; add `~/.local/bin`
to your PATH if it is not there. An `acta` that is already in `~/.local/bin` and is not
Acta's is left alone.

On Linux the AppImage uses the system's fontconfig, xkbcommon, X11 or Wayland libraries
and a Vulkan driver, which desktop distributions normally provide. It is built on
Ubuntu 22.04 and so needs glibc 2.35 or later.

### What has been verified

Verified, on Windows 11 only, with a throwaway test app packaged the same way and an
update feed served from the same machine (details in
[ADR 0004](docs/decisions/0004-packaging-and-updates.md), "Evidence"):

- The installer runs silently with no elevation and installs per user.
- An installed version updates itself at launch, from a delta package, in about a second.
- Uninstalling removes the app, its shortcuts and its PATH entry.

Not verified yet, because it needs a first real release and machines we have not run it
on:

- Everything on macOS and Linux: packaging, the ad-hoc signature, `install.sh`, first
  launch, the Linux desktop entry and icon, self-update.
- Both install scripts against a real GitHub release, and updates fetched from GitHub
  Releases rather than from a local feed.
- What the operating systems show for browser downloads (SmartScreen, Smart App
  Control, Gatekeeper). The next section describes it from Microsoft's and Apple's
  documented behaviour and third-party reports, not from our own tests.

This list shrinks as the first releases confirm each point; the working checklist is in
[docs/releasing.md](docs/releasing.md).

## Unsigned builds

Acta has no budget, so nothing is signed with a paid certificate: no Windows code-signing
certificate, no Apple Developer ID, no notarization. The install scripts above are meant
to avoid most of the consequences. If you download the files with a browser instead,
this is what we expect (see [what has been verified](#what-has-been-verified)):

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
- **Linux.** No warning expected. A manually downloaded AppImage needs `chmod +x`.

What this means for trust: the integrity of an install or an update rests on HTTPS to
GitHub and on the checksums published with each release, not on a publisher signature.

## Updates

By design, Acta checks GitHub Releases each time it starts. If a newer version exists it
downloads it (only the difference when possible), applies it and restarts before the
window opens, without asking. The check gets 2 seconds and the download 5; past that,
or if either fails, Acta opens normally and the update is applied at a later start. An
ordinary start can therefore take up to about 7 seconds before the window appears.
Updating uses [Velopack](https://velopack.io) and needs no administrator rights. This has been seen working on Windows against a local
feed; macOS, Linux and GitHub as the source are still to be confirmed.

## Uninstall

Uninstalling never deletes your data. The data directory is kept on every system;
delete it yourself only if you want everything gone.

The Windows uninstaller was verified with the test app. The macOS and Linux lists are
the locations Acta is designed to use; they have not been checked on a real install
yet, and there is no uninstaller on those systems.

- **Windows.** Settings > Apps > Installed apps > Acta > Uninstall, or
  `& "$env:LOCALAPPDATA\Acta\Update.exe" --uninstall --silent`. This removes the app,
  the `acta` command (`%LOCALAPPDATA%\ActaCli`) and its PATH entry. Your data stays in
  `%LOCALAPPDATA%\ActaData`, which also holds the log (`logs\acta.log`).
- **macOS.** Quit Acta and delete `~/Applications/Acta.app`, the link
  `~/.local/bin/acta`, `~/Library/Application Support/ActaCli` (the `acta` command) and
  `~/Library/Logs/Acta`. Your data stays in `~/Library/Application Support/Acta/data`.
- **Linux.** Quit Acta and delete the file `~/.local/share/acta/Acta.AppImage` (not
  the directory), the link `~/.local/bin/acta`, `~/.local/share/acta-cli` (the `acta`
  command), `~/.local/share/applications/acta.desktop`,
  `~/.local/share/icons/hicolor/512x512/apps/acta.png` and `~/.local/state/acta` (the
  log). Your data stays in `~/.local/share/acta/data`. If `XDG_DATA_HOME` or
  `XDG_STATE_HOME` is set, they replace `~/.local/share` and `~/.local/state` for
  everything but the AppImage, which the install script always puts in
  `~/.local/share/acta`.

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

## Third-party licences

Acta is built from open-source components: the Rust crates it links, its fonts, the
Lucide icons and the Velopack updater. Their licence texts ship inside every package,
in a `licenses` directory (`Acta.app/Contents/Resources/licenses` on macOS).

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
