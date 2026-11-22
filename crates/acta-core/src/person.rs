//! People (docs/design/data.md request 19; components-questions.md 5). A
//! person is free text, not a record: a question's Asked and the Report to
//! of a note, a topic or an epic. Two values name the same person when
//! their [`person_key`] is equal. One pure function shared by the UI, the
//! store and the agent, so suggestions, filters and `find(person=...)`
//! always agree.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Longest person value, in characters.
pub const MAX_CHARS: usize = 80;

/// The value as stored: trimmed, inner whitespace collapsed to one space,
/// otherwise as typed. `Ok(None)` for an empty value (clears); an error
/// over [`MAX_CHARS`].
pub fn clean(text: &str) -> Result<Option<String>, String> {
    let v = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if v.is_empty() {
        return Ok(None);
    }
    let n = v.chars().count();
    if n > MAX_CHARS {
        return Err(format!(
            "a person is {n} chars; the limit is {MAX_CHARS}. Shorten it."
        ));
    }
    Ok(Some(v))
}

/// What makes two values the same person: case folded, accents removed
/// (NFKD, combining marks dropped), whitespace collapsed. Never shown.
pub fn person_key(text: &str) -> String {
    let folded: String = text
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_case_accents_and_spacing() {
        assert_eq!(person_key("Marie Dubois"), "marie dubois");
        assert_eq!(person_key("  marie   DUBOIS "), "marie dubois");
        assert_eq!(person_key("Marie\tDubois"), person_key("marie dubois"));
        assert_eq!(person_key("Hélène Éluard"), "helene eluard");
        assert_eq!(person_key("le service RH"), person_key("Le Service rh"));
        assert_eq!(person_key("Ångström"), "angstrom");
        // Compatibility forms fold too (a full-width letter, a ligature).
        assert_eq!(person_key("ｍarie"), "marie");
        assert_eq!(person_key("ﬁona"), "fiona");
        assert_ne!(person_key("Marie"), person_key("Mario"));
        assert_eq!(person_key("Alexandre"), "alexandre");
    }

    #[test]
    fn values_are_trimmed_collapsed_and_limited() {
        assert_eq!(clean("  Legal   team "), Ok(Some("Legal team".into())));
        assert_eq!(clean("Hélène"), Ok(Some("Hélène".into())));
        assert_eq!(clean("   "), Ok(None));
        assert!(clean(&"a".repeat(80)).unwrap().is_some());
        assert!(clean(&"a".repeat(81)).is_err());
        // Characters, not bytes.
        assert!(clean(&"é".repeat(80)).unwrap().is_some());
    }
}
