//! Time as stored: integer Unix milliseconds, UTC. No date library: the only
//! calendar arithmetic needed is a UTC date for snapshot names.

use std::time::{SystemTime, UNIX_EPOCH};

pub const MS_PER_DAY: i64 = 86_400_000;

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Days since 1970-01-01 (UTC) of a timestamp, rounding towards the past.
pub fn day_number(ms: i64) -> i64 {
    ms.div_euclid(MS_PER_DAY)
}

/// Index of the Monday-to-Sunday week containing a timestamp. 1970-01-01 was
/// a Thursday, hence the offset of 3 days.
pub fn week_number(ms: i64) -> i64 {
    (day_number(ms) + 3).div_euclid(7)
}

/// `(year, month, day)` of a day number (Howard Hinnant's `civil_from_days`).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Inverse of [`civil_from_days`].
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Compact UTC stamp used in file names: `20261121T101502123Z`.
pub fn file_stamp(ms: i64) -> String {
    let (y, mo, d) = civil_from_days(day_number(ms));
    let in_day = ms.rem_euclid(MS_PER_DAY);
    let (h, mi, s, milli) = (
        in_day / 3_600_000,
        in_day / 60_000 % 60,
        in_day / 1000 % 60,
        in_day % 1000,
    );
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}{milli:03}Z")
}

/// Parses what [`file_stamp`] writes.
pub fn parse_file_stamp(stamp: &str) -> Option<i64> {
    let b = stamp.as_bytes();
    if b.len() != 19 || b[8] != b'T' || b[18] != b'Z' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = stamp.get(r)?;
        if !s.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    };
    let (y, mo, d) = (num(0..4)?, num(4..6)?, num(6..8)?);
    let (h, mi, s, milli) = (num(9..11)?, num(11..13)?, num(13..15)?, num(15..18)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let days = days_from_civil(y, mo as u32, d as u32);
    Some(days * MS_PER_DAY + h * 3_600_000 + mi * 60_000 + s * 1000 + milli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2026, 11, 21), 20_778);
        for days in [-800_000, -1, 0, 59, 11_016, 20_778, 2_000_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn file_stamps_round_trip() {
        let ms = 20_778 * MS_PER_DAY + 10 * 3_600_000 + 15 * 60_000 + 2_123;
        assert_eq!(file_stamp(ms), "20261121T101502123Z");
        assert_eq!(parse_file_stamp(&file_stamp(ms)), Some(ms));
        assert_eq!(parse_file_stamp("20261121T101502123"), None);
        assert_eq!(parse_file_stamp("2026112xT101502123Z"), None);
    }

    #[test]
    fn weeks_start_on_monday() {
        // 2026-11-16 is a Monday, 2026-11-22 a Sunday.
        let monday = days_from_civil(2026, 11, 16) * MS_PER_DAY;
        let sunday = days_from_civil(2026, 11, 22) * MS_PER_DAY;
        assert_eq!(week_number(monday), week_number(sunday));
        assert_eq!(week_number(monday - 1), week_number(monday) - 1);
    }
}
