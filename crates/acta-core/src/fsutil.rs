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
