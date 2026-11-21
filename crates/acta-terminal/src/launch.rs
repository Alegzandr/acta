//! What to start in the terminal, and how Claude Code is started (ADR 0002 point 3, ADR 0008).
//!
//! Nothing here touches GPUI. The existence checks and the `PATH` search read the
//! file system, so the app calls [`launch_spec_for_claude`] on a background executor.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

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

/// How `claude` is reached on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaudeProgram {
    /// A native executable, spawned directly.
    Exe(PathBuf),
    /// A Windows `.cmd` shim, run through `%ComSpec% /d /s /c`.
    CmdShim { comspec: String, shim: PathBuf },
    /// A Unix login shell that resolves `claude` from the user's own `PATH`.
    LoginShell(String),
}

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

/// Whether Claude Code still has the transcript of conversation `id`:
/// `<config>/projects/<project>/<id>.jsonl` (code.claude.com/docs/en/sessions,
/// "Where transcripts are stored"). Only existence is checked; nothing is
/// read. Transcripts age out after 30 days by default, and a resume of an id
/// that is gone ends at once with "No conversation found", so Acta starts a
/// new conversation instead.
pub fn transcript_exists(config_dir: &Path, id: &str) -> bool {
    if !is_uuid(id) {
        return false;
    }
    let file = format!("{id}.jsonl");
    std::fs::read_dir(config_dir.join("projects"))
        .map(|dirs| dirs.flatten().any(|d| d.path().join(&file).is_file()))
        .unwrap_or(false)
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

/// Find `claude` the way ADR 0002 point 3 says, for this platform.
pub fn resolve_claude() -> Option<ClaudeProgram> {
    #[cfg(windows)]
    {
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        if let Some(home) = std::env::var_os("USERPROFILE") {
            dirs.push(PathBuf::from(home).join(".local").join("bin"));
        }
        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned());
        resolve_windows(&dirs, comspec, |path| path.is_file())
    }
    #[cfg(not(windows))]
    {
        let shell = std::env::var_os("SHELL");
        let home = std::env::var_os("HOME").map(PathBuf::from);
        resolve_unix(shell, home.as_deref(), |path| path.is_file())
    }
}

/// `claude.exe` anywhere on the search path wins over a `claude.cmd` shim.
pub fn resolve_windows(
    dirs: &[PathBuf],
    comspec: String,
    is_file: impl Fn(&Path) -> bool,
) -> Option<ClaudeProgram> {
    if let Some(exe) = dirs
        .iter()
        .map(|dir| dir.join("claude.exe"))
        .find(|p| is_file(p))
    {
        return Some(ClaudeProgram::Exe(exe));
    }
    dirs.iter()
        .map(|dir| dir.join("claude.cmd"))
        .find(|p| is_file(p))
        .map(|shim| ClaudeProgram::CmdShim { comspec, shim })
}

/// Shells known to take `-l -i -c script $0 args...`; any other `$SHELL`
/// (fish, nu) falls back to the well-known install locations.
const POSIX_SHELLS: [&str; 5] = ["bash", "zsh", "sh", "dash", "ksh"];

pub fn resolve_unix(
    shell: Option<OsString>,
    home: Option<&Path>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<ClaudeProgram> {
    if let Some(shell) = shell {
        let path = PathBuf::from(&shell);
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if POSIX_SHELLS.contains(&name) && is_file(&path) {
            return Some(ClaudeProgram::LoginShell(
                shell.to_string_lossy().into_owned(),
            ));
        }
    }
    let mut candidates = Vec::new();
    if let Some(home) = home {
        candidates.push(home.join(".local/bin/claude"));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/claude"));
    candidates.push(PathBuf::from("/usr/local/bin/claude"));
    candidates
        .into_iter()
        .find(|p| is_file(p))
        .map(ClaudeProgram::Exe)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!transcript_exists(&dir, id));
        let project = dir.join("projects").join("C--Users-alexandre-acta");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(format!("{id}.jsonl")), "").unwrap();
        assert!(transcript_exists(&dir, id));
        assert!(!transcript_exists(
            &dir,
            "019a7a6e-1c2d-7f00-8a00-0123456789ac"
        ));
        assert!(!transcript_exists(&dir, "../x"));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(is_uuid(id) && !is_uuid("019a7a6e1c2d7f008a000123456789ab"));
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
}
