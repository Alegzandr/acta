//! Due dates in the interface (docs/design/components-records.md 5: Due
//! row, due date picker, topic table; layout.md 4.2.1: due marks; copy.md
//! 10.2.1): the words and tones of each state, the forms of a day, the
//! picker's model, and today's date, which is looked at again at local
//! midnight, on resume and when the clock or the time zone changes.
//!
//! Everything but [`start`] is pure: `today` is passed in, so every
//! threshold and the midnight change are tested without a window. The
//! states themselves come from `acta_core::date::due_state`, which the
//! agent shares, so the page and Claude Code always agree.

use std::time::Duration;

use acta_core::date::{Date, DueState, Zone, due_state, parse_typed};
use gpui_kit::{App, Global};

use crate::i18n::time::{self, LocalTime};
use crate::i18n::{Arg, Key, Locale, text, tr};

// ---- today ----

/// The local date the interface reads relative forms against.
pub struct Today(pub Date);

impl Global for Today {}

/// Today, as last looked at.
pub fn today(cx: &App) -> Date {
    cx.try_global::<Today>()
        .map_or_else(|| Zone::system().today(), |t| t.0)
}

/// How long to wait before looking at the date again: until just after the
/// next local midnight, and never more than a minute, so a resume from
/// sleep or a change of the clock or the time zone shows within a minute.
pub fn next_check(now_ms: i64, zone: Zone) -> Duration {
    const MAX_MS: i64 = 60_000;
    let next_midnight = zone.midnight(zone.date_of(now_ms).add_days(1));
    let until = (next_midnight - now_ms).clamp(0, MAX_MS);
    // A few milliseconds past midnight, so the date read is the new one.
    Duration::from_millis(until as u64 + 20)
}

/// Sets [`Today`] and keeps it current. Observers of the global (pages
/// whose counts depend on the day) reload; every window redraws its
/// relative forms and marks, without moving anything.
pub fn start(cx: &mut App) {
    cx.set_global(Today(Zone::system().today()));
    cx.spawn(async move |cx| {
        loop {
            let wait = next_check(acta_core::time::now_ms(), Zone::system());
            cx.background_executor().timer(wait).await;
            let now = Zone::system().today();
            cx.update(|cx| {
                if today(cx) != now {
                    cx.set_global(Today(now));
                    cx.refresh_windows();
                }
            });
        }
    })
    .detach();
}

// ---- forms (copy.md 10.2.1) ----

fn local(day: Date) -> LocalTime {
    let (y, m, d) = day.ymd();
    LocalTime::new(y as i32, m as u8, d as u8, 0, 0)
}

/// `{date}`: "15 Dec 2026" / "15 déc. 2026".
pub fn date(locale: Locale, day: Date) -> String {
    time::date(locale, local(day))
}

/// The picker's preview: "Friday 27 November 2026".
pub fn full(locale: Locale, day: Date) -> String {
    time::day_full(locale, local(day))
}

/// The Due column's compact form: "Today", "Fri", "15 Dec", "Feb 2027".
pub fn compact(locale: Locale, day: Date, today: Date) -> String {
    time::due_compact(locale, local(day), local(today))
}

/// The picker's month header: "November 2026".
pub fn month_header(locale: Locale, first: Date) -> String {
    let (y, m, _) = first.ymd();
    time::month_year(locale, y as i32, m as u8)
}

/// The tone a phrase or a cell is drawn in: `warning` for overdue, then
/// `text.secondary` and `text.tertiary` (components-records.md 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Warning,
    Secondary,
    Tertiary,
}

/// The relative phrase of the Due row and of the picker's preview, with
/// its tone. `None` without a date and on a closed record, which shows its
/// date alone.
pub fn phrase(locale: Locale, state: DueState) -> Option<(String, Tone)> {
    let days = |n: i64| [("n", Arg::Count(n.unsigned_abs()))];
    Some(match state {
        DueState::None | DueState::Closed => return None,
        DueState::Overdue(n) => (tr(locale, Key::DueOverdue, &days(n)), Tone::Warning),
        DueState::Today => (text(locale, Key::DueToday), Tone::Secondary),
        DueState::Soon(1) => (text(locale, Key::DueTomorrow), Tone::Secondary),
        DueState::Soon(n) => (tr(locale, Key::DueInDays, &days(n)), Tone::Secondary),
        DueState::Later(n) => (tr(locale, Key::DueInDays, &days(n)), Tone::Tertiary),
    })
}

/// The sentence form, for tooltips of tree marks and table cells: "Overdue
/// by 3 days, due 18 Nov 2026", "Due today", "Due 27 Nov 2026, in 6 days";
/// on a closed record "Due 18 Nov 2026", never overdue.
pub fn sentence(locale: Locale, day: Date, state: DueState) -> String {
    let date = date(locale, day);
    let with = |key, n: i64| {
        tr(
            locale,
            key,
            &[
                ("n", Arg::Count(n.unsigned_abs())),
                ("date", Arg::Text(&date)),
            ],
        )
    };
    match state {
        DueState::Overdue(n) => with(Key::DueTipOverdue, n),
        DueState::Today => text(locale, Key::DueTipToday),
        DueState::Soon(1) => text(locale, Key::DueTipTomorrow),
        DueState::Soon(n) | DueState::Later(n) => with(Key::DueTipInDays, n),
        DueState::None | DueState::Closed => {
            tr(locale, Key::DueTipDate, &[("date", Arg::Text(&date))])
        }
    }
}

/// A cell of the Due column (components-records.md 5, Due column).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub tone: Tone,
    /// `clock-alert` before the text: an overdue open record.
    pub alert: bool,
    pub tip: String,
}

/// The Due cell of a record (a topic, or an epic of the Overview); `None`
/// without a date: an empty cell.
pub fn cell(locale: Locale, due: Option<Date>, today: Date, open: bool) -> Option<Cell> {
    let day = due?;
    let state = due_state(due, today, open);
    let tone = match state {
        DueState::Overdue(_) => Tone::Warning,
        DueState::Today | DueState::Soon(_) => Tone::Secondary,
        _ => Tone::Tertiary,
    };
    Some(Cell {
        text: compact(locale, day, today),
        tone,
        alert: matches!(state, DueState::Overdue(_)),
        tip: sentence(locale, day, state),
    })
}

// ---- the picker (components-records.md 5, Due date picker) ----

/// What the preview line says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preview {
    /// Nothing typed: the record's date, if any.
    Empty(Option<Date>),
    /// The day `Enter` would set.
    Day(Date),
    /// Not a date.
    Invalid,
    /// A date, but its year is outside 2000 to 2099.
    Range,
}

/// The preview of what is typed, parsed with the grammar of 10.2.1.
pub fn preview(input: &str, today: Date, current: Option<Date>) -> Preview {
    let text = input.trim();
    if text.is_empty() {
        return Preview::Empty(current);
    }
    if let Some(day) = parse_typed(text, today) {
        return Preview::Day(day);
    }
    if year_out_of_range(text, today) {
        Preview::Range
    } else {
        Preview::Invalid
    }
}

/// Whether `text` would be a date with a year in range: each four-digit
/// year outside 2000 to 2099 is put in range and the text parsed again.
fn year_out_of_range(text: &str, today: Date) -> bool {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut digits = String::new();
    let mut flush = |digits: &mut String, out: &mut String| {
        let year: Option<i64> = (digits.len() == 4).then(|| digits.parse().ok()).flatten();
        match year {
            Some(y) if !(Date::MIN_YEAR..=Date::MAX_YEAR).contains(&y) => {
                out.push_str("2050");
                changed = true;
            }
            _ => out.push_str(digits),
        }
        digits.clear();
    };
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            flush(&mut digits, &mut out);
            out.push(ch);
        }
    }
    flush(&mut digits, &mut out);
    changed && parse_typed(&out, today).is_some()
}

/// Which part of the picker has the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Input,
    Grid,
    Clear,
}

/// A key the grid reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridKey {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    YearUp,
    YearDown,
    Today,
    Enter,
    Clear,
}

/// What a key or a click asks the host to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    /// Close and change nothing.
    Close,
    /// Write this date (`None` clears), then close.
    Set(Option<Date>),
}

/// The picker's state, without a window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picker {
    pub today: Date,
    /// The record's date when the picker opened.
    pub current: Option<Date>,
    pub preview: Preview,
    /// `Enter` was pressed on text that does not parse: the error state.
    pub error: bool,
    /// The first day of the month shown.
    pub month: Date,
    /// The grid's keyboard cursor.
    pub cursor: Date,
    pub area: Area,
}

fn first_of(day: Date) -> Date {
    day.add_days(1 - i64::from(day.ymd().2))
}

fn in_range(day: Date) -> Date {
    let min = Date::from_ymd(Date::MIN_YEAR, 1, 1).unwrap_or(day);
    let max = Date::from_ymd(Date::MAX_YEAR, 12, 31).unwrap_or(day);
    day.clamp(min, max)
}

impl Picker {
    pub fn new(today: Date, current: Option<Date>) -> Self {
        let shown = current.unwrap_or(today);
        Self {
            today,
            current,
            preview: Preview::Empty(current),
            error: false,
            month: first_of(shown),
            cursor: shown,
            area: Area::Input,
        }
    }

    /// The day `Enter` would set: the parsed input, else the record's date.
    pub fn selected(&self) -> Option<Date> {
        match self.preview {
            Preview::Day(day) => Some(day),
            _ => self.current,
        }
    }

    /// The input changed: parse it on this frame; a date turns the page to
    /// its month. Text that is no date (or out of range) turns it back to
    /// the month the picker opened on, never leaving it on the month of a
    /// prefix ("15/12/21" of "15/12/2150", QA 2026-11-22).
    pub fn input(&mut self, text: &str) {
        self.preview = preview(text, self.today, self.current);
        self.error = false;
        self.month = match self.preview {
            Preview::Day(day) => first_of(day),
            Preview::Invalid | Preview::Range => first_of(self.current.unwrap_or(self.today)),
            Preview::Empty(_) => self.month,
        };
    }

    fn set(&self, day: Option<Date>) -> Outcome {
        if day == self.current {
            Outcome::Close
        } else {
            Outcome::Set(day)
        }
    }

    /// `Enter` in the input.
    pub fn enter(&mut self) -> Outcome {
        match self.preview {
            Preview::Day(day) => self.set(Some(day)),
            Preview::Empty(_) => Outcome::Close,
            Preview::Invalid | Preview::Range => {
                self.error = true;
                Outcome::Stay
            }
        }
    }

    /// `Down` in the input: the cursor goes into the grid, on the selected
    /// day, else today.
    pub fn down(&mut self) {
        self.area = Area::Grid;
        self.move_to(self.selected().unwrap_or(self.today));
    }

    /// The footer's Clear exists only when the record has a date.
    pub fn has_clear(&self) -> bool {
        self.current.is_some()
    }

    /// `Tab` / `Shift+Tab`: input, grid, Clear, in a cycle.
    pub fn tab(&mut self, back: bool) {
        let mut areas = vec![Area::Input, Area::Grid];
        if self.has_clear() {
            areas.push(Area::Clear);
        }
        let at = areas.iter().position(|a| *a == self.area).unwrap_or(0);
        let next = if back {
            (at + areas.len() - 1) % areas.len()
        } else {
            (at + 1) % areas.len()
        };
        self.area = areas[next];
        if self.area == Area::Grid {
            self.move_to(self.cursor);
        }
    }

    /// Clear the date (the Clear button, `Delete` in the grid).
    pub fn clear(&self) -> Outcome {
        if self.current.is_some() {
            Outcome::Set(None)
        } else {
            Outcome::Stay
        }
    }

    /// A click on a day.
    pub fn pick(&self, day: Date) -> Outcome {
        self.set(Some(day))
    }

    /// The chevrons: the page turns, the cursor stays.
    pub fn turn(&mut self, months: i64) {
        let month = in_range(self.month.add_months(months));
        self.month = first_of(month);
    }

    /// The 42 days of the grid: 6 weeks from the Monday on or before the
    /// first of the month shown.
    pub fn grid(&self) -> [Date; 42] {
        let start = self.month.add_days(-self.month.weekday());
        std::array::from_fn(|i| start.add_days(i as i64))
    }

    /// Whether a day of the grid belongs to an adjacent month.
    pub fn outside(&self, day: Date) -> bool {
        first_of(day) != self.month
    }

    fn move_to(&mut self, day: Date) {
        self.cursor = in_range(day);
        if first_of(self.cursor) != self.month {
            self.month = first_of(self.cursor);
        }
    }

    /// A key in the grid.
    pub fn grid_key(&mut self, key: GridKey) -> Outcome {
        let c = self.cursor;
        match key {
            GridKey::Left => self.move_to(c.add_days(-1)),
            GridKey::Right => self.move_to(c.add_days(1)),
            GridKey::Down => self.move_to(c.add_days(7)),
            GridKey::Up if c < self.grid()[7] => self.area = Area::Input,
            GridKey::Up => self.move_to(c.add_days(-7)),
            GridKey::Home => self.move_to(c.add_days(-c.weekday())),
            GridKey::End => self.move_to(c.add_days(6 - c.weekday())),
            GridKey::PageUp => self.move_to(c.add_months(-1)),
            GridKey::PageDown => self.move_to(c.add_months(1)),
            GridKey::YearUp => self.move_to(c.add_months(-12)),
            GridKey::YearDown => self.move_to(c.add_months(12)),
            GridKey::Today => self.move_to(self.today),
            GridKey::Enter => return self.set(Some(c)),
            GridKey::Clear => return self.clear(),
        }
        Outcome::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_out_of_range_turns_the_month_back_not_to_a_prefix() {
        let today = Date::from_ymd(2026, 11, 22).unwrap();
        let mut picker = Picker::new(today, None);
        picker.input("15/12/21");
        assert_eq!(picker.month, Date::from_ymd(2021, 12, 1).unwrap());
        picker.input("15/12/2150");
        assert_eq!(picker.preview, Preview::Range);
        assert_eq!(picker.month, Date::from_ymd(2026, 11, 1).unwrap());
        picker.input("15/12/2026x");
        assert_eq!(picker.month, Date::from_ymd(2026, 11, 1).unwrap());
    }

    const EN: Locale = Locale::En;
    const FR: Locale = Locale::Fr;

    fn day(iso: &str) -> Date {
        Date::parse_iso(iso).unwrap()
    }

    /// The drawings' today: Saturday 21 November 2026.
    fn today() -> Date {
        day("2026-11-21")
    }

    fn phrase_at(iso: &str, open: bool) -> Option<(String, Tone)> {
        phrase(EN, due_state(Some(day(iso)), today(), open))
    }

    #[test]
    fn the_due_row_phrase_and_its_tone_follow_the_thresholds() {
        let p = |s: &str, tone| Some((s.to_owned(), tone));
        assert_eq!(
            phrase_at("2026-11-18", true),
            p("Overdue by 3 days", Tone::Warning)
        );
        assert_eq!(
            phrase_at("2026-11-20", true),
            p("Overdue by 1 day", Tone::Warning)
        );
        assert_eq!(phrase_at("2026-11-21", true), p("Today", Tone::Secondary));
        assert_eq!(
            phrase_at("2026-11-22", true),
            p("Tomorrow", Tone::Secondary)
        );
        assert_eq!(
            phrase_at("2026-11-23", true),
            p("In 2 days", Tone::Secondary)
        );
        assert_eq!(
            phrase_at("2026-11-24", true),
            p("In 3 days", Tone::Secondary)
        );
        assert_eq!(
            phrase_at("2026-11-25", true),
            p("In 4 days", Tone::Tertiary)
        );
        assert_eq!(
            phrase_at("2026-12-15", true),
            p("In 24 days", Tone::Tertiary)
        );
        // A closed record shows its date alone, whatever it is.
        assert_eq!(phrase_at("2026-11-18", false), None);
        assert_eq!(phrase(EN, due_state(None, today(), true)), None);
        let fr = phrase(FR, due_state(Some(day("2026-11-18")), today(), true));
        assert_eq!(fr.unwrap().0, "En retard de 3 jours");
        let fr = phrase(FR, due_state(Some(day("2026-11-21")), today(), true));
        assert_eq!(fr.unwrap().0, "Aujourd\u{2019}hui");
    }

    #[test]
    fn day_counts_are_calendar_days_across_a_daylight_saving_change() {
        // Europe/Paris falls back on 25 October 2026: a date across it is
        // still a whole number of days.
        let state = due_state(Some(day("2026-10-26")), day("2026-10-24"), true);
        assert_eq!(phrase(EN, state).unwrap().0, "In 2 days");
        let state = due_state(Some(day("2026-03-27")), day("2026-03-30"), true);
        assert_eq!(phrase(EN, state).unwrap().0, "Overdue by 3 days");
    }

    #[test]
    fn tooltips_are_the_sentence_form_and_a_closed_record_is_never_overdue() {
        let s = |iso: &str, open| {
            let d = day(iso);
            sentence(EN, d, due_state(Some(d), today(), open))
        };
        assert_eq!(s("2026-11-18", true), "Overdue by 3 days, due 18 Nov 2026");
        assert_eq!(s("2026-11-21", true), "Due today");
        assert_eq!(s("2026-11-22", true), "Due tomorrow");
        assert_eq!(s("2026-11-27", true), "Due 27 Nov 2026, in 6 days");
        assert_eq!(s("2026-11-18", false), "Due 18 Nov 2026");
        let d = day("2026-11-18");
        assert_eq!(
            sentence(FR, d, due_state(Some(d), today(), true)),
            "En retard de 3 jours, \u{e9}ch\u{e9}ance le 18 nov. 2026"
        );
    }

    #[test]
    fn table_cells_mark_overdue_with_icon_and_tone() {
        let c = |iso: &str, open| cell(EN, Some(day(iso)), today(), open).unwrap();
        let overdue = c("2026-11-18", true);
        assert_eq!(
            (overdue.text.as_str(), overdue.tone, overdue.alert),
            ("18 Nov", Tone::Warning, true)
        );
        assert_eq!(overdue.tip, "Overdue by 3 days, due 18 Nov 2026");
        let soon = c("2026-11-21", true);
        assert_eq!(
            (soon.text.as_str(), soon.tone, soon.alert),
            ("Today", Tone::Secondary, false)
        );
        assert_eq!(c("2026-11-24", true).tone, Tone::Secondary);
        assert_eq!(c("2026-11-24", true).text, "Tue");
        assert_eq!(c("2026-11-25", true).tone, Tone::Tertiary);
        let closed = c("2026-11-18", false);
        assert_eq!((closed.tone, closed.alert), (Tone::Tertiary, false));
        assert_eq!(closed.tip, "Due 18 Nov 2026");
        assert_eq!(cell(EN, None, today(), true), None);
    }

    #[test]
    fn relative_forms_change_at_midnight() {
        let d = Some(day("2026-11-22"));
        let before = cell(EN, d, today(), true).unwrap();
        let after = cell(EN, d, today().add_days(1), true).unwrap();
        let later = cell(EN, d, today().add_days(2), true).unwrap();
        assert_eq!((before.text.as_str(), before.alert), ("Tomorrow", false));
        assert_eq!((after.text.as_str(), after.alert), ("Today", false));
        assert_eq!((later.text.as_str(), later.alert), ("22 Nov", true));
    }

    #[test]
    fn the_clock_looks_again_just_after_midnight_and_at_least_every_minute() {
        // UTC, 23:59:30 on 21 Nov 2026: 30 s to midnight.
        let midnight = Zone::UTC.midnight(day("2026-11-22"));
        let wait = next_check(midnight - 30_000, Zone::UTC);
        assert_eq!(wait, Duration::from_millis(30_020));
        assert_eq!(
            next_check(midnight - 3_600_000, Zone::UTC),
            Duration::from_millis(60_020)
        );
        // Paris in winter (UTC+1): local midnight is 23:00 UTC.
        fn paris(_: i64) -> i64 {
            3600
        }
        let zone = Zone::from_fn(paris);
        let local_midnight = zone.midnight(day("2026-11-22"));
        assert_eq!(local_midnight, midnight - 3_600_000);
        assert_eq!(
            next_check(local_midnight - 10_000, zone),
            Duration::from_millis(10_020)
        );
    }

    #[test]
    fn the_preview_parses_on_every_keystroke() {
        let t = today();
        assert_eq!(preview("", t, None), Preview::Empty(None));
        assert_eq!(preview("  ", t, Some(t)), Preview::Empty(Some(t)));
        assert_eq!(preview("fri", t, None), Preview::Day(day("2026-11-27")));
        assert_eq!(preview("ven.", t, None), Preview::Day(day("2026-11-27")));
        assert_eq!(preview("in 3d", t, None), Preview::Day(day("2026-11-24")));
        assert_eq!(preview("15/12", t, None), Preview::Day(day("2026-12-15")));
        assert_eq!(
            preview("2026-12-15", t, None),
            Preview::Day(day("2026-12-15"))
        );
        assert_eq!(
            preview("yesterday", t, None),
            Preview::Day(day("2026-11-20"))
        );
        assert_eq!(preview("31/02", t, None), Preview::Invalid);
        assert_eq!(preview("soon", t, None), Preview::Invalid);
        assert_eq!(preview("15/12/1999", t, None), Preview::Range);
        assert_eq!(preview("2100-01-01", t, None), Preview::Range);
        assert_eq!(preview("15 dec 2100", t, None), Preview::Range);
        assert_eq!(preview("12345", t, None), Preview::Invalid);
    }

    #[test]
    fn enter_sets_the_parsed_day_or_shows_the_error() {
        let mut p = Picker::new(today(), None);
        assert_eq!(p.enter(), Outcome::Close, "empty input closes");
        p.input("soon");
        assert_eq!(p.enter(), Outcome::Stay);
        assert!(p.error);
        p.input("soo");
        assert!(!p.error, "typing again leaves the error state");
        p.input("15/12");
        assert_eq!(p.month, day("2026-12-01"), "the page turns to the date");
        assert_eq!(p.enter(), Outcome::Set(Some(day("2026-12-15"))));
        // The record's own date again changes nothing.
        let mut p = Picker::new(today(), Some(day("2026-12-15")));
        p.input("15/12");
        assert_eq!(p.enter(), Outcome::Close);
        assert_eq!(p.selected(), Some(day("2026-12-15")));
    }

    #[test]
    fn the_grid_starts_on_monday_and_shows_six_weeks() {
        let p = Picker::new(today(), None);
        let grid = p.grid();
        // November 2026 starts on a Sunday: the grid starts Monday 26 Oct.
        assert_eq!(grid[0], day("2026-10-26"));
        assert_eq!(grid[6], day("2026-11-01"));
        assert_eq!(grid[41], day("2026-12-06"));
        assert!(p.outside(grid[0]) && !p.outside(grid[6]) && p.outside(grid[41]));
    }

    #[test]
    fn grid_keys_move_the_cursor_and_turn_the_page() {
        let mut p = Picker::new(today(), Some(day("2026-12-15")));
        p.down();
        assert_eq!((p.area, p.cursor), (Area::Grid, day("2026-12-15")));
        p.grid_key(GridKey::Right);
        assert_eq!(p.cursor, day("2026-12-16"));
        p.grid_key(GridKey::Home);
        assert_eq!(p.cursor, day("2026-12-14"), "Monday of the row");
        p.grid_key(GridKey::End);
        assert_eq!(p.cursor, day("2026-12-20"), "Sunday of the row");
        p.grid_key(GridKey::Down);
        p.grid_key(GridKey::Down);
        assert_eq!(p.cursor, day("2027-01-03"));
        assert_eq!(
            p.month,
            day("2027-01-01"),
            "a move into another month turns the page"
        );
        p.grid_key(GridKey::PageUp);
        assert_eq!(p.cursor, day("2026-12-03"));
        // 31 January, a month on: clamped to the 28th of February.
        p.cursor = day("2027-01-31");
        p.grid_key(GridKey::PageDown);
        assert_eq!(p.cursor, day("2027-02-28"));
        p.grid_key(GridKey::YearUp);
        assert_eq!(p.cursor, day("2026-02-28"));
        p.grid_key(GridKey::YearDown);
        assert_eq!(p.cursor, day("2027-02-28"));
        p.grid_key(GridKey::Today);
        assert_eq!((p.cursor, p.month), (today(), day("2026-11-01")));
        assert_eq!(p.grid_key(GridKey::Enter), Outcome::Set(Some(today())));
        assert_eq!(p.grid_key(GridKey::Clear), Outcome::Set(None));
        // `Up` from the first row goes back to the input.
        p.cursor = day("2026-11-01");
        p.grid_key(GridKey::Up);
        assert_eq!(p.area, Area::Input);
    }

    #[test]
    fn down_from_the_input_lands_on_the_selected_day_else_today() {
        let mut p = Picker::new(today(), None);
        p.down();
        assert_eq!(p.cursor, today());
        let mut p = Picker::new(today(), None);
        p.input("in 2w");
        p.down();
        assert_eq!(p.cursor, day("2026-12-05"));
    }

    #[test]
    fn tab_cycles_and_clear_exists_only_with_a_date() {
        let mut p = Picker::new(today(), None);
        assert!(!p.has_clear());
        assert_eq!(p.clear(), Outcome::Stay);
        p.tab(false);
        assert_eq!(p.area, Area::Grid);
        p.tab(false);
        assert_eq!(p.area, Area::Input, "no Clear stop without a date");
        let mut p = Picker::new(today(), Some(today()));
        p.tab(true);
        assert_eq!(p.area, Area::Clear);
        assert_eq!(p.clear(), Outcome::Set(None));
        assert_eq!(
            p.pick(today()),
            Outcome::Close,
            "the same day changes nothing"
        );
        assert_eq!(
            p.pick(day("2026-11-30")),
            Outcome::Set(Some(day("2026-11-30")))
        );
    }

    #[test]
    fn the_cursor_and_the_page_stay_in_2000_to_2099() {
        let mut p = Picker::new(day("2099-12-30"), None);
        p.down();
        p.grid_key(GridKey::YearDown);
        assert_eq!(p.cursor, day("2099-12-31"));
        p.turn(1);
        assert_eq!(p.month, day("2099-12-01"));
    }

    #[test]
    fn month_headers_in_both_languages() {
        assert_eq!(month_header(EN, day("2026-11-01")), "November 2026");
        assert_eq!(month_header(FR, day("2026-11-01")), "Novembre 2026");
    }
}
