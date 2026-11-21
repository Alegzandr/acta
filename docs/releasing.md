# Releasing

## Release

1. Set the new version in `[workspace.package] version` of the root `Cargo.toml`, run
   `cargo check` so `Cargo.lock` follows, commit both, push `main`.
2. Wait for CI and for the release dry run on that commit to be green (see below).
3. Tag that commit of `main` and push the tag:
   ```sh
   git tag v0.1.1
   git push origin v0.1.1
   ```

Nothing else is manual. Versions only go up, and a version is never reused: clients
ignore anything that is not newer than what they run. A version with a pre-release
suffix (`0.2.0-rc.1`) is published as a GitHub pre-release, which installed apps and the
install scripts ignore.

## What the workflow does

`.github/workflows/release.yml`, on a `v*` tag:

1. `check`: fails unless the tagged commit is an ancestor of `origin/main`, unless the
   tag is `v` + the workspace version, and unless the `velopack` crate in `Cargo.lock`
   has the same version as the `vpk` tool the workflow installs. It then collects the
   third-party licence notices with `scripts/licenses.sh`, which fails if a dependency
   has a licence that `about.toml` does not accept.
2. `build`, one job per channel (`win-x64`, `osx-arm64`, `osx-x64`, `linux-x64`):
   `cargo build --release --locked`, then `scripts/package.ps1` or `scripts/package.sh`,
   which stage `Acta`, `acta-cli` and the `licenses` directory (on macOS inside an
   ad-hoc signed `Acta.app`), download the previous release of the channel and run `vpk pack`. That produces the
   installer or portable package, the full update package and a delta from the previous
   version. A tag build restores no cargo cache: everything that ships is compiled in
   that run.
3. `collect`, read-only: writes `SHA256SUMS` over the packages of the four channels
   and the two install scripts.
4. `publish`, the only job with write permissions, in the `release` environment, with no
   checkout: checks that `SHA256SUMS` is the file `collect` wrote and every file against
   it, attests build provenance,
   creates a draft release on the existing tag with `gh release create --verify-tag
   --draft` and no target, uploads the four channels into it with
   `vpk upload github --merge`, adds `install.sh` and `install.ps1`, then checks the
   draft: exactly the expected assets, GitHub's digest of each one equal to
   `SHA256SUMS`, and each `releases.<channel>.json` feed listing exactly this version's
   full and delta packages with their verified size, SHA-1 and SHA-256. It appends the
   feeds' hashes to `SHA256SUMS`, attests the feeds, uploads `SHA256SUMS` last and
   publishes the release as the latest.

Splitting `collect` from `publish` keeps the write token away from everything but the
upload. It does not make the content more trustworthy: a release contains what the
build jobs produced from the tagged commit.

It uses the workflow's `GITHUB_TOKEN` only. Every action is pinned to a commit SHA
(ADR 0004). If a run fails before the last step, the release stays a draft, invisible to
users and to installed apps: fix the cause and re-run the failed jobs from the Actions
page. A draft left behind is replaced. Once a release is published it is never
modified: a problem found afterwards is fixed by the next version.

The draft is never created with a target commit. GitHub refuses to create or edit a
release whose target commit differs from the default branch under `.github/workflows/`
unless the token may write workflows, and `GITHUB_TOKEN` never may: it answers 403
"Resource not accessible by integration". A tag's commit differs that way as soon as
`main` gets a workflow change after the tag, which is how v0.1.1 failed to publish. With
no target, GitHub checks the default branch against itself, and the release still
belongs to the tag that started the run, since the target is ignored for a tag that
exists. Do not add `--target` or vpk's `--targetCommitish` back.

## Repository settings

Set once, by the repository owner, in the GitHub web interface. All of it is free on a
public repository. The workflow runs without these settings; they are what keeps a
release from being produced or altered outside the procedure above. The workflow file
that runs for a tag is the one in the tagged commit, so the checks inside the workflow
only protect against mistakes; these settings are the actual barrier.

- **Tag ruleset.** Settings > Rules > Rulesets > New ruleset > New tag ruleset. Name
  `release tags`, enforcement `Active`, target tags matching `v*`, empty bypass list.
  Rules: **Restrict updates**, **Restrict deletions**, **Block force pushes**. A `v*`
  tag can then be created once and never moved or deleted, by anyone. Leave **Restrict
  creations** off: with an empty bypass list it would also stop the tag push that
  starts a release.
- **Immutable releases.** Settings > General > Releases > **Enable release
  immutability**. Once a release is published, its assets cannot be changed, added or
  removed and its tag cannot be moved. Title, notes and the pre-release and latest
  flags stay editable, so yanking (below) still works. The workflow already follows
  the required order: draft, upload everything, publish. Deleting an immutable release
  is possible; its tag name can then never be used again, which matches the rule above.
- **`release` environment.** Settings > Environments > `release` > Deployment branches
  and tags > **Selected branches and tags** > add the tag rule `v*`. The `publish` job,
  the only one with a token that can write, then cannot start from a branch. No
  required reviewers: that would add a manual step. If the environment does not exist,
  the first run that names it creates it without restrictions.
- **Account.** Two-factor authentication on the owner account. Builds are unsigned, so
  whoever can push a `v*` tag controls every install.

## Dry run

The same workflow runs as a dry run on every push to `main` that touches more than
documentation, and on demand. A dry run stops after `collect`: no release, no
attestation, and no job with write permissions in the run.

```sh
gh workflow run release.yml --ref main      # or any branch
gh run watch
gh run download <run-id> --dir dry-run      # the packages of the four channels
```

A dry run packages the current workspace version, whatever it is. Dry runs on `main`
are the only runs that save the cargo cache, and only dry runs read it.

Locally, for the current machine:

```sh
cargo build --release --locked
scripts/licenses.sh             # needs cargo-about 0.9.3; on Windows from Git Bash
scripts/package.sh              # macOS, Linux; needs vpk 1.2.161, Linux also squashfs-tools
scripts\package.ps1             # Windows
```

The result is in `releases/<channel>/`. `vpk` is a .NET tool:
`dotnet tool install -g vpk --version 1.2.161`. Its version must always equal the
`velopack` crate version; bump them together. `cargo-about` is installed with
`cargo install cargo-about --locked --version 0.9.3`; its version is pinned in
`scripts/licenses.sh` and, with the checksum of the Linux binary, in
`scripts/ci/install-cargo-about.sh`.

A new dependency under a licence that is not in `about.toml` stops CI. Read the licence;
if it allows shipping the binary with its notice, add it to `about.toml`, otherwise
drop the dependency.

## Verify a release

```sh
gh release view v0.1.1                               # assets of four channels, not a draft
gh release download v0.1.1 --dir check && cd check
sha256sum -c SHA256SUMS
gh attestation verify Acta-win-x64-Setup.exe --repo Alegzandr/acta
```

Then on a machine with the previous version installed: start Acta, it restarts as the
new version within a few seconds (an ordinary start may wait up to about 7 seconds for
the window: 2 for the check, 5 for the download). On a clean machine: run the install one-liner from the
README. Both are what the update end-to-end workflow below does on GitHub's runners.

## Update end to end

`.github/workflows/update-e2e.yml` proves self-update on real releases, on disposable
runners: windows-latest, macos-latest (Apple silicon, `osx-arm64`) and ubuntu-24.04
(Xvfb, lavapipe, `fuse3`). It never runs on its own and needs no write permission.
Run it once a release is published:

```sh
gh workflow run update-e2e.yml --ref main                   # previous release -> latest
gh workflow run update-e2e.yml --ref main -f from=v0.1.0 -f to=v0.1.1
gh run watch
```

`to` must be the latest release: installed apps update to the highest published,
non-pre-release version, whatever the test asks for. `from` defaults to the release
just before it. Each job, through `scripts/ci/update-e2e.sh` (macOS, Linux) or
`scripts/ci/update-e2e.ps1` (Windows):

1. Installs `from` with the `install.sh` or `install.ps1` of that release, pinned to
   its version, with `ACTA_NO_LAUNCH=1`. On Windows the script runs `Setup.exe --silent`
   and the Start menu shortcut must exist (checked again after the update).
2. macOS and Linux only: a first start with `--specimen`, which exports the CLI and
   creates `~/.local/bin/acta` (and the Linux desktop entry) without checking for
   updates, so the old version can be observed. On Windows the install hook already
   exported the CLI.
3. Checks that `from` is installed: the version in Velopack's `sq.version`, the
   uninstall entry (Windows) or `CFBundleShortVersionString` (macOS), the packaged
   `acta-cli --version`, the exported CLI and its marker, and that `acta` resolves to
   the exported CLI from the user PATH (Windows) or through `~/.local/bin` (macOS,
   Linux).
4. Starts Acta the way a user does (Start menu shortcut, `open Acta.app`, the AppImage)
   with the update feed on the real GitHub releases, and waits at most 3 minutes for
   a process of version `to` that logs "started by the updater". If a launch opened
   its window first (the download outran the launch budget, or the check failed), the
   app is closed and started again, at most twice, as a user would. It then checks
   the order of the update lines in the app log (`from` finds and downloads `to`; on
   Windows the after-update hook of `to` exports the CLI; `to` is restarted by the
   updater) and that `to` is still running 10 seconds later.
5. Runs the checks of step 3 for `to`; on macOS also `codesign --verify` of the
   updated bundle.
6. Windows: runs the uninstall entry's `QuietUninstallString` with Acta still open and
   waits up to 60 seconds for `%LOCALAPPDATA%\ActaCli`, the uninstall entry and the
   whole install root `%LOCALAPPDATA%\Acta` to be gone; then checks that the PATH
   entry and the Start menu shortcut are gone, no process runs from either directory,
   and `%LOCALAPPDATA%\ActaData` and its log are kept.
7. Always, last: stops any Acta process still running (the updated app is left open
   by step 4).

On failure the app log, Velopack's log and a listing of the install are uploaded as
the `update-e2e-logs-<os>` artifact. The job summary gives the time each update took
and how many launches it needed. `osx-x64` is not covered: the matrix has no Intel
runner; `macos-26-intel` (free) can be added the day it matters. A green run answers
the "Second tag" items below for Windows, macOS arm64 and Linux, and the install,
link, desktop entry, log and uninstall items of "First tag"; tick each one only from
what the run showed.

Two known sources of failure that are not necessarily a bug in Acta:

- **macOS relaunch without `VELOPACK_RESTART`.** If `UpdateMac` relaunches the app
  through LaunchServices (`open`), the environment variable may not reach the new
  process. The update itself then works (the bundle is `to`, the exported CLI prints
  `acta <to>`), but the restarted app never logs "update: started by the updater":
  the step fails with "Acta did not restart as `<to>` within 180 s", and the log shows
  lines of `[<to> pid N]` with "update: up to date" instead. That would mean the
  updater-restart guard of `update.rs` does not work on macOS; record it in ADR 0004
  rather than loosening the test.
- **GitHub API rate limit.** `GithubSource` calls the API without a token: 60
  requests per hour per IP, shared with whatever else ran on that runner's address.
  A refused check logs "update: check failed: <kind>; retrying at next
  launch"; the test starts the app again twice, then fails with "did not restart as
  `<to>`" and only "check failed" lines in the log. Re-run the failed job; if it
  keeps happening, the rate limit is biting real users too (ADR 0004 revisit
  condition).

## Yank a bad release

Installed apps look at the ten most recent published, non-pre-release releases and
update to the highest version found there. To stop clients from updating to a bad
version, hide it:

```sh
gh release edit v0.1.1 --prerelease      # hidden from apps and from the install scripts
# or remove it entirely; keep the tag so the number is never reused:
gh release delete v0.1.1 --yes
```

`releases/latest` then points at the previous release again, so new installs get that
one. Clients that already updated stay on the bad version, because Acta never
downgrades: fix forward and release the next patch version as soon as possible. Its
delta is built against the latest visible release, and clients on the yanked version
take the full package.

## To confirm on the first run

No workflow of this repository has run on GitHub yet and no release exists. Everything
below is written from documentation, from the vpk 1.2.161 source and from a local
Windows pack, and has to be observed once. Tick an item only after seeing it; when one
turns out wrong, fix it and record the finding in ADR 0004. Keep this list until every
item is ticked.

First push (`ci.yml`):

- [ ] `shellcheck` and the `PSScriptAnalyzer` module are present on `ubuntu-24.04`, and
      the lint job passes. Seen: the image ships shellcheck 0.9.0 (apt), which reports
      SC2015 on `[ a ] && [ b ] || c` where 0.11.0 does not. Lint locally with 0.9.0.
- [ ] clippy, tests and the build pass on the three systems; the Linux package list in
      `scripts/ci/linux-deps.sh` is complete.
- [ ] The GUI stays up for 10 seconds under Xvfb with lavapipe (the step is not
      blocking until it has).
- [ ] Only `main` saves the cargo cache, and branches restore it.
- [ ] `scripts/ci/install-cargo-about.sh` installs the pinned binary and
      `scripts/licenses.sh` passes on the Linux job.

First dry run (`release.yml` on `main`):

- [ ] The .NET SDK is preinstalled on the Windows, macOS and Ubuntu runners and
      `dotnet tool install vpk` works on each. Seen on Windows and Ubuntu in the first
      dry run (run 44819612791). On macOS the tool installed but could not start: the
      runner sets no `DOTNET_ROOT` and keeps .NET outside the default location. The
      install step now sets it and starts vpk once; confirm on the next dry run.
- [x] `x86_64-apple-darwin` cross-compiles on the Apple silicon runner (first dry
      run: about 2 minutes).
- [ ] `vpk pack --signAppIdentity -` works with no certificate, and the bundle inside
      the portable zip passes `codesign --verify` (the script fails otherwise).
- [x] vpk builds the AppImage with `mksquashfs`, and it stays up for 15 seconds under
      Xvfb without FUSE (first dry run).
- [ ] Asset names on macOS and Linux are the ones the install scripts download:
      `Acta-osx-arm64-Portable.zip`, `Acta-osx-x64-Portable.zip`,
      `Acta-linux-x64.AppImage`; `assets.<channel>.json` lists `RelativeFileName` on
      every system.
- [x] `vpk download github` with no published release fails softly and the build
      continues without a delta. Seen on Windows and Linux: vpk logs "No releases
      found" and "No full / applicable release was found to download. Aborting.", the
      script prints "no previous <channel> release downloaded; no delta package will
      be built". This is expected until a release exists, including on the `v0.1.0`
      tag run; from the second release on, the same warning means something is wrong.
- [ ] `collect` produces `SHA256SUMS` with no name collision between channels.
- [ ] Every package contains the `licenses` directory: next to `Acta.exe` after a
      Windows install, in `Acta.app/Contents/Resources`, and inside the AppImage
      (`--appimage-extract`).
- [x] A release build with no cache fits in the 60 minute job timeout on every runner
      (a tag build is always uncached). First dry run, no cache found on any job:
      `cargo build --release` took 7.5 min on Linux, 7.6 min on Windows, 6.2 min for
      osx-arm64 and 2.1 min for osx-x64; the whole Linux and Windows jobs took 9.3 and
      10 min. The macOS jobs stopped at packaging, which takes seconds elsewhere.

First tag (`v0.1.0`):

- [ ] The ancestor check finds `origin/main` after the `fetch-depth: 0` checkout of a
      tag.
- [ ] The `release` environment restricted to `v*` lets the tag run through.
- [ ] `actions/attest-build-provenance` accepts `subject-checksums` with every asset,
      and `gh attestation verify` succeeds on a downloaded one.
- [ ] `vpk upload github --merge` called four times from one job, with no checkout,
      fills a single draft; `--targetCommitish` is accepted for an existing tag.
- [ ] `publish` receives the digest of `SHA256SUMS` from `collect` and accepts it.
- [x] The `releases.<channel>.json` feeds vpk uploads are byte-identical to the ones
      the build wrote. Wrong as soon as a previous release exists (v0.1.2): vpk writes
      each feed anew from the version's own packages, without the previous full
      package that `vpk pack` fetched. The feeds are now out of the `collect`
      `SHA256SUMS`; `publish` checks the served ones and appends their hashes.
- [ ] The draft holds exactly the expected assets (the check step prints the
      difference otherwise). In particular: vpk uploads `Acta-win-x64-Portable.zip`
      and neither `assets.*.json` nor `RELEASES-*`.
- [ ] The tag ruleset does not stop vpk from creating the draft for the existing tag,
      and immutability does not stop the draft from being published.
- [ ] `releases/latest/download/install.sh` and `install.ps1` resolve.
- [ ] `install.ps1` on a clean Windows: no prompt, no elevation, Acta starts, `acta`
      is on the user PATH and the script says so only then.
- [ ] `install.sh` on macOS arm64 and x64: no Gatekeeper warning, Acta opens. Run it a
      second time with Acta open: the app quits, is replaced and reopens (this relies
      on `ps -o comm=` printing the full path of the executable).
- [ ] `install.sh` on Linux: the AppImage starts; `acta.desktop` appears in
      `~/.local/share/applications` and `acta.png` in
      `~/.local/share/icons/hicolor/512x512/apps`.
- [ ] On macOS and Linux `~/.local/bin/acta` exists after the first start and runs;
      an unrelated `acta` already there is left untouched.
- [ ] The log is where the README says on each system, and a Windows uninstall leaves
      `%LOCALAPPDATA%\ActaData` in place.
- [ ] A `Setup.exe` downloaded with a browser shows SmartScreen as the README says; a
      zip downloaded with a browser on macOS is blocked as the README says.

Second tag (`v0.1.1`):

- [ ] `vpk download github` finds 0.1.0 with the read-only token and a delta package is
      built for every channel.
- [ ] An installed 0.1.0 becomes 0.1.1 at launch on Windows, macOS arm64, macOS x64
      and Linux, from `GithubSource` with no token.
- [ ] On macOS the updated bundle, with its new ad-hoc signature, starts with no
      warning (keychain behaviour across updates is what `platform-checks.yml` answers).
- [ ] Marking 0.1.1 as a pre-release hides it from a 0.1.0 client and from the install
      scripts, with immutable releases enabled.
- [ ] `update-e2e.yml` with `from=v0.1.0`, `to=v0.1.1` passes on the three runners:
      install, update at launch, and on Windows uninstall with Acta open (the run
      summary says how many launches each update needed).
- [ ] The Windows runner opens Acta's window (GPUI on DirectX with no GPU): the
      restarted 0.1.1 is still running 10 seconds later.
