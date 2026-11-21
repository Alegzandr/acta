//! Every string Acta itself puts on screen, in English and French
//! (docs/design/copy.md 10). Code refers to a [`Key`]; the text lives in
//! `catalogue.rs`, generated from the catalogue of docs/design/catalogue.md 10.4.
//!
//! Completeness is checked by the compiler: the `catalogue!` macro takes an
//! `en` and an `fr` value for every key, and [`Key::messages`] is an
//! exhaustive match, so a key without both languages does not build.
//!
//! Nothing here depends on GPUI; what the user and Claude Code wrote is
//! passed in as placeholder values and never translated.

// No screen shows a time, a count or a multi-key hint yet. These parts are
// loaded and tested now so that the screens of M2 only reference them; the
// allowances go as each gets its first caller.
#[allow(
    dead_code,
    reason = "the time forms of docs/design/copy.md 10.2, used from M2"
)]
pub mod time;

use std::fmt::Write as _;

/// Interface language. English is the default on a new install; Acta does
/// not follow the OS language on its own (docs/design/copy.md 10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Locale {
    #[default]
    En,
    Fr,
}

impl Locale {
    pub const ALL: [Locale; 2] = [Locale::En, Locale::Fr];

    /// The language's own name, the same in every interface language.
    pub fn native_name(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Fr => "Fran\u{e7}ais",
        }
    }

    /// Which plural form `n` takes. French uses "one" for 0 and 1.
    pub fn is_one(self, n: u64) -> bool {
        match self {
            Locale::En => n == 1,
            Locale::Fr => n <= 1,
        }
    }
}

/// One language's text for a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    Text(&'static str),
    /// Chosen by the count placeholder `by` (usually `n`).
    Plural {
        by: &'static str,
        one: &'static str,
        other: &'static str,
    },
}

const fn t(text: &'static str) -> Msg {
    Msg::Text(text)
}

const fn p(one: &'static str, other: &'static str) -> Msg {
    Msg::Plural {
        by: "n",
        one,
        other,
    }
}

const fn p_by(by: &'static str, one: &'static str, other: &'static str) -> Msg {
    Msg::Plural { by, one, other }
}

/// Declares [`Key`] and its two messages. Both languages are required by
/// the syntax, so a key cannot exist in one language only.
macro_rules! catalogue {
    ($( $variant:ident $id:literal { en: $en:expr, fr: $fr:expr, } )*) => {
        /// A string of the catalogue (docs/design/catalogue.md 10.4), named as there.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[allow(
            dead_code,
            reason = "the whole catalogue of docs/design/catalogue.md 10.4 is loaded now; screens reference keys as they arrive"
        )]
        pub enum Key {
            $( $variant, )*
        }

        impl Key {
            /// Every key, in catalogue order.
            #[cfg(test)]
            pub const ALL: &'static [Key] = &[ $( Key::$variant, )* ];

            /// The key as docs/design/catalogue.md writes it, e.g. `common.cancel`.
            #[cfg(test)]
            pub fn id(self) -> &'static str {
                match self {
                    $( Key::$variant => $id, )*
                }
            }

            /// English and French.
            pub fn messages(self) -> (Msg, Msg) {
                match self {
                    $( Key::$variant => ($en, $fr), )*
                }
            }
        }
    };
}
mod catalogue;

pub use catalogue::Key;

/// A placeholder value.
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    /// Inserted as given: names, titles, paths, preformatted times.
    Text(&'a str),
    /// A count: formatted for the locale, and it chooses the plural form
    /// when the message is chosen by this placeholder.
    Count(u64),
}

impl Key {
    pub fn message(self, locale: Locale) -> Msg {
        let (en, fr) = self.messages();
        match locale {
            Locale::En => en,
            Locale::Fr => fr,
        }
    }
}

/// The text of `key` with its placeholders filled. A placeholder without a
/// value stays as written, so a mistake shows instead of vanishing.
pub fn tr(locale: Locale, key: Key, args: &[(&str, Arg)]) -> String {
    let template = match key.message(locale) {
        Msg::Text(text) => text,
        Msg::Plural { by, one, other } => {
            let n = args.iter().find_map(|(name, arg)| match arg {
                Arg::Count(n) if *name == by => Some(*n),
                _ => None,
            });
            match n {
                Some(n) if locale.is_one(n) => one,
                _ => other,
            }
        }
    };
    fill(locale, template, args)
}

/// The text of a key that takes no placeholder.
pub fn text(locale: Locale, key: Key) -> String {
    tr(locale, key, &[])
}

fn fill(locale: Locale, template: &str, args: &[(&str, Arg)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &after[..close];
        match args.iter().find(|(n, _)| *n == name) {
            Some((_, Arg::Text(value))) => out.push_str(value),
            Some((_, Arg::Count(n))) => out.push_str(&number(locale, *n)),
            None => {
                let _ = write!(out, "{{{name}}}");
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// A whole number with the locale's thousands separator: "1,000" or
/// "1 000" with U+202F (docs/design/copy.md 10.2).
pub fn number(locale: Locale, n: u64) -> String {
    let separator = match locale {
        Locale::En => ",",
        Locale::Fr => "\u{202f}",
    };
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + 8);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
}

/// A percentage: "68%", or "68 %" with U+202F in French (docs/design/copy.md 10.2).
pub fn percent(locale: Locale, n: u64) -> String {
    match locale {
        Locale::En => format!("{n}%"),
        Locale::Fr => format!("{n}\u{202f}%"),
    }
}

#[allow(dead_code, reason = "no screen shows a tree count yet")]
/// A count where space is short (tree, tabs): "99+" above 99 (docs/design/copy.md 7).
pub fn short_count(n: u64) -> String {
    if n > 99 { "99+".into() } else { n.to_string() }
}

/// A key of a shortcut hint, named for the platform and the language
/// (docs/design/components.md 5 kbd, 10.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "only Mod is shown so far, in the Search hint")]
pub enum KeyName {
    Mod,
    Ctrl,
    Shift,
    Alt,
    Enter,
    Esc,
    Delete,
    Tab,
}

impl KeyName {
    pub fn label(self, locale: Locale, macos: bool) -> &'static str {
        if macos {
            return match self {
                KeyName::Mod => "\u{2318}",
                KeyName::Ctrl => "\u{2303}",
                KeyName::Shift => "\u{21e7}",
                KeyName::Alt => "\u{2325}",
                KeyName::Enter => "\u{21b5}",
                KeyName::Esc => "Esc",
                KeyName::Delete => "\u{232b}",
                KeyName::Tab => "Tab",
            };
        }
        match (self, locale) {
            (KeyName::Mod | KeyName::Ctrl, _) => "Ctrl",
            (KeyName::Shift, Locale::En) => "Shift",
            (KeyName::Shift, Locale::Fr) => "Maj",
            (KeyName::Alt, _) => "Alt",
            (KeyName::Enter, Locale::En) => "Enter",
            (KeyName::Enter, Locale::Fr) => "Entr\u{e9}e",
            (KeyName::Esc, Locale::En) => "Esc",
            (KeyName::Esc, Locale::Fr) => "\u{c9}chap",
            (KeyName::Delete, Locale::En) => "Delete",
            (KeyName::Delete, Locale::Fr) => "Suppr",
            (KeyName::Tab, _) => "Tab",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name))
            .collect()
    }

    fn all_texts(msg: Msg) -> Vec<&'static str> {
        match msg {
            Msg::Text(text) => vec![text],
            Msg::Plural { one, other, .. } => vec![one, other],
        }
    }

    /// A cell of docs/design/catalogue.md 10.4 as the catalogue must hold it: notes for
    /// the reader removed, the typography of 10.1 applied, "one · other"
    /// split into a plural.
    fn expected(key: &str, cell: &str, french: bool) -> Vec<String> {
        // Parenthesised notes: platform marks, references to other keys,
        // and the few notes in words.
        const WORD_NOTES: [&str; 5] = [
            "full weekday name",
            "for the Kind filter",
            "no-break space before the dash",
            "the two counts are",
            "form chosen by",
        ];
        let mut text = String::new();
        let mut rest = cell;
        while let Some(open) = rest.find(" (") {
            let after = &rest[open + 2..];
            let close = after.find(')').map(|i| i + 1);
            let note = close.map(|close| &after[..close - 1]);
            let is_note = note.is_some_and(|note| {
                matches!(note, "W" | "M" | "L")
                    || note.starts_with('`')
                    || WORD_NOTES.iter().any(|word| note.starts_with(word))
            });
            match (is_note, close) {
                (true, Some(close)) => {
                    text.push_str(&rest[..open]);
                    rest = &after[close..];
                }
                _ => {
                    text.push_str(&rest[..open + 2]);
                    rest = after;
                }
            }
        }
        text.push_str(rest);

        let typeset = |part: &str| {
            let mut part = part.replace("...", "\u{2026}").replace('\'', "\u{2019}");
            if french {
                for mark in [";", ":", "!", "?", "\u{bb}"] {
                    part = part.replace(&format!(" {mark}"), &format!("\u{202f}{mark}"));
                }
                part = part
                    .replace("\u{ab} ", "\u{ab}\u{202f}")
                    .replace("400 Mo", "400\u{202f}Mo")
                    .replace("150 Mo", "150\u{202f}Mo")
                    .replace(" \u{2013} ", "\u{a0}\u{2013} ");
            }
            part
        };
        let parts: Vec<&str> = text.split(" \u{b7} ").collect();
        if parts.len() == 2 && !matches!(key, "kb.scope" | "import.meta") {
            parts.into_iter().map(typeset).collect()
        } else {
            vec![typeset(&text)]
        }
    }

    #[test]
    fn every_text_matches_design_md() {
        const DESIGN_MD: &str = include_str!("../../../../docs/design/catalogue.md");
        let section = DESIGN_MD
            .split("### 10.4 Copy catalogue")
            .nth(1)
            .expect("docs/design/catalogue.md has 10.4");
        let rows = section
            .lines()
            .take_while(|line| !line.starts_with("Strings that exist"))
            .filter(|line| line.starts_with("| `"));
        let mut checked = 0;
        for line in rows {
            let cells: Vec<&str> = line.trim().trim_matches('|').split(" | ").collect();
            let id = cells[0].trim().trim_matches('`');
            let key = Key::ALL
                .iter()
                .find(|key| key.id() == id)
                .unwrap_or_else(|| panic!("{id} is not in the catalogue"));
            let (en, fr) = key.messages();
            assert_eq!(
                all_texts(en),
                expected(id, cells[1].trim(), false),
                "{id} en"
            );
            assert_eq!(
                all_texts(fr),
                expected(id, cells[2].trim(), true),
                "{id} fr"
            );
            checked += 1;
        }
        assert_eq!(checked, Key::ALL.len());
    }

    #[test]
    fn every_key_of_design_md_is_in_the_table_and_nothing_else() {
        const DESIGN_MD: &str = include_str!("../../../../docs/design/catalogue.md");
        let section = DESIGN_MD
            .split("### 10.4 Copy catalogue")
            .nth(1)
            .expect("docs/design/catalogue.md has 10.4");
        let documented: Vec<&str> = section
            .lines()
            .take_while(|line| !line.starts_with("Strings that exist"))
            .filter_map(|line| line.strip_prefix("| `"))
            .filter_map(|line| line.split('`').next())
            .collect();
        let ours: Vec<&str> = Key::ALL.iter().map(|key| key.id()).collect();
        assert_eq!(ours, documented);
        assert_eq!(ours.len(), ours.iter().collect::<HashSet<_>>().len());
    }

    #[test]
    fn both_languages_take_the_same_placeholders() {
        for key in Key::ALL {
            let (en, fr) = key.messages();
            let names = |msg: Msg| -> BTreeSet<&str> {
                all_texts(msg).into_iter().flat_map(placeholders).collect()
            };
            assert_eq!(names(en), names(fr), "{}", key.id());
            for msg in [en, fr] {
                if let Msg::Plural { by, one, other } = msg {
                    assert!(
                        placeholders(one).contains(by) || placeholders(other).contains(by),
                        "{} is chosen by {{{by}}}, which it does not use",
                        key.id()
                    );
                }
            }
        }
    }

    #[test]
    fn typography_rules_of_10_1_hold() {
        for key in Key::ALL {
            let (en, fr) = key.messages();
            for text in all_texts(en).into_iter().chain(all_texts(fr)) {
                assert!(!text.contains("..."), "{}: three dots", key.id());
                assert!(!text.contains('\''), "{}: straight apostrophe", key.id());
                assert!(!text.contains(" \u{2026}"), "{}: space before …", key.id());
            }
            for text in all_texts(fr) {
                for mark in [" ;", " :", " !", " ?", " \u{bb}", "\u{ab} "] {
                    assert!(!text.contains(mark), "{}: {mark:?} needs U+202F", key.id());
                }
            }
        }
        assert_eq!(
            text(Locale::Fr, Key::SettingsLanguage),
            "Langue",
            "plain words stay plain"
        );
        assert_eq!(
            tr(Locale::Fr, Key::CmdTheme, &[("value", Arg::Text("Sombre"))]),
            "Th\u{e8}me\u{202f}: Sombre"
        );
        assert_eq!(text(Locale::En, Key::CommonOpening), "Opening\u{2026}");
    }

    #[test]
    fn plural_forms_follow_each_language() {
        let topics = |locale, n| tr(locale, Key::CountTopics, &[("n", Arg::Count(n))]);
        assert_eq!(topics(Locale::En, 0), "0 topics");
        assert_eq!(topics(Locale::En, 1), "1 topic");
        assert_eq!(topics(Locale::En, 2), "2 topics");
        assert_eq!(topics(Locale::Fr, 0), "0 sujet");
        assert_eq!(topics(Locale::Fr, 1), "1 sujet");
        assert_eq!(topics(Locale::Fr, 2), "2 sujets");
        assert_eq!(topics(Locale::En, 1200), "1,200 topics");
        assert_eq!(topics(Locale::Fr, 1200), "1\u{202f}200 sujets");
    }

    #[test]
    fn a_plural_can_be_chosen_by_another_placeholder() {
        let progress = |locale, done, total| {
            tr(
                locale,
                Key::PropProgressValue,
                &[("done", Arg::Count(done)), ("total", Arg::Count(total))],
            )
        };
        // English chooses by {total}, French by {done}.
        assert_eq!(progress(Locale::En, 3, 1), "3 of 1 topic done");
        assert_eq!(progress(Locale::En, 1, 7), "1 of 7 topics done");
        assert_eq!(progress(Locale::Fr, 1, 7), "1 sujet termin\u{e9} sur 7");
        assert_eq!(progress(Locale::Fr, 3, 7), "3 sujets termin\u{e9}s sur 7");
    }

    #[test]
    fn placeholders_are_filled_and_missing_ones_show() {
        let by = tr(
            Locale::Fr,
            Key::StateUpdated,
            &[
                ("when", Arg::Text("hier \u{e0} 18:42")),
                ("author", Arg::Text("Alexandre")),
            ],
        );
        assert_eq!(by, "Modifi\u{e9} hier \u{e0} 18:42 par Alexandre");
        assert_eq!(tr(Locale::En, Key::StatusbarVersion, &[]), "Acta {version}");
    }

    #[test]
    fn numbers_and_short_counts() {
        assert_eq!(number(Locale::En, 0), "0");
        assert_eq!(number(Locale::En, 999), "999");
        assert_eq!(number(Locale::En, 1000), "1,000");
        assert_eq!(number(Locale::En, 1234567), "1,234,567");
        assert_eq!(number(Locale::Fr, 1000), "1\u{202f}000");
        assert_eq!(percent(Locale::En, 68), "68%");
        assert_eq!(percent(Locale::Fr, 68), "68\u{202f}%");
        assert_eq!(short_count(99), "99");
        assert_eq!(short_count(100), "99+");
    }

    #[test]
    fn key_names_follow_platform_and_language() {
        assert_eq!(KeyName::Mod.label(Locale::En, false), "Ctrl");
        assert_eq!(KeyName::Shift.label(Locale::Fr, false), "Maj");
        assert_eq!(KeyName::Enter.label(Locale::Fr, false), "Entr\u{e9}e");
        assert_eq!(KeyName::Esc.label(Locale::Fr, false), "\u{c9}chap");
        assert_eq!(KeyName::Delete.label(Locale::Fr, false), "Suppr");
        assert_eq!(KeyName::Mod.label(Locale::Fr, true), "\u{2318}");
    }
}
