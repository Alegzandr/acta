//! Calendar days (due dates, docs/design/data.md request 16; copy.md 10.2.1).
//!
//! A due date is a day, not a moment: no time, no time zone. Everything
//! relative ("today", "overdue", "no activity for 7 days") is computed
//! against the local date of the machine doing the reading, which callers
//! pass in, so every function here is pure and testable. [`Zone`] maps a
//! stored instant (Unix ms) to the local day it falls on, and back.

use crate::time::{MS_PER_DAY, civil_from_days, days_from_civil};
use std::fmt;

/// One calendar day. Ordered; the difference of two days is a whole number
/// of calendar days, never of 24-hour periods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date(i64);

impl Date {
    /// Due dates live in these years (data.md request 16).
    pub const MIN_YEAR: i64 = 2000;
    pub const MAX_YEAR: i64 = 2099;

    /// The day `days` after 1970-01-01.
    pub fn from_days(days: i64) -> Date {
        Date(days)
    }

    /// Days since 1970-01-01.
    pub fn days(self) -> i64 {
        self.0
    }

    /// A day that exists in the calendar, in the years 2000 to 2099.
    pub fn from_ymd(year: i64, month: u32, day: u32) -> Option<Date> {
        if !(Self::MIN_YEAR..=Self::MAX_YEAR).contains(&year)
            || !(1..=12).contains(&month)
            || day < 1
            || day > days_in_month(year, month)
        {
            return None;
        }
        Some(Date(days_from_civil(year, month, day)))
    }

    pub fn ymd(self) -> (i64, u32, u32) {
        civil_from_days(self.0)
    }

    /// Whether the day is in the years 2000 to 2099.
    pub fn in_range(self) -> bool {
        (Self::MIN_YEAR..=Self::MAX_YEAR).contains(&self.ymd().0)
    }

    /// Strict `YYYY-MM-DD`, years 2000 to 2099: the stored, exported and
    /// agent form. Anything else is `None`.
    pub fn parse_iso(text: &str) -> Option<Date> {
        let b = text.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let digits = |r: std::ops::Range<usize>| -> Option<i64> {
            let s = &text[r];
            s.bytes()
                .all(|c| c.is_ascii_digit())
                .then(|| s.parse().ok())
                .flatten()
        };
        let (y, m, d) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
        Date::from_ymd(y, m as u32, d as u32)
    }

    pub fn add_days(self, n: i64) -> Date {
        Date(self.0 + n)
    }

    /// Calendar days from `self` to `other` (negative when `other` is
    /// earlier).
    pub fn days_until(self, other: Date) -> i64 {
        other.0 - self.0
    }

    /// 0 for Monday to 6 for Sunday (weeks start on Monday, 10.2.1).
    pub fn weekday(self) -> i64 {
        // 1970-01-01 was a Thursday.
        (self.0 + 3).rem_euclid(7)
    }

    /// The same day number `months` months later, clamped to the month's
    /// last day.
    pub fn add_months(self, months: i64) -> Date {
        let (y, m, d) = self.ymd();
        let index = y * 12 + i64::from(m) - 1 + months;
        let (y, m) = (index.div_euclid(12), (index.rem_euclid(12) + 1) as u32);
        Date(days_from_civil(y, m, d.min(days_in_month(y, m))))
    }
}

impl fmt::Display for Date {
    /// The ISO form, `2026-12-15`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The local time zone of the reader, as the UTC offset in force at an
/// instant. The app and the CLI use [`Zone::system`]; tests build their own,
/// with a daylight-saving change where they need one.
#[derive(Clone, Copy)]
pub struct Zone {
    /// Seconds east of UTC in force at the instant (Unix ms).
    offset_at: fn(i64) -> i64,
}

impl fmt::Debug for Zone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Zone")
    }
}

impl Zone {
    pub const UTC: Zone = Zone { offset_at: |_| 0 };

    /// A zone from its offset function (seconds east of UTC at an instant).
    pub const fn from_fn(offset_at: fn(i64) -> i64) -> Zone {
        Zone { offset_at }
    }

    /// The machine's time zone, as the OS reports it at each instant.
    pub fn system() -> Zone {
        fn offset(ms: i64) -> i64 {
            use chrono::{Local, Offset, TimeZone};
            Local
                .timestamp_millis_opt(ms)
                .single()
                .map_or(0, |at| i64::from(at.offset().fix().local_minus_utc()))
        }
        Zone { offset_at: offset }
    }

    /// The local day an instant falls on.
    pub fn date_of(self, ms: i64) -> Date {
        let local = ms + (self.offset_at)(ms) * 1000;
        Date(local.div_euclid(MS_PER_DAY))
    }

    /// Today, read from the system clock.
    pub fn today(self) -> Date {
        self.date_of(crate::time::now_ms())
    }

    /// The first instant of a local day: the earliest instant whose local
    /// time is 00:00 that day (a clock that falls back across midnight
    /// shows it twice), or, where a clock change skips local midnight, the
    /// first instant that falls on that day.
    pub fn midnight(self, date: Date) -> i64 {
        let local = date.0 * MS_PER_DAY;
        // Every offset in force within a day and a half of the day's
        // start; each gives one candidate instant for local 00:00.
        let mut offsets: Vec<i64> = (-36..=36)
            .map(|k| (self.offset_at)(local + k * 30 * 60_000))
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        let exact = offsets
            .iter()
            .map(|o| local - o * 1000)
            .filter(|at| (self.offset_at)(*at) * 1000 == local - at)
            .min();
        exact.unwrap_or_else(|| self.first_instant_on(date))
    }

    /// The first instant whose local day is `date` or later, by binary
    /// search between the widest offsets in use, UTC-12 and UTC+14.
    fn first_instant_on(self, date: Date) -> i64 {
        let (mut lo, mut hi) = (
            date.0 * MS_PER_DAY - 14 * 3_600_000 - 1,
            date.0 * MS_PER_DAY + 12 * 3_600_000,
        );
        // Invariant: `lo` is before the day, `hi` on it or after.
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.date_of(mid) >= date {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }
}

/// Where a due date stands for a reader on `today` (components-records.md 5).
/// Thresholds in whole local days: overdue below 0, today 0, due soon 1 to
/// 3, later 4 and more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DueState {
    /// No date.
    None,
    /// Before today, by this many days (1 or more).
    Overdue(i64),
    Today,
    /// 1 to 3 days ahead.
    Soon(i64),
    /// 4 days ahead or more.
    Later(i64),
    /// The record is closed: "Done" answers the date, which is never
    /// called overdue, whatever it is.
    Closed,
}

/// The one function the UI and the agent share, so both always agree.
/// `open`: a topic Draft, Planned, Active or Blocked; an epic or a project
/// while its progress is below 100% or it has no topic
/// ([`crate::model::Progress::is_open`]).
pub fn due_state(due_on: Option<Date>, today: Date, open: bool) -> DueState {
    let Some(due) = due_on else {
        return DueState::None;
    };
    if !open {
        return DueState::Closed;
    }
    match today.days_until(due) {
        n if n < 0 => DueState::Overdue(-n),
        0 => DueState::Today,
        n @ 1..=3 => DueState::Soon(n),
        n => DueState::Later(n),
    }
}

// ---- typed dates (copy.md 10.2.1) ----

/// Parses what a person types into the date picker, in English or French
/// whatever the interface language, relative to `today`. `None` when it is
/// not a date: nothing is guessed. The result is always in 2000 to 2099.
pub fn parse_typed(input: &str, today: Date) -> Option<Date> {
    let text = normalize(input);
    let tokens = tokenize(&text);
    let words: Vec<&str> = tokens.iter().map(String::as_str).collect();
    parse_words(&words, today).filter(|d| d.in_range())
}

/// Lower case, accents off, curly apostrophes straightened.
fn normalize(input: &str) -> String {
    input
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'û' | 'ù' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            '\u{2019}' | '\u{2018}' => '\'',
            c => c,
        })
        .collect()
}

/// Whitespace-separated words, each split where digits meet letters (`3d`,
/// `1er`), trailing periods dropped (`déc.`, `ven.`).
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        let word = word.trim_end_matches('.');
        let mut cur = String::new();
        let mut prev_digit: Option<bool> = None;
        for c in word.chars() {
            let digit = c.is_ascii_digit();
            let letter = c.is_alphabetic();
            if let Some(p) = prev_digit
                && ((p && letter)
                    || (!p && digit && cur.chars().last().is_some_and(char::is_alphabetic)))
            {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(c);
            if digit || letter {
                prev_digit = Some(digit);
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

fn weekday_of(word: &str) -> Option<i64> {
    Some(match word {
        "mon" | "monday" | "lun" | "lundi" => 0,
        "tue" | "tues" | "tuesday" | "mar" | "mardi" => 1,
        "wed" | "wednesday" | "mer" | "mercredi" => 2,
        "thu" | "thur" | "thurs" | "thursday" | "jeu" | "jeudi" => 3,
        "fri" | "friday" | "ven" | "vendredi" => 4,
        "sat" | "saturday" | "sam" | "samedi" => 5,
        "sun" | "sunday" | "dim" | "dimanche" => 6,
        _ => return None,
    })
}

fn month_of(word: &str) -> Option<u32> {
    Some(match word {
        "jan" | "january" | "janv" | "janvier" => 1,
        "feb" | "february" | "fev" | "fevr" | "fevrier" => 2,
        "mar" | "march" | "mars" => 3,
        "apr" | "april" | "avr" | "avril" => 4,
        "may" | "mai" => 5,
        "jun" | "june" | "juin" => 6,
        "jul" | "july" | "juil" | "juillet" => 7,
        "aug" | "august" | "aout" => 8,
        "sep" | "sept" | "september" | "septembre" => 9,
        "oct" | "october" | "octobre" => 10,
        "nov" | "november" | "novembre" => 11,
        "dec" | "december" | "decembre" => 12,
        _ => return None,
    })
}

enum Unit {
    Days,
    Weeks,
    Months,
}

fn unit_of(word: &str) -> Option<Unit> {
    Some(match word {
        "d" | "day" | "days" | "j" | "jour" | "jours" => Unit::Days,
        "w" | "week" | "weeks" | "sem" | "semaine" | "semaines" => Unit::Weeks,
        "m" | "month" | "months" | "mois" => Unit::Months,
        _ => return None,
    })
}

/// A plain number of at most `max_len` digits.
fn number(word: &str, max_len: usize) -> Option<i64> {
    (!word.is_empty() && word.len() <= max_len && word.bytes().all(|b| b.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

fn day_number(word: &str) -> Option<u32> {
    number(word, 2)
        .filter(|d| (1..=31).contains(d))
        .map(|d| d as u32)
}

/// A year as typed: four digits, or two meaning 20xx.
fn year_number(word: &str) -> Option<i64> {
    match word.len() {
        2 => number(word, 2).map(|y| 2000 + y),
        4 => number(word, 4),
        _ => None,
    }
}

fn offset(today: Date, n: i64, unit: Unit) -> Date {
    match unit {
        Unit::Days => today.add_days(n),
        Unit::Weeks => today.add_days(7 * n),
        Unit::Months => today.add_months(n),
    }
}

/// The next `day`/`month` on or after today, at most a few years ahead (a
/// 29 February waits for a leap year).
fn next_day_month(today: Date, day: u32, month: u32) -> Option<Date> {
    let year = today.ymd().0;
    (0..=4)
        .filter_map(|k| Date::from_ymd(year + k, month, day))
        .find(|d| *d >= today)
}

/// The next `day` of a month on or after today.
fn next_day(today: Date, day: u32) -> Option<Date> {
    let (y, m, _) = today.ymd();
    (0..=12)
        .filter_map(|k| {
            let index = y * 12 + i64::from(m) - 1 + k;
            Date::from_ymd(index.div_euclid(12), (index.rem_euclid(12) + 1) as u32, day)
        })
        .find(|d| *d >= today)
}

/// `15/12`, `15.12`, `15/12/2026`, `15/12/26`, `15.12.2026`.
fn numeric_date(word: &str, today: Date) -> Option<Option<Date>> {
    let sep = if word.contains('/') {
        '/'
    } else if word.contains('.') {
        '.'
    } else {
        return None;
    };
    let parts: Vec<&str> = word.split(sep).collect();
    let date = match parts[..] {
        [d, m] => {
            let (d, m) = (day_number(d)?, number(m, 2)? as u32);
            next_day_month(today, d, m)
        }
        [d, m, y] => Date::from_ymd(year_number(y)?, number(m, 2)? as u32, day_number(d)?),
        _ => return None,
    };
    Some(date)
}

fn parse_words(w: &[&str], today: Date) -> Option<Date> {
    // Single words first.
    if let [one] = w {
        let one = *one;
        match one {
            "today" | "tod" | "aujourd'hui" | "aujourdhui" | "auj" => return Some(today),
            "tomorrow" | "tom" | "tmr" | "demain" => return Some(today.add_days(1)),
            "apres-demain" => return Some(today.add_days(2)),
            "yesterday" | "hier" => return Some(today.add_days(-1)),
            "eom" => return Some(end_of_month(today)),
            _ => {}
        }
        if let Some(wd) = weekday_of(one) {
            return Some(next_weekday(today, wd));
        }
        if let Some(date) = Date::parse_iso(one) {
            return Some(date);
        }
        if let Some(sign) = one.strip_prefix('+') {
            return number(sign, 4).map(|n| today.add_days(n));
        }
        if let Some(rest) = one.strip_prefix('-') {
            return number(rest, 4).map(|n| today.add_days(-n));
        }
        if let Some(date) = numeric_date(one, today) {
            return date;
        }
        if let Some(d) = day_number(one) {
            return next_day(today, d);
        }
        return None;
    }
    match w {
        ["next", "week"] | ["semaine", "prochaine"] => {
            return Some(today.add_days(7 - today.weekday()));
        }
        ["next", "month"] | ["mois", "prochain"] => {
            let (y, m, _) = today.add_months(1).ymd();
            return Date::from_ymd(y, m, 1);
        }
        ["end", "of", "month"] | ["fin", "du", "mois"] => return Some(end_of_month(today)),
        ["next", wd] | [wd, "prochain"] => return weekday_of(wd).map(|d| next_weekday(today, d)),
        _ => {}
    }
    // Offsets: `+3 d`, `3 d`, `in 3 days`, `dans 2 sem`.
    let rest = match w {
        ["in" | "dans", rest @ ..] => rest,
        rest => rest,
    };
    if let [n, unit] = rest
        && let Some(unit) = unit_of(unit)
    {
        let (sign, digits) = match n.strip_prefix('+') {
            Some(d) => (1, d),
            None => match n.strip_prefix('-') {
                Some(d) => (-1, d),
                None => (1, *n),
            },
        };
        // `in -2 days` is not a form; a bare sign is only for `+3 d`.
        if rest.len() != w.len() && sign < 0 {
            return None;
        }
        return number(digits, 4).map(|n| offset(today, sign * n, unit));
    }
    // Day and month, with or without a year: `15 dec`, `dec 15`,
    // `1er mai`, `15 dec 2026`, `dec 15 2026`.
    let w: Vec<&str> = w.iter().copied().filter(|t| *t != "er").collect();
    let (day, month, year) = match w[..] {
        [d, m] => (d, m, None),
        [d, m, y] => (d, m, Some(y)),
        _ => return None,
    };
    let (day, month) = match (day_number(day), month_of(month)) {
        (Some(d), Some(m)) => (d, m),
        _ => match (month_of(day), day_number(month)) {
            (Some(m), Some(d)) => (d, m),
            _ => return None,
        },
    };
    match year {
        None => next_day_month(today, day, month),
        Some(y) => Date::from_ymd(year_number(y)?, month, day),
    }
}

fn end_of_month(today: Date) -> Date {
    let (y, m, _) = today.ymd();
    Date(days_from_civil(y, m, days_in_month(y, m)))
}

/// The next such weekday after today; today itself is never matched, so
/// `fri` on a Friday is in 7 days.
fn next_weekday(today: Date, weekday: i64) -> Date {
    let ahead = (weekday - today.weekday()).rem_euclid(7);
    today.add_days(if ahead == 0 { 7 } else { ahead })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i64, m: u32, day: u32) -> Date {
        Date::from_ymd(y, m, day).unwrap()
    }

    #[test]
    fn iso_dates_are_strict_and_in_range() {
        assert_eq!(Date::parse_iso("2026-12-15"), Some(d(2026, 12, 15)));
        assert_eq!(d(2026, 12, 15).to_string(), "2026-12-15");
        for bad in [
            "2026-02-31",
            "1999-12-31",
            "2100-01-01",
            "2026-1-15",
            "2026/12/15",
            "26-12-15",
            "2026-12-15 ",
            "",
        ] {
            assert_eq!(Date::parse_iso(bad), None, "{bad}");
        }
        assert_eq!(Date::parse_iso("2028-02-29"), Some(d(2028, 2, 29)));
        assert_eq!(Date::parse_iso("2000-01-01"), Some(d(2000, 1, 1)));
        assert_eq!(Date::parse_iso("2099-12-31"), Some(d(2099, 12, 31)));
    }

    #[test]
    fn weekdays_and_months() {
        // 2026-11-21 is a Saturday.
        assert_eq!(d(2026, 11, 21).weekday(), 5);
        assert_eq!(d(2026, 11, 23).weekday(), 0);
        assert_eq!(d(2026, 1, 31).add_months(1), d(2026, 2, 28));
        assert_eq!(d(2026, 12, 15).add_months(1), d(2027, 1, 15));
        assert_eq!(d(2027, 1, 15).add_months(-1), d(2026, 12, 15));
    }

    #[test]
    fn due_states_follow_the_thresholds() {
        let today = d(2026, 11, 21);
        let at = |n: i64| due_state(Some(today.add_days(n)), today, true);
        assert_eq!(due_state(None, today, true), DueState::None);
        assert_eq!(due_state(None, today, false), DueState::None);
        assert_eq!(at(-3), DueState::Overdue(3));
        assert_eq!(at(-1), DueState::Overdue(1));
        assert_eq!(at(0), DueState::Today);
        assert_eq!(at(1), DueState::Soon(1));
        assert_eq!(at(3), DueState::Soon(3));
        assert_eq!(at(4), DueState::Later(4));
        assert_eq!(at(400), DueState::Later(400));
        // A closed record is never overdue, whatever its date.
        assert_eq!(
            due_state(Some(today.add_days(-3)), today, false),
            DueState::Closed
        );
        assert_eq!(
            due_state(Some(today.add_days(5)), today, false),
            DueState::Closed
        );
    }

    #[test]
    fn epics_and_projects_are_open_until_complete() {
        use crate::model::Progress;
        let today = d(2026, 11, 21);
        let past = Some(today.add_days(-2));
        // No topic at all: open, so its date can be overdue.
        let empty = Progress { done: 0, total: 0 };
        assert_eq!(
            due_state(past, today, empty.is_open()),
            DueState::Overdue(2)
        );
        let half = Progress { done: 1, total: 2 };
        assert_eq!(due_state(past, today, half.is_open()), DueState::Overdue(2));
        let complete = Progress { done: 2, total: 2 };
        assert_eq!(due_state(past, today, complete.is_open()), DueState::Closed);
    }

    /// Central European time in 2026: UTC+1, UTC+2 from 29 March 01:00 UTC
    /// to 25 October 01:00 UTC.
    fn paris(ms: i64) -> i64 {
        let start = d(2026, 3, 29).days() * MS_PER_DAY + 3_600_000;
        let end = d(2026, 10, 25).days() * MS_PER_DAY + 3_600_000;
        if (start..end).contains(&ms) {
            7200
        } else {
            3600
        }
    }

    #[test]
    fn day_counts_ignore_daylight_saving_changes() {
        let zone = Zone::from_fn(paris);
        // 23:30 local on the Saturday before the spring change, and 00:30
        // local on the Monday after: two calendar days, 24 hours apart.
        let sat = d(2026, 3, 28).days() * MS_PER_DAY + 22 * 3_600_000 + 30 * 60_000;
        let mon = d(2026, 3, 30).days() * MS_PER_DAY - 90 * 60_000;
        assert_eq!(zone.date_of(sat), d(2026, 3, 28));
        assert_eq!(zone.date_of(mon), d(2026, 3, 30));
        assert_eq!(zone.date_of(sat).days_until(zone.date_of(mon)), 2);
        assert_eq!((mon - sat) / 3_600_000, 24);
        // A date due on the Monday, read late on the Saturday: in 2 days.
        assert_eq!(
            due_state(Some(d(2026, 3, 30)), zone.date_of(sat), true),
            DueState::Soon(2)
        );
        // Autumn: the 25-hour day still counts as one.
        let before = d(2026, 10, 24).days() * MS_PER_DAY + 21 * 3_600_000;
        let after = d(2026, 10, 26).days() * MS_PER_DAY + 3_600_000;
        assert_eq!(zone.date_of(before).days_until(zone.date_of(after)), 2);
    }

    #[test]
    fn local_midnights_move_with_the_offset() {
        let zone = Zone::from_fn(paris);
        for day in [
            d(2026, 3, 28),
            d(2026, 3, 29),
            d(2026, 3, 30),
            d(2026, 10, 25),
            d(2026, 10, 26),
        ] {
            let m = zone.midnight(day);
            assert_eq!(zone.date_of(m), day, "{day}");
            assert_eq!(zone.date_of(m - 1), day.add_days(-1), "{day}");
        }
        // Summer: local midnight is 22:00 UTC the day before.
        assert_eq!(
            zone.midnight(d(2026, 7, 1)),
            d(2026, 7, 1).days() * MS_PER_DAY - 2 * 3_600_000
        );
        assert_eq!(
            Zone::UTC.midnight(d(2026, 7, 1)),
            d(2026, 7, 1).days() * MS_PER_DAY
        );
    }

    #[test]
    fn a_clock_that_falls_back_across_midnight_starts_the_day_at_its_first_midnight() {
        // UTC+1 until local 00:30 on 1 July 2026, when the clock goes back
        // to 23:30 the day before (UTC+0): 00:00 to 00:30 on 1 July is
        // shown first, then 30 minutes of 30 June again.
        fn zone(ms: i64) -> i64 {
            let switch = d(2026, 7, 1).days() * MS_PER_DAY - 30 * 60_000;
            if ms < switch { 3600 } else { 0 }
        }
        let z = Zone::from_fn(zone);
        let day = d(2026, 7, 1);
        let first = day.days() * MS_PER_DAY - 3_600_000;
        assert_eq!(z.midnight(day), first);
        assert_eq!(z.date_of(first), day);
        assert_eq!(z.date_of(first - 1), day.add_days(-1));
        // A clock that skips midnight (00:00 to 01:00): the day starts at
        // the first instant that falls on it, local 01:00.
        fn skip(ms: i64) -> i64 {
            let switch = d(2026, 7, 1).days() * MS_PER_DAY;
            if ms < switch { 0 } else { 3600 }
        }
        let z = Zone::from_fn(skip);
        assert_eq!(z.midnight(day), day.days() * MS_PER_DAY);
        assert_eq!(z.date_of(z.midnight(day)), day);
        assert_eq!(z.date_of(z.midnight(day) - 1), day.add_days(-1));
    }

    /// Saturday 21 November 2026.
    const TODAY: (i64, u32, u32) = (2026, 11, 21);

    fn parsed(input: &str) -> Option<Date> {
        let (y, m, day) = TODAY;
        parse_typed(input, d(y, m, day))
    }

    #[test]
    fn every_row_of_the_typed_date_grammar() {
        let today = d(2026, 11, 21);
        let rows: &[(&str, Date)] = &[
            // Today, tomorrow, the day after.
            ("today", today),
            ("tod", today),
            ("aujourd'hui", today),
            ("auj", today),
            ("tomorrow", today.add_days(1)),
            ("tom", today.add_days(1)),
            ("tmr", today.add_days(1)),
            ("demain", today.add_days(1)),
            ("après-demain", today.add_days(2)),
            // Yesterday.
            ("yesterday", today.add_days(-1)),
            ("hier", today.add_days(-1)),
            // A weekday: the next one after today.
            ("fri", d(2026, 11, 27)),
            ("friday", d(2026, 11, 27)),
            ("next fri", d(2026, 11, 27)),
            ("ven", d(2026, 11, 27)),
            ("vendredi", d(2026, 11, 27)),
            ("vendredi prochain", d(2026, 11, 27)),
            ("mon", d(2026, 11, 23)),
            ("dimanche", d(2026, 11, 22)),
            // Offsets.
            ("+3", today.add_days(3)),
            ("3d", today.add_days(3)),
            ("3 d", today.add_days(3)),
            ("in 3d", today.add_days(3)),
            ("in 3 days", today.add_days(3)),
            ("in 2w", today.add_days(14)),
            ("in 2 weeks", today.add_days(14)),
            ("in 1m", d(2026, 12, 21)),
            ("in 1 month", d(2026, 12, 21)),
            ("-2", today.add_days(-2)),
            ("3j", today.add_days(3)),
            ("dans 3 j", today.add_days(3)),
            ("dans 3 jours", today.add_days(3)),
            ("dans 2 sem", today.add_days(14)),
            ("dans 2 semaines", today.add_days(14)),
            ("dans 1 mois", d(2026, 12, 21)),
            // Next week, next month, end of month.
            ("next week", d(2026, 11, 23)),
            ("semaine prochaine", d(2026, 11, 23)),
            ("next month", d(2026, 12, 1)),
            ("mois prochain", d(2026, 12, 1)),
            ("end of month", d(2026, 11, 30)),
            ("eom", d(2026, 11, 30)),
            ("fin du mois", d(2026, 11, 30)),
            // Day and month, no year: on or after today.
            ("15/12", d(2026, 12, 15)),
            ("15.12", d(2026, 12, 15)),
            ("15 dec", d(2026, 12, 15)),
            ("dec 15", d(2026, 12, 15)),
            ("15 december", d(2026, 12, 15)),
            ("15 déc", d(2026, 12, 15)),
            ("15 décembre", d(2026, 12, 15)),
            ("1er mai", d(2027, 5, 1)),
            ("21/11", today),
            ("20/11", d(2027, 11, 20)),
            // Day only.
            ("15", d(2026, 12, 15)),
            ("21", today),
            ("30", d(2026, 11, 30)),
            ("31", d(2026, 12, 31)),
            // With a year.
            ("15/12/2026", d(2026, 12, 15)),
            ("15/12/26", d(2026, 12, 15)),
            ("15 dec 2026", d(2026, 12, 15)),
            ("15 déc 2026", d(2026, 12, 15)),
            ("3/4/2025", d(2025, 4, 3)),
            // ISO.
            ("2026-12-15", d(2026, 12, 15)),
        ];
        for (typed, expected) in rows {
            assert_eq!(parsed(typed), Some(*expected), "{typed}");
        }
    }

    #[test]
    fn a_weekday_typed_on_that_weekday_is_next_week() {
        // TODAY is a Saturday.
        assert_eq!(parsed("sat"), Some(d(2026, 11, 28)));
        assert_eq!(parsed("samedi"), Some(d(2026, 11, 28)));
        assert_eq!(parsed("next saturday"), Some(d(2026, 11, 28)));
    }

    #[test]
    fn what_is_not_a_date() {
        for bad in [
            "31/02",
            "31/02/2027",
            "30 feb",
            "15/12/1999",
            "1/1/2100",
            "2100-01-01",
            "1999-12-31",
            "15 dec 1999",
            "friday the 13th",
            "soon",
            "",
            "32",
            "0",
            "15/13",
            "next",
            "in -2 days",
            "dans",
            "+",
        ] {
            assert_eq!(parsed(bad), None, "{bad:?}");
        }
        // Years at the edges.
        assert_eq!(parsed("31/12/2099"), Some(d(2099, 12, 31)));
        assert_eq!(parsed("1/1/2000"), Some(d(2000, 1, 1)));
        // An offset that leaves 2099 is not a date either.
        assert_eq!(parse_typed("+5", d(2099, 12, 30)), None);
    }

    #[test]
    fn mar_alone_is_tuesday_and_15_mar_is_march() {
        assert_eq!(parsed("mar"), Some(d(2026, 11, 24)));
        assert_eq!(parsed("mardi"), Some(d(2026, 11, 24)));
        assert_eq!(parsed("15 mar"), Some(d(2027, 3, 15)));
        assert_eq!(parsed("mar 15"), Some(d(2027, 3, 15)));
        assert_eq!(parsed("15 mars"), Some(d(2027, 3, 15)));
    }

    #[test]
    fn case_accents_and_trailing_periods_are_ignored() {
        assert_eq!(parsed("15 Déc."), Some(d(2026, 12, 15)));
        assert_eq!(parsed("15 DEC"), Some(d(2026, 12, 15)));
        assert_eq!(parsed("15 dec."), Some(d(2026, 12, 15)));
        assert_eq!(parsed("Ven."), Some(d(2026, 11, 27)));
        assert_eq!(parsed("AUJOURD\u{2019}HUI"), Some(d(2026, 11, 21)));
        assert_eq!(parsed("Après-Demain"), Some(d(2026, 11, 23)));
        assert_eq!(parsed("apres-demain"), Some(d(2026, 11, 23)));
        assert_eq!(parsed("  Tomorrow  "), Some(d(2026, 11, 22)));
        assert_eq!(parsed("15 févr."), Some(d(2027, 2, 15)));
        assert_eq!(parsed("15 août"), Some(d(2027, 8, 15)));
        assert_eq!(parsed("15.12."), Some(d(2026, 12, 15)));
    }

    #[test]
    fn months_keep_the_day_clamped_and_29_february_waits_for_a_leap_year() {
        let jan31 = d(2027, 1, 31);
        assert_eq!(parse_typed("in 1 month", jan31), Some(d(2027, 2, 28)));
        assert_eq!(
            parse_typed("dans 1 mois", d(2028, 1, 31)),
            Some(d(2028, 2, 29))
        );
        assert_eq!(parsed("29/02"), Some(d(2028, 2, 29)));
        assert_eq!(parsed("29/02/2027"), None);
        // `31` in a 30-day month skips to the next month that has one.
        assert_eq!(parse_typed("31", d(2026, 4, 2)), Some(d(2026, 5, 31)));
    }
}
