//! Acta's agent browser (ADR 0010): the operations Claude Code calls through
//! `acta mcp browser`, the navigation and input policy, the budgets and the
//! web-page fence, and the authenticated pipe between the CLI and the app.
//! No GUI: the app implements [`ops::Host`] over its WebView2; the CLI maps
//! MCP calls to [`args::Request`]s.

pub mod args;
pub mod auth;
pub mod events;
pub mod ops;
pub mod policy;
pub mod render;
pub mod server;
pub mod wire;

#[cfg(windows)]
pub mod client;
#[cfg(windows)]
pub mod pipe;

/// What every tool answers where the agent browser does not exist yet.
pub const UNSUPPORTED: &str =
    "Acta's agent browser is not available on this OS yet; use Claude in Chrome";
