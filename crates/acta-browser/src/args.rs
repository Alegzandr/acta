//! The nine tools' parameters (ADR 0010, Tools). The same types describe
//! the MCP schemas (`acta mcp browser`) and travel over the pipe, so the
//! app validates exactly what the agent was offered.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Go {
    Back,
    Forward,
    Reload,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenArgs {
    /// http(s) URL to load. Omit with go.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// back, forward or reload instead of a URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub go: Option<Go>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotFilter {
    #[default]
    All,
    Interactive,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotArgs {
    /// interactive: only controls, links and headings, flat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<SnapshotFilter>,
    /// Only this element's subtree (a ref like e19).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    /// Wait up to 10 s for this text to appear first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_for: Option<String>,
    /// Character budget, default 6000, at most 8000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadArgs {
    /// Only this element's text (a ref).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    /// From the previous reply's "more: cursor=N".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClickArgs {
    /// Element ref from snapshot, like e19.
    #[serde(rename = "ref")]
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Key {
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TypeArgs {
    /// Field ref from snapshot.
    #[serde(rename = "ref")]
    pub target: String,
    /// Text to insert, at most 2000 chars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Empty the field first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear: Option<bool>,
    /// A key to press after the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Key>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScreenshotArgs {
    /// Only this element, scrolled into view.
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Image width, 320 to 1568, default 1024.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warning,
    #[default]
    All,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConsoleArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<Level>,
    /// Keep messages containing this text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Default 20, at most 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NetworkArgs {
    /// Keep requests whose URL contains this text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Only failed requests and HTTP errors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<bool>,
    /// Default 20, at most 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HandoffArgs {
    /// What the user should do, at most 200 chars.
    pub message: String,
}

/// One request over the pipe: an operation, never CDP (ADR 0010, decision 4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "args", rename_all = "lowercase")]
pub enum Request {
    Open(OpenArgs),
    Snapshot(SnapshotArgs),
    Read(ReadArgs),
    Click(ClickArgs),
    Type(TypeArgs),
    Screenshot(ScreenshotArgs),
    Console(ConsoleArgs),
    Network(NetworkArgs),
    Handoff(HandoffArgs),
}

impl Request {
    pub fn name(&self) -> &'static str {
        match self {
            Request::Open(_) => "open",
            Request::Snapshot(_) => "snapshot",
            Request::Read(_) => "read",
            Request::Click(_) => "click",
            Request::Type(_) => "type",
            Request::Screenshot(_) => "screenshot",
            Request::Console(_) => "console",
            Request::Network(_) => "network",
            Request::Handoff(_) => "handoff",
        }
    }
}

/// One reply: plain text, and a base64 JPEG for `screenshot`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

impl Reply {
    pub fn ok(text: impl Into<String>) -> Reply {
        Reply {
            ok: true,
            text: text.into(),
            image: None,
        }
    }

    pub fn err(text: impl Into<String>) -> Reply {
        Reply {
            ok: false,
            text: text.into(),
            image: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_travel_as_op_and_args() {
        let r = Request::Click(ClickArgs {
            target: "e19".into(),
            double: None,
        });
        let line = serde_json::to_string(&r).unwrap();
        assert_eq!(line, r#"{"op":"click","args":{"ref":"e19"}}"#);
        assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), r);
        let t: Request = serde_json::from_str(
            r#"{"op":"type","args":{"ref":"e2","text":"Alexandre","key":"Enter"}}"#,
        )
        .unwrap();
        assert!(matches!(
            t,
            Request::Type(TypeArgs {
                key: Some(Key::Enter),
                ..
            })
        ));
        assert!(serde_json::from_str::<Request>(r#"{"op":"eval","args":{}}"#).is_err());
    }
}
