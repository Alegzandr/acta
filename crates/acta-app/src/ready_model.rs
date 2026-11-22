//! Ready for Claude Code as the app shows it (ADR 0013; components-app.md
//! 5, "Ready for Claude Code"; screens.md 8.26), without GPUI: which rows
//! show, each row's glyph, state, line and buttons from the detection and
//! what the app is doing for it, and the parts of the install
//! confirmation. `ready.rs` draws these; the tests here pin the rules.

use std::time::Duration;

use acta_core::paths::Os;
use acta_core::prereqs::commands::{install_plan, update_plan, winget_alternative};
use acta_core::prereqs::{
    Choice, Failure, Level, Plan, Platform, RunsIn, Scope, SignIn, Source, State, Tool, ToolRow,
};

/// The four rows, in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    Claude,
    SignIn,
    Git,
    Gh,
}

impl Row {
    /// The program of a row (the sign-in is Claude Code's).
    pub fn tool(self) -> Tool {
        match self {
            Row::Claude | Row::SignIn => Tool::Claude,
            Row::Git => Tool::Git,
            Row::Gh => Tool::Gh,
        }
    }

    pub fn of(tool: Tool) -> Row {
        match tool {
            Tool::Claude => Row::Claude,
            Tool::Git => Row::Git,
            Tool::Gh => Row::Gh,
        }
    }

    fn required(self) -> bool {
        matches!(self, Row::Claude | Row::SignIn)
    }
}

/// What the app is doing for a row, beside what detection found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Idle,
    /// The install (or update) runs as a child, for this long.
    Running {
        update: bool,
        elapsed: Duration,
    },
    Failed(Failure),
    /// A one-off session of the terminal runs it (Linux root installs,
    /// the sign-in): the row re-checks when it ends and on focus.
    Terminal,
    /// `xcode-select --install` opened Apple's window: re-checks on focus.
    MacOs,
}

/// What detection knows, and what the app does. `None` rows are not
/// checked yet: they show nothing rather than a guess.
#[derive(Clone, Debug)]
pub struct Facts<'a> {
    pub claude: Option<&'a ToolRow>,
    pub sign_in: Option<SignIn>,
    pub git: Option<&'a ToolRow>,
    pub gh: Option<&'a ToolRow>,
    pub platform: Platform,
    /// An attached folder has a `github.com` remote.
    pub github: bool,
    /// The install running now, if any.
    pub busy: Option<Tool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Check,
    Dashed,
    Alert,
    Terminal,
}

/// Colours by role; `ready.rs` maps them to tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Secondary,
    Tertiary,
    Success,
    Warning,
    Danger,
}

/// The state at the right of a row's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateText {
    Version(String),
    WithBash(String),
    WithoutBash(String),
    Missing,
    MissingOptional,
    Outdated { version: String, minimum: String },
    Installing(String),
    Updating(String),
    Terminal,
    MacOs,
    Failed,
    SignedIn,
    ApiKey,
    SignedOut,
    Unknown,
}

/// The row's one line under its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Claude,
    SignIn,
    Git,
    GitWindows,
    Gh,
}

/// The line under a failed row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Exit(Option<i32>),
    NotFound(Tool),
    NoStart,
}

/// What a button does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// The confirmation dialog of `plan`; `update` for an outdated tool.
    Confirm {
        tool: Tool,
        plan: Plan,
        update: bool,
    },
    /// The official page in the system browser.
    Download(&'static str),
    SignIn,
    ShowLog(Tool),
    ShowTerminal(Row),
    CheckAgain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Install,
    Update,
    ShowCommand,
    Download,
    SignIn,
    ShowLog,
    Retry,
    ShowTerminal,
    CheckAgain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
}

/// A tooltip, by role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip {
    /// Disabled: "Install Claude Code first".
    NeedsClaude,
    /// Disabled: "One install at a time".
    OneAtATime,
    /// "Acta works best with a Claude subscription."
    ApiKey,
    /// No package manager Acta can use: install from the page.
    NoManager(Tool),
    /// What happens on "Sign in...".
    SignInHint,
    /// Why a row could not be checked, one line (protocol text).
    Unknown(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    pub label: Label,
    pub act: Act,
    pub variant: Variant,
    /// Disabled, with why.
    pub disabled: Option<Tip>,
    pub tip: Option<Tip>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowView {
    pub row: Row,
    pub glyph: Glyph,
    pub tone: Tone,
    pub state: Option<StateText>,
    pub state_tone: Tone,
    /// The tooltip of the state text.
    pub state_tip: Option<Tip>,
    pub why: Why,
    pub buttons: Vec<Button>,
    pub reason: Option<Reason>,
    /// "Install with winget instead", under the reason.
    pub winget_instead: Option<Plan>,
    /// The tool whose install log this row shows, when it has one.
    pub log: Option<Tool>,
}

/// `m:ss`; nothing before 400 ms (components-app.md 5).
pub fn elapsed(d: Duration) -> Option<String> {
    if d < Duration::from_millis(400) {
        return None;
    }
    let s = d.as_secs();
    Some(format!("{}:{:02}", s / 60, s % 60))
}

/// Whether a row is ready: found, signed in, or (an optional row) not
/// wanted. `None` while it is not checked yet.
fn tool_ready(row: Option<&ToolRow>) -> Option<bool> {
    row.map(|r| r.state == State::Ok)
}

fn signed_in(s: Option<SignIn>) -> Option<bool> {
    s.map(|s| matches!(s, SignIn::SignedIn(_)))
}

/// The rows the block shows: GitHub CLI only when an attached folder has
/// a `github.com` remote or `gh` is found, and never on first run.
pub fn rows_shown(f: &Facts, first_run: bool) -> Vec<Row> {
    let gh_found = f.gh.is_some_and(|r| r.state != State::Missing);
    let mut rows = vec![Row::Claude, Row::SignIn, Row::Git];
    if !first_run && (f.github || gh_found) {
        rows.push(Row::Gh);
    }
    rows
}

/// First run shows the block only while a required or recommended row is
/// not ready, once it is known.
pub fn first_run_shows(f: &Facts) -> bool {
    [
        tool_ready(f.claude),
        signed_in(f.sign_in),
        tool_ready(f.git),
    ]
    .contains(&Some(false))
}

/// The status bar's view of the required rows (components-app.md 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// Not checked yet, or could not be: the bar claims nothing.
    Unknown,
    Missing,
    SignedOut,
    /// Found (outdated counts: it runs) and signed in.
    Ready,
}

pub fn readiness(claude: Option<&ToolRow>, sign_in: Option<SignIn>) -> Readiness {
    match claude.map(|r| r.state) {
        None | Some(State::Unknown) => Readiness::Unknown,
        Some(State::Missing) => Readiness::Missing,
        Some(State::Ok | State::Outdated) => match sign_in {
            Some(SignIn::SignedOut) => Readiness::SignedOut,
            Some(SignIn::SignedIn(_)) => Readiness::Ready,
            _ => Readiness::Unknown,
        },
    }
}

/// Every row of the block, in order, with the primary given to the first
/// required row that is not ready.
pub fn rows(f: &Facts, first_run: bool, activity: impl Fn(Row) -> Activity) -> Vec<RowView> {
    let mut out: Vec<RowView> = rows_shown(f, first_run)
        .into_iter()
        .map(|row| row_view(f, row, activity(row)))
        .collect();
    if let Some(first) = out
        .iter_mut()
        .find(|r| r.row.required() && !matches!(r.glyph, Glyph::Check) && r.state.is_some())
        && let Some(main) = first.buttons.first_mut()
        && !matches!(main.label, Label::ShowLog)
    {
        main.variant = Variant::Primary;
    }
    out
}

/// Whether the block has a primary button ("Set up Claude Code" is then
/// secondary).
pub fn has_primary(rows: &[RowView]) -> bool {
    rows.iter()
        .flat_map(|r| &r.buttons)
        .any(|b| b.variant == Variant::Primary)
}

fn level_variant(row: Row) -> Variant {
    if row.required() {
        Variant::Secondary
    } else {
        Variant::Ghost
    }
}

fn why(row: Row, os: Os) -> Why {
    match row {
        Row::Claude => Why::Claude,
        Row::SignIn => Why::SignIn,
        Row::Git if os == Os::Windows => Why::GitWindows,
        Row::Git => Why::Git,
        Row::Gh => Why::Gh,
    }
}

fn pending(row: Row, os: Os) -> RowView {
    RowView {
        row,
        glyph: Glyph::Dashed,
        tone: Tone::Tertiary,
        state: None,
        state_tone: Tone::Tertiary,
        state_tip: None,
        why: why(row, os),
        buttons: Vec::new(),
        reason: None,
        winget_instead: None,
        log: None,
    }
}

fn button(label: Label, act: Act, variant: Variant) -> Button {
    Button {
        label,
        act,
        variant,
        disabled: None,
        tip: None,
    }
}

/// One row.
pub fn row_view(f: &Facts, row: Row, activity: Activity) -> RowView {
    let os = f.platform.os;
    match row {
        Row::SignIn => sign_in_view(f, activity),
        Row::Claude | Row::Git | Row::Gh => {
            let found = match row {
                Row::Claude => f.claude,
                Row::Git => f.git,
                _ => f.gh,
            };
            match found {
                Some(r) => tool_view(f, row, r, activity),
                None => pending(row, os),
            }
        }
    }
}

fn sign_in_view(f: &Facts, activity: Activity) -> RowView {
    let row = Row::SignIn;
    let base = pending(row, f.platform.os);
    let Some(sign_in) = f.sign_in else {
        return base;
    };
    if activity == Activity::Terminal && !matches!(sign_in, SignIn::SignedIn(_)) {
        return waiting_terminal(base);
    }
    match sign_in {
        SignIn::SignedIn(method) => RowView {
            glyph: Glyph::Check,
            tone: Tone::Success,
            state: Some(if method.is_api_key() {
                StateText::ApiKey
            } else {
                StateText::SignedIn
            }),
            state_tone: Tone::Secondary,
            state_tip: method.is_api_key().then_some(Tip::ApiKey),
            ..base
        },
        SignIn::SignedOut | SignIn::NoClaude => {
            let mut b = button(Label::SignIn, Act::SignIn, level_variant(row));
            b.tip = Some(Tip::SignInHint);
            if sign_in == SignIn::NoClaude {
                b.disabled = Some(Tip::NeedsClaude);
            } else if f.busy.is_some() {
                b.disabled = Some(Tip::OneAtATime);
            }
            RowView {
                state: Some(StateText::SignedOut),
                buttons: vec![b],
                ..base
            }
        }
        SignIn::Unknown(why) => unknown(base, why),
    }
}

fn waiting_terminal(base: RowView) -> RowView {
    let row = base.row;
    RowView {
        glyph: Glyph::Terminal,
        tone: Tone::Secondary,
        state: Some(StateText::Terminal),
        state_tone: Tone::Secondary,
        buttons: vec![button(
            Label::ShowTerminal,
            Act::ShowTerminal(row),
            level_variant(row),
        )],
        ..base
    }
}

fn unknown(base: RowView, why: &'static str) -> RowView {
    let row = base.row;
    RowView {
        state: Some(StateText::Unknown),
        state_tip: Some(Tip::Unknown(why)),
        buttons: vec![button(
            Label::CheckAgain,
            Act::CheckAgain,
            level_variant(row),
        )],
        ..base
    }
}

fn tool_view(f: &Facts, row: Row, r: &ToolRow, activity: Activity) -> RowView {
    let tool = row.tool();
    let base = pending(row, f.platform.os);
    let version = r.version.map(|v| v.to_string()).unwrap_or_default();
    if r.state == State::Ok && !matches!(activity, Activity::Running { .. }) {
        let state = match r.git_bash {
            Some(true) => StateText::WithBash(version),
            Some(false) => StateText::WithoutBash(version),
            None => StateText::Version(version),
        };
        return RowView {
            glyph: Glyph::Check,
            tone: Tone::Success,
            state: Some(state),
            state_tone: Tone::Secondary,
            ..base
        };
    }
    if let State::Unknown = r.state {
        return unknown(base, r.reason.unwrap_or("unknown"));
    }
    let update = r.state == State::Outdated;
    let choice = if update {
        update_plan(tool, r.kind, &f.platform)
    } else {
        install_plan(tool, &f.platform)
    };
    let log = Some(tool);
    match activity {
        Activity::Running { update, elapsed: e } => {
            let e = elapsed(e);
            return RowView {
                glyph: Glyph::Dashed,
                tone: Tone::Secondary,
                state: e.map(|e| {
                    if update {
                        StateText::Updating(e)
                    } else {
                        StateText::Installing(e)
                    }
                }),
                state_tone: Tone::Secondary,
                buttons: vec![button(Label::ShowLog, Act::ShowLog(tool), Variant::Ghost)],
                log,
                ..base
            };
        }
        Activity::Terminal => return waiting_terminal(base),
        Activity::MacOs => {
            return RowView {
                tone: Tone::Secondary,
                state: Some(StateText::MacOs),
                state_tone: Tone::Secondary,
                ..base
            };
        }
        Activity::Failed(failure) => {
            if let Choice::Run(plan) = &choice {
                let mut retry = button(
                    Label::Retry,
                    Act::Confirm {
                        tool,
                        plan: plan.clone(),
                        update,
                    },
                    level_variant(row),
                );
                if f.busy.is_some() {
                    retry.disabled = Some(Tip::OneAtATime);
                }
                let reason = match failure {
                    Failure::Exit(code) => Reason::Exit(code),
                    Failure::NoStart => Reason::NoStart,
                    Failure::NotFound => Reason::NotFound(tool),
                };
                return RowView {
                    glyph: Glyph::Alert,
                    tone: Tone::Danger,
                    state: Some(StateText::Failed),
                    state_tone: Tone::Danger,
                    buttons: vec![
                        retry,
                        button(Label::ShowLog, Act::ShowLog(tool), Variant::Ghost),
                    ],
                    reason: Some(reason),
                    winget_instead: (failure != Failure::NotFound)
                        .then(|| winget_alternative(tool, &f.platform))
                        .flatten(),
                    log,
                    ..base
                };
            }
        }
        Activity::Idle => {}
    }
    let (glyph, tone, state) = if update {
        (
            Glyph::Alert,
            Tone::Warning,
            StateText::Outdated {
                version,
                minimum: tool.minimum().to_string(),
            },
        )
    } else if tool.level() == Level::Optional {
        (Glyph::Dashed, Tone::Tertiary, StateText::MissingOptional)
    } else {
        (Glyph::Dashed, Tone::Tertiary, StateText::Missing)
    };
    let mut main = match choice {
        Choice::Run(plan) => {
            let label = match (plan.runs_in, update) {
                (RunsIn::Copy, _) => Label::ShowCommand,
                (_, true) => Label::Update,
                (_, false) => Label::Install,
            };
            button(
                label,
                Act::Confirm { tool, plan, update },
                level_variant(row),
            )
        }
        Choice::Page(url) => {
            let mut b = button(Label::Download, Act::Download(url), level_variant(row));
            b.tip = Some(Tip::NoManager(tool));
            b
        }
    };
    if f.busy.is_some() && main.label != Label::Download && main.label != Label::ShowCommand {
        main.disabled = Some(Tip::OneAtATime);
    }
    RowView {
        glyph,
        tone,
        state: Some(state),
        state_tone: if update {
            Tone::Warning
        } else {
            Tone::Tertiary
        },
        buttons: vec![main],
        ..base
    }
}

/// What the diagnostic log keeps of a full check: only the rows that could
/// not be checked, by name and reason (compile-time text: never a path, a
/// version or an account). `None` when every row answered.
pub fn check_failures(rows: [Option<&ToolRow>; 3], sign_in: Option<SignIn>) -> Option<String> {
    let mut parts: Vec<String> = rows
        .iter()
        .flatten()
        .filter(|r| r.state == State::Unknown)
        .map(|r| format!("{} ({})", r.tool.exe_name(), r.reason.unwrap_or("unknown")))
        .collect();
    if let Some(SignIn::Unknown(why)) = sign_in {
        parts.push(format!("signed in ({why})"));
    }
    (!parts.is_empty()).then(|| format!("prereqs: could not check {}", parts.join(", ")))
}

/// The log line of an install that failed: the tool and the kind of
/// failure, nothing of its output.
pub fn install_failure_line(tool: Tool, failure: Failure) -> String {
    let what = match failure {
        Failure::Exit(Some(code)) => format!("exit code {code}"),
        Failure::Exit(None) => "stopped by a signal".to_owned(),
        Failure::NoStart => "could not start".to_owned(),
        Failure::NotFound => "not found after install".to_owned(),
    };
    format!("prereqs: {} install failed: {what}", tool.exe_name())
}

/// Quitting while an install runs asks first (ADR 0013: an install has no
/// cancel). A Quit answered once lets that quit through; the next install
/// asks again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuitGuard {
    confirmed: bool,
}

impl QuitGuard {
    /// The tool to name in the confirmation, when one is needed.
    pub fn must_ask(&self, busy: Option<Tool>) -> Option<Tool> {
        busy.filter(|_| !self.confirmed)
    }

    pub fn confirm(&mut self) {
        self.confirmed = true;
    }

    pub fn install_ended(&mut self) {
        self.confirmed = false;
    }
}

/// The sentences of the install confirmation (components-app.md 5,
/// "Confirmation dialog"), by role; `ready.rs` turns them into copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dialog {
    pub tool: Tool,
    pub update: bool,
    pub from: Option<Source>,
    pub scope: Option<ScopeText>,
    /// winget's agreements are accepted by the flags: said once.
    pub agreements: bool,
    /// How the source checks what it downloads, and the agreements, are
    /// mechanics (copy.md 7): behind "Details" rather than in the body.
    pub from_in_details: bool,
    pub updates: Option<Source>,
    /// How to remove it later; `None` for the native install's steps.
    pub uninstall: Option<Uninstall>,
    /// The exact command, as shown and copied.
    pub command: String,
    pub confirm: Option<Confirm>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeText {
    User,
    UserMayAsk,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uninstall {
    /// Claude Code's own setup page.
    Native,
    /// A manager's name, a proper noun ("winget", "Homebrew", "apt").
    Manager(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Install,
    Update,
    OpenInTerminal,
}

pub fn dialog(tool: Tool, plan: &Plan, update: bool, platform: &Platform) -> Dialog {
    let from = match plan.source {
        Source::Npm => None,
        s => Some(s),
    };
    let scope = match (plan.source, plan.scope) {
        (Source::Npm, _) => None,
        (_, Scope::User) if plan.may_elevate => Some(ScopeText::UserMayAsk),
        (_, Scope::User) => Some(ScopeText::User),
        (_, Scope::Machine) => Some(ScopeText::System),
    };
    let uninstall = if update {
        None
    } else {
        match plan.source {
            Source::Vendor => Some(Uninstall::Native),
            Source::Winget => Some(Uninstall::Manager("winget")),
            Source::Homebrew => Some(Uninstall::Manager("Homebrew")),
            Source::Distribution => platform.distro.map(|d| {
                Uninstall::Manager(match d.manager() {
                    "apt-get" => "apt",
                    other => other,
                })
            }),
            Source::Apple | Source::Npm => None,
        }
    };
    let confirm = match (plan.runs_in, update) {
        (RunsIn::Copy, _) => None,
        (RunsIn::Terminal, _) => Some(Confirm::OpenInTerminal),
        (_, true) => Some(Confirm::Update),
        (_, false) => Some(Confirm::Install),
    };
    Dialog {
        tool,
        update,
        from,
        scope,
        agreements: plan.source == Source::Winget,
        from_in_details: plan.source == Source::Winget,
        updates: from,
        uninstall,
        command: plan.display(),
        confirm,
    }
}

/// The words of a command as the dialog shows them, to wrap between words
/// only: one with a space is quoted, as [`Plan::display`] does.
pub fn command_words(plan: &Plan) -> Vec<String> {
    plan.argv
        .iter()
        .map(|a| {
            let a = a.to_string_lossy();
            if a.contains(' ') {
                format!("\"{a}\"")
            } else {
                a.into_owned()
            }
        })
        .collect()
}

/// Whether `text` (a folder's `.git/config`) names a `github.com` remote.
pub fn config_has_github_remote(text: &str) -> bool {
    let mut in_remote = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_remote = line.starts_with("[remote ");
            continue;
        }
        if !in_remote {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("url") {
            continue;
        }
        // `https://user@host:port/path` or scp-like `user@host:path`.
        let url = value.trim().trim_matches('"').to_ascii_lowercase();
        let rest = url.split_once("://").map_or(url.as_str(), |(_, r)| r);
        let authority = rest.split('/').next().unwrap_or("");
        let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = authority.split(':').next().unwrap_or("");
        if host == "github.com" {
            return true;
        }
    }
    false
}

/// The `config` file of a working folder's repository: `.git/config`, or
/// for a worktree (`.git` is a file) the common directory's.
pub fn git_config_of(folder: &std::path::Path) -> Option<std::path::PathBuf> {
    let dot = folder.join(".git");
    if dot.is_dir() {
        return Some(dot.join("config"));
    }
    let text = std::fs::read_to_string(&dot).ok()?;
    let gitdir = text.trim().strip_prefix("gitdir:")?.trim();
    let gitdir = folder.join(gitdir);
    let common = std::fs::read_to_string(gitdir.join("commondir"))
        .ok()
        .map(|c| gitdir.join(c.trim()))
        .unwrap_or(gitdir);
    Some(common.join("config"))
}

/// Whether any of `folders` has a `github.com` remote (read from disk).
pub fn any_github_remote(folders: &[std::path::PathBuf]) -> bool {
    folders.iter().any(|f| {
        git_config_of(f)
            .and_then(|c| std::fs::read_to_string(c).ok())
            .is_some_and(|t| config_has_github_remote(&t))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::prereqs::commands::Distro;
    use acta_core::prereqs::{AuthMethod, Kind, Version};
    use std::path::PathBuf;

    fn tool(tool: Tool, state: State, kind: Kind, v: Option<Version>) -> ToolRow {
        ToolRow {
            tool,
            state,
            path: (state != State::Missing).then(|| PathBuf::from("x")),
            version: v,
            kind,
            git_bash: None,
            reason: (state == State::Unknown).then_some("no answer within 5 s"),
        }
    }

    fn windows(winget: bool) -> Platform {
        Platform {
            os: Os::Windows,
            distro: None,
            winget,
            brew: false,
            distro_manager: false,
        }
    }

    fn linux_debian() -> Platform {
        Platform {
            os: Os::Linux,
            distro: Some(Distro::Debian),
            winget: false,
            brew: false,
            distro_manager: true,
        }
    }

    struct Rows {
        claude: ToolRow,
        git: ToolRow,
        gh: ToolRow,
    }

    fn all_found() -> Rows {
        Rows {
            claude: tool(
                Tool::Claude,
                State::Ok,
                Kind::Native,
                Some(Version::new(2, 1, 330)),
            ),
            git: ToolRow {
                git_bash: Some(true),
                ..tool(
                    Tool::Git,
                    State::Ok,
                    Kind::Winget,
                    Some(Version::new(2, 56, 1)),
                )
            },
            gh: tool(Tool::Gh, State::Missing, Kind::Unknown, None),
        }
    }

    fn facts<'a>(r: &'a Rows, sign_in: SignIn, platform: Platform) -> Facts<'a> {
        Facts {
            claude: Some(&r.claude),
            sign_in: Some(sign_in),
            git: Some(&r.git),
            gh: Some(&r.gh),
            platform,
            github: false,
            busy: None,
        }
    }

    const SIGNED: SignIn = SignIn::SignedIn(AuthMethod::ClaudeAi);

    #[test]
    fn found_rows_show_versions_and_no_action() {
        let r = all_found();
        let f = facts(&r, SIGNED, windows(true));
        let rows = rows(&f, false, |_| Activity::Idle);
        assert_eq!(
            rows.iter().map(|r| r.row).collect::<Vec<_>>(),
            [Row::Claude, Row::SignIn, Row::Git],
            "no GitHub remote and no gh: no GitHub CLI row"
        );
        assert_eq!(rows[0].state, Some(StateText::Version("2.1.330".into())));
        assert_eq!(rows[1].state, Some(StateText::SignedIn));
        assert_eq!(rows[2].state, Some(StateText::WithBash("2.56.1".into())));
        assert_eq!(rows[2].why, Why::GitWindows);
        assert!(
            rows.iter()
                .all(|r| r.buttons.is_empty() && r.glyph == Glyph::Check)
        );
        assert!(!has_primary(&rows));
        assert!(!first_run_shows(&f));
        assert_eq!(readiness(f.claude, f.sign_in), Readiness::Ready);
    }

    #[test]
    fn the_github_cli_row_needs_a_remote_or_gh_and_never_shows_on_first_run() {
        let r = all_found();
        let mut f = facts(&r, SIGNED, windows(true));
        f.github = true;
        assert!(rows_shown(&f, false).contains(&Row::Gh));
        assert!(!rows_shown(&f, true).contains(&Row::Gh));
        let gh = Rows {
            gh: tool(
                Tool::Gh,
                State::Ok,
                Kind::Winget,
                Some(Version::new(2, 105, 0)),
            ),
            ..all_found()
        };
        let f = facts(&gh, SIGNED, windows(true));
        assert!(rows_shown(&f, false).contains(&Row::Gh));
    }

    #[test]
    fn the_first_required_row_not_ready_gets_the_primary() {
        let r = Rows {
            git: tool(Tool::Git, State::Missing, Kind::Unknown, None),
            ..all_found()
        };
        let f = facts(&r, SignIn::SignedOut, windows(true));
        let rows = rows(&f, false, |_| Activity::Idle);
        assert_eq!(rows[1].state, Some(StateText::SignedOut));
        assert_eq!(rows[1].buttons[0].label, Label::SignIn);
        assert_eq!(rows[1].buttons[0].variant, Variant::Primary);
        assert_eq!(rows[1].buttons[0].tip, Some(Tip::SignInHint));
        // Git is recommended: ghost, Install... with winget.
        assert_eq!(rows[2].state, Some(StateText::Missing));
        assert_eq!(rows[2].buttons[0].label, Label::Install);
        assert_eq!(rows[2].buttons[0].variant, Variant::Ghost);
        assert!(has_primary(&rows));
        assert!(first_run_shows(&f));
        assert_eq!(readiness(f.claude, f.sign_in), Readiness::SignedOut);
    }

    #[test]
    fn missing_claude_code_disables_the_sign_in() {
        let r = Rows {
            claude: tool(Tool::Claude, State::Missing, Kind::Unknown, None),
            ..all_found()
        };
        let f = facts(&r, SignIn::NoClaude, windows(true));
        let rows = rows(&f, true, |_| Activity::Idle);
        assert_eq!(rows[0].buttons[0].label, Label::Install);
        assert_eq!(rows[0].buttons[0].variant, Variant::Primary);
        let Act::Confirm { plan, update, .. } = &rows[0].buttons[0].act else {
            panic!("{:?}", rows[0].buttons[0].act)
        };
        assert!(!update);
        assert_eq!(plan.source, Source::Vendor);
        assert_eq!(rows[1].buttons[0].variant, Variant::Secondary);
        assert_eq!(rows[1].buttons[0].disabled, Some(Tip::NeedsClaude));
        assert_eq!(readiness(f.claude, f.sign_in), Readiness::Missing);
    }

    #[test]
    fn outdated_offers_the_update_of_its_install_kind_or_the_command() {
        let r = Rows {
            claude: tool(
                Tool::Claude,
                State::Outdated,
                Kind::Native,
                Some(Version::new(2, 1, 100)),
            ),
            git: tool(
                Tool::Git,
                State::Outdated,
                Kind::Unknown,
                Some(Version::new(2, 20, 0)),
            ),
            gh: tool(Tool::Gh, State::Missing, Kind::Unknown, None),
        };
        let f = facts(&r, SIGNED, windows(true));
        let rows = rows(&f, false, |_| Activity::Idle);
        assert_eq!(rows[0].glyph, Glyph::Alert);
        assert_eq!(rows[0].tone, Tone::Warning);
        assert_eq!(
            rows[0].state,
            Some(StateText::Outdated {
                version: "2.1.100".into(),
                minimum: "2.1.329".into()
            })
        );
        assert_eq!(rows[0].buttons[0].label, Label::Update);
        assert_eq!(rows[0].buttons[0].variant, Variant::Primary);
        // An unknown install kind has no one-command update: the page.
        assert_eq!(rows[2].buttons[0].label, Label::Download);
        // An outdated Claude Code still runs: the bar does not cry wolf.
        assert_eq!(readiness(f.claude, f.sign_in), Readiness::Ready);
        // npm: the command with Copy only.
        let npm = Rows {
            claude: tool(
                Tool::Claude,
                State::Outdated,
                Kind::Npm,
                Some(Version::new(2, 1, 100)),
            ),
            ..all_found()
        };
        let f = facts(&npm, SIGNED, windows(true));
        assert_eq!(
            row_view(&f, Row::Claude, Activity::Idle).buttons[0].label,
            Label::ShowCommand
        );
    }

    #[test]
    fn one_install_at_a_time() {
        let r = Rows {
            claude: tool(Tool::Claude, State::Missing, Kind::Unknown, None),
            git: tool(Tool::Git, State::Missing, Kind::Unknown, None),
            ..all_found()
        };
        let mut f = facts(&r, SignIn::NoClaude, windows(true));
        f.busy = Some(Tool::Git);
        let running = Activity::Running {
            update: false,
            elapsed: Duration::from_secs(42),
        };
        let rows = rows(&f, false, |row| {
            if row == Row::Git {
                running
            } else {
                Activity::Idle
            }
        });
        assert_eq!(rows[2].state, Some(StateText::Installing("0:42".into())));
        assert_eq!(rows[2].buttons[0].label, Label::ShowLog);
        assert_eq!(rows[2].log, Some(Tool::Git));
        assert_eq!(rows[0].buttons[0].disabled, Some(Tip::OneAtATime));
        // Nothing is shown before 400 ms.
        let early = row_view(
            &f,
            Row::Git,
            Activity::Running {
                update: false,
                elapsed: Duration::from_millis(200),
            },
        );
        assert_eq!(early.state, None);
        assert_eq!(elapsed(Duration::from_secs(61)).as_deref(), Some("1:01"));
    }

    #[test]
    fn a_failure_offers_retry_the_log_and_winget_for_claude_code() {
        let r = Rows {
            claude: tool(Tool::Claude, State::Missing, Kind::Unknown, None),
            ..all_found()
        };
        let f = facts(&r, SignIn::NoClaude, windows(true));
        let v = row_view(&f, Row::Claude, Activity::Failed(Failure::Exit(Some(1603))));
        assert_eq!(v.glyph, Glyph::Alert);
        assert_eq!(v.tone, Tone::Danger);
        assert_eq!(v.state, Some(StateText::Failed));
        assert_eq!(v.reason, Some(Reason::Exit(Some(1603))));
        assert_eq!(
            v.buttons.iter().map(|b| b.label).collect::<Vec<_>>(),
            [Label::Retry, Label::ShowLog]
        );
        assert_eq!(v.winget_instead.unwrap().source, Source::Winget);
        // Without winget there is no alternative; Git has none either.
        let f = facts(&r, SignIn::NoClaude, windows(false));
        let v = row_view(&f, Row::Claude, Activity::Failed(Failure::Exit(Some(1))));
        assert_eq!(v.winget_instead, None);
        let v = row_view(&f, Row::Git, Activity::Failed(Failure::NotFound));
        // Git was found meanwhile: found wins over the old failure.
        assert_eq!(v.glyph, Glyph::Check);
    }

    #[test]
    fn no_package_manager_offers_the_download_page() {
        let r = Rows {
            git: tool(Tool::Git, State::Missing, Kind::Unknown, None),
            ..all_found()
        };
        let f = facts(&r, SIGNED, windows(false));
        let v = row_view(&f, Row::Git, Activity::Idle);
        assert_eq!(v.state, Some(StateText::Missing));
        assert_eq!(v.buttons[0].label, Label::Download);
        assert_eq!(v.buttons[0].tip, Some(Tip::NoManager(Tool::Git)));
        assert!(
            matches!(v.buttons[0].act, Act::Download(u) if u.starts_with("https://git-scm.com/"))
        );
    }

    #[test]
    fn linux_root_installs_wait_for_the_terminal() {
        let r = Rows {
            git: tool(Tool::Git, State::Missing, Kind::Unknown, None),
            ..all_found()
        };
        let f = facts(&r, SignIn::SignedOut, linux_debian());
        let v = row_view(&f, Row::Git, Activity::Idle);
        let Act::Confirm { plan, .. } = &v.buttons[0].act else {
            panic!()
        };
        assert_eq!(plan.runs_in, RunsIn::Terminal);
        let d = dialog(Tool::Git, plan, false, &f.platform);
        assert_eq!(d.confirm, Some(Confirm::OpenInTerminal));
        assert_eq!(d.uninstall, Some(Uninstall::Manager("apt")));
        assert_eq!(d.scope, Some(ScopeText::System));
        assert_eq!(d.command, "sudo apt-get install git");
        let v = row_view(&f, Row::Git, Activity::Terminal);
        assert_eq!(v.glyph, Glyph::Terminal);
        assert_eq!(v.state, Some(StateText::Terminal));
        assert_eq!(v.buttons[0].act, Act::ShowTerminal(Row::Git));
        let v = row_view(&f, Row::SignIn, Activity::Terminal);
        assert_eq!(v.state, Some(StateText::Terminal));
        // Signed in meanwhile: the wait is over.
        let f = facts(&r, SIGNED, linux_debian());
        assert_eq!(
            row_view(&f, Row::SignIn, Activity::Terminal).state,
            Some(StateText::SignedIn)
        );
    }

    #[test]
    fn an_api_key_is_not_an_error() {
        let r = all_found();
        let f = facts(&r, SignIn::SignedIn(AuthMethod::ApiKey), windows(true));
        let v = row_view(&f, Row::SignIn, Activity::Idle);
        assert_eq!(v.glyph, Glyph::Check);
        assert_eq!(v.state, Some(StateText::ApiKey));
        assert_eq!(v.state_tip, Some(Tip::ApiKey));
        assert!(v.buttons.is_empty());
    }

    #[test]
    fn rows_not_checked_yet_claim_nothing() {
        let f = Facts {
            claude: None,
            sign_in: None,
            git: None,
            gh: None,
            platform: windows(true),
            github: false,
            busy: None,
        };
        let rows = rows(&f, false, |_| Activity::Idle);
        assert!(
            rows.iter()
                .all(|r| r.state.is_none() && r.buttons.is_empty())
        );
        assert!(!first_run_shows(&f));
        assert_eq!(readiness(None, None), Readiness::Unknown);
    }

    #[test]
    fn the_winget_dialog_says_where_scope_agreements_and_updates() {
        let p = windows(true);
        let Choice::Run(plan) = install_plan(Tool::Git, &p) else {
            panic!()
        };
        let d = dialog(Tool::Git, &plan, false, &p);
        assert_eq!(d.from, Some(Source::Winget));
        assert!(
            d.from_in_details,
            "the hash and the licence are behind Details"
        );
        assert_eq!(d.scope, Some(ScopeText::UserMayAsk));
        assert!(d.agreements);
        assert_eq!(d.updates, Some(Source::Winget));
        assert_eq!(d.uninstall, Some(Uninstall::Manager("winget")));
        assert_eq!(d.confirm, Some(Confirm::Install));
        assert_eq!(
            d.command,
            "winget install --id Git.Git -e --source winget --scope user \
             --accept-package-agreements --accept-source-agreements --disable-interactivity"
        );
        let Choice::Run(plan) = install_plan(Tool::Claude, &p) else {
            panic!()
        };
        let d = dialog(Tool::Claude, &plan, false, &p);
        assert_eq!(d.from, Some(Source::Vendor));
        assert!(!d.from_in_details);
        assert_eq!(d.scope, Some(ScopeText::User));
        assert!(!d.agreements);
        assert_eq!(d.uninstall, Some(Uninstall::Native));
        assert!(
            d.command
                .ends_with("\"irm https://claude.ai/install.ps1 | iex\"")
        );
        let Choice::Run(plan) = update_plan(Tool::Claude, Kind::Native, &p) else {
            panic!()
        };
        let d = dialog(Tool::Claude, &plan, true, &p);
        assert_eq!(d.confirm, Some(Confirm::Update));
        assert_eq!(d.uninstall, None);
        assert_eq!(d.command, "claude update");
    }

    #[test]
    fn the_command_wraps_between_the_words_it_shows() {
        for p in [windows(true), windows(false), linux_debian()] {
            for tool in [Tool::Claude, Tool::Git, Tool::Gh] {
                if let Choice::Run(plan) = install_plan(tool, &p) {
                    assert_eq!(command_words(&plan).join(" "), plan.display());
                }
            }
        }
    }

    #[test]
    fn the_log_keeps_only_failures_without_paths() {
        let r = all_found();
        let rows = [Some(&r.claude), Some(&r.git), Some(&r.gh)];
        assert_eq!(check_failures(rows, Some(SIGNED)), None);
        let git = ToolRow {
            path: Some(PathBuf::from(r"C:\Users\alexandre\git.exe")),
            ..tool(Tool::Git, State::Unknown, Kind::Unknown, None)
        };
        let line = check_failures(
            [Some(&r.claude), Some(&git), None],
            Some(SignIn::Unknown("no answer within 10 s")),
        )
        .unwrap();
        assert_eq!(
            line,
            "prereqs: could not check git (no answer within 5 s), signed in (no answer within 10 s)"
        );
        assert!(!line.contains("alexandre"));
        assert_eq!(
            install_failure_line(Tool::Gh, Failure::Exit(Some(1603))),
            "prereqs: gh install failed: exit code 1603"
        );
    }

    #[test]
    fn quitting_asks_during_an_install_until_answered_and_again_next_time() {
        let mut q = QuitGuard::default();
        assert_eq!(q.must_ask(None), None);
        assert_eq!(q.must_ask(Some(Tool::Git)), Some(Tool::Git));
        q.confirm();
        assert_eq!(q.must_ask(Some(Tool::Git)), None);
        q.install_ended();
        assert_eq!(q.must_ask(Some(Tool::Gh)), Some(Tool::Gh));
    }

    #[test]
    fn a_github_remote_is_read_from_the_config() {
        let ssh = "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = git@github.com:alexandre/acta.git\n";
        assert!(config_has_github_remote(ssh));
        let https = "[remote \"origin\"]\n  url = https://github.com/alexandre/acta\n";
        assert!(config_has_github_remote(https));
        let token = "[remote \"o\"]\nurl = https://alexandre@GitHub.com/a/b.git\n";
        assert!(config_has_github_remote(token));
        let other = "[remote \"origin\"]\n\turl = https://gitlab.com/alexandre/acta.git\n\
                     [branch \"main\"]\n\turl = https://github.com/x\n";
        assert!(!config_has_github_remote(other));
        let lookalike = "[remote \"o\"]\n\turl = https://github.com.evil.example/a\n";
        assert!(!config_has_github_remote(lookalike));
        assert!(!config_has_github_remote(""));
    }

    #[test]
    fn a_worktree_reads_the_config_of_its_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("acta");
        std::fs::create_dir_all(main.join(".git").join("worktrees").join("wt")).unwrap();
        std::fs::write(
            main.join(".git").join("config"),
            "[remote \"origin\"]\n\turl = git@github.com:alexandre/acta.git\n",
        )
        .unwrap();
        let wt_git = main.join(".git").join("worktrees").join("wt");
        std::fs::write(wt_git.join("commondir"), "../..\n").unwrap();
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", wt_git.display())).unwrap();
        assert!(any_github_remote(&[wt]));
        assert!(any_github_remote(&[main]));
        assert!(!any_github_remote(&[tmp.path().join("none")]));
    }
}
