//! Finding URLs in terminal text. The session builds a logical line (wrapped
//! rows joined) as one `char` per cell and maps the ranges back to cells.

use std::ops::Range;

/// Only web links are found or opened: a link is handed to the system's
/// URL handler, which would also run `file:`, `ms-msdt:` and friends.
const SCHEMES: [&str; 2] = ["https://", "http://"];

/// Whether a link may be handed to the system's URL handler: `http` or
/// `https` (any case) with a host. Every link, OSC 8 or found in the text,
/// goes through this gate before it is shown or opened.
pub fn is_openable(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    let Some(rest) = SCHEMES.iter().find_map(|s| lower.strip_prefix(s)) else {
        return false;
    };
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    !host.is_empty()
        && !host.starts_with('@')
        && !uri.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Ranges (indices into `line`) of every URL in the line.
pub fn find_urls(line: &[char]) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let Some(scheme) = SCHEMES.iter().find(|s| starts_with(&line[i..], s)) else {
            i += 1;
            continue;
        };
        // A scheme glued to a word ("xhttps://") is not a URL start.
        if i > 0 && line[i - 1].is_alphanumeric() {
            i += 1;
            continue;
        }
        let mut end = i + scheme.len();
        while end < line.len() && is_url_char(line[end]) {
            end += 1;
        }
        end = trim_end(&line[i..end]) + i;
        if end > i + scheme.len() {
            found.push(i..end);
        }
        i = end.max(i + 1);
    }
    found
}

/// The URL covering `index`, if any.
pub fn url_at(line: &[char], index: usize) -> Option<(Range<usize>, String)> {
    find_urls(line)
        .into_iter()
        .find(|r| r.contains(&index))
        .map(|r| (r.clone(), line[r].iter().collect()))
}

fn starts_with(line: &[char], prefix: &str) -> bool {
    let mut chars = line.iter();
    prefix.chars().all(|p| chars.next() == Some(&p))
}

fn is_url_char(c: char) -> bool {
    !c.is_whitespace()
        && !c.is_control()
        && !matches!(c, '<' | '>' | '"' | '`' | '{' | '}' | '|' | '\\' | '^')
        // Box drawing and blocks: Claude Code frames text with them.
        && !('\u{2500}'..='\u{259F}').contains(&c)
}

/// Drop trailing punctuation that belongs to the sentence, and a closing
/// bracket that has no opening one inside the URL.
fn trim_end(url: &[char]) -> usize {
    let mut end = url.len();
    loop {
        let Some(&last) = url[..end].last() else {
            return end;
        };
        let unbalanced = |open: char, close: char| {
            let opens = url[..end].iter().filter(|&&c| c == open).count();
            let closes = url[..end].iter().filter(|&&c| c == close).count();
            closes > opens
        };
        let drop = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '*' => true,
            ')' => unbalanced('(', ')'),
            ']' => unbalanced('[', ']'),
            _ => false,
        };
        if !drop {
            return end;
        }
        end -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        find_urls(&chars)
            .into_iter()
            .map(|r| chars[r].iter().collect())
            .collect()
    }

    #[test]
    fn finds_plain_urls() {
        assert_eq!(
            urls("see https://example.com/a?b=1#c and http://x.io"),
            ["https://example.com/a?b=1#c", "http://x.io"]
        );
        assert!(urls("file:///home/alexandre/notes.md").is_empty());
    }

    #[test]
    fn trims_sentence_punctuation_and_unbalanced_brackets() {
        assert_eq!(urls("Go to https://example.com."), ["https://example.com"]);
        assert_eq!(urls("(https://example.com/x)"), ["https://example.com/x"]);
        assert_eq!(
            urls("https://en.wikipedia.org/wiki/Rust_(language)"),
            ["https://en.wikipedia.org/wiki/Rust_(language)"]
        );
        assert_eq!(urls("<https://example.com>"), ["https://example.com"]);
        assert_eq!(urls("│ https://example.com │"), ["https://example.com"]);
    }

    #[test]
    fn rejects_bare_schemes_and_glued_words() {
        assert!(urls("https:// nothing").is_empty());
        assert!(urls("xhttps://example.com").is_empty());
        assert!(urls("no links here").is_empty());
    }

    #[test]
    fn only_web_links_are_openable() {
        assert!(is_openable("https://example.com/a"));
        assert!(is_openable("HTTP://Example.com"));
        for uri in [
            "file:///C:/Users/alexandre/x.exe",
            "file://server/share/x",
            "javascript:alert(1)",
            "ms-msdt:/id PCWDiagnostic",
            r"C:\Users\alexandre\run.bat",
            "/home/alexandre/run.sh",
            r"\\server\share\x.exe",
            "https://",
            "https:///path",
            "https://exa mple.com",
        ] {
            assert!(!is_openable(uri), "{uri}");
        }
    }

    #[test]
    fn url_at_maps_an_index() {
        let chars: Vec<char> = "a https://b.c d".chars().collect();
        assert_eq!(url_at(&chars, 5), Some((2..13, "https://b.c".to_owned())));
        assert_eq!(url_at(&chars, 0), None);
        assert_eq!(url_at(&chars, 14), None);
    }
}
