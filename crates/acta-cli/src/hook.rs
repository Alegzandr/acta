//! `acta hook <event>`: the `command` hooks of the plugin (ADR 0005,
//! section 6).
//!
//! Fast path first: unless the launch is pinned by `ACTA_*` variables, the
//! launch directory is checked against the salted-hash pre-check file, and a
//! miss exits silently before the keychain or the database is touched.

use crate::open::Opener;
use acta_core::agent::{Agent, Launch, PRE_COMPACT, Session, hook_json};
use acta_core::model::Author;
use std::io::Read;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    SessionStart,
    PreCompact,
}

impl Event {
    pub fn parse(s: &str) -> Option<Event> {
        match s {
            "session-start" => Some(Event::SessionStart),
            "pre-compact" => Some(Event::PreCompact),
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
}

/// What the hook prints on stdout; `None` prints nothing.
pub fn run(event: Event, stdin: &str, opener: &Opener) -> Option<String> {
    let input: Input = serde_json::from_str(stdin).unwrap_or_default();
    let env = Launch::from_env();
    let launch = Launch {
        dir: input.cwd.map(PathBuf::from).or(env.dir),
        session_id: input.session_id.or(env.session_id),
        ..env
    };
    if !launch.worth_opening(&opener.data_dir()?) {
        return None;
    }
    match event {
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

/// Reads at most 1 MiB of stdin: hook payloads are small, and a hook must
/// never hang on a stream that does not end.
pub fn read_stdin() -> String {
    let mut buf = String::new();
    let _ = std::io::stdin().take(1 << 20).read_to_string(&mut buf);
    buf
}
