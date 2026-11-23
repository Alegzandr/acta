//! Ready for Claude Code in the app (ADR 0013; components-app.md 5;
//! screens.md 8.26): what detection found, kept in memory only (never on
//! disk), the installs the user confirmed, and the block drawn in
//! Settings > Claude Code and on first run.
//!
//! When it checks: the Claude Code and sign-in rows once at launch (the
//! status bar); every row when Settings opens, when first run shows the
//! block, on "Check again", after an install or a terminal session of the
//! checklist ends, and on focus while the block is shown (at most once per
//! 10 s). Detection and installs run off the UI thread.
//!
//! Debug builds started with `ACTA_DEV_READY=<state>` show a fixed
//! detection instead (see [`dev`]) and never detect, install or ask
//! `claude auth status` anything: captures of every state need no real
//! install.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::zoom::px;
use acta_core::paths::Os;
use acta_core::prereqs::detect::detect_tool;
use acta_core::prereqs::{
    Installer, InstallerEnv, Machine, Phase, Plan, Platform, SignIn, State, SystemExec, Tool,
    ToolRow, detect, detect::detect_sign_in, locate,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClipboardItem, Context, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, ScrollHandle, StatefulInteractiveElement, Styled, div,
};

use crate::db::Db;
use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::icons::{Icon, icon};
use crate::ready_model::{
    self as model, Act, Activity, Facts, Glyph, Label, Readiness, Reason, Row, RowView, StateText,
    Tip, Tone, Why,
};
use crate::theme::{RADIUS_SM, Tokens};
use crate::type_scale;
use crate::ui;

/// A focus re-check waits this long after the last full check.
const FOCUS_THROTTLE: Duration = Duration::from_secs(10);

/// What a block's button asks of the shell (a dialog, the terminal).
#[derive(Clone, Debug)]
pub enum Request {
    /// The install (or update) confirmation of `plan`.
    Confirm {
        tool: Tool,
        plan: Plan,
        update: bool,
    },
    /// "Sign in...": Claude Code in a one-off terminal session.
    SignIn,
    /// "Show terminal": the one-off session.
    ShowTerminal,
}

/// The installer's view of this computer: the fresh `PATH`, and whether a
/// tool is found after its install.
struct SystemEnv;

impl InstallerEnv for SystemEnv {
    fn fresh_path(&self) -> Option<OsString> {
        locate::system_fresh_path()
    }

    fn found(&self, tool: Tool, _path: Option<&std::ffi::OsStr>) -> bool {
        // The search reads the process and registry `PATH` and the known
        // folders, which covers the fresh `PATH`.
        let m = Machine::current();
        locate::locate(m.os, tool.exe_name(), &m.dirs, locate::is_file).is_some()
    }
}

/// The last detection and the installs of this run of Acta.
pub struct Ready {
    claude: Option<ToolRow>,
    sign_in: Option<SignIn>,
    git: Option<ToolRow>,
    gh: Option<ToolRow>,
    platform: Platform,
    /// An attached folder has a `github.com` remote.
    github: bool,
    checking: bool,
    /// A check was asked for while one ran: another follows it.
    again: bool,
    /// When the last full check finished.
    full_at: Option<Instant>,
    installer: Installer,
    /// Whether the job of each tool is an update.
    updating: HashMap<Tool, bool>,
    /// Rows waiting for a terminal session or for macOS.
    waits: HashMap<Row, Activity>,
    logs_open: HashSet<Tool>,
    log_scroll: HashMap<Tool, ScrollHandle>,
    polling: bool,
    /// Quitting during an install asks first.
    pub quit: model::QuitGuard,
    db: Option<Db>,
    #[cfg(debug_assertions)]
    dev: Option<dev::Scenario>,
}

impl Default for Ready {
    fn default() -> Self {
        Ready {
            claude: None,
            sign_in: None,
            git: None,
            gh: None,
            platform: Platform {
                os: Os::CURRENT,
                distro: None,
                winget: false,
                brew: false,
                distro_manager: false,
            },
            github: false,
            checking: false,
            again: false,
            full_at: None,
            installer: Installer::new(Arc::new(SystemEnv)),
            updating: HashMap::new(),
            waits: HashMap::new(),
            logs_open: HashSet::new(),
            log_scroll: HashMap::new(),
            polling: false,
            quit: model::QuitGuard::default(),
            db: None,
            #[cfg(debug_assertions)]
            dev: None,
        }
    }
}

impl gpui_kit::Global for Ready {}

impl Ready {
    fn facts(&self) -> Facts<'_> {
        Facts {
            claude: self.claude.as_ref(),
            sign_in: self.sign_in,
            git: self.git.as_ref(),
            gh: self.gh.as_ref(),
            platform: self.platform,
            github: self.github,
            busy: self.busy(),
        }
    }

    fn busy(&self) -> Option<Tool> {
        #[cfg(debug_assertions)]
        if let Some(dev) = &self.dev {
            return dev
                .activity
                .iter()
                .find_map(|(row, a)| matches!(a, Activity::Running { .. }).then(|| row.tool()));
        }
        self.installer.busy()
    }

    fn activity(&self, row: Row) -> Activity {
        #[cfg(debug_assertions)]
        if let Some(dev) = &self.dev {
            return dev.activity.get(&row).copied().unwrap_or(Activity::Idle);
        }
        if let Some(wait) = self.waits.get(&row) {
            return *wait;
        }
        if row == Row::SignIn {
            return Activity::Idle;
        }
        let tool = row.tool();
        match self.installer.phase(tool) {
            Some(Phase::Running) => Activity::Running {
                update: self.updating.get(&tool).copied().unwrap_or(false),
                elapsed: self.installer.elapsed(tool).unwrap_or_default(),
            },
            Some(Phase::Failed(f)) => Activity::Failed(f),
            Some(Phase::Installed) | None => Activity::Idle,
        }
    }

    fn log(&self, tool: Tool) -> Vec<String> {
        #[cfg(debug_assertions)]
        if let Some(dev) = &self.dev {
            return dev.logs.get(&tool).cloned().unwrap_or_default();
        }
        self.installer.log(tool)
    }

    fn rows(&self, first_run: bool) -> Vec<RowView> {
        model::rows(&self.facts(), first_run, |row| self.activity(row))
    }
}

/// Start keeping the checklist: at workbench start, with the store (for
/// the attached folders' remotes).
pub fn init(db: Db, cx: &mut App) {
    #[allow(unused_mut)]
    let mut ready = Ready {
        db: Some(db),
        #[cfg(debug_assertions)]
        dev: dev::from_env(),
        ..Ready::default()
    };
    #[cfg(debug_assertions)]
    if ready.dev.is_some() {
        ready.apply_dev();
        ready.full_at = Some(Instant::now());
    }
    cx.set_global(ready);
}

fn state(cx: &App) -> Option<&Ready> {
    cx.try_global::<Ready>()
}

/// The status bar's view of the required rows.
pub fn readiness(cx: &App) -> Readiness {
    state(cx).map_or(Readiness::Unknown, |r| {
        model::readiness(r.claude.as_ref(), r.sign_in)
    })
}

/// The install to name before quitting, when quitting must ask.
pub fn must_ask_quit(cx: &App) -> Option<Tool> {
    state(cx).and_then(|r| r.quit.must_ask(r.busy()))
}

/// Quit was answered: this quit goes through.
pub fn confirm_quit(cx: &mut App) {
    if cx.has_global::<Ready>() {
        cx.global_mut::<Ready>().quit.confirm();
    }
}

/// Whether the block has a primary button.
pub fn has_primary(cx: &App, first_run: bool) -> bool {
    state(cx).is_some_and(|r| model::has_primary(&r.rows(first_run)))
}

/// Whether first run shows the block.
pub fn first_run_shows(cx: &App) -> bool {
    state(cx).is_some_and(|r| model::first_run_shows(&r.facts()))
}

/// The platform the commands are chosen for.
pub fn platform(cx: &App) -> Option<Platform> {
    state(cx).map(|r| r.platform)
}

#[cfg(debug_assertions)]
pub(crate) fn is_dev(cx: &App) -> bool {
    state(cx).is_some_and(|r| r.dev.is_some())
}

#[cfg(not(debug_assertions))]
pub(crate) fn is_dev(_: &App) -> bool {
    false
}

/// The Claude Code and sign-in rows only, once at launch (the status
/// bar): no `git` or `gh` is run.
pub fn launch_check(cx: &mut App) {
    if is_dev(cx) || state(cx).is_none() {
        return;
    }
    let task = cx.background_executor().spawn(async move {
        let m = Machine::current();
        let exec = SystemExec::current();
        let claude = detect_tool(&m, Tool::Claude, &locate::is_file, &exec);
        let sign_in = detect_sign_in(&claude, &exec);
        (claude, sign_in)
    });
    cx.spawn(async move |cx| {
        let (claude, sign_in) = task.await;
        cx.update(|cx| {
            let r = cx.global_mut::<Ready>();
            // A full check that finished meanwhile is newer.
            if r.full_at.is_none() {
                r.claude = Some(claude);
                r.sign_in = Some(sign_in);
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

/// Every row, now ("Check again", after an install or a terminal session
/// ends).
pub fn check(cx: &mut App) {
    let Some(r) = state(cx) else { return };
    if r.checking {
        // Once it ends: what changed meanwhile (an install ended) counts.
        cx.global_mut::<Ready>().again = true;
        return;
    }
    if is_dev(cx) {
        cx.global_mut::<Ready>().full_at = Some(Instant::now());
        return;
    }
    let folders = r.db.clone().map(|db| {
        db.run(cx, |store| {
            Ok(store
                .all_folders()?
                .into_iter()
                .map(|f| PathBuf::from(f.path))
                .collect::<Vec<_>>())
        })
    });
    cx.global_mut::<Ready>().checking = true;
    cx.refresh_windows();
    let background = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        let folders = match folders {
            Some(task) => task.await.unwrap_or_default(),
            None => Vec::new(),
        };
        let (d, platform, github) = background
            .spawn(async move {
                let m = Machine::current();
                let exec = SystemExec::current();
                let d = detect(&m, &locate::is_file, &exec);
                let platform = Platform::detect(&m, &locate::is_file);
                (d, platform, model::any_github_remote(&folders))
            })
            .await;
        // The log keeps only the rows that could not be checked (ADR 0013).
        if let Some(line) = model::check_failures(
            [Some(&d.claude), Some(&d.git), Some(&d.gh)],
            Some(d.sign_in),
        ) {
            crate::log::line(&line);
        }
        cx.update(|cx| {
            let r = cx.global_mut::<Ready>();
            r.checking = false;
            r.full_at = Some(Instant::now());
            let again = std::mem::take(&mut r.again);
            r.claude = Some(d.claude);
            r.sign_in = Some(d.sign_in);
            r.git = Some(d.git);
            r.gh = Some(d.gh);
            r.platform = platform;
            r.github = github;
            // A wait ends once its row is ready.
            let ready_now = |row: Row, r: &Ready| match row {
                Row::SignIn => matches!(r.sign_in, Some(SignIn::SignedIn(_))),
                Row::Claude => r.claude.as_ref().is_some_and(|t| t.state == State::Ok),
                Row::Git => r.git.as_ref().is_some_and(|t| t.state == State::Ok),
                Row::Gh => r.gh.as_ref().is_some_and(|t| t.state == State::Ok),
            };
            let done: Vec<Row> = r
                .waits
                .keys()
                .copied()
                .filter(|row| ready_now(*row, r))
                .collect();
            for row in done {
                r.waits.remove(&row);
            }
            cx.refresh_windows();
            if again {
                check(cx);
            }
        });
    })
    .detach();
}

/// Every row unless the last full check is under 10 s old (Settings
/// opens; the window regains focus while the block is shown).
pub fn check_if_stale(cx: &mut App) {
    let recent = state(cx)
        .is_some_and(|r| r.checking || r.full_at.is_some_and(|at| at.elapsed() < FOCUS_THROTTLE));
    if !recent {
        check(cx);
    }
}

/// Run a confirmed plan whose command runs as a child (ADR 0013,
/// "Running"): one at a time, its log in memory, then a check.
pub fn start(tool: Tool, plan: &Plan, update: bool, cx: &mut App) {
    #[cfg(debug_assertions)]
    if let Some(dev) = cx.global_mut::<Ready>().dev.as_mut() {
        // Never a real install from a development state.
        dev.activity.insert(
            Row::of(tool),
            Activity::Running {
                update,
                elapsed: Duration::ZERO,
            },
        );
        cx.refresh_windows();
        return;
    }
    let r = cx.global_mut::<Ready>();
    r.waits.remove(&Row::of(tool));
    if r.installer.start(tool, &plan.argv).is_err() {
        return;
    }
    r.updating.insert(tool, update);
    poll(tool, cx);
}

/// Each second while an install runs: the elapsed time, then the check
/// once it ends.
fn poll(tool: Tool, cx: &mut App) {
    let r = cx.global_mut::<Ready>();
    if r.polling {
        return;
    }
    r.polling = true;
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let running = cx.update(|cx| {
                cx.refresh_windows();
                cx.global::<Ready>().installer.busy().is_some()
            });
            if !running {
                break;
            }
        }
        cx.update(|cx| {
            let r = cx.global_mut::<Ready>();
            r.polling = false;
            // The next install asks again before a quit.
            r.quit.install_ended();
            // Only a failure is logged: the tool and the kind, no output.
            if let Some(Phase::Failed(failure)) = r.installer.phase(tool) {
                crate::log::line(&model::install_failure_line(tool, failure));
            }
            check(cx);
        });
    })
    .detach();
}

/// `xcode-select --install`: Apple's window opens and the command returns
/// at once; the row waits for the next focus.
pub fn start_apple(plan: &Plan, cx: &mut App) {
    if is_dev(cx) {
        return;
    }
    let argv = plan.argv.clone();
    cx.background_executor()
        .spawn(async move {
            if let Some((program, args)) = argv.split_first() {
                let _ = std::process::Command::new(program)
                    .args(args)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        })
        .detach();
    cx.global_mut::<Ready>()
        .waits
        .insert(Row::Git, Activity::MacOs);
    cx.refresh_windows();
}

/// A terminal session now runs for `row`.
pub fn wait_terminal(row: Row, cx: &mut App) {
    if cx.has_global::<Ready>() {
        let r = cx.global_mut::<Ready>();
        #[cfg(debug_assertions)]
        if let Some(dev) = r.dev.as_mut() {
            dev.activity.insert(row, Activity::Terminal);
        }
        r.waits.insert(row, Activity::Terminal);
    }
    cx.refresh_windows();
}

/// The terminal session of `row` ended: the row re-checks.
pub fn terminal_ended(row: Row, cx: &mut App) {
    if state(cx).is_none() {
        return;
    }
    let r = cx.global_mut::<Ready>();
    r.waits.remove(&row);
    #[cfg(debug_assertions)]
    if let Some(dev) = r.dev.as_mut() {
        dev.activity.remove(&row);
    }
    check(cx);
    cx.refresh_windows();
}

/// The one-off session of an install that runs in the terminal: the
/// program found on the fresh search directories, in the home folder.
/// Blocking: call it off the UI thread.
pub fn one_off_spec(argv: &[OsString]) -> Option<acta_terminal::LaunchSpec> {
    let m = Machine::current();
    let home = crate::paths_view::home().map(PathBuf::from)?;
    acta_terminal::launch::one_off_spec(
        argv,
        home,
        &m.dirs,
        locate::system_fresh_path().as_deref(),
        locate::is_file,
    )
}

/// "Sign in...": Claude Code alone, in `cwd` (the open project's folder)
/// or the home folder, with the fresh `PATH`. Blocking.
pub fn sign_in_spec(cwd: Option<PathBuf>) -> Option<acta_terminal::LaunchSpec> {
    let program = acta_terminal::launch::resolve_claude()?;
    let cwd = cwd
        .filter(|c| c.is_dir())
        .or_else(|| crate::paths_view::home().map(PathBuf::from))?;
    let mut spec = acta_terminal::launch::plain_claude_spec(&program, cwd)?;
    if let Some(path) = locate::system_fresh_path() {
        acta_terminal::launch::set_path(
            &mut spec.env,
            cfg!(windows),
            &acta_terminal::launch::env_names(),
            &path,
        );
    }
    Some(spec)
}

fn tone(tone: Tone, t: &Tokens) -> gpui_kit::Hsla {
    match tone {
        Tone::Secondary => t.text_secondary,
        Tone::Tertiary => t.text_tertiary,
        Tone::Success => t.success,
        Tone::Warning => t.warning,
        Tone::Danger => t.danger,
    }
}

fn glyph(g: Glyph) -> Icon {
    match g {
        Glyph::Check => Icon::Check,
        Glyph::Dashed => Icon::CircleDashed,
        Glyph::Alert => Icon::TriangleAlert,
        Glyph::Terminal => Icon::SquareTerminal,
    }
}

fn name_key(row: Row) -> Key {
    match row {
        Row::Claude => Key::ReadyClaude,
        Row::SignIn => Key::ReadySignin,
        Row::Git => Key::ReadyGit,
        Row::Gh => Key::ReadyGh,
    }
}

fn why_key(why: Why) -> Key {
    match why {
        Why::Claude => Key::ReadyClaudeWhy,
        Why::SignIn => Key::ReadySigninWhy,
        Why::Git => Key::ReadyGitWhy,
        Why::GitWindows => Key::ReadyGitWhyWindows,
        Why::Gh => Key::ReadyGhWhy,
    }
}

/// The state text of a row, in `locale`.
pub fn state_text(locale: Locale, s: &StateText) -> String {
    let v = |key: Key, version: &str| tr(locale, key, &[("version", Arg::Text(version))]);
    let e = |key: Key, elapsed: &str| tr(locale, key, &[("elapsed", Arg::Text(elapsed))]);
    match s {
        StateText::Version(x) => v(Key::ReadyStateFound, x),
        StateText::WithBash(x) => v(Key::ReadyStateWithBash, x),
        StateText::WithoutBash(x) => v(Key::ReadyStateWithoutBash, x),
        StateText::Missing => text(locale, Key::ReadyStateMissing),
        StateText::MissingOptional => text(locale, Key::ReadyStateMissingOptional),
        StateText::Outdated { version, minimum } => tr(
            locale,
            Key::ReadyStateOutdated,
            &[
                ("version", Arg::Text(version)),
                ("minimum", Arg::Text(minimum)),
            ],
        ),
        StateText::Installing(x) => e(Key::ReadyStateInstalling, x),
        StateText::Updating(x) => e(Key::ReadyStateUpdating, x),
        StateText::Terminal => text(locale, Key::ReadyStateTerminal),
        StateText::MacOs => text(locale, Key::ReadyStateMacos),
        StateText::Failed => text(locale, Key::ReadyStateFailed),
        StateText::SignedIn => text(locale, Key::ReadyStateSignedIn),
        StateText::ApiKey => text(locale, Key::ReadyStateApiKeyUsed),
        StateText::SignedOut => text(locale, Key::ReadyStateSignedOut),
        StateText::Unknown => text(locale, Key::SettingsCheckUnknown),
    }
}

fn label_text(locale: Locale, label: Label) -> String {
    text(
        locale,
        match label {
            Label::Install => Key::ReadyInstall,
            Label::Update => Key::ReadyUpdate,
            Label::ShowCommand => Key::ReadyShowCommand,
            Label::Download => Key::ReadyDownload,
            Label::SignIn => Key::ReadySigninButton,
            Label::ShowLog => Key::ReadyShowLog,
            Label::Retry => Key::CommonRetry,
            Label::ShowTerminal => Key::ReadyShowTerminal,
            Label::CheckAgain => Key::ReadyCheckAgain,
        },
    )
}

fn tip_text(locale: Locale, tip: Tip) -> String {
    match tip {
        Tip::NeedsClaude => text(locale, Key::ReadySigninNeedsClaude),
        Tip::OneAtATime => text(locale, Key::ReadyOneAtATime),
        Tip::ApiKey => text(locale, Key::ReadyStateApiKeyTip),
        Tip::NoManager(tool) => tr(
            locale,
            Key::ReadyNoManager,
            &[("name", Arg::Text(tool.display_name()))],
        ),
        Tip::SignInHint => text(locale, Key::ReadySigninHint),
        Tip::Unknown(why) => why.to_owned(),
    }
}

fn reason_text(locale: Locale, reason: Reason) -> Option<String> {
    match reason {
        Reason::Exit(Some(code)) => Some(tr(
            locale,
            Key::ReadyFailedExit,
            &[("code", Arg::Text(&code.to_string()))],
        )),
        Reason::Exit(None) => None,
        Reason::NotFound(tool) => Some(tr(
            locale,
            Key::ReadyFailedNotFound,
            &[("name", Arg::Text(tool.display_name()))],
        )),
        Reason::NoStart => Some(text(locale, Key::ReadyFailedNoStart)),
    }
}

fn row_id(row: Row) -> &'static str {
    match row {
        Row::Claude => "ready-claude",
        Row::SignIn => "ready-signin",
        Row::Git => "ready-git",
        Row::Gh => "ready-gh",
    }
}

fn variant(v: model::Variant) -> ui::Variant {
    match v {
        model::Variant::Primary => ui::Variant::Primary,
        model::Variant::Secondary => ui::Variant::Secondary,
        model::Variant::Ghost => ui::Variant::Ghost,
    }
}

/// What a click does: the shell's part goes out as a [`Request`].
fn act<T: EventEmitter<E> + 'static, E: From<Request> + 'static>(act: &Act, cx: &mut Context<T>) {
    match act {
        Act::Confirm { tool, plan, update } => cx.emit(E::from(Request::Confirm {
            tool: *tool,
            plan: plan.clone(),
            update: *update,
        })),
        Act::Download(url) => cx.open_url(url),
        Act::SignIn => cx.emit(E::from(Request::SignIn)),
        // The log closes with its own "Hide log" (screens.md 8.26).
        Act::ShowLog(tool) => {
            cx.global_mut::<Ready>().logs_open.insert(*tool);
            cx.refresh_windows();
        }
        Act::ShowTerminal(_) => cx.emit(E::from(Request::ShowTerminal)),
        Act::CheckAgain => check(cx),
    }
}

/// The block (components-app.md 5): heading, line, rows, "Check again"
/// (not on first run). Nothing when the checklist is not kept.
pub fn block<T: EventEmitter<E> + 'static, E: From<Request> + 'static>(
    first_run: bool,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<T>,
) -> Vec<AnyElement> {
    let Some(r) = state(cx) else {
        return Vec::new();
    };
    let rows = r.rows(first_run);
    let busy = r.busy().is_some();
    let checking = r.checking;
    let logs: Vec<(Tool, Vec<String>, ScrollHandle, bool)> = rows
        .iter()
        .filter_map(|v| v.log)
        .filter(|tool| r.logs_open.contains(tool))
        .map(|tool| {
            let running = r.busy() == Some(tool);
            let handle = r
                .log_scroll
                .get(&tool)
                .cloned()
                .unwrap_or_else(ScrollHandle::new);
            (tool, r.log(tool), handle, running)
        })
        .collect();
    for (tool, _, handle, _) in &logs {
        cx.global_mut::<Ready>()
            .log_scroll
            .entry(*tool)
            .or_insert_with(|| handle.clone());
    }
    let mut out = vec![
        type_scale::LABEL
            .el()
            .text_color(t.text_secondary)
            .child(text(locale, Key::ReadyTitle))
            .into_any_element(),
        type_scale::SMALL
            .el()
            .mt(px(2.))
            .text_color(t.text_tertiary)
            .child(text(locale, Key::ReadyHint))
            .into_any_element(),
    ];
    let mut list = div().flex().flex_col().mt(px(8.)).gap(px(8.));
    for view in rows {
        let log = view
            .log
            .and_then(|tool| logs.iter().find(|(t, ..)| *t == tool).cloned());
        list = list.child(row_element::<T, E>(view, log, locale, t, cx));
    }
    out.push(list.into_any_element());
    if !first_run {
        let disabled = busy || checking;
        out.push(
            div()
                .mt(px(4.))
                .ml(px(-12.))
                .flex()
                .flex_row()
                .child(
                    ui::button(
                        "ready-check-again",
                        text(locale, Key::ReadyCheckAgain),
                        ui::Variant::Ghost,
                        disabled,
                        t,
                    )
                    .child(icon(
                        Icon::RotateCw,
                        14.,
                        if disabled {
                            t.text_disabled
                        } else {
                            t.text_tertiary
                        },
                    ))
                    .when(!disabled, |this| this.on_click(|_, _, cx| check(cx))),
                )
                .into_any_element(),
        );
    }
    out
}

fn row_element<T: EventEmitter<E> + 'static, E: From<Request> + 'static>(
    view: RowView,
    log: Option<(Tool, Vec<String>, ScrollHandle, bool)>,
    locale: Locale,
    t: &Tokens,
    cx: &mut Context<T>,
) -> AnyElement {
    let id = row_id(view.row);
    let mut buttons = div().flex().flex_row().items_center().gap(px(4.));
    for (i, b) in view.buttons.iter().enumerate() {
        let label = b.label;
        let disabled = b.disabled.is_some();
        let tip = b.disabled.or(b.tip).map(|tip| tip_text(locale, tip));
        let act_of = b.act.clone();
        let mut button = ui::button(
            (id, i),
            label_text(locale, label),
            variant(b.variant),
            disabled,
            t,
        );
        if b.label == Label::Download {
            button = button.child(icon(Icon::ExternalLink, 14., t.text_tertiary));
        }
        buttons = buttons.child(
            button
                .when_some(tip, |this, tip| this.tooltip(ui::tip(tip)))
                .when(!disabled, |this| {
                    this.on_click(cx.listener(move |_, _, _, cx| act::<T, E>(&act_of, cx)))
                }),
        );
    }
    let state = view.state.as_ref().map(|s| {
        type_scale::SMALL
            .el()
            .id((id, 100usize))
            .font_features(type_scale::tabular())
            .text_color(tone(view.state_tone, t))
            .child(state_text(locale, s))
            .when_some(view.state_tip, |this, tip| {
                this.tooltip(ui::tip(tip_text(locale, tip)))
            })
    });
    let line1 = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .min_h(px(28.))
        .child(icon(glyph(view.glyph), 14., tone(view.tone, t)))
        .child(
            type_scale::UI
                .el()
                .flex_1()
                .min_w_0()
                .text_color(t.text_primary)
                .child(text(locale, name_key(view.row))),
        )
        .children(state)
        .child(buttons);
    let mut under = div().flex().flex_col().pl(px(22.)).child(
        type_scale::SMALL
            .el()
            .max_w(px(520.))
            .text_color(t.text_tertiary)
            .child(text(locale, why_key(view.why))),
    );
    if let Some(reason) = view.reason.and_then(|r| reason_text(locale, r)) {
        under = under.child(
            type_scale::SMALL
                .el()
                .text_color(t.text_secondary)
                .child(reason),
        );
    }
    if let Some(plan) = view.winget_instead {
        let tool = view.row.tool();
        under = under.child(
            div().ml(px(-12.)).flex().flex_row().child(
                ui::button(
                    (id, 50usize),
                    text(locale, Key::ReadyWingetInstead),
                    ui::Variant::Ghost,
                    false,
                    t,
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(E::from(Request::Confirm {
                        tool,
                        plan: plan.clone(),
                        update: false,
                    }))
                })),
            ),
        );
    }
    if let Some((tool, lines, handle, running)) = log {
        under = under.child(log_element(id, tool, lines, handle, running, locale, t));
    }
    div()
        .flex()
        .flex_col()
        .min_h(px(44.))
        .py(px(4.))
        .child(line1)
        .child(under)
        .into_any_element()
}

/// The install log (components-app.md 5, "Log"): read-only, at most 200
/// high, following the end while it runs unless the reader scrolled up.
fn log_element(
    id: &'static str,
    tool: Tool,
    lines: Vec<String>,
    handle: ScrollHandle,
    running: bool,
    locale: Locale,
    t: &Tokens,
) -> AnyElement {
    let at_end = {
        let max = f32::from(handle.max_offset().y);
        let offset = f32::from(handle.offset().y);
        max <= 0. || (offset + max).abs() < 4.
    };
    if running && at_end {
        handle.scroll_to_bottom();
    }
    let copy = lines.join("\n");
    div()
        .flex()
        .flex_col()
        .mt(px(8.))
        .child(
            div()
                .id((id, 200usize))
                .max_h(px(200.))
                .overflow_y_scroll()
                .track_scroll(&handle)
                .p(px(8.))
                .rounded(px(RADIUS_SM))
                .bg(t.bg_sunken)
                .border(px(1.))
                .border_color(t.border_subtle)
                .children(lines.into_iter().map(|line| {
                    type_scale::CODE
                        .el()
                        .min_h(px(type_scale::CODE.line))
                        .text_color(t.text_secondary)
                        .child(line)
                })),
        )
        .child(
            div()
                .mt(px(4.))
                .ml(px(-12.))
                .flex()
                .flex_row()
                .gap(px(4.))
                .child(
                    ui::button(
                        (id, 201usize),
                        text(locale, Key::ReadyCopyLog),
                        ui::Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                    }),
                )
                .child(
                    ui::button(
                        (id, 202usize),
                        text(locale, Key::ReadyHideLog),
                        ui::Variant::Ghost,
                        false,
                        t,
                    )
                    .on_click(move |_, _, cx| {
                        cx.global_mut::<Ready>().logs_open.remove(&tool);
                        cx.refresh_windows();
                    }),
                ),
        )
        .into_any_element()
}

/// The install confirmation (components-app.md 5, "Confirmation dialog"):
/// its title, its body (one sentence each, then the exact command with
/// Copy) and its confirming label, `None` for a command shown only. How
/// winget checks what it downloads and the agreements its flags accept
/// are behind "Details" (`details_open`, toggled by `toggle`).
#[allow(clippy::too_many_arguments)]
pub fn dialog_parts(
    tool: Tool,
    plan: &Plan,
    update: bool,
    platform: &Platform,
    details_open: bool,
    toggle: impl Fn(&mut gpui_kit::Window, &mut App) + 'static,
    locale: Locale,
    t: &Tokens,
) -> (String, AnyElement, Option<String>) {
    use acta_core::prereqs::Source;
    use model::{Confirm, ScopeText, Uninstall};
    let d = model::dialog(tool, plan, update, platform);
    let name = tool.display_name();
    let named = |key: Key| tr(locale, key, &[("name", Arg::Text(name))]);
    let title = named(if update {
        Key::ReadyDialogUpdate
    } else {
        Key::ReadyDialogInstall
    });
    let mut sentences: Vec<String> = Vec::new();
    let mut details: Vec<String> = Vec::new();
    if let Some(from) = d.from {
        let to = if d.from_in_details {
            &mut details
        } else {
            &mut sentences
        };
        to.push(match from {
            Source::Vendor => named(Key::ReadyFromVendor),
            Source::Winget => named(Key::ReadyFromWinget),
            Source::Homebrew => named(Key::ReadyFromHomebrew),
            Source::Apple => text(locale, Key::ReadyFromApple),
            Source::Distribution | Source::Npm => text(locale, Key::ReadyFromSystem),
        });
    }
    if let Some(scope) = d.scope {
        sentences.push(text(
            locale,
            match scope {
                ScopeText::User => Key::ReadyScopeUser,
                ScopeText::UserMayAsk => Key::ReadyScopeUserUac,
                ScopeText::System => Key::ReadyScopeSystem,
            },
        ));
    }
    if d.agreements {
        details.push(text(locale, Key::ReadyAgreements));
    }
    if let Some(updates) = d.updates {
        sentences.push(text(
            locale,
            match updates {
                Source::Vendor => Key::ReadyUpdatesSelf,
                Source::Winget => Key::ReadyUpdatesWinget,
                Source::Homebrew => Key::ReadyUpdatesHomebrew,
                Source::Apple => Key::ReadyUpdatesApple,
                Source::Distribution | Source::Npm => Key::ReadyUpdatesSystem,
            },
        ));
    }
    if let Some(uninstall) = d.uninstall {
        let manager = match uninstall {
            Uninstall::Native => text(locale, Key::ReadyUninstallNative),
            Uninstall::Manager(m) => m.to_owned(),
        };
        sentences.push(tr(
            locale,
            Key::ReadyUninstall,
            &[("manager", Arg::Text(&manager))],
        ));
    }
    let command = d.command.clone();
    let body = div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .children(sentences.into_iter().map(|s| {
            type_scale::READING
                .el()
                .text_color(t.text_secondary)
                .child(s)
        }))
        .when(!details.is_empty(), |this| {
            this.child(
                div().ml(px(-12.)).flex().flex_row().child(
                    ui::button(
                        "ready-dialog-details",
                        text(locale, Key::CommonDetails),
                        ui::Variant::Ghost,
                        false,
                        t,
                    )
                    .child(icon(
                        if details_open {
                            Icon::ChevronUp
                        } else {
                            Icon::ChevronDown
                        },
                        14.,
                        t.text_tertiary,
                    ))
                    .on_click(move |_, window, cx| toggle(window, cx)),
                ),
            )
            .when(details_open, |this| {
                this.children(
                    details
                        .into_iter()
                        .map(|s| type_scale::SMALL.el().text_color(t.text_tertiary).child(s)),
                )
            })
        })
        .child(
            type_scale::LABEL
                .el()
                .mt(px(12.))
                .text_color(t.text_secondary)
                .child(text(locale, Key::ReadyDialogCommand)),
        )
        .child(
            div()
                .p(px(8.))
                .rounded(px(RADIUS_SM))
                .bg(t.bg_sunken)
                .border(px(1.))
                .border_color(t.border_subtle)
                // Wrapped between words only, never inside an option.
                .child(div().flex().flex_row().flex_wrap().gap_x(px(8.)).children(
                    model::command_words(plan).into_iter().map(|word| {
                        let long = word.chars().count() > 40;
                        type_scale::CODE
                            .el()
                            .max_w_full()
                            .when(!long, |this| this.whitespace_nowrap())
                            .text_color(t.text_primary)
                            .child(word)
                    }),
                )),
        )
        .child(
            div().ml(px(-12.)).flex().flex_row().child(
                ui::button(
                    "ready-copy-command",
                    text(locale, Key::ReadyCopyCommand),
                    ui::Variant::Ghost,
                    false,
                    t,
                )
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(command.clone()))
                }),
            ),
        )
        .into_any_element();
    let confirm = d.confirm.map(|c| {
        text(
            locale,
            match c {
                Confirm::Install => Key::ReadyDialogConfirmInstall,
                Confirm::Update => Key::ReadyDialogConfirmUpdate,
                Confirm::OpenInTerminal => Key::ReadyDialogConfirmTerminal,
            },
        )
    });
    (title, body, confirm)
}

/// Development states for captures (`ACTA_DEV_READY`), debug builds only.
/// Each fixes the detection, the platform (Windows with winget unless
/// said) and what the app is doing; nothing is detected or installed.
#[cfg(debug_assertions)]
pub mod dev {
    use super::*;
    use acta_core::prereqs::{AuthMethod, Failure, Kind, Version};

    pub struct Scenario {
        pub claude: ToolRow,
        pub sign_in: SignIn,
        pub git: ToolRow,
        pub gh: ToolRow,
        pub platform: Platform,
        pub github: bool,
        pub activity: HashMap<Row, Activity>,
        pub logs: HashMap<Tool, Vec<String>>,
        pub open: Vec<Tool>,
    }

    fn row(tool: Tool, state: State, kind: Kind, v: Option<Version>) -> ToolRow {
        ToolRow {
            tool,
            state,
            path: (state != State::Missing).then(|| PathBuf::from("dev")),
            version: v,
            kind,
            git_bash: None,
            reason: None,
        }
    }

    fn found() -> Scenario {
        Scenario {
            claude: row(
                Tool::Claude,
                State::Ok,
                Kind::Native,
                Some(Version::new(2, 1, 330)),
            ),
            sign_in: SignIn::SignedIn(AuthMethod::ClaudeAi),
            git: ToolRow {
                git_bash: Some(true),
                ..row(
                    Tool::Git,
                    State::Ok,
                    Kind::Winget,
                    Some(Version::new(2, 56, 1)),
                )
            },
            gh: row(
                Tool::Gh,
                State::Ok,
                Kind::Winget,
                Some(Version::new(2, 105, 0)),
            ),
            platform: Platform {
                os: Os::CURRENT,
                distro: None,
                winget: true,
                brew: false,
                distro_manager: false,
            },
            github: true,
            activity: HashMap::new(),
            logs: HashMap::new(),
            open: Vec::new(),
        }
    }

    fn missing(tool: Tool) -> ToolRow {
        row(tool, State::Missing, Kind::Unknown, None)
    }

    fn gh_log() -> Vec<String> {
        [
            "Found GitHub CLI [GitHub.cli] Version 2.105.0",
            "This application is licensed to you by its owner.",
            "Microsoft is not responsible for, nor does it grant any licenses to, third-party packages.",
            "Downloading https://github.com/cli/cli/releases/download/v2.105.0/gh_2.105.0_windows_amd64.zip",
            "  14.2 MB / 14.2 MB",
            "Successfully verified installer hash",
            "Extracting archive...",
            "An unexpected error occurred while executing the command:",
            "0x80070005 : Access is denied.",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
    }

    fn git_log() -> Vec<String> {
        [
            "Found Git [Git.Git] Version 2.56.1",
            "This application is licensed to you by its owner.",
            "Downloading https://github.com/git-for-windows/git/releases/download/v2.56.1.windows.1/Git-2.56.1-64-bit.exe",
            "  68.3 MB / 68.3 MB",
            "Successfully verified installer hash",
            "Starting package install...",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
    }

    /// The state named `name`.
    pub fn scenario(name: &str) -> Option<Scenario> {
        let mut s = found();
        match name {
            "found" => {}
            // First run with Claude Code missing uses this one too.
            "missing" => {
                s.claude = missing(Tool::Claude);
                s.sign_in = SignIn::NoClaude;
                s.git = missing(Tool::Git);
                s.gh = missing(Tool::Gh);
            }
            "outdated" => {
                s.claude = row(
                    Tool::Claude,
                    State::Outdated,
                    Kind::Native,
                    Some(Version::new(2, 1, 100)),
                );
                s.git = ToolRow {
                    git_bash: Some(false),
                    ..row(
                        Tool::Git,
                        State::Outdated,
                        Kind::Winget,
                        Some(Version::new(2, 20, 1)),
                    )
                };
                s.gh = row(
                    Tool::Gh,
                    State::Outdated,
                    Kind::Unknown,
                    Some(Version::new(2, 40, 0)),
                );
            }
            "installing" => {
                s.sign_in = SignIn::SignedOut;
                s.git = missing(Tool::Git);
                s.gh = missing(Tool::Gh);
                s.activity.insert(
                    Row::Git,
                    Activity::Running {
                        update: false,
                        elapsed: Duration::from_secs(42),
                    },
                );
                s.logs.insert(Tool::Git, git_log());
                s.open.push(Tool::Git);
            }
            // screens.md 8.26.
            "failed" => {
                s.sign_in = SignIn::SignedOut;
                s.git = missing(Tool::Git);
                s.gh = missing(Tool::Gh);
                s.activity
                    .insert(Row::Gh, Activity::Failed(Failure::Exit(Some(1))));
                s.logs.insert(Tool::Gh, gh_log());
                s.open.push(Tool::Gh);
            }
            "failed-claude" => {
                s.claude = missing(Tool::Claude);
                s.sign_in = SignIn::NoClaude;
                s.activity
                    .insert(Row::Claude, Activity::Failed(Failure::Exit(Some(1603))));
                s.logs.insert(
                    Tool::Claude,
                    vec![
                        "Setting up Claude Code...".to_owned(),
                        "Installation failed: the download was interrupted.".to_owned(),
                    ],
                );
            }
            "terminal" => {
                s.sign_in = SignIn::SignedOut;
                s.activity.insert(Row::SignIn, Activity::Terminal);
            }
            "nomanager" => {
                s.platform.winget = false;
                s.git = missing(Tool::Git);
                s.gh = missing(Tool::Gh);
            }
            "signedout" => s.sign_in = SignIn::SignedOut,
            "apikey" => s.sign_in = SignIn::SignedIn(AuthMethod::ApiKey),
            _ => return None,
        }
        Some(s)
    }

    pub fn from_env() -> Option<Scenario> {
        scenario(&std::env::var("ACTA_DEV_READY").ok()?)
    }

    impl Ready {
        /// The scenario's rows, as if detected.
        pub(super) fn apply_dev(&mut self) {
            let Some(s) = &self.dev else { return };
            self.claude = Some(s.claude.clone());
            self.sign_in = Some(s.sign_in);
            self.git = Some(s.git.clone());
            self.gh = Some(s.gh.clone());
            self.platform = s.platform;
            self.github = s.github;
            self.logs_open = s.open.iter().copied().collect();
        }
    }
}
