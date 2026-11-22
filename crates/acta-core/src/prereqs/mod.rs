//! Ready for Claude Code (ADR 0013): what Claude Code needs on this
//! computer, found, versioned and, on the user's click only, installed.
//!
//! - [`locate`]: where a tool is (the process `PATH`, on Windows the `PATH`
//!   now in the registry, then known folders), `claude` for the terminal,
//!   Git Bash for the status line, and the fresh `PATH` for children.
//! - [`detect`]: versions, install kinds, the macOS Git rule and the
//!   sign-in, through an injectable runner.
//! - [`commands`]: the install and update commands, compile-time constants
//!   built as argv, or the official download page.
//!
//! Nothing here runs at import, touches the network or writes a file.

pub mod commands;
pub mod detect;
pub mod locate;
pub mod version;

pub use commands::{Choice, Distro, Plan, Platform, RunsIn, Scope, Source};
pub use detect::{
    AuthMethod, Detection, Exec, Machine, Outcome, SignIn, State, SystemExec, ToolRow, detect,
};
pub use locate::{
    ClaudeProgram, claude_session_vars, git_bash, locate, resolve_claude, resolve_unix,
    resolve_windows,
};
pub use version::{CLAUDE_MIN, GH_MIN, GIT_MIN, Version, parse_version};

/// The three programs the checklist looks for (the fourth row, the
/// sign-in, is Claude Code's own).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tool {
    Claude,
    Git,
    Gh,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Claude, Tool::Git, Tool::Gh];

    /// The executable's name, without extension.
    pub fn exe_name(self) -> &'static str {
        match self {
            Tool::Claude => "claude",
            Tool::Git => "git",
            Tool::Gh => "gh",
        }
    }

    /// The name shown to the user; never translated.
    pub fn display_name(self) -> &'static str {
        match self {
            Tool::Claude => "Claude Code",
            Tool::Git => "Git",
            Tool::Gh => "GitHub CLI",
        }
    }

    pub fn level(self) -> Level {
        match self {
            Tool::Claude => Level::Required,
            Tool::Git => Level::Recommended,
            Tool::Gh => Level::Optional,
        }
    }

    /// The oldest version Acta accepts.
    pub fn minimum(self) -> Version {
        match self {
            Tool::Claude => CLAUDE_MIN,
            Tool::Git => GIT_MIN,
            Tool::Gh => GH_MIN,
        }
    }
}

/// How much a missing row matters (ADR 0013, section 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Required,
    Recommended,
    Optional,
}

/// How a tool was installed, read from its path: it decides the update
/// command. `Unknown` is a valid answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Native,
    Winget,
    Homebrew,
    Npm,
    System,
    Unknown,
}

impl Kind {
    /// The protocol name (`acta setup --check --json`).
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Native => "native",
            Kind::Winget => "winget",
            Kind::Homebrew => "homebrew",
            Kind::Npm => "npm",
            Kind::System => "system",
            Kind::Unknown => "unknown",
        }
    }
}
