//! The agent-facing layer (ADR 0005): session binding, the nine tools and
//! the hook briefings, rendered as compact text under hard budgets. The MCP
//! server and the CLI are thin transports over this module.

pub mod args;
pub mod attach;
pub mod due;
pub mod folders;
pub mod git;
pub mod presence;
pub mod questions;
pub mod render;
pub mod session;
pub mod tools;

pub use attach::{Answer, AttachReply, Step, set_pdf_worker};
pub use session::{Launch, Session, parse_owner};
pub use tools::{Agent, INSTRUCTIONS, NOT_LINKED, PRE_COMPACT, Reply, hook_json};

#[cfg(test)]
mod tests;
