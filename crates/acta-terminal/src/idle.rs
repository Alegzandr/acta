//! Whether Claude Code sits idle at its prompt, so a command can be typed
//! into the session (the inbox's "File with Claude Code"). Strict on
//! purpose: a wrong "idle" would type into a session that is mid-task,
//! while a wrong "busy" only costs a new session, so anything unusual reads
//! as busy.

use std::time::Duration;

/// How long the screen must have been still.
pub const QUIET: Duration = Duration::from_millis(1500);

/// What Claude Code shows while it works (its status line).
const BUSY_MARKERS: [&str; 2] = ["esc to interrupt", "esc to cancel"];

/// Prompt markers Claude Code's input box starts with.
const PROMPT_MARKERS: [char; 2] = ['>', '\u{276F}'];

/// `screen` is the visible text, `cursor_row` the line the cursor is on,
/// `quiet` how long since the last output. Idle means: nothing printed for
/// [`QUIET`], no busy marker on screen, and the cursor on an empty prompt
/// (a prompt marker with nothing typed after it, box borders aside).
pub fn claude_idle(screen: &str, cursor_row: &str, quiet: Duration) -> bool {
    if quiet < QUIET {
        return false;
    }
    let lower = screen.to_lowercase();
    if BUSY_MARKERS.iter().any(|m| lower.contains(m)) {
        return false;
    }
    let line = cursor_row
        .trim()
        .trim_matches(|c: char| c == '\u{2502}' || c == '|')
        .trim();
    let Some(rest) = line.strip_prefix(PROMPT_MARKERS) else {
        return false;
    };
    rest.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONG: Duration = Duration::from_secs(3);

    fn screen(status: &str) -> String {
        format!(
            "Welcome back Alexandre\n\u{256D}\u{2500}\u{2500}\u{256E}\n\u{2502} > \u{2502}\n\u{2570}\u{2500}\u{2500}\u{256F}\n{status}\n"
        )
    }

    #[test]
    fn an_empty_prompt_on_a_still_screen_is_idle() {
        assert!(claude_idle(
            &screen("? for shortcuts"),
            "\u{2502} >  \u{2502}",
            LONG
        ));
        assert!(claude_idle(&screen(""), "\u{276F} ", LONG));
    }

    #[test]
    fn work_typing_or_recent_output_is_busy() {
        // Working: the status line says so.
        assert!(!claude_idle(
            &screen("* Thinking... (esc to interrupt)"),
            "\u{2502} > \u{2502}",
            LONG
        ));
        // Alexandre has typed something.
        assert!(!claude_idle(
            &screen(""),
            "\u{2502} > fix the tests \u{2502}",
            LONG
        ));
        // Output a moment ago.
        assert!(!claude_idle(
            &screen(""),
            "\u{2502} > \u{2502}",
            Duration::from_millis(200)
        ));
        // Not at a prompt at all (a shell, a menu, a permission question).
        assert!(!claude_idle("Do you want to proceed?\n", "1. Yes", LONG));
        assert!(!claude_idle("", "", LONG));
    }
}
