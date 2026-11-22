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

/// Whether a path from somewhere else (an import) may be touched on this
/// machine: an absolute local path, a Windows drive path (`C:\...`) or a
/// Unix one (`/...`). Anything that can name another computer (`\\server`,
/// `//server`, `\\?\UNC\`, a `\\.\` device, `/net/<host>`, `/Network/`)
/// is refused whatever this
/// machine's OS: on Windows merely asking whether it exists sends the
/// user's credentials (NTLM) to that computer. Pure text, so a file from
/// any OS is judged the same way.
pub fn is_probe_safe(path: &str) -> bool {
    let b = path.as_bytes();
    let sep = |c: u8| c == b'/' || c == b'\\';
    if path.contains('\0') {
        return false;
    }
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && sep(b[2]) {
        return true;
    }
    // `/home/...`, but not `//server` or `/\server`, which Windows reads as
    // a network path, nor `/net/<host>` (autofs) or `/Network/...` (macOS),
    // which reach other computers when looked at.
    if b.first() != Some(&b'/') || b.get(1).is_some_and(|&c| sep(c)) {
        return false;
    }
    let first = path[1..].split(['/', '\\']).next().unwrap_or("");
    !first.eq_ignore_ascii_case("net") && !first.eq_ignore_ascii_case("network")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_absolute_paths_are_probe_safe() {
        for ok in [
            r"C:\Dev\Acta",
            "c:/dev",
            r"Z:\",
            "/home/alexandre/acta",
            "/",
            "/home/alexandre/net",
            "/netlify/site",
            "/Networks",
        ] {
            assert!(is_probe_safe(ok), "{ok}");
        }
        for bad in [
            r"\\attacker@SSL\x",
            r"\\10.0.0.5\share",
            r"\\?\UNC\server\share",
            r"\\?\C:\Dev",
            r"\\.\pipe\x",
            r"\??\UNC\server\share",
            "//server/share",
            r"/\server\share",
            "/net/fileserver/home",
            "/net",
            "/Network/Servers/host/x",
            "/network/servers",
            r"\Dev",
            "C:Dev",
            "relative/path",
            "",
            "/home/a\0b",
        ] {
            assert!(!is_probe_safe(bad), "{bad}");
        }
    }

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
