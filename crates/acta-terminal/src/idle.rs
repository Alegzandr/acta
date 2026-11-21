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

/// The start of the hint Claude Code shows in an empty input box, should
/// it arrive without the dim attribute that normally strips it.
const PLACEHOLDER: &str = "Try \"";

/// `screen` is the visible text, every cell; `plain` the same rows without
/// dimmed or inverse cells (Claude Code's placeholder and its own cursor);
/// `quiet` how long since the last output. Idle means: nothing printed for
/// [`QUIET`], no busy marker on screen, and the last prompt line on screen
/// (a prompt marker after the box border) has nothing typed after it. The
/// terminal's cursor is not looked at: Claude Code hides it and draws its
/// own.
pub fn claude_idle(screen: &str, plain: &[String], quiet: Duration) -> bool {
    if quiet < QUIET {
        return false;
    }
    let lower = screen.to_lowercase();
    if BUSY_MARKERS.iter().any(|m| lower.contains(m)) {
        return false;
    }
    let Some(rest) = plain.iter().rev().find_map(|row| {
        let line = row
            .trim()
            .trim_matches(|c: char| c == '\u{2502}' || c == '|')
            .trim();
        line.strip_prefix(PROMPT_MARKERS).map(str::trim)
    }) else {
        return false;
    };
    rest.is_empty() || rest.starts_with(PLACEHOLDER)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONG: Duration = Duration::from_secs(3);

    fn rows(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| (*l).to_owned()).collect()
    }

    fn input(line: &str) -> Vec<String> {
        rows(&[
            "> Alexandre asked something earlier",
            "Claude answered.",
            "\u{256D}\u{2500}\u{2500}\u{256E}",
            line,
            "\u{2570}\u{2500}\u{2500}\u{256F}",
            "  ? for shortcuts",
        ])
    }

    #[test]
    fn an_empty_prompt_on_a_still_screen_is_idle_whatever_the_cursor() {
        let screen = input("\u{2502} > \u{2502}").join("\n");
        assert!(claude_idle(&screen, &input("\u{2502} >  \u{2502}"), LONG));
        assert!(claude_idle("", &rows(&["\u{276F} "]), LONG));
        // The placeholder, when it is not dimmed (dimmed, it is not in
        // `plain` at all).
        assert!(claude_idle(
            &screen,
            &input("\u{2502} > Try \"fix lint errors\" \u{2502}"),
            LONG
        ));
    }

    #[test]
    fn work_typing_or_recent_output_is_busy() {
        let screen = input("\u{2502} > \u{2502}").join("\n");
        // Working: the status line says so.
        let working = format!("{screen}\n* Thinking... (esc to interrupt)");
        assert!(!claude_idle(&working, &input("\u{2502} > \u{2502}"), LONG));
        // Alexandre has typed something.
        assert!(!claude_idle(
            &screen,
            &input("\u{2502} > fix the tests \u{2502}"),
            LONG
        ));
        // Output a moment ago.
        assert!(!claude_idle(
            &screen,
            &input("\u{2502} > \u{2502}"),
            Duration::from_millis(200)
        ));
        // A permission question: its menu is the last marked line.
        let menu = rows(&["Do you want to proceed?", "\u{276F} 1. Yes", "  2. No"]);
        assert!(!claude_idle(&menu.join("\n"), &menu, LONG));
        // No prompt at all (a shell).
        assert!(!claude_idle("PS C:\\>", &rows(&["PS C:\\>"]), LONG));
        assert!(!claude_idle("", &[], LONG));
    }
}
