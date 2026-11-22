# 0013 - Ready for Claude Code: detecting and installing what it needs

Status: proposed
Date: 2026-11-22

## Context
The user asked on 2026-11-22 whether Acta can make sure what Claude Code needs is there from the start, installing it as a dependency (examples given: Python, git, Claude Code). It follows the principle settled on 2026-11-21 (PLAN.md, "Settled with the user"): working in Acta means the fewest logins possible and Claude Code having, as directly as possible, everything it needs.

Today Acta only reports "Claude Code found" or "not found: install Claude Code" (`acta setup --check`, Settings > Claude Code, ADR 0005). It finds `claude` the way the terminal does (`acta-terminal::launch::resolve_claude`: `claude.exe` on `PATH` or in `~/.local/bin`, else a `claude.cmd` shim; a POSIX login shell on macOS and Linux) and finds Git Bash for the status line (`acta-cli::statusline::shell_for`).

Forces: CLAUDE.md (zero budget, unsigned builds, nothing that cannot be undone without asking, no plaintext user data on disk, in logs or in process arguments, never take focus or send input); ADR 0004 (per-user install, no admin); ADR 0005 (`acta setup` idempotent, reversible, never clobbers what it did not write).

Facts checked on 2026-11-22 (docs read, nothing installed on this machine; `winget show`, which installs nothing, was run with winget 1.29.380):

| Fact | Source |
|------|--------|
| Claude Code's recommended install is the native installer: `curl -fsSL https://claude.ai/install.sh \| bash` (macOS, Linux, WSL), `irm https://claude.ai/install.ps1 \| iex` (Windows PowerShell). No Administrator on Windows. Launcher `~/.local/bin/claude` (`%USERPROFILE%\.local\bin\claude.exe`), versions under `~/.local/share/claude/versions/`. It updates itself in the background. | code.claude.com/docs/en/setup |
| Other official channels: `brew install --cask claude-code` (stable channel) or `claude-code@latest`; `winget install Anthropic.ClaudeCode`; signed apt, dnf and apk repositories; `npm install -g @anthropic-ai/claude-code@latest` (Node.js 22+, same native binary). Homebrew, WinGet and Linux packages do not auto-update by default (`CLAUDE_CODE_PACKAGE_MANAGER_AUTO_UPDATE=1` opts Homebrew and WinGet in). | code.claude.com/docs/en/setup |
| The WinGet package `Anthropic.ClaudeCode` 2.1.330 is a `portable` installer of `downloads.claude.ai/.../claude.exe` with a SHA256. | github.com/microsoft/winget-pkgs `manifests/a/Anthropic/ClaudeCode/2.1.330` |
| Requirements: macOS 13+, Windows 10 1809+, Ubuntu 20.04+, Debian 10+, Alpine 3.19+ (needs `bash` and `curl`); 4 GB RAM. ripgrep is "usually included" (bundled); only if search fails does the user install a system ripgrep and set `USE_BUILTIN_RIPGREP=0`. Git is optional (2.31+ for `claude plugin eval` and repository uploads). | code.claude.com/docs/en/setup, /troubleshooting |
| Windows: Git for Windows is optional but provides Git Bash, which the Bash tool and the Monitor tool need; without it Claude Code uses the PowerShell tool. `CLAUDE_CODE_GIT_BASH_PATH` points it at a non-standard `bash.exe`. | code.claude.com/docs/en/setup, /tools-reference |
| Claude Code links a session to a pull request when it creates it with `gh pr create`. | code.claude.com/docs/en/common-workflows |
| Integrity: each release has a GPG-signed `manifest.json` with SHA256 per binary (key fingerprint `31DD DE24 DDFA B679 F42D 7BD2 BAA9 29FF 1A7E CACE`); Windows binaries are Authenticode-signed by "Anthropic, PBC", macOS binaries signed by "Anthropic PBC" and notarized; Linux binaries are not individually signed (the apt/dnf/apk repositories are). | code.claude.com/docs/en/setup |
| `claude --version` prints `2.1.330 (Claude Code)`. `claude auth status` prints JSON and exits 0 when logged in, 1 when not; its `authMethod` is one of `none`, `claude.ai`, `oauth_token`, `api_key`, `api_key_helper`, `third_party`. `claude update` updates a native install. `claude doctor` is read-only diagnostics (free text, not a stable format). | code.claude.com/docs/en/cli-reference |
| WinGet ships with App Installer on Windows 10 1809+ and 11; it may be missing (then App Installer comes from the Microsoft Store). It verifies each installer against the manifest's SHA256 (`--ignore-security-hash` exists to skip that; Acta never passes it). `--scope user` filters installers; "packages installed in user scope may still require UAC". Default mode shows the installer's progress; `--silent` hides all UI. | learn.microsoft.com/windows/package-manager/winget/, /install, /troubleshooting |
| `Git.Git` 2.56.1 (Inno) has user and machine scope installers and `ElevationRequirement: elevatesSelf`. git-scm.com gives `winget install --id Git.Git -e --source winget` and notes the winget package is not maintained by the Git for Windows project. `winget show --scope user` selects the Inno installer. | winget-pkgs `manifests/g/Git/Git/2.56.1`, git-scm.com/install/windows, run here |
| `GitHub.cli` 2.105.0 has machine-scope MSIs and a portable zip with no scope; `winget show --id GitHub.cli --scope user` selects the portable zip (per user, no elevation). | winget-pkgs `manifests/g/GitHub/cli/2.105.0`, run here |
| macOS: Apple ships Git with the Xcode Command Line Tools (`xcode-select --install`); `brew install git` with Homebrew. Homebrew itself is installed by a script that needs a writable `/opt/homebrew` (admin on first install) and now supports macOS 15+ on Apple Silicon as tier 1. On a Mac without the tools, `/usr/bin/git` is a stub that opens Apple's install dialog when run. | git-scm.com/install/mac, docs.brew.sh/Installation |
| Linux: `apt-get install git`, `dnf install git`, `pacman -S git`, `zypper install git`, `apk add git`, all as root. gh: official apt repository from cli.github.com (adds a key and a source); community packages `dnf install gh` (Fedora), `pacman -S github-cli` (Arch), `zypper install gh` (openSUSE); Ubuntu's own `gh` 2.45/2.46 is "broken due to deprecated GitHub APIs"; never the snap. | git-scm.com/install/linux, github.com/cli/cli/blob/trunk/docs/install_linux.md |

## Decision

### 1. What is checked: four rows, no more
| Row | Level | Why it is there (the row's one line) | Minimum |
|-----|-------|--------------------------------------|---------|
| Claude Code | required | It is what runs in Acta's terminal and does the work. | `CLAUDE_MIN`, the oldest version Acta's integration is proven on, a constant in `acta-core` (2.1.329 today; raised when Acta relies on something newer) |
| Signed in to Claude | required | Claude Code needs the user's Claude subscription; the sign-in is the user's own. | - |
| Git (on Windows: Git for Windows, with Git Bash) | recommended | Commits, branches and worktrees (ADR 0011); on Windows, Git Bash is the shell Claude Code's Bash and Monitor tools need. | 2.31 (Claude Code's own floor for the features that need git) |
| GitHub CLI | optional, shown only when an attached folder has a `github.com` remote, or `gh` is already installed | Lets Claude Code open pull requests and read issues. | 2.47 (below it, GitHub calls the packaged versions broken) |

Not checked, and why:
- **Python**: Claude Code does not need it. A project that needs Python needs a specific version and tooling that only the project knows; installing one globally would be a guess that conflicts with pyenv, uv, conda or the system Python. Claude Code can install what a project needs, in the terminal, with the user watching.
- **Node.js**: not needed; the native installer and the npm package both run a native binary.
- **ripgrep**: bundled with Claude Code. Only a broken bundled copy needs a system one, which `claude doctor` reports; Acta does not duplicate that check.
- **Homebrew, winget, WSL**: package managers and subsystems are the user's choice. Acta uses one that is there and never installs one.

### 2. Detection: read-only, off the UI thread, never surprising
- **Where to look.** One resolver in `acta-core` (`prereqs::locate`) shared by the checklist, `acta setup --check` and the terminal: the process `PATH`; on Windows, the `PATH` as it is now in the registry (`HKCU\Environment` and `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`, `REG_EXPAND_SZ` expanded), since a running Acta keeps the `PATH` it started with and an install changes the registry only; then known per-user and system folders per tool (Claude Code: `~/.local/bin`; Windows: `%LOCALAPPDATA%\Microsoft\WinGet\Links`, `%LOCALAPPDATA%\Programs\Git\cmd`, `%ProgramFiles%\Git\cmd`, `%ProgramFiles%\GitHub CLI`; macOS: `/opt/homebrew/bin`, `/usr/local/bin`; Linux: `/usr/bin`, `/usr/local/bin`). The fresh `PATH` is also what Acta passes to every child it starts (terminal included), with `Command::env`, never by changing its own environment.
- **Version.** Run the found executable with `--version` (argv, no shell, no window, 5 s limit, Claude Code session variables removed as the terminal does) and parse the first `major.minor.patch` of the first line. A `claude.cmd` shim runs through `cmd.exe /d /c` with the existing metacharacter refusal. Fixtures: `2.1.330 (Claude Code)`, `git version 2.56.1.windows.1`, `git version 2.39.5 (Apple Git-154)`, `gh version 2.105.0 (2026-11-18)`.
- **macOS Git.** `/usr/bin/git` is never run unless `xcode-select -p` exits 0 (or a Homebrew git is found first): running the stub would open Apple's install dialog, which is an install the user did not ask for.
- **Git Bash** (Windows): the order Acta already uses (`CLAUDE_CODE_GIT_BASH_PATH`, next to `git.exe`, the usual folders) moves into the same resolver; the Git row says "with Git Bash" or, when git is found without it, that Claude Code will use PowerShell.
- **Sign-in.** `claude auth status` (argv, 10 s limit): exit 0 is signed in, 1 is not. Acta reads only `authMethod` from the JSON, to say "Using an API key" for `api_key` and `api_key_helper` (Acta targets a subscription; not an error), and discards the rest, email included; nothing of it is logged or stored. `gh auth status` likewise by exit code only, output discarded.
- **How the install was made** (for updates): by the path of the resolved executable: `~/.local/bin` is the native installer; `WinGet\Links` or `WinGet\Packages` is winget; `/opt/homebrew` or `Caskroom` is Homebrew; `node_modules` or an npm prefix is npm; `/usr/bin` on Linux is the system package manager. Unknown is a valid answer.
- **When.** When the checklist is shown (first run, Settings), when the window regains focus while it is shown (at most once per 10 s), after an install or a sign-in session ends, and once at launch for the Claude Code and sign-in rows only (the status bar item). Never a network call.

### 3. Installing: one click, one command, the vendor's or the system's
- **Only on the user's click**, from a row, after a confirmation dialog that names the publisher, where the bits come from, the scope (for you only, or for all users with administrator rights), who updates it afterwards, and shows **the exact command** in `type.code` with Copy. Never at startup, never in the background, never chained (installing Git does not install the GitHub CLI). One install runs at a time.
- **Commands are compile-time constants of Acta**, built as argv (`Vec<OsString>`), never by formatting a shell string from a variable. The two vendor one-liners (the native installer of Claude Code) are run exactly as the vendor documents them, as a constant third argument of `bash -c` or `powershell -Command`; a test asserts they equal the documented text. No package id, URL or flag comes from a file, the network or the user.
- **Sources, in order of preference**: the vendor's official installer where the vendor recommends it (Claude Code's native installer: per-user, no admin, self-updating, and the binary is Authenticode- or Apple-signed by Anthropic); else the OS's package manager with the vendor's official package (winget on Windows; Homebrew on macOS only when it is already installed; Apple's own `xcode-select --install` for Git on macOS; the distribution's package manager on Linux). Never Chocolatey, Scoop, snap or a community mirror; never `--ignore-security-hash`.
- **Per user where possible.** winget always gets `--scope user --accept-package-agreements --accept-source-agreements --disable-interactivity` (winget has no console to ask on; the dialog says that installing accepts the publisher's licence and the catalogue's terms) and never `--silent`, so the installer's own progress shows (Git's Inno installer has a user installer; GitHub CLI's user choice is the portable zip). When the package can still raise UAC (Git, `elevatesSelf`), the dialog says "Windows may still ask for permission". Linux system packages need root: their command (`sudo apt-get install git`, ...) runs as a one-off command in a new session of Acta's terminal (argv started directly in the PTY, no shell), where `sudo` asks for the password itself; Acta never sees it, never uses `pkexec` or stores anything. macOS `xcode-select --install` opens Apple's dialog and returns at once; the row waits for the next focus to re-check.
- **Running.** On Windows and macOS the command is a child process with no window, its stdout and stderr captured into the row's log (in memory only, control sequences stripped, last 400 lines, gone when Acta quits; Copy log puts it on the clipboard), its exit code mapped to Installed / Failed. No timeout and no Cancel: interrupting an installer half way does more harm than waiting; the row shows the elapsed time. After success Acta re-detects with the fresh `PATH`; found means done, not found means "Installed, but Acta cannot find it yet. Restart Acta."
- **No package manager.** Without winget (Windows), without Homebrew for the GitHub CLI (macOS), on a Linux distribution Acta does not recognise (from `/etc/os-release` `ID` and `ID_LIKE`: debian/ubuntu, fedora/rhel, arch, opensuse, alpine), or for the GitHub CLI on Debian and Ubuntu (whose official route adds a third-party apt source, which Acta does not do for the user): the row's action is "Open download page" (the official page, in the system browser) and "Check again".
- **Updates** stay with the package manager or the vendor: the native Claude Code updates itself; winget, Homebrew and Linux packages are upgraded by the user. Only an **outdated** row offers an action, with the command for the detected install kind: `claude update` (native), `winget upgrade --id <id> -e --source winget` (winget), `brew upgrade claude-code` or `brew upgrade gh` (Homebrew), the distribution's command in Acta's terminal (Linux packages); npm and unknown kinds get the command with Copy only.
- **Sign-in** stays the user's own: "Sign in..." opens Acta's terminal on Claude Code (an ordinary session, in the open project's folder or the home folder), whose own first-run flow opens the browser. The GitHub CLI's "Sign in..." runs `gh auth login --web` as a one-off command in Acta's terminal. Acta never handles a password, token or API key, and never sets `ANTHROPIC_API_KEY`.
- **No bundling.** Acta ships no third-party executable for these tools: licences to carry, tens to hundreds of MB per platform, and security updates Acta would then owe. The only strong reason would be a tool with no official channel on a supported platform; none of the four has that.

### 4. Where it shows
- **Settings > Claude Code** starts with "Ready for Claude Code": the four rows (GitHub CLI when it applies), above the existing setup checks; the old "Claude Code found" check is replaced by the Claude Code row.
- **First run** (screens.md 8.5) adds the same block above "Start with a project" only while a required or recommended row is not ready; GitHub CLI never appears there (no folder is attached yet).
- **Status bar**: the Claude Code item reads "Claude Code: not installed" or "Claude Code: not signed in" when a required row is not ready (before setup status), and opens Settings at the block.
- **`acta setup --check`** gains the rows (section 5). No MCP tool: the agent does not install software; it can read `acta setup --check`.
- `acta setup --remove` never uninstalls these tools: once installed they are the user's software, used outside Acta too. The confirmation dialog says how to uninstall (the package manager's own command), and nothing records that Acta installed them.

### 5. `acta setup --check` additions
Rows first, before the plugin steps, one line each, the whole output under 1,200 chars (tested). Text form `name  state  detail`:

```text
claude       ok        2.1.330, native install (updates itself)
signed in    ok        claude.ai
git          ok        2.56.1, with Git Bash
gh           optional  not installed: winget install --id GitHub.cli -e --source winget --scope user
executable   ok        found
...
```

States: `ok`, `missing` (required or recommended), `outdated`, `optional` (an optional row that is missing; never fails the exit code), `unknown` (could not run it). A missing or outdated row prints the exact command Acta would run for this machine, or the download page. `--json` adds `state`, `version`, `kind` (native, winget, homebrew, npm, system, unknown) and `command` (an argv array) to each step. The exit code fails only for required rows and the existing plugin steps.

## Alternatives
- Installing silently at first launch: against CLAUDE.md (nothing irreversible without asking) and against trust; one click per tool costs little.
- Bundling git, gh or ripgrep inside Acta: licences, size, and Acta would own their security updates; unsigned Acta builds would also carry third-party binaries without their publishers' signatures.
- Installing Python, Node or Homebrew: not needed by Claude Code (Python, Node), or a system-wide choice with admin rights that belongs to the user (Homebrew).
- winget for Claude Code on Windows: it does not auto-update by default and Anthropic recommends the native installer; it stays the fallback named in the failure state.
- Running installs through a shell string assembled by Acta: injection risk and quoting bugs; argv with constants removes both.
- `pkexec` for Linux root installs: needs a polkit agent that is not always there; `sudo` in Acta's terminal works everywhere and lets the user read what runs.
- Verifying signatures in Acta (WinVerifyTrust, `codesign`, GPG): duplicates what winget (SHA256 against the manifest), Homebrew, apt/dnf/apk (repository signatures) and Anthropic's installer already enforce; revisit if a channel without verification is ever added.
- Parsing `claude doctor`: free-text output that changes between releases.

## Consequences
- A new module `acta-core::prereqs` (detection, version parsing, install kinds, command tables) used by the app, the CLI and the terminal; `resolve_claude` and the Git Bash lookup move into it.
- Acta's terminal gains a one-off command session kind (argv, a title, ends with the process) for Linux root installs and `gh auth login`.
- The command tables are a maintenance point: package ids and vendor commands can change. A weekly CI job (no installs) checks that each winget id resolves with `winget show` on the Windows runner and that the documented Claude Code commands still appear on code.claude.com/docs/en/setup; a failure opens no issue by itself, it fails the job for the lead to read.
- Elevation can still happen on Windows (Git's installer); it is said in the dialog, and a declined prompt ends as Failed with the log.
- Nothing about installs is written to disk by Acta; the log lives only in memory. The diagnostic log file gets only failures: a row that could not be checked (its name and a compile-time reason) and an install that failed (the tool and the kind: exit code, could not start, not found after install). Never a path, a version, an account or installer output; a successful check or install writes nothing.
- Revisit when Claude Code adds or drops a dependency (its setup page), when Homebrew or winget change scope behaviour, or if users ask for more rows (each new row needs an agent workflow that uses it, as with tools).

## Evidence
Docs only; no spike, no install. Read on 2026-11-22: code.claude.com/docs/en/{setup, troubleshooting, tools-reference, common-workflows, cli-reference}; learn.microsoft.com/windows/package-manager/winget/{index, install, settings, troubleshooting}; github.com/microsoft/winget-pkgs manifests of `Anthropic.ClaudeCode` 2.1.330, `Git.Git` 2.56.1, `GitHub.cli` 2.105.0; git-scm.com/install/{windows, mac, linux}; docs.brew.sh/Installation; github.com/cli/cli/blob/trunk/docs/install_linux.md.

Run on Windows 11 (read-only): `winget show --id GitHub.cli -e --source winget --scope user` selects the portable zip; `winget show --id Git.Git ... --scope user` the Inno installer; `winget show --id Anthropic.ClaudeCode` the portable `claude.exe`; `winget show --id BurntSushi.ripgrep.MSVC --scope user` a portable zip.

Not verified, to be proven when built: a real `--scope user` install of Git for Windows without UAC; the `/usr/bin/git` stub behaviour on a Mac without the Command Line Tools (CI runners have Xcode); `sudo` prompting in Acta's terminal on Linux; `claude auth status` on a signed-out machine.

## Note (2026-11-22): the weekly check is built
`.github/workflows/weekly.yml`, job `prereqs` (Mondays 06:17 UTC and on demand, `windows-latest`), runs `scripts/ci/check-prereqs.ps1`. It reads `WINGET_CLAUDE`, `WINGET_GIT`, `WINGET_GH`, `CLAUDE_INSTALL_SH`, `CLAUDE_INSTALL_PS1` and `PAGE_CLAUDE` from `acta-core::prereqs::commands`, so the check follows the constants Acta runs. For each id: `winget show --id <id> -e --source winget --scope user --accept-source-agreements --disable-interactivity`, which installs nothing; a non-zero exit fails the job. The setup page is fetched and its HTML tags stripped before the two one-liners are searched for as plain text (the page highlights code token by token, so the raw HTML never holds them whole). A table goes to the run's summary.

Run locally on 2026-11-22 (winget 1.29.380): all five checks pass; an unknown id exits non-zero. Not proven until the first run on GitHub: winget on the `windows-latest` image. Windows Server 2025 ships winget, but registers it for a user at their first sign-in; the script registers App Installer when `winget` is missing (Microsoft's documented `Add-AppxPackage -RegisterByFamilyName`) and, failing that, bootstraps it with Microsoft's `Microsoft.WinGet.Client` module pinned at 1.29.380 (`Repair-WinGetPackageManager`), on the runner only. Also unproven: whether `--scope user` makes `winget show` fail when a package loses its user-scope installer (no such package was at hand to try).
