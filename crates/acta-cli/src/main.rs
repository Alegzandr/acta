//! `acta`: console executable. Packaged as `acta-cli`, exported to the user
//! as `acta` by the GUI (ADR 0004). The CLI, the MCP server (`acta mcp`) and
//! the hooks (`acta hook`) share the domain layer of `acta-core`; nothing
//! here decides anything about the data.
//!
//! Free text (log text, titles, states, queries, knowledge bodies) is read
//! from stdin or `--file`, never from arguments: process arguments are
//! visible to other processes (CLAUDE.md).

mod browser;
mod folders;
mod hook;
mod link;
mod mcp;
mod open;
mod ready;
mod setup;
mod statusline;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use acta_core::agent::args::*;
use acta_core::agent::{Agent, Launch, Session};
use acta_core::model::{Author, KbKind, Level, Owner, ProjectId};
use acta_core::paths::{self, BaseDirs, Os};
use acta_core::store::{ExportFile, NewKb};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};
use open::Opener;

#[derive(Parser)]
#[command(
    name = "acta",
    bin_name = "acta",
    version = acta_core::VERSION,
    about = "Acta: project tracker and scoped memory for Claude Code",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Args, Clone, Default)]
struct TextSource {
    /// Read the text from this file instead of stdin
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,
}

#[derive(Subcommand, Clone, Copy)]
enum McpServer {
    /// The agent browser's server (Windows; drives Acta's browser view)
    Browser,
}

#[derive(Subcommand)]
enum Command {
    /// Print the version
    Version,
    /// Run the MCP server on stdio (started by Claude Code)
    Mcp {
        #[command(subcommand)]
        server: Option<McpServer>,
    },
    /// Run a Claude Code hook: session-start, pre-compact or pre-git (hook JSON on stdin)
    Hook { event: String },
    /// Folder settings set by the user
    Folders {
        #[command(subcommand)]
        action: FoldersAction,
    },
    /// Install Acta in Claude Code: plugin, MCP server, hooks, skills, allow rule
    Setup {
        /// Report each step without changing anything
        #[arg(long)]
        check: bool,
        /// Undo what setup did
        #[arg(long, visible_alias = "undo")]
        remove: bool,
        /// Do not add the allow rule to Claude Code's settings.json
        #[arg(long)]
        no_allow: bool,
        /// Do not set Acta's status line (plan usage) in Claude Code
        #[arg(long)]
        no_statusline: bool,
        /// Only write the plugin files (Acta loads them with --plugin-dir in
        /// the sessions it starts); nothing in Claude Code is changed
        #[arg(long, conflicts_with_all = ["check", "remove"])]
        files_only: bool,
        /// With --check: leave out the Ready for Claude Code rows (claude,
        /// signed in, git, gh), which run each tool; for a cheap re-check
        #[arg(long, requires = "check")]
        no_prereqs: bool,
        /// Structured output
        #[arg(long)]
        json: bool,
    },
    /// Attach a folder (default: this repository) to a project, an epic or a topic
    Link {
        path: Option<PathBuf>,
        #[arg(long, value_name = "P-n")]
        project: Option<String>,
        #[arg(long, value_name = "E-n")]
        epic: Option<String>,
        #[arg(long, value_name = "T-n")]
        topic: Option<String>,
        /// Add as an additional folder instead of the primary one
        #[arg(long)]
        add: bool,
        /// Detach the folder
        #[arg(long)]
        remove: bool,
        /// Leave Claude Code's auto memory on in this folder
        #[arg(long)]
        keep_auto_memory: bool,
        /// The folders already attached in Acta (of --project, or all),
        /// read from Acta: no path is given
        #[arg(long, conflicts_with_all = ["path", "epic", "topic", "remove"])]
        attached: bool,
    },
    /// Where things stand: the focus topic, an epic or the project
    Brief {
        id: Option<String>,
        /// Make ID the focus of this folder and branch
        #[arg(long)]
        focus: bool,
    },
    /// A topic's timeline, newest first
    Entries {
        #[arg(long)]
        topic: Option<String>,
        #[arg(long = "type")]
        kind: Option<String>,
        #[arg(long)]
        before: Option<i64>,
        #[arg(long)]
        limit: Option<i64>,
        /// One entry in full: T-42#14
        #[arg(long)]
        id: Option<String>,
    },
    /// Append a timeline entry (text on stdin or --file)
    Log {
        /// progress, decision or discussion
        kind: String,
        #[arg(long)]
        topic: Option<String>,
        #[arg(long)]
        status: Option<String>,
        /// Also replace the topic's state with this file's content
        #[arg(long, value_name = "PATH")]
        state_file: Option<PathBuf>,
        #[command(flatten)]
        text: TextSource,
    },
    /// Change a topic, epic, note or question; --field reads that field's
    /// value from stdin or --file (json: several fields as one JSON object)
    Update {
        id: String,
        /// As in log; a note: open or dismissed; a question: open, answered
        /// or dropped
        #[arg(long)]
        status: Option<String>,
        /// E-n, or none
        #[arg(long)]
        epic: Option<String>,
        /// Due date YYYY-MM-DD, or "" to clear (topics, epics and projects)
        #[arg(long, value_name = "DATE")]
        due: Option<String>,
        /// A question answered on a topic: also log the answer as a decision
        #[arg(long)]
        log_decision: bool,
        /// A topic or an epic was reported to its contact (true or false)
        #[arg(long, value_name = "BOOL")]
        reported: Option<bool>,
        /// A contact's level: all, P-n or E-n
        #[arg(long)]
        level: Option<String>,
        /// Print the record's fields as JSON
        #[arg(long)]
        json: bool,
        /// title, description, state, answer, asked_to, report_to, or json
        /// (an object with those names)
        #[arg(long)]
        field: Option<String>,
        #[command(flatten)]
        text: TextSource,
    },
    /// Find topics, epics, notes or entries; --query reads the query from stdin
    Find {
        #[arg(long)]
        query: bool,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        epic: Option<String>,
        /// overdue, or week: open topics and epics due before today, or
        /// within 7 days, by date (questions: by answer date)
        #[arg(long)]
        due: Option<String>,
        /// "-": read a person's name from stdin; lists everything open
        /// naming them
        #[arg(long, value_name = "-")]
        person: Option<String>,
        #[arg(long)]
        limit: Option<i64>,
        #[arg(long)]
        cursor: Option<i64>,
        /// Structured output, with due and overdue on topics and epics
        #[arg(long)]
        json: bool,
    },
    /// Create a topic, epic, note or question (first line of stdin is the
    /// title, the rest the description; or one JSON object with the item's
    /// fields, e.g. {"title": "...", "asked_to": "...", "report_to": "..."});
    /// file notes into a topic's timeline (entry); dismiss notes; or apply a
    /// triage plan (plan: create parameters as JSON on stdin)
    Create {
        /// topic, epic, note, question, contact, entry, dismiss or plan
        kind: String,
        /// E-n for a new topic
        #[arg(long)]
        epic: Option<String>,
        /// T-n for entry
        #[arg(long)]
        topic: Option<String>,
        /// Due date of a new topic or epic, or a question's answer date:
        /// YYYY-MM-DD
        #[arg(long, value_name = "DATE")]
        due: Option<String>,
        /// A question's owner: T-n, E-n or P-n (default: the focus)
        #[arg(long, value_name = "ID")]
        owner: Option<String>,
        /// A contact's level: all, P-n or E-n (default: the project)
        #[arg(long)]
        level: Option<String>,
        /// stdin is one JSON object with the item's fields (title,
        /// description, asked_to, report_to) instead of text
        #[arg(long)]
        json: bool,
        /// Comma-separated note ids to file into the new item, or to dismiss
        #[arg(long, value_delimiter = ',')]
        from_notes: Vec<String>,
        #[command(flatten)]
        text: TextSource,
    },
    /// Knowledge in scope; subcommands read, write, import, export
    Kb {
        #[command(subcommand)]
        action: Option<KbAction>,
        /// Read a search query from stdin
        #[arg(long)]
        query: bool,
        #[arg(long)]
        kind: Option<String>,
        /// Scope: T-n, E-n or P-n
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        cursor: Option<i64>,
    },
    /// Export everything, or one project, to an .acta.json file (plaintext)
    Export {
        path: PathBuf,
        #[arg(long, value_name = "P-n")]
        project: Option<String>,
        /// Leave attached files' content out (names and sizes stay)
        #[arg(long)]
        no_files: bool,
    },
    /// Internal: the text of some pages of an attached PDF, as JSON. Run
    /// by `acta mcp` and `acta attachment` as a child with a time, memory
    /// and output limit, so a malformed file never reaches the server.
    #[command(hide = true)]
    PdfText {
        id: String,
        #[arg(long, default_value_t = 1)]
        from: u32,
        #[arg(long, default_value_t = 1)]
        to: u32,
        #[arg(long, default_value_t = 8_000)]
        max_chars: usize,
    },
    /// Read one attached file (A-n): its text, a PDF's text, or what an
    /// image is (the picture itself reaches Claude through the MCP tool)
    Attachment {
        id: String,
        /// Text files: e.g. 40-80
        #[arg(long)]
        lines: Option<String>,
        /// PDFs: e.g. 3-5
        #[arg(long)]
        pages: Option<String>,
        /// Images: full for 1568 px
        #[arg(long)]
        size: Option<String>,
    },
    /// Merge an .acta.json export into the database
    Import {
        path: PathBuf,
        /// Report what would change without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// The Claude Code sessions running now in linked folders, and what they do
    Sessions,
    /// Claude Code's status line (Claude Code runs it, JSON on stdin): keeps
    /// the plan's usage for Acta and prints a short line
    Statusline,
    /// List or restore automatic snapshots
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },
}

#[derive(Subcommand)]
enum KbAction {
    /// Read an entry: whole if small, else its outline
    Read {
        id: String,
        #[arg(long)]
        section: Option<String>,
        #[arg(long)]
        lines: Option<String>,
    },
    /// Create or edit an entry: the kb_write parameters as JSON on stdin or --file
    Write {
        #[command(flatten)]
        text: TextSource,
    },
    /// Create an entry from a .md or .txt file
    Import {
        path: PathBuf,
        /// document or memory
        #[arg(long, default_value = "document")]
        kind: String,
        /// topic, epic or project (relative to the focus); default project
        #[arg(long)]
        level: Option<String>,
        /// Owner T-n, E-n or P-n
        #[arg(long)]
        owner: Option<String>,
    },
    /// Save an entry's body to a new file
    Export { id: String, path: PathBuf },
}

#[derive(Subcommand)]
enum SnapshotAction {
    /// List snapshots, newest first
    List,
    /// Replace the database with a snapshot (the current state is snapshotted first)
    Restore {
        /// A name from `acta snapshot list`, or "latest"
        name: String,
    },
}

#[derive(Subcommand)]
enum FoldersAction {
    /// The folders a page sees (default: the focus), numbered, the active one marked
    List {
        /// T-n, E-n or P-n
        id: Option<String>,
    },
    /// Show the page's active folder, or set it: a number from list, a name, or default
    Active {
        folder: Option<String>,
        /// T-n, E-n or P-n (default: the focus)
        #[arg(long)]
        id: Option<String>,
    },
    /// Set an attached folder's commit mode: manual, commit or push
    Commit {
        /// manual, commit (commit automatically) or push (commit and push automatically)
        mode: String,
        /// The folder (default: this repository)
        path: Option<PathBuf>,
        /// Commit in my name, without Co-Authored-By Claude
        #[arg(long)]
        as_me: bool,
        /// Also commit and push automatically on the default branch
        #[arg(long)]
        on_default: bool,
    },
}

fn main() -> ExitCode {
    // A hook never fails or disturbs a Claude Code session: whatever goes
    // wrong, `acta hook ...` exits 0 and prints nothing.
    if std::env::args_os().nth(1).is_some_and(|a| a == "hook") {
        return match Cli::try_parse() {
            Ok(Cli {
                command: Some(command @ Command::Hook { .. }),
            }) => run(command),
            _ => ExitCode::SUCCESS,
        };
    }
    match Cli::try_parse() {
        Ok(Cli {
            command: Some(command),
        }) => run(command),
        Ok(Cli { command: None }) => {
            // Nothing asked: show what exists.
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
        Err(err) if is_help_or_version(&err) => {
            let _ = err.print();
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}", one_line(&err.to_string()));
            ExitCode::from(2)
        }
    }
}

/// `find --json` carries stored titles without the fence, so it is for
/// scripts, not for Claude Code.
const FIND_JSON_REFUSED: &str =
    "find --json is for scripts; in Claude Code use find without --json (stored text is fenced)";

/// The author is Claude Code when it runs the CLI (ADR 0005, section 4).
/// Any of Claude Code's variables counts: clearing `CLAUDECODE` alone does
/// not make a call the user's. A guard rail, not a boundary (SECURITY.md).
fn author() -> Author {
    author_from(|k| std::env::var_os(k))
}

fn author_from(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Author {
    let set = |k: &str| var(k).is_some_and(|v| !v.is_empty());
    if var("CLAUDECODE").is_some_and(|v| v == "1")
        || set("CLAUDE_CODE_ENTRYPOINT")
        || set("CLAUDE_PROJECT_DIR")
    {
        Author::Claude
    } else {
        Author::Human
    }
}

fn read_text(src: &TextSource) -> Result<String, String> {
    match &src.file {
        Some(p) => {
            std::fs::read_to_string(p).map_err(|e| format!("cannot read the file: {}", e.kind()))
        }
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| format!("cannot read stdin: {}", e.kind()))?;
            Ok(s)
        }
    }
}

fn finish(r: Result<String, String>) -> ExitCode {
    match r {
        Ok(text) => {
            if !text.is_empty() {
                println!("{text}");
            }
            ExitCode::SUCCESS
        }
        Err(text) => {
            eprintln!("{text}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> ExitCode {
    let opener = Opener::from_env();
    // PDFs are parsed by a child of this same executable (`pdf-text`).
    if matches!(
        command,
        Command::Mcp { server: None } | Command::Attachment { .. }
    ) && let Ok(exe) = std::env::current_exe()
    {
        acta_core::agent::set_pdf_worker(exe);
    }
    match command {
        Command::Version => {
            println!("{}", version_line());
            ExitCode::SUCCESS
        }
        Command::Mcp { server: None } => finish(mcp::serve(opener).map(|_| String::new())),
        Command::Mcp {
            server: Some(McpServer::Browser),
        } => finish(browser::serve(opener).map(|_| String::new())),
        Command::Hook { event } => {
            let Some(event) = hook::Event::parse(&event) else {
                return ExitCode::SUCCESS;
            };
            let stdin = hook::read_stdin();
            // A hook never fails the session: errors print nothing.
            if let Some(out) = hook::run(event, &stdin, &opener) {
                println!("{out}");
            }
            ExitCode::SUCCESS
        }
        Command::Setup {
            check,
            remove,
            no_allow,
            no_statusline,
            files_only,
            no_prereqs,
            json,
        } => {
            if files_only {
                run_plugin_files(&opener, json)
            } else {
                let prereqs = check && !no_prereqs;
                run_setup(
                    &opener,
                    check,
                    prereqs,
                    remove,
                    no_allow,
                    no_statusline,
                    json,
                )
            }
        }
        Command::PdfText {
            id,
            from,
            to,
            max_chars,
        } => finish(pdf_text(&opener, &id, from, to, max_chars)),
        Command::Statusline => {
            let input = statusline::read_stdin();
            let data_dir = opener.data_dir();
            println!(
                "{}",
                statusline::run(&input, data_dir.as_deref(), acta_core::time::now_ms())
            );
            ExitCode::SUCCESS
        }
        other => finish(with_store(&opener, other)),
    }
}

/// `acta pdf-text`: the child that parses one attached PDF. It reads the
/// file from the store itself (only an id is on its command line) and
/// prints the pages as JSON on stdout.
fn pdf_text(
    opener: &Opener,
    id: &str,
    from: u32,
    to: u32,
    max_chars: usize,
) -> Result<String, String> {
    let store = opener.open()?;
    let id = acta_core::model::AttachmentId::parse(id).ok_or("not a file id")?;
    let att = store.attachment(id).map_err(|e| e.to_string())?;
    if att.kind != acta_core::model::FileKind::Pdf {
        return Err("not a PDF".into());
    }
    let bytes = store
        .attachment_prefix(id, att.size.max(0) as usize)
        .map_err(|e| e.to_string())?;
    drop(store);
    match acta_core::files::pdf_pages(&bytes, from, to, max_chars) {
        acta_core::files::PdfText::Pages(p) => serde_json::to_string(&p).map_err(|e| e.to_string()),
        acta_core::files::PdfText::Unreadable => Err("unreadable".into()),
    }
}

/// The `acta` executable written into the plugin: the exported CLI when
/// there is one (ADR 0004: its path is stable across updates).
fn plugin_exe(opener: &Opener, base: &BaseDirs) -> Option<PathBuf> {
    let exported = paths::cli_export_dir(Os::CURRENT, base)
        .map(|d| d.join(paths::exported_cli_name(Os::CURRENT)))
        .filter(|p| p.is_file() && !opener.dev);
    exported.or_else(|| std::env::current_exe().ok())
}

/// `setup --files-only`: the plugin tree, nothing else. The app runs it
/// at start and before a terminal launch, so sessions it starts load the
/// plugin of this very build with `--plugin-dir`, set up or not.
fn run_plugin_files(opener: &Opener, json: bool) -> ExitCode {
    let (Some(data_dir), Some(exe)) =
        (opener.data_dir(), plugin_exe(opener, &BaseDirs::from_env()))
    else {
        eprintln!("Acta cannot find its data directory or its executable.");
        return ExitCode::FAILURE;
    };
    struct NoClaude;
    impl setup::ClaudeCli for NoClaude {
        fn run(&self, _: &[&str]) -> Result<String, String> {
            Err("not used".into())
        }
    }
    let s = setup::Setup {
        plugin_root: data_dir.join("claude"),
        claude_dir: PathBuf::new(),
        exe,
        claude: &NoClaude,
    };
    let step = s.plugin_files();
    println!("{}", setup::render_steps(std::slice::from_ref(&step), json));
    if step.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run_setup(
    opener: &Opener,
    check: bool,
    prereqs: bool,
    remove: bool,
    no_allow: bool,
    no_statusline: bool,
    json: bool,
) -> ExitCode {
    let Some(data_dir) = opener.data_dir() else {
        eprintln!(
            "{}",
            opener
                .refusal
                .unwrap_or("Acta cannot find its data directory.")
        );
        return ExitCode::FAILURE;
    };
    let base = BaseDirs::from_env();
    let claude_env = std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    if opener.dev && claude_env.is_none() {
        eprintln!(
            "acta setup with {} needs CLAUDE_CONFIG_DIR, so a development data directory is never registered in the real Claude Code configuration.",
            open::DEV_DATA_DIR
        );
        return ExitCode::FAILURE;
    }
    let Some(claude_dir) = claude_env
        .clone()
        .or_else(|| base.home.as_ref().map(|h| h.join(".claude")))
    else {
        eprintln!("Cannot find the home directory.");
        return ExitCode::FAILURE;
    };
    let Some(exe) = plugin_exe(opener, &base) else {
        eprintln!("Cannot find the acta executable.");
        return ExitCode::FAILURE;
    };
    let (machine, dev_search) = ready::machine();
    let is_file = acta_core::prereqs::locate::is_file;
    let program = acta_core::prereqs::locate(machine.os, "claude", &machine.dirs, is_file);
    let claude = setup::RealClaude {
        config_dir: claude_env,
        program,
    };
    let s = setup::Setup {
        plugin_root: data_dir.join("claude"),
        claude_dir,
        exe,
        claude: &claude,
    };
    let mut steps = if check {
        // Ready for Claude Code first (ADR 0013, section 5). The search
        // override keeps the process PATH for what it runs.
        let exec = if dev_search {
            acta_core::prereqs::SystemExec::default()
        } else {
            acta_core::prereqs::SystemExec::current()
        };
        let mut rows = if prereqs {
            ready::check(&machine, &is_file, &exec)
        } else {
            Vec::new()
        };
        rows.extend(s.check());
        rows
    } else if remove {
        s.remove()
    } else {
        let steps = s.install_with(!no_allow, !no_statusline);
        // The one-time switch to incremental auto-vacuum (ADR 0012, 4), if
        // the app has not done it yet; quiet, and skipped when busy.
        if let Some(Ok(mut store)) = opener.open_existing() {
            let _ = store.enable_incremental_vacuum();
        }
        steps
    };
    if check {
        steps.push(link_index_step(opener));
    }
    if remove && let Some(step) = auto_memory_step(opener) {
        steps.push(step);
    }
    println!("{}", setup::render_steps(&steps, json));
    let ok = steps.iter().all(|x| x.ok);
    if ok && !json && !check && !remove {
        println!(
            "Acta is set up. Link a repository with `acta link` in it, then start a new Claude Code session there. Undo with `acta setup --remove`."
        );
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `setup --check`: the hooks' pre-check file can be written from the
/// current folders.
fn link_index_step(opener: &Opener) -> setup::Step {
    let (ok, detail) = match opener.open_existing() {
        None => (true, "no database yet: nothing linked".to_owned()),
        Some(Ok(store)) => match store.refresh_link_index() {
            Ok(()) => (true, "up to date".to_owned()),
            Err(e) => (
                false,
                format!(
                    "cannot write the pre-check file ({}): hooks may miss linked folders",
                    e.kind()
                ),
            ),
        },
        Some(Err(e)) => (false, e),
    };
    setup::Step::new("link index", ok, detail)
}

/// `setup --remove`: the linked folders where `acta link` turned Claude
/// Code's auto memory off stay that way until unlinked; say where.
fn auto_memory_step(opener: &Opener) -> Option<setup::Step> {
    let mut store = opener.open_existing()?.ok()?;
    let (restored, kept) = match link::restore_all_auto_memory(&mut store) {
        Ok(counts) => counts,
        Err(e) => {
            return Some(setup::Step::new(
                "auto memory",
                false,
                format!("cannot restore: {e}"),
            ));
        }
    };
    if restored == 0 && kept == 0 {
        return None;
    }
    let detail = match kept {
        0 => format!("back on in {restored} folder(s)"),
        _ => format!(
            "back on in {restored} folder(s); {kept} left as they were (changed since, or not on this computer)"
        ),
    };
    Some(setup::Step::new("auto memory", true, detail))
}

fn launch() -> Launch {
    let mut l = Launch::from_env();
    // The CLI runs where the user is; CLAUDE_PROJECT_DIR is for the server.
    l.dir = std::env::current_dir().ok();
    l
}

fn with_store(opener: &Opener, command: Command) -> Result<String, String> {
    let mut store = opener.open()?;
    let result = run_on(&mut store, command);
    if store.link_index_failed() {
        eprintln!(
            "warning: the hooks' pre-check file could not be updated; run `acta setup --check`"
        );
    }
    result
}

fn run_on(store: &mut acta_core::Store, command: Command) -> Result<String, String> {
    let launch = launch();
    let mut session = Session::resolve(store, &launch, author());
    let command = match command {
        Command::Link {
            path,
            project,
            epic,
            topic,
            add,
            remove,
            keep_auto_memory,
            attached,
        } => {
            if attached {
                return link::run_attached(store, project.as_deref(), keep_auto_memory);
            }
            let cwd = launch.dir.clone().unwrap_or_default();
            let args = link::LinkArgs {
                path,
                project,
                epic,
                topic,
                add,
                remove,
                keep_auto_memory,
            };
            return link::run(store, args, &cwd, session.author);
        }
        Command::Folders {
            action: FoldersAction::List { id },
        } => return folders::list(store, &session, id.as_deref()),
        Command::Folders {
            action: FoldersAction::Active { folder, id },
        } => {
            return folders::active(store, &session, id.as_deref(), folder.as_deref(), author());
        }
        Command::Folders {
            action:
                FoldersAction::Commit {
                    mode,
                    path,
                    as_me,
                    on_default,
                },
        } => {
            let cwd = launch.dir.clone().unwrap_or_default();
            let args = folders::CommitArgs {
                mode,
                path,
                as_me,
                on_default,
            };
            return folders::commit(store, args, &cwd, author());
        }
        Command::Export {
            path,
            project,
            no_files,
        } => return export(store, &path, project, !no_files),
        Command::Import { path, dry_run } => return import(store, &path, dry_run),
        Command::Snapshot { action } => return snapshot(store, action),
        Command::Sessions => {
            return acta_core::agent::presence::render_sessions(store, acta_core::time::now_ms())
                .map_err(|e| e.to_string());
        }
        other => other,
    };
    let mut ag = Agent {
        store,
        session: &mut session,
    };
    match command {
        Command::Brief { id, focus } => ag.brief(BriefArgs { id, focus }),
        Command::Entries {
            topic,
            kind,
            before,
            limit,
            id,
        } => ag.entries(EntriesArgs {
            topic,
            kind,
            before,
            limit,
            id,
        }),
        Command::Log {
            kind,
            topic,
            status,
            state_file,
            text,
        } => {
            let state = state_file
                .map(|p| {
                    std::fs::read_to_string(p)
                        .map_err(|e| format!("cannot read the state file: {}", e.kind()))
                })
                .transpose()?;
            ag.log(LogArgs {
                kind,
                text: read_text(&text)?,
                state,
                status,
                topic,
            })
        }
        Command::Update {
            id,
            status,
            epic,
            due,
            log_decision,
            reported,
            level,
            json,
            field,
            text,
        } => {
            let mut a = UpdateArgs {
                id: id.clone(),
                status,
                epic,
                due,
                log_decision: log_decision.then_some(true),
                reported,
                level,
                ..UpdateArgs::default()
            };
            if let Some(f) = field {
                let value = read_text(&text)?.trim_end().to_owned();
                match f.as_str() {
                    "title" => a.title = Some(value),
                    "description" => a.description = Some(value),
                    "state" => a.state = Some(value),
                    "answer" => a.answer = Some(value),
                    "asked_to" => a.asked_to = Some(value),
                    "report_to" => a.report_to = Some(value),
                    "json" => {
                        let t: TextFields = serde_json::from_str(&value)
                            .map_err(|e| format!("--field json takes one JSON object: {e}"))?;
                        a.title = t.title;
                        a.description = t.description;
                        a.state = t.state;
                        a.answer = t.answer;
                        a.asked_to = t.asked_to;
                        a.report_to = t.report_to;
                    }
                    _ => {
                        return Err(
                            "--field must be title, description, state, answer, asked_to, report_to or json"
                                .into(),
                        );
                    }
                }
            }
            let ack = ag.update(a)?;
            if json { ag.record_json(&id) } else { Ok(ack) }
        }
        Command::Find {
            query,
            kind,
            status,
            epic,
            due,
            person,
            limit,
            cursor,
            json,
        } => {
            if person.as_deref().is_some_and(|p| p != "-") {
                return Err("a person is read from stdin: --person -".into());
            }
            if query && person.is_some() {
                return Err("--query and --person both read stdin; use one".into());
            }
            let stdin = if query || person.is_some() {
                Some(read_text(&TextSource::default())?.trim().to_owned())
            } else {
                None
            };
            let (query, person) = match person {
                Some(_) => (None, stdin),
                None => (stdin, None),
            };
            let a = FindArgs {
                query,
                kind,
                status,
                epic,
                person,
                due,
                limit,
                cursor,
            };
            if json && ag.session.author == Author::Claude {
                // Titles in JSON are not fenced: Claude Code reads the text.
                return Err(FIND_JSON_REFUSED.into());
            }
            if json { ag.find_json(a) } else { ag.find(a) }
        }
        Command::Create {
            kind,
            epic,
            topic,
            due,
            owner,
            level,
            json,
            from_notes,
            text,
        } => {
            let notes = Some(from_notes).filter(|n| !n.is_empty());
            match kind.trim().to_ascii_lowercase().as_str() {
                "plan" => {
                    let a: CreateArgs = serde_json::from_str(&read_text(&text)?).map_err(|e| {
                        format!("create plan takes the create parameters as JSON: {e}")
                    })?;
                    ag.create(a)
                }
                "dismiss" => ag.create(CreateArgs {
                    items: Vec::new(),
                    dismiss: Some(notes.ok_or("dismiss needs --from-notes N-1,N-2")?),
                }),
                // An entry's text is the notes' own: nothing on stdin.
                "entry" => ag.create(CreateArgs {
                    items: vec![CreateItem {
                        kind,
                        topic,
                        from_notes: notes,
                        ..CreateItem::default()
                    }],
                    dismiss: None,
                }),
                _ => {
                    let t = read_text(&text)?;
                    let t = t.trim();
                    // A question or a contact is all of its text.
                    let question = ["question", "contact"]
                        .iter()
                        .any(|k| kind.trim().eq_ignore_ascii_case(k));
                    // With --json, one JSON object carries the person (user
                    // text never comes from an argument); otherwise the
                    // first line is the title. A question is all its text.
                    let fields = if json {
                        serde_json::from_str::<TextFields>(t)
                            .map_err(|e| format!("--json takes one JSON object: {e}"))?
                    } else if question {
                        TextFields {
                            title: Some(t.to_owned()),
                            ..TextFields::default()
                        }
                    } else {
                        let (title, rest) = t.split_once('\n').unwrap_or((t, ""));
                        TextFields {
                            title: Some(title.trim().to_owned()),
                            description: Some(rest.trim().to_owned()).filter(|d| !d.is_empty()),
                            ..TextFields::default()
                        }
                    };
                    ag.create(CreateArgs {
                        items: vec![CreateItem {
                            kind,
                            title: fields.title.unwrap_or_default(),
                            description: fields.description,
                            epic,
                            topic,
                            from_notes: notes,
                            due,
                            asked_to: fields.asked_to,
                            owner,
                            report_to: fields.report_to,
                            level,
                        }],
                        dismiss: None,
                    })
                }
            }
        }
        Command::Kb {
            action,
            query,
            kind,
            id,
            cursor,
        } => match action {
            None => {
                let query = if query {
                    Some(read_text(&TextSource::default())?.trim().to_owned())
                } else {
                    None
                };
                ag.kb(KbArgs {
                    query,
                    kind,
                    id,
                    cursor,
                })
            }
            Some(KbAction::Read { id, section, lines }) => {
                ag.kb_read(KbReadArgs { id, section, lines })
            }
            Some(KbAction::Write { text }) => {
                let a: KbWriteArgs = serde_json::from_str(&read_text(&text)?)
                    .map_err(|e| format!("kb write takes the kb_write parameters as JSON: {e}"))?;
                ag.kb_write(a)
            }
            Some(KbAction::Import {
                path,
                kind,
                level,
                owner,
            }) => kb_import(&mut ag, &path, &kind, level, owner),
            Some(KbAction::Export { id, path }) => kb_export(&mut ag, &id, &path),
        },
        Command::Attachment {
            id,
            lines,
            pages,
            size,
        } => {
            let answer = ag.attachment(AttachmentArgs {
                id,
                lines,
                pages,
                size,
            })?;
            Ok(match answer.image {
                // A terminal cannot show it; the MCP tool sends it.
                Some(_) => format!(
                    "{}\nThe image itself reaches Claude through the attachment tool of Acta's MCP server.",
                    answer.text
                ),
                None => answer.text,
            })
        }
        Command::Version
        | Command::Mcp { .. }
        | Command::Hook { .. }
        | Command::Setup { .. }
        | Command::Link { .. }
        | Command::Folders { .. }
        | Command::Export { .. }
        | Command::Import { .. }
        | Command::Snapshot { .. }
        | Command::Sessions
        | Command::PdfText { .. }
        | Command::Statusline => Err("handled elsewhere".into()),
    }
}

/// User text read from stdin as one JSON object, with the tools' field
/// names (data.md requests 18 and 19): never from process arguments.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TextFields {
    title: Option<String>,
    description: Option<String>,
    state: Option<String>,
    answer: Option<String>,
    asked_to: Option<String>,
    report_to: Option<String>,
}

fn kb_import(
    ag: &mut Agent<'_>,
    path: &Path,
    kind: &str,
    level: Option<String>,
    owner: Option<String>,
) -> Result<String, String> {
    let body =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read the file: {}", e.kind()))?;
    let kind = match kind {
        "document" | "doc" => KbKind::Document,
        "memory" | "mem" => KbKind::Memory,
        _ => return Err("--kind must be document or memory".into()),
    };
    let title = body
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix('#')
                .map(|h| h.trim_start_matches('#').trim().to_owned())
        })
        .filter(|t| !t.is_empty())
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Imported".into());
    let first = body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or("Imported file, to be summarised.");
    let summary: String = first.chars().take(117).collect();
    let project = || ag.session.project.ok_or(acta_core::agent::NOT_LINKED);
    let owner = match (owner, level.as_deref().map(Level::parse)) {
        (Some(o), _) => {
            acta_core::agent::parse_owner(&o).ok_or("--owner must be T-n, E-n or P-n")?
        }
        (None, None | Some(Some(Level::Project))) => Owner::Project(project()?),
        (None, Some(Some(Level::Topic))) => match ag.session.focus {
            Some(f @ Owner::Topic(_)) => f,
            _ => return Err("no focus topic; pass --owner".into()),
        },
        (None, Some(Some(Level::Epic))) => match ag.session.focus {
            Some(f @ Owner::Epic(_)) => f,
            _ => return Err("no focus epic; pass --owner".into()),
        },
        (None, Some(None)) => return Err("--level must be topic, epic or project".into()),
    };
    // A file the human imports is not limited like an agent's memory: ADR
    // 0005 has an old MEMORY.md imported whole for the human to split.
    let id = ag
        .store
        .kb_create(
            NewKb {
                owner,
                kind,
                title,
                summary,
                body,
                from_file: true,
            },
            ag.session.author,
        )
        .map_err(|e| e.to_string())?;
    Ok(format!("ok {id} {} · {owner} · rev 1", kind.as_str()))
}

fn kb_export(ag: &mut Agent<'_>, id: &str, path: &Path) -> Result<String, String> {
    let k = acta_core::model::KbId::parse(id).ok_or("id must be K-n")?;
    let e = ag.store.kb_entry(k).map_err(|e| e.to_string())?;
    write_new(path, &e.body)?;
    Ok(format!("ok {k} rev {} saved", e.rev))
}

/// Writes a file that must not exist yet: the check and the creation are
/// one operation (`create_new`), so nothing already there is overwritten.
/// The file is readable by the user only: an export is plaintext.
fn write_new(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut f = acta_core::fsutil::create_new_private_file(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => "the file exists; choose a new path".to_owned(),
        k => format!("cannot write: {k}"),
    })?;
    f.write_all(text.as_bytes())
        .and_then(|()| f.sync_all())
        .map_err(|e| format!("cannot write: {}", e.kind()))
}

fn export(
    store: &mut acta_core::Store,
    path: &Path,
    project: Option<String>,
    include_files: bool,
) -> Result<String, String> {
    // An export writes every stored text to a plaintext file.
    if author() == Author::Claude {
        return Err("Export is for you to run, not Claude Code.".into());
    }
    let project = project
        .map(|p| ProjectId::parse(&p).ok_or("--project takes P-n"))
        .transpose()?;
    let file = store
        .export_with(project, include_files)
        .map_err(|e| e.to_string())?;
    let text = file.to_json().map_err(|e| e.to_string())?;
    write_new(path, &text)?;
    store.record_export().map_err(|e| e.to_string())?;
    let files = match (file.attachments.len(), include_files) {
        (0, _) => String::new(),
        (n, true) => format!(", {n} files"),
        (n, false) => format!(", {n} files (names only)"),
    };
    Ok(format!(
        "ok exported {} projects, {} topics, {} entries, {} knowledge entries{files}. The file is plaintext: keep it somewhere private.",
        file.projects.len(),
        file.topics.len(),
        file.entries.len(),
        file.knowledge.len()
    ))
}

fn import(store: &mut acta_core::Store, path: &Path, dry_run: bool) -> Result<String, String> {
    // An import can set what only the user sets (commit modes, active
    // folders: ADR 0011); the store also guards it.
    if author() == Author::Claude {
        return Err("Import is for you to run, not Claude Code.".into());
    }
    let file = ExportFile::read(path).map_err(|e| match e {
        acta_core::Error::Io(e) => format!("cannot read the file: {}", e.kind()),
        e => e.to_string(),
    })?;
    let report = store.import(&file, dry_run).map_err(|e| e.to_string())?;
    let t = report.total();
    Ok(format!(
        "{} {} created, {} updated, {} skipped{}{}",
        if dry_run {
            "would import:"
        } else {
            "ok imported:"
        },
        t.created,
        t.updated,
        t.skipped,
        if dry_run {
            " (nothing written)"
        } else {
            " (a snapshot was taken first)"
        },
        match report.folders_refused {
            0 => String::new(),
            1 => "; 1 folder on a network path left out".to_owned(),
            n => format!("; {n} folders on a network path left out"),
        }
    ) + &match report.files_not_in_export.len() {
        0 => String::new(),
        1 => "; 1 file not in the export".to_owned(),
        n => format!("; {n} files not in the export"),
    })
}

fn snapshot(store: &mut acta_core::Store, action: SnapshotAction) -> Result<String, String> {
    let name = |s: &acta_core::snapshots::Snapshot| {
        s.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut all = store.snapshots().map_err(|e| e.to_string())?;
    all.sort_by_key(|s| std::cmp::Reverse(s.taken_at));
    match action {
        SnapshotAction::List => {
            if all.is_empty() {
                return Ok("no snapshots yet".into());
            }
            Ok(all
                .iter()
                .map(|s| format!("{}  {}", name(s), s.kind.as_str()))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        SnapshotAction::Restore { name: wanted } => {
            if author() == Author::Claude {
                return Err("Restoring a snapshot is for you to run, not Claude Code.".into());
            }
            let pick = if wanted == "latest" {
                all.first()
            } else {
                all.iter().find(|s| name(s) == wanted)
            }
            .ok_or("no such snapshot; see acta snapshot list")?;
            let picked = name(pick);
            let path = pick.path.clone();
            store.restore_snapshot(&path).map_err(|e| e.to_string())?;
            Ok(format!(
                "ok restored {picked}; the state before it was snapshotted first"
            ))
        }
    }
}

fn is_help_or_version(err: &clap::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    )
}

fn version_line() -> String {
    format!("acta {}", acta_core::VERSION)
}

/// clap renders an error as several lines (message, usage, hint). Agents and
/// hooks read this output, so keep the message and point at `--help`.
fn one_line(rendered: &str) -> String {
    let first = rendered.lines().next().unwrap_or("invalid arguments");
    let message = first.strip_prefix("error: ").unwrap_or(first);
    format!("acta: {message}. Run 'acta --help' for the commands.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_claude_code_variable_makes_the_author_claude() {
        use std::ffi::OsString;
        let with = |pairs: &'static [(&str, &str)]| {
            author_from(move |k| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| OsString::from(v))
            })
        };
        assert_eq!(with(&[]), Author::Human);
        assert_eq!(with(&[("CLAUDECODE", "0")]), Author::Human);
        assert_eq!(with(&[("CLAUDE_PROJECT_DIR", "")]), Author::Human);
        assert_eq!(with(&[("CLAUDECODE", "1")]), Author::Claude);
        assert_eq!(with(&[("CLAUDE_CODE_ENTRYPOINT", "cli")]), Author::Claude);
        assert_eq!(
            with(&[("CLAUDE_PROJECT_DIR", "/home/alexandre/acta")]),
            Author::Claude
        );
    }

    #[test]
    fn version_line_is_name_and_version() {
        assert_eq!(
            version_line(),
            format!("acta {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn version_flag_prints_the_same_line_as_the_subcommand() {
        let err = Cli::try_parse_from(["acta", "--version"]).err().unwrap();
        assert!(is_help_or_version(&err));
        assert_eq!(err.to_string().trim_end(), version_line());
    }

    #[test]
    fn version_subcommand_parses() {
        let cli = Cli::try_parse_from(["acta", "version"]).ok().unwrap();
        assert!(matches!(cli.command, Some(Command::Version)));
    }

    #[test]
    fn commands_parse() {
        for args in [
            vec!["acta", "mcp"],
            vec!["acta", "hook", "session-start"],
            vec!["acta", "setup", "--check", "--json"],
            vec!["acta", "setup", "--check", "--no-prereqs", "--json"],
            vec!["acta", "setup", "--undo"],
            vec!["acta", "link", "--topic", "T-3", "--add"],
            vec!["acta", "brief", "T-1", "--focus"],
            vec!["acta", "log", "decision", "--topic", "T-1"],
            vec!["acta", "kb", "read", "K-2", "--section", "3.1"],
            vec!["acta", "kb", "--query", "--kind", "memory"],
            vec!["acta", "snapshot", "restore", "latest"],
            vec!["acta", "export", "out.acta.json", "--project", "P-1"],
        ] {
            assert!(Cli::try_parse_from(&args).is_ok(), "{args:?}");
        }
    }

    #[test]
    fn unknown_command_is_reported_on_one_line() {
        let err = Cli::try_parse_from(["acta", "frobnicate"]).err().unwrap();
        assert!(!is_help_or_version(&err));
        let line = one_line(&err.to_string());
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains("frobnicate"), "{line}");
    }

    #[test]
    fn clap_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
