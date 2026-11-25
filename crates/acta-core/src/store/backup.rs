//! Backups (ADR 0006, amended 2026-11-22): the payload of `exchange.rs`
//! encrypted with a passphrase the human chooses, in an `.acta-backup`
//! file. The only way data leaves Acta and comes back.
//!
//! The file is a standard age v1 file (https://age-encryption.org/v1)
//! with a single scrypt recipient: the passphrase goes through scrypt
//! (N = 2^18, r = 8, p = 1: 256 MiB and about a second) to unwrap a random
//! file key; the payload is ChaCha20-Poly1305 in 64 KiB chunks (STREAM),
//! each authenticated, the last one marked, so a truncated or altered file
//! fails before anything is imported. The file starts with age's own magic
//! and version line, `age-encryption.org/v1`; the payload inside carries
//! Acta's format name and version. Any age implementation opens it with
//! the passphrase (`age -d backup.acta-backup`), so a backup stays
//! readable without Acta, as ADR 0006 asks.
//!
//! Writing streams: the JSON goes through the encryptor as it is
//! produced. Reading streams too, with the decrypted payload capped at
//! [`MAX_IMPORT_BYTES`] while it is read, so neither a large backup nor a
//! hostile one makes Acta hold more than the records it parses. A file
//! that asks for more than N = 2^20 (1 GiB of scrypt memory) is refused
//! before any work. The passphrase is held in a [`Passphrase`], wiped on
//! drop, and never written, logged or passed as an argument.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use age::secrecy::{ExposeSecret, SecretString};

use super::exchange::{ExportFile, Header, MAX_IMPORT_BYTES, check_header};
use crate::error::{Error, Result};

/// The file extension, without the leading dot.
pub const BACKUP_EXTENSION: &str = "acta-backup";
/// The shortest passphrase a new backup takes, in characters. The only
/// rule: a strength hint says the rest.
pub const MIN_PASSPHRASE_CHARS: usize = 8;

/// How every age file starts: its magic and version line.
const MAGIC: &[u8] = b"age-encryption.org/v1\n";
/// scrypt work factor of a new backup: N = 2^18 (256 MiB, about a second
/// on a recent computer). Unit tests use a small one: scrypt is the slow
/// part of every round trip.
#[cfg(not(test))]
const LOG_N: u8 = 18;
#[cfg(test)]
const LOG_N: u8 = 10;
/// The most work a backup may ask for: N = 2^20, 1 GiB. A file asking more
/// was not made by Acta (or by `age` on a much faster machine) and could
/// tie up the computer: it is refused before scrypt runs.
const MAX_LOG_N: u8 = 20;
/// Buffer between the JSON and the encryption, both ways.
const BUFFER: usize = 64 * 1024;

/// A passphrase, wiped from memory when dropped. Its `Debug` shows
/// nothing.
pub struct Passphrase(SecretString);

impl Passphrase {
    pub fn new(text: String) -> Self {
        Passphrase(SecretString::from(text))
    }

    /// Its length in characters.
    pub fn chars(&self) -> usize {
        self.0.expose_secret().chars().count()
    }

    /// Whether the two are the same text (the "repeat" check).
    pub fn same_as(&self, other: &Passphrase) -> bool {
        self.0.expose_secret() == other.0.expose_secret()
    }

    /// The strength hint shown while it is typed.
    pub fn strength(&self) -> Strength {
        strength(self.0.expose_secret())
    }
}

impl From<String> for Passphrase {
    fn from(text: String) -> Self {
        Passphrase::new(text)
    }
}

impl std::fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Passphrase(..)")
    }
}

/// A hint, never a rule beyond [`MIN_PASSPHRASE_CHARS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strength {
    TooShort,
    Weak,
    Fair,
    Strong,
}

/// A rough estimate in bits: the length (at most twice the number of
/// distinct characters, so "aaaaaaaa" counts as two) times the bits of the
/// character classes used. Under 45 bits is weak, under 70 fair. Four
/// random words are strong; "Password1!" is fair; "12345678" is weak.
pub fn strength(text: &str) -> Strength {
    let length = text.chars().count();
    if length < MIN_PASSPHRASE_CHARS {
        return Strength::TooShort;
    }
    let mut distinct: Vec<char> = text.chars().collect();
    distinct.sort_unstable();
    distinct.dedup();
    let has = |f: fn(&char) -> bool| text.chars().any(|c| f(&c));
    let pool = [
        (has(|c| c.is_ascii_lowercase()), 26),
        (has(|c| c.is_ascii_uppercase()), 26),
        (has(|c| c.is_ascii_digit()), 10),
        (has(|c| !c.is_ascii_alphanumeric()), 33),
    ]
    .iter()
    .filter(|(used, _)| *used)
    .map(|(_, n)| n)
    .sum::<u32>();
    let counted = length.min(2 * distinct.len()) as f64;
    let bits = counted * f64::from(pool.max(2)).log2();
    if bits < 45.0 {
        Strength::Weak
    } else if bits < 70.0 {
        Strength::Fair
    } else {
        Strength::Strong
    }
}

/// Whether the file at `path` is a backup (it starts as an age file does).
/// Reads its first 22 bytes only.
pub fn is_backup(path: &Path) -> io::Result<bool> {
    let mut head = [0u8; MAGIC.len()];
    let mut file = File::open(path)?;
    let mut read = 0;
    while read < head.len() {
        match file.read(&mut head[read..])? {
            0 => break,
            n => read += n,
        }
    }
    Ok(read == head.len() && head == MAGIC)
}

/// Refuses, before any passphrase is asked, a file that is not a backup
/// or is larger than an import reads.
pub fn check_backup_file(path: &Path) -> Result<()> {
    if !is_backup(path)? {
        return Err(Error::Invalid("this is not an Acta backup".into()));
    }
    let size = std::fs::metadata(path)?.len();
    if size > largest_file(MAX_IMPORT_BYTES) {
        return Err(Error::FileTooLarge {
            size,
            limit: MAX_IMPORT_BYTES,
        });
    }
    Ok(())
}

/// The largest backup file a payload of `cap` bytes makes: age's header
/// and 16 bytes per 64 KiB chunk.
fn largest_file(cap: u64) -> u64 {
    cap + cap / 4096 + BUFFER as u64
}

impl ExportFile {
    /// Encrypts this payload with `passphrase` into `out`, streaming, and
    /// returns `out` once the last chunk is written.
    pub fn write_backup<W: Write>(&self, passphrase: &Passphrase, out: W) -> Result<W> {
        if passphrase.chars() < MIN_PASSPHRASE_CHARS {
            return Err(Error::PassphraseTooShort {
                min: MIN_PASSPHRASE_CHARS,
            });
        }
        let mut recipient = age::scrypt::Recipient::new(passphrase.0.clone());
        recipient.set_work_factor(LOG_N);
        let encryptor =
            age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
                .map_err(|_| Error::Invalid("the backup could not be encrypted".into()))?;
        let mut stream = encryptor.wrap_output(out)?;
        {
            let mut buffered = BufWriter::with_capacity(BUFFER, &mut stream);
            self.write_json(&mut buffered).map_err(io_first)?;
            buffered.flush()?;
        }
        Ok(stream.finish()?)
    }

    /// Writes a backup to `path`, replacing what the save dialog agreed to
    /// replace: a temporary file next to it, renamed over it once whole.
    pub fn save_backup(&self, path: &Path, passphrase: &Passphrase) -> Result<()> {
        crate::fsutil::write_private_atomic_with(path, |file| {
            self.write_backup(passphrase, file).map(|_| ())
        })
    }

    /// Writes a backup to `path`, which must not exist (the CLI never
    /// overwrites). A failed write leaves nothing behind.
    pub fn save_backup_new(&self, path: &Path, passphrase: &Passphrase) -> Result<()> {
        if passphrase.chars() < MIN_PASSPHRASE_CHARS {
            return Err(Error::PassphraseTooShort {
                min: MIN_PASSPHRASE_CHARS,
            });
        }
        let file = crate::fsutil::create_new_private_file(path)?;
        let written = self
            .write_backup(passphrase, file)
            .and_then(|file| file.sync_all().map_err(Error::from));
        if written.is_err() {
            let _ = std::fs::remove_file(path);
        }
        written
    }

    /// Decrypts and parses a backup. Nothing is returned, so nothing can
    /// be imported, unless the whole file decrypted and authenticated.
    pub fn read_backup(path: &Path, passphrase: &Passphrase) -> Result<ExportFile> {
        read_backup_within(path, passphrase, MAX_IMPORT_BYTES)
    }
}

/// A serialisation error that is the writer's own failure stays an I/O
/// error.
fn io_first(e: Error) -> Error {
    match e {
        Error::Json(j) if j.is_io() => Error::Io(io::Error::from(j)),
        e => e,
    }
}

/// [`ExportFile::read_backup`] against a given payload limit (tests).
pub(crate) fn read_backup_within(
    path: &Path,
    passphrase: &Passphrase,
    cap: u64,
) -> Result<ExportFile> {
    let size = std::fs::metadata(path)?.len();
    if size > largest_file(cap) {
        return Err(Error::FileTooLarge { size, limit: cap });
    }
    let mut capped = Capped {
        inner: open(path, passphrase)?,
        left: cap,
        over: false,
    };
    let parsed =
        serde_json::from_reader::<_, ExportFile>(BufReader::with_capacity(BUFFER, &mut capped));
    match parsed {
        Ok(file) => {
            check_header(&file.format, file.version)?;
            Ok(file)
        }
        Err(_) if capped.over => Err(Error::FileTooLarge { size, limit: cap }),
        Err(e) if e.is_io() => Err(payload_io(io::Error::from(e))),
        Err(e) => {
            // Decrypted and authenticated, but not a payload this build
            // reads: a foreign, newer or older document says so rather
            // than which field failed. Its header is read again, the rest
            // skipped without being kept.
            let mut again = Capped {
                inner: open(path, passphrase)?,
                left: cap,
                over: false,
            };
            if let Ok(header) =
                serde_json::from_reader::<_, Header>(BufReader::with_capacity(BUFFER, &mut again))
            {
                check_header(&header.format, header.version)?;
            }
            Err(e.into())
        }
    }
}

/// The decrypted payload of the backup at `path`.
fn open(path: &Path, passphrase: &Passphrase) -> Result<impl Read> {
    if !is_backup(path)? {
        return Err(Error::Invalid("this is not an Acta backup".into()));
    }
    let input = BufReader::with_capacity(BUFFER, File::open(path)?);
    let decryptor = age::Decryptor::new_buffered(input).map_err(key_error)?;
    if !decryptor.is_scrypt() {
        // An age file for a key, not a passphrase: not one Acta made.
        return Err(Error::Invalid("this is not an Acta backup".into()));
    }
    let mut identity = age::scrypt::Identity::new(passphrase.0.clone());
    identity.set_max_work_factor(MAX_LOG_N);
    decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(key_error)
}

/// The header and the passphrase check, before any payload.
fn key_error(e: age::DecryptError) -> Error {
    use age::DecryptError as D;
    match e {
        D::DecryptionFailed | D::KeyDecryptionFailed | D::NoMatchingKeys => Error::WrongPassphrase,
        D::ExcessiveWork { .. } => Error::Invalid("this is not an Acta backup".into()),
        D::Io(e) => payload_io(e),
        _ => Error::BackupDamaged,
    }
}

/// A payload chunk that fails its check, or a file that ends early, is a
/// damaged backup; anything else is the disk's own error.
fn payload_io(e: io::Error) -> Error {
    match e.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => Error::BackupDamaged,
        _ => Error::Io(e),
    }
}

/// A reader that fails once more than `left` bytes come through.
struct Capped<R> {
    inner: R,
    left: u64,
    over: bool,
}

impl<R: Read> Read for Capped<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            let mut one = [0u8; 1];
            return match self.inner.read(&mut one)? {
                0 => Ok(0),
                _ => {
                    self.over = true;
                    Err(io::Error::other(
                        "the backup is larger than an import reads",
                    ))
                }
            };
        }
        let at_most = buf
            .len()
            .min(usize::try_from(self.left).unwrap_or(usize::MAX));
        let n = self.inner.read(&mut buf[..at_most])?;
        self.left -= n as u64;
        Ok(n)
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
