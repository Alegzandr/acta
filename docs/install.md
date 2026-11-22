# Installing, updating and uninstalling Acta

The short version is in the [README](../README.md#install). This page has the details:
where files go, what an unsigned build means on each system, how to verify a download,
how updates behave, how to uninstall, and how much of this has been seen working.

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

The files of each release, if you prefer to download by hand:

| File | What it is |
|---|---|
| `Acta-win-x64-Setup.exe` | Windows installer (same result as `install.ps1`) |
| `Acta-win-x64-Portable.zip` | Windows, unpack anywhere |
| `Acta-osx-arm64-Portable.zip`, `Acta-osx-x64-Portable.zip` | macOS app bundle, Apple silicon and Intel |
| `Acta-linux-x64.AppImage` | Linux |
| `SHA256SUMS` | Checksums of every file of the release, including the update packages and feeds Acta itself downloads, and of the two install scripts |

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

## Updates

By design, Acta checks GitHub Releases each time it starts. If a newer version exists it
downloads it (only the difference when possible), applies it and restarts before the
window opens, without asking. The check gets 2 seconds and the download 5; past that,
or if either fails, Acta opens normally and the update is applied at a later start. An
ordinary start can therefore take up to about 7 seconds before the window appears.
Updating uses [Velopack](https://velopack.io) and needs no administrator rights. It is
proven from GitHub Releases on Windows, macOS (Apple silicon) and Linux for every
release since 0.3.0 (see [what has been verified](#what-has-been-verified)).

A version with a pre-release suffix (`0.2.0-rc.1`) is published as a GitHub
pre-release, which installed apps and the install scripts ignore.

## Uninstall

Uninstalling never deletes your data. The data directory is kept on every system;
delete it yourself only if you want everything gone.

The Windows uninstaller is verified on every release. The macOS and Linux lists are
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

## What has been verified

Verified on GitHub's Windows, macOS (Apple silicon) and Linux runners by the
end-to-end update workflow (`update-e2e.yml`), against the published releases, for each
release since 0.3.0 (details in [releasing.md](releasing.md#update-end-to-end)):

- The install scripts of a release install it per user, with no prompt and no
  elevation; on Windows the Start menu shortcut and the `acta` command on the user PATH
  are created, on macOS and Linux the `~/.local/bin/acta` link on first start, and on
  Linux the desktop entry and its icon.
- An installed version updates itself from GitHub Releases at launch and restarts as
  the new version; on macOS the updated bundle passes `codesign --verify`.
- On Windows, uninstalling with Acta open removes the app, the `acta` command, its PATH
  entry and the Start menu shortcut, and keeps `%LOCALAPPDATA%\ActaData` and its log.

Not verified yet:

- macOS on Intel (`osx-x64`): the workflow has no Intel runner.
- The macOS and Linux uninstall lists above.
- What the operating systems show for browser downloads (SmartScreen, Smart App
  Control, Gatekeeper). The [unsigned builds](#unsigned-builds) section describes it
  from Microsoft's and Apple's documented behaviour and third-party reports, not from
  our own tests.

This list shrinks as the releases confirm each point; the working checklist is in
[releasing.md](releasing.md).

## Licences

Acta is released under the MIT licence. It is built from open-source components: the
Rust crates it links, its fonts, the Lucide icons and the Velopack updater. Their licence
texts ship inside every package, in a `licenses` directory
(`Acta.app/Contents/Resources/licenses` on macOS), together with Acta's own as
`Acta-LICENSE.txt`.
