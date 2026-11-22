//! The first connections of a process, opened at the same moment from
//! several threads. SQLCipher registers its crypto provider in
//! `SQLITE_EXTRA_INIT`, which SQLite runs after it has marked itself
//! initialised and outside its init mutex: a second thread could open a
//! connection before the provider existed, and the open failed with a
//! database error (CI, ubuntu-24.04, `open_existing_never_creates_a_database`).
//! Its own test binary, so nothing has touched SQLite before.

use acta_core::keys::MemoryKeyStore;
use acta_core::{Store, StoreConfig};
use std::sync::{Arc, Barrier};

#[test]
fn simultaneous_first_opens_all_succeed() {
    const THREADS: usize = 16;
    let start = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                let tmp = tempfile::tempdir().unwrap();
                let keys = MemoryKeyStore::new();
                let config = StoreConfig {
                    data_dir: tmp.path().join("data"),
                    keys: keys.stores(),
                };
                start.wait();
                Store::open(&config).map(|_| ()).map_err(|e| e.to_string())
            })
        })
        .collect();
    let failures: Vec<String> = handles
        .into_iter()
        .filter_map(|h| h.join().unwrap().err())
        .collect();
    assert!(failures.is_empty(), "{failures:?}");
}
