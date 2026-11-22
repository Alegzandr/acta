//! What Acta would run to install or update a tool (ADR 0013, section 3).
//!
//! Every command is a compile-time constant built as argv: no element
//! comes from a file, the network, the user or a path found on disk. The
//! two vendor one-liners (Claude Code's native installer) are run exactly
//! as the vendor documents them, as the constant script of `bash -c` or
//! `powershell -Command`. When no package manager Acta uses is there, the
//! answer is the official download page.

use super::{Kind, Tool};
use crate::paths::Os;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The native installer, macOS and Linux (code.claude.com/docs/en/setup).
pub const CLAUDE_INSTALL_SH: &str = "curl -fsSL https://claude.ai/install.sh | bash";
/// The native installer, Windows PowerShell (code.claude.com/docs/en/setup).
pub const CLAUDE_INSTALL_PS1: &str = "irm https://claude.ai/install.ps1 | iex";

/// The flags every winget command carries: one exact package from the
/// winget catalogue, per user, and no console prompt (winget has none to
/// ask on; the dialog says installing accepts the terms). Never `--silent`
/// (the installer's own progress shows) nor `--ignore-security-hash`.
pub const WINGET_FLAGS: [&str; 8] = [
    "-e",
    "--source",
    "winget",
    "--scope",
    "user",
    "--accept-package-agreements",
    "--accept-source-agreements",
    "--disable-interactivity",
];

pub const WINGET_CLAUDE: &str = "Anthropic.ClaudeCode";
pub const WINGET_GIT: &str = "Git.Git";
pub const WINGET_GH: &str = "GitHub.cli";

pub const PAGE_CLAUDE: &str = "https://code.claude.com/docs/en/setup";
pub const PAGE_GIT_WINDOWS: &str = "https://git-scm.com/install/windows";
pub const PAGE_GIT_MAC: &str = "https://git-scm.com/install/mac";
pub const PAGE_GIT_LINUX: &str = "https://git-scm.com/install/linux";
pub const PAGE_GH: &str = "https://cli.github.com/";
pub const PAGE_GH_LINUX: &str = "https://github.com/cli/cli/blob/trunk/docs/install_linux.md";

/// Linux families Acta knows (from `/etc/os-release` `ID` and `ID_LIKE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Distro {
    /// Debian, Ubuntu and their derivatives (apt).
    Debian,
    /// Fedora itself (dnf, with the community `gh`).
    Fedora,
    /// RHEL and its rebuilds (dnf, no `gh` package).
    Rhel,
    Arch,
    Opensuse,
    Alpine,
}

impl Distro {
    /// From the text of `/etc/os-release`.
    pub fn from_os_release(text: &str) -> Option<Distro> {
        let field = |key: &str| {
            text.lines()
                .filter_map(|l| l.trim().split_once('='))
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.trim().trim_matches(['"', '\'']).to_ascii_lowercase())
                .unwrap_or_default()
        };
        let id = field("ID");
        let like = field("ID_LIKE");
        let words: Vec<&str> = std::iter::once(id.as_str())
            .chain(like.split_whitespace())
            .collect();
        let any = |names: &[&str]| words.iter().any(|w| names.contains(w));
        if id == "fedora" {
            Some(Distro::Fedora)
        } else if any(&["debian", "ubuntu"]) {
            Some(Distro::Debian)
        } else if any(&["rhel", "centos", "fedora"]) {
            Some(Distro::Rhel)
        } else if any(&["arch"]) {
            Some(Distro::Arch)
        } else if any(&[
            "opensuse",
            "suse",
            "opensuse-leap",
            "opensuse-tumbleweed",
            "sles",
        ]) {
            Some(Distro::Opensuse)
        } else if any(&["alpine"]) {
            Some(Distro::Alpine)
        } else {
            None
        }
    }

    /// The package manager's executable.
    pub fn manager(self) -> &'static str {
        match self {
            Distro::Debian => "apt-get",
            Distro::Fedora | Distro::Rhel => "dnf",
            Distro::Arch => "pacman",
            Distro::Opensuse => "zypper",
            Distro::Alpine => "apk",
        }
    }
}

/// What decides the command: the OS, the distribution and which package
/// managers are there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub distro: Option<Distro>,
    pub winget: bool,
    pub brew: bool,
    /// The distribution's own package manager is installed.
    pub distro_manager: bool,
}

impl Platform {
    /// From the search directories of a machine.
    pub fn detect(m: &super::Machine, is_file: &dyn Fn(&Path) -> bool) -> Platform {
        let has = |name: &str| super::locate::locate(m.os, name, &m.dirs, is_file).is_some();
        let distro = (m.os == Os::Linux)
            .then(|| std::fs::read_to_string("/etc/os-release").ok())
            .flatten()
            .and_then(|t| Distro::from_os_release(&t));
        Platform {
            os: m.os,
            distro,
            winget: m.os == Os::Windows && has("winget"),
            brew: m.os == Os::MacOs
                && (has("brew")
                    || ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
                        .iter()
                        .any(|p| is_file(&PathBuf::from(p)))),
            distro_manager: distro.is_some_and(|d| has(d.manager())),
        }
    }
}

/// Where the command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunsIn {
    /// A child with no window; its output goes to the row's log.
    Child,
    /// A one-off session of Acta's terminal (root installs: `sudo` asks
    /// for the password itself).
    Terminal,
    /// It opens Apple's dialog and returns at once (`xcode-select --install`).
    AppleDialog,
    /// Shown with Copy only (npm, which Acta does not run for the user).
    Copy,
}

/// Who it is installed for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// For this user only, no administrator rights.
    User,
    /// For all users, with administrator rights.
    Machine,
}

/// Where the bits come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The publisher's own installer.
    Vendor,
    Winget,
    Homebrew,
    Apple,
    /// The Linux distribution's packages.
    Distribution,
    Npm,
}

/// One command, ready to confirm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub argv: Vec<OsString>,
    pub runs_in: RunsIn,
    pub scope: Scope,
    pub source: Source,
    /// The tool's official page (the fallback the dialog can name).
    pub download_url: &'static str,
    /// Windows may still ask for permission (Git's installer elevates
    /// itself when it wants to).
    pub may_elevate: bool,
}

impl Plan {
    fn new(
        argv: &[&'static str],
        runs_in: RunsIn,
        scope: Scope,
        source: Source,
        page: &'static str,
    ) -> Plan {
        Plan {
            argv: argv.iter().map(OsString::from).collect(),
            runs_in,
            scope,
            source,
            download_url: page,
            may_elevate: false,
        }
    }

    /// The argv as text: what the dialog and `acta setup --check` show.
    pub fn display(&self) -> String {
        self.argv
            .iter()
            .map(|a| {
                let a = a.to_string_lossy();
                if a.contains(' ') {
                    format!("\"{a}\"")
                } else {
                    a.into_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The argv as strings (`--json`).
    pub fn argv_strings(&self) -> Vec<String> {
        self.argv
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }
}

/// A command, or the page to install from by hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Run(Plan),
    Page(&'static str),
}

fn winget(verb: &'static str, id: &'static str, page: &'static str) -> Choice {
    let mut argv = vec!["winget", verb, "--id", id];
    argv.extend(WINGET_FLAGS);
    let mut plan = Plan::new(&argv, RunsIn::Child, Scope::User, Source::Winget, page);
    plan.may_elevate = id == WINGET_GIT;
    Choice::Run(plan)
}

fn page_for(tool: Tool, os: Os) -> &'static str {
    match (tool, os) {
        (Tool::Claude, _) => PAGE_CLAUDE,
        (Tool::Git, Os::Windows) => PAGE_GIT_WINDOWS,
        (Tool::Git, Os::MacOs) => PAGE_GIT_MAC,
        (Tool::Git, Os::Linux) => PAGE_GIT_LINUX,
        (Tool::Gh, Os::Linux) => PAGE_GH_LINUX,
        (Tool::Gh, _) => PAGE_GH,
    }
}

fn terminal(argv: &[&'static str], page: &'static str) -> Choice {
    Choice::Run(Plan::new(
        argv,
        RunsIn::Terminal,
        Scope::Machine,
        Source::Distribution,
        page,
    ))
}

/// The install command for a missing tool.
pub fn install_plan(tool: Tool, p: &Platform) -> Choice {
    let page = page_for(tool, p.os);
    match (tool, p.os) {
        (Tool::Claude, Os::Windows) => Choice::Run(Plan::new(
            &[
                "powershell.exe",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                CLAUDE_INSTALL_PS1,
            ],
            RunsIn::Child,
            Scope::User,
            Source::Vendor,
            page,
        )),
        (Tool::Claude, _) => Choice::Run(Plan::new(
            &["bash", "-c", CLAUDE_INSTALL_SH],
            RunsIn::Child,
            Scope::User,
            Source::Vendor,
            page,
        )),
        (Tool::Git, Os::Windows) if p.winget => winget("install", WINGET_GIT, page),
        (Tool::Gh, Os::Windows) if p.winget => winget("install", WINGET_GH, page),
        (Tool::Git, Os::MacOs) => Choice::Run(Plan::new(
            &["xcode-select", "--install"],
            RunsIn::AppleDialog,
            Scope::Machine,
            Source::Apple,
            page,
        )),
        (Tool::Gh, Os::MacOs) if p.brew => Choice::Run(Plan::new(
            &["brew", "install", "gh"],
            RunsIn::Child,
            Scope::Machine,
            Source::Homebrew,
            page,
        )),
        (_, Os::Linux) if p.distro_manager => match (tool, p.distro) {
            (Tool::Git, Some(Distro::Debian)) => {
                terminal(&["sudo", "apt-get", "install", "git"], page)
            }
            (Tool::Git, Some(Distro::Fedora | Distro::Rhel)) => {
                terminal(&["sudo", "dnf", "install", "git"], page)
            }
            (Tool::Git, Some(Distro::Arch)) => terminal(&["sudo", "pacman", "-S", "git"], page),
            (Tool::Git, Some(Distro::Opensuse)) => {
                terminal(&["sudo", "zypper", "install", "git"], page)
            }
            (Tool::Git, Some(Distro::Alpine)) => terminal(&["sudo", "apk", "add", "git"], page),
            (Tool::Gh, Some(Distro::Fedora)) => terminal(&["sudo", "dnf", "install", "gh"], page),
            (Tool::Gh, Some(Distro::Arch)) => {
                terminal(&["sudo", "pacman", "-S", "github-cli"], page)
            }
            (Tool::Gh, Some(Distro::Opensuse)) => {
                terminal(&["sudo", "zypper", "install", "gh"], page)
            }
            // Debian and Ubuntu: the official route adds a third-party apt
            // source, which Acta does not do for the user.
            _ => Choice::Page(page),
        },
        _ => Choice::Page(page),
    }
}

/// The update command for an outdated tool, by how it was installed.
pub fn update_plan(tool: Tool, kind: Kind, p: &Platform) -> Choice {
    let page = page_for(tool, p.os);
    let id = match tool {
        Tool::Claude => WINGET_CLAUDE,
        Tool::Git => WINGET_GIT,
        Tool::Gh => WINGET_GH,
    };
    match (tool, kind) {
        (Tool::Claude, Kind::Native) => Choice::Run(Plan::new(
            &["claude", "update"],
            RunsIn::Child,
            Scope::User,
            Source::Vendor,
            page,
        )),
        (_, Kind::Winget) if p.os == Os::Windows && p.winget => winget("upgrade", id, page),
        (_, Kind::Homebrew) if p.brew || p.os != Os::Windows => {
            let argv: &[&'static str] = match tool {
                Tool::Claude => &["brew", "upgrade", "claude-code"],
                Tool::Git => &["brew", "upgrade", "git"],
                Tool::Gh => &["brew", "upgrade", "gh"],
            };
            Choice::Run(Plan::new(
                argv,
                RunsIn::Child,
                Scope::Machine,
                Source::Homebrew,
                page,
            ))
        }
        (Tool::Claude, Kind::Npm) => Choice::Run(Plan::new(
            &["npm", "install", "-g", "@anthropic-ai/claude-code@latest"],
            RunsIn::Copy,
            Scope::User,
            Source::Npm,
            page,
        )),
        (_, Kind::System) if p.os == Os::Linux && p.distro_manager => match (tool, p.distro) {
            (Tool::Claude, Some(Distro::Fedora | Distro::Rhel)) => {
                terminal(&["sudo", "dnf", "upgrade", "claude-code"], page)
            }
            (Tool::Git, Some(Distro::Debian)) => terminal(
                &["sudo", "apt-get", "install", "--only-upgrade", "git"],
                page,
            ),
            (Tool::Git, Some(Distro::Fedora | Distro::Rhel)) => {
                terminal(&["sudo", "dnf", "upgrade", "git"], page)
            }
            (Tool::Git, Some(Distro::Arch)) => terminal(&["sudo", "pacman", "-S", "git"], page),
            (Tool::Git, Some(Distro::Opensuse)) => {
                terminal(&["sudo", "zypper", "update", "git"], page)
            }
            (Tool::Git, Some(Distro::Alpine)) => terminal(&["sudo", "apk", "upgrade", "git"], page),
            (Tool::Gh, Some(Distro::Fedora)) => terminal(&["sudo", "dnf", "upgrade", "gh"], page),
            (Tool::Gh, Some(Distro::Arch)) => {
                terminal(&["sudo", "pacman", "-S", "github-cli"], page)
            }
            (Tool::Gh, Some(Distro::Opensuse)) => {
                terminal(&["sudo", "zypper", "update", "gh"], page)
            }
            _ => Choice::Page(page),
        },
        _ => Choice::Page(page),
    }
}

/// Claude Code with winget, when the native installer failed on Windows
/// ("Install with winget instead").
pub fn winget_alternative(tool: Tool, p: &Platform) -> Option<Plan> {
    if tool != Tool::Claude || p.os != Os::Windows || !p.winget {
        return None;
    }
    match winget("install", WINGET_CLAUDE, PAGE_CLAUDE) {
        Choice::Run(plan) => Some(plan),
        Choice::Page(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every platform Acta distinguishes.
    fn platforms() -> Vec<Platform> {
        let mut out = Vec::new();
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            let distros: Vec<Option<Distro>> = if os == Os::Linux {
                vec![
                    None,
                    Some(Distro::Debian),
                    Some(Distro::Fedora),
                    Some(Distro::Rhel),
                    Some(Distro::Arch),
                    Some(Distro::Opensuse),
                    Some(Distro::Alpine),
                ]
            } else {
                vec![None]
            };
            for distro in distros {
                for flags in 0..8u8 {
                    out.push(Platform {
                        os,
                        distro,
                        winget: flags & 1 != 0,
                        brew: flags & 2 != 0,
                        distro_manager: flags & 4 != 0,
                    });
                }
            }
        }
        out
    }

    const KINDS: [Kind; 6] = [
        Kind::Native,
        Kind::Winget,
        Kind::Homebrew,
        Kind::Npm,
        Kind::System,
        Kind::Unknown,
    ];

    fn every_choice() -> Vec<(Platform, Tool, Option<Kind>, Choice)> {
        let mut out = Vec::new();
        for p in platforms() {
            for tool in Tool::ALL {
                out.push((p, tool, None, install_plan(tool, &p)));
                for kind in KINDS {
                    out.push((p, tool, Some(kind), update_plan(tool, kind, &p)));
                }
            }
        }
        out
    }

    fn plans() -> Vec<Plan> {
        every_choice()
            .into_iter()
            .filter_map(|(_, _, _, c)| match c {
                Choice::Run(p) => Some(p),
                Choice::Page(_) => None,
            })
            .collect()
    }

    /// The words a command may contain: the constants of this module and
    /// nothing else. A word built from runtime input (a path, a version,
    /// an id read from somewhere) would not be in it.
    #[test]
    fn every_entry_is_argv_of_constants() {
        let allowed = [
            "winget",
            "install",
            "upgrade",
            "--id",
            WINGET_CLAUDE,
            WINGET_GIT,
            WINGET_GH,
            "-e",
            "--source",
            "--scope",
            "user",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            CLAUDE_INSTALL_PS1,
            "bash",
            "-c",
            CLAUDE_INSTALL_SH,
            "xcode-select",
            "--install",
            "brew",
            "gh",
            "git",
            "claude-code",
            "claude",
            "update",
            "npm",
            "-g",
            "@anthropic-ai/claude-code@latest",
            "sudo",
            "apt-get",
            "dnf",
            "pacman",
            "-S",
            "zypper",
            "apk",
            "add",
            "github-cli",
            "--only-upgrade",
        ];
        for plan in plans() {
            assert!(!plan.argv.is_empty());
            for word in &plan.argv {
                let w = word.to_str().unwrap();
                assert!(allowed.contains(&w), "{w:?} in {}", plan.display());
            }
        }
    }

    #[test]
    fn winget_entries_carry_the_safe_flags() {
        let want = [
            "-e",
            "--source",
            "winget",
            "--scope",
            "user",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ];
        let mut seen = 0;
        for plan in plans().iter().chain(
            winget_alternative(
                Tool::Claude,
                &Platform {
                    os: Os::Windows,
                    distro: None,
                    winget: true,
                    brew: false,
                    distro_manager: false,
                },
            )
            .iter(),
        ) {
            let argv = plan.argv_strings();
            if argv[0] != "winget" {
                assert!(
                    argv.iter()
                        .all(|a| a != "--ignore-security-hash" && a != "--silent")
                );
                continue;
            }
            seen += 1;
            assert_eq!(argv[2], "--id");
            assert!([WINGET_CLAUDE, WINGET_GIT, WINGET_GH].contains(&argv[3].as_str()));
            assert_eq!(&argv[4..], want, "{}", plan.display());
            assert!(
                !argv
                    .iter()
                    .any(|a| a == "--ignore-security-hash" || a == "--silent" || a == "-h")
            );
            assert_eq!(plan.scope, Scope::User);
            assert_eq!(plan.may_elevate, argv[3] == WINGET_GIT);
        }
        assert!(seen > 0);
    }

    #[test]
    fn the_claude_one_liners_are_the_documented_text() {
        // code.claude.com/docs/en/setup, read on 2026-11-22.
        assert_eq!(
            CLAUDE_INSTALL_SH,
            "curl -fsSL https://claude.ai/install.sh | bash"
        );
        assert_eq!(
            CLAUDE_INSTALL_PS1,
            "irm https://claude.ai/install.ps1 | iex"
        );
        let any = |os| Platform {
            os,
            distro: None,
            winget: false,
            brew: false,
            distro_manager: false,
        };
        let Choice::Run(win) = install_plan(Tool::Claude, &any(Os::Windows)) else {
            panic!()
        };
        assert_eq!(win.argv.last().unwrap(), CLAUDE_INSTALL_PS1);
        assert_eq!(win.argv[win.argv.len() - 2], "-Command");
        assert_eq!(
            (win.source, win.scope, win.runs_in),
            (Source::Vendor, Scope::User, RunsIn::Child)
        );
        for os in [Os::MacOs, Os::Linux] {
            let Choice::Run(sh) = install_plan(Tool::Claude, &any(os)) else {
                panic!()
            };
            assert_eq!(sh.argv_strings(), ["bash", "-c", CLAUDE_INSTALL_SH]);
        }
    }

    #[test]
    fn linux_root_entries_use_sudo_in_the_terminal() {
        for (p, _, _, choice) in every_choice() {
            let Choice::Run(plan) = choice else { continue };
            if p.os == Os::Linux && plan.source == Source::Distribution {
                assert_eq!(plan.argv[0], "sudo", "{}", plan.display());
                assert_eq!(plan.runs_in, RunsIn::Terminal);
                assert_eq!(plan.scope, Scope::Machine);
            }
            if plan.argv[0] == "sudo" {
                assert_eq!(plan.runs_in, RunsIn::Terminal);
            }
        }
    }

    #[test]
    fn missing_managers_and_unknown_distributions_give_the_page() {
        let linux = |distro, distro_manager| Platform {
            os: Os::Linux,
            distro,
            winget: false,
            brew: false,
            distro_manager,
        };
        assert_eq!(
            install_plan(Tool::Gh, &linux(Some(Distro::Debian), true)),
            Choice::Page(PAGE_GH_LINUX)
        );
        assert_eq!(
            install_plan(Tool::Gh, &linux(Some(Distro::Rhel), true)),
            Choice::Page(PAGE_GH_LINUX)
        );
        assert_eq!(
            install_plan(Tool::Git, &linux(None, true)),
            Choice::Page(PAGE_GIT_LINUX)
        );
        assert_eq!(
            install_plan(Tool::Git, &linux(Some(Distro::Arch), false)),
            Choice::Page(PAGE_GIT_LINUX)
        );
        let Choice::Run(apt) = install_plan(Tool::Git, &linux(Some(Distro::Debian), true)) else {
            panic!()
        };
        assert_eq!(apt.argv_strings(), ["sudo", "apt-get", "install", "git"]);
        let win = Platform {
            os: Os::Windows,
            distro: None,
            winget: false,
            brew: false,
            distro_manager: false,
        };
        assert_eq!(
            install_plan(Tool::Git, &win),
            Choice::Page(PAGE_GIT_WINDOWS)
        );
        assert_eq!(install_plan(Tool::Gh, &win), Choice::Page(PAGE_GH));
        assert_eq!(winget_alternative(Tool::Claude, &win), None);
        let mac = Platform {
            os: Os::MacOs,
            ..win
        };
        assert_eq!(install_plan(Tool::Gh, &mac), Choice::Page(PAGE_GH));
        let Choice::Run(xcode) = install_plan(Tool::Git, &mac) else {
            panic!()
        };
        assert_eq!(xcode.runs_in, RunsIn::AppleDialog);
    }

    #[test]
    fn the_outdated_command_matches_the_install_kind() {
        let win = Platform {
            os: Os::Windows,
            distro: None,
            winget: true,
            brew: false,
            distro_manager: false,
        };
        let mac = Platform {
            os: Os::MacOs,
            distro: None,
            winget: false,
            brew: true,
            distro_manager: false,
        };
        let fedora = Platform {
            os: Os::Linux,
            distro: Some(Distro::Fedora),
            winget: false,
            brew: false,
            distro_manager: true,
        };
        let run = |c: Choice| match c {
            Choice::Run(p) => (p.argv_strings().join(" "), p.runs_in),
            Choice::Page(url) => (url.to_owned(), RunsIn::Copy),
        };
        assert_eq!(
            run(update_plan(Tool::Claude, Kind::Native, &win)).0,
            "claude update"
        );
        assert!(
            run(update_plan(Tool::Claude, Kind::Winget, &win))
                .0
                .starts_with("winget upgrade --id Anthropic.ClaudeCode -e")
        );
        assert_eq!(
            run(update_plan(Tool::Claude, Kind::Homebrew, &mac)).0,
            "brew upgrade claude-code"
        );
        assert_eq!(
            run(update_plan(Tool::Gh, Kind::Homebrew, &mac)).0,
            "brew upgrade gh"
        );
        assert_eq!(
            run(update_plan(Tool::Claude, Kind::Npm, &win)),
            (
                "npm install -g @anthropic-ai/claude-code@latest".to_owned(),
                RunsIn::Copy
            )
        );
        assert_eq!(
            run(update_plan(Tool::Git, Kind::System, &fedora)),
            ("sudo dnf upgrade git".to_owned(), RunsIn::Terminal)
        );
        assert_eq!(
            update_plan(Tool::Git, Kind::Unknown, &win),
            Choice::Page(PAGE_GIT_WINDOWS)
        );
        assert_eq!(
            update_plan(Tool::Gh, Kind::Unknown, &fedora),
            Choice::Page(PAGE_GH_LINUX)
        );
        assert_eq!(
            update_plan(Tool::Git, Kind::System, &mac),
            Choice::Page(PAGE_GIT_MAC)
        );
    }

    #[test]
    fn distributions_from_os_release() {
        let cases = [
            ("ID=ubuntu\nID_LIKE=debian\n", Some(Distro::Debian)),
            ("ID=debian\n", Some(Distro::Debian)),
            (
                "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n",
                Some(Distro::Debian),
            ),
            ("ID=fedora\n", Some(Distro::Fedora)),
            (
                "ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n",
                Some(Distro::Rhel),
            ),
            ("ID=arch\n", Some(Distro::Arch)),
            ("ID=manjaro\nID_LIKE=arch\n", Some(Distro::Arch)),
            (
                "ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n",
                Some(Distro::Opensuse),
            ),
            ("ID=alpine\n", Some(Distro::Alpine)),
            ("ID=nixos\n", None),
            ("", None),
        ];
        for (text, want) in cases {
            assert_eq!(Distro::from_os_release(text), want, "{text}");
        }
    }
}
