//! The plaintext pre-check file of hooks (ADR 0005, 0008): salted hashes of
//! every attached working folder, never the paths.
//!
//! `acta hook` runs on every session start of every repository. Before it
//! touches the keychain or the database it hashes the launch directory and
//! its ancestors and looks them up here; a miss exits silently in a few
//! milliseconds. The store rewrites the file whenever folders change.
//!
//! Format, one item per line: the header, the salt (16 random bytes, hex),
//! then one hash per folder (the first 16 bytes of SHA-256 over the salt and
//! the folder's stored form, lower-cased where paths are case-insensitive).

use crate::folder_path;
use crate::fsutil;
use crate::paths::Os;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const FILE: &str = "linked-folders";
const HEADER: &str = "acta-linked-folders 1";

pub struct LinkIndex {
    salt: [u8; 16],
    hashes: BTreeSet<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// The form that is hashed: stored form, case-folded where the OS compares
/// paths without case.
fn key(os: Os, stored: &str) -> String {
    let n = folder_path::normalize(os, stored);
    if os == Os::Linux { n } else { n.to_lowercase() }
}

impl LinkIndex {
    fn hash(&self, os: Os, stored: &str) -> String {
        let mut h = Sha256::new();
        h.update(self.salt);
        h.update(key(os, stored).as_bytes());
        hex(&h.finalize()[..16])
    }

    pub fn read(data_dir: &Path) -> Option<LinkIndex> {
        let text = fs::read_to_string(data_dir.join(FILE)).ok()?;
        let mut lines = text.lines();
        if lines.next()? != HEADER {
            return None;
        }
        let salt: [u8; 16] = unhex(lines.next()?)?.try_into().ok()?;
        let hashes = lines
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        Some(LinkIndex { salt, hashes })
    }

    /// Whether `dir` or one of its ancestors is an attached folder. `dirs`
    /// are the candidate directories in stored form (the launch directory and,
    /// for a git worktree, its main checkout).
    pub fn matches(&self, os: Os, dirs: &[String]) -> bool {
        dirs.iter().any(|d| {
            Path::new(d)
                .ancestors()
                .filter_map(|a| a.to_str())
                .filter(|a| !a.is_empty())
                .any(|a| self.hashes.contains(&self.hash(os, a)))
        })
    }

    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }
}

/// Rewrites the index for `folders` (stored forms), keeping the salt of an
/// existing file. Writes only when the content changes.
pub fn write(data_dir: &Path, os: Os, folders: &[String]) -> io::Result<()> {
    let salt = match LinkIndex::read(data_dir) {
        Some(i) => i.salt,
        None => {
            let mut s = [0u8; 16];
            getrandom::fill(&mut s).map_err(|e| io::Error::other(e.to_string()))?;
            s
        }
    };
    let mut index = LinkIndex {
        salt,
        hashes: BTreeSet::new(),
    };
    let hashes: BTreeSet<String> = folders.iter().map(|f| index.hash(os, f)).collect();
    index.hashes = hashes;
    let mut text = format!("{HEADER}\n{}\n", hex(&index.salt));
    for h in &index.hashes {
        text.push_str(h);
        text.push('\n');
    }
    let path = data_dir.join(FILE);
    if fs::read_to_string(&path).is_ok_and(|cur| cur == text) {
        return Ok(());
    }
    fsutil::create_private_dir(data_dir)?;
    let tmp: PathBuf = data_dir.join(format!("{FILE}.tmp-{}", std::process::id()));
    let _ = fs::remove_file(&tmp);
    {
        let mut f = fsutil::create_new_private_file(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_folders_and_their_descendants_only() {
        let dir = tempfile::tempdir().unwrap();
        let os = Os::Windows;
        write(dir.path(), os, &[r"C:\work\acta".into()]).unwrap();
        let text = fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!text.contains("acta\n") && !text.to_lowercase().contains("work"));
        let i = LinkIndex::read(dir.path()).unwrap();
        assert!(i.matches(os, &[r"C:\work\acta".into()]));
        assert!(i.matches(os, &[r"c:\WORK\acta\crates\core".into()]));
        assert!(!i.matches(os, &[r"C:\work".into()]));
        assert!(!i.matches(os, &[r"C:\work\actable".into()]));
        assert!(!i.matches(os, &[r"D:\other".into(), r"C:\elsewhere".into()]));
    }

    #[test]
    fn salt_survives_rewrites_and_unchanged_content_is_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), Os::Linux, &["/home/alexandre/a".into()]).unwrap();
        let first = fs::read_to_string(dir.path().join(FILE)).unwrap();
        write(
            dir.path(),
            Os::Linux,
            &["/home/alexandre/a".into(), "/home/alexandre/b".into()],
        )
        .unwrap();
        let second = fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert_eq!(first.lines().nth(1), second.lines().nth(1));
        assert_eq!(second.lines().count(), 4);
        let i = LinkIndex::read(dir.path()).unwrap();
        assert!(i.matches(Os::Linux, &["/home/alexandre/b/src".into()]));
        assert!(!i.matches(Os::Linux, &["/home/Alexandre/b".into()]));
    }

    #[test]
    fn missing_or_foreign_file_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(LinkIndex::read(dir.path()).is_none());
        fs::write(dir.path().join(FILE), "something else\n").unwrap();
        assert!(LinkIndex::read(dir.path()).is_none());
    }
}
