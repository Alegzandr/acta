//! Acta's embedded terminal (ADR 0002): a child process in a PTY, emulated by
//! `alacritty_terminal`, drawn by a GPUI view of our own.
//!
//! - [`session`], [`keys`], [`mouse`], [`launch`], [`theme`], [`url`] and
//!   [`boxes`] are GPUI-free and unit-tested.
//! - [`view`] is the GPUI side: [`TerminalView`] and its element.
//!
//! Keyboard routing (docs/design/interaction.md 6.1): while the terminal has
//! focus, every key goes to the child except the ones [`keys::kept_by_app`]
//! names, and the terminal's own copy, paste and text size keys. GPUI matches
//! key bindings before the focused view sees a key, so the app must scope its
//! other bindings out of the [`view::KEY_CONTEXT`] context (`!Terminal`).

pub mod boxes;
pub mod keys;
pub mod launch;
pub mod mouse;
pub mod session;
pub mod theme;
pub mod url;
pub mod view;

pub use launch::{
    ClaudeLaunch, ClaudeOptions, Conversation, Focus, LaunchError, LaunchSpec, claude_config_dir,
    launch_spec_for_claude, transcript_exists,
};
pub use theme::TerminalTheme;
pub use view::{TerminalEvent, TerminalView};
