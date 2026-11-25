//! The database key and where it is kept (ADR 0003).
//!
//! 256 random bits, given to SQLCipher as a raw key and stored as 64 hex
//! characters. Never logged, never in process arguments or the environment.
//!
//! Where the key lives is a [`KeyStore`]. The OS stores are the keychain
//! ([`OsKeychain`]) and, where the ADR says so, a key file ([`KeyFile`]). Which
//! one a database uses is decided once, when the database is created, and
//! written in the `keystore` marker file next to it; it is never re-decided.
//! Tests inject [`MemoryKeyStore`] and never touch the real keychain.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zeroize::{Zeroize, Zeroizing};

/// Keychain service and account names of the database key.
pub const KEYCHAIN_SERVICE: &str = "Acta";
pub const KEYCHAIN_ACCOUNT: &str = "database-key-v1";
/// File name of the marker that records the store a database's key is in.
pub const MARKER_FILE: &str = "keystore";
/// File name of the key file, for the stores that use one.
pub const KEY_FILE: &str = "acta.key";

/// A 256-bit database key. Its `Debug` prints nothing of it, and it is wiped
/// from memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; 32]);

impl Key {
    /// A fresh key from the OS random number generator.
    pub fn generate() -> io::Result<Key> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Key(bytes))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Key {
        Key(bytes)
    }

    /// `HMAC-SHA256(self, label)`: a secret for another purpose that reveals
    /// nothing of the key.
    pub fn derive(&self, label: &[u8]) -> Zeroizing<[u8; 32]> {
        use hmac::{Hmac, KeyInit, Mac};
        let mut mac = <Hmac<sha2::Sha256> as KeyInit>::new_from_slice(&self.0)
            .expect("HMAC takes a key of any length");
        mac.update(label);
        Zeroizing::new(mac.finalize().into_bytes().into())
    }

    /// The stored form: 64 lower-case hex characters, wiped when dropped.
    pub fn to_hex(&self) -> Zeroizing<String> {
        let mut s = Zeroizing::new(String::with_capacity(64));
        for b in self.0 {
            s.push(char::from(HEX[usize::from(b >> 4)]));
            s.push(char::from(HEX[usize::from(b & 15)]));
        }
        s
    }

    pub fn from_hex(text: &str) -> Option<Key> {
        let t = text.trim().as_bytes();
        if t.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, pair) in t.chunks(2).enumerate() {
            let hi = (pair[0] as char).to_digit(16)?;
            let lo = (pair[1] as char).to_digit(16)?;
            bytes[i] = (hi * 16 + lo) as u8;
        }
        Some(Key(bytes))
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(..)")
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Error from a key store. The message never contains key material.
#[derive(Debug)]
pub struct KeyStoreError(pub String);

impl fmt::Display for KeyStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KeyStoreError {}

/// Somewhere a key can be kept.
pub trait KeyStore: Send + Sync {
    /// Name written in the marker file: `keychain`, `file`, or a test name.
    fn name(&self) -> &'static str;
    /// `Ok(None)` means the store works and holds no key, which is distinct
    /// from an error (for example a Secret Service that does not answer).
    fn load(&self) -> Result<Option<Key>, KeyStoreError>;
    fn save(&self, key: &Key) -> Result<(), KeyStoreError>;
    /// Deletes the key (Reset Acta, ADR 0006). No key is not an error.
    fn delete(&self) -> Result<(), KeyStoreError>;
}

/// The stores a database may use, in order of preference for a new database.
/// An existing database uses the one its marker names.
#[derive(Clone)]
pub struct KeyStores(pub Vec<Arc<dyn KeyStore>>);

impl KeyStores {
    /// The stores of ADR 0003 for this OS, for a database in `data_dir`:
    /// - macOS: the key file `data_dir/acta.key`, always. The keychain is not
    ///   offered: with ad-hoc signed builds every update would prompt
    ///   (confirmed on a macOS runner, ADR 0003).
    /// - Windows: the Credential Manager.
    /// - Linux and other Unix: the Secret Service, falling back to the key
    ///   file when the Secret Service is unavailable at creation.
    pub fn os_default(data_dir: &Path) -> KeyStores {
        let file = || -> Arc<dyn KeyStore> { Arc::new(KeyFile::new(data_dir.join(KEY_FILE))) };
        let keychain = || -> Arc<dyn KeyStore> { Arc::new(OsKeychain::new()) };
        #[cfg(target_os = "macos")]
        let stores = {
            let _ = keychain;
            vec![file()]
        };
        #[cfg(windows)]
        let stores = {
            let _ = file;
            vec![keychain()]
        };
        #[cfg(not(any(target_os = "macos", windows)))]
        let stores = vec![keychain(), file()];
        KeyStores(stores)
    }

    pub fn by_name(&self, name: &str) -> Option<&Arc<dyn KeyStore>> {
        self.0.iter().find(|s| s.name() == name)
    }
}

/// The OS keychain through `keyring`: Credential Manager, Secret Service or
/// the macOS login keychain.
pub struct OsKeychain {
    service: String,
    account: String,
}

impl OsKeychain {
    pub fn new() -> Self {
        Self::with_names(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
    }

    pub fn with_names(service: &str, account: &str) -> Self {
        Self {
            service: service.to_owned(),
            account: account.to_owned(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, KeyStoreError> {
        keyring::Entry::new(&self.service, &self.account).map_err(keyring_error)
    }
}

impl Default for OsKeychain {
    fn default() -> Self {
        Self::new()
    }
}

fn keyring_error(e: keyring::Error) -> KeyStoreError {
    KeyStoreError(format!("keychain: {e}"))
}

impl KeyStore for OsKeychain {
    fn name(&self) -> &'static str {
        "keychain"
    }

    fn load(&self) -> Result<Option<Key>, KeyStoreError> {
        match self.entry()?.get_password().map(Zeroizing::new) {
            Ok(text) => Key::from_hex(&text)
                .map(Some)
                .ok_or_else(|| KeyStoreError("keychain entry is not a 256-bit hex key".into())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_error(e)),
        }
    }

    fn save(&self, key: &Key) -> Result<(), KeyStoreError> {
        self.entry()?
            .set_password(&key.to_hex())
            .map_err(keyring_error)
    }

    fn delete(&self) -> Result<(), KeyStoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keyring_error(e)),
        }
    }
}

/// A key file: mode 0600 in a 0700 directory on Unix, the profile ACL on
/// Windows. Never replaced once written.
pub struct KeyFile {
    path: PathBuf,
}

impl KeyFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl KeyStore for KeyFile {
    fn name(&self) -> &'static str {
        "file"
    }

    fn load(&self) -> Result<Option<Key>, KeyStoreError> {
        match fs::read_to_string(&self.path).map(Zeroizing::new) {
            Ok(text) => Key::from_hex(&text)
                .map(Some)
                .ok_or_else(|| KeyStoreError("key file is not a 256-bit hex key".into())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(KeyStoreError(format!("key file: {e}"))),
        }
    }

    fn save(&self, key: &Key) -> Result<(), KeyStoreError> {
        let err = |e: io::Error| KeyStoreError(format!("key file: {e}"));
        if let Some(dir) = self.path.parent() {
            crate::fsutil::create_private_dir(dir).map_err(err)?;
        }
        let mut file = crate::fsutil::create_new_private_file(&self.path).map_err(err)?;
        file.write_all(key.to_hex().as_bytes()).map_err(err)?;
        file.sync_all().map_err(err)
    }

    fn delete(&self) -> Result<(), KeyStoreError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(KeyStoreError(format!("key file: {e}"))),
        }
    }
}

/// An in-memory store for tests and tools: shared between clones, so two
/// "processes" of a test see the same key.
#[derive(Clone, Default)]
pub struct MemoryKeyStore {
    key: Arc<Mutex<Option<Key>>>,
    fail: Arc<Mutex<bool>>,
}

impl MemoryKeyStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes the key, as a lost keychain entry would.
    pub fn forget(&self) {
        *self.key.lock().unwrap() = None;
    }

    /// Makes every call fail, as an unreachable Secret Service would.
    pub fn set_failing(&self, fail: bool) {
        *self.fail.lock().unwrap() = fail;
    }

    pub fn set(&self, key: Key) {
        *self.key.lock().unwrap() = Some(key);
    }

    pub fn stores(&self) -> KeyStores {
        KeyStores(vec![Arc::new(self.clone())])
    }

    fn check(&self) -> Result<(), KeyStoreError> {
        if *self.fail.lock().unwrap() {
            Err(KeyStoreError("store unavailable".into()))
        } else {
            Ok(())
        }
    }
}

impl KeyStore for MemoryKeyStore {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn load(&self) -> Result<Option<Key>, KeyStoreError> {
        self.check()?;
        Ok(self.key.lock().unwrap().clone())
    }

    fn save(&self, key: &Key) -> Result<(), KeyStoreError> {
        self.check()?;
        self.set(key.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), KeyStoreError> {
        self.check()?;
        self.forget();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_secrets_depend_on_key_and_label() {
        let key = Key::from_bytes([7; 32]);
        let a = key.derive(b"acta agent-browser ipc v1");
        assert_eq!(
            *a,
            *Key::from_bytes([7; 32]).derive(b"acta agent-browser ipc v1")
        );
        assert_ne!(*a, *key.derive(b"another purpose"));
        assert_ne!(
            *a,
            *Key::from_bytes([8; 32]).derive(b"acta agent-browser ipc v1")
        );
        assert_ne!(&a[..], &[7u8; 32][..]);
    }

    /// Known answer, computed with an independent HMAC-SHA256: a crate
    /// upgrade must never change the secrets derived from a stored key.
    #[test]
    fn derive_is_hmac_sha256_of_the_label() {
        let key = Key::from_bytes([7; 32]);
        let hex: String = key
            .derive(b"acta agent-browser ipc v1")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "8377a154c994913b00a76a8a786052aa317cb5333c9feca05328e6d637ef508b"
        );
    }

    #[test]
    fn hex_round_trip_and_redacted_debug() {
        let key = Key::generate().unwrap();
        let hex = key.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(Key::from_hex(&hex), Some(key.clone()));
        assert_eq!(Key::from_hex(&hex[..62]), None);
        assert_eq!(Key::from_hex(&"zz".repeat(32)), None);
        assert_eq!(format!("{key:?}"), "Key(..)");
        assert_ne!(Key::generate().unwrap(), key);
    }

    #[test]
    fn key_file_is_written_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeyFile::new(dir.path().join("sub").join(KEY_FILE));
        assert!(store.load().unwrap().is_none());
        let key = Key::generate().unwrap();
        store.save(&key).unwrap();
        assert_eq!(store.load().unwrap(), Some(key));
        // Never overwritten: a second save fails instead of replacing the key.
        assert!(store.save(&Key::generate().unwrap()).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.path().join("sub").join(KEY_FILE);
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
            let dir_mode = fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(dir_mode & 0o777, 0o700);
        }
    }

    fn default_names(dir: &Path) -> Vec<&'static str> {
        KeyStores::os_default(dir)
            .0
            .iter()
            .map(|s| s.name())
            .collect()
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_always_uses_the_private_key_file() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Acta").join("data");
        assert_eq!(default_names(&dir), ["file"]);
        let key = Key::generate().unwrap();
        KeyStores::os_default(&dir).0[0].save(&key).unwrap();
        let path = dir.join(KEY_FILE);
        assert_eq!(KeyFile::new(path.clone()).load().unwrap(), Some(key));
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&dir), 0o700);
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_the_credential_manager_only() {
        assert_eq!(default_names(Path::new("data")), ["keychain"]);
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn linux_uses_the_secret_service_then_the_key_file() {
        assert_eq!(default_names(Path::new("data")), ["keychain", "file"]);
    }
}
