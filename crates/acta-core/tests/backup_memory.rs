//! A large backup is written and read without holding its whole text: the
//! JSON streams through the encryption, and the decrypted payload streams
//! into the parser. Measured with a counting allocator, in its own test
//! binary so no other test allocates meanwhile. scrypt's own 256 MiB work
//! area (one allocation per key derivation) is left out of the count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use acta_core::store::{ExportFile, Passphrase};

/// Allocations this large are scrypt's work area, not the payload.
const SCRYPT_AREA: usize = 128 * 1024 * 1024;

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn counted(size: usize) -> bool {
    size < SCRYPT_AREA
}

fn grow(by: usize) {
    let now = LIVE.fetch_add(by, Ordering::Relaxed) + by;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counted(layout.size()) {
            grow(layout.size());
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if counted(layout.size()) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if counted(layout.size()) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        if counted(new_size) {
            grow(new_size);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The peak of live bytes above what was live when `f` started.
fn peak_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let out = f();
    (out, PEAK.load(Ordering::Relaxed) - base)
}

const MIB: usize = 1024 * 1024;

#[test]
fn a_large_backup_streams_both_ways() {
    // A payload of about 32 MiB in 512 attached files of 64 KiB each:
    // no single string is large, so a reader that held the whole text
    // next to the records would need twice what streaming needs.
    let blobs: Vec<serde_json::Value> = (0..512)
        .map(|i| {
            let content = format!("{i:04}").repeat(64 * 1024 / 4);
            serde_json::json!({ "hash": format!("{i:064}"), "size": 48 * 1024,
                                "media_type": "application/octet-stream", "content": content })
        })
        .collect();
    let json = serde_json::json!({
        "format": "acta", "version": 5, "exported_at": 0, "project": null,
        "projects": [], "epics": [], "topics": [], "entries": [], "notes": [],
        "knowledge": [], "folders": [], "blobs": blobs,
    });
    let file: ExportFile = serde_json::from_value(json).unwrap();
    let text_len = file.to_json().unwrap().len();
    assert!(text_len > 32 * MIB);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.acta-backup");
    let pass = Passphrase::new("Alexandre's long passphrase".to_owned());

    let (written, peak) = peak_during(|| file.save_backup(&path, &pass));
    written.unwrap();
    assert!(
        peak < 4 * MIB,
        "writing held {} MiB beyond the records",
        peak / MIB
    );
    assert!(std::fs::metadata(&path).unwrap().len() as usize > text_len);

    let (back, peak) = peak_during(|| ExportFile::read_backup(&path, &pass));
    let back = back.unwrap();
    // The records themselves (32 MiB of strings) and small buffers. A
    // reader holding the whole decrypted text next to them would need
    // about twice the payload.
    assert!(
        peak < text_len + 8 * MIB,
        "reading peaked at {} MiB for a {} MiB payload",
        peak / MIB,
        text_len / MIB
    );
    assert_eq!(back, file);
}
