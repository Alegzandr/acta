//! Path identity of working folders (ADR 0008, "Path identity"). The UI, the
//! CLI and hook resolution share this one implementation.
//!
//! A folder is stored as an absolute canonical path: symlinks and junctions
//! resolved, no trailing separator, no `\\?\` prefix, drive letter upper-case.
//! Two paths are the same folder when their stored forms are equal, compared
//! case-insensitively on Windows and macOS.

use crate::paths::Os;
use std::io;
use std::path::Path;

/// The stored form of an existing folder on this machine.
pub fn canonical(path: &Path) -> io::Result<String> {
    let resolved = std::fs::canonicalize(path)?;
    if !resolved.is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a folder"));
    }
    let text = resolved
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "path is not valid Unicode"))?;
    Ok(normalize(Os::CURRENT, text))
}

/// The textual part of canonicalisation, for a path that is already
/// resolved: prefix, drive letter and trailing separator. Pure, so every
/// OS's rules are tested on any host.
pub fn normalize(os: Os, path: &str) -> String {
    let mut p = path.to_owned();
    if os == Os::Windows {
        if let Some(rest) = p.strip_prefix(r"\\?\UNC\") {
            p = format!(r"\\{rest}");
        } else if let Some(rest) = p.strip_prefix(r"\\?\") {
            p = rest.to_owned();
        }
        let bytes = p.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            p.replace_range(0..1, &(bytes[0] as char).to_ascii_uppercase().to_string());
        }
    }
    while p.len() > root_len(os, &p) && p.ends_with(|c| is_sep(os, c)) {
        p.pop();
    }
    p
}

/// Length of the part that must keep its trailing separator: `/` or `C:\`.
fn root_len(os: Os, p: &str) -> usize {
    if os == Os::Windows {
        let b = p.as_bytes();
        if b.len() >= 3 && b[1] == b':' && is_sep(os, b[2] as char) {
            return 3;
        }
    }
    usize::from(p.starts_with(|c| is_sep(os, c)))
}

fn is_sep(os: Os, c: char) -> bool {
    c == '/' || (os == Os::Windows && c == '\\')
}

/// The comparison key of a stored path.
pub fn key(os: Os, stored: &str) -> String {
    let p = if os == Os::Windows {
        stored.replace('/', "\\")
    } else {
        stored.to_owned()
    };
    match os {
        Os::Windows | Os::MacOs => p.to_lowercase(),
        Os::Linux => p,
    }
}

/// Whether two stored paths are the same folder.
pub fn same(os: Os, a: &str, b: &str) -> bool {
    key(os, a) == key(os, b)
}

/// Whether `dir` is `folder` or lies inside it. Both are stored forms.
/// Returns the depth of `folder` (its length in components) when it matches,
/// so the deepest attached folder can win.
pub fn contains(os: Os, folder: &str, dir: &str) -> Option<usize> {
    let f = key(os, folder);
    let d = key(os, dir);
    let sep = if os == Os::Windows { '\\' } else { '/' };
    let inside =
        d == f || (d.starts_with(&f) && (f.ends_with(sep) || d[f.len()..].starts_with(sep)));
    inside.then(|| f.split(sep).filter(|s| !s.is_empty()).count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_forms() {
        let w = Os::Windows;
        assert_eq!(normalize(w, r"\\?\c:\Dev\Acta\"), r"C:\Dev\Acta");
        assert_eq!(normalize(w, r"\\?\UNC\server\share\x"), r"\\server\share\x");
        assert_eq!(normalize(w, r"\\?\C:\"), r"C:\");
        assert!(same(w, r"C:\Dev\Acta", r"c:\dev\acta"));
        assert_eq!(contains(w, r"C:\Dev", r"c:\dev\acta\src"), Some(2));
        assert_eq!(contains(w, r"C:\Dev", r"C:\Devices"), None);
        assert_eq!(contains(w, r"C:\", r"C:\Dev"), Some(1));
    }

    #[test]
    fn unix_forms() {
        assert_eq!(
            normalize(Os::Linux, "/home/alexandre/acta/"),
            "/home/alexandre/acta"
        );
        assert_eq!(normalize(Os::Linux, "/"), "/");
        assert!(!same(Os::Linux, "/home/Alexandre", "/home/alexandre"));
        assert!(same(Os::MacOs, "/Users/Alexandre", "/users/alexandre"));
        assert_eq!(
            contains(Os::Linux, "/home/alexandre", "/home/alexandre/acta"),
            Some(2)
        );
        assert_eq!(contains(Os::Linux, "/home/alex", "/home/alexandre"), None);
        assert_eq!(contains(Os::Linux, "/", "/home"), Some(0));
        // Backslash is an ordinary character outside Windows.
        assert_eq!(normalize(Os::Linux, r"/tmp/a\"), r"/tmp/a\");
    }

    #[test]
    fn canonical_resolves_an_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("Acta");
        std::fs::create_dir(&sub).unwrap();
        let stored = canonical(&sub.join("..").join("Acta")).unwrap();
        assert!(!stored.starts_with(r"\\?\"));
        assert!(!stored.ends_with(['/', '\\']));
        assert!(stored.ends_with("Acta"));
        assert_eq!(canonical(Path::new(&stored)).unwrap(), stored);
        assert!(canonical(&dir.path().join("missing")).is_err());
    }
}
