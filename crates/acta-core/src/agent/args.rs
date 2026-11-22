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
    /// Default: the focus.
    pub id: Option<String>,
    #[serde(default)]
    pub focus: bool,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct EntriesArgs {
    /// Default: the focus.
    pub topic: Option<String>,
    /// progress, decision or discussion.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Older than this entry number.
    pub before: Option<i64>,
    /// Default 5, max 20.
    pub limit: Option<i64>,
    /// T-42#14.
    pub id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct LogArgs {
    /// progress, decision or discussion.
    #[serde(rename = "type")]
    pub kind: String,
    /// Max 1500 chars.
    pub text: String,
    /// True now, and next; max 600 chars.
    pub state: Option<String>,
    /// draft, planned, active, blocked, done or dropped.
    pub status: Option<String>,
    /// Default: the focus.
    pub topic: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct UpdateArgs {
    /// T, E, N, Q or C-n; P-n for due only.
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<String>,
    /// As log; N: open, dismissed; Q: open, answered, dropped.
    pub status: Option<String>,
    /// E-n or none.
    pub epic: Option<String>,
    /// YYYY-MM-DD, or "" to clear.
    pub due: Option<String>,
    /// Q: C-n or name.
    pub asked_to: Option<String>,
    pub answer: Option<String>,
    /// Q: also log as a decision.
    pub log_decision: Option<bool>,
    /// C-n or name.
    pub report_to: Option<String>,
    /// The user said so.
    pub reported: Option<bool>,
    /// C: all, P-n or E-n.
    pub level: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct FindArgs {
    pub query: Option<String>,
    /// topic, epic, note, entry, question or contact.
    pub kind: Option<String>,
    /// A status, or open (default), closed, all.
    pub status: Option<String>,
    pub epic: Option<String>,
    /// C-n or name.
    pub person: Option<String>,
    /// overdue or week.
    pub due: Option<String>,
    /// Default 8, max 20.
    pub limit: Option<i64>,
    pub cursor: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct CreateItem {
    /// topic, epic, note, entry, question or contact.
    pub kind: String,
    /// Or note text, question, name.
    #[serde(default)]
    pub title: String,
    /// Default with from_notes: their text.
    pub description: Option<String>,
    /// E-n, or @k: item k of this call.
    pub epic: Option<String>,
    /// T-n, for entry.
    pub topic: Option<String>,
    /// N-ids filed into this item.
    pub from_notes: Option<Vec<String>>,
    /// YYYY-MM-DD.
    pub due: Option<String>,
    /// C-n, name or @k.
    pub asked_to: Option<String>,
    /// T-n, E-n or P-n.
    pub owner: Option<String>,
    /// C-n, name or @k.
    pub report_to: Option<String>,
    /// all, P-n or E-n.
    pub level: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct CreateArgs {
    #[serde(default)]
    pub items: Vec<CreateItem>,
    /// N-ids to close unfiled.
    pub dismiss: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct KbArgs {
    pub query: Option<String>,
    pub kind: Option<String>,
    /// Default: the focus.
    pub id: Option<String>,
    pub cursor: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct KbReadArgs {
    /// K-n.
    pub id: String,
    /// Outline number: 3, 3.1.
    #[serde(default, deserialize_with = "str_or_num")]
    pub section: Option<String>,
    /// e.g. 40-80.
    pub lines: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct KbWriteArgs {
    /// K-n to edit; omit to create.
    pub id: Option<String>,
    /// The revision read.
    pub rev: Option<i64>,
    /// replace_body, replace_section, append_section, verify or retire
    /// (text: why).
    pub op: Option<String>,
    /// document or memory.
    pub kind: Option<String>,
    pub title: Option<String>,
    /// What it answers; max 120 chars.
    pub summary: Option<String>,
    /// Markdown; memory max 800.
    #[serde(default)]
    pub text: String,
    /// topic (default), epic, project or branch.
    pub level: Option<String>,
    /// Instead of level.
    pub owner: Option<String>,
    #[serde(default, deserialize_with = "str_or_num")]
    pub section: Option<String>,
    pub heading: Option<String>,
    /// K ids it replaces.
    pub supersedes: Option<String>,
    /// Max 3: T-12#14, Q-3, commit:sha, a path.
    pub refs: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct AttachmentArgs {
    pub id: String,
    /// Text: 40-80.
    pub lines: Option<String>,
    /// PDF: 3-5.
    pub pages: Option<String>,
    /// Image: full.
    pub size: Option<String>,
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
