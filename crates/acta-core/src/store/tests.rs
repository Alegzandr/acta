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
        Err(OpenError::KeyMissing { db: path, store }) => {
            assert_eq!(path, db);
            assert_eq!(store, Some("memory"));
        }
        other => panic!("expected KeyMissing, got {:?}", other.err()),
    }
    assert_eq!(std::fs::read(&db).unwrap(), before);
    // No key was created behind the user's back.
    assert!(t.keys.load().unwrap().is_none());

    // Recovery: the file is kept aside, intact, and a new database starts.
    let name = crate::db::unreadable_name(1_795_250_112_000);
    assert_eq!(name, "acta.unreadable-2026-11-21-083512.db");
    let aside = crate::db::set_aside_unreadable(&config.data_dir, &name).unwrap();
    assert_eq!(std::fs::read(&aside).unwrap(), before);
    assert!(aside.ends_with(&name));
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
    let bytes = std::fs::read(&snap.path).unwrap();
    assert!(!bytes.windows(6).any(|w| w == b"Before"));
    assert!(!bytes.windows(15).any(|w| w == b"SQLite format 3"));
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
fn a_timeline_write_updates_the_topic() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let t = topic(&mut store, p, None, "Keys");
    let created = store.topic(t).unwrap().stamp;
    std::thread::sleep(std::time::Duration::from_millis(5));
    let e = store
        .log(t, entry(EntryKind::Progress, "Alexandre tried it"), C)
        .unwrap();
    let logged = store.topic(t).unwrap().stamp;
    assert!(logged.updated_at > created.updated_at);
    assert_eq!(logged.updated_by, C);
    assert_eq!(
        store.tree(p).unwrap().loose_topics[0].updated_at,
        logged.updated_at
    );
    std::thread::sleep(std::time::Duration::from_millis(5));
    store
        .update_entry(e.id, "Alexandre tried it twice", EntryKind::Progress, H)
        .unwrap();
    let edited = store.topic(t).unwrap().stamp;
    assert!(edited.updated_at > logged.updated_at);
    assert_eq!(edited.updated_by, H);
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
    let history = store.kb_history(k, all()).unwrap().items;
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
        ..SearchQuery::default()
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

// ---- export and import ----

fn populate(store: &mut Store, dir: &std::path::Path) -> ProjectId {
    let p = store
        .create_project("Acta", "Tracker for Alexandre", H)
        .unwrap();
    let e = store
        .create_epic(p, "Storage", "Encrypted at rest", H)
        .unwrap();
    let t = topic(store, p, Some(e), "Keys");
    let loose = topic(store, p, None, "Loose");
    let decision = NewEntry {
        kind: EntryKind::Decision,
        text: "Raw key".into(),
        state: Some("Decided.".into()),
        status: Some(Status::Active),
    };
    store.log(t, decision, C).unwrap();
    let to_epic = TopicPatch {
        epic: Some(Some(e)),
        ..Default::default()
    };
    store.update_topic(loose, to_epic, H).unwrap();
    let n = store.add_note(p, "Note from Alexandre", H).unwrap();
    store.add_note(p, "Unfiled", C).unwrap();
    store.file_notes(&[n], t, H).unwrap();
    let as_epic = store.add_note(p, "Sync between laptops", H).unwrap();
    store.file_notes_in_epic(&[as_epic], e, C).unwrap();
    let noise = store.add_note(p, "Duplicate", H).unwrap();
    store.dismiss_notes(&[noise], C).unwrap();
    let k = store
        .kb_create(kb(Owner::Topic(t), KbKind::Document, "Spec", "# A\nx\n"), H)
        .unwrap();
    store
        .kb_update(k, update(Some(1), KbEdit::Body("# A\ny\n".into())), C)
        .unwrap();
    let folder = dir.join("repo");
    std::fs::create_dir_all(&folder).unwrap();
    store.add_folder(Owner::Epic(e), &folder, H).unwrap();
    store
        .set_include_inherited(Owner::Epic(e), true, H)
        .unwrap();
    let trashed = topic(store, p, None, "Trashed");
    store.delete(Record::Topic(trashed), H).unwrap();
    let other = store.create_project("Other", "", H).unwrap();
    topic(store, other, None, "Elsewhere");
    p
}

#[test]
fn export_import_export_is_lossless() {
    let mut a = open();
    populate(&mut a.store, a.dir.path());
    let first = a.store.export(None).unwrap();
    let trashed = first.topics.iter().filter(|t| t.stamp.deleted_at.is_some());
    assert_eq!(trashed.count(), 1);
    assert!(first.folders.iter().all(|f| f.local));

    let mut b = open();
    let text = first.to_json().unwrap();
    let parsed = ExportFile::from_json(&text).unwrap();
    assert_eq!(parsed, first);
    let report = b.store.import(&parsed, false).unwrap();
    assert_eq!(report.total().updated + report.total().skipped, 0);
    assert_eq!(report.topics.created, 4);
    let mut second = b.store.export(None).unwrap();
    second.exported_at = first.exported_at;
    assert_eq!(second.to_json().unwrap(), text);

    // Importing the same file again changes nothing.
    let again = b.store.import(&parsed, false).unwrap();
    assert_eq!(again.total().created + again.total().updated, 0);
    let mut third = b.store.export(None).unwrap();
    third.exported_at = first.exported_at;
    assert_eq!(third, first);
}

#[test]
fn a_project_export_holds_only_that_project() {
    let mut a = open();
    let p = populate(&mut a.store, a.dir.path());
    let file = a.store.export(Some(p)).unwrap();
    assert_eq!(file.projects.len(), 1);
    let uuid = file.projects[0].stamp.uuid.clone();
    assert_eq!(file.project.as_deref(), Some(uuid.as_str()));
    assert!(file.topics.iter().all(|t| t.project == uuid));
    let mut b = open();
    b.store.import(&file, false).unwrap();
    assert_eq!(b.store.projects().unwrap().len(), 1);
}

#[test]
fn import_merges_newer_wins_and_never_deletes() {
    let mut a = open();
    let p = a.store.create_project("Acta", "", H).unwrap();
    let t = topic(&mut a.store, p, None, "Keys");
    let gone = topic(&mut a.store, p, None, "Gone");
    let base = a.store.export(None).unwrap();

    // The file has a newer title for one topic, which it also says is
    // trashed, and an older title for the other.
    let mut file = base.clone();
    for x in &mut file.topics {
        if x.title == "Keys" {
            x.title = "Keys (renamed by Alexandre)".into();
            x.stamp.updated_at += 10_000;
            x.stamp.deleted_at = Some(x.stamp.updated_at);
            x.stamp.deleted_by = Some(H);
        } else {
            x.title = "Stale".into();
            x.stamp.updated_at -= 10_000;
        }
    }
    a.store.delete(Record::Topic(gone), H).unwrap();

    let preview = a.store.import(&file, true).unwrap();
    let expected = ImportCounts {
        created: 0,
        updated: 1,
        skipped: 1,
    };
    assert_eq!(preview.topics, expected);
    assert_eq!(a.store.topic(t).unwrap().title, "Keys");

    let report = a.store.import(&file, false).unwrap();
    assert_eq!(report.topics, expected);
    // Updated, but never trashed by an import.
    assert_eq!(
        a.store.topic(t).unwrap().title,
        "Keys (renamed by Alexandre)"
    );
    // Never revived by an older version.
    assert!(a.store.topic(gone).is_err());
    let snapshots = a.store.snapshots().unwrap();
    assert!(snapshots.iter().any(|s| s.kind == SnapshotKind::Import));

    let mut future = base;
    future.version = FORMAT_VERSION + 1;
    let text = future.to_json().unwrap();
    assert!(matches!(
        ExportFile::from_json(&text),
        Err(Error::FormatTooNew { .. })
    ));
    assert!(matches!(
        ExportFile::from_json("{\"format\":\"x\",\"version\":1}"),
        Err(Error::Invalid(_))
    ));
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

// ---- review regressions ----

#[test]
fn restoring_a_restore_snapshot_keeps_it_and_the_last_three() {
    let mut t = open();
    let p = t.store.create_project("Original", "", H).unwrap();
    let s1 = t.store.snapshot(SnapshotKind::Manual).unwrap();
    let rename = |s: &mut Store, title: &str| {
        let patch = ProjectPatch {
            title: Some(title.into()),
            ..Default::default()
        };
        s.update_project(p, patch, H).unwrap();
    };
    rename(&mut t.store, "Changed");
    t.store.restore_snapshot(&s1.path).unwrap();
    let r1 = t
        .store
        .snapshots()
        .unwrap()
        .into_iter()
        .find(|s| s.kind == SnapshotKind::Restore)
        .unwrap();
    // r1 holds "Changed". Restoring it takes a new restore snapshot, which
    // must not delete r1 through retention.
    t.store.restore_snapshot(&r1.path).unwrap();
    assert!(r1.path.exists());
    assert_eq!(t.store.project(p).unwrap().title, "Changed");
    for title in ["a", "b", "c"] {
        rename(&mut t.store, title);
        t.store.restore_snapshot(&s1.path).unwrap();
    }
    let restores = t
        .store
        .snapshots()
        .unwrap()
        .into_iter()
        .filter(|s| s.kind == SnapshotKind::Restore)
        .count();
    assert_eq!(restores, 3);
}

#[test]
fn a_busy_database_fails_a_migration_without_restoring() {
    let mut t = open();
    let p = t.store.create_project("Acta", "", H).unwrap();
    let config = t.config();
    // Another process holds the write lock for longer than the busy timeout.
    t.store.conn.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let mut steps: Vec<&str> = crate::schema::MIGRATIONS.to_vec();
    steps.push("CREATE TABLE extra (a INTEGER) STRICT;");
    match Store::open_with(&config, &steps) {
        Err(OpenError::MigrationFailed { restored_from, .. }) => {
            assert!(restored_from.is_none())
        }
        other => panic!("expected MigrationFailed, got {:?}", other.err()),
    }
    t.store.conn.execute_batch("COMMIT;").unwrap();
    assert_eq!(t.store.project(p).unwrap().title, "Acta");
    // Once the lock is free the same ladder succeeds.
    drop(Store::open_with(&config, &steps).unwrap());
}

#[test]
fn an_import_that_lowers_rev_does_not_block_edits() {
    let mut a = open();
    let p = a.store.create_project("Acta", "", H).unwrap();
    let doc = kb(Owner::Project(p), KbKind::Document, "Spec", "x");
    let k = a.store.kb_create(doc, H).unwrap();
    let base = a.store.export(None).unwrap();
    a.store
        .kb_update(k, update(None, KbEdit::Body("y".into())), H)
        .unwrap();
    a.store
        .kb_update(k, update(None, KbEdit::Body("z".into())), H)
        .unwrap();
    let mut file = base;
    file.knowledge[0].stamp.updated_at += 60_000;
    file.knowledge[0].body = "remote".into();
    a.store.import(&file, false).unwrap();
    // The imported body is saved as a revision of its own and is current.
    let now = a.store.kb_entry(k).unwrap();
    assert_eq!((now.body.as_str(), now.rev), ("remote", 4));
    assert_eq!(a.store.kb_revision(k, 4).unwrap().body, "remote");
    let rev = a
        .store
        .kb_update(k, update(Some(4), KbEdit::Body("after".into())), H)
        .unwrap();
    assert_eq!(rev, 5);
}

#[test]
fn an_import_skipping_an_entry_keeps_its_revisions_without_collisions() {
    let mut a = open();
    let p = a.store.create_project("Acta", "", H).unwrap();
    let doc = kb(Owner::Project(p), KbKind::Document, "Spec", "x");
    let k = a.store.kb_create(doc, H).unwrap();
    let mut b = open();
    b.store
        .import(&a.store.export(None).unwrap(), false)
        .unwrap();
    let bp = b.store.projects().unwrap()[0].id;
    let bk = b
        .store
        .kb_rollup(Owner::Project(bp), &KbFilter::default(), all())
        .unwrap()
        .items[0]
        .id;
    for body in ["b2", "b3"] {
        b.store
            .kb_update(bk, update(None, KbEdit::Body(body.into())), H)
            .unwrap();
    }
    std::thread::sleep(std::time::Duration::from_millis(5));
    a.store
        .kb_update(k, update(None, KbEdit::Body("a2".into())), H)
        .unwrap();
    let rep = a
        .store
        .import(&b.store.export(None).unwrap(), false)
        .unwrap();
    assert_eq!(rep.knowledge.skipped, 1);
    // b2 and b3 are kept, renumbered after a's own history.
    assert_eq!(rep.revisions.created, 2);
    let now = a.store.kb_entry(k).unwrap();
    assert_eq!((now.body.as_str(), now.rev), ("a2", 2));
    assert_eq!(a.store.kb_history(k, all()).unwrap().total, 4);
    let rev = a
        .store
        .kb_update(k, update(Some(2), KbEdit::Body("a3".into())), H)
        .unwrap();
    assert_eq!(rev, 5);
    let page = a.store.kb_history(k, PageReq::first(2)).unwrap();
    assert_eq!((page.items.len(), page.total, page.next()), (2, 5, Some(2)));
}

#[test]
fn merged_timelines_keep_unique_entry_numbers() {
    let mut a = open();
    let p = a.store.create_project("Acta", "", H).unwrap();
    let t = topic(&mut a.store, p, None, "T");
    let mut b = open();
    b.store
        .import(&a.store.export(None).unwrap(), false)
        .unwrap();
    let bt = b
        .store
        .topics(&TopicFilter::default(), all())
        .unwrap()
        .items[0]
        .id;
    let from_a = a
        .store
        .log(t, entry(EntryKind::Progress, "from A"), H)
        .unwrap();
    b.store
        .log(bt, entry(EntryKind::Progress, "from B"), H)
        .unwrap();
    a.store
        .import(&b.store.export(None).unwrap(), false)
        .unwrap();
    let entries = a.store.entries(t, &EntryFilter::default(), all()).unwrap();
    let mut seqs: Vec<_> = entries.items.iter().filter_map(|e| e.seq).collect();
    seqs.sort();
    assert_eq!(seqs, [1, 2]);
    // The local entry keeps its number; the imported one takes the next.
    assert_eq!(a.store.entry_by_seq(t, 1).unwrap().id, from_a.id);
    assert_eq!(a.store.entry_by_seq(t, 2).unwrap().text, "from B");
}

#[test]
fn folder_resolution_prefers_the_most_specific_level() {
    let TestStore { mut store, dir, .. } = open();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let p = store.create_project("Acta", "", H).unwrap();
    let e = store.create_epic(p, "Storage", "", H).unwrap();
    let t = topic(&mut store, p, Some(e), "Keys");
    store.add_folder(Owner::Project(p), &repo, H).unwrap();
    store.add_folder(Owner::Epic(e), &repo, H).unwrap();
    let hit = store.resolve_folder(&repo).unwrap().unwrap();
    assert_eq!((hit.epic, hit.topic), (Some(e), None));
    assert!(hit.candidates.is_empty());
    store.add_folder(Owner::Topic(t), &repo, H).unwrap();
    let hit = store.resolve_folder(&repo).unwrap().unwrap();
    assert_eq!((hit.epic, hit.topic), (Some(e), Some(t)));
    assert!(hit.candidates.is_empty());
}

#[test]
fn a_restore_brings_back_only_what_one_delete_trashed() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let e = store.create_epic(p, "Storage", "", H).unwrap();
    let t = topic(&mut store, p, Some(e), "Keys");
    let entry_id = store.log(t, entry(EntryKind::Progress, "x"), H).unwrap().id;
    // Two deletes in the same millisecond still form two batches.
    store.delete(Record::Entry(entry_id), H).unwrap();
    store.delete(Record::Epic(e), H).unwrap();
    store.restore(Record::Epic(e), H).unwrap();
    assert!(store.topic(t).is_ok());
    assert!(store.entry(entry_id).is_err());
    let trash = store.trash(all()).unwrap();
    assert_eq!(trash.items[0].record, Record::Entry(entry_id));
}

#[test]
fn a_dry_run_import_works_on_a_read_only_database() {
    let mut a = open();
    let p = a.store.create_project("Acta", "", H).unwrap();
    topic(&mut a.store, p, None, "Keys");
    let file = a.store.export(None).unwrap();
    let b = open();
    let config = b.config();
    b.store
        .conn
        .execute(
            "INSERT INTO meta (key, value) VALUES ('min_compatible_version', ?1)",
            [(crate::schema::SCHEMA_VERSION + 1).to_string()],
        )
        .unwrap();
    drop(b.store);
    let mut ro = Store::open(&config).unwrap();
    assert!(!ro.is_writable());
    let preview = ro.import(&file, true).unwrap();
    assert_eq!(preview.topics.created, 1);
    assert!(ro.projects().unwrap().is_empty());
    assert!(matches!(ro.import(&file, false), Err(Error::ReadOnly)));
}

#[test]
fn search_reports_whether_the_total_is_capped() {
    let TestStore { mut store, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let t = topic(&mut store, p, None, "Keys");
    for i in 0..3 {
        let text = format!("keychain {i}");
        store.log(t, entry(EntryKind::Progress, &text), H).unwrap();
    }
    let q = SearchQuery {
        text: "keychain".into(),
        project: Some(p),
        kinds: vec![],
        ..SearchQuery::default()
    };
    let page = store.search(&q, all()).unwrap();
    assert_eq!((page.total, page.capped), (3, false));
}

#[test]
fn a_fresh_database_has_entry_numbers_unique_and_trash_batches() {
    let t = open();
    assert_eq!(crate::schema::SCHEMA_VERSION, 3);
    let count = |sql: &str| -> i64 { t.store.conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(
        count("SELECT count(*) FROM sqlite_master WHERE name = 'entries_topic_seq'"),
        1
    );
    for table in [
        "projects",
        "epics",
        "topics",
        "entries",
        "notes",
        "kb_entries",
        "folders",
    ] {
        let sql = format!(
            "SELECT count(*) FROM pragma_table_info('{table}') WHERE name = 'deleted_batch'"
        );
        assert_eq!(count(&sql), 1, "{table}");
    }
}

#[test]
fn an_import_that_changes_the_trash_state_clears_the_batch() {
    let mut a = open();
    let p = a.store.create_project("Acta", "", H).unwrap();
    let t = topic(&mut a.store, p, None, "Keys");
    let mut file = a.store.export(None).unwrap();
    a.store.delete(Record::Topic(t), H).unwrap();
    // A newer, live version revives the topic; no batch is left behind.
    file.topics[0].stamp.updated_at += 60_000;
    a.store.import(&file, false).unwrap();
    assert!(a.store.topic(t).is_ok());
    let batch: Option<String> = a
        .store
        .conn
        .query_row(
            "SELECT deleted_batch FROM topics WHERE id = ?1",
            [t.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(batch, None);
}

#[test]
fn atomically_commits_everything_or_nothing() {
    let mut t = open();
    let s = &mut t.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let a = topic(s, p, None, "A");
    let result: Result<()> = s.atomically(|s| {
        s.update_topic(
            a,
            TopicPatch {
                title: Some("Renamed".into()),
                ..TopicPatch::default()
            },
            H,
        )?;
        Err(Error::Invalid("stop".into()))
    });
    assert!(result.is_err());
    assert_eq!(s.topic(a).unwrap().title, "A", "rolled back");
    let b = s
        .atomically(|s| {
            s.update_topic(
                a,
                TopicPatch {
                    status: Some(Status::Active),
                    ..TopicPatch::default()
                },
                H,
            )?;
            Ok(topic(s, p, None, "B"))
        })
        .unwrap();
    assert_eq!(s.topic(a).unwrap().status, Status::Active);
    assert_eq!(s.topic(b).unwrap().title, "B");
}

#[test]
fn a_note_becomes_a_topic_in_one_step_and_can_go_back_to_the_inbox() {
    let mut t = open();
    let s = &mut t.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let other = s.create_project("Atlas", "", H).unwrap();
    let note = s.add_note(p, "Weekly digest of decisions", H).unwrap();
    let new = |project| NewTopic {
        project,
        epic: None,
        title: "Weekly digest".into(),
        description: String::new(),
        status: Status::Draft,
    };
    assert!(s.note_to_topic(note, new(other), H).is_err());
    assert!(
        s.topics(&TopicFilter::default(), all())
            .unwrap()
            .items
            .is_empty()
    );
    let topic = s.note_to_topic(note, new(p), H).unwrap();
    assert_eq!(s.note(note).unwrap().filed_topic, Some(topic));
    s.unfile_note(note, H).unwrap();
    assert_eq!(s.note(note).unwrap().filed_topic, None);
}

#[test]
fn notes_leave_the_inbox_for_a_topic_an_epic_or_nowhere_and_come_back() {
    let mut t = open();
    let s = &mut t.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let e = s.create_epic(p, "Sync", "", H).unwrap();
    let k = topic(s, p, Some(e), "Keys");
    let n: Vec<_> = (0..3)
        .map(|i| s.add_note(p, &format!("Note {i} by Alexandre"), H).unwrap())
        .collect();
    s.file_notes(&[n[0]], k, C).unwrap();
    s.file_notes_in_epic(&[n[1]], e, C).unwrap();
    s.dismiss_notes(&[n[2]], H).unwrap();
    assert_eq!(s.inbox(p, all()).unwrap().total, 0);
    assert_eq!(s.projects().unwrap()[0].inbox, 0);
    let filed = s.filed_notes(p, all()).unwrap();
    let fates: Vec<_> = filed.items.iter().map(|n| n.fate()).collect();
    assert_eq!(fates.len(), 3);
    assert!(fates.contains(&NoteFate::Topic(k)));
    assert!(fates.contains(&NoteFate::Epic(e)));
    assert!(fates.contains(&NoteFate::Dismissed));
    assert_eq!(s.note(n[1]).unwrap().filed_by, Some(C));
    for id in &n {
        s.unfile_note(*id, H).unwrap();
        let note = s.note(*id).unwrap();
        assert!(note.is_open() && note.filed_by.is_none());
    }
    assert_eq!(s.inbox(p, all()).unwrap().total, 3);
    assert_eq!(s.filed_notes(p, all()).unwrap().total, 0);
}

#[test]
fn order_index_follows_the_stored_order_and_folder_paths_set_together() {
    let mut t = open();
    let s = &mut t.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let a = topic(s, p, None, "A");
    let b = topic(s, p, None, "B");
    assert_eq!(s.order_index(Record::Topic(b)).unwrap(), 1);
    s.reorder(Record::Topic(b), 0, H).unwrap();
    assert_eq!(s.order_index(Record::Topic(a)).unwrap(), 1);

    let dir = tempfile::tempdir().unwrap();
    let f1 = s.add_folder(Owner::Project(p), dir.path(), H).unwrap();
    let gone = if cfg!(windows) {
        "D:\\gone\\acta"
    } else {
        "/gone/acta"
    };
    let stored = crate::folder_path::normalize(crate::paths::Os::CURRENT, gone);
    s.set_folder_paths(&[(f1, stored.clone())], H).unwrap();
    assert_eq!(s.folder(f1).unwrap().path, stored);

    // Not in stored form, or another folder of the same owner: refused.
    let unnormal = format!("{stored}/");
    assert!(s.set_folder_paths(&[(f1, unnormal)], H).is_err());
    let other = tempfile::tempdir().unwrap();
    let f2 = s.add_folder(Owner::Project(p), other.path(), H).unwrap();
    assert!(s.set_folder_paths(&[(f2, stored.clone())], H).is_err());
    assert_ne!(s.folder(f2).unwrap().path, stored);
}

#[test]
fn a_panic_inside_atomically_rolls_back_and_leaves_autocommit() {
    let mut t = open();
    let s = &mut t.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let a = topic(s, p, None, "A");
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<()> = s.atomically(|s| {
            s.update_topic(
                a,
                TopicPatch {
                    title: Some("Renamed".into()),
                    ..TopicPatch::default()
                },
                H,
            )?;
            panic!("in the middle");
        });
    }));
    assert!(caught.is_err());
    assert!(s.conn.is_autocommit(), "no transaction left open");
    assert_eq!(s.topic(a).unwrap().title, "A");
    s.update_topic(
        a,
        TopicPatch {
            title: Some("B".into()),
            ..TopicPatch::default()
        },
        H,
    )
    .unwrap();
}

#[test]
fn a_version_1_database_with_notes_migrates_to_version_2() {
    let dir = tempfile::tempdir().unwrap();
    let keys = crate::keys::MemoryKeyStore::new();
    let config = StoreConfig {
        data_dir: dir.path().to_path_buf(),
        keys: keys.stores(),
    };
    // A database written by 0.3.0: schema 1, one open and one filed note.
    let mut v1 = Store::open_with(&config, &crate::schema::MIGRATIONS[..1]).unwrap();
    assert_eq!(crate::db::user_version(&v1.conn).unwrap(), 1);
    let p = v1.create_project("Acta", "", H).unwrap();
    let k = topic(&mut v1, p, None, "Keys");
    for (uuid, text, filed) in [
        ("n1", "Open note by Alexandre", None),
        ("n2", "Filed note", Some(k.0)),
    ] {
        v1.conn
            .execute(
                "INSERT INTO notes (uuid, project_id, text, author, filed_topic_id, filed_at,
                                    created_at, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, 'human', ?4, CASE WHEN ?4 IS NULL THEN NULL ELSE 5 END, 1, 1, 'human')",
                rusqlite::params![uuid, p.0, text, filed],
            )
            .unwrap();
    }
    drop(v1);
    let s = Store::open(&config).unwrap();
    assert_eq!(
        crate::db::user_version(&s.conn).unwrap(),
        crate::schema::SCHEMA_VERSION
    );
    let open_notes = s.inbox(p, all()).unwrap();
    assert_eq!(open_notes.total, 1);
    assert_eq!(open_notes.items[0].fate(), NoteFate::Open);
    assert_eq!(s.projects().unwrap()[0].inbox, 1);
    let filed = s.filed_notes(p, all()).unwrap();
    assert_eq!(filed.total, 1);
    assert_eq!(filed.items[0].fate(), NoteFate::Topic(k));
    assert_eq!(filed.items[0].filed_by, None);
}

#[test]
fn the_last_claude_write_names_the_topic_claude_code_touched_last() {
    let mut t = open();
    let p = t.store.create_project("Acta", "", H).unwrap();
    let a = topic(&mut t.store, p, None, "Embed the terminal");
    assert_eq!(t.store.last_claude_write(p).unwrap(), None);
    t.store
        .log(a, entry(EntryKind::Progress, "Alexandre's note"), C)
        .unwrap();
    let (id, title, _) = t.store.last_claude_write(p).unwrap().unwrap();
    assert_eq!((id, title.as_str()), (a, "Embed the terminal"));
}

#[test]
fn an_entry_is_edited_in_place_with_its_previous_wording_kept() {
    let mut t = open();
    let p = t.store.create_project("Acta", "", H).unwrap();
    let a = topic(&mut t.store, p, None, "Embed the terminal");
    let e = t
        .store
        .log(a, entry(EntryKind::Progress, "Alexandre wrote this"), H)
        .unwrap();
    t.store
        .update_entry(e.id, "Alexandre decided this", EntryKind::Decision, C)
        .unwrap();
    let back = t.store.entry(e.id).unwrap();
    assert_eq!(
        (back.text.as_str(), back.kind),
        ("Alexandre decided this", EntryKind::Decision)
    );
    assert_eq!(
        (back.seq, back.author, back.stamp.updated_by),
        (e.seq, H, C)
    );
    // The previous wording is in an `edited` system event.
    let all = t
        .store
        .entries(
            a,
            &EntryFilter {
                include_system: true,
                ..EntryFilter::default()
            },
            PageReq::first(50),
        )
        .unwrap()
        .items;
    let edited = all
        .iter()
        .find(|x| {
            x.event
                .as_ref()
                .is_some_and(|ev| ev.kind == EventKind::Edited)
        })
        .expect("an edited event");
    let old = edited.event.as_ref().unwrap().old.as_deref().unwrap();
    assert!(
        old.contains("Alexandre wrote this") && old.contains("progress"),
        "{old}"
    );
    assert!(
        t.store
            .update_entry(e.id, "", EntryKind::Progress, H)
            .is_err()
    );
    assert!(
        t.store
            .update_entry(e.id, "x", EntryKind::System, H)
            .is_err()
    );
    assert!(matches!(
        t.store
            .update_entry(EntryId(9999), "x", EntryKind::Progress, H),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn an_entry_edit_is_refused_on_a_read_only_database() {
    let mut t = open();
    let p = t.store.create_project("Acta", "", H).unwrap();
    let a = topic(&mut t.store, p, None, "Keys");
    let e = t
        .store
        .log(a, entry(EntryKind::Progress, "Alexandre"), H)
        .unwrap();
    t.store.writable = false;
    assert!(matches!(
        t.store
            .update_entry(e.id, "changed", EntryKind::Progress, H),
        Err(Error::ReadOnly)
    ));
    assert_eq!(t.store.entry(e.id).unwrap().text, "Alexandre");
}

// ---- live folders (ADR 0008, "Live files") ----

fn linked(store_dir: &std::path::Path, path: &str) -> bool {
    crate::link_index::LinkIndex::read(store_dir)
        .is_some_and(|i| i.matches(crate::paths::Os::CURRENT, &[path.to_owned()]))
}

#[test]
fn a_renamed_folder_is_followed_with_its_links() {
    use crate::folder_identity::Monitor;
    let TestStore { mut store, dir, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let repo = dir.path().join("repos").join("acta");
    std::fs::create_dir_all(&repo).unwrap();
    let f = store.add_folder(Owner::Project(p), &repo, H).unwrap();
    let old = store.folder(f).unwrap().path;
    let known = store.monitored_folders().unwrap();
    assert!(known[0].identity.is_some(), "remembered when attached");
    store
        .set_local_value(&format!("{AUTO_MEMORY_PREFIX}{old}"), Some("created"))
        .unwrap();
    assert!(linked(dir.path(), &old));

    let mut monitor = Monitor::new();
    assert!(!monitor.pass(&known, false).has_writes());
    let renamed = dir.path().join("repos").join("acta-alexandre");
    std::fs::rename(&repo, &renamed).unwrap();
    let pass = monitor.pass(&known, false);
    let moved = store
        .apply_folder_pass(&pass.learned, &pass.followed)
        .unwrap();
    assert_eq!(moved, [f]);

    let new = crate::folder_path::canonical(&renamed).unwrap();
    assert_eq!(store.folder(f).unwrap().path, new);
    assert_eq!(
        store.monitored_folders().unwrap()[0].identity,
        known[0].identity,
        "the identity stays with the folder"
    );
    assert_eq!(
        store
            .local_value(&format!("{AUTO_MEMORY_PREFIX}{new}"))
            .unwrap()
            .as_deref(),
        Some("created"),
        "the auto-memory marker follows"
    );
    assert_eq!(
        store
            .local_value(&format!("{AUTO_MEMORY_PREFIX}{old}"))
            .unwrap(),
        None
    );
    assert!(linked(dir.path(), &new), "hooks find the new path");
    assert!(!linked(dir.path(), &old));
    assert_eq!(
        store.resolve_folder(&renamed).unwrap().map(|r| r.project),
        Some(p)
    );

    // Applying the same pass again changes nothing: the path moved since.
    assert!(
        store
            .apply_folder_pass(&[], &pass.followed)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_moved_folder_without_a_proven_identity_stays_not_found() {
    use crate::folder_identity::Monitor;
    let TestStore { mut store, dir, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let repo = dir.path().join("acta");
    std::fs::create_dir_all(&repo).unwrap();
    let f = store.add_folder(Owner::Project(p), &repo, H).unwrap();
    let old = store.folder(f).unwrap().path;
    // As after an import from another machine: no identity on record.
    store
        .set_local_value(&format!("folderid.{}", f.0), None)
        .unwrap();
    let known = store.monitored_folders().unwrap();
    assert_eq!(known[0].identity, None);
    std::fs::rename(&repo, dir.path().join("acta-2")).unwrap();
    let pass = Monitor::new().pass(&known, true);
    assert!(!pass.has_writes());
    assert_eq!(store.folder(f).unwrap().path, old);

    // A pass that read the folder before a Locate does not undo it.
    let located = crate::folder_path::canonical(&dir.path().join("acta-2")).unwrap();
    store.set_folder_paths(&[(f, located.clone())], H).unwrap();
    let stale = crate::folder_identity::Followed {
        id: f,
        old: old.clone(),
        new: old,
        identity: "v1:1:2:-".into(),
    };
    assert!(store.apply_folder_pass(&[], &[stale]).unwrap().is_empty());
    assert_eq!(store.folder(f).unwrap().path, located);
}

#[test]
fn locate_and_its_undo_rewrite_the_hooks_pre_check() {
    let TestStore { mut store, dir, .. } = open();
    let p = store.create_project("Acta", "", H).unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let f = store.add_folder(Owner::Project(p), &a, H).unwrap();
    let old = store.folder(f).unwrap().path;
    let new = crate::folder_path::canonical(&b).unwrap();
    store.set_folder_paths(&[(f, new.clone())], H).unwrap();
    assert!(linked(dir.path(), &new));
    assert!(!linked(dir.path(), &old));
    store.restore_folder_path(f, &old, H).unwrap();
    assert!(linked(dir.path(), &old));
    assert!(!linked(dir.path(), &new));
}
