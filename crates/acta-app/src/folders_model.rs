//! Repairing working folders (docs/design/components-folders.md 5,
//! Repairing the rest): after one Locate, the part of the path that changed
//! is taken as the old and the new root, and other folders that were not
//! found and exist under the new root are offered the same change.

use acta_core::folder_path;
use acta_core::paths::Os;

/// The roots of a relocation: `D:\work\acta` located at
/// `C:\Users\Alexandre\dev\acta` gives `D:\work` and `C:\Users\Alexandre\dev`
/// (the longest common trailing segments are dropped). `None` when nothing
/// but the last segments differ in a way that gives no root (one path is a
/// suffix of the other).
pub fn roots(old: &str, new: &str) -> Option<(String, String)> {
    let split =
        |p: &str| -> Vec<String> { p.split(['/', '\\']).map(str::to_owned).collect::<Vec<_>>() };
    let old_parts = split(old);
    let new_parts = split(new);
    let mut common = 0;
    while common < old_parts.len().saturating_sub(1)
        && common < new_parts.len().saturating_sub(1)
        && old_parts[old_parts.len() - 1 - common]
            .eq_ignore_ascii_case(&new_parts[new_parts.len() - 1 - common])
    {
        common += 1;
    }
    let old_root = rejoin(old, old_parts.len() - common);
    let new_root = rejoin(new, new_parts.len() - common);
    if old_root.is_empty() || new_root.is_empty() || old_root == new_root {
        return None;
    }
    Some((old_root, new_root))
}

/// The first `segments` segments of `path`, with its own separators.
fn rejoin(path: &str, segments: usize) -> String {
    let mut seen = 0;
    for (i, c) in path.char_indices() {
        if c == '/' || c == '\\' {
            seen += 1;
            if seen == segments {
                return path[..i].to_owned();
            }
        }
    }
    path.to_owned()
}

/// The folders (as stored) the same change would resolve: not found now,
/// under `old_root`, and existing under `new_root`. `exists` checks the
/// disk, so call this off the UI thread.
pub fn candidates(
    paths: &[String],
    old_root: &str,
    new_root: &str,
    exists: impl Fn(&str) -> bool,
) -> Vec<String> {
    let os = Os::CURRENT;
    let old_root = folder_path::normalize(os, old_root);
    paths
        .iter()
        .filter(|path| !exists(path))
        .filter(|path| folder_path::contains(os, &old_root, path).is_some())
        .filter(|path| {
            let rest = path
                .get(old_root.len()..)
                .unwrap_or("")
                .trim_start_matches(['/', '\\']);
            let sep = if new_root.contains('\\') { "\\" } else { "/" };
            let candidate = if rest.is_empty() {
                new_root.to_owned()
            } else {
                format!("{new_root}{sep}{rest}")
            };
            exists(&candidate)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_changed_part_is_the_root() {
        assert_eq!(
            roots("D:\\work\\acta", "C:\\Users\\Alexandre\\dev\\acta"),
            Some(("D:\\work".into(), "C:\\Users\\Alexandre\\dev".into()))
        );
        assert_eq!(
            roots("/mnt/old/clients/acta", "/home/alexandre/clients/acta"),
            Some(("/mnt/old".into(), "/home/alexandre".into()))
        );
        assert_eq!(roots("D:\\work\\acta", "D:\\work\\acta"), None);
    }

    #[test]
    fn only_missing_folders_that_exist_under_the_new_root_are_offered() {
        let on_disk = ["/home/a/dev/acta-docs", "/home/a/dev/acta"];
        let exists = |p: &str| on_disk.contains(&p);
        let paths = vec![
            "/mnt/old/acta-docs".to_string(),
            "/mnt/old/design".to_string(),
            "/elsewhere/acta".to_string(),
            "/home/a/dev/acta".to_string(),
        ];
        if cfg!(windows) {
            // Path normalisation on Windows upper-cases drive letters and
            // keeps the separators; the rule is checked with Windows paths.
            let on_disk = ["C:\\dev\\acta-docs"];
            let exists = |p: &str| on_disk.contains(&p);
            let paths = vec![
                "D:\\work\\acta-docs".to_string(),
                "D:\\work\\design".to_string(),
            ];
            assert_eq!(
                candidates(&paths, "D:\\work", "C:\\dev", exists),
                ["D:\\work\\acta-docs"]
            );
        } else {
            assert_eq!(
                candidates(&paths, "/mnt/old", "/home/a/dev", exists),
                ["/mnt/old/acta-docs"]
            );
        }
    }
}
