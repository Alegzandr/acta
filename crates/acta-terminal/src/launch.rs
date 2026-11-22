//! What to start in the terminal, and how Claude Code is started (ADR 0002 point 3, ADR 0008).
//!
//! Nothing here touches GPUI. The existence checks and the `PATH` search read the
//! file system, so the app calls [`launch_spec_for_claude`] on a background executor.

use std::fmt;
use std::path::{Path, PathBuf};

/// The resolver moved into `acta-core` (ADR 0013): one search for the
/// terminal, the checklist and `acta setup --check`.
pub use acta_core::prereqs::{
    ClaudeProgram, claude_session_vars, resolve_claude, resolve_unix, resolve_windows,
};

/// A program to run in the terminal. `env` is added to the inherited
/// environment; the session adds `TERM`, `COLORTERM` and `TERM_PROGRAM` itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// Windows only: `args` are already quoted for the command line and are
    /// joined as they are, not escaped (the `cmd /s /c` shim path).
    pub verbatim_args: bool,
}

/// What the terminal was opened from (ADR 0005 ids). Each level is set as
/// `ACTA_PROJECT`, `ACTA_EPIC`, `ACTA_TOPIC`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Focus {
    pub project: String,
    pub epic: Option<String>,
    pub topic: Option<String>,
}

/// Which conversation a launch opens. Claude Code's `--session-id` and
/// `--resume` take the id (a UUID); Acta keeps it per page, opaque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conversation {
    /// A new conversation under this id (`--session-id`).
    New(String),
    /// The page's last conversation (`--resume`).
    Resume(String),
}

/// What Acta adds to a launch besides folders and focus (ADR 0008
/// amendment, 2026-11-21). Everything else (model, permission mode,
/// settings) is the user's own Claude Code configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClaudeOptions {
    /// Acta's plugin directory, loaded with `--plugin-dir` so a session
    /// started in Acta has Acta's tools and hooks whether or not setup ran.
    /// A same-named installed plugin is replaced for that session.
    pub plugin_dir: Option<PathBuf>,
    pub conversation: Option<Conversation>,
    /// `ACTA_LAUNCH`: an opaque token naming this terminal, so the app can
    /// match the session's presence row to it.
    pub token: Option<String>,
    /// A first prompt Claude Code starts on (`/acta:triage`), passed as its
    /// positional argument, last. Left out on the `cmd` shim path when
    /// `cmd.exe` would interpret it.
    pub prompt: Option<String>,
}

/// A launch of Claude Code, with the additional folders that were left out
/// (ADR 0008: reported, never blocking): missing ones, and on the Windows
/// `.cmd` shim path those whose name `cmd.exe` would interpret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaudeLaunch {
    pub spec: LaunchSpec,
    pub left_out: Vec<PathBuf>,
}

/// Why Claude Code cannot be started. Each maps to a state of the terminal
/// pane in `docs/design/components-folders.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchError {
    /// No working folder anywhere up the chain: the pane asks for one.
    NoFolder,
    /// The primary folder is not on this computer: the pane offers Locate.
    PrimaryMissing(PathBuf),
    /// The `claude` command was not found.
    ClaudeNotFound,
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::NoFolder => f.write_str("no working folder"),
            LaunchError::PrimaryMissing(path) => {
                write!(f, "primary folder not found: {}", path.display())
            }
            LaunchError::ClaudeNotFound => f.write_str("the claude command was not found"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// `claude` with the effective folders of the page it was opened from:
/// working directory = the first folder, `--add-dir` for each other folder
/// that exists, and the focus in `ACTA_*` variables.
pub fn launch_spec_for_claude(
    folders: &[PathBuf],
    focus: &Focus,
    options: &ClaudeOptions,
) -> Result<ClaudeLaunch, LaunchError> {
    // Check the folders first: a missing folder must be reported even when
    // the search for `claude` would also fail.
    if folders.is_empty() {
        return Err(LaunchError::NoFolder);
    }
    let program = resolve_claude().ok_or(LaunchError::ClaudeNotFound)?;
    build_claude_launch(&program, folders, focus, options, |path| path.is_dir())
}

/// The pure part of [`launch_spec_for_claude`], with the program already
/// resolved and the existence check injected.
pub fn build_claude_launch(
    program: &ClaudeProgram,
    folders: &[PathBuf],
    focus: &Focus,
    options: &ClaudeOptions,
    exists: impl Fn(&Path) -> bool,
) -> Result<ClaudeLaunch, LaunchError> {
    let (primary, additional) = folders.split_first().ok_or(LaunchError::NoFolder)?;
    if !exists(primary) {
        return Err(LaunchError::PrimaryMissing(primary.clone()));
    }
    let mut claude_args = Vec::new();
    let mut left_out = Vec::new();
    for folder in additional {
        let unsafe_for_cmd = matches!(program, ClaudeProgram::CmdShim { .. })
            && has_cmd_metachar(&folder.to_string_lossy());
        if exists(folder) && !unsafe_for_cmd {
            claude_args.push("--add-dir".to_owned());
            claude_args.push(folder.to_string_lossy().into_owned());
        } else {
            left_out.push(folder.clone());
        }
    }
    let shim = matches!(program, ClaudeProgram::CmdShim { .. });
    if let Some(dir) = &options.plugin_dir
        && exists(dir)
        && !(shim && has_cmd_metachar(&dir.to_string_lossy()))
    {
        claude_args.push("--plugin-dir".to_owned());
        claude_args.push(dir.to_string_lossy().into_owned());
    }
    match &options.conversation {
        Some(Conversation::New(id)) if is_uuid(id) => {
            claude_args.push("--session-id".to_owned());
            claude_args.push(id.clone());
        }
        Some(Conversation::Resume(id)) if is_uuid(id) => {
            claude_args.push("--resume".to_owned());
            claude_args.push(id.clone());
        }
        _ => {}
    }
    if let Some(prompt) = options.prompt.as_deref().filter(|p| !p.is_empty())
        && !(shim && has_cmd_metachar(prompt))
    {
        claude_args.push(prompt.to_owned());
    }

    let mut env = vec![
        ("ACTA_TERMINAL".to_owned(), "1".to_owned()),
        ("ACTA_PROJECT".to_owned(), focus.project.clone()),
    ];
    if let Some(token) = &options.token {
        env.push(("ACTA_LAUNCH".to_owned(), token.clone()));
    }
    if let Some(epic) = &focus.epic {
        env.push(("ACTA_EPIC".to_owned(), epic.clone()));
    }
    if let Some(topic) = &focus.topic {
        env.push(("ACTA_TOPIC".to_owned(), topic.clone()));
    }

    let mut verbatim_args = false;
    let (program, args) = match program {
        ClaudeProgram::Exe(path) => (path.to_string_lossy().into_owned(), claude_args),
        ClaudeProgram::CmdShim { comspec, shim } => {
            // `cmd /s /c "<line>"` strips the outer quotes and runs <line>,
            // where every word is quoted. cmd has no escape for `"` or `%`
            // inside quotes, so a shim path with metacharacters is refused.
            let shim = shim.to_string_lossy();
            if has_cmd_metachar(&shim) {
                return Err(LaunchError::ClaudeNotFound);
            }
            let words: Vec<String> = std::iter::once(shim.into_owned())
                .chain(claude_args)
                .map(|word| quote_word(&word))
                .collect();
            verbatim_args = true;
            let line = format!("\"{}\"", words.join(" "));
            (
                comspec.clone(),
                vec!["/d".to_owned(), "/s".to_owned(), "/c".to_owned(), line],
            )
        }
        ClaudeProgram::LoginShell(shell) => {
            // `exec claude "$@"`: the arguments stay separate words and never
            // pass through the shell's parser. The word after the script is $0.
            let mut args = vec![
                "-l".to_owned(),
                "-i".to_owned(),
                "-c".to_owned(),
                "exec claude \"$@\"".to_owned(),
                "claude".to_owned(),
            ];
            args.extend(claude_args);
            (shell.clone(), args)
        }
    };

    Ok(ClaudeLaunch {
        spec: LaunchSpec {
            program,
            args,
            cwd: primary.clone(),
            env,
            verbatim_args,
        },
        left_out,
    })
}

/// A UUID in its 8-4-4-4-12 hex form: the only form `--session-id` takes,
/// and nothing a shell would interpret.
pub fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Claude Code's configuration directory: `CLAUDE_CONFIG_DIR`, else
/// `~/.claude`.
pub fn claude_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".claude"))
}

/// Claude Code's directory name for a working directory: every character
/// but an ASCII letter or digit becomes `-` (one per UTF-16 unit, as
/// JavaScript counts them). Names over 200 characters are truncated and
/// hashed by Claude Code (code.claude.com/docs/en/sessions, "Where
/// transcripts are stored"); Acta does not reproduce the hash, so `None`.
pub fn project_dir_name(cwd: &Path) -> Option<String> {
    let mut name = String::new();
    for c in cwd.to_string_lossy().chars() {
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                name.push('-');
            }
        }
    }
    (!name.is_empty() && name.len() <= 200).then_some(name)
}

/// Whether conversation `id` belongs to `cwd` in Claude Code's storage:
/// `<config>/projects/<project_dir_name(cwd)>/<id>.jsonl`. Only existence is
/// checked; nothing is read. A transcript elsewhere (the page's folder
/// changed) or gone (transcripts age out after 30 days) means a new
/// conversation: a resume across folders would carry the old folder's
/// conversation into this one, and one of a missing id ends at once.
pub fn transcript_exists(config_dir: &Path, cwd: &Path, id: &str) -> bool {
    if !is_uuid(id) {
        return false;
    }
    let Some(dir) = project_dir_name(cwd) else {
        return false;
    };
    config_dir
        .join("projects")
        .join(dir)
        .join(format!("{id}.jsonl"))
        .is_file()
}

/// Characters `cmd.exe` interprets even inside double quotes, or that end
/// a quoted word.
fn has_cmd_metachar(text: &str) -> bool {
    text.contains(['"', '%', '!', '^', '&', '|', '<', '>'])
}

/// One word in double quotes, for the program behind the shim (node parses
/// with MSVCRT rules): backslashes right before the closing quote are
/// doubled, or `"D:\"` would escape the quote and swallow the next word.
/// The word holds no `"` (checked by [`has_cmd_metachar`]).
fn quote_word(word: &str) -> String {
    let trailing = word.len() - word.trim_end_matches('\\').len();
    format!("\"{word}{}\"", "\\".repeat(trailing))
}

/// The spelling of `PATH` to give a child. Windows names are
/// case-insensitive and its block usually spells it `Path`: the child gets
/// the name the parent's environment uses, so it never holds two spellings
/// (which one a program then reads is up to the program). Elsewhere names
/// are case-sensitive and the variable is `PATH`.
pub fn path_var_name<S: AsRef<str>>(windows: bool, inherited: &[S]) -> String {
    if !windows {
        return "PATH".to_owned();
    }
    inherited
        .iter()
        .map(AsRef::as_ref)
        .find(|n| n.eq_ignore_ascii_case("PATH"))
        .unwrap_or("Path")
        .to_owned()
}

/// Set `PATH` in a launch's `env` to `path` (ADR 0013: the fresh `PATH`,
/// so a tool installed while Acta runs is found by what the terminal
/// starts), replacing any other spelling of it there. A `path` that is
/// not Unicode is left out: the child then inherits Acta's own.
pub fn set_path<S: AsRef<str>>(
    env: &mut Vec<(String, String)>,
    windows: bool,
    inherited: &[S],
    path: &std::ffi::OsStr,
) {
    let Some(value) = path.to_str() else { return };
    let same = |n: &str| {
        if windows {
            n.eq_ignore_ascii_case("PATH")
        } else {
            n == "PATH"
        }
    };
    env.retain(|(name, _)| !same(name));
    env.push((path_var_name(windows, inherited), value.to_owned()));
}

/// The names of this process's environment variables.
pub fn env_names() -> Vec<String> {
    std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .collect()
}

/// A one-off command session (ADR 0013): `argv` started directly in the
/// PTY, with no shell between Acta and the program, in `cwd`, with the
/// fresh `PATH` when there is one. The program is `argv[0]` as given when
/// it is a path, else the first match on `dirs` (the search of
/// `acta_core::prereqs::locate`, with the registry `PATH`, so a manager
/// installed since Acta started is found). `None` when the program is not
/// found, or on Windows when it is a `.cmd` or `.bat` script, which only a
/// shell can run.
pub fn one_off_spec(
    argv: &[std::ffi::OsString],
    cwd: PathBuf,
    dirs: &[PathBuf],
    path: Option<&std::ffi::OsStr>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<LaunchSpec> {
    let (first, rest) = argv.split_first()?;
    let given = Path::new(first);
    let program = if given.is_absolute() {
        is_file(given).then(|| given.to_path_buf())?
    } else {
        let name = first.to_str()?;
        acta_core::prereqs::locate(acta_core::paths::Os::CURRENT, name, dirs, &is_file)?
    };
    let script = program
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if cfg!(windows) && script {
        return None;
    }
    let mut env = Vec::new();
    if let Some(path) = path {
        set_path(&mut env, cfg!(windows), &env_names(), path);
    }
    Some(LaunchSpec {
        program: program.to_string_lossy().into_owned(),
        args: rest
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect(),
        cwd,
        env,
        verbatim_args: false,
    })
}

/// Claude Code on its own, as a user starts it (the checklist's "Sign
/// in...": its own first-run flow signs in): no folders, focus, plugin or
/// conversation of Acta's, in `cwd`.
pub fn plain_claude_spec(program: &ClaudeProgram, cwd: PathBuf) -> Option<LaunchSpec> {
    let launch = build_claude_launch(
        program,
        std::slice::from_ref(&cwd),
        &Focus::default(),
        &ClaudeOptions::default(),
        |_| true,
    )
    .ok()?;
    Some(LaunchSpec {
        env: Vec::new(),
        ..launch.spec
    })
}

/// Remove [`claude_session_vars`] from this process's environment, so
/// every program Acta starts (the terminal's shell and `claude`) begins a
/// session of its own.
///
/// This is done to the process, not to the terminal child's environment,
/// because `alacritty_terminal` can only add to the child's: its
/// `tty::Options::env` is merged over the inherited environment (the
/// ConPTY block appends every inherited variable not overridden, the Unix
/// `Command` never clears), and an empty value is still a set variable.
///
/// # Safety
///
/// Call it first in `main`, before any thread exists: changing the
/// environment while another thread reads it is undefined behaviour.
pub unsafe fn scrub_claude_session() {
    let names: Vec<String> = std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .collect();
    for name in claude_session_vars(&names) {
        // SAFETY: the caller guarantees no other thread runs yet.
        unsafe { std::env::remove_var(name) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_code_session_is_not_passed_on() {
        let names = [
            "PATH",
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_PID",
            "ACTA_PROJECT",
            "ANTHROPIC_MODEL",
        ];
        assert_eq!(
            claude_session_vars(&names),
            [
                "CLAUDECODE",
                "CLAUDE_CODE_ENTRYPOINT",
                "CLAUDE_CODE_CHILD_SESSION",
                "CLAUDE_CODE_SESSION_ID",
                "CLAUDE_PID",
            ]
        );
        // Outside a session, a user's own `CLAUDE_CODE_*` options stay.
        assert!(claude_session_vars(&["PATH", "CLAUDE_CODE_USE_BEDROCK"]).is_empty());
        // Windows names are case-insensitive.
        assert_eq!(
            claude_session_vars(&["ClaudeCode", "claude_code_sse_port"]),
            ["ClaudeCode", "claude_code_sse_port"]
        );
    }

    fn focus() -> Focus {
        Focus {
            project: "P-1".into(),
            epic: Some("E-2".into()),
            topic: Some("T-3".into()),
        }
    }

    fn alexandre(dir: &str) -> PathBuf {
        PathBuf::from("/home/alexandre").join(dir)
    }

    #[test]
    fn primary_is_cwd_and_others_are_add_dirs() {
        let program = ClaudeProgram::Exe(PathBuf::from("/bin/claude"));
        let folders = [alexandre("acta"), alexandre("docs"), alexandre("wt")];
        let launch = build_claude_launch(
            &program,
            &folders,
            &focus(),
            &ClaudeOptions::default(),
            |_| true,
        )
        .unwrap();
        assert_eq!(launch.spec.program, "/bin/claude");
        assert_eq!(launch.spec.cwd, alexandre("acta"));
        let docs = alexandre("docs").to_string_lossy().into_owned();
        let wt = alexandre("wt").to_string_lossy().into_owned();
        assert_eq!(launch.spec.args, ["--add-dir", &docs, "--add-dir", &wt]);
        assert!(launch.left_out.is_empty());
        assert_eq!(
            launch.spec.env,
            [
                ("ACTA_TERMINAL".to_owned(), "1".to_owned()),
                ("ACTA_PROJECT".to_owned(), "P-1".to_owned()),
                ("ACTA_EPIC".to_owned(), "E-2".to_owned()),
                ("ACTA_TOPIC".to_owned(), "T-3".to_owned()),
            ]
        );
    }

    #[test]
    fn options_add_the_plugin_the_conversation_and_the_token() {
        let id = "019a7a6e-1c2d-7f00-8a00-0123456789ab";
        let plugin = alexandre("acta-data/claude/plugins/acta");
        let options = ClaudeOptions {
            plugin_dir: Some(plugin.clone()),
            conversation: Some(Conversation::Resume(id.into())),
            token: Some("tok-1".into()),
            prompt: None,
        };
        let program = ClaudeProgram::Exe(PathBuf::from("claude"));
        let folders = [alexandre("acta")];
        let launch = build_claude_launch(&program, &folders, &focus(), &options, |_| true).unwrap();
        let plugin_s = plugin.to_string_lossy().into_owned();
        assert_eq!(
            launch.spec.args,
            ["--plugin-dir", plugin_s.as_str(), "--resume", id]
        );
        assert!(
            launch
                .spec
                .env
                .contains(&("ACTA_LAUNCH".to_owned(), "tok-1".to_owned()))
        );

        // A new conversation takes --session-id; a missing plugin directory
        // and an id that is not a UUID are left out, never passed.
        let options = ClaudeOptions {
            conversation: Some(Conversation::New(id.into())),
            ..options
        };
        let launch = build_claude_launch(&program, &folders, &focus(), &options, |p| {
            p != plugin.as_path()
        })
        .unwrap();
        assert_eq!(launch.spec.args, ["--session-id", id]);
        let options = ClaudeOptions {
            conversation: Some(Conversation::Resume("x & del".into())),
            plugin_dir: None,
            token: None,
            prompt: None,
        };
        let launch = build_claude_launch(&program, &folders, &focus(), &options, |_| true).unwrap();
        assert!(launch.spec.args.is_empty());
        assert!(!launch.spec.env.iter().any(|(k, _)| k == "ACTA_LAUNCH"));
    }

    #[test]
    fn transcripts_are_found_by_id_in_any_project() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("acta-launch-{}-{n}", std::process::id()));
        let id = "019a7a6e-1c2d-7f00-8a00-0123456789ab";
        let cwd = Path::new("C:\\Users\\alexandre\\acta");
        assert!(!transcript_exists(&dir, cwd, id));
        let project = dir.join("projects").join("C--Users-alexandre-acta");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(format!("{id}.jsonl")), "").unwrap();
        // Only under the directory of this working directory.
        let here = if cfg!(windows) {
            cwd.to_path_buf()
        } else {
            PathBuf::from("C:\\Users\\alexandre\\acta")
        };
        assert!(transcript_exists(&dir, &here, id));
        assert!(!transcript_exists(
            &dir,
            Path::new("C:\\Users\\alexandre\\other"),
            id
        ));
        assert!(!transcript_exists(
            &dir,
            &here,
            "019a7a6e-1c2d-7f00-8a00-0123456789ac"
        ));
        assert!(!transcript_exists(&dir, &here, "../x"));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(is_uuid(id) && !is_uuid("019a7a6e1c2d7f008a000123456789ab"));
    }

    #[test]
    fn project_directory_names_follow_claude_codes_layout() {
        // Seen on disk for a real run (Claude Code 2.1.330, Windows).
        assert_eq!(
            project_dir_name(Path::new(
                "C:\\Users\\Alegzandr\\AppData\\Local\\Temp\\claude\\scratchpad\\ccint\\hl\\proj"
            ))
            .as_deref(),
            Some("C--Users-Alegzandr-AppData-Local-Temp-claude-scratchpad-ccint-hl-proj")
        );
        assert_eq!(
            project_dir_name(Path::new("/home/alexandre/my app.v2")).as_deref(),
            Some("-home-alexandre-my-app-v2")
        );
        assert_eq!(
            project_dir_name(Path::new("/home/\u{e9}t\u{e9}")).as_deref(),
            Some("-home--t-")
        );
        assert_eq!(project_dir_name(&PathBuf::from("a".repeat(201))), None);
    }

    #[test]
    fn project_focus_sets_no_epic_or_topic() {
        let program = ClaudeProgram::Exe(PathBuf::from("claude"));
        let focus = Focus {
            project: "P-1".into(),
            ..Focus::default()
        };
        let launch = build_claude_launch(
            &program,
            &[alexandre("acta")],
            &focus,
            &ClaudeOptions::default(),
            |_| true,
        )
        .unwrap();
        assert!(launch.spec.args.is_empty());
        assert!(
            !launch
                .spec
                .env
                .iter()
                .any(|(k, _)| k == "ACTA_EPIC" || k == "ACTA_TOPIC")
        );
    }

    #[test]
    fn missing_additional_folder_is_left_out() {
        let program = ClaudeProgram::Exe(PathBuf::from("claude"));
        let folders = [alexandre("acta"), alexandre("gone"), alexandre("docs")];
        let gone = alexandre("gone");
        let launch = build_claude_launch(
            &program,
            &folders,
            &focus(),
            &ClaudeOptions::default(),
            |p| p != gone.as_path(),
        )
        .unwrap();
        let docs = alexandre("docs").to_string_lossy().into_owned();
        assert_eq!(launch.spec.args, ["--add-dir", &docs]);
        assert_eq!(launch.left_out, [gone]);
    }

    #[test]
    fn missing_primary_or_no_folder_stops_the_launch() {
        let program = ClaudeProgram::Exe(PathBuf::from("claude"));
        assert_eq!(
            build_claude_launch(&program, &[], &focus(), &ClaudeOptions::default(), |_| true),
            Err(LaunchError::NoFolder)
        );
        let primary = alexandre("acta");
        assert_eq!(
            build_claude_launch(
                &program,
                std::slice::from_ref(&primary),
                &focus(),
                &ClaudeOptions::default(),
                |_| { false }
            ),
            Err(LaunchError::PrimaryMissing(primary))
        );
        assert_eq!(
            launch_spec_for_claude(&[], &focus(), &ClaudeOptions::default()),
            Err(LaunchError::NoFolder)
        );
    }

    #[test]
    fn a_first_prompt_is_the_last_argument() {
        let program = ClaudeProgram::Exe(PathBuf::from("claude"));
        let folders = [alexandre("acta"), alexandre("docs")];
        let triage = |prompt: &str| ClaudeOptions {
            prompt: Some(prompt.into()),
            conversation: Some(Conversation::New(
                "019a7a6e-1c2d-7f00-8a00-0123456789ab".into(),
            )),
            ..ClaudeOptions::default()
        };
        let launch = build_claude_launch(
            &program,
            &folders,
            &focus(),
            &triage("/acta:triage"),
            |_| true,
        )
        .unwrap();
        // After every option, the conversation's included.
        assert_eq!(
            launch.spec.args.last().map(String::as_str),
            Some("/acta:triage")
        );
        let shim = ClaudeProgram::CmdShim {
            comspec: "cmd.exe".into(),
            shim: PathBuf::from("C:\\npm\\claude.cmd"),
        };
        let launch =
            build_claude_launch(&shim, &folders, &focus(), &triage("/acta:triage"), |_| true)
                .unwrap();
        assert!(launch.spec.args[3].ends_with(" \"/acta:triage\"\""));
        let launch =
            build_claude_launch(&shim, &folders, &focus(), &triage("a&b"), |_| true).unwrap();
        assert!(!launch.spec.args[3].contains("a&b"), "left out");
    }

    #[test]
    fn cmd_shim_runs_through_comspec() {
        let program = ClaudeProgram::CmdShim {
            comspec: "C:\\Windows\\system32\\cmd.exe".into(),
            shim: PathBuf::from("C:\\npm\\claude.cmd"),
        };
        let folders = [PathBuf::from("C:\\acta"), PathBuf::from("D:\\docs")];
        let launch = build_claude_launch(
            &program,
            &folders,
            &focus(),
            &ClaudeOptions::default(),
            |_| true,
        )
        .unwrap();
        assert_eq!(launch.spec.program, "C:\\Windows\\system32\\cmd.exe");
        assert!(launch.spec.verbatim_args);
        assert_eq!(
            launch.spec.args,
            [
                "/d",
                "/s",
                "/c",
                r#"""C:\npm\claude.cmd" "--add-dir" "D:\docs"""#
            ]
        );

        // Trailing backslashes are doubled before the closing quote, so a
        // drive root does not swallow the next word when node parses `%*`.
        let folders = [
            PathBuf::from("C:\\acta"),
            PathBuf::from("D:\\"),
            PathBuf::from("E:\\x\\\\"),
            PathBuf::from("F:\\docs"),
        ];
        let launch = build_claude_launch(
            &program,
            &folders,
            &focus(),
            &ClaudeOptions::default(),
            |_| true,
        )
        .unwrap();
        let line = &launch.spec.args[3];
        assert_eq!(
            line,
            r#"""C:\npm\claude.cmd" "--add-dir" "D:\\" "--add-dir" "E:\x\\\\" "--add-dir" "F:\docs"""#
        );
        // What the shim's `%*` hands node: the line without cmd's outer
        // quotes and the shim word, split by MSVCRT rules.
        let rest = line[1..line.len() - 1]
            .split_once(' ')
            .map(|(_, rest)| rest)
            .unwrap();
        assert_eq!(
            msvcrt_split(rest),
            [
                "--add-dir",
                "D:\\",
                "--add-dir",
                "E:\\x\\\\",
                "--add-dir",
                "F:\\docs"
            ]
        );
    }

    /// Command-line splitting as the Microsoft C runtime does it.
    fn msvcrt_split(line: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut in_word = false;
        let mut quoted = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    let mut n = 1;
                    while chars.peek() == Some(&'\\') {
                        chars.next();
                        n += 1;
                    }
                    if chars.peek() == Some(&'"') {
                        word.push_str(&"\\".repeat(n / 2));
                        if n % 2 == 1 {
                            chars.next();
                            word.push('"');
                        }
                    } else {
                        word.push_str(&"\\".repeat(n));
                    }
                    in_word = true;
                }
                '"' => {
                    quoted = !quoted;
                    in_word = true;
                }
                ' ' | '\t' if !quoted => {
                    if in_word {
                        words.push(std::mem::take(&mut word));
                        in_word = false;
                    }
                }
                _ => {
                    word.push(c);
                    in_word = true;
                }
            }
        }
        if in_word {
            words.push(word);
        }
        words
    }

    #[test]
    fn cmd_shim_metacharacters() {
        let program = ClaudeProgram::CmdShim {
            comspec: "C:\\Windows\\system32\\cmd.exe".into(),
            shim: PathBuf::from("C:\\npm\\claude.cmd"),
        };

        // Folders cmd would interpret are left out and reported.
        let folders = [
            PathBuf::from("C:\\acta"),
            PathBuf::from("D:\\R&D"),
            PathBuf::from("D:\\100%"),
            PathBuf::from("D:\\my docs"),
        ];
        let launch = build_claude_launch(
            &program,
            &folders,
            &focus(),
            &ClaudeOptions::default(),
            |_| true,
        )
        .unwrap();
        assert_eq!(
            launch.spec.args[3],
            r#"""C:\npm\claude.cmd" "--add-dir" "D:\my docs"""#
        );
        assert_eq!(
            launch.left_out,
            [PathBuf::from("D:\\R&D"), PathBuf::from("D:\\100%")]
        );

        let bad = ClaudeProgram::CmdShim {
            comspec: "cmd.exe".into(),
            shim: PathBuf::from("C:\\R&D\\claude.cmd"),
        };
        assert_eq!(
            build_claude_launch(
                &bad,
                &folders[..1],
                &focus(),
                &ClaudeOptions::default(),
                |_| true
            ),
            Err(LaunchError::ClaudeNotFound)
        );
    }

    #[test]
    fn login_shell_passes_folders_as_positional_words() {
        let program = ClaudeProgram::LoginShell("/bin/zsh".into());
        let folders = [alexandre("acta"), PathBuf::from("/tmp/it's here")];
        let launch = build_claude_launch(
            &program,
            &folders,
            &focus(),
            &ClaudeOptions::default(),
            |_| true,
        )
        .unwrap();
        assert_eq!(launch.spec.program, "/bin/zsh");
        assert_eq!(
            launch.spec.args,
            [
                "-l",
                "-i",
                "-c",
                "exec claude \"$@\"",
                "claude",
                "--add-dir",
                "/tmp/it's here"
            ]
        );
    }

    #[test]
    fn windows_prefers_exe_anywhere_over_cmd() {
        let dirs = [PathBuf::from("A"), PathBuf::from("B")];
        let a_cmd = PathBuf::from("A").join("claude.cmd");
        let b_exe = PathBuf::from("B").join("claude.exe");
        let found = resolve_windows(&dirs, "cmd.exe".into(), |p| p == a_cmd || p == b_exe);
        assert_eq!(found, Some(ClaudeProgram::Exe(b_exe)));
        let found = resolve_windows(&dirs, "cmd.exe".into(), |p| p == a_cmd);
        assert_eq!(
            found,
            Some(ClaudeProgram::CmdShim {
                comspec: "cmd.exe".into(),
                shim: a_cmd
            })
        );
        assert_eq!(resolve_windows(&dirs, "cmd.exe".into(), |_| false), None);
    }

    #[test]
    fn unix_uses_posix_login_shell_else_known_paths() {
        let home = alexandre("");
        let found = resolve_unix(Some("/bin/zsh".into()), Some(&home), |_| true);
        assert_eq!(found, Some(ClaudeProgram::LoginShell("/bin/zsh".into())));
        let local = home.join(".local/bin/claude");
        let found = resolve_unix(Some("/usr/bin/fish".into()), Some(&home), |p| {
            p == local || p == Path::new("/usr/bin/fish")
        });
        assert_eq!(found, Some(ClaudeProgram::Exe(local)));
        let found = resolve_unix(None, None, |p| p == Path::new("/usr/local/bin/claude"));
        assert_eq!(
            found,
            Some(ClaudeProgram::Exe(PathBuf::from("/usr/local/bin/claude")))
        );
    }

    /// An absolute folder of Alexandre's on this OS.
    fn abs(dir: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from("C:\\Users\\alexandre").join(dir)
        } else {
            alexandre(dir)
        }
    }

    #[test]
    fn path_keeps_the_spelling_of_the_parent_on_windows_only() {
        assert_eq!(path_var_name(true, &["TEMP", "Path"]), "Path");
        assert_eq!(path_var_name(true, &["PATH"]), "PATH");
        assert_eq!(path_var_name(true, &["TEMP"]), "Path");
        assert_eq!(path_var_name(false, &["Path"]), "PATH");
        // Any spelling already in the launch is replaced, never doubled.
        let mut env = vec![
            ("path".to_owned(), "old".to_owned()),
            ("ACTA_TERMINAL".to_owned(), "1".to_owned()),
        ];
        set_path(&mut env, true, &["Path"], std::ffi::OsStr::new("C:\\new"));
        assert_eq!(
            env,
            vec![
                ("ACTA_TERMINAL".to_owned(), "1".to_owned()),
                ("Path".to_owned(), "C:\\new".to_owned()),
            ]
        );
        // Elsewhere `Path` is another variable and stays.
        let mut env = vec![("Path".to_owned(), "x".to_owned())];
        set_path(&mut env, false, &["PATH"], std::ffi::OsStr::new("/usr/bin"));
        assert_eq!(env.len(), 2);
        assert_eq!(env[1], ("PATH".to_owned(), "/usr/bin".to_owned()));
    }

    /// The one-off session's command line: the program itself, found on
    /// the search directories, its arguments as separate words for the PTY
    /// to quote, and no shell in between.
    #[test]
    fn a_one_off_session_starts_the_program_itself() {
        use std::ffi::OsString;
        let dir = abs("tools");
        let exe = if cfg!(windows) {
            dir.join("gh.exe")
        } else {
            dir.join("gh")
        };
        let argv: Vec<OsString> = ["gh", "auth", "login", "--web"]
            .iter()
            .map(OsString::from)
            .collect();
        let found = |p: &Path| p == exe;
        let dirs = std::slice::from_ref(&dir);
        let spec = one_off_spec(&argv, abs("work"), dirs, None, found).unwrap();
        assert_eq!(spec.program, exe.to_string_lossy());
        assert_eq!(spec.args, ["auth", "login", "--web"]);
        assert_eq!(spec.cwd, abs("work"));
        assert!(!spec.verbatim_args, "the PTY quotes each word itself");
        assert!(spec.env.is_empty());
        for shell in ["cmd", "sh", "bash", "powershell", "zsh"] {
            assert!(
                !Path::new(&spec.program)
                    .file_stem()
                    .is_some_and(|s| s.eq_ignore_ascii_case(shell)),
                "{shell} between Acta and the program"
            );
        }
        // Not found: nothing starts.
        assert_eq!(
            one_off_spec(&argv, abs("work"), dirs, None, |_| false),
            None
        );
        // A path is taken as given; the fresh `PATH` goes with it.
        let argv: Vec<OsString> = vec![exe.clone().into_os_string(), "--version".into()];
        let fresh = Some(std::ffi::OsStr::new("fresh"));
        let spec = one_off_spec(&argv, abs("work"), &[], fresh, found).unwrap();
        assert_eq!(spec.program, exe.to_string_lossy());
        assert_eq!(spec.args, ["--version"]);
        assert_eq!(spec.env.len(), 1);
        assert!(spec.env[0].0.eq_ignore_ascii_case("PATH"));
        assert_eq!(spec.env[0].1, "fresh");
    }

    /// A script needs a shell to run: refused, never wrapped in one.
    #[cfg(windows)]
    #[test]
    fn a_one_off_session_refuses_a_script() {
        let dir = abs("npm");
        let shim = dir.join("gh.cmd");
        let argv = vec![std::ffi::OsString::from("gh")];
        assert_eq!(
            one_off_spec(&argv, abs("work"), &[dir], None, |p| p == shim),
            None
        );
    }

    #[test]
    fn sign_in_starts_claude_code_alone() {
        let exe = abs("claude.exe");
        let spec = plain_claude_spec(&ClaudeProgram::Exe(exe.clone()), abs("home")).unwrap();
        assert_eq!(spec.program, exe.to_string_lossy());
        assert!(spec.args.is_empty(), "{:?}", spec.args);
        assert!(spec.env.is_empty());
        assert_eq!(spec.cwd, abs("home"));
    }
}
