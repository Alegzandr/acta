# Releasing

## Release

1. Set the new version in `[workspace.package] version` of the root `Cargo.toml`, run
   `cargo check` so `Cargo.lock` follows, commit both, push `main`.
2. Wait for CI and for the release dry run on that commit to be green (see below).
3. Tag and push the tag:
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

1. `check`: fails unless the tag is `v` + the workspace version, and unless the
   `velopack` crate in `Cargo.lock` has the same version as the `vpk` tool it installs.
2. `build`, one job per channel (`win-x64`, `osx-arm64`, `osx-x64`, `linux-x64`):
   `cargo build --release --locked`, then `scripts/package.ps1` or `scripts/package.sh`,
   which stage `Acta` and `acta-cli` (on macOS inside an ad-hoc signed `Acta.app`),
   download the previous release of the channel and run `vpk pack`. That produces the
   installer or portable package, the full update package and a delta from the previous
   version.
3. `publish`, a single job: writes `SHA256SUMS`, attests build provenance, uploads the
   four channels to one draft release with `vpk upload github --merge`, adds
   `SHA256SUMS`, `install.sh` and `install.ps1`, checks that the draft holds exactly the
   expected assets, then publishes it as the latest release.

It uses the workflow's `GITHUB_TOKEN` only. If a run fails before the last step, the
release stays a draft, invisible to users and to installed apps: fix the cause and
re-run the failed jobs from the Actions page. A draft left behind is replaced.

## Dry run

The same workflow runs as a dry run, with no release and no attestation, on every push
to `main` that touches more than documentation, and on demand:

```sh
gh workflow run release.yml --ref main      # or any branch
gh run watch
gh run download <run-id> --dir dry-run      # the packages of the four channels
```

A dry run packages the current workspace version, whatever it is.

Locally, for the current machine:

```sh
cargo build --release --locked
scripts/package.sh              # macOS, Linux; needs vpk 1.2.161, Linux also squashfs-tools
scripts\package.ps1             # Windows
```

The result is in `releases/<channel>/`. `vpk` is a .NET tool:
`dotnet tool install -g vpk --version 1.2.161`. Its version must always equal the
`velopack` crate version; bump them together.

## Verify a release

```sh
gh release view v0.1.1                               # assets of four channels, not a draft
gh release download v0.1.1 --dir check && cd check
sha256sum -c SHA256SUMS
gh attestation verify Acta-win-x64-Setup.exe --repo Alegzandr/acta
```

Then on a machine with the previous version installed: start Acta, it restarts as the
new version within a few seconds. On a clean machine: run the install one-liner from the
README.

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
