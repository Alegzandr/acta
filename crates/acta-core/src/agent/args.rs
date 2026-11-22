//! Parameters of the agent tools (ADR 0005, section 3). The MCP server takes
//! their JSON schemas as tool input schemas and the CLI fills them from its
//! flags, so both transports validate the same way. Field docs are what the
//! agent reads: short, and they count against the `tools/list` budget.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};

/// Accepts `3`, `"3"` or `"3.1"`.
fn str_or_num<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        S(String),
        I(i64),
        F(f64),
    }
    Ok(match Option::<V>::deserialize(d)? {
        None => None,
        Some(V::S(s)) => Some(s),
        Some(V::I(i)) => Some(i.to_string()),
        Some(V::F(f)) => Some(f.to_string()),
    })
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct BriefArgs {
    /// T-n or E-n. Default: the focus.
    pub id: Option<String>,
    /// Make `id` this session's focus.
    #[serde(default)]
    pub focus: bool,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct EntriesArgs {
    /// Default: the focus topic.
    pub topic: Option<String>,
    /// progress, decision or discussion.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Entries numbered below this one (older).
    pub before: Option<i64>,
    /// Default 5, max 20.
    pub limit: Option<i64>,
    /// One entry in full: T-42#14.
    pub id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct LogArgs {
    /// progress, decision or discussion.
    #[serde(rename = "type")]
    pub kind: String,
    /// What happened and why. Max 1500 chars.
    pub text: String,
    /// New current state: what is true now and next. Max 600 chars.
    pub state: Option<String>,
    /// draft, planned, active, blocked, done or dropped.
    pub status: Option<String>,
    /// Default: the focus topic.
    pub topic: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct UpdateArgs {
    /// T-n, E-n or N-n; P-n for due only.
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Replaces the current state; the old one stays in history.
    pub state: Option<String>,
    /// Topics: draft, planned, active, blocked, done, dropped. Notes: open
    /// (back to the inbox) or dismissed.
    pub status: Option<String>,
    /// Move a topic: E-n, or "none".
    pub epic: Option<String>,
    /// YYYY-MM-DD, or "" to clear.
    pub due: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct FindArgs {
    pub query: Option<String>,
    /// topic, epic, note or entry. note without query: the inbox.
    pub kind: Option<String>,
    /// A status, or open (default), closed, all.
    pub status: Option<String>,
    /// Only topics of this epic.
    pub epic: Option<String>,
    /// overdue or week: open topics and epics by due date.
    pub due: Option<String>,
    /// Default 8, max 20.
    pub limit: Option<i64>,
    pub cursor: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct CreateItem {
    /// topic, epic, note, or entry (files notes into an existing topic's timeline).
    pub kind: String,
    /// Title; for a note, its text.
    #[serde(default)]
    pub title: String,
    /// Default with from_notes: the notes' text.
    pub description: Option<String>,
    /// Epic of a new topic: E-n, or @k for epic item k of this call.
    pub epic: Option<String>,
    /// entry: the existing topic T-n.
    pub topic: Option<String>,
    /// Open N-ids this item comes from; they leave the inbox, linked to it.
    pub from_notes: Option<Vec<String>>,
    /// topic, epic: YYYY-MM-DD.
    pub due: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct CreateArgs {
    /// Max 10.
    #[serde(default)]
    pub items: Vec<CreateItem>,
    /// N-ids to close without filing: duplicates, done, noise.
    pub dismiss: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct KbArgs {
    /// Search text; without it, list.
    pub query: Option<String>,
    /// document or memory.
    pub kind: Option<String>,
    /// Scope: T-n, E-n or P-n. Default: the focus.
    pub id: Option<String>,
    pub cursor: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct KbReadArgs {
    /// K-n.
    pub id: String,
    /// Outline number, e.g. 3 or 3.1.
    #[serde(default, deserialize_with = "str_or_num")]
    pub section: Option<String>,
    /// Line range, e.g. 40-80.
    pub lines: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct KbWriteArgs {
    /// Existing entry K-n; omit to create.
    pub id: Option<String>,
    /// Revision you read; required to change a body.
    pub rev: Option<i64>,
    /// replace_body, replace_section or append_section.
    pub op: Option<String>,
    /// New: document or memory.
    pub kind: Option<String>,
    pub title: Option<String>,
    /// One sentence: what it answers, when to read it. Max 120 chars.
    pub summary: Option<String>,
    /// Markdown body or section text. A memory body is max 800 chars.
    #[serde(default)]
    pub text: String,
    /// topic (default), epic or project, relative to the focus.
    pub level: Option<String>,
    /// Owner T-n, E-n or P-n instead of level.
    pub owner: Option<String>,
    #[serde(default, deserialize_with = "str_or_num")]
    pub section: Option<String>,
    /// Heading of an appended or renamed section.
    pub heading: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct HookArgs {
    /// clear, compact or subagent (briefings); prompt, tool, waiting, stop
    /// or end (session presence).
    pub event: String,
    pub session: Option<String>,
    pub agent: Option<String>,
    /// tool: the tool name; end: the reason.
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_accepts_numbers_and_strings() {
        let a: KbReadArgs = serde_json::from_str(r#"{"id":"K-2","section":3}"#).unwrap();
        assert_eq!(a.section.as_deref(), Some("3"));
        let a: KbReadArgs = serde_json::from_str(r#"{"id":"K-2","section":"3.1"}"#).unwrap();
        assert_eq!(a.section.as_deref(), Some("3.1"));
        let a: KbReadArgs = serde_json::from_str(r#"{"id":"K-2"}"#).unwrap();
        assert_eq!(a.section, None);
    }
}
