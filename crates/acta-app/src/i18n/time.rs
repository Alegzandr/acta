//! The time forms of docs/design/copy.md 10.2. Both languages use the 24-hour clock
//! and day-month-year order, whatever the OS region.
//!
//! Every function takes local wall-clock times ([`LocalTime`]) and, where
//! the form is relative, the current time as an argument, so the rules are
//! tested against fixed clocks. Converting a stored instant to local time
//! is the caller's (with the offset the OS gives).

use super::{Arg, Key, Locale, tr};

/// A moment: its local date and time (what is shown, and what the day
/// bands count in) and the instant itself (what the minute and hour bands
/// count in, so a change of UTC offset, as at a DST switch, does not move
/// them). A date out of range (month 0 or 13) never panics; it shows
/// clamped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    /// Seconds since the Unix epoch.
    pub unix: i64,
}

impl LocalTime {
    /// A moment at UTC+0, to the second.
    pub const fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        let unix = days_from_civil(year, month, day) * 86_400
            + hour as i64 * 3600
            + minute as i64 * 60
            + second as i64;
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            unix,
        }
    }

    /// A moment at UTC+0, to the minute.
    pub const fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Self {
        Self::at(year, month, day, hour, minute, 0)
    }

    /// From seconds since the Unix epoch and the local offset from UTC.
    pub fn from_unix(seconds: i64, offset_seconds: i32) -> Self {
        let local = seconds + offset_seconds as i64;
        let days = local.div_euclid(86_400);
        let of_day = local.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: (of_day / 3600) as u8,
            minute: (of_day / 60 % 60) as u8,
            second: (of_day % 60) as u8,
            unix: seconds,
        }
    }

    /// Days since 1970-01-01 of the local date.
    pub fn day_number(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// 0 is Monday, 6 is Sunday.
    pub fn weekday(self) -> usize {
        // 1970-01-01 was a Thursday.
        (self.day_number() + 3).rem_euclid(7) as usize
    }
}

/// Howard Hinnant's `days_from_civil`.
const fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = year as i64 - (month <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month, day)
}

const WEEKDAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const WEEKDAYS_EN_FULL: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const WEEKDAYS_FR: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
const WEEKDAYS_FR_FULL: [&str; 7] = [
    "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
];
const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTHS_EN_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTHS_FR: [&str; 12] = [
    "janv.",
    "f\u{e9}vr.",
    "mars",
    "avr.",
    "mai",
    "juin",
    "juil.",
    "ao\u{fb}t",
    "sept.",
    "oct.",
    "nov.",
    "d\u{e9}c.",
];
const MONTHS_FR_FULL: [&str; 12] = [
    "janvier",
    "f\u{e9}vrier",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "ao\u{fb}t",
    "septembre",
    "octobre",
    "novembre",
    "d\u{e9}cembre",
];

/// "14:05".
pub fn clock(at: LocalTime) -> String {
    format!("{:02}:{:02}", at.hour, at.minute)
}

/// "Mon", "lun.": a day of the coming week (plan usage resets).
pub fn weekday_short(locale: Locale, at: LocalTime) -> &'static str {
    match locale {
        Locale::En => WEEKDAYS_EN[at.weekday()],
        Locale::Fr => WEEKDAYS_FR[at.weekday()],
    }
}

/// The day of the month; "1er" for the first in French (10.2).
fn day_of_month(locale: Locale, day: u8) -> String {
    match (locale, day) {
        (Locale::Fr, 1) => "1er".into(),
        _ => day.to_string(),
    }
}

/// 0 to 11, for a month that may be out of range.
fn month_index(month: u8) -> usize {
    (month as usize).clamp(1, 12) - 1
}

fn month_short(locale: Locale, month: u8) -> &'static str {
    let i = month_index(month);
    match locale {
        Locale::En => MONTHS_EN[i],
        Locale::Fr => MONTHS_FR[i],
    }
}

/// The first letter in upper case, for a form that starts a string or
/// stands alone ("Yesterday 18:42", "Hier à 18:42").
pub fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `{date}`: "18 Nov 2026" / "18 nov. 2026", no article.
pub fn date(locale: Locale, at: LocalTime) -> String {
    format!(
        "{} {} {}",
        day_of_month(locale, at.day),
        month_short(locale, at.month),
        at.year
    )
}

/// `{datetime}`: "18 Nov 2026, 09:12" / "18 nov. 2026 à 09:12".
pub fn datetime(locale: Locale, at: LocalTime) -> String {
    let date = date(locale, at);
    let time = clock(at);
    tr(
        locale,
        Key::TimeDatetime,
        &[("date", Arg::Text(&date)), ("time", Arg::Text(&time))],
    )
}

/// "today, 09:12" / "aujourd'hui à 09:12" (the snapshot line only).
pub fn today_with_time(locale: Locale, at: LocalTime) -> String {
    let time = clock(at);
    tr(locale, Key::TimeToday, &[("time", Arg::Text(&time))])
}

/// Whole days, for the age of the last export: "12 days ago", "today".
pub fn days_ago(locale: Locale, days: u64) -> String {
    if days == 0 {
        tr(locale, Key::TimeTodayWord, &[])
    } else {
        tr(locale, Key::TimeDays, &[("n", Arg::Count(days))])
    }
}

/// Tooltips: "Tuesday 17 November 2026, 14:05" /
/// "mardi 17 novembre 2026 à 14:05".
pub fn full(locale: Locale, at: LocalTime) -> String {
    let (weekday, month) = match locale {
        Locale::En => (WEEKDAYS_EN_FULL, MONTHS_EN_FULL),
        Locale::Fr => (WEEKDAYS_FR_FULL, MONTHS_FR_FULL),
    };
    let date = format!(
        "{} {} {} {}",
        weekday[at.weekday()],
        day_of_month(locale, at.day),
        month[month_index(at.month)],
        at.year
    );
    let time = clock(at);
    tr(
        locale,
        Key::TimeDatetime,
        &[("date", Arg::Text(&date)), ("time", Arg::Text(&time))],
    )
}

/// How old a moment is, in the bands of the 10.2 table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Age {
    Now,
    Minutes(u64),
    Hours(u64),
    Yesterday,
    ThisWeek,
    ThisYear,
    Older,
}

fn age(then: LocalTime, now: LocalTime) -> Age {
    let seconds = now.unix - then.unix;
    // A moment in the future (a clock that moved back) reads as now.
    if seconds < 60 {
        return Age::Now;
    }
    if seconds < 3600 {
        return Age::Minutes((seconds / 60) as u64);
    }
    match now.day_number() - then.day_number() {
        0 => Age::Hours((seconds / 3600) as u64),
        1 => Age::Yesterday,
        2..=6 => Age::ThisWeek,
        _ if then.year == now.year => Age::ThisYear,
        _ => Age::Older,
    }
}

/// `{when}`, the long form, as it reads inside a sentence ("Updated 2 h ago
/// by Claude Code", "Modifié mardi à 14:05 par Claude Code"). Lower case;
/// pass it through [`capitalize`] where it starts a string or stands alone.
pub fn long(locale: Locale, then: LocalTime, now: LocalTime) -> String {
    let time = clock(then);
    match age(then, now) {
        Age::Now => tr(locale, Key::TimeNow, &[]),
        Age::Minutes(n) => tr(locale, Key::TimeMinutes, &[("n", Arg::Count(n))]),
        Age::Hours(n) => tr(locale, Key::TimeHours, &[("n", Arg::Count(n))]),
        Age::Yesterday => tr(locale, Key::TimeYesterday, &[("time", Arg::Text(&time))]),
        Age::ThisWeek => {
            let weekday = match locale {
                Locale::En => WEEKDAYS_EN[then.weekday()],
                Locale::Fr => WEEKDAYS_FR_FULL[then.weekday()],
            };
            tr(
                locale,
                Key::TimeWeekday,
                &[("weekday", Arg::Text(weekday)), ("time", Arg::Text(&time))],
            )
        }
        Age::ThisYear | Age::Older => {
            let date = date(locale, then);
            tr(locale, Key::TimeDate, &[("date", Arg::Text(&date))])
        }
    }
}

/// `{when_short}`, the compact form for the fixed time columns: "2 h ago",
/// "Yesterday", "Tue", "12 Nov", "Feb 2025". Always stands alone, so it
/// starts with a capital.
pub fn compact(locale: Locale, then: LocalTime, now: LocalTime) -> String {
    let text = match age(then, now) {
        Age::Now | Age::Minutes(_) | Age::Hours(_) => long(locale, then, now),
        Age::Yesterday => tr(locale, Key::TimeShortYesterday, &[]),
        Age::ThisWeek => match locale {
            Locale::En => WEEKDAYS_EN[then.weekday()].into(),
            Locale::Fr => WEEKDAYS_FR[then.weekday()].into(),
        },
        Age::ThisYear => format!(
            "{} {}",
            day_of_month(locale, then.day),
            month_short(locale, then.month)
        ),
        Age::Older => format!("{} {}", month_short(locale, then.month), then.year),
    };
    capitalize(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: Locale = Locale::En;
    const FR: Locale = Locale::Fr;
    /// The fixed clock: Friday 20 November 2026, 15:00.
    const NOW: LocalTime = LocalTime::new(2026, 11, 20, 15, 0);

    fn at(month: u8, day: u8, hour: u8, minute: u8) -> LocalTime {
        LocalTime::new(2026, month, day, hour, minute)
    }

    fn both(then: LocalTime) -> [String; 4] {
        [
            long(EN, then, NOW),
            long(FR, then, NOW),
            compact(EN, then, NOW),
            compact(FR, then, NOW),
        ]
    }

    #[test]
    fn civil_dates_round_trip() {
        for days in [-1, 0, 59, 365, 10_957, 20_777, 2_932_896] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        // 2026-11-20 00:00 UTC.
        assert_eq!(
            LocalTime::from_unix(1_795_132_800, 0),
            LocalTime::new(2026, 11, 20, 0, 0)
        );
        // An hour east of UTC moves the wall clock forward, not the instant.
        let east = LocalTime::from_unix(1_795_132_800 - 1, 3600);
        let wall = (east.day, east.hour, east.minute, east.second);
        assert_eq!(wall, (20, 0, 59, 59));
        assert_eq!(east.unix, 1_795_132_799);
        assert_eq!(NOW.weekday(), 4, "20 Nov 2026 is a Friday");
    }

    #[test]
    fn under_a_minute() {
        let then = LocalTime::at(2026, 11, 20, 14, 59, 30);
        assert_eq!(
            both(then),
            [
                "just now",
                "\u{e0} l\u{2019}instant",
                "Just now",
                "\u{c0} l\u{2019}instant"
            ]
        );
        // A moment in the future reads as now.
        assert_eq!(long(EN, at(11, 20, 16, 0), NOW), "just now");
    }

    #[test]
    fn under_an_hour() {
        assert_eq!(
            both(at(11, 20, 14, 48)),
            ["12 min ago", "il y a 12 min", "12 min ago", "Il y a 12 min"]
        );
        assert_eq!(compact(FR, at(11, 20, 14, 1), NOW), "Il y a 59 min");
        // Under an hour wins over "yesterday" across midnight.
        let just_after_midnight = LocalTime::new(2026, 11, 21, 0, 10);
        assert_eq!(
            long(EN, at(11, 20, 23, 50), just_after_midnight),
            "20 min ago"
        );
    }

    #[test]
    fn earlier_today() {
        assert_eq!(
            both(at(11, 20, 12, 30)),
            ["2 h ago", "il y a 2 h", "2 h ago", "Il y a 2 h"]
        );
    }

    #[test]
    fn yesterday() {
        assert_eq!(
            both(at(11, 19, 18, 42)),
            ["yesterday 18:42", "hier \u{e0} 18:42", "Yesterday", "Hier"]
        );
        assert_eq!(
            capitalize(&long(EN, at(11, 19, 18, 42), NOW)),
            "Yesterday 18:42"
        );
        assert_eq!(
            capitalize(&long(FR, at(11, 19, 18, 42), NOW)),
            "Hier \u{e0} 18:42"
        );
    }

    #[test]
    fn two_to_six_days_ago() {
        // Tuesday 17 November.
        assert_eq!(
            both(at(11, 17, 14, 5)),
            ["Tue 14:05", "mardi \u{e0} 14:05", "Tue", "Mar."]
        );
        assert_eq!(
            capitalize(&long(FR, at(11, 17, 14, 5), NOW)),
            "Mardi \u{e0} 14:05"
        );
        // Six days back is still the weekday; seven is a date.
        assert_eq!(compact(EN, at(11, 14, 9, 0), NOW), "Sat");
        assert_eq!(compact(EN, at(11, 13, 9, 0), NOW), "13 Nov");
    }

    #[test]
    fn older_this_year() {
        assert_eq!(
            both(at(11, 12, 9, 0)),
            ["12 Nov 2026", "le 12 nov. 2026", "12 Nov", "12 nov."]
        );
        assert_eq!(
            capitalize(&long(FR, at(11, 12, 9, 0), NOW)),
            "Le 12 nov. 2026"
        );
    }

    #[test]
    fn older_another_year() {
        let then = LocalTime::new(2025, 2, 3, 9, 0);
        assert_eq!(
            both(then),
            [
                "3 Feb 2025",
                "le 3 f\u{e9}vr. 2025",
                "Feb 2025",
                "F\u{e9}vr. 2025"
            ]
        );
    }

    #[test]
    fn the_first_of_the_month_is_1er_in_french() {
        let then = at(11, 1, 9, 0);
        assert_eq!(long(FR, then, NOW), "le 1er nov. 2026");
        assert_eq!(compact(FR, then, NOW), "1er nov.");
        assert_eq!(long(EN, then, NOW), "1 Nov 2026");
        assert_eq!(compact(EN, then, NOW), "1 Nov");
        assert_eq!(date(FR, then), "1er nov. 2026");
    }

    #[test]
    fn minutes_and_hours_count_real_time_across_a_dst_change() {
        // 29 Mar 2026: Paris moves from UTC+1 to UTC+2 at 02:00 local.
        let then = LocalTime::from_unix(1_774_742_400, 3600); // 01:00 local
        let now = LocalTime::from_unix(1_774_742_400 + 90 * 60, 7200); // 03:30 local
        assert_eq!((then.hour, now.hour, now.minute), (1, 3, 30));
        // 2 h 30 on the wall clock, 1 h 30 in fact.
        assert_eq!(long(EN, then, now), "1 h ago");
        let now = LocalTime::from_unix(1_774_742_400 + 50 * 60, 7200); // 02:50 local
        assert_eq!(long(EN, then, now), "50 min ago");
    }

    #[test]
    fn day_bands_count_local_dates_across_midnight_and_years() {
        // Just after midnight on New Year's Day.
        let now = LocalTime::new(2027, 1, 1, 0, 30);
        let then = LocalTime::new(2026, 12, 31, 22, 0);
        assert_eq!(long(EN, then, now), "yesterday 22:00");
        assert_eq!(long(FR, then, now), "hier \u{e0} 22:00");
        // Four days back across the year: the weekday (Tue 29 Dec 2026).
        let now = LocalTime::new(2027, 1, 2, 12, 0);
        let then = LocalTime::new(2026, 12, 29, 9, 15);
        assert_eq!(long(EN, then, now), "Tue 09:15");
        assert_eq!(compact(FR, then, now), "Mar.");
        // Older and in another year.
        let now = LocalTime::new(2027, 1, 10, 12, 0);
        let then = LocalTime::new(2026, 12, 20, 9, 0);
        assert_eq!(compact(EN, then, now), "Dec 2026");
        assert_eq!(compact(FR, then, now), "D\u{e9}c. 2026");
        assert_eq!(long(EN, then, now), "20 Dec 2026");
        // Midnight this morning is earlier today.
        assert_eq!(long(EN, at(11, 20, 0, 0), NOW), "15 h ago");
    }

    #[test]
    fn an_out_of_range_month_does_not_panic() {
        for month in [0, 13, 255] {
            let then = LocalTime::new(2026, month, 1, 9, 0);
            let _ = (long(FR, then, NOW), compact(EN, then, NOW), full(FR, then));
            let _ = (date(EN, then), datetime(FR, then));
        }
    }

    #[test]
    fn absolute_forms() {
        let then = at(11, 18, 9, 12);
        assert_eq!(date(EN, then), "18 Nov 2026");
        assert_eq!(date(FR, then), "18 nov. 2026");
        assert_eq!(datetime(EN, then), "18 Nov 2026, 09:12");
        assert_eq!(datetime(FR, then), "18 nov. 2026 \u{e0} 09:12");
        assert_eq!(today_with_time(EN, then), "today, 09:12");
        assert_eq!(today_with_time(FR, then), "aujourd\u{2019}hui \u{e0} 09:12");
        let tuesday = at(11, 17, 14, 5);
        assert_eq!(full(EN, tuesday), "Tuesday 17 November 2026, 14:05");
        assert_eq!(full(FR, tuesday), "mardi 17 novembre 2026 \u{e0} 14:05");
    }

    #[test]
    fn whole_days() {
        assert_eq!(days_ago(EN, 0), "today");
        assert_eq!(days_ago(FR, 0), "aujourd\u{2019}hui");
        assert_eq!(days_ago(EN, 1), "1 day ago");
        assert_eq!(days_ago(EN, 12), "12 days ago");
        assert_eq!(days_ago(FR, 1), "il y a 1 jour");
        assert_eq!(days_ago(FR, 12), "il y a 12 jours");
    }
}
