//! Detection (ADR 0013, section 2): read-only, never a network call, never
//! an install. Each found tool is run with `--version` (argv, no shell, no
//! window, 5 s, Claude Code session variables removed); the sign-in comes
//! from `claude auth status` (10 s), of which only the exit code and
//! `authMethod` are kept. Everything runs through [`Exec`], so the tests
//! use fake runners that record every argv.

use super::locate::{self, Bases};
use super::version::{Version, parse_version};
use super::{Kind, Level, Tool};
use crate::paths::Os;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// How long `--version` may take.
pub const VERSION_LIMIT: Duration = Duration::from_secs(5);
/// How long `claude auth status` may take.
pub const AUTH_LIMIT: Duration = Duration::from_secs(10);

/// What a run gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It ended by itself: its exit code (`None` when killed by a signal)
    /// and its stdout.
    Exited { code: Option<i32>, stdout: String },
    /// Still running at the limit: it was killed.
    TimedOut,
    /// It could not be started.
    NotRun,
}

/// Runs a program for detection.
pub trait Exec {
    fn run(&self, program: &Path, args: &[&str], limit: Duration) -> Outcome;
}

/// The real runner: no window, stdin closed, the whole tree killed at the
/// limit, Claude Code session variables removed, and the fresh `PATH`.
#[derive(Clone, Debug, Default)]
pub struct SystemExec {
    /// The `PATH` children get ([`locate::fresh_path`]); `None` keeps the
    /// process's own.
    pub path: Option<OsString>,
}

impl SystemExec {
    /// With this process's fresh `PATH`.
    pub fn current() -> SystemExec {
        SystemExec {
            path: locate::system_fresh_path(),
        }
    }
}

impl Exec for SystemExec {
    fn run(&self, program: &Path, args: &[&str], limit: Duration) -> Outcome {
        let Some(mut cmd) = command_for(program, args) else {
            return Outcome::NotRun;
        };
        if let Some(path) = &self.path {
            cmd.env("PATH", path);
        }
        for v in locate::session_vars_here() {
            cmd.env_remove(v);
        }
        match crate::process::output_within(&mut cmd, limit) {
            Ok(Some(out)) => Outcome::Exited {
                code: out.status.code(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            },
            Ok(None) => Outcome::TimedOut,
            Err(_) => Outcome::NotRun,
        }
    }
}

/// Characters `cmd.exe` interprets even inside double quotes, or that end
/// a quoted word.
pub fn has_cmd_metachar(text: &str) -> bool {
    text.contains(['"', '%', '!', '^', '&', '|', '<', '>'])
}

/// `program args`, a `.cmd` shim through `%ComSpec% /d /s /c` with every
/// word quoted; `None` for a shim whose words cmd could misread.
pub fn command_for(program: &Path, args: &[&str]) -> Option<Command> {
    let shim = cfg!(windows)
        && program
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if !shim {
        let mut cmd = Command::new(program);
        cmd.args(args);
        return Some(cmd);
    }
    let prog = program.to_string_lossy();
    if has_cmd_metachar(&prog) || args.iter().any(|a| has_cmd_metachar(a)) {
        return None;
    }
    let words: Vec<String> = std::iter::once(prog.as_ref())
        .chain(args.iter().copied())
        .map(|w| {
            let trailing = w.len() - w.trim_end_matches('\\').len();
            format!("\"{w}{}\"", "\\".repeat(trailing))
        })
        .collect();
    let comspec = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
    let mut cmd = Command::new(comspec);
    cmd.args(["/d", "/s", "/c"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.raw_arg(format!("\"{}\"", words.join(" ")));
    }
    #[cfg(not(windows))]
    let _ = words;
    Some(cmd)
}

/// Where to look, for one OS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub os: Os,
    /// The search order ([`locate::search_dirs`]).
    pub dirs: Vec<PathBuf>,
    pub bases: Bases,
    /// `CLAUDE_CODE_GIT_BASH_PATH`.
    pub git_bash_env: Option<PathBuf>,
}

impl Machine {
    /// This computer: process `PATH`, registry `PATH`, known folders.
    pub fn current() -> Machine {
        Machine {
            os: Os::CURRENT,
            dirs: locate::system_dirs(),
            bases: Bases::from_env(),
            git_bash_env: std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from),
        }
    }

    /// Only the directories of `path`: no registry, no known folders (a
    /// development override, so a test never finds the real tools).
    pub fn path_only(os: Os, path: &OsStr) -> Machine {
        Machine {
            os,
            dirs: locate::split_path(path),
            bases: Bases::default(),
            git_bash_env: None,
        }
    }
}

/// A row's state (ADR 0013, section 5; "optional" is how a missing
/// optional row is shown).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Ok,
    Missing,
    Outdated,
    /// Found, but it could not be run or did not say its version.
    Unknown,
}

/// One tool, as found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolRow {
    pub tool: Tool,
    pub state: State,
    pub path: Option<PathBuf>,
    pub version: Option<Version>,
    pub kind: Kind,
    /// Git on Windows: whether Git Bash is there too.
    pub git_bash: Option<bool>,
    /// Why the state is `Unknown`, one line.
    pub reason: Option<&'static str>,
}

impl ToolRow {
    fn missing(tool: Tool, kind: Kind) -> ToolRow {
        ToolRow {
            tool,
            state: State::Missing,
            path: None,
            version: None,
            kind,
            git_bash: None,
            reason: None,
        }
    }

    /// The protocol state: `ok`, `missing`, `outdated`, `optional` (a
    /// missing optional row) or `unknown`.
    pub fn state_str(&self) -> &'static str {
        match self.state {
            State::Ok => "ok",
            State::Missing if self.tool.level() == Level::Optional => "optional",
            State::Missing => "missing",
            State::Outdated => "outdated",
            State::Unknown => "unknown",
        }
    }
}

/// How Claude Code is signed in. Only the method survives detection: the
/// rest of `claude auth status` (the email, the organisation) is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMethod {
    ClaudeAi,
    OauthToken,
    ApiKey,
    ApiKeyHelper,
    ThirdParty,
    None,
    Other,
}

impl AuthMethod {
    fn parse(json: &str) -> AuthMethod {
        let value: Option<serde_json::Value> = serde_json::from_str(json.trim()).ok();
        match value
            .as_ref()
            .and_then(|v| v.get("authMethod"))
            .and_then(|m| m.as_str())
        {
            Some("claude.ai") => AuthMethod::ClaudeAi,
            Some("oauth_token") => AuthMethod::OauthToken,
            Some("api_key") => AuthMethod::ApiKey,
            Some("api_key_helper") => AuthMethod::ApiKeyHelper,
            Some("third_party") => AuthMethod::ThirdParty,
            Some("none") => AuthMethod::None,
            _ => AuthMethod::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AuthMethod::ClaudeAi => "claude.ai",
            AuthMethod::OauthToken => "oauth_token",
            AuthMethod::ApiKey => "api_key",
            AuthMethod::ApiKeyHelper => "api_key_helper",
            AuthMethod::ThirdParty => "third_party",
            AuthMethod::None => "none",
            AuthMethod::Other => "other",
        }
    }

    /// "Using an API key" rather than "Signed in" (not an error).
    pub fn is_api_key(self) -> bool {
        matches!(self, AuthMethod::ApiKey | AuthMethod::ApiKeyHelper)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignIn {
    SignedIn(AuthMethod),
    SignedOut,
    /// No usable Claude Code to ask.
    NoClaude,
    /// It could not be asked; why, one line.
    Unknown(&'static str),
}

/// The four rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detection {
    pub claude: ToolRow,
    pub sign_in: SignIn,
    pub git: ToolRow,
    pub gh: ToolRow,
}

impl Detection {
    /// One line for a diagnostic log: states, versions and kinds only.
    pub fn summary(&self) -> String {
        let row = |r: &ToolRow| {
            let mut s = format!("{} {}", r.tool.exe_name(), r.state_str());
            if let Some(v) = r.version {
                s.push_str(&format!(" {v} {}", r.kind.as_str()));
            }
            s
        };
        let sign_in = match self.sign_in {
            SignIn::SignedIn(m) => m.as_str(),
            SignIn::SignedOut => "signed out",
            SignIn::NoClaude => "no claude",
            SignIn::Unknown(_) => "unknown",
        };
        format!(
            "prereqs: {}; sign-in {sign_in}; {}; {}",
            row(&self.claude),
            row(&self.git),
            row(&self.gh)
        )
    }

    /// Whether the required rows are ready.
    pub fn ready(&self) -> bool {
        self.claude.state == State::Ok && matches!(self.sign_in, SignIn::SignedIn(_))
    }
}

/// All four rows.
pub fn detect(m: &Machine, is_file: &dyn Fn(&Path) -> bool, exec: &dyn Exec) -> Detection {
    let claude = detect_tool(m, Tool::Claude, is_file, exec);
    let sign_in = detect_sign_in(&claude, exec);
    Detection {
        claude,
        sign_in,
        git: detect_tool(m, Tool::Git, is_file, exec),
        gh: detect_tool(m, Tool::Gh, is_file, exec),
    }
}

/// `xcode-select`, Apple's: run before any `/usr/bin/git`.
pub const XCODE_SELECT: &str = "/usr/bin/xcode-select";

/// One tool: located, run with `--version`, its kind read from its path.
pub fn detect_tool(
    m: &Machine,
    tool: Tool,
    is_file: &dyn Fn(&Path) -> bool,
    exec: &dyn Exec,
) -> ToolRow {
    let Some(mut path) = locate::locate(m.os, tool.exe_name(), &m.dirs, is_file) else {
        return ToolRow::missing(tool, Kind::Unknown);
    };
    if tool == Tool::Git && m.os == Os::MacOs && path == Path::new("/usr/bin").join("git") {
        // Apple's /usr/bin/git is a stub that opens an install dialog when
        // the Command Line Tools are missing: never run it before
        // `xcode-select -p` says they are there, or a Homebrew git is.
        let brew = ["/opt/homebrew/bin", "/usr/local/bin"]
            .iter()
            .map(|d| Path::new(d).join("git"))
            .find(|p| is_file(p));
        match brew {
            Some(b) => path = b,
            None => {
                let ok = matches!(
                    exec.run(Path::new(XCODE_SELECT), &["-p"], VERSION_LIMIT),
                    Outcome::Exited { code: Some(0), .. }
                );
                if !ok {
                    return ToolRow::missing(tool, Kind::System);
                }
            }
        }
    }
    let canonical = std::fs::canonicalize(&path).ok();
    let kind = kind_of(m.os, tool, &path, canonical.as_deref());
    let git_bash = (tool == Tool::Git && m.os == Os::Windows).then(|| {
        locate::git_bash_in(m.git_bash_env.as_deref(), &m.dirs, &m.bases, is_file).is_some()
    });
    let (state, version, reason) = match exec.run(&path, &["--version"], VERSION_LIMIT) {
        Outcome::Exited {
            code: Some(0),
            stdout,
        } => match parse_version(&stdout) {
            Some(v) if v >= tool.minimum() => (State::Ok, Some(v), None),
            Some(v) => (State::Outdated, Some(v), None),
            None => (State::Unknown, None, Some("it did not print a version")),
        },
        Outcome::Exited { .. } => (State::Unknown, None, Some("--version failed")),
        Outcome::TimedOut => (State::Unknown, None, Some("no answer within 5 s")),
        Outcome::NotRun => (State::Unknown, None, Some("it could not be run")),
    };
    ToolRow {
        tool,
        state,
        path: Some(path),
        version,
        kind,
        git_bash,
        reason,
    }
}

/// `claude auth status`: exit 0 is signed in, 1 is not. Only `authMethod`
/// is read; nothing of the output is kept or logged.
pub fn detect_sign_in(claude: &ToolRow, exec: &dyn Exec) -> SignIn {
    let Some(path) = claude
        .path
        .as_deref()
        .filter(|_| matches!(claude.state, State::Ok | State::Outdated))
    else {
        return SignIn::NoClaude;
    };
    match exec.run(path, &["auth", "status"], AUTH_LIMIT) {
        Outcome::Exited {
            code: Some(0),
            stdout,
        } => SignIn::SignedIn(AuthMethod::parse(&stdout)),
        Outcome::Exited { code: Some(1), .. } => SignIn::SignedOut,
        Outcome::Exited { .. } => SignIn::Unknown("claude auth status failed"),
        Outcome::TimedOut => SignIn::Unknown("no answer within 10 s"),
        Outcome::NotRun => SignIn::Unknown("claude could not be run"),
    }
}

/// How the install was made, from the resolved path and, when it is a
/// link, its target (ADR 0013, section 2). Unknown is a valid answer.
pub fn kind_of(os: Os, tool: Tool, path: &Path, target: Option<&Path>) -> Kind {
    let read = |p: &Path| {
        let mut s = p.to_string_lossy().replace('\\', "/");
        if os == Os::Windows {
            s = s.to_lowercase();
        }
        s
    };
    let of = |s: &str| {
        let has = |needle: &str| s.contains(needle);
        if tool == Tool::Claude && (has("/.local/bin/") || has("/.local/share/claude/")) {
            Kind::Native
        } else if has("/winget/links/") || has("/winget/packages/") {
            Kind::Winget
        } else if has("/opt/homebrew/")
            || has("/Caskroom/")
            || has("/Cellar/")
            || has("/home/linuxbrew/")
        {
            Kind::Homebrew
        } else if has("/node_modules/") || has("/appdata/roaming/npm/") || has("/.npm-global/") {
            Kind::Npm
        } else if os != Os::Windows && s.starts_with("/usr/bin/") {
            Kind::System
        } else {
            Kind::Unknown
        }
    };
    // Under Program Files it is a machine-wide install (winget's machine
    // scope, an MSI, Git's own installer): a per-user upgrade would not
    // replace it, so its kind stays unknown and the update is the page.
    let machine_wide = |p: &Path| {
        let s = read(p);
        os == Os::Windows
            && (s.contains(":/program files/") || s.contains(":/program files (x86)/"))
    };
    if machine_wide(path) || target.is_some_and(machine_wide) {
        return Kind::Unknown;
    }
    let from_target = target.map(|t| of(&read(t))).unwrap_or(Kind::Unknown);
    if from_target != Kind::Unknown {
        from_target
    } else {
        of(&read(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::time::Instant;

    /// Answers by program file name; records every argv.
    #[derive(Default)]
    struct FakeExec {
        answers: HashMap<String, Outcome>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeExec {
        fn with(mut self, call: &str, out: Outcome) -> Self {
            self.answers.insert(call.to_owned(), out);
            self
        }
    }

    impl Exec for FakeExec {
        fn run(&self, program: &Path, args: &[&str], _: Duration) -> Outcome {
            let name = program.file_name().unwrap().to_string_lossy().into_owned();
            let call = format!("{name} {}", args.join(" "));
            self.calls
                .borrow_mut()
                .push(format!("{} {}", program.display(), args.join(" ")));
            self.answers.get(&call).cloned().unwrap_or(Outcome::NotRun)
        }
    }

    fn says(text: &str) -> Outcome {
        Outcome::Exited {
            code: Some(0),
            stdout: text.into(),
        }
    }

    fn mac(dirs: &[&str]) -> Machine {
        Machine {
            os: Os::MacOs,
            dirs: dirs.iter().map(PathBuf::from).collect(),
            bases: Bases::default(),
            git_bash_env: None,
        }
    }

    #[test]
    fn apples_git_stub_is_never_run_without_xcode_select() {
        let usr_git = Path::new("/usr/bin").join("git");
        let only_stub = |p: &Path| p == usr_git;
        let m = mac(&["/usr/bin"]);
        let exec = FakeExec::default()
            .with(
                "xcode-select -p",
                Outcome::Exited {
                    code: Some(2),
                    stdout: String::new(),
                },
            )
            .with("git --version", says("git version 2.39.5 (Apple Git-154)"));
        let row = detect_tool(&m, Tool::Git, &only_stub, &exec);
        assert_eq!(row.state, State::Missing);
        assert_eq!(*exec.calls.borrow(), [format!("{XCODE_SELECT} -p")]);

        let exec = FakeExec::default()
            .with(
                "xcode-select -p",
                says("/Library/Developer/CommandLineTools"),
            )
            .with("git --version", says("git version 2.39.5 (Apple Git-154)"));
        let row = detect_tool(&m, Tool::Git, &only_stub, &exec);
        assert_eq!(row.state, State::Ok);
        assert_eq!(row.kind, Kind::System);
        assert_eq!(exec.calls.borrow().len(), 2);
        assert!(exec.calls.borrow()[0].starts_with(XCODE_SELECT));

        // A Homebrew git is used without asking xcode-select.
        let brew_git = Path::new("/opt/homebrew/bin").join("git");
        let both = |p: &Path| p == usr_git || p == brew_git;
        let exec = FakeExec::default().with("git --version", says("git version 2.56.1"));
        let row = detect_tool(&m, Tool::Git, &both, &exec);
        assert_eq!(row.state, State::Ok);
        assert_eq!(row.kind, Kind::Homebrew);
        assert!(
            exec.calls
                .borrow()
                .iter()
                .all(|c| !c.contains("xcode-select"))
        );
    }

    #[test]
    fn rows_are_found_outdated_unknown_or_missing() {
        let m = Machine {
            os: Os::Windows,
            dirs: vec![PathBuf::from("bin")],
            bases: Bases::default(),
            git_bash_env: None,
        };
        let files = ["claude.exe", "git.exe", "gh.exe"].map(|f| Path::new("bin").join(f));
        let is = |p: &Path| files.iter().any(|f| f == p);
        let exec = FakeExec::default()
            .with("claude.exe --version", says("2.1.100 (Claude Code)"))
            .with("git.exe --version", says("Alexandre's garbage"))
            .with("gh.exe --version", Outcome::TimedOut);
        let d = detect(&m, &is, &exec);
        assert_eq!(d.claude.state, State::Outdated);
        assert_eq!(d.claude.version, Some(Version::new(2, 1, 100)));
        assert_eq!(d.git.state, State::Unknown);
        assert_eq!(d.git.git_bash, Some(false));
        assert_eq!(d.gh.state, State::Unknown);
        assert_eq!(d.sign_in, SignIn::Unknown("claude could not be run"));

        let none = detect(&m, &|_: &Path| false, &FakeExec::default());
        assert_eq!(none.claude.state, State::Missing);
        assert_eq!(none.sign_in, SignIn::NoClaude);
        assert_eq!(none.gh.state_str(), "optional");
        assert_eq!(none.git.state_str(), "missing");
    }

    #[test]
    fn only_the_auth_method_survives_auth_status() {
        let m = Machine::path_only(Os::Linux, OsStr::new(""));
        let m = Machine {
            dirs: vec![PathBuf::from("/bin")],
            ..m
        };
        let claude = Path::new("/bin").join("claude");
        let is = |p: &Path| p == claude;
        let json = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"alexandre@example.com","orgName":"Alexandre's org","subscriptionType":"max"}"#;
        let exec = FakeExec::default()
            .with("claude --version", says("2.1.330 (Claude Code)"))
            .with("claude auth status", says(json));
        let d = detect(&m, &is, &exec);
        assert_eq!(d.sign_in, SignIn::SignedIn(AuthMethod::ClaudeAi));
        let debug = format!("{d:?}");
        let log = d.summary();
        for text in [&debug, &log] {
            assert!(!text.contains("alexandre@"), "{text}");
            assert!(!text.contains("Alexandre's org"), "{text}");
            assert!(!text.contains("max"), "{text}");
        }
        assert!(log.contains("claude ok 2.1.330"), "{log}");

        let key = says(r#"{"loggedIn":true,"authMethod":"api_key"}"#);
        let exec = FakeExec::default()
            .with("claude --version", says("2.1.330 (Claude Code)"))
            .with("claude auth status", key);
        let d = detect(&m, &is, &exec);
        assert!(matches!(d.sign_in, SignIn::SignedIn(a) if a.is_api_key()));

        let out = Outcome::Exited {
            code: Some(1),
            stdout: r#"{"loggedIn":false,"authMethod":"none"}"#.into(),
        };
        let exec = FakeExec::default()
            .with("claude --version", says("2.1.330 (Claude Code)"))
            .with("claude auth status", out);
        assert_eq!(detect(&m, &is, &exec).sign_in, SignIn::SignedOut);
    }

    #[test]
    fn install_kind_from_each_path_shape() {
        let p = PathBuf::from;
        let cases = [
            (
                Os::Windows,
                Tool::Claude,
                r"C:\Users\Alexandre\.local\bin\claude.exe",
                None,
                Kind::Native,
            ),
            (
                Os::Linux,
                Tool::Claude,
                "/home/alexandre/.local/bin/claude",
                Some("/home/alexandre/.local/share/claude/versions/2.1.330"),
                Kind::Native,
            ),
            (
                Os::Windows,
                Tool::Claude,
                r"C:\Users\Alexandre\AppData\Local\Microsoft\WinGet\Links\claude.exe",
                None,
                Kind::Winget,
            ),
            (
                Os::Windows,
                Tool::Gh,
                r"C:\Users\Alexandre\AppData\Local\Microsoft\WinGet\Packages\GitHub.cli_x\bin\gh.exe",
                None,
                Kind::Winget,
            ),
            (
                Os::MacOs,
                Tool::Claude,
                "/opt/homebrew/bin/claude",
                None,
                Kind::Homebrew,
            ),
            (
                Os::MacOs,
                Tool::Gh,
                "/usr/local/bin/gh",
                Some("/usr/local/Cellar/gh/2.105.0/bin/gh"),
                Kind::Homebrew,
            ),
            (
                Os::MacOs,
                Tool::Claude,
                "/usr/local/bin/claude",
                Some("/usr/local/Caskroom/claude-code/2.1.330/claude"),
                Kind::Homebrew,
            ),
            (
                Os::Windows,
                Tool::Claude,
                r"C:\Users\Alexandre\AppData\Roaming\npm\claude.cmd",
                None,
                Kind::Npm,
            ),
            (
                Os::Linux,
                Tool::Claude,
                "/home/alexandre/.npm-global/bin/claude",
                Some(
                    "/home/alexandre/.npm-global/lib/node_modules/@anthropic-ai/claude-code/bin/claude",
                ),
                Kind::Npm,
            ),
            (Os::Linux, Tool::Git, "/usr/bin/git", None, Kind::System),
            (Os::MacOs, Tool::Git, "/usr/bin/git", None, Kind::System),
            (
                Os::Windows,
                Tool::Git,
                r"C:\Program Files\Git\cmd\git.exe",
                None,
                Kind::Unknown,
            ),
            (
                Os::Linux,
                Tool::Git,
                "/home/alexandre/.local/bin/git",
                None,
                Kind::Unknown,
            ),
            (Os::Linux, Tool::Gh, "/opt/tools/gh", None, Kind::Unknown),
            // winget's machine scope: under Program Files, never "winget".
            (
                Os::Windows,
                Tool::Gh,
                r"C:\Program Files\WinGet\Links\gh.exe",
                Some(r"C:\Program Files\WinGet\Packages\GitHub.cli_x\bin\gh.exe"),
                Kind::Unknown,
            ),
            (
                Os::Windows,
                Tool::Claude,
                r"C:\Users\Alexandre\AppData\Local\Microsoft\WinGet\Links\claude.exe",
                Some(r"C:\Program Files (x86)\Anthropic\claude.exe"),
                Kind::Unknown,
            ),
        ];
        for (os, tool, path, target, want) in cases {
            let target = target.map(p);
            assert_eq!(
                kind_of(os, tool, &p(path), target.as_deref()),
                want,
                "{path}"
            );
        }
    }

    /// The real runner on a stub that never answers: killed at the limit.
    #[test]
    fn a_hanging_claude_is_killed_at_five_seconds() {
        let tmp = tempfile::tempdir().unwrap();
        let (name, body) = if cfg!(windows) {
            ("claude.cmd", "@echo off\r\nping -n 30 127.0.0.1 >NUL\r\n")
        } else {
            ("claude", "#!/bin/sh\nsleep 30\n")
        };
        let stub = tmp.path().join(name);
        std::fs::write(&stub, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let m = Machine::path_only(Os::CURRENT, tmp.path().as_os_str());
        let started = Instant::now();
        let row = detect_tool(
            &m,
            Tool::Claude,
            &|p: &Path| p.is_file(),
            &SystemExec {
                path: Some(crate::test_child_path()),
            },
        );
        let took = started.elapsed();
        assert_eq!(row.state, State::Unknown);
        assert_eq!(row.reason, Some("no answer within 5 s"));
        assert!(
            took >= VERSION_LIMIT && took < Duration::from_secs(15),
            "{took:?}"
        );
    }

    #[test]
    fn a_shim_with_cmd_metacharacters_is_not_run() {
        if cfg!(windows) {
            assert!(
                command_for(Path::new(r"C:\Alexandre & co\claude.cmd"), &["--version"]).is_none()
            );
            assert!(command_for(Path::new(r"C:\Alexandre\claude.cmd"), &["--version"]).is_some());
        }
        assert!(command_for(Path::new("claude"), &["--version"]).is_some());
    }
}
