<p align="center">
  <img src="assets/icons/acta.png" width="96" height="96" alt="Acta app icon">
</p>

<h1 align="center">Acta</h1>

<p align="center">
  <strong>The project tracker you and Claude Code share.</strong><br>
  You see where the work stands; Claude Code starts every session already knowing it.
</p>

<p align="center">
  <a href="https://github.com/Alegzandr/acta/releases/latest">Download</a> ·
  <a href="#get-started">Get started</a> ·
  <a href="docs/install.md">Install guide</a> ·
  <a href="LICENSE">MIT licence</a>
</p>

## Why

Claude Code starts each session from zero. You explain the context again, decisions get
lost in old conversations, and the `CLAUDE.md` file keeps growing until nobody reads it.

Acta keeps the work in one place, split into small topics, that both of you use:

- **You** get a tracker: projects, epics and topics, each with a status, a short
  "where we are", and a timeline of what was done and decided.
- **Claude Code** gets that same tracker as its memory. When a session starts, it is
  briefed on the topic it works on, only that topic, in a few lines. As it works, it
  logs progress and decisions and saves what it must remember.

The next session picks up where the last one stopped, and you can read, correct or
delete anything Claude Code remembers.

What Claude Code sees at the start of a session (an example):

```
Acta briefing (project tracker and memory; see the acta:using-acta skill).
T-12 Resume interrupted downloads · active
State (11-21, claude): Range requests work and are tested. Next: retry with backoff.
#4 11-21 decision human  Keep the partial file next to the target, never in temp
#3 11-20 progress claude  Resume tested against a local server: cut, wrong hash, redirect
```

## Get started

You need a Claude subscription to use Claude Code. Acta installs anything else
that is missing (Claude Code itself, Git), but only after you confirm.

**1. Install.** Per user, no administrator rights; Acta opens when it is done.

Windows 10 or 11, in PowerShell:

```powershell
irm https://github.com/Alegzandr/acta/releases/latest/download/install.ps1 | iex
```

macOS (Apple silicon or Intel) and Linux (x86_64, glibc 2.35+):

```sh
curl -fsSL https://github.com/Alegzandr/acta/releases/latest/download/install.sh | sh
```

**2. Choose a folder.** Pick the repository you work on: it becomes your first project.

**3. Set up Claude Code.** Click **Set up Claude Code** (in Settings > Claude Code), or
run `acta setup` in a terminal. This adds Acta to Claude Code, for you only; `acta setup
--remove` undoes it.

**4. Work as usual.** Start Claude Code in Acta's terminal (`` Ctrl+` ``) or in your own
terminal, in that folder. Ask it to work on something; Acta fills itself in as it goes.

Acta updates itself when it starts. Builds are not signed (the project has no budget
for certificates); the commands above avoid most warnings, and the
[install guide](docs/install.md) covers browser downloads, checksums and uninstalling.

## Also in Acta

- **Notes inbox.** Jot things down (`Ctrl+N`); Claude Code turns them into topics.
- **Knowledge base.** Documents and memories on each project, epic or topic, that
  Claude Code reads only when they matter.
- **Your repositories.** Folders per page, with branch state and the commit mode you
  allow Claude Code (manual, commit, or commit and push).
- **Questions and contacts.** Track what you asked of whom, and who you report to.
- **Files and voice.** Attach files to notes; dictate with a local Whisper model.
- **Keyboard first.** One command palette (`Ctrl+K`) reaches everything.

On macOS, `Cmd` replaces `Ctrl`, except for the terminal's `` Ctrl+` ``.

## Your data stays yours

Everything is in one encrypted file on your computer. The key is in Windows Credential
Manager or the Linux Secret Service; on macOS it is a private file beside the data,
because unsigned builds would otherwise ask for your password after every update
([why](docs/decisions/0003-storage.md)). No account, no Acta server, no telemetry.
Export to an open JSON format at any time.

## Status

Alpha: used every day to build Acta itself, with rough edges. What is planned and open
is in [PLAN.md](PLAN.md); report problems in
[issues](https://github.com/Alegzandr/acta/issues).

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md); building from source is in
[docs/building.md](docs/building.md). Security problems are reported privately, as
[SECURITY.md](SECURITY.md) explains. Design choices live in [DESIGN.md](DESIGN.md) and
[docs/decisions/](docs/decisions/).

## Licence

[MIT](LICENSE). Third-party components keep their own licences; their texts ship in
every package.
