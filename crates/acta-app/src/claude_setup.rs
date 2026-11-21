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

/// The four checks the Settings page lists, from the steps of
/// `acta setup --check` (ADR 0005).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Check {
    /// The `claude` command runs ("Claude Code found").
    pub claude: bool,
    /// The plugin files are written and the `acta` command exists.
    pub files: bool,
    /// The marketplace is added and the plugin installed.
    pub plugin: bool,
    /// The permission rule is in `~/.claude/settings.json`.
    pub rule: bool,
}

impl Check {
    pub fn complete(self) -> bool {
        self.claude && self.files && self.plugin && self.rule
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
            claude: ok("claude"),
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

/// `acta setup --check --json`. `None` when the command could not run or
/// answered something else; the status bar then reports "not set up".
pub fn check() -> Option<Check> {
    Check::parse(&run(&["setup", "--check", "--json"], CHECK_LIMIT)?)
}

/// "Set up Claude Code" / "Run setup again": `acta setup`, then every
/// attached folder through `acta link` (idempotent: it never reorders, and
/// it turns Claude Code's own memory off there, ADR 0005), then the check.
/// One button makes Claude Code ready everywhere Acta knows a folder.
pub fn install(folders: Vec<LinkTarget>) -> Option<Check> {
    run(&["setup", "--json"], SETUP_LIMIT)?;
    for f in folders {
        let path = f.path.to_string_lossy().into_owned();
        if f.path.is_dir() {
            let _ = run(&["link", &path, f.level, &f.owner, "--add"], CHECK_LIMIT);
        }
    }
    check()
}

/// An attached folder and its owner, as `acta link` takes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkTarget {
    pub path: PathBuf,
    /// `--project`, `--epic` or `--topic`.
    pub level: &'static str,
    pub owner: String,
}

/// Every attached folder of every project, for [`install`].
pub fn link_targets(store: &acta_core::Store) -> acta_core::Result<Vec<LinkTarget>> {
    use acta_core::model::Owner;
    Ok(store
        .all_folders()?
        .into_iter()
        .map(|f| {
            let level = match f.owner {
                Owner::Project(_) => "--project",
                Owner::Epic(_) => "--epic",
                Owner::Topic(_) => "--topic",
            };
            LinkTarget {
                path: PathBuf::from(&f.path),
                level,
                owner: f.owner.to_string(),
            }
        })
        .collect())
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
    NotSetUp,
    NotFound,
    /// Set up, but the active project has no working folder.
    NoFolder,
    Ready,
}

/// The status from the last check and whether the active project has a
/// folder (`None`: no project, or not known yet; setup alone decides).
pub fn status(check: Option<Check>, project_has_folder: Option<bool>) -> Status {
    match check {
        None => Status::NotSetUp,
        Some(c) if !c.claude => Status::NotFound,
        Some(c) if !c.complete() => Status::NotSetUp,
        Some(_) if project_has_folder == Some(false) => Status::NoFolder,
        Some(_) => Status::Ready,
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

/// "Set up Claude Code" / "Run setup again".
pub fn run_install(cx: &mut gpui_kit::App) {
    let db = cx.try_global::<SetupDb>().map(|d| d.0.clone());
    start(
        cx,
        move || {
            let targets = db
                .and_then(|db| link_targets(&db.lock()).ok())
                .unwrap_or_default();
            install(targets)
        },
        true,
    );
}

/// "Remove from Claude Code".
pub fn run_remove(cx: &mut gpui_kit::App) {
    start(cx, remove, true);
}

/// The store, for the folders setup links.
struct SetupDb(crate::db::Db);

impl gpui_kit::Global for SetupDb {}

/// Called once the store is open.
pub fn set_db(db: crate::db::Db, cx: &mut gpui_kit::App) {
    cx.set_global(SetupDb(db));
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
    fn the_four_checks_come_from_the_steps_of_the_cli() {
        let all = r#"{"ok":true,"steps":[
            {"step":"claude","ok":true,"detail":"found"},
            {"step":"executable","ok":true,"detail":"found"},
            {"step":"plugin files","ok":true,"detail":"up to date"},
            {"step":"marketplace","ok":true,"detail":"added"},
            {"step":"plugin","ok":true,"detail":"installed"},
            {"step":"allow rule","ok":true,"detail":"present"},
            {"step":"link index","ok":true,"detail":"up to date"}]}"#;
        let check = Check::parse(all).unwrap();
        assert!(check.complete());
        let partial = r#"{"ok":false,"steps":[
            {"step":"claude","ok":true,"detail":"found"},
            {"step":"executable","ok":true,"detail":"found"},
            {"step":"plugin files","ok":false,"detail":"missing"},
            {"step":"marketplace","ok":true,"detail":"added"},
            {"step":"plugin","ok":false,"detail":"not installed"},
            {"step":"allow rule","ok":true,"detail":"present"}]}"#;
        assert_eq!(
            Check::parse(partial).unwrap(),
            Check {
                claude: true,
                files: false,
                plugin: false,
                rule: true
            }
        );
        assert_eq!(Check::parse("not json"), None);
    }

    #[test]
    fn a_focus_check_waits_30_seconds_after_the_last() {
        assert!(!stale(Duration::from_secs(5)));
        assert!(stale(Duration::from_secs(30)));
    }

    #[test]
    fn the_status_reports_setup_first_then_the_project_folder() {
        let done = Check {
            claude: true,
            files: true,
            plugin: true,
            rule: true,
        };
        let missing = Check {
            claude: false,
            ..done
        };
        assert_eq!(status(None, Some(true)), Status::NotSetUp);
        assert_eq!(status(Some(missing), Some(true)), Status::NotFound);
        assert_eq!(
            status(
                Some(Check {
                    rule: false,
                    ..done
                }),
                Some(true)
            ),
            Status::NotSetUp
        );
        assert_eq!(status(Some(done), Some(false)), Status::NoFolder);
        assert_eq!(status(Some(done), Some(true)), Status::Ready);
        assert_eq!(status(Some(done), None), Status::Ready);
    }
}
