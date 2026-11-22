//! Due dates and the Overview in agent output (data.md requests 16 and 17,
//! ADR 0005 changelog). Pure functions of a date and "today": the text the
//! agent reads is computed by [`due_state`], the function the UI uses, so
//! both always agree.

use super::render;
use crate::date::{Date, DueState, due_state};

/// The refusal for anything that is not an exact date: the agent's writes
/// are exact by design, so natural language is never guessed.
pub fn refusal(given: &str) -> String {
    format!(
        "due must be YYYY-MM-DD (2000-2099) or \"\" to clear, got \"{}\"",
        render::clip(&render::one_line(given), 30)
    )
}

/// `"YYYY-MM-DD"` sets, `""` clears (`None`); anything else is refused.
pub fn parse_arg(given: &str) -> Result<Option<Date>, String> {
    let t = given.trim();
    if t.is_empty() {
        return Ok(None);
    }
    Date::parse_iso(t).map(Some).ok_or_else(|| refusal(given))
}

/// `11-27` in the reader's year, `2027-02-03` in another.
pub fn day(due: Date, today: Date) -> String {
    if due.ymd().0 == today.ymd().0 {
        let (_, m, d) = due.ymd();
        format!("{m:02}-{d:02}")
    } else {
        due.to_string()
    }
}

/// The header segment: `· due 11-27 (6d)`, `· due today`, `· due tomorrow`,
/// `· overdue 3d (due 11-18)`; a closed record shows `· due 11-18` and is
/// never called overdue. Empty without a date.
pub fn segment(due: Option<Date>, today: Date, open: bool) -> String {
    let Some(d) = due else {
        return String::new();
    };
    match due_state(due, today, open) {
        DueState::None => String::new(),
        DueState::Overdue(n) => format!(" · overdue {n}d (due {})", day(d, today)),
        DueState::Today => " · due today".into(),
        DueState::Soon(1) => " · due tomorrow".into(),
        DueState::Soon(n) | DueState::Later(n) => format!(" · due {} ({n}d)", day(d, today)),
        DueState::Closed => format!(" · due {}", day(d, today)),
    }
}

/// The shortened segment on a list line: `· due 11-27`, `· overdue 3d`,
/// `· due today`, `· due tomorrow`; empty for a closed record.
pub fn short_segment(due: Option<Date>, today: Date, open: bool) -> String {
    let Some(d) = due else {
        return String::new();
    };
    match due_state(due, today, open) {
        DueState::None | DueState::Closed => String::new(),
        DueState::Overdue(n) => format!(" · overdue {n}d"),
        DueState::Today => " · due today".into(),
        DueState::Soon(1) => " · due tomorrow".into(),
        DueState::Soon(_) | DueState::Later(_) => format!(" · due {}", day(d, today)),
    }
}

/// What an `update` ack says, so the agent sees the date it set:
/// `due 2026-12-15 (in 24d)`, `due cleared`.
pub fn ack(due: Option<Date>, today: Date) -> String {
    match due {
        None => "due cleared".into(),
        Some(d) => {
            let rel = match today.days_until(d) {
                0 => "today".to_owned(),
                1 => "tomorrow".to_owned(),
                n if n > 0 => format!("in {n}d"),
                n => format!("{}d ago", -n),
            };
            format!("due {d} ({rel})")
        }
    }
}

/// Whether a record is overdue for `--json` (`"overdue": true|false`).
pub fn is_overdue(due: Option<Date>, today: Date, open: bool) -> bool {
    matches!(due_state(due, today, open), DueState::Overdue(_))
}

/// `find(due=...)`: the first day a record may NOT be due on to match.
pub fn find_cutoff(filter: &str, today: Date) -> Result<Date, String> {
    match filter.trim().to_ascii_lowercase().as_str() {
        "overdue" => Ok(today),
        "week" => Ok(today.add_days(7)),
        _ => Err("due must be overdue or week".into()),
    }
}

/// Characters of the Attention line of a project (request 17).
pub const ATTENTION_PROJECT: usize = 160;
/// Characters of the Attention line of an epic.
pub const ATTENTION_EPIC: usize = 120;

/// The `Attention:` line (request 17): one segment per non-empty list, in
/// the order given (blocked, overdue, idle), then `fixed` (the inbox). Lists
/// are cut at item boundaries with `+N`, fairly: every segment keeps its
/// first item, then items are added round-robin while the line fits `max`.
/// `None` when nothing needs attention.
pub fn attention_line(
    lists: &[(&str, Vec<String>)],
    fixed: Option<String>,
    max: usize,
) -> Option<String> {
    let lists: Vec<&(&str, Vec<String>)> = lists.iter().filter(|(_, l)| !l.is_empty()).collect();
    if lists.is_empty() && fixed.is_none() {
        return None;
    }
    let render = |shown: &[usize]| {
        let mut parts: Vec<String> = lists
            .iter()
            .zip(shown)
            .map(|((label, items), k)| {
                let more = if *k < items.len() {
                    format!(" +{}", items.len() - k)
                } else {
                    String::new()
                };
                format!("{label} {}{more}", items[..*k].join(", "))
            })
            .collect();
        parts.extend(fixed.clone());
        format!("Attention: {}", parts.join(" · "))
    };
    let mut shown = vec![1; lists.len()];
    loop {
        let mut grew = false;
        for i in 0..lists.len() {
            if shown[i] < lists[i].1.len() {
                shown[i] += 1;
                if render::chars(&render(&shown)) <= max {
                    grew = true;
                } else {
                    shown[i] -= 1;
                }
            }
        }
        if !grew {
            break;
        }
    }
    Some(render::clip(&render(&shown), max))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        Date::parse_iso(s).unwrap()
    }

    #[test]
    fn segments_in_every_state() {
        let today = d("2026-11-21");
        let seg = |due: &str, open| segment(Some(d(due)), today, open);
        assert_eq!(seg("2026-11-27", true), " · due 11-27 (6d)");
        assert_eq!(seg("2026-11-21", true), " · due today");
        assert_eq!(seg("2026-11-22", true), " · due tomorrow");
        assert_eq!(seg("2026-11-23", true), " · due 11-23 (2d)");
        assert_eq!(seg("2026-11-18", true), " · overdue 3d (due 11-18)");
        assert_eq!(seg("2026-11-18", false), " · due 11-18");
        assert_eq!(seg("2027-02-03", true), " · due 2027-02-03 (74d)");
        assert_eq!(seg("2025-12-31", true), " · overdue 325d (due 2025-12-31)");
        assert_eq!(segment(None, today, true), "");
        let short = |due: &str, open| short_segment(Some(d(due)), today, open);
        assert_eq!(short("2026-11-27", true), " · due 11-27");
        assert_eq!(short("2026-11-18", true), " · overdue 3d");
        assert_eq!(short("2026-11-21", true), " · due today");
        assert_eq!(short("2026-11-22", true), " · due tomorrow");
        assert_eq!(short("2026-11-18", false), "");
        // At most 30 chars per line (request 16).
        assert!(render::chars(&seg("2099-12-31", true)) <= 30);
        assert!(render::chars(&segment(Some(d("2000-01-01")), d("2099-12-31"), true)) <= 40);
    }

    #[test]
    fn arguments_are_exact() {
        assert_eq!(parse_arg("2026-12-15"), Ok(Some(d("2026-12-15"))));
        assert_eq!(parse_arg(" "), Ok(None));
        assert_eq!(
            parse_arg("friday").unwrap_err(),
            "due must be YYYY-MM-DD (2000-2099) or \"\" to clear, got \"friday\""
        );
        for bad in [
            "2026-02-30",
            "1999-12-31",
            "2100-01-01",
            "15/12/2026",
            "2026-1-5",
        ] {
            assert!(parse_arg(bad).is_err(), "{bad}");
        }
        let today = d("2026-11-21");
        assert_eq!(ack(Some(d("2026-12-15")), today), "due 2026-12-15 (in 24d)");
        assert_eq!(ack(Some(today), today), "due 2026-11-21 (today)");
        assert_eq!(ack(Some(d("2026-11-18")), today), "due 2026-11-18 (3d ago)");
        assert_eq!(ack(None, today), "due cleared");
        assert_eq!(find_cutoff("week", today), Ok(d("2026-11-28")));
        assert!(find_cutoff("soon", today).is_err());
    }

    #[test]
    fn attention_is_cut_fairly_at_item_boundaries() {
        assert_eq!(attention_line(&[("blocked", vec![])], None, 160), None);
        let ids = |p: &str, n: usize, tail: &str| -> Vec<String> {
            (1..=n).map(|i| format!("{p}-{i}{tail}")).collect()
        };
        let line = attention_line(
            &[
                ("blocked", ids("T", 2, "")),
                ("overdue", ids("T", 2, " 3d")),
                ("idle", vec![]),
            ],
            Some("inbox 6, oldest 12d".into()),
            160,
        )
        .unwrap();
        assert_eq!(
            line,
            "Attention: blocked T-1, T-2 · overdue T-1 3d, T-2 3d · inbox 6, oldest 12d"
        );
        let line = attention_line(
            &[
                ("blocked", ids("T", 500, "")),
                ("overdue", ids("T", 500, " 3d")),
                ("idle", ids("T", 500, " 12d")),
            ],
            Some("inbox 600, oldest 120d".into()),
            160,
        )
        .unwrap();
        assert!(render::chars(&line) <= 160, "{line}");
        assert!(line.contains("blocked T-1"), "{line}");
        assert!(line.contains("overdue T-1 3d"), "{line}");
        assert!(line.contains("idle T-1 12d"), "{line}");
        assert!(line.ends_with("inbox 600, oldest 120d"), "{line}");
        assert!(line.contains(" +4"), "{line}");
        let epic = attention_line(&[("blocked", ids("T", 500, ""))], None, 120).unwrap();
        assert!(render::chars(&epic) <= 120, "{epic}");
    }
}
