# 0004 - Packaging, silent install and auto-update

Status: accepted (macOS and Linux paths to be proven by the M1 releases)
Date: 2026-11-20

## Context
- Requirements: silent per-user install (no wizard, no admin), update at launch without asking, from GitHub Releases of the public repo `Alegzandr/acta`; CI on every push; a `v*` tag publishes everything. Zero budget, permanently: builds are unsigned and no tool may need a paid certificate or account.
- One codebase is the GUI, the CLI (`acta ...`, called from terminals and Claude Code hooks) and the MCP stdio server (`acta mcp`, alive for hours under a Claude Code session).
- Candidates, checked 2026-11-20:
  - Velopack 1.2.161 (2026-09-29; 1.3.39 is a prerelease), MIT, crate `velopack` + `vpk` CLI (a .NET tool, needs .NET SDK >= 8). Windows `Setup.exe` (one click, per-user, `%LocalAppData%\{packId}`), macOS `.app` in a portable zip and a `.pkg`, Linux self-updating AppImage, delta packages, GitHub source built in. https://docs.velopack.io https://github.com/velopack/velopack
  - cargo-dist 0.34.0 + axoupdater 0.10.3: CLI-oriented (shell/PowerShell installers, MSI); no app bundle, AppImage, shortcuts or deltas. https://axodotdev.github.io/cargo-dist/book/
  - `self_update` 1.3.3: swaps a single executable; no installer, bundle or deltas.
  - cargo-packager 0.11.8 (2025-11-27) + cargo-packager-updater 0.2.3 (2025-07-21): NSIS/app/AppImage, but updates need a minisign key pair kept as a CI secret, run the full installer, have no deltas, and there has been no release for a year. https://docs.rs/crate/cargo-packager-updater/latest
- Facts read in the Velopack 1.2.161 source that shape the design:
  - Windows: `Update.exe` kills every process whose image is under the install root (`force_stop_package(&root_path)` in `src/bins/src/commands/apply_windows_impl.rs`), then renames `current\`. An `acta mcp` running from there dies on every update.
  - macOS and Linux: the bundle is swapped atomically (`renamex_np`) or the AppImage replaced with `mv`; nothing is killed.
  - `vpk pack` on macOS only warns when no signing identity is given; macOS packages can only be built on macOS; Windows and Linux packages can be built on their own runners. https://docs.velopack.io/packaging/cross-compiling
  - The bundled AppImage runtime is the static `type2-runtime`, so `libfuse2` is not required.
- macOS 26 (tested by eclecticlight.co on 26.6.1, 2026-08-11): fully unsigned code never runs on Apple silicon; an ad-hoc signed app opens with no warning when it has no quarantine attribute and is blocked when it has one. https://eclecticlight.co/2026/08/11/how-can-you-run-code-that-hasnt-been-notarised/

## Decision
**Tooling.** Velopack, pinned: crate `velopack = "=1.2.161"`, `dotnet tool install -g vpk --version 1.2.161` (crate and tool versions must match). Pack id `Acta`. One channel per target so asset names never collide in a release: `win-x64`, `osx-arm64`, `osx-x64`, `linux-x64` (later `win-arm64`, `linux-arm64`).

**Binary layout: two thin executables, one package, one version.**
- `Acta[.exe]`: GUI, `windows_subsystem = "windows"`, Velopack `--mainExe`; the only process that installs, updates or uninstalls anything.
- `acta-cli[.exe]`: console subsystem, CLI and `acta mcp`, same workspace and domain crates, no GPUI linked. It is named `acta-cli` inside the package because Windows and macOS file systems are case-insensitive.
- The GUI exports the CLI to a stable per-user directory outside the Velopack root and puts that directory on the user PATH, in the after-install and after-update hooks and again at every start (idempotent, compares a version marker):
  - Windows `%LocalAppData%\ActaCli\bin\acta.exe`, added to `HKCU\Environment\Path` (value type preserved, `WM_SETTINGCHANGE` broadcast).
  - macOS `~/Library/Application Support/Acta/bin/acta`, Linux `~/.local/share/acta/bin/acta`, both symlinked from `~/.local/bin/acta`; `acta setup` adds `~/.local/bin` to the shell profile when it is missing.
  - Replacement is rename-aside then copy then rename: a running `acta.exe` cannot be overwritten on Windows but can be renamed. Old files are deleted on a later start once unlocked.
- Claude Code configuration (MCP command, hooks) always references the exported path, never `current\`.

**Update policy.**
- Only the GUI updates. At launch it checks the feed with a 2 s budget; if an update exists it downloads (delta first), applies and relaunches before the main window opens. If the check or download is slow or fails, the app opens normally, the download continues in the background and Velopack applies it at the next launch. No prompt in either case.
- The CLI and MCP server never update themselves and never run from the Velopack root. A running MCP server keeps its old version until its Claude Code session restarts; every new `acta` invocation gets the new version.
- Compatibility rule that makes this safe: the database written by version N+1 must stay usable by version N. Migrations between adjacent releases are additive only; a destructive change is split over two releases. Each process checks the schema version when it opens the database and, if it is too old to continue, the CLI/MCP answers with one short "restart the session to load Acta x.y.z" error instead of writing.
- Uninstall hook: remove the PATH entry, terminate processes running from the exported directory, delete it.
- Update source: `velopack::sources::GithubSource("https://github.com/Alegzandr/acta", None, false)`, no token (60 requests/hour per IP, one or two per launch; a refused check is retried at the next launch).

**Installation, unsigned, free means only.**
- Windows: `Acta-win-x64-Setup.exe`, plus a documented one-liner `irm <release>/install.ps1 | iex` that downloads and runs it with `--silent`.
- macOS: recommended path is `curl -fsSL <release>/install.sh | sh`: it downloads the portable zip with curl (no quarantine attribute), unpacks `Acta.app` into `~/Applications` and opens it. CI signs the bundle ad hoc (`codesign --force --deep -s -`). The `.pkg` is not shipped (`--noInst`): unsigned it only adds a second warning.
- Linux: the same `install.sh` puts the AppImage in `~/.local/share/acta/`, marks it executable and starts it; at first run the GUI writes its own `.desktop` file and icon under `~/.local/share`.
- Every release carries `SHA256SUMS` and a GitHub build provenance attestation (free on public repos; `gh attestation verify`). Velopack checks the SHA-256 from the feed before applying.

**CI workflow (`ci.yml`, every push and pull request).** `fmt` job on ubuntu (`cargo fmt --all --check`) that the others need; then matrix windows-latest, macos-latest, ubuntu-latest: `Swatinem/rust-cache@v2`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Concurrency group cancels superseded runs. `permissions: contents: read`.

**Release workflow (`release.yml`, on tag `v*`).** `permissions: contents: write, id-token: write, attestations: write`; `GITHUB_TOKEN` only, no secrets.
1. `check`: fail unless the tag equals `v` + the workspace version from `cargo metadata`.
2. `build`, matrix: windows-latest (`win-x64`), macos-latest arm64 building both `aarch64-apple-darwin` and `x86_64-apple-darwin` (fallback `macos-26-intel`), ubuntu-22.04 (`linux-x64`, old glibc baseline). Steps: checkout, rust-cache, `cargo build --release --locked`, stage `Acta` + `acta-cli` (macOS: assemble `Acta.app`, ad-hoc sign), `vpk download github --repoUrl ... --channel <ch>` (previous release, needed for the delta; tolerated to fail on the first release), `vpk pack --packId Acta --packVersion <v> --runtime <rid> --channel <ch> --mainExe Acta[.exe] --icon ...`, upload the output directory as an artifact.
3. `publish`, one job so nothing races: download artifacts, write `SHA256SUMS`, attest, `vpk upload github --channel <ch> --merge --tag v<v> --releaseName "Acta <v>"` once per channel into a draft, `gh release upload` for `SHA256SUMS`, `install.sh`, `install.ps1`, then publish the draft.
- Assets per channel: `Acta-<v>-<ch>-full.nupkg`, `Acta-<v>-<ch>-delta.nupkg`, `releases.<ch>.json`, `assets.<ch>.json`, `RELEASES-<ch>`, and `Acta-win-x64-Setup.exe`, `Acta-<ch>-Portable.zip`, `Acta.AppImage` (exact Linux and macOS names to be read from the first CI run).
- Actions pinned to majors: `actions/checkout@v7`, `Swatinem/rust-cache@v2`, `actions/upload-artifact@v7`, `actions/download-artifact@v8`, `actions/attest-build-provenance@v4`.

## Alternatives
- One `acta.exe` with the GUI subsystem and `AttachConsole(ATTACH_PARENT_PROCESS)`: the shell does not wait for a GUI-subsystem process, so the prompt returns first and output and interactive stdin are unreliable. Only redirected pipes work.
- One console-subsystem `acta.exe` that frees its console for the GUI: a console window flashes at every GUI launch; the manifest `consoleAllocationPolicy` that avoids it exists only on Windows 11 24H2 and later.
- Either single-exe variant also leaves the MCP server inside the directory Velopack kills and renames, so a second copy is needed anyway.
- cargo-dist + axoupdater, `self_update`: built for CLIs, no desktop integration. cargo-packager: paid nothing but needs a signing secret, runs a full installer per update, stale.
- Hand-rolled updater: full control, but we would own delta, rollback, shortcut and registry code on three OSes.
- `HttpSource` on `releases/latest/download/`: no API rate limit, but a feed entry pointing at an asset of an older release 404s; kept as a fallback.
- Universal macOS binary: one artifact, twice the download on every update.

## Consequences
- What users see, permanently:
  - Windows: a `Setup.exe` downloaded with a browser shows SmartScreen "Windows protected your PC"; "More info" then "Run anyway". With Smart App Control on, unsigned executables are blocked with no per-app override; the user has to turn it off. Antivirus false positives are possible. Updates are fetched by `Update.exe` itself and showed no prompt in the spike.
  - macOS: with `install.sh` no warning is expected. A zip downloaded with a browser is quarantined and Gatekeeper blocks it; the user must use System Settings > Privacy & Security > "Open Anyway", or run `xattr -dr com.apple.quarantine`. Self-update is expected to keep working unsigned because Velopack downloads without quarantine and the new bundle is ad-hoc signed; this is unproven until the M1 release.
  - Linux: no warning. A manually downloaded AppImage needs `chmod +x`.
- An ad-hoc signature has a new identity on every build, so macOS keychain items bound to the app's signature will prompt again after each update. ADR 0003 must take this into account.
- Without signing, update integrity rests on HTTPS to GitHub plus the feed checksum: whoever controls the GitHub account controls every install. Protect the account (2FA) and the tag.
- The two-executable layout departs from "the same binary"; what is kept is one install, one version, always in sync.
- A CLI process still running at uninstall keeps its file locked; the uninstall hook must terminate it (the spike left one file behind without that).
- Velopack writes `%LocalAppData%\velopack\velopack_<id>.log`, which uninstall does not remove.
- CI depends on the .NET SDK preinstalled on GitHub runners (otherwise `actions/setup-dotnet@v6`).
- Revisit if Velopack 1.3 becomes stable (pin both sides together), if `GithubSource` rate limits bite, or if a free signing route appears.
- To verify in M1 on real runners and a real release, because it cannot be done locally: `vpk pack` unsigned on macOS and whether the ad-hoc signature survives packaging (or `--signAppIdentity -`); first launch and self-update on macOS arm64 and x64; AppImage install, desktop entry and self-update; `GithubSource` against real release assets; `vpk upload --merge` from one job for four channels; both `install` scripts; SmartScreen on a browser download.

## Amendments from the M1 pipeline (2026-11-20)
Read in the vpk 1.2.161 source and observed with a local Windows pack; the macOS and Linux points still need the first real run.
- Exact asset names: `Acta-win-x64-Setup.exe`, `Acta-win-x64-Portable.zip`, `Acta-osx-arm64-Portable.zip`, `Acta-osx-x64-Portable.zip`, `Acta-linux-x64.AppImage`, `Acta-<v>-<ch>-full.nupkg`, `Acta-<v>-<ch>-delta.nupkg`, `releases.<ch>.json`. `vpk upload github` does not upload `assets.<ch>.json` or `RELEASES-<ch>`; they stay build-side.
- macOS signing has to happen inside `vpk pack` (`--signAppIdentity -`): vpk copies `UpdateMac` into the bundle after staging, which invalidates a signature made before. vpk then runs `codesign -s - -f --timestamp --options runtime --entitlements <its default> --deep`. `scripts/package.sh` verifies the signature of the bundle inside the portable zip and fails the build otherwise.
- Bundle id `io.github.alegzandr.acta` (a domain the project controls) instead of `com.acta.app`.
- The Linux AppImage is built by vpk with `mksquashfs` (`squashfs-tools` on the runner).
- `release.yml` also runs as a dry run (no release, no attestation) on pushes to `main` and on `workflow_dispatch`; this is where a broken release build of any channel, including `osx-x64`, is caught before a tag, and it fills the cargo cache that tag builds read.
- Yanking: `GithubSource` is created with `prerelease = false` and skips pre-releases and drafts, so marking a release as pre-release hides it from clients. See `docs/releasing.md`.

## Amendments from the review of the M1 pipeline (2026-11-20)
These replace the corresponding lines of the Decision above.
- Actions are pinned to full commit SHAs, not to majors, with the release in a trailing comment: a tag of an action can be moved to other code, a commit cannot, and these actions run with the release token. SHAs resolved with `gh api repos/<owner>/<repo>/commits/<tag>` on 2026-11-20: `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1` (v7.0.1), `Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0` (v2.9.3), `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9` (v7.0.2), `actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333` (v8.0.2), `actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8` (v4.2.2). Bumping one means resolving the new tag the same way and changing the SHA and the comment together; nothing updates them automatically.
- The release workflow has `permissions: contents: read` at the top. What was one `publish` job is two. `collect` (read-only, every run) gathers the four build artifacts, writes `SHA256SUMS` and hands it on with the two install scripts as the `release-extras` artifact. `publish` (`contents: write`, `id-token: write`, `attestations: write`) has a job-level `if` so it only exists for a `v*` tag push, runs in the `release` environment, checks out no code, re-verifies every file against `SHA256SUMS`, attests, uploads to a draft and publishes. Checkouts use `persist-credentials: false`. A dry run therefore never holds a token that can write.
- Tag builds restore no cargo cache (the `Swatinem/rust-cache` step is skipped): a cache entry can be written by any earlier run on `main`, and nothing that such a run left behind may end up in a release. This supersedes "it fills the cargo cache that tag builds read" above; the cost is a full release build per channel on every tag. Dry runs still read the cache and only `main` saves it.
- `check` refuses a tag whose commit is not an ancestor of `origin/main`, before running anything from that commit. This catches a tag put on the wrong commit; it cannot stop someone who can push a tag, because the workflow that runs is the one in the tagged commit. The barriers for that are repository settings, documented in `docs/releasing.md` and to be applied by the owner: a tag ruleset on `v*` (no update, no deletion, no force push), immutable releases, and the `release` environment restricted to `v*` tags. All three are free on a public repository.

## Evidence
Spike: `spikes/updater` (Windows 11 26300, rustc 1.97.1 MSVC, velopack 1.2.161, vpk 1.2.161, .NET SDK 10.0.302). `pack.ps1 <version>` builds and packages; the feed was the `releases/` folder served on `http://127.0.0.1:8765/` through `HttpSource`. Observed on Windows only:
- `vpk pack` took under 1 s and produced `ActaSpike-0.0.1-win-x64-full.nupkg`, `ActaSpike-win-x64-Setup.exe` (11.6 MB for a 3.4 MB app), `ActaSpike-win-x64-Portable.zip`, `releases.win-x64.json`; packing 0.0.2 in the same folder added a 2.9 kB delta. Without `--runtime` vpk assumes x86.
- `Setup.exe --silent` from a non-elevated shell: exit 0 in 2.1 s, no elevation, app not launched (without `--silent` it launches). Created `%LocalAppData%\ActaSpike\{current\, packages\, Update.exe, Acta Spike.exe stub}`, Start menu and Desktop shortcuts targeting `current\Acta.exe`, and `HKCU\...\Uninstall\ActaSpike` (with `QuietUninstallString`); nothing in HKLM. The after-install hook exported `acta.exe` and added its directory to the user PATH.
- Self-update: launching the installed 0.0.1 fetched the feed and the delta, applied it and relaunched as 0.0.2 about 1.0 s after start. `DisplayVersion` became 0.0.2 and the exported CLI reported 0.0.2. With the feed unreachable the check failed after 2.0 s and the app carried on.
- Processes alive during the update: `current\acta-cli.exe mcp` and a second `current\Acta.exe` were killed by `Update.exe` (logged "Killing process"). `ActaSpikeCli\bin\acta.exe mcp`, outside the root, kept running as 0.0.1; its file was renamed aside and the new CLI installed next to it.
- `Update.exe --uninstall --silent`: root directory, shortcuts and registry key removed, user PATH byte-identical to the backup. Failure: the exported directory was not deleted (access denied) because the surviving MCP process still held the renamed file; it was removed once that process ended.
- A GUI-subsystem exe writes to a redirected pipe correctly; interactive console behaviour was not tested.
- Not tested: macOS, Linux, GitHub as the source, a browser-downloaded installer, Smart App Control.
