<p align="center">
  <img src="assets/icons/acta.png" width="96" height="96" alt="Acta app icon">
</p>

<h1 align="center">Acta</h1>

<p align="center">
  A local-first desktop workbench for long projects: a tracker for you, and a structured,
  scoped memory for Claude Code.
</p>

<p align="center">
  <a href="https://github.com/Alegzandr/acta/releases/latest">Latest release</a> ·
  <a href="docs/install.md">Install guide</a> ·
  <a href="PLAN.md">Roadmap</a> ·
  <a href="LICENSE">MIT licence</a>
</p>

> **Pre-alpha.** Today Acta is an empty window that installs, updates itself and carries
> the design foundations. What is listed under "What it will do" is not built yet; see
> [Status](#status).

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/images/screenshot-light.png">
  <img src="docs/images/screenshot-dark.png" alt="The Acta window as of v0.1.3: titlebar with search, an empty sidebar, the first-run page and the status bar">
</picture>

## What it will do

- **Track the work.** Projects, epics and topics in a tree, each topic a page with a
  status, a current state and a timeline of what happened and why.
- **Give Claude Code a scoped memory.** An MCP server, a CLI and hooks let a session read
  and write the topic or epic it is working on, in compact answers that do not flood its
  context.
- **Keep a knowledge base you can read.** Documents and the agent's durable memories,
  filed on projects, epics and topics, visible and editable by you.
- **Run Claude Code beside the page.** An integrated terminal that opens in the working
  folder of the topic it was launched from.
- **Stay on your machine.** One encrypted local database, its key in the system keychain.
  No Acta account, no Acta server.
- **Work from the keyboard.** One command palette reaches everything; every action has a
  key path.

## Status

| Milestone | What | Release | State |
|---|---|---|---|
| M1 | Silent install, self-update, design foundations, app icon | v0.1.x | in progress |
| M2 | Encrypted data model, tree, topic pages, command palette, knowledge base | v0.2 | planned |
| M3 | MCP server, CLI, Claude Code hooks and one-step setup | v0.3 | planned |
| M4 | Integrated terminal running Claude Code | v0.4 | planned |
| M5-M7 | Chat panel, notes inbox, polish | v0.5-v1.0 | planned |

The 0.1.x releases exist to prove the pipeline: that Acta installs per user without a
wizard and updates itself on Windows, macOS and Linux. They store no data yet. Details
and progress are in [PLAN.md](PLAN.md).

## Install

Windows 10/11 (PowerShell):

```powershell
irm https://github.com/Alegzandr/acta/releases/latest/download/install.ps1 | iex
```

macOS and Linux:

```sh
curl -fsSL https://github.com/Alegzandr/acta/releases/latest/download/install.sh | sh
```

| System | Architecture | Installs to |
|---|---|---|
| Windows 10, 11 | x64 | `%LocalAppData%\Acta` |
| macOS | Apple silicon, Intel | `~/Applications/Acta.app` |
| Linux (glibc 2.35+) | x86_64 | `~/.local/share/acta/Acta.AppImage` |

Per user, no administrator rights; Acta starts when the install finishes. Builds are not
signed: the project has no budget for certificates. The install scripts avoid most
warnings. A file downloaded with a browser triggers SmartScreen on Windows (**More
info**, then **Run anyway**) and Gatekeeper on macOS (**System Settings > Privacy &
Security > Open Anyway**). Checksums, provenance, uninstalling and what has been
verified so far are in the [install guide](docs/install.md).

## Updates

Acta checks GitHub Releases each time it starts. When a newer version exists it
downloads it, only the difference when it can, applies it and restarts before the window
opens, without asking and without administrator rights. The check is limited to a few
seconds; if it is slow or fails, Acta opens normally and updates at a later start.
Updating uses [Velopack](https://velopack.io).

## Build from source

Install Rust with [rustup](https://rustup.rs); the toolchain is pinned in
`rust-toolchain.toml` and installed on first use.

- **Windows:** Visual Studio Build Tools with the Windows SDK. The encrypted storage
  (from M2) builds a vendored OpenSSL, which needs a native Perl such as
  [Strawberry Perl](https://strawberryperl.com); Git's Perl does not work. Point
  `OPENSSL_SRC_PERL` at its `perl.exe` if it is not first on your PATH.
- **macOS:** Xcode command line tools.
- **Linux:** the packages listed in `scripts/ci/linux-deps.sh` (Ubuntu names).

```sh
cargo run -p acta-app
```

`cargo build --release --locked` produces `target/release/Acta` (the app) and
`target/release/acta-cli` (the command line, installed as `acta`). Packaging is
described in [docs/releasing.md](docs/releasing.md).

## Project docs

- [PLAN.md](PLAN.md): milestones and what is done.
- [DESIGN.md](DESIGN.md): the design system every screen follows.
- [docs/decisions/](docs/decisions/): architecture decision records.
- [docs/releasing.md](docs/releasing.md): how a release is cut and checked.
- [docs/install.md](docs/install.md): install, updates, uninstall and verification in
  detail.

## Contributing

Issues and pull requests are welcome; read [CONTRIBUTING.md](CONTRIBUTING.md) first.
Report security problems privately, as [SECURITY.md](SECURITY.md) explains.

## Licence

Acta is released under the [MIT licence](LICENSE). The third-party components it ships
keep their own licences; their texts are included in every package.
