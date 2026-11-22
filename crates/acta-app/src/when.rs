//! Stored instants (milliseconds since the Unix epoch, UTC) in the forms of
//! docs/design/copy.md 10.2, in the local time of this computer.

use acta_core::model::Author;
use chrono::{Local, Offset, TimeZone};

use crate::i18n::Locale;
use crate::i18n::time::{self, LocalTime};

/// The product's name, never translated (docs/design/copy.md 7).
pub const CLAUDE_CODE: &str = "Claude Code";

/// `ms` as local wall-clock time, with the offset in force at that instant.
pub fn local(ms: i64) -> LocalTime {
    let seconds = ms.div_euclid(1000);
    let offset = Local
        .timestamp_opt(seconds, 0)
        .single()
        .map(|at| at.offset().fix().local_minus_utc())
        .unwrap_or(0);
    LocalTime::from_unix(seconds, offset)
}

fn now() -> LocalTime {
    local(acta_core::time::now_ms())
}

/// The long form inside a sentence ("2 h ago", "yesterday 18:42").
pub fn long(locale: Locale, ms: i64) -> String {
    time::long(locale, local(ms), now())
}

/// The long form standing alone, capitalised.
pub fn long_alone(locale: Locale, ms: i64) -> String {
    time::capitalize(&long(locale, ms))
}

/// The compact form of the fixed time columns ("2 h ago", "Tue", "12 Nov").
pub fn compact(locale: Locale, ms: i64) -> String {
    time::compact(locale, local(ms), now())
}

/// `{datetime}`: "18 Nov 2026, 09:12".
pub fn datetime(locale: Locale, ms: i64) -> String {
    time::datetime(locale, local(ms))
}

/// `{date}`: "18 Nov 2026".
pub fn date(locale: Locale, ms: i64) -> String {
    time::date(locale, local(ms))
}

/// Whole local days between `ms` and now (0 for today).
pub fn days_between(ms: i64) -> u64 {
    (now().day_number() - local(ms).day_number()).max(0) as u64
}

/// The name shown for an author. Claude Code is named as such in every
/// language; the human is the user of this computer (the model stores no
/// name, docs/design/data.md 11).
pub fn author(locale: Locale, author: Author) -> String {
    match author {
        Author::Claude => CLAUDE_CODE.into(),
        Author::Human => human_name(locale),
    }
}

/// The name of the sample data's human (CLAUDE.md).
pub const SAMPLE_HUMAN: &str = "Alexandre";

/// The user's name as the OS gives it, with a capital initial. A debug
/// build run on the dev seed (`ACTA_DEV_SEED`) shows the sample data's
/// human instead, so captures never carry the OS user's name.
pub fn human_name(locale: Locale) -> String {
    if cfg!(debug_assertions) && std::env::var_os("ACTA_DEV_SEED").is_some() {
        return SAMPLE_HUMAN.into();
    }
    let name = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    let name = name.trim();
    if name.is_empty() {
        return crate::i18n::text(locale, crate::i18n::Key::AuthorYou);
    }
    time::capitalize(name)
}

/// The initial in the human author mark.
pub fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_time_is_the_same_instant() {
        let ms = 1_763_650_800_000; // 2025-11-20T15:00:00Z
        assert_eq!(local(ms).unix, ms / 1000);
    }

    #[test]
    fn initials_are_one_upper_case_letter() {
        assert_eq!(initial("alexandre"), "A");
        assert_eq!(initial("élodie"), "É");
        assert_eq!(initial(""), "");
    }

    #[test]
    fn claude_code_is_named_as_such() {
        assert_eq!(author(Locale::En, Author::Claude), "Claude Code");
    }
}
