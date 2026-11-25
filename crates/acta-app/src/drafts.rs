//! Writing drafts to the encrypted database (data.md request 15,
//! interaction.md 6.6), for every editor that keeps one.
//!
//! Two rules keep a draft from coming back after it was discarded or from
//! being overwritten by another view of the same record:
//! - every write of a key takes a new generation; a write applies only if
//!   no later write (or clear) of that key was asked for, checked while the
//!   store is held, so a delayed write never lands after a clear;
//! - one editor owns a key: the one that last had the keyboard. Another
//!   view of the same record (the page open in two groups) does not write
//!   it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use gpui_kit::{App, Task};

use crate::db::Db;

/// How long after the last change a draft is written.
pub const DELAY: Duration = Duration::from_millis(400);

#[derive(Default)]
struct Keys {
    generation: HashMap<String, u64>,
    owner: HashMap<String, u64>,
}

static KEYS: Mutex<Option<Keys>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Keys) -> R) -> R {
    let mut guard = KEYS.lock().unwrap_or_else(|p| p.into_inner());
    f(guard.get_or_insert_with(Keys::default))
}

/// The editor `writer` (an entity id) has the keyboard: it owns `key`.
pub fn claim(key: &str, writer: u64) {
    with(|k| {
        k.owner.insert(key.to_owned(), writer);
    });
}

/// Whether `writer` may write `key`: it owns it, or nobody does yet (and
/// then it does from now on).
pub fn may_write(key: &str, writer: u64) -> bool {
    with(|k| *k.owner.entry(key.to_owned()).or_insert(writer) == writer)
}

/// The editor went away without its draft (saved, cancelled): the key is
/// free again.
pub fn release(key: &str, writer: u64) {
    with(|k| {
        if k.owner.get(key) == Some(&writer) {
            k.owner.remove(key);
        }
    });
}

/// A new generation of `key`.
fn next(key: &str) -> u64 {
    with(|k| {
        let g = k.generation.entry(key.to_owned()).or_insert(0);
        *g += 1;
        *g
    })
}

/// Apply `value` to `key` (`None` removes it) if `generation` is still the
/// newest asked for. The check and the write happen under the store's lock
/// and the keys' lock, so a newer call cannot slip between them.
fn apply(db: &Db, key: &str, generation: u64, value: Option<&str>) -> bool {
    let mut store = db.lock();
    let mut guard = KEYS.lock().unwrap_or_else(|p| p.into_inner());
    let keys = guard.get_or_insert_with(Keys::default);
    if keys.generation.get(key) != Some(&generation) {
        return false;
    }
    if let Err(err) = store.set_local_value(key, value) {
        crate::log::line(&format!(
            "draft: write failed: {}",
            crate::workspace::err_kind(&err)
        ));
    }
    true
}

/// Write `value` to `key` after [`DELAY`] (a later call for the key wins),
/// on a background thread.
pub fn write_later(db: &Db, key: String, value: Option<String>, cx: &App) -> Task<()> {
    let generation = next(&key);
    let db = db.clone();
    let timer = cx.background_executor().timer(DELAY);
    cx.background_executor().spawn(async move {
        timer.await;
        apply(&db, &key, generation, value.as_deref());
    })
}

/// Write `value` to `key` now (a pending write of it is dropped).
pub fn write_now(db: &Db, key: String, value: Option<String>, cx: &App) -> Task<()> {
    let generation = next(&key);
    let db = db.clone();
    cx.background_executor().spawn(async move {
        apply(&db, &key, generation, value.as_deref());
    })
}

/// Remove `key` now (a pending write of it is dropped).
pub fn clear(db: &Db, key: String, cx: &App) -> Task<()> {
    let generation = next(&key);
    let db = db.clone();
    cx.background_executor().spawn(async move {
        apply(&db, &key, generation, None);
    })
}

/// An editor changed: keep `value` under `key` after [`DELAY`], if
/// `writer` owns the key (`None`: the text is the stored one, no draft).
pub fn changed(db: &Db, key: String, writer: u64, value: Option<String>, cx: &App) {
    if may_write(&key, writer) {
        write_later(db, key, value, cx).detach();
    }
}

/// The editor saved or was cancelled: its draft goes now, and the key is
/// free again. Another view's editor of the same record keeps its own.
pub fn discard(db: &Db, key: String, writer: u64, cx: &App) {
    if may_write(&key, writer) {
        release(&key, writer);
        clear(db, key, cx).detach();
    }
}

/// The values kept under `prefix` for a record, or none when the record
/// is gone for good (`exists` false): they are dropped then.
pub fn read_kept(
    store: &mut acta_core::Store,
    prefix: &str,
    exists: bool,
) -> acta_core::Result<Vec<(String, String)>> {
    let kept = store.local_values(prefix)?;
    if exists {
        return Ok(kept);
    }
    for (key, _) in kept {
        store.set_local_value(&key, None)?;
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_write_never_lands_after_a_newer_clear() {
        let crate::db::test_support::TempStore {
            store,
            keys: _keys,
            dir: _dir,
        } = crate::db::test_support::temp_store();
        let db = Db::new(store);
        let key = "draft.T-1.state";
        let old = next(key);
        let newer = next(key);
        // The newer call (a clear) applies; the older write, when its
        // timer fires later, is dropped.
        assert!(apply(&db, key, newer, None));
        assert!(!apply(&db, key, old, Some("Alexandre's old text")));
        assert_eq!(db.lock().local_value(key).unwrap(), None);
    }

    #[test]
    fn one_editor_owns_a_key_the_last_focused() {
        let key = "draft.T-2.description";
        assert!(may_write(key, 1));
        assert!(!may_write(key, 2));
        claim(key, 2);
        assert!(may_write(key, 2));
        assert!(!may_write(key, 1));
        release(key, 1);
        assert!(!may_write(key, 1), "only the owner releases");
        release(key, 2);
        assert!(may_write(key, 1));
    }

    #[test]
    fn the_drafts_of_a_record_gone_for_good_are_dropped_when_read() {
        let crate::db::test_support::TempStore {
            mut store,
            keys: _keys,
            dir: _dir,
        } = crate::db::test_support::temp_store();
        store
            .set_local_value("draft.T-9.state", Some("Alexandre's text"))
            .unwrap();
        assert_eq!(read_kept(&mut store, "draft.T-9.", true).unwrap().len(), 1);
        assert!(
            read_kept(&mut store, "draft.T-9.", false)
                .unwrap()
                .is_empty()
        );
        assert!(store.local_values("draft.T-9.").unwrap().is_empty());
    }
}
