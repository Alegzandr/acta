//! Live working folders (ADR 0008, "Live files"): whether each attached
//! folder is on this computer, and following one that was renamed or moved
//! when its identity proves it is the same folder.
//!
//! Nothing here keeps a handle on a user folder. A directory handle held
//! open on Windows, even with `FILE_SHARE_DELETE` (which is how
//! `ReadDirectoryChangesW` watchers hold the folders they watch), stops the
//! user from renaming or deleting every folder above it. So presence is
//! polled with `GetFileAttributesW`, which opens nothing the caller holds,
//! and an identity is read through a handle that shares everything and is
//! closed before the call returns.
//!
//! The identity is the volume and the file id (Windows: `FILE_ID_INFO`;
//! Unix: `st_dev` and `st_ino`) plus the creation time where the file
//! system keeps one, which guards against an id reused by a newer folder.
//! A move to another volume changes the identity, so it is never followed:
//! the folder then reads as not found, with Locate.

use crate::folder_path;
use crate::model::FolderId;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// How deep the search for a moved folder goes below each place it looks.
const SEARCH_DEPTH: usize = 3;
/// Directories the search may list, in all: a missing folder near a huge
/// tree costs a bounded pass, then reads as not found.
const SEARCH_BUDGET: usize = 4_000;
/// Directories the search never goes into (it still matches them).
const SKIPPED: [&str; 3] = ["node_modules", "target", ".git"];

/// What makes a folder the same folder after a rename or a move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    volume: u64,
    file: u128,
    /// Creation time in nanoseconds since the Unix epoch, when known.
    born: Option<i64>,
}

impl Identity {
    /// The stored form: `v1:<volume>:<file>:<born or ->`, hex and decimal.
    pub fn encode(&self) -> String {
        let born = self.born.map_or_else(|| "-".to_owned(), |b| b.to_string());
        format!("v1:{:x}:{:x}:{born}", self.volume, self.file)
    }

    pub fn decode(text: &str) -> Option<Identity> {
        let mut parts = text.split(':');
        if parts.next()? != "v1" {
            return None;
        }
        let volume = u64::from_str_radix(parts.next()?, 16).ok()?;
        let file = u128::from_str_radix(parts.next()?, 16).ok()?;
        let born = match parts.next()? {
            "-" => None,
            b => Some(b.parse().ok()?),
        };
        parts
            .next()
            .is_none()
            .then_some(Identity { volume, file, born })
    }

    /// Same volume, same file id, and the same creation time when both
    /// sides know it.
    pub fn same(&self, other: &Identity) -> bool {
        self.volume == other.volume
            && self.file == other.file
            && match (self.born, other.born) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

fn born_of(meta: &std::fs::Metadata) -> Option<i64> {
    let since = meta.created().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(since.as_nanos()).ok()
}

/// The identity of the folder at `path`, or `None` when it is not a folder
/// or its file system gives no stable id. The handle is closed on return.
#[cfg(windows)]
pub fn identity(path: &Path) -> Option<Identity> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FileIdInfo, GetFileInformationByHandleEx,
    };
    let file = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        // Needed to open a directory; it asks for no privilege.
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_dir() {
        return None;
    }
    let mut info = FILE_ID_INFO::default();
    // SAFETY: the handle is open for the duration of the call, and the
    // buffer is a FILE_ID_INFO of the size passed.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&raw mut info).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    drop(file);
    let id = u128::from_le_bytes(info.FileId.Identifier);
    (ok != 0 && id != 0).then(|| Identity {
        volume: info.VolumeSerialNumber,
        file: id,
        born: born_of(&meta),
    })
}

/// The identity of the folder at `path` (`st_dev`, `st_ino`).
#[cfg(unix)]
pub fn identity(path: &Path) -> Option<Identity> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).ok()?;
    meta.is_dir().then(|| Identity {
        volume: meta.dev(),
        file: u128::from(meta.ino()),
        born: born_of(&meta),
    })
}

/// Whether `path` is a folder on this computer, without opening it.
#[cfg(windows)]
pub fn is_dir(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, GetFileAttributesW, INVALID_FILE_ATTRIBUTES,
    };
    let text = path.as_os_str();
    // Long paths need the verbatim prefix; stored paths are absolute and
    // canonical, which the prefix requires.
    let long = text.len() >= 240 && !text.to_string_lossy().starts_with(r"\\");
    let mut wide: Vec<u16> = Vec::with_capacity(text.len() + 5);
    if long && path.is_absolute() {
        wide.extend(r"\\?\".encode_utf16());
    }
    wide.extend(text.encode_wide());
    wide.push(0);
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
    let attributes = unsafe { GetFileAttributesW(wide.as_ptr()) };
    attributes != INVALID_FILE_ATTRIBUTES && attributes & FILE_ATTRIBUTE_DIRECTORY != 0
}

/// Whether `path` is a folder on this computer (`stat` holds nothing).
#[cfg(not(windows))]
pub fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

/// Where the folder that was at `old` is now: searched below the nearest
/// folder above `old` that still exists, then below its parent, a few
/// levels deep, in a bounded number of directories. `None` when no folder
/// or more than one has the identity.
pub fn find_moved(old: &Path, id: &Identity) -> Option<PathBuf> {
    let mut base = old.parent()?;
    while !is_dir(base) {
        base = base.parent()?;
    }
    let mut budget = SEARCH_BUDGET;
    let mut found = Vec::new();
    search(base, None, id, &mut budget, &mut found);
    if found.is_empty()
        && let Some(above) = base.parent()
    {
        search(above, Some(base), id, &mut budget, &mut found);
    }
    match found.len() {
        1 => found.pop(),
        _ => None,
    }
}

fn search(
    root: &Path,
    skip: Option<&Path>,
    id: &Identity,
    budget: &mut usize,
    found: &mut Vec<PathBuf>,
) {
    let mut queue = VecDeque::from([(root.to_path_buf(), 0)]);
    while let Some((dir, depth)) = queue.pop_front() {
        if *budget == 0 || found.len() > 1 {
            return;
        }
        *budget -= 1;
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            // Junctions and symbolic links are not followed: the stored
            // path is the resolved one.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if !kind.is_dir() || kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if skip == Some(path.as_path()) {
                continue;
            }
            if could_be(&entry, id) && identity(&path).is_some_and(|i| i.same(id)) {
                found.push(path.clone());
            }
            let name = entry.file_name();
            if depth + 1 < SEARCH_DEPTH && !SKIPPED.iter().any(|s| name == *s) {
                queue.push_back((path, depth + 1));
            }
        }
    }
}

/// A cheap first test from what listing the directory already gave:
/// the inode on Unix, the creation time on Windows (from the find data, so
/// no handle is opened for a folder that cannot match).
fn could_be(entry: &std::fs::DirEntry, id: &Identity) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirEntryExt;
        u128::from(entry.ino()) == id.file
    }
    #[cfg(not(unix))]
    {
        match (id.born, entry.metadata().ok().as_ref().and_then(born_of)) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
}

/// A live folder as the monitor sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub id: FolderId,
    /// The stored form (ADR 0008).
    pub path: String,
    /// The stored identity ([`Identity::encode`]), when known.
    pub identity: Option<String>,
}

/// A folder found under another path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Followed {
    pub id: FolderId,
    pub old: String,
    /// The new path, in stored form.
    pub new: String,
    pub identity: String,
}

/// What one pass found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pass {
    /// A folder appeared or disappeared since the previous pass: what shows
    /// presence must be checked again.
    pub changed: bool,
    /// Identities to remember, for folders that had none (or a stale one).
    pub learned: Vec<(FolderId, String)>,
    pub followed: Vec<Followed>,
}

impl Pass {
    /// Whether the store has anything to write.
    pub fn has_writes(&self) -> bool {
        !self.learned.is_empty() || !self.followed.is_empty()
    }
}

/// Polls the attached folders. Blocking: run it off the UI thread and never
/// while holding the store; the store applies the result
/// ([`crate::Store::apply_folder_pass`]).
#[derive(Debug, Default)]
pub struct Monitor {
    present: HashMap<FolderId, bool>,
    /// Missing folders already searched for at this path: not searched
    /// again on every tick, only on a thorough pass.
    searched: HashSet<(FolderId, String)>,
}

impl Monitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// One pass over `folders`. A quick pass only asks whether each is a
    /// folder, and reads identities only for folders that have none or
    /// that just went missing. A `thorough` pass (window focus) also reads
    /// every present folder's identity again and searches again for every
    /// missing one.
    pub fn pass(&mut self, folders: &[Known], thorough: bool) -> Pass {
        let mut out = Pass::default();
        let mut seen = HashMap::with_capacity(folders.len());
        for folder in folders {
            let path = Path::new(&folder.path);
            let present = is_dir(path);
            if let Some(before) = self.present.get(&folder.id)
                && *before != present
            {
                out.changed = true;
            }
            seen.insert(folder.id, present);
            if present {
                self.searched.retain(|(id, _)| *id != folder.id);
                if (folder.identity.is_none() || thorough)
                    && let Some(now) = identity(path).map(|i| i.encode())
                    && folder.identity.as_deref() != Some(now.as_str())
                {
                    out.learned.push((folder.id, now));
                }
                continue;
            }
            let Some(known) = folder.identity.as_deref().and_then(Identity::decode) else {
                continue;
            };
            let key = (folder.id, folder.path.clone());
            if !thorough && self.searched.contains(&key) {
                continue;
            }
            match find_moved(path, &known).and_then(|p| folder_path::canonical(&p).ok()) {
                Some(new) => out.followed.push(Followed {
                    id: folder.id,
                    old: folder.path.clone(),
                    new,
                    identity: known.encode(),
                }),
                None => {
                    self.searched.insert(key);
                }
            }
        }
        if seen.len() != self.present.len() && !self.present.is_empty() {
            out.changed = true;
        }
        self.present = seen;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn known(id: i64, path: &Path) -> Known {
        let stored = folder_path::canonical(path).unwrap();
        let identity = identity(Path::new(&stored)).map(|i| i.encode());
        Known {
            id: FolderId(id),
            path: stored,
            identity,
        }
    }

    fn stored(path: &Path) -> String {
        folder_path::canonical(path).unwrap()
    }

    #[test]
    fn identity_round_trips_and_survives_a_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("acta");
        fs::create_dir(&a).unwrap();
        let before = identity(&a).expect("a folder on a local disk has an identity");
        assert_eq!(Identity::decode(&before.encode()), Some(before));
        assert_eq!(Identity::decode("v2:1:2:-"), None);
        assert_eq!(Identity::decode("v1:1:2:-:x"), None);
        fs::rename(&a, tmp.path().join("acta-old")).unwrap();
        let after = identity(&tmp.path().join("acta-old")).unwrap();
        assert!(before.same(&after));
        let other = tmp.path().join("other");
        fs::create_dir(&other).unwrap();
        assert!(!before.same(&identity(&other).unwrap()));
        assert_eq!(identity(&tmp.path().join("nothing")), None);
    }

    #[test]
    fn a_birth_time_known_on_one_side_only_does_not_decide() {
        let a = Identity {
            volume: 1,
            file: 2,
            born: Some(5),
        };
        let unknown = Identity { born: None, ..a };
        let newer = Identity { born: Some(6), ..a };
        assert!(a.same(&unknown));
        assert!(!a.same(&newer));
    }

    #[test]
    fn presence_is_read_without_holding_anything() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("acta");
        fs::create_dir(&a).unwrap();
        fs::write(tmp.path().join("file"), b"x").unwrap();
        assert!(is_dir(&a));
        assert!(!is_dir(&tmp.path().join("file")));
        assert!(!is_dir(&tmp.path().join("nothing")));
    }

    #[test]
    fn a_renamed_folder_is_followed_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("acta");
        fs::create_dir(&a).unwrap();
        let folders = [known(1, &a)];
        let mut monitor = Monitor::new();
        assert_eq!(monitor.pass(&folders, false), Pass::default());

        let renamed = tmp.path().join("acta-renamed");
        fs::rename(&a, &renamed).unwrap();
        let pass = monitor.pass(&folders, false);
        assert!(pass.changed);
        assert_eq!(pass.followed.len(), 1);
        assert_eq!(pass.followed[0].old, folders[0].path);
        assert_eq!(pass.followed[0].new, stored(&renamed));
    }

    #[test]
    fn a_folder_moved_into_a_sibling_out_of_its_parent_or_with_its_parent_is_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let work = root.join("work");
        let a = work.join("acta");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(work.join("archive")).unwrap();
        let id = identity(&a).unwrap();

        // Into a sibling.
        let into = work.join("archive").join("acta");
        fs::rename(&a, &into).unwrap();
        assert_eq!(find_moved(&a, &id), Some(into.clone()));

        // Out of its parent: found below the parent's parent.
        let deep = work.join("acta");
        fs::rename(&into, &deep).unwrap();
        assert_eq!(find_moved(&into, &id), Some(deep.clone()));

        // With its parent renamed: the old parent is gone.
        let jobs = root.join("jobs");
        fs::rename(&work, &jobs).unwrap();
        assert_eq!(find_moved(&deep, &id), Some(jobs.join("acta")));
    }

    #[test]
    fn without_a_proven_identity_a_missing_folder_is_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("acta");
        let b = tmp.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let with_identity = known(1, &a);
        let mut without = known(2, &b);
        without.identity = None;
        let mut monitor = Monitor::new();
        monitor.pass(&[with_identity.clone(), without.clone()], false);

        // Deleted, while an unrelated folder stays next to it.
        fs::remove_dir(&a).unwrap();
        fs::create_dir(tmp.path().join("c")).unwrap();
        // Renamed, but nothing proves where it went.
        fs::rename(&b, tmp.path().join("b2")).unwrap();
        let pass = monitor.pass(&[with_identity.clone(), without.clone()], false);
        assert!(pass.changed);
        assert!(pass.followed.is_empty(), "{pass:?}");
        assert!(pass.learned.is_empty());

        // The failed search is not repeated on every tick, only when thorough.
        assert!(
            monitor
                .searched
                .contains(&(FolderId(1), with_identity.path.clone()))
        );
        let again = monitor.pass(&[with_identity, without], true);
        assert!(again.followed.is_empty());
        assert!(!again.changed);
    }

    #[test]
    fn identities_are_learned_once_and_refreshed_when_thorough() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("acta");
        fs::create_dir(&a).unwrap();
        let mut folder = known(1, &a);
        let real = folder.identity.take().unwrap();
        let mut monitor = Monitor::new();
        let pass = monitor.pass(std::slice::from_ref(&folder), false);
        assert_eq!(pass.learned, [(FolderId(1), real.clone())]);

        folder.identity = Some(real.clone());
        assert!(
            !monitor
                .pass(std::slice::from_ref(&folder), true)
                .has_writes()
        );

        // Deleted and made again under the same name: the path is what the
        // user attached, so it stays, with the new folder's identity.
        fs::remove_dir(&a).unwrap();
        fs::create_dir(tmp.path().join("spacer")).unwrap();
        fs::create_dir(&a).unwrap();
        let pass = monitor.pass(std::slice::from_ref(&folder), true);
        assert!(pass.followed.is_empty());
        if identity(&a).unwrap().encode() != real {
            assert_eq!(pass.learned.len(), 1);
        }
    }

    #[test]
    fn nothing_is_held_between_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path().join("work");
        let a = parent.join("acta");
        fs::create_dir_all(&a).unwrap();
        let mut folder = known(1, &a);
        folder.identity = None;
        let mut monitor = Monitor::new();
        // Reads the identity (a handle) and presence, then returns.
        assert_eq!(
            monitor
                .pass(std::slice::from_ref(&folder), true)
                .learned
                .len(),
            1
        );
        fs::rename(&parent, tmp.path().join("work2")).expect("no handle left open");
        fs::rename(
            tmp.path().join("work2").join("acta"),
            tmp.path().join("acta"),
        )
        .expect("no handle left open");
    }

    /// The user can rename the watched folder and the folder above it while
    /// the monitor polls (ADR 0008, "Live files"). A `ReadDirectoryChangesW`
    /// watcher fails the second rename on Windows.
    #[test]
    fn polling_never_stops_a_rename_of_the_folder_or_its_parent() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path().join("work");
        let a = parent.join("acta");
        fs::create_dir_all(&a).unwrap();
        let folders = vec![known(1, &a)];
        let stop = Arc::new(AtomicBool::new(false));
        let polling = {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut monitor = Monitor::new();
                let mut passes = 0;
                while !stop.load(Ordering::Relaxed) {
                    monitor.pass(&folders, false);
                    passes += 1;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                passes
            })
        };
        // A search for the folder just renamed lists the directory above
        // it for a moment, as Explorer does; a rename that lands in that
        // moment is tried once more, as Explorer offers. Nothing is held
        // longer.
        let rename = |from: &Path, to: &Path| {
            fs::rename(from, to)
                .or_else(|_| {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    fs::rename(from, to)
                })
                .expect("rename while the monitor polls")
        };
        let (mut here, mut up) = (a.clone(), parent.clone());
        for i in 0..20 {
            let next = up.join(format!("acta-{i}"));
            rename(&here, &next);
            let next_up = tmp.path().join(format!("work-{i}"));
            rename(&up, &next_up);
            here = next_up.join(next.file_name().unwrap());
            up = next_up;
        }
        stop.store(true, Ordering::Relaxed);
        assert!(polling.join().unwrap() > 0);
        assert!(here.is_dir());
    }
}
