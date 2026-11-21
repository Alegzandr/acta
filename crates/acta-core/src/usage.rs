//! Claude plan usage (ADR 0005 amendment, 2026-11-21): the two rate-limit
//! windows Claude Code hands its status line command on stdin
//! (code.claude.com/docs/en/statusline, "Rate limit usage"), kept in a tiny
//! file the app reads for its status bar.
//!
//! `rate_limits.five_hour` and `rate_limits.seven_day` each carry
//! `used_percentage` (0 to 100) and `resets_at` (Unix seconds). They exist
//! only for Pro, Max and Ultra plans, only after a session's first answer,
//! and each may be absent; Claude Code drops a window once it reset. This is
//! the only documented source: nothing here calls an endpoint, reads Claude
//! Code's files or touches a token.
//!
//! The file `usage.json` in the data directory holds the numbers and when
//! they were seen, nothing else (no session, path or text). It is written
//! atomically by `acta statusline`, which must stay fast: no database, no
//! keychain.

use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const USAGE_FILE: &str = "usage.json";

/// One rate-limit window.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// 0 to 100 (clamped).
    pub used: f64,
    /// Unix seconds.
    pub resets_at: i64,
}

impl Window {
    /// The window reset after the data was seen: the number no longer
    /// describes the current window.
    pub fn is_stale(&self, now_ms: i64) -> bool {
        now_ms >= self.resets_at.saturating_mul(1000)
    }

    /// Whole percent, rounded.
    pub fn percent(&self) -> u8 {
        self.used.round().clamp(0.0, 100.0) as u8
    }
}

/// What the app shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// When `acta statusline` last saw rate limits (Unix ms).
    pub observed_at: i64,
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
}

/// The rate limits in a status line payload; `None` when it has none.
pub fn parse_status_input(json: &str) -> Option<(Option<Window>, Option<Window>)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let limits = v.get("rate_limits")?;
    let window = |name: &str| {
        let w = limits.get(name)?;
        let used = w.get("used_percentage")?.as_f64()?;
        let resets_at = w.get("resets_at")?.as_f64()? as i64;
        used.is_finite().then_some(Window {
            used: used.clamp(0.0, 100.0),
            resets_at,
        })
    };
    let five = window("five_hour");
    let seven = window("seven_day");
    (five.is_some() || seven.is_some()).then_some((five, seven))
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(USAGE_FILE)
}

/// The last usage seen; `None` before any.
pub fn read(data_dir: &Path) -> Option<Usage> {
    let text = std::fs::read_to_string(path(data_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Merges the windows seen now into the file: a window absent from this
/// payload keeps its last value (the app marks it stale after its reset).
/// Written only when something changed, atomically.
pub fn record(
    data_dir: &Path,
    seen: (Option<Window>, Option<Window>),
    now_ms: i64,
) -> io::Result<Usage> {
    let old = read(data_dir).unwrap_or_default();
    let new = Usage {
        observed_at: now_ms,
        five_hour: seen.0.or(old.five_hour),
        seven_day: seen.1.or(old.seven_day),
    };
    if new.five_hour == old.five_hour
        && new.seven_day == old.seven_day
        && now_ms - old.observed_at < 60_000
    {
        return Ok(old);
    }
    crate::fsutil::create_private_dir(data_dir)?;
    let target = path(data_dir);
    let tmp = data_dir.join(format!("{USAGE_FILE}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    {
        let mut f = crate::fsutil::create_new_private_file(&tmp)?;
        f.write_all(
            serde_json::to_string(&new)
                .map_err(io::Error::other)?
                .as_bytes(),
        )?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, &target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(new)
}

/// Claude Code's own line: "5h 42% · 7d 18%", windows that reset left out.
pub fn status_segment(u: &Usage, now_ms: i64) -> Option<String> {
    let parts: Vec<String> = [("5h", u.five_hour), ("7d", u.seven_day)]
        .into_iter()
        .filter_map(|(label, w)| {
            let w = w?;
            (!w.is_stale(now_ms)).then(|| format!("{label} {}%", w.percent()))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" \u{b7} "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"{"session_id":"x","model":{"display_name":"Opus"},
        "rate_limits":{"five_hour":{"used_percentage":42.4,"resets_at":1738425600},
        "seven_day":{"used_percentage":18,"resets_at":1738857600}}}"#;

    #[test]
    fn windows_parse_and_either_may_be_missing() {
        let (five, seven) = parse_status_input(FULL).unwrap();
        assert_eq!(five.unwrap().percent(), 42);
        assert_eq!(seven.unwrap().resets_at, 1738857600);
        let only = r#"{"rate_limits":{"seven_day":{"used_percentage":101,"resets_at":5}}}"#;
        let (five, seven) = parse_status_input(only).unwrap();
        assert!(five.is_none());
        assert_eq!(seven.unwrap().used, 100.0);
        assert_eq!(parse_status_input(r#"{"model":{}}"#), None);
        assert_eq!(parse_status_input(r#"{"rate_limits":{}}"#), None);
        assert_eq!(
            parse_status_input(r#"{"rate_limits":{"five_hour":{"used_percentage":"x"}}}"#),
            None
        );
        assert_eq!(parse_status_input("not json"), None);
    }

    #[test]
    fn the_file_keeps_absent_windows_and_marks_reset_ones_stale() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path()).is_none());
        let seen = parse_status_input(FULL).unwrap();
        let now = 1_738_420_000_000;
        let u = record(dir.path(), seen, now).unwrap();
        assert_eq!(read(dir.path()), Some(u));
        assert_eq!(
            status_segment(&u, now).as_deref(),
            Some("5h 42% \u{b7} 7d 18%")
        );
        // A later payload without the 5-hour window keeps the old one.
        let later = (
            None,
            Some(Window {
                used: 20.0,
                resets_at: 1738857600,
            }),
        );
        let u = record(dir.path(), later, now + 120_000).unwrap();
        assert_eq!(u.five_hour.unwrap().percent(), 42);
        assert_eq!(u.seven_day.unwrap().percent(), 20);
        // After the 5-hour reset its number is stale and left out.
        let after = 1_738_425_600_000;
        assert!(u.five_hour.unwrap().is_stale(after));
        assert!(!u.seven_day.unwrap().is_stale(after));
        assert_eq!(status_segment(&u, after).as_deref(), Some("7d 20%"));
        // The file holds the numbers and a time, nothing else.
        let text = std::fs::read_to_string(path(dir.path())).unwrap();
        assert!(
            !text.contains("session") && !text.contains("Opus"),
            "{text}"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
