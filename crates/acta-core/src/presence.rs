//! Session presence (docs/design/data.md request 2, ADR 0005 amendment): which
//! Claude Code sessions are live, where they are focused and what they are
//! doing, so the app can show it.
//!
//! Presence lives in its own encrypted database, `presence.db`, next to the
//! main one and opened with the same key. It is local and ephemeral: never
//! exported, never in a snapshot, deleted and recreated when it cannot be
//! read. Keeping it apart means the heartbeats of running sessions never wake
//! the app's watcher on the main database (every commit there reloads every
//! page), and the app watches this file on its own.
//!
//! One row per Claude Code session, written only by that session's `acta mcp`
//! process (at start, on a heartbeat, on Acta's `mcp_tool` hooks and when the
//! agent writes to Acta). What a row may hold is fixed: the opaque session id,
//! the focus ids, an opaque launch token, times, a coarse state, the name of
//! the last tool, and a one-line activity taken from what the agent itself
//! wrote to Acta (a `log` text, a knowledge title). Never prompt text, tool
//! input or output, or file content.

use crate::db::{self, ChangeWatcher, Mode};
use crate::error::{OpenError, Result};
use crate::keys::Key;
use crate::model::{EpicId, Owner, ProjectId, TopicId};
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub const PRESENCE_FILE: &str = "presence.db";

/// A live session writes `alive_at` this often.
pub const HEARTBEAT_MS: i64 = 30_000;
/// A session whose heartbeat is older than this is gone (crashed, killed,
/// machine asleep): three missed beats.
pub const STALE_MS: i64 = 3 * HEARTBEAT_MS;
/// Rows that ended or went stale are deleted after this.
const PRUNE_MS: i64 = crate::time::MS_PER_DAY;
/// State changes are written at once; the same state again (a run of tool
/// calls) at most this often.
pub const THROTTLE_MS: i64 = 15_000;
/// The activity line is cut to this many characters.
pub const ACTIVITY_CHARS: usize = 100;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    project_id INTEGER NOT NULL,
    focus TEXT,
    launch TEXT,
    started_at INTEGER NOT NULL,
    activity_at INTEGER NOT NULL,
    alive_at INTEGER NOT NULL,
    state TEXT NOT NULL,
    tool TEXT,
    activity TEXT,
    ended_at INTEGER
) STRICT;
";

/// What a session is doing, coarse on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// A prompt was submitted, a tool ran or a subagent started.
    Working,
    /// Blocked on the human: a permission prompt or a question.
    Waiting,
    /// The turn ended; nothing runs.
    Idle,
    Ended,
}

impl SessionState {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionState::Working => "working",
            SessionState::Waiting => "waiting",
            SessionState::Idle => "idle",
            SessionState::Ended => "ended",
        }
    }

    pub fn parse(s: &str) -> Option<SessionState> {
        Some(match s {
            "working" => SessionState::Working,
            "waiting" => SessionState::Waiting,
            "idle" => SessionState::Idle,
            "ended" => SessionState::Ended,
            _ => return None,
        })
    }
}

/// One session as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveSession {
    /// Claude Code's session id: opaque, compared only.
    pub id: String,
    pub project: ProjectId,
    /// A topic or an epic; `None` is the project itself.
    pub focus: Option<Owner>,
    /// The token of the Acta terminal that started it (`ACTA_LAUNCH`).
    pub launch: Option<String>,
    pub started_at: i64,
    pub activity_at: i64,
    pub alive_at: i64,
    pub state: SessionState,
    /// Short name of the last tool (`Bash`, `Edit`, `acta log`).
    pub tool: Option<String>,
    /// One line from what the agent wrote to Acta.
    pub activity: Option<String>,
}

impl LiveSession {
    /// Whether the session counts as running at `now`.
    pub fn is_live(&self, now: i64) -> bool {
        self.state != SessionState::Ended && now - self.alive_at < STALE_MS
    }
}

/// What a session registers with when it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewSession {
    pub id: String,
    pub project: ProjectId,
    pub focus: Option<Owner>,
    pub launch: Option<String>,
}

/// A connection to `presence.db`.
pub struct Presence {
    conn: Connection,
    path: PathBuf,
    key: Key,
}

impl Presence {
    /// Opens `presence.db` in `data_dir` with the database key, creating it.
    /// A file the key does not open (left by an earlier key) is deleted and
    /// created again: presence is rebuilt by the sessions within a minute.
    pub(crate) fn open(data_dir: &Path, key: &Key) -> Result<Presence, OpenError> {
        let path = data_dir.join(PRESENCE_FILE);
        let conn = match db::connect(&path, key, Mode::Live { create: true }) {
            Ok(c) => c,
            Err(OpenError::KeyWrongOrCorrupt { .. }) => {
                for suffix in ["", "-wal", "-shm"] {
                    let mut p = path.clone().into_os_string();
                    p.push(suffix);
                    let _ = std::fs::remove_file(PathBuf::from(p));
                }
                db::connect(&path, key, Mode::Live { create: true })?
            }
            Err(e) => return Err(e),
        };
        conn.execute_batch(SCHEMA)?;
        Ok(Presence {
            conn,
            path,
            key: key.clone(),
        })
    }

    /// A watcher on its own connection, for the app.
    pub fn watcher(&self) -> Result<ChangeWatcher> {
        let conn = db::connect(&self.path, &self.key, Mode::Live { create: false })
            .map_err(crate::store::open_to_error)?;
        Ok(ChangeWatcher::new(conn)?)
    }

    /// Starts (or restarts, for the same id) a session.
    pub fn register(&mut self, s: &NewSession, now: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, project_id, focus, launch, started_at, activity_at,
                 alive_at, state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5, 'idle')
             ON CONFLICT (id) DO UPDATE SET project_id = excluded.project_id,
                 focus = excluded.focus, launch = coalesce(excluded.launch, launch),
                 alive_at = excluded.alive_at, state = 'idle', ended_at = NULL",
            rusqlite::params![
                s.id,
                s.project.0,
                s.focus.map(|f| f.to_string()),
                s.launch,
                now
            ],
        )?;
        Ok(())
    }

    pub fn heartbeat(&mut self, id: &str, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET alive_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
            rusqlite::params![id, now],
        )?;
        Ok(())
    }

    /// A new state; `tool` replaces the last tool name when given.
    pub fn set_state(
        &mut self,
        id: &str,
        state: SessionState,
        tool: Option<&str>,
        now: i64,
    ) -> Result<()> {
        let ended = (state == SessionState::Ended).then_some(now);
        self.conn.execute(
            "UPDATE sessions SET state = ?2, tool = coalesce(?3, tool), activity_at = ?4,
                 alive_at = ?4, ended_at = ?5
             WHERE id = ?1",
            rusqlite::params![id, state.as_str(), tool, now, ended],
        )?;
        Ok(())
    }

    /// The activity line, cut to one line of [`ACTIVITY_CHARS`].
    pub fn set_activity(&mut self, id: &str, text: &str, now: i64) -> Result<()> {
        let line = activity_line(text);
        if line.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "UPDATE sessions SET activity = ?2, activity_at = ?3, alive_at = ?3 WHERE id = ?1",
            rusqlite::params![id, line, now],
        )?;
        Ok(())
    }

    pub fn set_focus(&mut self, id: &str, project: ProjectId, focus: Option<Owner>) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET project_id = ?2, focus = ?3 WHERE id = ?1",
            rusqlite::params![id, project.0, focus.map(|f| f.to_string())],
        )?;
        Ok(())
    }

    /// Claude Code gave the same process a new session id (`/clear`).
    pub fn rename(&mut self, old: &str, new: &str) -> Result<()> {
        if old == new {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM sessions WHERE id = ?1", [new])?;
        tx.execute(
            "UPDATE sessions SET id = ?2 WHERE id = ?1",
            rusqlite::params![old, new],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<LiveSession>> {
        Ok(self
            .conn
            .query_row(&format!("{SELECT} WHERE id = ?1"), [id], row)
            .optional()?)
    }

    /// The live sessions, newest activity first. Rows ended or stale for
    /// a day are deleted on the way.
    pub fn live(&mut self, now: i64) -> Result<Vec<LiveSession>> {
        self.conn.execute(
            "DELETE FROM sessions WHERE coalesce(ended_at, alive_at) < ?1",
            [now - PRUNE_MS],
        )?;
        let mut stmt = self
            .conn
            .prepare(&format!("{SELECT} ORDER BY activity_at DESC, id"))?;
        let rows = stmt.query_map([], row)?;
        let mut out = Vec::new();
        for r in rows {
            let r = r?;
            if r.is_live(now) {
                out.push(r);
            }
        }
        Ok(out)
    }
}

const SELECT: &str = "SELECT id, project_id, focus, launch, started_at, activity_at, alive_at,
    state, tool, activity FROM sessions";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LiveSession> {
    let focus: Option<String> = r.get(2)?;
    let state: String = r.get(7)?;
    Ok(LiveSession {
        id: r.get(0)?,
        project: ProjectId(r.get(1)?),
        focus: focus.as_deref().and_then(parse_focus),
        launch: r.get(3)?,
        started_at: r.get(4)?,
        activity_at: r.get(5)?,
        alive_at: r.get(6)?,
        state: SessionState::parse(&state).unwrap_or(SessionState::Idle),
        tool: r.get(8)?,
        activity: r.get(9)?,
    })
}

fn parse_focus(s: &str) -> Option<Owner> {
    if let Some(t) = TopicId::parse(s) {
        return Some(Owner::Topic(t));
    }
    EpicId::parse(s).map(Owner::Epic)
}

/// The first non-empty line of `text`, whitespace collapsed, cut at
/// [`ACTIVITY_CHARS`] with an ellipsis.
pub fn activity_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let line: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= ACTIVITY_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(ACTIVITY_CHARS - 1).collect();
    cut.push('\u{2026}');
    cut
}

/// A tool name as people read it: `mcp__plugin_acta_acta__log` is
/// `acta log`, `mcp__github__create_issue` is `github create_issue`, a
/// built-in tool keeps its name. At most 40 characters, no control
/// characters (the name comes from the hook payload).
pub fn short_tool(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name.starts_with("${") {
        return None;
    }
    let short = match name.strip_prefix("mcp__") {
        Some(rest) => {
            let (server, tool) = rest.split_once("__").unwrap_or((rest, ""));
            let server = server
                .strip_prefix("plugin_")
                .map(|s| s.split_once('_').map_or(s, |(_, srv)| srv))
                .unwrap_or(server);
            format!("{server} {tool}").trim().to_owned()
        }
        None => name.to_owned(),
    };
    let clean: String = short.chars().filter(|c| !c.is_control()).take(40).collect();
    (!clean.is_empty()).then_some(clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;

    fn open() -> (tempfile::TempDir, Presence) {
        let dir = tempfile::tempdir().unwrap();
        let key = Key::generate().unwrap();
        let p = Presence::open(dir.path(), &key).unwrap();
        (dir, p)
    }

    fn new(id: &str) -> NewSession {
        NewSession {
            id: id.into(),
            project: ProjectId(1),
            focus: Some(Owner::Topic(TopicId(12))),
            launch: Some("launch-a".into()),
        }
    }

    #[test]
    fn a_session_lives_while_it_beats_and_ends_or_goes_stale() {
        let (_d, mut p) = open();
        p.register(&new("s1"), 1_000).unwrap();
        let live = p.live(1_000).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].state, SessionState::Idle);
        assert_eq!(live[0].focus, Some(Owner::Topic(TopicId(12))));
        assert_eq!(live[0].launch.as_deref(), Some("launch-a"));

        // No beat for STALE_MS: gone from the live list, row kept.
        assert!(p.live(1_000 + STALE_MS).unwrap().is_empty());
        assert!(p.get("s1").unwrap().is_some());
        // A beat brings it back.
        p.heartbeat("s1", 1_000 + STALE_MS).unwrap();
        assert_eq!(p.live(1_000 + STALE_MS).unwrap().len(), 1);

        p.set_state("s1", SessionState::Ended, None, 2_000 + STALE_MS)
            .unwrap();
        assert!(p.live(2_000 + STALE_MS).unwrap().is_empty());
        // Ended rows ignore beats, and are pruned after a day.
        p.heartbeat("s1", 3_000 + STALE_MS).unwrap();
        assert!(p.live(3_000 + STALE_MS).unwrap().is_empty());
        p.live(3_000 + STALE_MS + PRUNE_MS).unwrap();
        assert!(p.get("s1").unwrap().is_none());
    }

    #[test]
    fn states_tools_activity_focus_and_rename() {
        let (_d, mut p) = open();
        p.register(&new("s1"), 0).unwrap();
        p.set_state("s1", SessionState::Working, Some("Bash"), 10)
            .unwrap();
        p.set_state("s1", SessionState::Waiting, None, 20).unwrap();
        p.set_activity("s1", "\n  WAL on;   GUI and CLI write\nsecond line", 30)
            .unwrap();
        p.set_focus("s1", ProjectId(1), Some(Owner::Epic(EpicId(3))))
            .unwrap();
        p.rename("s1", "s2").unwrap();
        assert!(p.get("s1").unwrap().is_none());
        let s = p.get("s2").unwrap().unwrap();
        assert_eq!(s.state, SessionState::Waiting);
        assert_eq!(s.tool.as_deref(), Some("Bash"));
        assert_eq!(s.activity.as_deref(), Some("WAL on; GUI and CLI write"));
        assert_eq!(s.focus, Some(Owner::Epic(EpicId(3))));
        assert_eq!(s.activity_at, 30);
        // A restart with the same id comes back idle, launch kept.
        p.register(
            &NewSession {
                launch: None,
                ..new("s2")
            },
            40,
        )
        .unwrap();
        let s = p.get("s2").unwrap().unwrap();
        assert_eq!(s.state, SessionState::Idle);
        assert_eq!(s.launch.as_deref(), Some("launch-a"));
    }

    #[test]
    fn a_file_from_another_key_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Presence::open(dir.path(), &Key::generate().unwrap()).unwrap();
        p.register(&new("s1"), 0).unwrap();
        drop(p);
        let mut p = Presence::open(dir.path(), &Key::generate().unwrap()).unwrap();
        assert!(p.live(0).unwrap().is_empty());
    }

    #[test]
    fn activity_and_tool_names_are_short_and_plain() {
        let long = "x".repeat(300);
        assert_eq!(activity_line(&long).chars().count(), ACTIVITY_CHARS);
        assert_eq!(activity_line("  \n\n"), "");
        assert_eq!(
            short_tool("mcp__plugin_acta_acta__log").as_deref(),
            Some("acta log")
        );
        assert_eq!(
            short_tool("mcp__plugin_acta_browser__open").as_deref(),
            Some("browser open")
        );
        assert_eq!(
            short_tool("mcp__github__create_issue").as_deref(),
            Some("github create_issue")
        );
        assert_eq!(short_tool("Bash").as_deref(), Some("Bash"));
        assert_eq!(short_tool("${tool_name}"), None);
        assert_eq!(short_tool(""), None);
        assert_eq!(short_tool("a\u{7}b").as_deref(), Some("ab"));
    }
}
