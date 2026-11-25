//! `acta hook <event>`: the `command` hooks of the plugin (ADR 0005,
//! section 6).
//!
//! Fast path first: unless the launch is pinned by `ACTA_*` variables, the
//! launch directory is checked against the salted-hash pre-check file, and a
//! miss exits silently before the keychain or the database is touched.

use crate::open::Opener;
use acta_core::agent::{Agent, Launch, PRE_COMPACT, Session, hook_json};
use acta_core::claude_settings::page;
use acta_core::model::Author;
use acta_core::paths::Os;
use acta_core::{folder_path, git_guard};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    SessionStart,
    PreCompact,
    /// `PreToolUse` on `git commit` / `git push` (ADR 0011, the guard).
    PreGit,
}

impl Event {
    pub fn parse(s: &str) -> Option<Event> {
        match s {
            "session-start" => Some(Event::SessionStart),
            "pre-compact" => Some(Event::PreCompact),
            "pre-git" => Some(Event::PreGit),
            _ => None,
        }
    }
}

/// The fields of the hook's stdin JSON that Acta uses.
#[derive(Default, serde::Deserialize)]
struct Input {
    session_id: Option<String>,
    cwd: Option<String>,
    source: Option<String>,
    tool_input: Option<ToolInput>,
}

#[derive(Default, serde::Deserialize)]
struct ToolInput {
    command: Option<String>,
}

/// What the hook prints on stdout; `None` prints nothing.
pub fn run(event: Event, stdin: &str, opener: &Opener) -> Option<String> {
    let input: Input = serde_json::from_str(stdin).unwrap_or_default();
    // The guard's handlers match every git call and the wrappers that may
    // hide one: most need nothing, decided before any file is read.
    let command = input.tool_input.as_ref().and_then(|t| t.command.clone());
    if event == Event::PreGit && !command.as_deref().is_some_and(git_guard::needs_check) {
        return None;
    }
    let env = Launch::from_env();
    let launch = Launch {
        dir: input.cwd.map(PathBuf::from).or(env.dir),
        session_id: input.session_id.or(env.session_id),
        ..env
    };
    let data_dir = opener.data_dir()?;
    if event == Event::PreGit {
        // The session's folder holds the allow rules: it counts as much as
        // the directory the shell is in now.
        let project = std::env::var_os("CLAUDE_PROJECT_DIR").map(PathBuf::from);
        let in_project = Launch {
            dir: project.clone(),
            ..Launch::default()
        };
        if !launch.worth_opening(&data_dir) && !in_project.worth_opening(&data_dir) {
            return None;
        }
        let cwd = launch.dir?;
        let session = project.unwrap_or_else(|| cwd.clone());
        // The session's page comes from its launch token alone: `ACTA_TOPIC`
        // and the like say where to focus, not what may be committed.
        return pre_git(&command?, &cwd, &session, launch.token.as_deref(), opener);
    }
    if !launch.worth_opening(&data_dir) {
        return None;
    }
    match event {
        Event::PreGit => None,
        Event::PreCompact => Some(PRE_COMPACT.to_owned()),
        Event::SessionStart => {
            let mut store = opener.open().ok()?;
            let mut session = Session::resolve(&store, &launch, Author::Claude);
            let agent = Agent {
                store: &mut store,
                session: &mut session,
            };
            let compact = input.source.as_deref() == Some("compact");
            let text = agent.session_briefing(compact)?;
            Some(hook_json("SessionStart", &text))
        }
    }
}

/// The guard, in a session the pre-check links to Acta: each commit or push
/// is judged in the folder it acts in, with the commit settings of the
/// session's page (ADR 0011 addendum 2026-11-23). It answers deny, ask or
/// nothing, never allow; when Acta cannot read the folders, it denies (fail
/// closed).
fn pre_git(
    command: &str,
    cwd: &Path,
    session: &Path,
    token: Option<&str>,
    opener: &Opener,
) -> Option<String> {
    let Ok(store) = opener.open() else {
        return Some(git_guard::fail_closed());
    };
    let Ok(page) = page::session_page(&store, token) else {
        return Some(git_guard::fail_closed());
    };
    let place = |dir: &Path| place(&store, &page, dir);
    let alias = |dir: &Path, name: &str| match acta_core::git_cmd::alias(dir, name) {
        Ok(Some(v)) => git_guard::Alias::Value(v),
        Ok(None) => git_guard::Alias::None,
        Err(_) => git_guard::Alias::Failed,
    };
    let cx = git_guard::Context {
        cwd,
        session,
        place: &place,
        alias: &alias,
    };
    git_guard::judge(command, &cx).map(|v| v.json())
}

/// The attached folder containing `dir`, with the commit settings the
/// session's page gives it and its git state.
fn place(store: &acta_core::Store, page: &page::SessionPage, dir: &Path) -> git_guard::Place {
    use git_guard::Place;
    let stored = folder_path::canonical(dir)
        .unwrap_or_else(|_| folder_path::normalize(Os::CURRENT, &dir.to_string_lossy()));
    match page::guard_view(store, &stored, page) {
        Err(_) => Place::Unreadable,
        Ok(None) => Place::Unlinked,
        Ok(Some(v)) => Place::Folder {
            name: v.folder.name,
            settings: v.folder.settings,
            git: acta_core::agent::git::inspect(dir).map(Box::new),
            permitted: v.permitted,
            granted: v.granted,
        },
    }
}

/// Reads at most 1 MiB of stdin: hook payloads are small, and a hook must
/// never hang on a stream that does not end.
pub fn read_stdin() -> String {
    let mut buf = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut buf);
    buf
}
