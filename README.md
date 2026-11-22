<p align="center">
  <img src="assets/icons/acta.png" width="96" height="96" alt="Acta app icon">
</p>

<h1 align="center">Acta</h1>

<p align="center">
  A local-first desktop workbench for long projects: a tracker for you, and a structured,
  scoped memory for Claude Code.
  Written in Rust with GPUI, for Windows, macOS and Linux.
</p>

<p align="center">
  <a href="https://github.com/Alegzandr/acta/releases/latest">Latest release</a> ·
  <a href="docs/install.md">Install guide</a> ·
  <a href="PLAN.md">Roadmap</a> ·
  <a href="LICENSE">MIT licence</a>
</p>

> **Alpha.** Acta is usable day to day and tracks its own development. Expect rough
> edges; your data stays in one encrypted file on your machine. Progress is in
> [PLAN.md](PLAN.md).

## What it does

- **Track the work.** Projects, epics and topics in a tree. Each page has a status, an
  optional due date, a current state, open questions and a timeline written by Claude
  Code. A calm Overview shows only what needs you: blocked, overdue, idle, unanswered.
- **Give Claude Code a scoped memory.** An MCP server, a CLI and hooks brief each session
  on the topic it works on and let it read and write that topic, its epic and project, in
  compact answers that do not flood its context.
- **Keep a knowledge base you can read.** Documents and the agent's durable memories,
  filed on projects, epics and topics, visible and editable by you.
- **Know who is who.** Contacts (with levels: all projects, a project or an epic) for the
  people you ask questions or report back to; your own profile.
- **Run Claude Code beside the page.** An integrated terminal opens Claude Code in the
  page's active folder, resumes that page's conversation, and shows which session works
  where. Your plan usage stays visible in the status bar.
- **Work in your repositories.** Several named folders per page, inherited from above,
  with branch state, Pull and branch switching, and per-folder commit modes (manual,
  commit, commit and push), guarded so Claude Code never skips hooks or force-pushes.
- **Notes, files and voice.** A notes inbox Claude Code can triage; files attached to notes
  and entries, encrypted in the database; dictation with a local Whisper model you choose
  to download.
- **Ready for Claude Code.** Acta checks Claude Code, your sign-in, Git and the GitHub CLI,
  and installs what is missing with one confirmed click.
- **Embedded tools.** A claude.ai chat panel and a browser Claude Code can drive, both with
  their own profile (Windows).
- **Stay on your machine.** One encrypted local database, its key in the system keychain.
  No Acta account, no Acta server, no telemetry.
- **Work from the keyboard.** One command palette reaches everything; every view can be
  moved, hidden and restored, VS Code style.

## Status

Released versions are on the [releases page](https://github.com/Alegzandr/acta/releases).
Milestones M0 to M8 are done or in hands-on QA; the features added since (folder sets,
due dates, questions and contacts, attachments, dictation, Ready for Claude Code, memory
quality) are tracked in [PLAN.md](PLAN.md). Self-update is proven end to end on Windows,
macOS and Linux for every release.

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
signed: the project has no budget for certificates. The install scripts are meant to
avoid most warnings. A file downloaded with a browser is expected to trigger SmartScreen
on Windows (**More info**, then **Run anyway**) and Gatekeeper on macOS (**System
Settings > Privacy & Security > Open Anyway**); these warnings are described from the
vendors' documentation, not yet from our own tests
([what has been verified](docs/install.md#what-has-been-verified)). Checksums,
provenance and uninstalling are in the [install guide](docs/install.md).

## Updates

By design, Acta checks GitHub Releases each time it starts. When a newer version exists
it downloads it, only the difference when it can, applies it and restarts before the
window opens, without asking and without administrator rights. The check is limited to a
few seconds; if it is slow or fails, Acta opens normally and updates at a later start.
Updating uses [Velopack](https://velopack.io).

Each release is checked by an end-to-end update test on Windows, macOS and Linux.

## Build from source

Install Rust with [rustup](https://rustup.rs); the toolchain is pinned in
`rust-toolchain.toml` and installed on first use.

- **Windows:** Visual Studio Build Tools with the Windows SDK, and LLVM (libclang, for
  the Whisper bindings; set `LIBCLANG_PATH` if it is not in `C:\Program Files\LLVM\bin`).
  The encrypted storage builds a vendored OpenSSL, which needs a native Perl such as
  [Strawberry Perl](https://strawberryperl.com); Git's Perl does not work. Point
  `OPENSSL_SRC_PERL` at its `perl.exe` if it is not first on your PATH.
- **macOS:** Xcode command line tools and CMake.
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
keep their own licences; their texts are included in every package, next to Acta's own
(`licenses/Acta-LICENSE.txt`).
