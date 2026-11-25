//! File creation with private permissions: directories 0700 and files 0600
//! on Unix (ADR 0003). On Windows the profile ACL inherited from
//! `%LOCALAPPDATA%` already restricts them to the user.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Creates `dir` and its missing parents; the directories created get 0700.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

/// Creates a new file (fails if it exists) readable by the user only.
pub fn create_new_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Writes `bytes` to `path` without ever leaving half a file: a new file
/// with a random name (`.{16 hex}.tmp`) next to the target, created
/// exclusively so nothing planted there, a symlink included, is followed
/// or reused; flushed, then renamed over the target. On Unix the file is
/// 0600. On Windows it inherits the ACL of the directory it is written in:
/// private in the data directory, as open as that directory elsewhere (a
/// backup the user saves to a shared folder is readable there).
pub fn write_private_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    write_private_atomic_with(path, |file| file.write_all(bytes))
}

/// [`write_private_atomic`] with the content written by `write`, which may
/// stream it: the target appears whole once `write` succeeded, and the
/// temporary file is removed on any failure.
pub fn write_private_atomic_with<E: From<io::Error>>(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), E>,
) -> Result<(), E> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let temp = dir.join(format!(".{suffix}.tmp"));
    let mut file = create_new_private_file(&temp)?;
    let written = write(&mut file).and_then(|()| file.sync_all().map_err(E::from));
    drop(file);
    let result = written.and_then(|()| fs::rename(&temp, path).map_err(E::from));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Sets 0600 on a file SQLite created (the database, its WAL, a snapshot).
pub fn make_private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_private_atomic_write_replaces_the_file_and_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("acta-2026-11-21.acta-backup");
        fs::write(&path, "old").unwrap();
        write_private_atomic(&path, b"{\"format\":\"acta\"}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"format\":\"acta\"}");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(write_private_atomic(&dir.path().join("missing").join("x.json"), b"x").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_temporary_name_is_not_predictable_from_the_process() {
        // The old scheme used `.{name}.{pid}.tmp`: a symlink planted there
        // must be neither followed nor written.
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim");
        fs::write(&victim, "keep").unwrap();
        let path = dir.path().join("out.json");
        let planted = dir
            .path()
            .join(format!(".out.json.{}.tmp", std::process::id()));
        std::os::unix::fs::symlink(&victim, &planted).unwrap();
        write_private_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }
}
