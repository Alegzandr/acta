//! What one `acta mcp` process reports about its Claude Code session
//! (session presence, `crate::presence`): registered when the server starts
//! in a linked folder, beaten every [`HEARTBEAT_MS`], moved by Acta's
//! `mcp_tool` hooks, and ended when Claude Code ends the session or the
//! server's stdin closes.
//!
//! Hook events, as the plugin's `hooks.json` sends them to the `hook` tool:
//!
//! | Hook | `event` | State |
//! |---|---|---|
//! | `UserPromptSubmit` | `prompt` | working |
//! | `PostToolUse` | `tool` (detail: `${tool_name}`) | working, last tool |
//! | `SubagentStart` | `subagent` | working (and the subagent note) |
//! | `Notification` (permission or question) | `waiting` | waiting |
//! | `Stop` | `stop` | idle |
//! | `SessionEnd` | `end` (detail: `${reason}`) | ended, except reason `clear` |

use super::session::Session;
use crate::presence::{NewSession, Presence, SessionState, THROTTLE_MS, activity_line, short_tool};
use crate::store::Store;

pub use crate::presence::HEARTBEAT_MS;

/// A presence hook event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Prompt,
    Tool(Option<String>),
    Subagent,
    Waiting,
    Stop,
    End,
    /// `SessionEnd` because of `/clear`: the process goes on under a new id.
    EndForClear,
}

impl Event {
    /// The presence events of the `hook` tool; `None` for the briefing
    /// events (`clear`, `compact`, `subagent` is both).
    pub fn parse(event: &str, detail: Option<&str>) -> Option<Event> {
        Some(match event {
            "prompt" => Event::Prompt,
            "tool" => Event::Tool(detail.and_then(short_tool)),
            "waiting" => Event::Waiting,
            "stop" => Event::Stop,
            "end" if detail == Some("clear") => Event::EndForClear,
            "end" => Event::End,
            _ => return None,
        })
    }

    fn state(&self) -> Option<SessionState> {
        match self {
            Event::Prompt | Event::Tool(_) | Event::Subagent => Some(SessionState::Working),
            Event::Waiting => Some(SessionState::Waiting),
            Event::Stop => Some(SessionState::Idle),
            Event::End => Some(SessionState::Ended),
            Event::EndForClear => None,
        }
    }
}

pub struct Tracker {
    presence: Presence,
    id: String,
    state: SessionState,
    written_at: i64,
    /// The knowledge touches last written (ADR 0015, 6).
    kb_written: Vec<crate::presence::KbTouch>,
}

impl Tracker {
    /// Registers the session. `None` when it is bound to no project (the
    /// folder is not linked) or has no id yet.
    pub fn start(
        store: &Store,
        session: &Session,
        token: Option<String>,
        now: i64,
    ) -> crate::Result<Option<Tracker>> {
        let (Some(project), Some(id)) = (session.project, session.session_id.clone()) else {
            return Ok(None);
        };
        let mut presence = store.presence()?;
        presence.register(
            &NewSession {
                id: id.clone(),
                project,
                focus: session.focus,
                launch: token,
            },
            now,
        )?;
        Ok(Some(Tracker {
            presence,
            id,
            state: SessionState::Idle,
            written_at: now,
            kb_written: Vec::new(),
        }))
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Applies a hook event. The same state again within [`THROTTLE_MS`]
    /// is not written (a run of tool calls costs one write).
    pub fn on_event(&mut self, event: &Event, now: i64) -> crate::Result<()> {
        let Some(state) = event.state() else {
            return Ok(());
        };
        if self.state == SessionState::Ended && state != SessionState::Ended {
            // A late hook after the end does not bring the row back.
            return Ok(());
        }
        if state == self.state && now - self.written_at < THROTTLE_MS {
            return Ok(());
        }
        let tool = match event {
            Event::Tool(t) => t.as_deref(),
            _ => None,
        };
        self.presence.set_state(&self.id, state, tool, now)?;
        self.state = state;
        self.written_at = now;
        Ok(())
    }

    pub fn heartbeat(&mut self, now: i64) -> crate::Result<()> {
        if self.state == SessionState::Ended {
            return Ok(());
        }
        self.presence.heartbeat(&self.id, now)
    }

    /// The agent wrote to Acta: its words become the activity line.
    pub fn activity(&mut self, text: &str, now: i64) -> crate::Result<()> {
        if activity_line(text).is_empty() || self.state == SessionState::Ended {
            return Ok(());
        }
        self.presence.set_activity(&self.id, text, now)?;
        self.written_at = now;
        Ok(())
    }

    /// Writes the session's knowledge touches when they changed.
    pub fn kb_recent(&mut self, session: &Session) -> crate::Result<()> {
        if session.kb.touched == self.kb_written || self.state == SessionState::Ended {
            return Ok(());
        }
        self.presence.set_kb_recent(&self.id, &session.kb.touched)?;
        self.kb_written = session.kb.touched.clone();
        Ok(())
    }

    /// The session's focus or project moved (`brief(focus: true)`).
    pub fn follow(&mut self, session: &Session) -> crate::Result<()> {
        if let Some(project) = session.project {
            self.presence.set_focus(&self.id, project, session.focus)?;
        }
        Ok(())
    }

    /// `/clear`: same process, new session id; the row follows, live again.
    pub fn adopt(&mut self, id: &str, now: i64) -> crate::Result<()> {
        if id.is_empty() || id == self.id {
            return Ok(());
        }
        self.presence.rename(&self.id, id)?;
        self.id = id.to_owned();
        self.presence.set_state(id, SessionState::Idle, None, now)?;
        self.state = SessionState::Idle;
        self.written_at = now;
        Ok(())
    }

    /// The server is going away: the session ended.
    pub fn end(&mut self, now: i64) -> crate::Result<()> {
        self.on_event(&Event::End, now)
    }
}

/// `acta sessions`: one line per live session, newest activity first,
/// under the agent budget: `T-12 working · Embed the terminal · 4 min ·
/// last: WAL on · in Acta`.
pub fn render_sessions(store: &Store, now: i64) -> crate::Result<String> {
    use super::render::{self, Text, budget};
    use crate::model::Owner;
    let mut presence = store.presence()?;
    let live = presence.live(now)?;
    if live.is_empty() {
        return Ok("no Claude Code session is running in a linked folder".into());
    }
    let mut out = Text::new(budget::HARD);
    out.line(format!("{} running", live.len()));
    for s in &live {
        let (id, title) = match s.focus {
            Some(Owner::Topic(t)) => (t.to_string(), store.topic(t).map(|x| x.title).ok()),
            Some(Owner::Epic(e)) => (e.to_string(), store.epic(e).map(|x| x.title).ok()),
            _ => (
                s.project.to_string(),
                store.project(s.project).map(|x| x.title).ok(),
            ),
        };
        let mut parts = vec![format!("{id} {}", s.state.as_str())];
        if let Some(t) = title {
            parts.push(render::clip(&render::one_line(&t), 60));
        }
        parts.push(format!("{} min", (now - s.started_at).max(0) / 60_000));
        if let Some(a) = &s.activity {
            parts.push(format!("last: {a}"));
        } else if let Some(t) = &s.tool {
            parts.push(format!("last tool: {t}"));
        }
        parts.push(
            if s.launch.is_some() {
                "in Acta"
            } else {
                "outside Acta"
            }
            .into(),
        );
        if !out.line(parts.join(" \u{b7} ")) {
            break;
        }
    }
    Ok(out.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Author, Owner, TopicId};
    use crate::presence::STALE_MS;
    use crate::store::test_support;

    fn bound(session_id: &str, project: crate::model::ProjectId) -> Session {
        let mut s = Session::unbound(Author::Claude);
        s.project = Some(project);
        s.focus = Some(Owner::Topic(TopicId(7)));
        s.session_id = Some(session_id.into());
        s
    }

    #[test]
    fn an_unbound_session_registers_nothing() {
        let ts = test_support::open();
        let s = Session::unbound(Author::Claude);
        assert!(Tracker::start(&ts.store, &s, None, 0).unwrap().is_none());
        assert!(ts.store.presence().unwrap().live(0).unwrap().is_empty());
    }

    #[test]
    fn hook_events_move_the_state_and_writes_are_throttled() {
        let mut ts = test_support::open();
        let p = ts.store.create_project("Acta", "", Author::Human).unwrap();
        let mut t = Tracker::start(&ts.store, &bound("s1", p), Some("tok".into()), 0)
            .unwrap()
            .unwrap();
        let read =
            |ts: &test_support::TestStore, now| ts.store.presence().unwrap().live(now).unwrap();
        let live = read(&ts, 0);
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].launch.as_deref(), Some("tok"));
        assert_eq!(live[0].project, p);

        t.on_event(&Event::Prompt, 1_000).unwrap();
        assert_eq!(read(&ts, 1_000)[0].state, SessionState::Working);
        // Same state within the throttle: the tool name is not written.
        t.on_event(&Event::Tool(Some("Bash".into())), 2_000)
            .unwrap();
        assert_eq!(read(&ts, 2_000)[0].tool, None);
        t.on_event(&Event::Tool(Some("Edit".into())), 1_000 + THROTTLE_MS)
            .unwrap();
        assert_eq!(read(&ts, 20_000)[0].tool.as_deref(), Some("Edit"));
        t.on_event(&Event::Waiting, 20_001).unwrap();
        assert_eq!(read(&ts, 20_001)[0].state, SessionState::Waiting);
        t.on_event(&Event::Stop, 20_002).unwrap();
        assert_eq!(read(&ts, 20_002)[0].state, SessionState::Idle);

        t.activity("Logged: WAL on\nmore", 20_003).unwrap();
        assert_eq!(
            read(&ts, 20_003)[0].activity.as_deref(),
            Some("Logged: WAL on")
        );

        // /clear: SessionEnd(clear) is ignored, the row follows the new id.
        t.on_event(&Event::parse("end", Some("clear")).unwrap(), 20_004)
            .unwrap();
        t.adopt("s2", 20_005).unwrap();
        let live = read(&ts, 20_005);
        assert_eq!((live.len(), live[0].id.as_str()), (1, "s2"));

        // Heartbeats keep it live; without them it expires.
        t.heartbeat(20_005 + STALE_MS - 1).unwrap();
        assert_eq!(read(&ts, 20_005 + STALE_MS).len(), 1);
        assert!(read(&ts, 20_005 + 3 * STALE_MS).is_empty());

        // The end is final: late hooks do not revive it.
        t.end(30_000).unwrap();
        t.on_event(&Event::Prompt, 30_001).unwrap();
        t.heartbeat(30_002).unwrap();
        assert!(read(&ts, 30_002).is_empty());
    }

    #[test]
    fn events_parse_from_the_hook_tool() {
        assert_eq!(Event::parse("prompt", None), Some(Event::Prompt));
        assert_eq!(
            Event::parse("tool", Some("mcp__plugin_acta_acta__log")),
            Some(Event::Tool(Some("acta log".into())))
        );
        assert_eq!(Event::parse("end", Some("logout")), Some(Event::End));
        assert_eq!(Event::parse("end", Some("clear")), Some(Event::EndForClear));
        assert_eq!(Event::parse("clear", None), None);
        assert_eq!(Event::parse("subagent", None), None);
    }
}
