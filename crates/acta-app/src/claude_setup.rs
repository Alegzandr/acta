//! The Claude Code setup as the app sees it (docs/design/layout.md 4.6,
//! components-app.md 5 Settings, data.md 11 request 4): the structured
//! result of `acta setup --check --json`, and setup and removal, run
//! through the exported `acta` command so the app and the terminal share
//! one implementation. Every call here blocks: callers run it on a
//! background executor.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

/// The three checks of Acta's own setup the Settings page lists, from the
/// steps of `acta setup --check --no-prereqs` (ADR 0005). Whether Claude
/// Code is there is the Ready for Claude Code block's (`ready.rs`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Check {
    /// The plugin files are written and the `acta` command exists.
    pub files: bool,
    /// The marketplace is added and the plugin installed.
    pub plugin: bool,
    /// The permission rule is in `~/.claude/settings.json`.
    pub rule: bool,
}

impl Check {
    pub fn complete(self) -> bool {
        self.files && self.plugin && self.rule
    }

    /// Parse the JSON of `acta setup --check --json`. Unknown steps are
    /// ignored; a missing step counts as not done.
    pub fn parse(json: &str) -> Option<Check> {
        #[derive(Deserialize)]
        struct Out {
            steps: Vec<Step>,
        }
        #[derive(Deserialize)]
        struct Step {
            step: String,
            ok: bool,
        }
        let out: Out = serde_json::from_str(json.trim()).ok()?;
        let ok = |name: &str| out.steps.iter().any(|s| s.step == name && s.ok);
        Some(Check {
            files: ok("executable") && ok("plugin files"),
            plugin: ok("marketplace") && ok("plugin"),
            rule: ok("allow rule"),
        })
    }
}

/// The `acta` command: the exported copy (ADR 0004), else the one in the
/// package next to this executable (development builds).
pub fn cli_path() -> Option<PathBuf> {
    let layout = crate::cli_export::Layout::locate()?;
    let exported = layout.exported_exe();
    if exported.is_file() {
        return Some(exported);
    }
    layout.source.is_file().then_some(layout.source)
}

/// How long a check may take, and setup or removal (which install and
/// uninstall a plugin through `claude`).
const CHECK_LIMIT: Duration = Duration::from_secs(30);
const SETUP_LIMIT: Duration = Duration::from_secs(120);

/// Run `acta` with `args`, without a console window; `None` when it could
/// not be started or ran past `limit` (it is then killed). Its output is
/// returned whatever the exit status.
fn run(args: &[&str], limit: Duration) -> Option<String> {
    let mut command = Command::new(cli_path()?);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // CREATE_NO_WINDOW: no console flashes up, and none takes focus.
        command.creation_flags(0x0800_0000);
    }
    let out = acta_core::process::output_within(&mut command, limit).ok()??;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `acta setup --check --no-prereqs --json`: Acta's own steps only (the
/// Ready rows are detected in the app, `ready.rs`, and run each tool).
/// `None` when the command could not run or answered something else; the
/// status bar then reports "not set up".
pub fn check() -> Option<Check> {
    Check::parse(&run(
        &["setup", "--check", "--no-prereqs", "--json"],
        CHECK_LIMIT,
    )?)
}

/// "Set up Claude Code" / "Run setup again": `acta setup`, then the open
/// project's attached folders through `acta link --attached` (read from
/// the store: no path on a command line; idempotent, never reorders; turns
/// Claude Code's own memory off there, ADR 0005), then the check.
pub fn install(project: Option<String>) -> Option<Check> {
    run(&["setup", "--json"], SETUP_LIMIT)?;
    if let Some(p) = project {
        let _ = run(
            &["link", "--attached", "--project", &p, "--add"],
            CHECK_LIMIT,
        );
    }
    check()
}

/// `acta setup --files-only`: the plugin tree of this build, which the
/// terminal loads with `--plugin-dir` (ADR 0008 amendment). Nothing in
/// Claude Code changes. Run at launch, so an update refreshes it at once.
pub fn write_plugin_files() -> bool {
    run(&["setup", "--files-only", "--json"], CHECK_LIMIT)
        .is_some_and(|out| out.contains("\"ok\":true"))
}

/// Where `acta setup` writes the plugin: `<data dir>/claude/plugins/acta`.
pub fn plugin_dir() -> Option<PathBuf> {
    Some(
        crate::db::config()?
            .data_dir
            .join("claude")
            .join("plugins")
            .join("acta"),
    )
}

/// "Remove from Claude Code": `acta setup --remove`, then the check.
pub fn remove() -> Option<Check> {
    run(&["setup", "--remove", "--json"], SETUP_LIMIT)?;
    check()
}

/// The Claude Code status of the status bar (layout.md 4.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Claude Code is not installed (components-app.md 5, Ready).
    NotInstalled,
    /// Claude Code is not signed in.
    SignedOut,
    NotSetUp,
    /// Set up, but the active project has no working folder.
    NoFolder,
    Ready,
}

/// The status from the required Ready rows (first, before any setup
/// status), the last setup check and whether the active project has a
/// folder (`None`: no project, or not known yet; setup alone decides).
pub fn status(
    ready: crate::ready_model::Readiness,
    check: Option<Check>,
    project_has_folder: Option<bool>,
) -> Status {
    use crate::ready_model::Readiness;
    match (ready, check) {
        (Readiness::Missing, _) => Status::NotInstalled,
        (Readiness::SignedOut, _) => Status::SignedOut,
        (_, None) => Status::NotSetUp,
        (_, Some(c)) if !c.complete() => Status::NotSetUp,
        (_, Some(_)) if project_has_folder == Some(false) => Status::NoFolder,
        (_, Some(_)) => Status::Ready,
    }
}

/// The last check, shared by the status bar and the Settings page.
#[derive(Default)]
pub struct SetupState {
    /// `None` until the first check answers, or when it could not run.
    pub check: Option<Check>,
    /// The first check has answered (until then nothing is claimed).
    pub checked: bool,
    /// A check, setup or removal is running.
    pub busy: bool,
    /// When the last one finished.
    pub finished_at: Option<std::time::Instant>,
}

impl gpui_kit::Global for SetupState {}

pub fn state(cx: &gpui_kit::App) -> Option<&SetupState> {
    cx.try_global::<SetupState>()
}

/// Check again off the UI thread (at launch, after setup, when the window
/// regains focus); a check already running is not repeated.
pub fn refresh(cx: &mut gpui_kit::App) {
    start(cx, check, false);
}

/// The window regained focus: check again unless the last check finished
/// less than 30 s ago (focus moves often; each check runs `claude`).
pub fn refresh_if_stale(cx: &mut gpui_kit::App) {
    let recent = state(cx)
        .and_then(|s| s.finished_at)
        .is_some_and(|at| !stale(at.elapsed()));
    if !recent {
        refresh(cx);
    }
}

/// Whether a check that finished `ago` is old enough to repeat on focus.
pub fn stale(ago: Duration) -> bool {
    ago >= FOCUS_THROTTLE
}

const FOCUS_THROTTLE: Duration = Duration::from_secs(30);

/// "Set up Claude Code" / "Run setup again", confirmed: `project` is the
/// open project, whose folders setup links.
pub fn run_install(project: Option<acta_core::model::ProjectId>, cx: &mut gpui_kit::App) {
    let project = project.map(|p| p.to_string());
    start(cx, move || install(project), true);
}

/// "Remove from Claude Code".
pub fn run_remove(cx: &mut gpui_kit::App) {
    start(cx, remove, true);
}

fn start(
    cx: &mut gpui_kit::App,
    work: impl FnOnce() -> Option<Check> + Send + 'static,
    writes: bool,
) {
    let state = cx.default_global::<SetupState>();
    if state.busy && !writes {
        return;
    }
    state.busy = true;
    cx.refresh_windows();
    let task = cx.background_executor().spawn(async move { work() });
    cx.spawn(async move |cx| {
        let check = task.await;
        cx.update(|cx| {
            let state = cx.default_global::<SetupState>();
            state.check = check;
            state.checked = true;
            state.busy = false;
            state.finished_at = Some(std::time::Instant::now());
            cx.refresh_windows();
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_checks_come_from_the_steps_of_the_cli() {
        let all = r#"{"ok":true,"steps":[
            {"step":"executable","ok":true,"detail":"found"},
            {"step":"plugin files","ok":true,"detail":"up to date"},
            {"step":"marketplace","ok":true,"detail":"added"},
            {"step":"plugin","ok":true,"detail":"installed"},
            {"step":"allow rule","ok":true,"detail":"present"},
            {"step":"link index","ok":true,"detail":"up to date"}]}"#;
        let check = Check::parse(all).unwrap();
        assert!(check.complete());
        let partial = r#"{"ok":false,"steps":[
            {"step":"executable","ok":true,"detail":"found"},
            {"step":"plugin files","ok":false,"detail":"missing"},
            {"step":"marketplace","ok":true,"detail":"added"},
            {"step":"plugin","ok":false,"detail":"not installed"},
            {"step":"allow rule","ok":true,"detail":"present"}]}"#;
        assert_eq!(
            Check::parse(partial).unwrap(),
            Check {
                files: false,
                plugin: false,
                rule: true
            }
        );
        assert_eq!(Check::parse("not json"), None);
        // Rows of a full check (ADR 0013) change nothing here: Claude
        // Code's presence is the Ready block's.
        let with_rows = r#"{"ok":false,"steps":[
            {"step":"claude","ok":false,"detail":"not installed","state":"missing","found":false},
            {"step":"executable","ok":true,"detail":"found"},
            {"step":"plugin files","ok":true,"detail":"up to date"},
            {"step":"marketplace","ok":true,"detail":"added"},
            {"step":"plugin","ok":true,"detail":"installed"},
            {"step":"allow rule","ok":true,"detail":"present"}]}"#;
        assert!(Check::parse(with_rows).unwrap().complete());
    }

    #[test]
    fn a_focus_check_waits_30_seconds_after_the_last() {
        assert!(!stale(Duration::from_secs(5)));
        assert!(stale(Duration::from_secs(30)));
    }

    #[test]
    fn the_status_reports_the_required_rows_then_setup_then_the_folder() {
        use crate::ready_model::Readiness;
        let done = Check {
            files: true,
            plugin: true,
            rule: true,
        };
        let r = Readiness::Ready;
        assert_eq!(
            status(Readiness::Missing, Some(done), Some(true)),
            Status::NotInstalled
        );
        assert_eq!(
            status(Readiness::SignedOut, None, Some(true)),
            Status::SignedOut
        );
        // Not known yet: setup alone decides.
        assert_eq!(
            status(Readiness::Unknown, Some(done), Some(true)),
            Status::Ready
        );
        assert_eq!(status(r, None, Some(true)), Status::NotSetUp);
        assert_eq!(
            status(
                r,
                Some(Check {
                    rule: false,
                    ..done
                }),
                Some(true)
            ),
            Status::NotSetUp
        );
        assert_eq!(status(r, Some(done), Some(false)), Status::NoFolder);
        assert_eq!(status(r, Some(done), Some(true)), Status::Ready);
        assert_eq!(status(r, Some(done), None), Status::Ready);
    }
}
