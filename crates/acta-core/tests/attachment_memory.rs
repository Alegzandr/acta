//! A 25 MB file goes into the database and comes back out with bounded
//! memory (ADR 0012, 1; PLAN "Attachments", Store). Its own test binary, so
//! the counting allocator sees only this test. SQLite's page cache is
//! allocated by SQLite itself, outside Rust's allocator, and is capped by
//! `cache_size`; what this measures is everything Acta's code holds.

use acta_core::keys::MemoryKeyStore;
use acta_core::model::{AttachOwner, Author};
use acta_core::store::{AttachSource, MAX_FILE_BYTES, Store, StoreConfig};
use sha2::{Digest, Sha256};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = NOW.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        NOW.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Bytes held above the level when the measure started.
fn measure<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = NOW.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let out = f();
    (out, PEAK.load(Ordering::Relaxed).saturating_sub(base))
}

/// Deterministic bytes that are not text and do not compress.
fn block(i: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    while out.len() < 64 * 1024 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

#[test]
fn a_25_mb_file_round_trips_with_bounded_memory() {
    const LIMIT: usize = 4 * 1024 * 1024;
    let dir = tempfile::tempdir().unwrap();
    let keys = MemoryKeyStore::new();
    let config = StoreConfig {
        data_dir: dir.path().join("data"),
        keys: keys.stores(),
    };
    let mut store = Store::open(&config).unwrap();
    let project = store
        .create_project("Alexandre", "", Author::Human)
        .unwrap();
    let note = store
        .add_note(project, "Recording of the meeting", Author::Human)
        .unwrap();

    // The file, written in blocks, and its hash.
    let path = dir.path().join("meeting.bin");
    let mut hasher = Sha256::new();
    let mut f = std::fs::File::create(&path).unwrap();
    let mut written = 0u64;
    let mut i = 0;
    while written < MAX_FILE_BYTES {
        let b = block(i);
        let take = (MAX_FILE_BYTES - written).min(b.len() as u64) as usize;
        f.write_all(&b[..take]).unwrap();
        hasher.update(&b[..take]);
        written += take as u64;
        i += 1;
    }
    drop(f);
    let expected = hasher.finalize();

    let (attached, held) = measure(|| {
        store
            .attach(
                AttachOwner::Note(note),
                "meeting.bin",
                AttachSource::Path(path.clone()),
                Author::Human,
                &mut |_, _| {},
            )
            .unwrap()
    });
    assert_eq!(attached.size as u64, MAX_FILE_BYTES);
    eprintln!("attach held {held} bytes");
    assert!(held < LIMIT, "attach held {held} bytes");

    let (got, held) = measure(|| {
        let mut reader = store.read_attachment(attached.id).unwrap();
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            total += n as u64;
        }
        (hasher.finalize(), total)
    });
    assert_eq!(got.1, MAX_FILE_BYTES);
    assert_eq!(got.0, expected);
    eprintln!("read held {held} bytes");
    assert!(held < LIMIT, "read held {held} bytes");
}
