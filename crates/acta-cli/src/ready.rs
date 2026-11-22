//! The Ready for Claude Code rows of `acta setup --check` (ADR 0013,
//! section 5): `claude`, `signed in`, `git`, `gh`, before the plugin steps.
//! A missing or outdated row prints the exact command Acta would run on
//! this machine, or the download page. Only the required rows (`claude`,
//! `signed in`) can fail the check. Nothing here installs anything.

use crate::setup::{Prereq, Step};
use acta_core::paths::Os;
use acta_core::prereqs::commands::{install_plan, update_plan};
use acta_core::prereqs::{
    Choice, Detection, Exec, Kind, Level, Machine, Platform, SignIn, State, ToolRow, detect,
};
use std::path::Path;

/// Development override of the search: only the directories of this
/// `PATH`, no registry, no known folders. Honoured by debug builds only,
/// so a test never finds (or runs) the real tools.
pub const DEV_PREREQS_PATH: &str = "ACTA_DEV_PREREQS_PATH";

/// Where to look: this computer, or the development override.
pub fn machine() -> (Machine, bool) {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os(DEV_PREREQS_PATH).filter(|v| !v.is_empty()) {
        return (Machine::path_only(Os::CURRENT, &path), true);
    }
    (Machine::current(), false)
}

/// The four rows of this machine.
pub fn check(m: &Machine, is_file: &dyn Fn(&Path) -> bool, exec: &dyn Exec) -> Vec<Step> {
    let d = detect(m, is_file, exec);
    let platform = Platform::detect(m, is_file);
    steps(&d, &platform)
}

/// The rows from a detection.
pub fn steps(d: &Detection, p: &Platform) -> Vec<Step> {
    vec![
        tool_step("claude", &d.claude, p),
        sign_in_step(d),
        tool_step("git", &d.git, p),
        tool_step("gh", &d.gh, p),
    ]
}

fn kind_words(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Native => Some("native install (updates itself)"),
        Kind::Winget => Some("winget"),
        Kind::Homebrew => Some("Homebrew"),
        Kind::Npm => Some("npm"),
        Kind::System => Some("system package"),
        Kind::Unknown => None,
    }
}

/// The command as text and as argv, or the page.
fn what_to_run(choice: &Choice) -> (String, Option<Vec<String>>) {
    match choice {
        Choice::Run(plan) => (plan.display(), Some(plan.argv_strings())),
        Choice::Page(url) => (format!("download from {url}"), None),
    }
}

fn tool_step(name: &'static str, row: &ToolRow, p: &Platform) -> Step {
    let version = row.version.map(|v| v.to_string());
    let (detail, command) = match row.state {
        State::Ok => {
            let mut d = version.clone().unwrap_or_default();
            if let Some(words) =
                kind_words(row.kind).filter(|_| row.tool.level() == Level::Required)
            {
                d.push_str(&format!(", {words}"));
            }
            match row.git_bash {
                Some(true) => d.push_str(", with Git Bash"),
                Some(false) => d.push_str(", without Git Bash: Claude Code will use PowerShell"),
                None => {}
            }
            (d, None)
        }
        State::Missing => {
            let (run, argv) = what_to_run(&install_plan(row.tool, p));
            (format!("not installed: {run}"), argv)
        }
        State::Outdated => {
            let (run, argv) = what_to_run(&update_plan(row.tool, row.kind, p));
            let found = version.clone().unwrap_or_default();
            (
                format!("{found}, needs {}+: {run}", row.tool.minimum()),
                argv,
            )
        }
        State::Unknown => (
            format!("could not check: {}", row.reason.unwrap_or("unknown")),
            None,
        ),
    };
    let ok = row.tool.level() != Level::Required || row.state == State::Ok;
    Step {
        name,
        ok,
        detail,
        prereq: Some(Prereq {
            state: row.state_str(),
            version,
            kind: (row.state != State::Missing).then(|| row.kind.as_str()),
            command,
            found: Some(row.state != State::Missing),
        }),
    }
}

fn sign_in_step(d: &Detection) -> Step {
    let (state, detail): (&'static str, String) = match d.sign_in {
        SignIn::SignedIn(m) if m.is_api_key() => ("ok", "using an API key".into()),
        SignIn::SignedIn(m) => ("ok", m.as_str().into()),
        SignIn::SignedOut => (
            "missing",
            "not signed in: run claude and follow the browser prompts".into(),
        ),
        SignIn::NoClaude => ("missing", "install Claude Code first".into()),
        SignIn::Unknown(why) => ("unknown", format!("could not check: {why}")),
    };
    Step {
        name: "signed in",
        ok: matches!(d.sign_in, SignIn::SignedIn(_)),
        detail,
        prereq: Some(Prereq {
            state,
            version: None,
            kind: None,
            command: None,
            found: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::prereqs::commands::Distro;
    use acta_core::prereqs::{Tool, Version};
    use std::path::PathBuf;

    pub fn row(tool: Tool, state: State, kind: Kind, version: Option<Version>) -> ToolRow {
        ToolRow {
            tool,
            state,
            path: (state != State::Missing).then(|| PathBuf::from("x")),
            version,
            kind,
            git_bash: None,
            reason: (state == State::Unknown).then_some("no answer within 5 s"),
        }
    }

    #[test]
    fn optional_and_recommended_rows_never_fail() {
        let p = Platform {
            os: Os::Windows,
            distro: None,
            winget: true,
            brew: false,
            distro_manager: false,
        };
        let d = Detection {
            claude: row(
                Tool::Claude,
                State::Ok,
                Kind::Native,
                Some(Version::new(2, 1, 330)),
            ),
            sign_in: SignIn::SignedIn(acta_core::prereqs::AuthMethod::ClaudeAi),
            git: row(Tool::Git, State::Missing, Kind::Unknown, None),
            gh: row(Tool::Gh, State::Missing, Kind::Unknown, None),
        };
        let s = steps(&d, &p);
        assert!(s.iter().all(|x| x.ok), "{s:?}");
        assert_eq!(s[3].state(), "optional");
        assert_eq!(s[2].state(), "missing");
        assert!(
            s[3].detail
                .starts_with("not installed: winget install --id GitHub.cli -e")
        );
        assert_eq!(s[0].detail, "2.1.330, native install (updates itself)");

        let outdated = Detection {
            claude: row(
                Tool::Claude,
                State::Outdated,
                Kind::Native,
                Some(Version::new(2, 1, 100)),
            ),
            sign_in: SignIn::SignedOut,
            ..d
        };
        let s = steps(&outdated, &p);
        assert!(!s[0].ok && !s[1].ok);
        assert_eq!(s[0].detail, "2.1.100, needs 2.1.329+: claude update");
        assert_eq!(
            s[0].prereq.as_ref().unwrap().command.as_deref(),
            Some(&["claude".to_owned(), "update".to_owned()][..])
        );
    }

    fn platforms() -> Vec<Platform> {
        let mut out = Vec::new();
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            for distro in [
                None,
                Some(Distro::Debian),
                Some(Distro::Fedora),
                Some(Distro::Arch),
            ] {
                for flags in 0..8u8 {
                    out.push(Platform {
                        os,
                        distro: distro.filter(|_| os == Os::Linux),
                        winget: flags & 1 != 0,
                        brew: flags & 2 != 0,
                        distro_manager: flags & 4 != 0,
                    });
                }
            }
        }
        out
    }

    /// Every row in its longest state, on any platform, then the plugin
    /// steps with their longest details: the whole text under 1,200.
    #[test]
    fn the_check_fits_its_budget_in_the_longest_states() {
        let kinds = [
            Kind::Native,
            Kind::Winget,
            Kind::Homebrew,
            Kind::Npm,
            Kind::System,
            Kind::Unknown,
        ];
        let states = [State::Ok, State::Missing, State::Outdated, State::Unknown];
        let long = Some(Version::new(2, 105, 1000));
        let mut longest: Vec<Step> = Vec::new();
        for p in platforms() {
            for kind in kinds {
                for state in states {
                    for bash in [None, Some(false)] {
                        let mk = |tool| ToolRow {
                            git_bash: bash,
                            ..row(tool, state, kind, long)
                        };
                        let sign_ins = [
                            SignIn::SignedIn(acta_core::prereqs::AuthMethod::ApiKey),
                            SignIn::SignedOut,
                            SignIn::NoClaude,
                            SignIn::Unknown("claude could not be run"),
                        ];
                        for sign_in in sign_ins {
                            let d = Detection {
                                claude: mk(Tool::Claude),
                                sign_in,
                                git: mk(Tool::Git),
                                gh: mk(Tool::Gh),
                            };
                            let s = steps(&d, &p);
                            if longest.is_empty() {
                                longest = s;
                            } else {
                                for (best, new) in longest.iter_mut().zip(s) {
                                    if new.detail.len() > best.detail.len() {
                                        *best = new;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // The plugin steps at their longest: nothing set up, `claude` not
        // runnable, another status line in place.
        struct NoClaude;
        impl crate::setup::ClaudeCli for NoClaude {
            fn run(&self, _: &[&str]) -> Result<String, String> {
                Err("cannot run claude (NotFound): is Claude Code installed?".into())
            }
        }
        let tmp = tempfile::tempdir().unwrap();
        let claude_dir = tmp.path().join("home").join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"statusLine":{"type":"command","command":"~/.claude/alexandre-line.sh"}}"#,
        )
        .unwrap();
        let setup = crate::setup::Setup {
            plugin_root: tmp.path().join("data").join("claude"),
            claude_dir,
            exe: tmp.path().join("acta.exe"),
            claude: &NoClaude,
        };
        let mut all = longest.clone();
        all.extend(setup.check());
        all.push(Step::new(
            "link index",
            false,
            "cannot write the pre-check file (PermissionDenied): hooks may miss linked folders",
        ));
        let text = crate::setup::render_steps(&all, false);
        assert!(
            text.chars().count() < 1200,
            "{} chars:
{text}",
            text.chars().count()
        );
        for s in &longest {
            assert!(s.detail.len() < 200, "{}", s.detail);
        }
    }
}
