//! Store tests. Each runs on a temporary directory with an in-memory key
//! store: the real keychain and data directory are never touched.

use super::test_support::{TestStore, open};
use super::*;
use crate::keys::{Key, KeyStore, MemoryKeyStore};
use crate::model::*;
use crate::snapshots::SnapshotKind;
use std::sync::Arc;

const H: Author = Author::Human;
const C: Author = Author::Claude;

fn topic(s: &mut Store, project: ProjectId, epic: Option<EpicId>, title: &str) -> TopicId {
    s.create_topic(
        NewTopic {
            project,
            epic,
            title: title.into(),
            description: String::new(),
            status: Status::Draft,
        },
        H,
    )
    .unwrap()
}

fn entry(kind: EntryKind, text: &str) -> NewEntry {
    NewEntry {
        kind,
        text: text.into(),
        state: None,
        status: None,
    }
}

fn kb(owner: Owner, kind: KbKind, title: &str, body: &str) -> NewKb {
    NewKb {
        owner,
        kind,
        title: title.into(),
        summary: format!("About {title}"),
        body: body.into(),
        from_file: false,
    }
}

fn all() -> PageReq {
    PageReq::first(100)
}

fn files_contain(dir: &std::path::Path, needle: &[u8]) -> bool {
    ["acta.db", "acta.db-wal", "acta.db-shm"]
        .iter()
        .any(|name| {
            std::fs::read(dir.join(name))
                .map(|bytes| bytes.windows(needle.len()).any(|w| w == needle))
                .unwrap_or(false)
        })
}

// ---- storage ----

#[test]
fn the_database_is_encrypted_and_reopens_with_its_key() {
    let mut t = open();
    assert!(t.store.was_created());
    let (version, provider) = t.store.cipher_info().unwrap();
    assert!(version.starts_with('4'), "cipher_version {version}");
    // ADR 0003: CommonCrypto on macOS, vendored OpenSSL elsewhere.
    let expected = if cfg!(target_os = "macos") {
        "commoncrypto"
    } else {
        "openssl"
    };
    assert_eq!(provider, expected);
    let marker = "Alexandre-plaintext-marker-5d1c";
    let p = t.store.create_project(marker, marker, H).unwrap();
    t.store.add_note(p, marker, H).unwrap();
    assert!(!files_contain(t.dir.path(), marker.as_bytes()));
    assert!(!files_contain(t.dir.path(), b"SQLite format 3"));
    assert!(!files_contain(t.dir.path(), b"CREATE TABLE"));
    let config = t.config();
    drop(t.store);
    let again = Store::open(&config).unwrap();
    assert!(!again.was_created());
    assert_eq!(again.project(p).unwrap().title, marker);
    // The marker file records the store, and holds no key.
    let marker_file =
        std::fs::read_to_string(config.data_dir.join(crate::keys::MARKER_FILE)).unwrap();
    assert_eq!(marker_file, "memory");
}

#[test]
fn a_lost_key_is_refused_and_the_file_left_untouched() {
    let t = open();
    let config = t.config();
    drop(t.store);
    let db = config.data_dir.join(DB_FILE);
    let before = std::fs::read(&db).unwrap();
    t.keys.forget();
    match Store::open(&config) {
        Err(OpenError::KeyMissing { db: path }) => assert_eq!(path, db),
        other => panic!("expected KeyMissing, got {:?}", other.err()),
    }
    assert_eq!(std::fs::read(&db).unwrap(), before);
    // No key was created behind the user's back.
    assert!(t.keys.load().unwrap().is_none());

    // Recovery: the file is kept aside, intact, and a new database starts.
    let aside = crate::db::set_aside_unreadable(&config.data_dir).unwrap();
    assert_eq!(std::fs::read(&aside).unwrap(), before);
    let fresh = Store::open(&config).unwrap();
    assert!(fresh.was_created());
    assert!(fresh.projects().unwrap().is_empty());
}

#[test]
fn a_wrong_key_is_refused() {
    let t = open();
    let config = t.config();
    drop(t.store);
    t.keys.set(Key::generate().unwrap());
    assert!(matches!(
        Store::open(&config),
        Err(OpenError::KeyWrongOrCorrupt { .. })
    ));
}

#[test]
fn an_unreachable_key_store_is_an_error_not_a_new_key() {
    let t = open();
    let config = t.config();
    drop(t.store);
    let key = t.keys.load().unwrap();
    t.keys.set_failing(true);
    assert!(matches!(Store::open(&config), Err(OpenError::KeyStore(_))));
    t.keys.set_failing(false);
    assert_eq!(t.keys.load().unwrap(), key);
}

#[test]
fn a_new_database_falls_back_to_the_next_store_and_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let keychain = MemoryKeyStore::new();
    keychain.set_failing(true);
    let file = crate::keys::KeyFile::new(dir.path().join(crate::keys::KEY_FILE));
    let config = StoreConfig {
        data_dir: dir.path().to_path_buf(),
        keys: crate::keys::KeyStores(vec![Arc::new(keychain.clone()), Arc::new(file)]),
    };
    drop(Store::open(&config).unwrap());
    let marker = std::fs::read_to_string(dir.path().join(crate::keys::MARKER_FILE)).unwrap();
    assert_eq!(marker, "file");
    // Once decided, never re-decided: the keychain working later changes nothing.
    keychain.set_failing(false);
    drop(Store::open(&config).unwrap());
    assert!(keychain.load().unwrap().is_none());
}

#[test]
fn a_failed_migration_restores_the_snapshot() {
    let mut t = open();
    let p = t.store.create_project("Acta", "", H).unwrap();
    let config = t.config();
    drop(t.store);
    let mut steps: Vec<&str> = crate::schema::MIGRATIONS.to_vec();
    steps.push("CREATE TABLE extra (a INTEGER) STRICT;");
    steps.push("SELECT no_such_column FROM no_such_table;");
    match Store::open_with(&config, &steps) {
        Err(OpenError::MigrationFailed {
            restored_from: Some(path),
            ..
        }) => assert!(path.exists()),
        other => panic!("expected MigrationFailed, got {:?}", other.err()),
    }
    let store = Store::open(&config).unwrap();
    assert_eq!(
        crate::db::user_version(&store.conn).unwrap(),
        crate::schema::SCHEMA_VERSION
    );
    let extra: i64 = store
        .conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'extra'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(extra, 0);
    assert_eq!(store.project(p).unwrap().title, "Acta");
    let kinds: Vec<_> = store.snapshots().unwrap().iter().map(|s| s.kind).collect();
    assert_eq!(kinds, [SnapshotKind::Migration]);
}

#[test]
fn a_newer_schema_is_left_alone_and_a_raised_minimum_makes_it_read_only() {
    let t = open();
    let config = t.config();
    drop(t.store);
    let mut steps: Vec<&str> = crate::schema::MIGRATIONS.to_vec();
    steps.push("CREATE TABLE future (a INTEGER) STRICT;");
    let newer = Store::open_with(&config, &steps).unwrap();
    newer
        .conn
        .execute(
            "INSERT INTO meta (key, value) VALUES ('min_compatible_version', ?1)",
            [(crate::schema::SCHEMA_VERSION + 1).to_string()],
        )
        .unwrap();
    drop(newer);
    let mut old = Store::open(&config).unwrap();
    assert!(!old.is_writable());
    assert!(old.projects().unwrap().is_empty());
    assert!(matches!(
        old.create_project("Acta", "", H),
        Err(Error::ReadOnly)
    ));
}

#[test]
fn the_watcher_sees_commits_from_any_connection() {
    let mut t = open();
    let mut watcher = t.store.watcher().unwrap();
    assert!(!watcher.changed().unwrap());
    t.store.create_project("Acta", "", H).unwrap();
    assert!(watcher.changed().unwrap());
    assert!(!watcher.changed().unwrap());
    let mut other = Store::open(&t.config()).unwrap();
    other.create_project("Other", "", H).unwrap();
    assert!(watcher.changed().unwrap());
}

#[test]
fn snapshots_are_daily_encrypted_and_restorable() {
    let mut t = open();
    let p = t.store.create_project("Before", "", H).unwrap();
    let snap = t.store.snapshot_if_due().unwrap().expect("first daily");
    assert!(t.store.snapshot_if_due().unwrap().is_none());
    assert!(
        !std::fs::read(&snap.path)
            .unwrap()
            .windows(6)
            .any(|w| w == b"Before")
    );
    t.store
        .update_project(
            p,
            ProjectPatch {
                title: Some("After".into()),
                ..Default::default()
            },
            H,
        )
        .unwrap();
    t.store.restore_snapshot(&snap.path).unwrap();
    assert_eq!(t.store.project(p).unwrap().title, "Before");
    let kinds: Vec<_> = t
        .store
        .snapshots()
        .unwrap()
        .iter()
        .map(|s| s.kind)
        .collect();
    assert!(kinds.contains(&SnapshotKind::Restore));
    assert!(kinds.contains(&SnapshotKind::Daily));
}

// ---- projects, epics, topics ----

#[test]
fn the_tree_orders_loose_topics_then_epics_with_progress() {
    let TestStore { mut store, .. } = open();
    let p = store
        .create_project("Acta", "Tracker for Alexandre", H)
        .unwrap();
    let e1 = store.create_epic(p, "Storage", "", H).unwrap();
    let e2 = store.create_epic(p, "Terminal", "", H).unwrap();
    let loose = topic(&mut store, p, None, "Loose");
    let a = topic(&mut store, p, Some(e1), "Encrypted storage");
    let b = topic(&mut store, p, Some(e1), "Snapshots");
    let c = topic(&mut store, p, Some(e1), "Dropped idea");
    let status = |s| TopicPatch {
        status: Some(s),
        ..Default::default()
    };
    store.update_topic(a, status(Status::Done), H).unwrap();
    store.update_topic(c, status(Status::Dropped), H).unwrap();
    store.reorder(Record::Epic(e2), 0, H).unwrap();
    store.reorder(Record::Topic(b), 0, H).unwrap();

    let tree = store.tree(p).unwrap();
    let ids = |v: &[TopicSummary]| v.iter().map(|t| t.id).collect::<Vec<_>>();
    assert_eq!(ids(&tree.loose_topics), [loose]);
    assert_eq!(
        tree.epics.iter().map(|(e, _)| e.id).collect::<Vec<_>>(),
        [e2, e1]
    );
    let storage = &tree.epics[1];
    assert_eq!(ids(&storage.1), [b, a, c]);
    assert_eq!(storage.0.progress, Progress { done: 1, total: 2 });
    assert_eq!(tree.project.progress, Progress { done: 1, total: 3 });

    let filter = TopicFilter {
        project: Some(p),
        ..Default::default()
    };
    let page = store.topics(&filter, PageReq::first(2)).unwrap();
    assert_eq!(page.total, 4);
    assert_eq!(page.items[0].id, loose);
    assert_eq!(page.next(), Some(2));
    let drafts = TopicFilter {
        statuses: vec![Status::Draft],
        ..Default::default()
    };
    assert_eq!(store.topics(&drafts, all()).unwrap().total, 2);
}

#[test]
fn topic_changes_are_recorded_as_system_events() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let e = store.create_epic(p, "Storage", "", H).unwrap();
    let other = store.create_project("Other", "", H).unwrap();
    let oe = store.create_epic(other, "Elsewhere", "", H).unwrap();
    let t = topic(&mut store, p, None, "Keys");
    store
        .update_topic(
            t,
            TopicPatch {
                status: Some(Status::Active),
                state: Some("Key in Credential Manager.".into()),
                epic: Some(Some(e)),
                ..Default::default()
            },
            C,
        )
        .unwrap();
    let cross = TopicPatch {
        epic: Some(Some(oe)),
        ..Default::default()
    };
    assert!(matches!(
        store.update_topic(t, cross, H),
        Err(Error::Invalid(_))
    ));
    let now = store.topic(t).unwrap();
    assert_eq!(now.epic, Some(e));
    assert_eq!(now.state_updated_by, Some(C));
    assert_eq!(now.stamp.updated_by, C);

    let filter = EntryFilter {
        include_system: true,
        ..Default::default()
    };
    let events: Vec<EventKind> = store
        .entries(t, &filter, all())
        .unwrap()
        .items
        .iter()
        .filter_map(|e| e.event.as_ref().map(|ev| ev.kind))
        .collect();
    for kind in [
        EventKind::Created,
        EventKind::Status,
        EventKind::State,
        EventKind::Moved,
    ] {
        assert!(events.contains(&kind), "{kind:?} missing from {events:?}");
    }
    // System events are not timeline entries for counts and agent pages.
    let plain = store.entries(t, &EntryFilter::default(), all()).unwrap();
    assert_eq!(plain.total, 0);
    assert!(store.log(t, entry(EntryKind::System, "x"), H).is_err());
}

#[test]
fn log_numbers_entries_and_refreshes_state() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let t = topic(&mut store, p, None, "Keys");
    let first = store
        .log(
            t,
            entry(EntryKind::Discussion, "Alexandre prefers one file"),
            H,
        )
        .unwrap();
    let second = store
        .log(
            t,
            NewEntry {
                kind: EntryKind::Decision,
                text: "Raw 32-byte key".into(),
                state: Some("Raw key chosen.".into()),
                status: Some(Status::Active),
            },
            C,
        )
        .unwrap();
    assert_eq!((first.seq, second.seq), (Some(1), Some(2)));
    assert_eq!(store.entry_by_seq(t, 2).unwrap().text, "Raw 32-byte key");
    let now = store.topic(t).unwrap();
    assert_eq!(now.state, "Raw key chosen.");
    assert_eq!(now.status, Status::Active);
    let page = store
        .entries(t, &EntryFilter::default(), PageReq::first(1))
        .unwrap();
    assert_eq!((page.total, page.items[0].seq), (2, Some(2)));
    let older = EntryFilter {
        before_seq: Some(2),
        ..Default::default()
    };
    assert_eq!(store.entries(t, &older, all()).unwrap().items.len(), 1);
    assert_eq!(store.tree(p).unwrap().loose_topics[0].entries, 2);
    assert!(store.log(t, entry(EntryKind::Progress, "   "), H).is_err());
}

#[test]
fn notes_wait_in_the_inbox_until_filed() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let n1 = store.add_note(p, "Ask Alexandre about macOS", H).unwrap();
    let n2 = store.add_note(p, "Quick capture shortcut", H).unwrap();
    assert_eq!(store.projects().unwrap()[0].inbox, 2);
    let t = topic(&mut store, p, None, "Capture");
    store.file_notes(&[n2], t, C).unwrap();
    let inbox = store.inbox(p, all()).unwrap();
    assert_eq!(inbox.items.iter().map(|n| n.id).collect::<Vec<_>>(), [n1]);
    assert_eq!(store.note(n2).unwrap().filed_topic, Some(t));
}

// ---- knowledge ----

fn update(expected_rev: Option<i64>, edit: KbEdit) -> KbUpdate {
    KbUpdate {
        expected_rev,
        title: None,
        summary: None,
        edit,
    }
}

#[test]
fn knowledge_revisions_sections_and_stale_writes() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let body = "# Crates\nThree.\n# Data flow\nDown.\n";
    let k = store
        .kb_create(
            kb(Owner::Project(p), KbKind::Document, "Architecture", body),
            H,
        )
        .unwrap();
    let section = KbEdit::ReplaceSection {
        number: "1".into(),
        heading: None,
        text: "Four.".into(),
    };
    assert_eq!(store.kb_update(k, update(Some(1), section), C).unwrap(), 2);
    assert!(
        store
            .kb_entry(k)
            .unwrap()
            .body
            .contains("# Crates\n\nFour.\n")
    );
    match store.kb_update(k, update(Some(1), KbEdit::Body("lost".into())), C) {
        Err(Error::StaleRevision { current }) => assert_eq!(current, 2),
        other => panic!("{other:?}"),
    }
    let append = KbEdit::AppendSection {
        heading: "Open questions".into(),
        text: "None.".into(),
    };
    store.kb_update(k, update(None, append), H).unwrap();
    let history = store.kb_history(k).unwrap();
    let changes: Vec<_> = history
        .iter()
        .map(|h| (h.rev, h.change, h.section.clone()))
        .collect();
    assert_eq!(
        changes,
        [
            (
                3,
                ChangeKind::AppendedSection,
                Some("Open questions".into())
            ),
            (2, ChangeKind::ReplacedSection, Some("Crates".into())),
            (1, ChangeKind::Created, None),
        ]
    );
    assert_eq!(store.kb_restore_revision(k, 1, H).unwrap(), 4);
    let now = store.kb_entry(k).unwrap();
    assert_eq!(now.body, store.kb_revision(k, 1).unwrap().body);
    assert_eq!(now.rev, 4);
    // An identical save makes no revision.
    assert_eq!(
        store
            .kb_update(k, update(Some(4), KbEdit::Keep), H)
            .unwrap(),
        4
    );
    let no_summary = NewKb {
        summary: " ".into(),
        ..kb(Owner::Project(p), KbKind::Memory, "x", "")
    };
    assert!(store.kb_create(no_summary, C).is_err());
}

#[test]
fn knowledge_scope_looks_up_and_rollup_looks_down() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let e = store.create_epic(p, "Storage", "", H).unwrap();
    let t = topic(&mut store, p, Some(e), "Keys");
    let sibling = topic(&mut store, p, Some(e), "Snapshots");
    let mut add =
        |owner, kind, title, author| store.kb_create(kb(owner, kind, title, ""), author).unwrap();
    let kp = add(Owner::Project(p), KbKind::Document, "Architecture", H);
    let ke = add(Owner::Epic(e), KbKind::Document, "Storage requirements", H);
    let kt = add(Owner::Topic(t), KbKind::Memory, "Run tests serially", C);
    let ks = add(Owner::Topic(sibling), KbKind::Memory, "Snapshot naming", C);

    let ids = |page: Page<KbSummary>| page.items.iter().map(|k| k.id).collect::<Vec<_>>();
    let none = KbFilter::default();
    assert_eq!(
        ids(store.kb_scope(Owner::Topic(t), &none, all()).unwrap()),
        [kt, ke, kp]
    );
    assert_eq!(
        ids(store.kb_scope(Owner::Epic(e), &none, all()).unwrap()),
        [ke, kp]
    );
    let mut down = ids(store.kb_rollup(Owner::Epic(e), &none, all()).unwrap());
    down.sort();
    assert_eq!(down, [ke, kt, ks]);
    let rollup = store.kb_rollup(Owner::Project(p), &none, all()).unwrap();
    assert_eq!(rollup.total, 4);
    let by_claude = KbFilter {
        kind: Some(KbKind::Memory),
        created_by: Some(C),
        level: None,
    };
    let mine = store
        .kb_rollup(Owner::Project(p), &by_claude, all())
        .unwrap();
    assert_eq!(mine.total, 2);
    assert_eq!(
        store.kb_counts(Owner::Topic(t)).unwrap(),
        KbCounts {
            documents: 2,
            memories: 1
        }
    );
    assert_eq!(store.projects().unwrap()[0].knowledge, 4);

    // The human moves a memory up to the epic.
    store.kb_move(kt, Owner::Epic(e), H).unwrap();
    let k = store.kb_entry(kt).unwrap();
    assert_eq!((k.owner, k.rev), (Owner::Epic(e), 2));
    let seen = store.kb_scope(Owner::Topic(sibling), &none, all()).unwrap();
    assert_eq!(seen.total, 4);
}

// ---- folders ----

#[test]
fn folders_inherit_nearest_wins_and_resolve_deepest() {
    let TestStore { mut store, dir, .. } = open();
    let root = dir.path().join("work");
    let repo = root.join("acta");
    let worktree = root.join("acta-wt").join("keys");
    let docs = root.join("docs");
    for d in [&repo, &worktree.join("src"), &docs, &repo.join("crates")] {
        std::fs::create_dir_all(d).unwrap();
    }
    let p = store.create_project("Acta", "", H).unwrap();
    let e = store.create_epic(p, "Storage", "", H).unwrap();
    let t = topic(&mut store, p, Some(e), "Keys");
    let t2 = topic(&mut store, p, Some(e), "Snapshots");

    // Nothing attached anywhere.
    let none = store.effective_folders(Owner::Topic(t)).unwrap();
    assert!(none.folders.is_empty());

    store.add_folder(Owner::Project(p), &repo, H).unwrap();
    store.add_folder(Owner::Project(p), &docs, H).unwrap();
    let same_again = repo.join("crates").join("..");
    assert!(store.add_folder(Owner::Project(p), &same_again, H).is_err());
    let inherited = store.effective_folders(Owner::Topic(t)).unwrap();
    assert_eq!(inherited.inherited_from, Some(Owner::Project(p)));
    assert_eq!(inherited.folders.len(), 2);
    assert!(inherited.folders[0].path.ends_with("acta"));

    store.add_folder(Owner::Topic(t), &worktree, H).unwrap();
    let own = store.effective_folders(Owner::Topic(t)).unwrap();
    assert_eq!(own.inherited_from, None);
    assert_eq!(own.folders.len(), 1);
    store
        .set_include_inherited(Owner::Topic(t), true, H)
        .unwrap();
    let both = store.effective_folders(Owner::Topic(t)).unwrap();
    assert_eq!(
        both.folders.iter().map(|f| f.from).collect::<Vec<_>>(),
        [Owner::Topic(t), Owner::Project(p), Owner::Project(p)]
    );

    // Reverse lookup: the deepest attached folder wins.
    let hit = store
        .resolve_folder(&worktree.join("src"))
        .unwrap()
        .unwrap();
    assert_eq!((hit.project, hit.epic, hit.topic), (p, Some(e), Some(t)));
    let hit = store.resolve_folder(&repo.join("crates")).unwrap().unwrap();
    assert_eq!((hit.epic, hit.topic), (None, None));
    assert!(store.resolve_folder(&root).unwrap().is_none());

    // The same folder on two topics of one epic: epic focus, both candidates.
    store.add_folder(Owner::Topic(t2), &worktree, H).unwrap();
    let hit = store.resolve_folder(&worktree).unwrap().unwrap();
    assert_eq!((hit.epic, hit.topic), (Some(e), None));
    assert_eq!(hit.candidates.len(), 2);

    // Trashed owners no longer resolve.
    store.delete(Record::Topic(t2), H).unwrap();
    let hit = store.resolve_folder(&worktree).unwrap().unwrap();
    assert_eq!(hit.topic, Some(t));
}

#[test]
fn unresolved_folders_are_relocated_by_root() {
    let TestStore { mut store, dir, .. } = open();
    let new_root = dir.path().join("new");
    std::fs::create_dir_all(new_root.join("acta")).unwrap();
    let p = store.create_project("Acta", "", H).unwrap();
    let (old_root, sep) = if cfg!(windows) {
        (r"Z:\old", r"\")
    } else {
        ("/nonexistent/old", "/")
    };
    let owner = Owner::Project(p);
    let f1 = store
        .add_folder_stored(owner, &format!("{old_root}{sep}acta"), H)
        .unwrap();
    let f2 = store
        .add_folder_stored(owner, &format!("{old_root}{sep}gone"), H)
        .unwrap();
    let changed = store.relocate_folders(old_root, &new_root, H).unwrap();
    assert_eq!(changed, [f1]);
    let moved = store.folder(f1).unwrap().path;
    assert!(moved.ends_with("acta"));
    assert!(std::path::Path::new(&moved).is_dir());
    assert!(store.folder(f2).unwrap().path.starts_with(old_root));
    assert_eq!(store.all_folders().unwrap().len(), 2);
}

// ---- trash ----

#[test]
fn delete_cascades_and_restore_brings_everything_back() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let e = store.create_epic(p, "Storage", "", H).unwrap();
    let t = topic(&mut store, p, Some(e), "Keys");
    let earlier = topic(&mut store, p, Some(e), "Old idea");
    store
        .log(t, entry(EntryKind::Progress, "WAL on"), C)
        .unwrap();
    let k = store
        .kb_create(kb(Owner::Topic(t), KbKind::Memory, "Fact", ""), C)
        .unwrap();

    store.delete(Record::Topic(earlier), H).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    store.delete(Record::Epic(e), C).unwrap();
    assert!(matches!(store.topic(t), Err(Error::NotFound(_))));
    assert!(store.kb_entry(k).is_err());
    assert!(store.delete(Record::Epic(e), H).is_err());

    let trash = store.trash(all()).unwrap();
    let records: Vec<Record> = trash.items.iter().map(|i| i.record).collect();
    assert_eq!(records, [Record::Epic(e), Record::Topic(earlier)]);
    let first = &trash.items[0];
    assert_eq!(first.deleted_by, C);
    assert_eq!(first.expires_at - first.deleted_at, TRASH_RETENTION_MS);

    // A topic cannot come back while its epic is in the trash.
    assert!(matches!(
        store.restore(Record::Topic(earlier), H),
        Err(Error::ParentDeleted(_))
    ));
    store.restore(Record::Epic(e), H).unwrap();
    assert_eq!(store.topic(t).unwrap().epic, Some(e));
    assert_eq!(store.kb_entry(k).unwrap().title, "Fact");
    let entries = store.entries(t, &EntryFilter::default(), all()).unwrap();
    assert_eq!(entries.total, 1);
    // What was deleted on its own stays in the trash.
    assert!(store.topic(earlier).is_err());
    assert_eq!(store.trash(all()).unwrap().total, 1);
}

// ---- search ----

#[test]
fn search_finds_live_records_with_snippets() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let t = topic(&mut store, p, None, "Keychain storage");
    let gone = topic(&mut store, p, None, "Keychain on macOS");
    store
        .log(
            t,
            entry(EntryKind::Decision, "Décision: raw key in the keychain"),
            C,
        )
        .unwrap();
    store
        .add_note(p, "Ask whether the keychain prompts", H)
        .unwrap();
    let doc = kb(
        Owner::Project(p),
        KbKind::Document,
        "Keys",
        "The keychain holds the key.",
    );
    let k = store.kb_create(doc, H).unwrap();
    store.delete(Record::Topic(gone), H).unwrap();

    let q = |text: &str, kinds: Vec<SearchKind>| SearchQuery {
        text: text.into(),
        project: Some(p),
        kinds,
    };
    let hits = store.search(&q("keych", vec![]), all()).unwrap();
    assert_eq!(hits.total, 4);
    assert!(!hits.items.iter().any(|h| h.record == Record::Topic(gone)));
    let kb_only = store
        .search(&q("keychain", vec![SearchKind::Kb]), all())
        .unwrap();
    assert_eq!(kb_only.items[0].record, Record::Kb(k));
    assert!(kb_only.items[0].snippet.contains(SNIPPET_OPEN));
    // Diacritics fold both ways.
    let decision = store
        .search(&q("decision", vec![SearchKind::Entry]), all())
        .unwrap();
    assert_eq!(decision.total, 1);
    assert_eq!(decision.items[0].topic, Some(t));
    assert_eq!(store.search(&q("\"", vec![]), all()).unwrap().total, 0);
}

#[test]
fn export_history_is_recorded() {
    let mut t = open();
    assert_eq!(t.store.last_export_at().unwrap(), None);
    t.store.record_export().unwrap();
    assert!(t.store.last_export_at().unwrap().is_some());
}

#[test]
fn concurrent_writers_lose_nothing() {
    let mut t = open();
    let p = t.store.create_project("Acta", "", H).unwrap();
    let topic_id = topic(&mut t.store, p, None, "Contention");
    let config = t.config();
    let writers: Vec<_> = (0..4)
        .map(|w| {
            let config = config.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(&config).unwrap();
                for i in 0..25 {
                    let text = format!("writer {w} step {i}");
                    store
                        .log(topic_id, entry(EntryKind::Progress, &text), C)
                        .unwrap();
                }
            })
        })
        .collect();
    for w in writers {
        w.join().unwrap();
    }
    let page = t
        .store
        .entries(topic_id, &EntryFilter::default(), PageReq::first(200))
        .unwrap();
    assert_eq!(page.total, 100);
    let mut seqs: Vec<i64> = page.items.iter().filter_map(|e| e.seq).collect();
    seqs.sort();
    assert_eq!(seqs, (1..=100).collect::<Vec<_>>());
}

#[cfg(unix)]
#[test]
fn database_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let t = open();
    let mode = |name: &str| {
        std::fs::metadata(t.dir.path().join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode(DB_FILE), 0o600);
    assert_eq!(mode(crate::keys::MARKER_FILE), 0o600);
    let snap = t.store.snapshot(SnapshotKind::Manual).unwrap();
    let snap_mode = std::fs::metadata(&snap.path).unwrap().permissions().mode() & 0o777;
    assert_eq!(snap_mode, 0o600);
}
