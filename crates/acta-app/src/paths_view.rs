//! How a working folder's path is shown (docs/design/components-folders.md
//! 5, Path display): as stored, the home folder shortened to `~`, and when
//! it does not fit, cut in the middle by whole segments with a single `…`,
//! keeping the root and as many trailing segments as fit.

/// The width of one character of `type.code` (JetBrains Mono at 13 has an
/// advance of 0.6 em), to turn a pixel width into a character budget.
pub const CODE_CHAR_WIDTH: f32 = 7.8;
/// The path slot is never narrower than 160.
pub const MIN_PATH_WIDTH: f32 = 160.;

/// `path` with the home folder replaced by `~`, when it is under it.
/// Separators stay as stored; the comparison ignores case on Windows and
/// macOS, as path identity does (ADR 0008).
pub fn shorten_home(path: &str, home: Option<&str>) -> String {
    let Some(home) = home.map(|h| h.trim_end_matches(['/', '\\'])) else {
        return path.to_owned();
    };
    if home.is_empty() || path.len() < home.len() {
        return path.to_owned();
    }
    let (head, rest) = path.split_at(home.len());
    let same = if cfg!(any(windows, target_os = "macos")) {
        head.eq_ignore_ascii_case(home)
    } else {
        head == home
    };
    if !same {
        return path.to_owned();
    }
    if rest.is_empty() {
        return "~".into();
    }
    if rest.starts_with(['/', '\\']) {
        return format!("~{rest}");
    }
    path.to_owned()
}

/// The home folder of this user, as the OS reports it.
pub fn home() -> Option<String> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var(var).ok().filter(|h| !h.is_empty())
}

/// The last segment of a path ("embed-terminal"), for toasts, which never
/// name a folder by its full path.
pub fn last_segment(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}

/// Split into the root (`~`, `C:`, `/`, `\\server\share`) and the
/// segments after it, keeping the separator the path uses.
fn split_root(path: &str) -> (String, Vec<&str>, char) {
    let sep = if path.contains('\\') { '\\' } else { '/' };
    if let Some(rest) = path.strip_prefix("\\\\") {
        // UNC: \\server\share is the root.
        let mut parts = rest.split('\\');
        let server = parts.next().unwrap_or("");
        let share = parts.next().unwrap_or("");
        let root = format!("\\\\{server}\\{share}");
        return (root, parts.filter(|s| !s.is_empty()).collect(), '\\');
    }
    if let Some(rest) = path.strip_prefix('/') {
        return (
            "".into(),
            rest.split('/').filter(|s| !s.is_empty()).collect(),
            '/',
        );
    }
    let mut parts = path.split(['/', '\\']);
    let root = parts.next().unwrap_or("").to_owned();
    (root, parts.filter(|s| !s.is_empty()).collect(), sep)
}

/// Fit `path` into `max` characters, cutting in the middle by whole
/// segments: `~\…\acta-wt\embed-terminal`, then `…\embed-terminal`, then the
/// last segment cut at its end.
pub fn fit(path: &str, max: usize) -> String {
    if path.chars().count() <= max {
        return path.to_owned();
    }
    let (root, segments, sep) = split_root(path);
    let Some((last, before)) = segments.split_last() else {
        return cut_end(path, max);
    };
    // The root followed by its separator ("/" alone for a Unix root).
    let root_part = if root.is_empty() {
        sep.to_string()
    } else {
        format!("{root}{sep}")
    };
    let mut kept: Vec<&str> = vec![last];
    let build = |kept: &[&str], with_root: bool| {
        let tail: Vec<&str> = kept.iter().rev().copied().collect();
        let tail = tail.join(&sep.to_string());
        if with_root {
            format!("{root_part}\u{2026}{sep}{tail}")
        } else {
            format!("\u{2026}{sep}{tail}")
        }
    };
    if build(&kept, true).chars().count() <= max {
        for segment in before.iter().rev() {
            kept.push(segment);
            if build(&kept, true).chars().count() > max {
                kept.pop();
                break;
            }
        }
        return build(&kept, true);
    }
    let without_root = build(&kept, false);
    if without_root.chars().count() <= max {
        return without_root;
    }
    cut_end(last, max)
}

fn cut_end(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// The characters a path slot of `width` pixels holds.
pub fn budget(width: f32) -> usize {
    (width.max(MIN_PATH_WIDTH) / CODE_CHAR_WIDTH).floor() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_folder_reads_as_a_tilde() {
        let home = Some("C:\\Users\\Alexandre");
        assert_eq!(
            shorten_home("C:\\Users\\Alexandre\\dev\\acta", home),
            "~\\dev\\acta"
        );
        assert_eq!(shorten_home("C:\\Users\\Alexandre", home), "~");
        assert_eq!(
            shorten_home("C:\\Users\\Alexandrette\\x", home),
            "C:\\Users\\Alexandrette\\x"
        );
        assert_eq!(
            shorten_home("/home/alexandre/dev/acta", Some("/home/alexandre/")),
            "~/dev/acta"
        );
        assert_eq!(shorten_home("D:\\work", None), "D:\\work");
    }

    #[test]
    fn long_paths_lose_middle_segments_first() {
        let path = "~\\dev\\clients\\alexandre\\acta-wt\\embed-terminal";
        assert_eq!(fit(path, 100), path);
        assert_eq!(fit(path, 28), "~\\\u{2026}\\acta-wt\\embed-terminal");
        assert_eq!(fit(path, 20), "~\\\u{2026}\\embed-terminal");
        assert_eq!(fit(path, 16), "\u{2026}\\embed-terminal");
        assert_eq!(fit(path, 8), "embed-t\u{2026}");
    }

    #[test]
    fn unix_and_unc_roots_are_kept() {
        assert_eq!(fit("/usr/local/share/acta/data", 17), "/\u{2026}/acta/data");
        assert_eq!(
            fit("\\\\server\\share\\projects\\acta\\docs", 24),
            "\\\\server\\share\\\u{2026}\\docs"
        );
    }

    #[test]
    fn toasts_name_the_last_segment() {
        assert_eq!(last_segment("C:\\dev\\acta-docs"), "acta-docs");
        assert_eq!(last_segment("/home/a/acta/"), "acta");
    }

    #[test]
    fn the_slot_is_never_narrower_than_160() {
        assert_eq!(budget(40.), budget(160.));
        assert!(budget(468.) > budget(160.));
    }
}
