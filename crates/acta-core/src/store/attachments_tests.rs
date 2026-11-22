//! Attachments in the store (ADR 0012): limits, dedup, trash and purge,
//! filing and its undo, export and import, and the migration that adds
//! them.

use super::test_support::{self, TestStore};
use super::*;
use crate::model::{AttachOwner, Attachment, FileKind, NoteId, PageReq, Record, Status};
use std::io::Read;

const H: Author = Author::Human;

fn setup() -> (TestStore, ProjectId, TopicId) {
    let mut ts = test_support::open();
    let p = ts.store.create_project("Acta", "", H).unwrap();
    let t = ts
        .store
        .create_topic(
            NewTopic {
                project: p,
                epic: None,
                title: "Storage".into(),
                description: String::new(),
                status: Status::Active,
            },
            H,
        )
        .unwrap();
    (ts, p, t)
}

fn bytes(seed: u8, n: usize) -> Vec<u8> {
    // Not text: a NUL every 97 bytes.
    (0..n)
        .map(|i| {
            if i % 97 == 0 {
                0
            } else {
                (i as u8).wrapping_mul(seed) | 1
            }
        })
        .collect()
}

fn add(s: &mut Store, owner: AttachOwner, name: &str, data: Vec<u8>) -> Attachment {
    s.attach(owner, name, AttachSource::Bytes(data), H, &mut |_, _| {})
        .unwrap()
}

fn content(s: &Store, a: &Attachment) -> Vec<u8> {
    let mut out = Vec::new();
    s.read_attachment(a.id)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn count(s: &Store, sql: &str) -> i64 {
    s.conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([x as u8, y as u8, 7]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn files_round_trip_with_kind_name_and_thumbnail() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let n = s
        .add_note(p, "Screenshot of the bug, from Alexandre", H)
        .unwrap();
    let data = bytes(3, 3 * CHUNK_BYTES + 17);
    let mut calls = Vec::new();
    let a = s
        .attach(
            AttachOwner::Note(n),
            "dump.bin",
            AttachSource::Bytes(data.clone()),
            H,
            &mut |done, total| calls.push((done, total)),
        )
        .unwrap();
    assert_eq!(a.size, data.len() as i64);
    assert_eq!(a.kind, FileKind::Other);
    assert_eq!(content(s, &a), data);
    assert_eq!(calls.len(), 5, "four chunks and the end: {calls:?}");
    assert_eq!(calls.last(), Some(&(data.len() as u64, data.len() as u64)));
    let img = add(s, AttachOwner::Note(n), "shot.png", png(600, 300));
    assert_eq!(img.kind, FileKind::Image);
    assert_eq!(img.pixels, Some((600, 300)));
    let thumb = s.attachment_thumbnail(img.id).unwrap().unwrap();
    let t = crate::files::decode(&thumb).unwrap();
    assert_eq!((t.width(), t.height()), (256, 128));
    let txt = add(
        s,
        AttachOwner::Note(n),
        "notes.md",
        b"# Plan\nAlexandre".to_vec(),
    );
    assert_eq!(txt.kind, FileKind::Text);
    let list = s.attachments(AttachOwner::Note(n)).unwrap();
    assert_eq!(
        list.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["dump.bin", "shot.png", "notes.md"]
    );
    assert_eq!(s.attachment_prefix(txt.id, 6).unwrap(), b"# Plan");
    // Names are cleaned: no control characters, never empty, 255 at most
    // with the extension kept.
    let odd = add(s, AttachOwner::Note(n), "a\nb\u{7}.txt", b"x".to_vec());
    assert_eq!(odd.name, "a b .txt");
    let long = format!("{}.pdf", "x".repeat(400));
    assert_eq!(clean_name(&long).chars().count(), NAME_MAX);
    assert!(clean_name(&long).ends_with("….pdf"));
    assert_eq!(clean_name("  "), "file");
}

#[test]
fn a_file_from_disk_is_read_twice_and_refused_when_it_changes_or_is_too_big() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "From disk", H).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.csv");
    std::fs::write(&path, "a,b\n1,2\n").unwrap();
    let a = s
        .attach(
            AttachOwner::Note(n),
            "report.csv",
            AttachSource::Path(path.clone()),
            H,
            &mut |_, _| {},
        )
        .unwrap();
    assert_eq!(content(s, &a), b"a,b\n1,2\n");
    assert_eq!(a.kind, FileKind::Text);
    // Over 25 MB: refused before anything is read or written.
    let big = dir.path().join("big.bin");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(MAX_FILE_BYTES + 1).unwrap();
    drop(f);
    let e = s
        .attach(
            AttachOwner::Note(n),
            "big.bin",
            AttachSource::Path(big),
            H,
            &mut |_, _| {},
        )
        .unwrap_err();
    assert!(
        matches!(e, Error::AttachmentTooLarge { size, limit } if size == MAX_FILE_BYTES + 1 && limit == MAX_FILE_BYTES)
    );
    // A missing file is an I/O error (the editor's "Could not read").
    let e = s
        .attach(
            AttachOwner::Note(n),
            "gone.txt",
            AttachSource::Path(dir.path().join("gone.txt")),
            H,
            &mut |_, _| {},
        )
        .unwrap_err();
    assert!(matches!(e, Error::Io(_)));
    assert_eq!(
        count(
            s,
            "SELECT count(*) FROM attachment_blobs WHERE complete = 0"
        ),
        0
    );
}

#[test]
fn twenty_files_is_the_most_an_owner_holds() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Many", H).unwrap();
    for i in 0..MAX_FILES_PER_OWNER {
        add(
            s,
            AttachOwner::Note(n),
            &format!("f{i}.txt"),
            format!("{i}").into_bytes(),
        );
    }
    let e = s
        .attach(
            AttachOwner::Note(n),
            "one-more.txt",
            AttachSource::Bytes(b"21".to_vec()),
            H,
            &mut |_, _| {},
        )
        .unwrap_err();
    assert!(matches!(e, Error::TooManyFiles { limit: 20 }));
    // A removed file frees its place.
    let first = s.attachments(AttachOwner::Note(n)).unwrap()[0].id;
    s.delete(Record::Attachment(first), H).unwrap();
    add(s, AttachOwner::Note(n), "one-more.txt", b"21".to_vec());
}

#[test]
fn the_same_bytes_under_two_names_share_one_blob() {
    let (mut ts, p, t) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Twice", H).unwrap();
    let data = bytes(5, CHUNK_BYTES + 10);
    let a = add(s, AttachOwner::Note(n), "IMG_2041.bin", data.clone());
    let mut wrote = 0;
    let b = s
        .attach(
            AttachOwner::Topic(t),
            "copy of IMG_2041.bin",
            AttachSource::Bytes(data.clone()),
            H,
            &mut |done, total| {
                assert_eq!(done, total, "a duplicate writes no chunk");
                wrote += 1;
            },
        )
        .unwrap();
    assert_eq!(wrote, 1);
    assert_ne!(a.id, b.id);
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 1);
    assert_eq!(count(s, "SELECT count(*) FROM attachment_chunks"), 2);
    assert_eq!(content(s, &b), data);
    let totals = s.attachment_totals(None).unwrap();
    assert_eq!(totals.bytes, data.len() as i64);
    assert_eq!(totals.files, 2);
}

fn freelist(s: &Store) -> i64 {
    count(s, "PRAGMA freelist_count")
}

#[test]
fn purge_deletes_expired_rows_then_blobs_no_row_names_and_returns_the_pages() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    assert_eq!(crate::db::auto_vacuum_mode(&s.conn).unwrap(), 2);
    let n = s.add_note(p, "Purge", H).unwrap();
    let m = s.add_note(p, "Keeps a copy", H).unwrap();
    let shared = bytes(7, 2 * CHUNK_BYTES);
    let only = bytes(9, 2 * CHUNK_BYTES);
    let a = add(s, AttachOwner::Note(n), "shared.bin", shared.clone());
    let b = add(s, AttachOwner::Note(m), "shared-copy.bin", shared);
    let c = add(s, AttachOwner::Note(n), "only.bin", only);
    let live = add(s, AttachOwner::Note(m), "live.txt", b"live".to_vec());
    s.delete(Record::Attachment(a.id), H).unwrap();
    s.delete(Record::Attachment(c.id), H).unwrap();
    s.delete(Record::Attachment(b.id), H).unwrap();
    let now = crate::time::now_ms();
    // Nothing is over 30 days yet.
    assert_eq!(s.purge_attachments(now).unwrap(), Purged::default());
    // Blobs stored now are within the grace; these were stored long ago.
    s.conn
        .execute("UPDATE attachment_blobs SET created_at = 0", [])
        .unwrap();
    // `a` and `c` expire; `b` (same content as `a`) is still in the trash.
    let old = now - TRASH_RETENTION_MS - 1;
    s.conn
        .execute(
            "UPDATE attachments SET deleted_at = ?1 WHERE id IN (?2, ?3)",
            rusqlite::params![old, a.id.0, c.id.0],
        )
        .unwrap();
    let pages_before = count(s, "PRAGMA page_count");
    let purged = s.purge_attachments(now).unwrap();
    assert_eq!(purged.rows, 2);
    assert_eq!(purged.blobs, 1, "the shared blob is still named by b");
    assert_eq!(purged.bytes, 2 * CHUNK_BYTES as i64);
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 2);
    assert_eq!(freelist(s), 0, "incremental_vacuum returned the free pages");
    assert!(count(s, "PRAGMA page_count") < pages_before);
    // b expires: its blob goes; the live file stays untouched.
    s.conn
        .execute(
            "UPDATE attachments SET deleted_at = ?1 WHERE id = ?2",
            rusqlite::params![old, b.id.0],
        )
        .unwrap();
    let purged = s.purge_attachments(now).unwrap();
    assert_eq!((purged.rows, purged.blobs), (1, 1));
    assert_eq!(content(s, &live), b"live");
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 1);
    // A half-written blob (a crash) goes after an hour, not before.
    s.conn
        .execute(
            "INSERT INTO attachment_blobs (hash, size, media_type, complete, created_at)
             VALUES ('pending:x', 3, 'text/plain', 0, ?1)",
            [now],
        )
        .unwrap();
    assert_eq!(s.purge_attachments(now).unwrap().blobs, 0);
    assert_eq!(s.purge_attachments(now + 2 * 3_600_000).unwrap().blobs, 1);
}

#[test]
fn files_follow_their_owner_to_the_trash_and_back() {
    let (mut ts, p, t) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Note with files", H).unwrap();
    let a = add(s, AttachOwner::Note(n), "a.txt", b"a".to_vec());
    let b = add(s, AttachOwner::Note(n), "b.txt", b"b".to_vec());
    s.delete(Record::Note(n), H).unwrap();
    assert!(s.attachment(a.id).is_err());
    let trash = s.trash(PageReq::first(20)).unwrap();
    // The note is listed; its files went with it and are not listed alone.
    assert_eq!(trash.items.len(), 1);
    s.restore(Record::Note(n), H).unwrap();
    assert_eq!(s.attachments(AttachOwner::Note(n)).unwrap().len(), 2);
    // A file removed on its own is a trash row, and comes back at the end.
    s.delete(Record::Attachment(a.id), H).unwrap();
    let trash = s.trash(PageReq::first(20)).unwrap();
    assert_eq!(trash.items[0].record, Record::Attachment(a.id));
    assert_eq!(trash.items[0].label, "a.txt");
    s.restore(Record::Attachment(a.id), H).unwrap();
    let names: Vec<String> = s
        .attachments(AttachOwner::Note(n))
        .unwrap()
        .into_iter()
        .map(|x| x.name)
        .collect();
    assert_eq!(names, ["b.txt", "a.txt"]);
    // Entries' and descriptions' files go with their topic.
    let e = s
        .log(
            t,
            NewEntry {
                kind: crate::model::EntryKind::Progress,
                text: "Done".into(),
                state: None,
                status: None,
            },
            H,
        )
        .unwrap();
    add(s, AttachOwner::Entry(e.id), "log.txt", b"log".to_vec());
    add(s, AttachOwner::Topic(t), "spec.txt", b"spec".to_vec());
    s.delete(Record::Topic(t), H).unwrap();
    assert_eq!(
        count(
            s,
            "SELECT count(*) FROM attachments WHERE deleted_at IS NULL"
        ),
        2,
        "only the note's two files stay live"
    );
    s.restore(Record::Topic(t), H).unwrap();
    assert_eq!(s.attachments(AttachOwner::Entry(e.id)).unwrap().len(), 1);
    assert_eq!(s.attachments(AttachOwner::Topic(t)).unwrap().len(), 1);
    let _ = b;
}

#[test]
fn filing_moves_files_and_its_undo_moves_them_back() {
    let (mut ts, p, t) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Quick capture from the tray", H).unwrap();
    let a = add(s, AttachOwner::Note(n), "plan.pdf.bin", b"plan".to_vec());
    add(s, AttachOwner::Topic(t), "already.txt", b"there".to_vec());
    // Filed into a topic: the files go to its description, after its own.
    s.file_notes(&[n], t, H).unwrap();
    assert!(s.attachments(AttachOwner::Note(n)).unwrap().is_empty());
    let on_topic = s.attachments(AttachOwner::Topic(t)).unwrap();
    assert_eq!(on_topic.len(), 2);
    assert_eq!(on_topic[1].id, a.id);
    assert_eq!(on_topic[1].moved_from, Some(n));
    assert_eq!(s.attachments_moved_from(n).unwrap().len(), 1);
    // Undo: back to the note, nothing else moves.
    let before = s.note(n).unwrap();
    s.unfile_note(n, H).unwrap();
    assert_eq!(s.attachments(AttachOwner::Note(n)).unwrap()[0].id, a.id);
    assert_eq!(s.attachments(AttachOwner::Topic(t)).unwrap().len(), 1);
    // Redo: to the topic again.
    s.restore_note_fate(n, &before, H).unwrap();
    assert_eq!(s.attachments(AttachOwner::Topic(t)).unwrap().len(), 2);
    s.unfile_note(n, H).unwrap();

    // Filed as an entry: the files go to that entry, and come back.
    let e = s.note_to_entry(n, t, H).unwrap();
    assert_eq!(e.text, "Quick capture from the tray");
    assert_eq!(s.attachments(AttachOwner::Entry(e.id)).unwrap()[0].id, a.id);
    let before = s.note(n).unwrap();
    s.unfile_note(n, H).unwrap();
    assert_eq!(s.attachments(AttachOwner::Note(n)).unwrap().len(), 1);
    s.restore_note_fate(n, &before, H).unwrap();
    assert_eq!(
        s.attachments(AttachOwner::Entry(e.id)).unwrap().len(),
        1,
        "redo follows the entry"
    );
    s.unfile_note(n, H).unwrap();

    // Made into an epic: its description.
    let ep = s.create_epic(p, "Capture", "", H).unwrap();
    s.file_notes_in_epic(&[n], ep, H).unwrap();
    assert_eq!(s.attachments(AttachOwner::Epic(ep)).unwrap().len(), 1);
    s.unfile_note(n, H).unwrap();
    // A dismissed note keeps its files.
    s.dismiss_notes(&[n], H).unwrap();
    assert_eq!(s.attachments(AttachOwner::Note(n)).unwrap().len(), 1);
    // A file removed after filing is not brought back by the undo.
    s.unfile_note(n, H).unwrap();
    s.file_notes(&[n], t, H).unwrap();
    s.delete(Record::Attachment(a.id), H).unwrap();
    s.unfile_note(n, H).unwrap();
    assert!(s.attachments(AttachOwner::Note(n)).unwrap().is_empty());
}

fn fresh() -> TestStore {
    test_support::open()
}

#[test]
fn export_with_and_without_content_and_import_dedups_by_hash() {
    let (mut ts, p, t) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Files", H).unwrap();
    let data = bytes(11, CHUNK_BYTES + 2);
    let a = add(s, AttachOwner::Note(n), "a.bin", data.clone());
    add(s, AttachOwner::Topic(t), "a-again.bin", data.clone());
    let img = add(s, AttachOwner::Note(n), "shot.png", png(300, 200));
    s.delete(Record::Attachment(img.id), H).unwrap();

    let full = s.export(None).unwrap();
    assert_eq!(full.version, FORMAT_VERSION);
    assert_eq!(full.blobs.len(), 2, "one per content");
    assert_eq!(full.attachments.len(), 3, "the trashed one included");
    assert!(full.blobs.iter().all(|b| b.content.is_some()));
    let json = full.to_json().unwrap();
    let back = ExportFile::from_json(&json).unwrap();
    assert_eq!(back, full);

    let mut other = fresh();
    let rep = other.store.import(&back, false).unwrap();
    assert_eq!(rep.attachments.created, 3);
    assert!(rep.files_not_in_export.is_empty());
    let o = &other.store;
    let note = NoteId(
        o.conn
            .query_row("SELECT id FROM notes", [], |r| r.get(0))
            .unwrap(),
    );
    let got = o.attachments(AttachOwner::Note(note)).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(content(o, &got[0]), data);
    assert_eq!(count(o, "SELECT count(*) FROM attachment_blobs"), 2);
    // The image's thumbnail is made again on import.
    assert_eq!(
        count(
            o,
            "SELECT count(*) FROM attachment_blobs WHERE thumbnail IS NOT NULL"
        ),
        1
    );
    // Importing again changes nothing.
    let again = other.store.import(&back, false).unwrap();
    assert_eq!(again.attachments.created, 0);

    // Without content: names and sizes, no bytes.
    let light = s.export_with(None, false).unwrap();
    assert!(light.blobs.iter().all(|b| b.content.is_none()));
    assert_eq!(light.attachments[0].size, light.attachments[0].size.max(1));
    assert!(light.to_json().unwrap().len() < json.len() / 2);
    let mut empty = fresh();
    let rep = empty.store.import(&light, false).unwrap();
    assert_eq!(rep.attachments.created, 0);
    let mut missing = rep.files_not_in_export.clone();
    missing.sort();
    assert_eq!(missing, ["a-again.bin", "a.bin", "shot.png"]);
    // Into a database that holds the content, the rows link to it.
    let rep = other.store.import(&light, true).unwrap();
    assert!(rep.files_not_in_export.is_empty());

    // A tampered blob is refused.
    let mut bad = full.clone();
    bad.blobs[0].content = Some("AAAA".into());
    let mut target = fresh();
    assert!(target.store.import(&bad, false).is_err());
    let _ = a;
}

#[test]
fn an_older_export_imports_and_an_update_from_it_leaves_files_alone() {
    let (mut ts, p, t) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Old format note", H).unwrap();
    let e = s.note_to_entry(n, t, H).unwrap();
    let file = s.export(None).unwrap();
    // A version 3 file: no blobs, no attachments, no filed_entry.
    let mut v: serde_json::Value = serde_json::from_str(&file.to_json().unwrap()).unwrap();
    v["version"] = 3.into();
    v.as_object_mut().unwrap().remove("blobs");
    v.as_object_mut().unwrap().remove("attachments");
    for note in v["notes"].as_array_mut().unwrap() {
        note.as_object_mut().unwrap().remove("filed_entry");
        // Newer than the local copy, so the import updates the note.
        note["updated_at"] = (crate::time::now_ms() + 60_000).into();
        note["text"] = "Edited elsewhere".into();
    }
    let old = ExportFile::from_json(&v.to_string()).unwrap();
    assert!(old.attachments.is_empty() && old.blobs.is_empty());
    // Into an empty database: everything but files.
    let mut other = fresh();
    let rep = other.store.import(&old, false).unwrap();
    assert_eq!(rep.notes.created, 1);
    assert_eq!(count(&other.store, "SELECT count(*) FROM attachments"), 0);
    // Over this one, after files were added: the note updates, its filing
    // link to the entry and the entry's files stay.
    add(s, AttachOwner::Entry(e.id), "kept.txt", b"kept".to_vec());
    let rep = s.import(&old, false).unwrap();
    assert_eq!(rep.notes.updated, 1);
    assert_eq!(s.note(n).unwrap().text, "Edited elsewhere");
    assert_eq!(s.attachments(AttachOwner::Entry(e.id)).unwrap().len(), 1);
    let link: Option<i64> = s
        .conn
        .query_row(
            "SELECT filed_entry_id FROM notes WHERE id = ?1",
            [n.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(link, Some(e.id.0));
}

#[test]
fn the_migration_snapshot_restores_and_the_upgrade_sets_incremental_vacuum() {
    let dir = tempfile::tempdir().unwrap();
    let keys = crate::keys::MemoryKeyStore::new();
    let config = StoreConfig {
        data_dir: dir.path().to_path_buf(),
        keys: keys.stores(),
    };
    // A schema 5 database (0.5.0) with a project in it.
    let p = {
        let mut old = Store::open_with(&config, &schema::MIGRATIONS[..5]).unwrap();
        // 0.5.0 made databases without auto-vacuum.
        old.conn
            .execute_batch("PRAGMA auto_vacuum = NONE; VACUUM;")
            .unwrap();
        assert_eq!(crate::db::auto_vacuum_mode(&old.conn).unwrap(), 0);
        old.create_project("Alexandre's project", "", H).unwrap()
    };
    // This build: snapshot first, then the step, then one VACUUM.
    let mut s = Store::open(&config).unwrap();
    assert_eq!(
        crate::db::user_version(&s.conn).unwrap(),
        schema::SCHEMA_VERSION
    );
    // Opening (as a hook, `acta mcp` or the CLI do) never runs the VACUUM.
    assert_eq!(crate::db::auto_vacuum_mode(&s.conn).unwrap(), 0);
    // While another connection writes, the switch gives up at once.
    {
        let other = crate::db::connect(
            &s.db_path(),
            &s.key,
            crate::db::Mode::Live { create: false },
        )
        .unwrap();
        other.execute_batch("BEGIN IMMEDIATE;").unwrap();
        let started = std::time::Instant::now();
        assert!(!s.enable_incremental_vacuum().unwrap());
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
        other.execute_batch("COMMIT").unwrap();
    }
    assert!(s.enable_incremental_vacuum().unwrap());
    assert_eq!(crate::db::auto_vacuum_mode(&s.conn).unwrap(), 2);
    let snap = s
        .snapshots()
        .unwrap()
        .into_iter()
        .find(|x| x.kind == SnapshotKind::Migration)
        .expect("a migration snapshot");
    // Work done after the upgrade.
    let n = s.add_note(p, "After the upgrade", H).unwrap();
    add(&mut s, AttachOwner::Note(n), "new.txt", b"new".to_vec());
    // Restoring the snapshot brings back the schema 5 data, upgraded again.
    s.restore_snapshot(&snap.path).unwrap();
    assert_eq!(s.project(p).unwrap().title, "Alexandre's project");
    assert!(s.note(n).is_err(), "the note came after the snapshot");
    assert_eq!(
        crate::db::user_version(&s.conn).unwrap(),
        schema::SCHEMA_VERSION
    );
    // The app's daily maintenance switches it again.
    assert_eq!(crate::db::auto_vacuum_mode(&s.conn).unwrap(), 0);
    s.snapshot_if_due().unwrap();
    assert_eq!(crate::db::auto_vacuum_mode(&s.conn).unwrap(), 2);
    assert_eq!(count(&s, "SELECT count(*) FROM attachments"), 0);
    let n = s.add_note(p, "Files work after the restore", H).unwrap();
    add(&mut s, AttachOwner::Note(n), "again.txt", b"again".to_vec());
}

#[test]
fn a_purge_between_lookup_and_row_never_leaves_a_row_without_content() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Race", H).unwrap();
    let data = bytes(13, CHUNK_BYTES + 5);
    let a = add(s, AttachOwner::Note(n), "first.bin", data.clone());
    // The row goes (an expired trash row, deleted by a purge); its blob is
    // an orphan, young, so the purge keeps it for now.
    s.conn
        .execute("DELETE FROM attachments WHERE id = ?1", [a.id.0])
        .unwrap();
    let now = crate::time::now_ms();
    assert_eq!(
        s.purge_attachments(now).unwrap().blobs,
        0,
        "within the grace"
    );
    // Another process purges it between this attach's lookup and its row.
    let ran = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = ran.clone();
    super::attachments::BEFORE_ROW.with(|h| {
        *h.borrow_mut() = Some(Box::new(move |conn: &rusqlite::Connection| {
            if seen.get() == 0 {
                conn.execute_batch("DELETE FROM attachment_chunks; DELETE FROM attachment_blobs;")
                    .unwrap();
            }
            seen.set(seen.get() + 1);
        }));
    });
    let again = add(s, AttachOwner::Note(n), "again.bin", data.clone());
    super::attachments::BEFORE_ROW.with(|h| *h.borrow_mut() = None);
    assert_eq!(ran.get(), 2, "looked up, lost it, stored it again");
    assert_eq!(content(s, &again), data);
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 1);
    // An hour later an orphan goes.
    s.conn
        .execute("DELETE FROM attachments WHERE id = ?1", [again.id.0])
        .unwrap();
    assert_eq!(s.purge_attachments(now + 2 * 3_600_000).unwrap().blobs, 1);
}

#[test]
fn an_export_an_import_could_not_read_is_refused_unless_without_files() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Big", H).unwrap();
    add(s, AttachOwner::Note(n), "big.bin", bytes(3, 30_000));
    let e = s.export_within(None, true, 20_000).unwrap_err();
    assert!(
        matches!(
            e,
            Error::ExportTooLarge {
                size: 40_000,
                limit: 20_000
            }
        ),
        "{e}"
    );
    assert!(e.to_string().contains("--no-files"), "{e}");
    let light = s.export_within(None, false, 20_000).unwrap();
    assert_eq!(light.attachments.len(), 1);
    assert!(s.export_within(None, true, 1_000_000).is_ok());
}

#[test]
fn imported_blobs_over_the_limit_or_unlike_their_size_are_refused() {
    use base64::Engine as _;
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let n = s.add_note(p, "Blob checks", H).unwrap();
    add(s, AttachOwner::Note(n), "x.bin", bytes(5, 3_000));
    let good = s.export(None).unwrap();
    let import = |f: &ExportFile| fresh().store.import(f, false);
    assert!(import(&good).is_ok());
    // Declared size not the decoded length.
    let mut f = good.clone();
    f.blobs[0].size -= 100;
    assert!(
        import(&f)
            .unwrap_err()
            .to_string()
            .contains("larger than its size says")
    );
    let mut f = good.clone();
    f.blobs[0].size += 3;
    let e = import(&f).unwrap_err().to_string();
    assert!(e.contains("not the"), "{e}");
    // Over 25 MB: refused before decoding.
    let mut f = good.clone();
    f.blobs[0].size = MAX_FILE_BYTES as i64 + 1;
    f.blobs[0].content = Some(base64::engine::general_purpose::STANDARD.encode(vec![0u8; 3]));
    assert!(import(&f).unwrap_err().to_string().contains("limit"));
}

#[test]
fn a_staged_file_gets_its_row_when_the_note_is_saved_or_goes_when_discarded() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let data = bytes(5, 2 * CHUNK_BYTES + 3);
    let mut calls = Vec::new();
    let staged = s
        .stage(&AttachSource::Bytes(data.clone()), &mut |d, t| {
            calls.push((d, t))
        })
        .unwrap();
    assert_eq!(staged.size(), data.len() as u64);
    assert_eq!(staged.kind(), FileKind::Other);
    assert_eq!(calls.last(), Some(&(data.len() as u64, data.len() as u64)));
    // Pending: no row names it, the blob is not complete yet.
    assert_eq!(count(s, "SELECT count(*) FROM attachments"), 0);
    assert_eq!(
        count(
            s,
            "SELECT count(*) FROM attachment_blobs WHERE complete = 0"
        ),
        1
    );
    let n = s.add_note(p, "Logs from Alexandre", H).unwrap();
    let a = s
        .attach_staged(AttachOwner::Note(n), "trace.bin", &staged, H)
        .unwrap();
    assert_eq!(content(s, &a), data);
    assert_eq!(
        count(
            s,
            "SELECT count(*) FROM attachment_blobs WHERE complete = 0"
        ),
        0
    );
    // The same staging named twice links the one blob.
    let b = s
        .attach_staged(AttachOwner::Note(n), "copy.bin", &staged, H)
        .unwrap();
    assert_eq!(content(s, &b), data);
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 1);
    // Discarding after the row exists leaves the content alone.
    s.discard_staged(&staged).unwrap();
    assert_eq!(content(s, &a), data);

    // Content already stored stages without writing anything.
    let again = s
        .stage(&AttachSource::Bytes(data.clone()), &mut |_, _| {})
        .unwrap();
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 1);
    s.discard_staged(&again).unwrap();
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 1);

    // A cancelled editor drops its pending blob.
    let other = s
        .stage(&AttachSource::Bytes(bytes(9, 100)), &mut |_, _| {})
        .unwrap();
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 2);
    s.discard_staged(&other).unwrap();
    assert_eq!(count(s, "SELECT count(*) FROM attachment_blobs"), 1);
    // ... after which it cannot be saved any more.
    assert!(
        s.attach_staged(AttachOwner::Note(n), "gone.bin", &other, H)
            .is_err()
    );

    // Images carry their kind and pixel size before they have a row.
    let img = s
        .stage(&AttachSource::Bytes(png(40, 30)), &mut |_, _| {})
        .unwrap();
    assert_eq!(img.kind(), FileKind::Image);
    assert_eq!(img.pixels(), Some((40, 30)));
    assert!(s.staged_thumbnail(&img).unwrap().is_some());
    let shot = s
        .attach_staged(AttachOwner::Note(n), "shot.png", &img, H)
        .unwrap();
    assert!(s.attachment_thumbnail(shot.id).unwrap().is_some());
    assert_eq!(s.attachment_owner(shot.id).unwrap(), AttachOwner::Note(n));
}

#[test]
fn staging_refuses_too_large_and_saving_refuses_the_twenty_first() {
    let (mut ts, p, _) = setup();
    let s = &mut ts.store;
    let big = vec![1u8; MAX_FILE_BYTES as usize + 1];
    assert!(matches!(
        s.stage(&AttachSource::Bytes(big), &mut |_, _| {}),
        Err(Error::AttachmentTooLarge { .. })
    ));
    let n = s.add_note(p, "Twenty", H).unwrap();
    for i in 0..MAX_FILES_PER_OWNER {
        add(s, AttachOwner::Note(n), "f.bin", bytes(i as u8 + 1, 10));
    }
    let one = s
        .stage(&AttachSource::Bytes(bytes(200, 10)), &mut |_, _| {})
        .unwrap();
    assert!(matches!(
        s.attach_staged(AttachOwner::Note(n), "x.bin", &one, H),
        Err(Error::TooManyFiles { .. })
    ));
}
