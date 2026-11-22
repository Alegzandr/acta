//! Version numbers as the tools print them, and Acta's minimums.

use std::fmt;

/// `major.minor.patch`, compared numerically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The oldest Claude Code Acta's integration is proven on (ADR 0013, 1);
/// raised when Acta relies on something newer.
pub const CLAUDE_MIN: Version = Version::new(2, 1, 329);
/// Claude Code's own floor for the features that need git.
pub const GIT_MIN: Version = Version::new(2, 31, 0);
/// Below it, GitHub calls the packaged versions broken.
pub const GH_MIN: Version = Version::new(2, 47, 0);

/// The first `major.minor.patch` of the first line: `2.1.330 (Claude
/// Code)`, `git version 2.56.1.windows.1`, `gh version 2.105.0 (...)`.
pub fn parse_version(text: &str) -> Option<Version> {
    let line = text.lines().next()?;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() || (i > 0 && bytes[i - 1].is_ascii_digit()) {
            i += 1;
            continue;
        }
        let end = bytes[i..]
            .iter()
            .position(|b| !(b.is_ascii_digit() || *b == b'.'))
            .map_or(bytes.len(), |n| i + n);
        let mut parts = line[i..end].split('.');
        let mut next = || parts.next().and_then(|p| p.parse::<u32>().ok());
        if let (Some(major), Some(minor), Some(patch)) = (next(), next(), next()) {
            return Some(Version::new(major, minor, patch));
        }
        i = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_each_tools_line() {
        let cases = [
            ("2.1.330 (Claude Code)", Some(Version::new(2, 1, 330))),
            ("git version 2.56.1.windows.1", Some(Version::new(2, 56, 1))),
            (
                "git version 2.39.5 (Apple Git-154)",
                Some(Version::new(2, 39, 5)),
            ),
            (
                "gh version 2.105.0 (2026-11-18)\nhttps://github.com/cli/cli/releases/tag/v2.105.0",
                Some(Version::new(2, 105, 0)),
            ),
            ("Alexandre's garbage 12 and 3.4", None),
            ("", None),
            ("\n2.1.330 (Claude Code)", None),
            ("version 99999999999.1.1", None),
        ];
        for (text, want) in cases {
            assert_eq!(parse_version(text), want, "{text:?}");
        }
    }

    #[test]
    fn minimums_compare_numerically() {
        assert!(Version::new(2, 1, 330) >= CLAUDE_MIN);
        assert!(Version::new(2, 1, 329) >= CLAUDE_MIN);
        assert!(Version::new(2, 1, 100) < CLAUDE_MIN);
        assert!(Version::new(2, 0, 999) < CLAUDE_MIN);
        assert!(Version::new(2, 56, 1) >= GIT_MIN);
        assert!(Version::new(2, 39, 5) >= GIT_MIN);
        assert!(Version::new(2, 30, 9) < GIT_MIN);
        assert!(Version::new(2, 105, 0) >= GH_MIN);
        assert!(Version::new(2, 46, 0) < GH_MIN);
        assert_eq!(CLAUDE_MIN.to_string(), "2.1.329");
    }
}
