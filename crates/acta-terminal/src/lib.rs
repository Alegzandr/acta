//! Acta's embedded terminal (ADR 0002): a child process in a PTY, emulated by
//! `alacritty_terminal`, drawn by a GPUI view of our own.
//!
//! - [`session`], [`keys`], [`mouse`], [`launch`], [`theme`], [`url`] and
//!   [`boxes`] are GPUI-free and unit-tested.

pub mod boxes;
pub mod keys;
pub mod launch;
pub mod mouse;
pub mod session;
pub mod theme;
pub mod url;

pub use launch::{ClaudeLaunch, Focus, LaunchError, LaunchSpec, launch_spec_for_claude};
pub use theme::TerminalTheme;
