//! Errors. Messages name records by short id and never quote user text or
//! key material, so they are safe to log and to show to an agent.

use std::fmt;
use std::path::PathBuf;

/// Why the database could not be opened. The first three are the key-loss
/// and damage cases of ADR 0006: the file is left untouched and the UI offers
/// recovery (import an export, or restore a snapshot).
#[derive(Debug)]
pub enum OpenError {
    /// The database exists but its key is in none of the stores.
    KeyMissing {
        db: PathBuf,
        /// The store the key should be in (`keychain` or `file`), or `None`
        /// when no store had it and none was recorded.
        store: Option<&'static str>,
    },
    /// The database exists, a key was found, and it does not decrypt it (or
    /// the file is not a database).
    KeyWrongOrCorrupt {
        db: PathBuf,
    },
    /// The marker names a store this build does not offer, or that store
    /// failed. Never a reason to make a new key.
    KeyStore(String),
    /// A migration failed; the pre-migration snapshot was restored.
    MigrationFailed {
        message: String,
        restored_from: Option<PathBuf>,
    },
    Io(std::io::Error),
    Db(rusqlite::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::KeyMissing { db, store } => {
                let where_ = match store {
                    Some("keychain") => "its key is not in the keychain",
                    Some("file") => "its key file (acta.key) is missing",
                    _ => "its key was not found",
                };
                write!(f, "the database {} exists but {where_}", db.display())
            }
            OpenError::KeyWrongOrCorrupt { db } => write!(
                f,
                "the key does not open the database {} (wrong key or damaged file)",
                db.display()
            ),
            OpenError::KeyStore(e) => write!(f, "key store: {e}"),
            OpenError::MigrationFailed {
                message,
                restored_from,
            } => match restored_from {
                Some(p) => write!(
                    f,
                    "database upgrade failed ({message}); restored the snapshot {}",
                    p.display()
                ),
                None => write!(f, "database upgrade failed ({message})"),
            },
            OpenError::Io(e) => write!(f, "{e}"),
            OpenError::Db(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<std::io::Error> for OpenError {
    fn from(e: std::io::Error) -> Self {
        OpenError::Io(e)
    }
}

impl From<rusqlite::Error> for OpenError {
    fn from(e: rusqlite::Error) -> Self {
        OpenError::Db(e)
    }
}

/// Error of a query or command on an open store.
#[derive(Debug)]
pub enum Error {
    /// No live record with this id (or it is in the trash).
    NotFound(String),
    /// The request breaks a rule: empty title, wrong level, cross-project move.
    Invalid(String),
    /// A knowledge write named a revision that is no longer current.
    StaleRevision {
        current: i64,
    },
    /// The record cannot be restored while its parent is in the trash.
    ParentDeleted(String),
    /// The database was written by a newer Acta whose changes this build must
    /// not overwrite (`min_compatible_version`); restart after the update.
    ReadOnly,
    /// An export file of a newer format than this build reads.
    FormatTooNew {
        found: i64,
        supported: i64,
    },
    Io(std::io::Error),
    Json(serde_json::Error),
    Db(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(what) => write!(f, "{what} not found"),
            Error::Invalid(why) => f.write_str(why),
            Error::StaleRevision { current } => {
                write!(f, "stale revision: the current revision is {current}")
            }
            Error::ParentDeleted(what) => write!(f, "{what} is in the trash; restore it first"),
            Error::ReadOnly => f.write_str(
                "the database was upgraded by a newer Acta; restart Acta to finish the update",
            ),
            Error::FormatTooNew { found, supported } => write!(
                f,
                "this file is format {found}; this Acta reads up to format {supported}. Update Acta."
            ),
            Error::Io(e) => write!(f, "{e}"),
            Error::Json(e) => write!(f, "{e}"),
            Error::Db(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
