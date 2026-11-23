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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
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
    /// Browser number on this page (default: yours).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadArgs {
    /// Only this element's text (a ref).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
    /// From the previous reply's "more: cursor=N".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClickArgs {
    /// Element ref from snapshot, like e19.
    #[serde(rename = "ref")]
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScreenshotArgs {
    /// Only this element, scrolled into view.
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Image width, 320 to 1568, default 1024.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HandoffArgs {
    /// What the user should do, at most 200 chars.
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<i64>")]
    pub browser: Option<u8>,
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

/// Field limits, in characters. `acta mcp browser` checks them before
/// sending and the app again on receipt.
pub mod limits {
    pub const URL: usize = 2_048;
    pub const TEXT: usize = 2_000;
    pub const MESSAGE: usize = 200;
    pub const REF: usize = 24;
    pub const QUERY: usize = 200;
}

fn within(field: &str, value: Option<&str>, max: usize) -> Result<(), String> {
    match value.map(|v| v.chars().count()) {
        Some(n) if n > max => Err(format!("refused: {field} is {n} chars, at most {max}")),
        _ => Ok(()),
    }
}

impl Request {
    /// Every field within its limit, or the one-line refusal.
    pub fn check(&self) -> Result<(), String> {
        use limits::*;
        match self {
            Request::Open(a) => within("url", a.url.as_deref(), URL),
            Request::Snapshot(a) => {
                within("within", a.within.as_deref(), REF)?;
                within("wait_for", a.wait_for.as_deref(), QUERY)
            }
            Request::Read(a) => within("within", a.within.as_deref(), REF),
            Request::Click(a) => within("ref", Some(&a.target), REF),
            Request::Type(a) => {
                within("ref", Some(&a.target), REF)?;
                within("text", a.text.as_deref(), TEXT)
            }
            Request::Screenshot(a) => within("ref", a.target.as_deref(), REF),
            Request::Console(a) => within("pattern", a.pattern.as_deref(), QUERY),
            Request::Network(a) => within("pattern", a.pattern.as_deref(), QUERY),
            Request::Handoff(a) => within("message", Some(&a.message), MESSAGE),
        }
    }

    /// The browser the call names, else the session's own.
    pub fn browser(&self) -> Option<u8> {
        match self {
            Request::Open(a) => a.browser,
            Request::Snapshot(a) => a.browser,
            Request::Read(a) => a.browser,
            Request::Click(a) => a.browser,
            Request::Type(a) => a.browser,
            Request::Screenshot(a) => a.browser,
            Request::Console(a) => a.browser,
            Request::Network(a) => a.browser,
            Request::Handoff(a) => a.browser,
        }
    }

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
            browser: None,
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
        let long = Request::Type(TypeArgs {
            target: "e2".into(),
            text: Some("a".repeat(limits::TEXT + 1)),
            ..Default::default()
        });
        assert!(long.check().unwrap_err().contains("at most 2000"));
        assert!(t.check().is_ok());
        let url = Request::Open(OpenArgs {
            url: Some(format!("https://example.com/{}", "a".repeat(limits::URL))),
            go: None,
            browser: None,
        });
        let named: Request =
            serde_json::from_str(r#"{"op":"console","args":{"browser":2}}"#).unwrap();
        assert_eq!(named.browser(), Some(2));
        assert!(url.check().is_err());
    }
}
