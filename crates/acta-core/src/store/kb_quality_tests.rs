//! Memory quality in the store (ADR 0015, phase 1 and the store side of
//! phase 2).

use super::test_support::{TestStore, open};
use super::*;
use crate::date::{Date, Zone};
use crate::kb_quality::{KbFlag, Text, Verdict};
use crate::model::*;
use crate::time::{MS_PER_DAY, now_ms};

const H: Author = Author::Human;
const C: Author = Author::Claude;

struct Seed {
    ts: TestStore,
    p: ProjectId,
    e: EpicId,
    t: TopicId,
    /// Another topic of the same epic.
    u: TopicId,
}

fn seed() -> Seed {
    let mut ts = open();
    let s = &mut ts.store;
    let p = s.create_project("Alexandre's app", "", H).unwrap();
    let e = s.create_epic(p, "Storage", "", H).unwrap();
    let topic = |s: &mut Store, title: &str| {
        s.create_topic(
            NewTopic {
                project: p,
                epic: Some(e),
                title: title.into(),
                description: String::new(),
                status: Status::Active,
            },
            H,
        )
        .unwrap()
    };
    let t = topic(s, "Encryption");
    let u = topic(s, "Backups");
    Seed { ts, p, e, t, u }
}

fn memory(owner: Owner, title: &str, summary: &str, body: &str) -> NewKb {
    NewKb {
        owner,
        kind: KbKind::Memory,
        title: title.into(),
        summary: summary.into(),
        body: body.into(),
        from_file: false,
    }
}

fn mem(s: &mut Store, owner: Owner, title: &str) -> KbId {
    s.kb_create(memory(owner, title, &format!("About {title}"), "Text"), C)
        .unwrap()
}

fn scope_ids(s: &Store, owner: Owner) -> Vec<KbId> {
    s.kb_scope(owner, &KbFilter::default(), PageReq::first(100))
        .unwrap()
        .items
        .into_iter()
        .map(|k| k.id)
        .collect()
}

fn changes(s: &Store, id: KbId) -> Vec<ChangeKind> {
    s.kb_history(id, PageReq::first(100))
        .unwrap()
        .items
        .into_iter()
        .map(|r| r.change)
        .collect()
}

fn set_verified(s: &Store, id: KbId, at: i64) {
    s.conn
        .execute(
            "UPDATE kb_entries SET verified_at = ?2 WHERE id = ?1",
            params![id.0, at],
        )
        .unwrap();
}

use rusqlite::params;

#[test]
fn the_migration_backfills_the_verified_date_from_the_last_update() {
    let dir = tempfile::tempdir().unwrap();
    let keys = crate::keys::MemoryKeyStore::new();
    let config = StoreConfig {
        data_dir: dir.path().to_path_buf(),
        keys: keys.stores(),
    };
    let (k, updated) = {
        let mut old = Store::open_with(&config, &crate::schema::MIGRATIONS[..6]).unwrap();
        let p = old.create_project("Alexandre's app", "", H).unwrap();
        // As schema 6 wrote it (0.6.0).
        old.conn
            .execute(
                "INSERT INTO kb_entries (uuid, project_id, kind, title, summary, body, rev, created_by,
                                         created_at, updated_at, updated_by)
                 VALUES ('k1', ?1, 'memory', 'Keychain entry name', 'x', 'y', 1, 'claude', 500, 1000, 'human')",
                [p.0],
            )
            .unwrap();
        (KbId(old.conn.last_insert_rowid()), 1000)
    };
    let s = Store::open(&config).unwrap();
    // A 0.6 build (schema 6) now opens it read-only: the minimum it
    // compares its own schema with is 7.
    let min: String = s
        .conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'min_compatible_version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(min, "7");
    let e = s.kb_entry(k).unwrap();
    assert_eq!((e.verified_at, e.verified_by), (updated, H));
    assert_eq!(e.state, KbState::Live);
    assert_eq!(e.superseded_by, None);
    // An older build keeps writing rows without a verified date: the
    // last update stands in.
    s.conn
        .execute(
            "UPDATE kb_entries SET verified_at = NULL, verified_by = NULL",
            [],
        )
        .unwrap();
    assert_eq!(s.kb_entry(k).unwrap().verified_at, updated);
}

#[test]
fn only_content_writes_move_the_verified_date() {
    let Seed { mut ts, p, e, .. } = seed();
    let s = &mut ts.store;
    let k = mem(s, Owner::Project(p), "Keychain entry name");
    let created = s.kb_entry(k).unwrap();
    assert_eq!(created.verified_at, created.stamp.created_at);
    assert_eq!(created.verified_by, C);
    set_verified(s, k, 5);
    // Title, summary, move: not an assertion that the text is true.
    s.kb_update(
        k,
        KbUpdate {
            expected_rev: None,
            title: Some("Keychain entry".into()),
            summary: Some("The entry's name".into()),
            edit: KbEdit::Body("Text".into()),
        },
        H,
    )
    .unwrap();
    s.kb_move(k, Owner::Epic(e), H).unwrap();
    assert_eq!(s.kb_entry(k).unwrap().verified_at, 5);
    // A body write is.
    s.kb_update(
        k,
        KbUpdate {
            expected_rev: None,
            title: None,
            summary: None,
            edit: KbEdit::Body("acta.db".into()),
        },
        H,
    )
    .unwrap();
    let e1 = s.kb_entry(k).unwrap();
    assert!(e1.verified_at > 5);
    assert_eq!(e1.verified_by, H);
    // So is a restore.
    set_verified(s, k, 5);
    s.kb_restore_revision(k, 1, C).unwrap();
    assert!(s.kb_entry(k).unwrap().verified_at > 5);
}

#[test]
fn verify_vouches_without_new_text_and_refuses_a_stale_revision() {
    let Seed { mut ts, p, .. } = seed();
    let s = &mut ts.store;
    let k = mem(s, Owner::Project(p), "Keychain entry name");
    set_verified(s, k, 5);
    let ctx = KbWriteCtx {
        session: Some("session-1".into()),
        branch: Some("feature/kb".into()),
        ..KbWriteCtx::default()
    };
    assert!(matches!(
        s.kb_verify(k, Some(7), &[], &ctx, C),
        Err(crate::Error::StaleRevision { current: 1 })
    ));
    let rev = s.kb_verify(k, Some(1), &[], &ctx, C).unwrap().rev;
    assert_eq!(rev, 2);
    let e = s.kb_entry(k).unwrap();
    assert!(e.verified_at > 5);
    assert_eq!(e.body, "Text");
    assert_eq!(changes(s, k), [ChangeKind::Verified, ChangeKind::Created]);
    let (session, branch): (Option<String>, Option<String>) = s
        .conn
        .query_row(
            "SELECT session, branch FROM kb_revisions WHERE entry_id = ?1 AND rev = 2",
            [k.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (session.as_deref(), branch.as_deref()),
        (Some("session-1"), Some("feature/kb"))
    );
}

#[test]
fn supersede_retire_and_make_live_are_revisions_and_hide_from_scope_and_search() {
    let Seed { mut ts, p, t, .. } = seed();
    let s = &mut ts.store;
    let old = s
        .kb_create(
            memory(
                Owner::Topic(t),
                "Tests run in parallel",
                "cargo test runs every test concurrently",
                "Safe to run in parallel.",
            ),
            C,
        )
        .unwrap();
    let gone = mem(s, Owner::Project(p), "Old release script");
    // A pair the checker found between them is resolved by the supersede.
    let (new, rev, _) = s
        .kb_supersede(
            KbSuccessor::Create(memory(
                Owner::Topic(t),
                "Store tests run serially",
                "The store tests share a key store",
                "Never run them in parallel.",
            )),
            &[old],
            &KbWriteCtx::default(),
            C,
        )
        .unwrap();
    assert_eq!(rev, 1);
    let o = s.kb_entry(old).unwrap();
    assert_eq!((o.state, o.superseded_by), (KbState::Superseded, Some(new)));
    assert_eq!(changes(s, old)[0], ChangeKind::Superseded);
    s.kb_retire(
        gone,
        Some(1),
        "  replaced by   the CI job ",
        &KbWriteCtx::default(),
        H,
    )
    .unwrap();
    let g = s.kb_entry(gone).unwrap();
    assert_eq!(g.state, KbState::Retired);
    let note = s.kb_revision(gone, 2).unwrap();
    assert_eq!(
        (note.change, note.section.as_deref()),
        (ChangeKind::Retired, Some("replaced by the CI job"))
    );
    // Neither appears in the scope, its counts or the agent's search; both
    // still read by id and count as inactive.
    assert_eq!(scope_ids(s, Owner::Topic(t)), [new]);
    assert_eq!(s.kb_counts(Owner::Topic(t)).unwrap().memories, 1);
    assert_eq!(s.kb_inactive_counts(Owner::Topic(t)).unwrap(), (1, 1));
    let hits = s
        .kb_search(Owner::Topic(t), "parallel", None, PageReq::first(10))
        .unwrap();
    assert_eq!(
        hits.items.iter().map(|h| h.record).collect::<Vec<_>>(),
        [Record::Kb(new)]
    );
    let inactive = s
        .kb_rollup(
            Owner::Project(p),
            &KbFilter {
                states: KbStates::Inactive,
                ..KbFilter::default()
            },
            PageReq::first(10),
        )
        .unwrap();
    assert_eq!(inactive.total, 2);
    // State changes are refused on a non-live entry, and checked.
    assert!(
        s.kb_verify(old, None, &[], &KbWriteCtx::default(), C)
            .is_err()
    );
    assert!(
        s.kb_retire(new, None, &"x".repeat(201), &KbWriteCtx::default(), C)
            .is_err()
    );
    assert!(
        s.kb_supersede(
            KbSuccessor::Update(
                new,
                KbUpdate {
                    expected_rev: None,
                    title: None,
                    summary: None,
                    edit: KbEdit::Keep
                }
            ),
            &[old],
            &KbWriteCtx::default(),
            C
        )
        .is_err(),
        "already superseded"
    );
    // A refused supersede writes nothing (one transaction).
    let before = scope_ids(s, Owner::Project(p)).len();
    assert!(
        s.kb_supersede(
            KbSuccessor::Create(memory(Owner::Topic(t), "Orphan", "x", "y")),
            &[KbId(999)],
            &KbWriteCtx::default(),
            C
        )
        .is_err()
    );
    assert_eq!(scope_ids(s, Owner::Project(p)).len(), before);
    // Make live again: both come back, one revision each.
    s.kb_make_live(old, H).unwrap();
    s.kb_make_live(gone, H).unwrap();
    assert!(s.kb_make_live(gone, H).is_err());
    let o = s.kb_entry(old).unwrap();
    assert_eq!((o.state, o.superseded_by), (KbState::Live, None));
    assert_eq!(changes(s, gone)[0], ChangeKind::MadeLive);
    assert_eq!(scope_ids(s, Owner::Topic(t)).len(), 3);
}

#[test]
fn merge_lives_at_the_wider_level_copies_sources_and_unmerges_in_one_step() {
    let Seed {
        mut ts, p, e, t, u, ..
    } = seed();
    let s = &mut ts.store;
    let survivor = s
        .kb_create(
            memory(
                Owner::Topic(t),
                "Keychain entry",
                "Name of the entry",
                "acta.db",
            ),
            C,
        )
        .unwrap();
    let other = s
        .kb_create(
            memory(
                Owner::Topic(u),
                "Keychain entry name",
                "The entry's name",
                "It is acta.db.",
            ),
            C,
        )
        .unwrap();
    s.kb_add_links(
        other,
        &[
            KbSource::Record(Record::Topic(u)),
            KbSource::Commit("9E92BA4".into()),
        ],
        C,
    )
    .unwrap();
    // The survivor already names the topic: not copied twice.
    s.kb_add_links(survivor, &[KbSource::Record(Record::Topic(u))], C)
        .unwrap();
    s.kb_record_pair(survivor, other, KbPairKind::Duplicate, 0.8, C)
        .unwrap();
    let before = s.kb_entry(survivor).unwrap();
    let trace = s.kb_merge(survivor, other, H).unwrap();
    let m = s.kb_entry(survivor).unwrap();
    // Two topics of one epic: the merge lives at the epic.
    assert_eq!(m.owner, Owner::Epic(e));
    assert_eq!(m.body, "acta.db\n\nIt is acta.db.");
    assert_eq!(changes(s, survivor)[0], ChangeKind::Merged);
    let o = s.kb_entry(other).unwrap();
    assert_eq!(
        (o.state, o.superseded_by),
        (KbState::Superseded, Some(survivor))
    );
    assert_eq!(trace.copied_links.len(), 1);
    let links = s.kb_links(survivor).unwrap();
    assert_eq!(links.len(), 2);
    assert!(
        links
            .iter()
            .any(|l| l.target == KbLinkTarget::Text("9e92ba4".into()))
    );
    // The other keeps its own.
    assert_eq!(s.kb_links(other).unwrap().len(), 2);
    let pairs = s.kb_pairs_of(survivor).unwrap();
    assert_eq!(pairs[0].state, KbPairState::Resolved);
    // Undo: everything as it was, the other live again.
    s.kb_unmerge(&trace, H).unwrap();
    let back = s.kb_entry(survivor).unwrap();
    assert_eq!(
        (back.owner, back.body.as_str(), back.verified_at),
        (Owner::Topic(t), "acta.db", before.verified_at)
    );
    assert_eq!(s.kb_entry(other).unwrap().state, KbState::Live);
    assert_eq!(s.kb_links(survivor).unwrap().len(), 1);
    assert_eq!(s.kb_pairs_of(survivor).unwrap()[0].state, KbPairState::Open);
    // An unmerge after the survivor changed again is refused.
    let trace = s.kb_merge(survivor, other, H).unwrap();
    s.kb_verify(survivor, None, &[], &KbWriteCtx::default(), H)
        .unwrap();
    assert!(s.kb_unmerge(&trace, H).is_err());
    // A project-level and a topic-level memory merge at the project.
    let a = mem(s, Owner::Project(p), "Release script");
    let b = mem(s, Owner::Topic(t), "Release script steps");
    s.kb_merge(b, a, H).unwrap();
    assert_eq!(s.kb_entry(b).unwrap().owner, Owner::Project(p));
    assert!(s.kb_merge(b, b, H).is_err());
}

#[test]
fn sources_are_checked_and_one_folder_becomes_the_context() {
    let Seed { mut ts, p, t, .. } = seed();
    let repo = ts.dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let s = &mut ts.store;
    let f = s.add_folder(Owner::Project(p), &repo, H).unwrap();
    let ctx = KbWriteCtx {
        session: Some("abc".into()),
        topic: Some(t),
        branch: Some("feature/kb".into()),
        links: vec![
            KbSource::File {
                folder: f,
                path: "src\\store\\kb.rs".into(),
                blob: Some("0123abcd".into()),
            },
            KbSource::Record(Record::Topic(t)),
        ],
        ..KbWriteCtx::default()
    };
    let k = s
        .kb_create_with(
            memory(Owner::Topic(t), "Store layout", "Where kb lives", "x"),
            &ctx,
            C,
        )
        .unwrap();
    let e = s.kb_entry(k).unwrap();
    assert_eq!(e.context_folder, Some(f));
    assert_eq!(e.created_session.as_deref(), Some("abc"));
    assert_eq!(e.created_topic, Some(t));
    let links = s.kb_links(k).unwrap();
    assert_eq!(
        links[0].target,
        KbLinkTarget::Text("src/store/kb.rs".into())
    );
    assert_eq!(links[1].target, KbLinkTarget::Record(Record::Topic(t)));
    // Verify records a new blob id.
    s.kb_verify(
        k,
        None,
        &[(links[0].id, Some("4567ef".into()))],
        &KbWriteCtx::default(),
        C,
    )
    .unwrap();
    assert_eq!(s.kb_links(k).unwrap()[0].blob.as_deref(), Some("4567ef"));
    for bad in [
        KbSource::Commit("xyz".into()),
        KbSource::Record(Record::Project(p)),
        KbSource::Record(Record::Topic(TopicId(999))),
        KbSource::File {
            folder: f,
            path: "../secret".into(),
            blob: None,
        },
        KbSource::File {
            folder: f,
            path: "C:/x".into(),
            blob: None,
        },
    ] {
        assert!(
            s.kb_add_links(k, std::slice::from_ref(&bad), C).is_err(),
            "{bad:?}"
        );
    }
    // A branch context needs its folder.
    let bad = KbWriteCtx {
        context_branch: Some("feature/x".into()),
        ..KbWriteCtx::default()
    };
    assert!(
        s.kb_create_with(memory(Owner::Topic(t), "B", "b", "b"), &bad, C)
            .is_err()
    );
}

#[test]
fn pairs_keep_a_distinct_decision_and_the_log_prunes_old_rows() {
    let Seed { mut ts, p, .. } = seed();
    let s = &mut ts.store;
    let a = mem(s, Owner::Project(p), "One");
    let b = mem(s, Owner::Project(p), "Two");
    assert_eq!(
        s.kb_record_pair(b, a, KbPairKind::Conflict, 0.5, C)
            .unwrap(),
        KbPairState::Open
    );
    s.kb_set_pair_state(a, b, KbPairKind::Conflict, KbPairState::Distinct, C)
        .unwrap();
    // Found again: stays kept apart.
    assert_eq!(
        s.kb_record_pair(a, b, KbPairKind::Conflict, 0.6, C)
            .unwrap(),
        KbPairState::Distinct
    );
    assert!(
        s.kb_record_pair(a, a, KbPairKind::Conflict, 0.6, C)
            .is_err()
    );
    s.kb_log(KbLogEvent::ConflictWarning, Some(a), Some(b), Some("s1"), C)
        .unwrap();
    s.conn
        .execute(
            "INSERT INTO kb_quality_log (at, event, author) VALUES (?1, 'kept_both', 'claude')",
            [now_ms() - (QUALITY_LOG_DAYS + 1) * MS_PER_DAY],
        )
        .unwrap();
    s.kb_log(KbLogEvent::KeptBoth, Some(a), Some(b), Some("s1"), C)
        .unwrap();
    let rows = s.kb_log_since(0).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.event).collect::<Vec<_>>(),
        [KbLogEvent::ConflictWarning, KbLogEvent::KeptBoth]
    );
    // The log is local: an export carries pairs but never the log.
    let file = s.export(None).unwrap();
    assert_eq!(file.kb_pairs.len(), 1);
    assert!(!file.to_json().unwrap().contains("kept_both"));
}

#[test]
fn the_round_trip_keeps_quality_fields_and_an_older_file_imports_live() {
    let Seed {
        mut ts, p, t, u, ..
    } = seed();
    let repo = ts.dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let s = &mut ts.store;
    let f = s.add_folder(Owner::Project(p), &repo, H).unwrap();
    let ctx = KbWriteCtx {
        session: Some("local-only".into()),
        topic: Some(t),
        branch: Some("feature/kb".into()),
        context_folder: Some(f),
        context_branch: Some("feature/kb".into()),
        links: vec![
            KbSource::Commit("9e92ba4".into()),
            KbSource::Record(Record::Topic(u)),
        ],
        replace_links: false,
    };
    let old = mem(s, Owner::Topic(t), "Old fact");
    let (new, _, _) = s
        .kb_supersede(
            KbSuccessor::Create(memory(Owner::Topic(t), "New fact", "Replaces it", "x")),
            &[old],
            &ctx,
            C,
        )
        .unwrap();
    let other = mem(s, Owner::Topic(t), "Another fact");
    s.kb_record_pair(new, other, KbPairKind::Duplicate, 0.75, C)
        .unwrap();
    let file = s.export(None).unwrap();
    assert_eq!(file.version, FORMAT_VERSION);
    let json = file.to_json().unwrap();
    assert!(!json.contains("local-only"), "session ids are not exported");

    let mut other_db = open();
    other_db
        .store
        .import(&ExportFile::from_json(&json).unwrap(), false)
        .unwrap();
    let o = &other_db.store;
    let id = |title: &str| -> KbId {
        KbId(
            o.conn
                .query_row("SELECT id FROM kb_entries WHERE title = ?1", [title], |r| {
                    r.get(0)
                })
                .unwrap(),
        )
    };
    let n = o.kb_entry(id("New fact")).unwrap();
    let src = s.kb_entry(new).unwrap();
    assert_eq!(
        (n.verified_at, n.verified_by),
        (src.verified_at, src.verified_by)
    );
    assert_eq!(n.context_branch.as_deref(), Some("feature/kb"));
    assert!(n.context_folder.is_some());
    assert!(n.created_topic.is_some());
    assert_eq!(n.created_session, None);
    let ol = o.kb_entry(id("Old fact")).unwrap();
    assert_eq!(
        (ol.state, ol.superseded_by),
        (KbState::Superseded, Some(n.id))
    );
    assert_eq!(o.kb_links(n.id).unwrap().len(), 2);
    assert_eq!(o.kb_pairs_of(n.id).unwrap().len(), 1);
    let branch: Option<String> = o
        .conn
        .query_row(
            "SELECT branch FROM kb_revisions WHERE entry_id = ?1 AND rev = 1",
            [n.id.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(branch.as_deref(), Some("feature/kb"));
    // Importing the same file again changes nothing.
    let again = other_db
        .store
        .import(&ExportFile::from_json(&json).unwrap(), false)
        .unwrap();
    assert_eq!(again.knowledge.created + again.knowledge.updated, 0);
    assert_eq!(other_db.store.kb_links(n.id).unwrap().len(), 2);
}

#[test]
fn the_prefilter_covers_ancestors_and_descendants_and_the_check_sorts() {
    let Seed {
        mut ts, p, e, t, u, ..
    } = seed();
    let s = &mut ts.store;
    let at_project = s
        .kb_create(
            memory(
                Owner::Project(p),
                "Store tests in parallel",
                "The store tests run in parallel",
                "cargo test runs the store tests in parallel.",
            ),
            C,
        )
        .unwrap();
    let at_topic = s
        .kb_create(
            memory(
                Owner::Topic(t),
                "Keychain entry name",
                "The key is stored under acta.db",
                "The keychain entry is named acta.db.",
            ),
            C,
        )
        .unwrap();
    // Another topic's memory is not in a topic's shared scope.
    let elsewhere = s
        .kb_create(
            memory(
                Owner::Topic(u),
                "Keychain entry name",
                "The key is stored under acta.db",
                "The keychain entry is named acta.db.",
            ),
            C,
        )
        .unwrap();
    let draft = Text {
        title: "Keychain entry name",
        summary: "The key is stored under acta.db",
        body: "The keychain entry is named acta.db.",
    };
    // From the epic: its descendants (both topics).
    let from_epic: Vec<KbId> = s
        .kb_candidates(Owner::Epic(e), &draft, None)
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert!(from_epic.contains(&at_topic) && from_epic.contains(&elsewhere));
    // From the topic: own and ancestors only.
    let found = s.kb_check(Owner::Topic(t), &draft, None).unwrap();
    assert_eq!(found[0].candidate.id, at_topic);
    assert_eq!(found[0].comparison.verdict, Verdict::Same);
    assert!(found.iter().all(|f| f.candidate.id != elsewhere));
    // A conflict with the project's memory, seen from the topic.
    let conflict = Text {
        title: "Store tests in parallel",
        summary: "Never run the store tests in parallel",
        body: "They share a key store.",
    };
    let found = s.kb_check(Owner::Topic(t), &conflict, None).unwrap();
    assert_eq!(found[0].candidate.id, at_project);
    assert_eq!(found[0].comparison.verdict, Verdict::Conflict);
    // Kept apart, it is not found again for an edit of the other side.
    s.kb_set_pair_state(
        at_topic,
        at_project,
        KbPairKind::Conflict,
        KbPairState::Distinct,
        C,
    )
    .unwrap();
    let found = s
        .kb_check(Owner::Topic(t), &conflict, Some(at_topic))
        .unwrap();
    assert!(found.iter().all(|f| f.candidate.id != at_project));
    // Nothing to look for.
    let empty = Text {
        title: "the",
        summary: "a",
        body: "",
    };
    assert!(
        s.kb_check(Owner::Topic(t), &empty, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn store_flags_read_age_topic_pairs_and_folder() {
    let Seed { mut ts, p, t, .. } = seed();
    let repo = ts.dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let s = &mut ts.store;
    let f = s.add_folder(Owner::Project(p), &repo, H).unwrap();
    let ctx = KbWriteCtx {
        context_folder: Some(f),
        ..KbWriteCtx::default()
    };
    let k = s
        .kb_create_with(memory(Owner::Topic(t), "Old one", "x", "y"), &ctx, C)
        .unwrap();
    let dup = mem(s, Owner::Topic(t), "Twin");
    let gone = mem(s, Owner::Topic(t), "Gone twin");
    let fresh = mem(s, Owner::Project(p), "Fresh");
    let zone = Zone::UTC;
    let today = zone.today();
    set_verified(s, k, zone.midnight(today.add_days(-140)));
    s.kb_record_pair(k, dup, KbPairKind::Duplicate, 0.8, C)
        .unwrap();
    // An open pair with an entry that is no longer live is not a flag.
    s.kb_record_pair(k, gone, KbPairKind::Conflict, 0.5, C)
        .unwrap();
    s.kb_retire(gone, None, "", &KbWriteCtx::default(), C)
        .unwrap();
    s.update_topic(
        t,
        TopicPatch {
            status: Some(Status::Done),
            ..TopicPatch::default()
        },
        H,
    )
    .unwrap();
    s.delete(Record::Folder(f), H).unwrap();
    let flags = s
        .kb_store_flags(&[k, fresh, KbId(999)], zone, today)
        .unwrap();
    assert_eq!(
        flags[0].1,
        [
            KbFlag::Duplicate(dup),
            KbFlag::FolderRemoved,
            KbFlag::TopicClosed(Status::Done),
            KbFlag::Old { days: 140 }
        ]
    );
    assert!(flags[1].1.is_empty() && flags[2].1.is_empty());
    let _ = Date::from_days(0);
}

#[test]
fn the_overview_counts_memories_by_verified_date_and_live_state() {
    let Seed { mut ts, p, .. } = seed();
    let s = &mut ts.store;
    let zone = Zone::UTC;
    let today = zone.today();
    let long_ago = zone.midnight(today.add_days(-200));
    let a = mem(s, Owner::Project(p), "Verified long ago");
    let b = mem(s, Owner::Project(p), "Retired long ago");
    let c = mem(s, Owner::Project(p), "Edited long ago, verified now");
    for k in [a, b] {
        set_verified(s, k, long_ago);
    }
    s.conn
        .execute(
            "UPDATE kb_entries SET updated_at = ?2 WHERE id = ?1",
            params![c.0, long_ago],
        )
        .unwrap();
    s.kb_retire(b, None, "", &KbWriteCtx::default(), H).unwrap();
    set_verified(s, b, long_ago);
    let o = s.overview(Scope::Project(p), today, zone).unwrap();
    assert_eq!(o.stale_memories, 1);
}

/// The checker's cost at write time with 2,000 memories in scope:
/// reported, not asserted (ADR 0015, Consequences). Run with
/// `cargo test -p acta-core --release -- --ignored --nocapture kb_check_timing`.
#[test]
#[ignore]
fn kb_check_timing_with_2000_memories() {
    let Seed { mut ts, p, t, .. } = seed();
    let s = &mut ts.store;
    let words = [
        "keychain",
        "snapshot",
        "migration",
        "terminal",
        "export",
        "import",
        "folder",
        "branch",
        "commit",
        "release",
        "update",
        "theme",
        "font",
        "hook",
        "session",
        "budget",
        "clippy",
        "test",
        "parallel",
        "timeout",
        "port",
        "server",
        "client",
        "cache",
    ];
    s.atomically(|s| {
        for i in 0..2_000usize {
            let w = |k: usize| words[(i * 7 + k * 5) % words.len()];
            let title = format!("{} {} {}", w(0), w(1), i);
            let summary = format!("The {} uses the {} with {}", w(2), w(3), w(4));
            let body = format!(
                "Alexandre noted that the {} and the {} interact with the {} on {}.",
                w(5),
                w(6),
                w(7),
                i
            );
            s.kb_create(memory(Owner::Project(p), &title, &summary, &body), C)?;
        }
        Ok(())
    })
    .unwrap();
    let draft = Text {
        title: "keychain snapshot",
        summary: "The migration uses the terminal with export",
        body: "Alexandre noted that the folder and the branch interact with the commit.",
    };
    let runs = 50;
    let start = std::time::Instant::now();
    let mut found = 0;
    for _ in 0..runs {
        found = s.kb_check(Owner::Topic(t), &draft, None).unwrap().len();
    }
    println!(
        "kb_check over 2,000 memories: {:?} per write ({found} findings)",
        start.elapsed() / runs
    );
}

#[test]
fn an_unmerge_whose_old_owner_was_trashed_keeps_the_merged_level() {
    let Seed {
        mut ts, e, t, u, ..
    } = seed();
    let s = &mut ts.store;
    let a = mem(s, Owner::Topic(t), "Keychain entry");
    let b = mem(s, Owner::Topic(u), "Keychain entry name");
    let trace = s.kb_merge(a, b, H).unwrap();
    assert_eq!(s.kb_entry(a).unwrap().owner, Owner::Epic(e));
    s.delete(Record::Topic(t), H).unwrap();
    s.kb_unmerge(&trace, H).unwrap();
    assert_eq!(s.kb_entry(a).unwrap().owner, Owner::Epic(e));
    assert_eq!(s.kb_entry(b).unwrap().state, KbState::Live);
}

#[test]
fn hidden_entries_leave_counts_unread_and_global_search() {
    let Seed { mut ts, p, .. } = seed();
    let s = &mut ts.store;
    let k = s
        .kb_create(
            memory(
                Owner::Project(p),
                "Keychain entry",
                "Where the key is",
                "acta.db",
            ),
            C,
        )
        .unwrap();
    let before = s.projects().unwrap()[0].knowledge;
    s.kb_retire(k, None, "", &KbWriteCtx::default(), C).unwrap();
    assert_eq!(s.projects().unwrap()[0].knowledge, before - 1);
    let q = SearchQuery {
        text: "keychain".into(),
        project: Some(p),
        kinds: vec![SearchKind::Kb],
        ..SearchQuery::default()
    };
    assert_eq!(s.search(&q, PageReq::first(10)).unwrap().total, 0);
    assert!(s.unread().unwrap().kb.iter().all(|(id, _)| *id != k));
}

#[test]
fn the_overview_counts_every_store_reason() {
    let Seed { mut ts, p, t, .. } = seed();
    let repo = ts.dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let s = &mut ts.store;
    let zone = Zone::UTC;
    let today = zone.today();
    let count = |s: &Store| {
        s.overview(Scope::Project(p), today, zone)
            .unwrap()
            .stale_memories
    };
    let fresh = mem(s, Owner::Project(p), "Fresh");
    assert_eq!(count(s), 0);
    // An open pair with a live entry: both count; kept apart, neither.
    let twin = mem(s, Owner::Project(p), "Twin");
    s.kb_record_pair(fresh, twin, KbPairKind::Duplicate, 0.8, C)
        .unwrap();
    assert_eq!(count(s), 2);
    s.kb_set_pair_state(fresh, twin, KbPairKind::Duplicate, KbPairState::Distinct, C)
        .unwrap();
    assert_eq!(count(s), 0);
    // A closed topic.
    let on_topic = mem(s, Owner::Topic(t), "On the topic");
    s.update_topic(
        t,
        TopicPatch {
            status: Some(Status::Dropped),
            ..TopicPatch::default()
        },
        H,
    )
    .unwrap();
    assert_eq!(count(s), 1);
    // A context folder in the trash.
    let f = s.add_folder(Owner::Project(p), &repo, H).unwrap();
    s.kb_create_with(
        memory(Owner::Project(p), "In a folder", "x", "y"),
        &KbWriteCtx {
            context_folder: Some(f),
            ..KbWriteCtx::default()
        },
        C,
    )
    .unwrap();
    assert_eq!(count(s), 1);
    s.delete(Record::Folder(f), H).unwrap();
    assert_eq!(count(s), 2);
    // The list holds the same entries.
    let list = s
        .overview_list(
            Scope::Project(p),
            OverviewFilter::StaleMemories,
            today,
            zone,
        )
        .unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().any(|i| i.record == Record::Kb(on_topic)));
}

#[test]
fn every_state_change_has_its_undo() {
    let Seed { mut ts, p, t, .. } = seed();
    let repo = ts.dir.path().join("repo2");
    std::fs::create_dir_all(&repo).unwrap();
    let s = &mut ts.store;
    // Verify: the verified date and file records come back; the revision
    // stays in history.
    let f = s.add_folder(Owner::Project(p), &repo, H).unwrap();
    let k = s
        .kb_create_with(
            memory(Owner::Project(p), "Store layout", "Where kb lives", "x"),
            &KbWriteCtx {
                links: vec![KbSource::File {
                    folder: f,
                    path: "src/kb.rs".into(),
                    blob: Some("aaaa".into()),
                }],
                ..KbWriteCtx::default()
            },
            C,
        )
        .unwrap();
    set_verified(s, k, 5);
    let link = s.kb_links(k).unwrap()[0].id;
    let v = s
        .kb_verify(
            k,
            None,
            &[(link, Some("bbbb".into()))],
            &KbWriteCtx::default(),
            H,
        )
        .unwrap();
    assert_eq!(v.verified_at, 5);
    assert_eq!(v.blobs, vec![(link, Some("aaaa".to_owned()))]);
    s.kb_unverify(&v).unwrap();
    assert_eq!(s.kb_entry(k).unwrap().verified_at, 5);
    assert_eq!(s.kb_links(k).unwrap()[0].blob.as_deref(), Some("aaaa"));
    assert_eq!(changes(s, k)[0], ChangeKind::Verified);
    // Retire, then its undo: live again; a second undo is refused.
    let r = s.kb_retire(k, None, "", &KbWriteCtx::default(), H).unwrap();
    s.kb_revert_state(&r, H).unwrap();
    assert_eq!(s.kb_entry(k).unwrap().state, KbState::Live);
    assert!(s.kb_revert_state(&r, H).is_err());
    // This replaces it (no text edit), then its undo: the other live, the
    // pair open again, the survivor untouched.
    let other = mem(s, Owner::Topic(t), "Store layout, older");
    s.kb_record_pair(k, other, KbPairKind::Conflict, 0.5, C)
        .unwrap();
    let before_rev = s.kb_entry(k).unwrap().rev;
    let traces = s.kb_replace_with(k, &[other], H).unwrap();
    let o = s.kb_entry(other).unwrap();
    assert_eq!((o.state, o.superseded_by), (KbState::Superseded, Some(k)));
    assert_eq!(s.kb_pairs_of(k).unwrap()[0].state, KbPairState::Resolved);
    assert_eq!(s.kb_entry(k).unwrap().rev, before_rev);
    s.kb_revert_state(&traces[0], H).unwrap();
    assert_eq!(s.kb_entry(other).unwrap().state, KbState::Live);
    assert_eq!(s.kb_pairs_of(k).unwrap()[0].state, KbPairState::Open);
    // Make live, then its undo: superseded again, by the same successor.
    s.kb_replace_with(k, &[other], H).unwrap();
    let live = s.kb_make_live(other, H).unwrap();
    s.kb_revert_state(&live, H).unwrap();
    let o = s.kb_entry(other).unwrap();
    assert_eq!((o.state, o.superseded_by), (KbState::Superseded, Some(k)));
    // A retired entry cannot replace anything.
    let gone = mem(s, Owner::Project(p), "Gone");
    s.kb_retire(gone, None, "", &KbWriteCtx::default(), H)
        .unwrap();
    let third = mem(s, Owner::Project(p), "Third");
    assert!(s.kb_replace_with(gone, &[third], H).is_err());
}

fn body(text: &str) -> KbUpdate {
    KbUpdate {
        expected_rev: None,
        title: None,
        summary: None,
        edit: KbEdit::Body(text.into()),
    }
}

#[test]
fn a_merge_tidied_by_the_human_is_still_taken_back() {
    let Seed { mut ts, t, u, .. } = seed();
    let s = &mut ts.store;
    let a = s
        .kb_create(
            memory(Owner::Topic(t), "Keychain entry", "Name", "acta.db"),
            C,
        )
        .unwrap();
    let b = s
        .kb_create(
            memory(
                Owner::Topic(u),
                "Keychain entry name",
                "Name",
                "It is acta.db.",
            ),
            C,
        )
        .unwrap();
    let trace = s.kb_merge(a, b, H).unwrap();
    // The editor opened on the merge: the human tidies, twice.
    for text in ["acta.db (both said so).", "The entry is acta.db."] {
        s.kb_update(a, body(text), H).unwrap();
    }
    s.kb_unmerge(&trace, H).unwrap();
    assert_eq!(s.kb_entry(a).unwrap().body, "acta.db");
    assert_eq!(s.kb_entry(b).unwrap().state, KbState::Live);
    // An agent's edit after the merge refuses it.
    let trace = s.kb_merge(a, b, H).unwrap();
    s.kb_update(a, body("The agent's text."), C).unwrap();
    assert!(s.kb_unmerge(&trace, H).is_err());
    // So does a tidy outside the window.
    let c = mem(s, Owner::Topic(t), "Third");
    let trace = s.kb_merge(a, c, H).unwrap();
    s.conn
        .execute(
            "UPDATE kb_revisions SET created_at = created_at - ?2 WHERE entry_id = ?1 AND rev = ?3",
            params![a.0, UNMERGE_WINDOW_MS + 1, trace.merged_rev],
        )
        .unwrap();
    s.kb_update(a, body("Late tidy."), H).unwrap();
    assert!(s.kb_unmerge(&trace, H).is_err());
}

#[test]
fn a_branch_memory_can_be_made_true_everywhere_and_back() {
    let Seed { mut ts, p, t, .. } = seed();
    let repo = ts.dir.path().join("repo3");
    std::fs::create_dir_all(&repo).unwrap();
    let s = &mut ts.store;
    let f = s.add_folder(Owner::Project(p), &repo, H).unwrap();
    let k = s
        .kb_create_with(
            memory(Owner::Topic(t), "Schema at V7 here", "On this branch", "x"),
            &KbWriteCtx {
                context_folder: Some(f),
                context_branch: Some("feature/kb".into()),
                ..KbWriteCtx::default()
            },
            C,
        )
        .unwrap();
    let place = KbPlace {
        folder: Some(f),
        branch: Some("main".into()),
    };
    let on_main = |s: &Store| {
        s.kb_scope(
            Owner::Topic(t),
            &KbFilter {
                place: Some(place.clone()),
                ..KbFilter::default()
            },
            PageReq::first(10),
        )
        .unwrap()
        .total
    };
    assert_eq!(on_main(s), 0);
    let trace = s.kb_clear_branch(k, H).unwrap();
    assert_eq!(on_main(s), 1);
    let e = s.kb_entry(k).unwrap();
    assert_eq!((e.context_branch, e.context_folder), (None, Some(f)));
    assert!(s.kb_clear_branch(k, H).is_err());
    s.kb_restore_branch(&trace, H).unwrap();
    assert_eq!(on_main(s), 0);
}

#[test]
fn the_unmerge_check_agrees_with_unmerge() {
    let Seed { mut ts, t, u, .. } = seed();
    let s = &mut ts.store;
    let a = mem(s, Owner::Topic(t), "Keychain entry");
    let b = mem(s, Owner::Topic(u), "Keychain entry name");
    let trace = s.kb_merge(a, b, H).unwrap();
    // The merge revisions name the other entry, as the history shows.
    let merged = s.kb_history(a, PageReq::first(5)).unwrap().items;
    assert_eq!(merged[0].change, ChangeKind::Merged);
    assert_eq!(merged[0].section.as_deref(), Some(b.to_string().as_str()));
    let superseded = s.kb_history(b, PageReq::first(5)).unwrap().items;
    assert_eq!(superseded[0].change, ChangeKind::Superseded);
    assert_eq!(
        superseded[0].section.as_deref(),
        Some(a.to_string().as_str())
    );
    s.kb_update(a, body("Tidied."), H).unwrap();
    assert!(s.kb_can_unmerge(&trace).unwrap());
    s.kb_update(a, body("The agent's text."), C).unwrap();
    assert!(!s.kb_can_unmerge(&trace).unwrap());
    assert!(s.kb_unmerge(&trace, H).is_err());
}

#[test]
fn a_human_edit_records_and_settles_pairs() {
    let Seed { mut ts, p, t, .. } = seed();
    let s = &mut ts.store;
    let theirs = s
        .kb_create(
            memory(
                Owner::Project(p),
                "Store tests in parallel",
                "The store tests run in parallel",
                "cargo test runs the store tests in parallel.",
            ),
            C,
        )
        .unwrap();
    let mine = s
        .kb_create(
            memory(
                Owner::Topic(t),
                "Store tests in parallel",
                "Never run the store tests in parallel",
                "They share a key store.",
            ),
            H,
        )
        .unwrap();
    s.kb_recheck(mine, H).unwrap();
    let open: Vec<_> = s
        .kb_pairs_of(mine)
        .unwrap()
        .into_iter()
        .filter(|p| p.state == KbPairState::Open)
        .collect();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].kind, KbPairKind::Conflict);
    assert_eq!(open[0].other(mine), theirs);
    // Rewritten about something else: the pair is settled.
    s.kb_update(
        mine,
        KbUpdate {
            expected_rev: None,
            title: Some("Keychain entry name".into()),
            summary: Some("Alexandre's key lives in the keychain".into()),
            edit: KbEdit::Body("The keychain entry is named acta.db.".into()),
        },
        H,
    )
    .unwrap();
    s.kb_recheck(mine, H).unwrap();
    assert!(
        s.kb_pairs_of(mine)
            .unwrap()
            .iter()
            .all(|p| p.state == KbPairState::Resolved)
    );
    // Back to the contradicting text: the pair is open again.
    s.kb_update(
        mine,
        KbUpdate {
            expected_rev: None,
            title: Some("Store tests in parallel".into()),
            summary: Some("Never run the store tests in parallel".into()),
            edit: KbEdit::Body("They share a key store.".into()),
        },
        H,
    )
    .unwrap();
    s.kb_recheck(mine, H).unwrap();
    assert!(
        s.kb_pairs_of(mine)
            .unwrap()
            .iter()
            .any(|p| p.state == KbPairState::Open && p.other(mine) == theirs)
    );
    // Revisions carry their session and branch for the history.
    let rev = s.kb_history(theirs, PageReq::first(1)).unwrap().items;
    assert_eq!(rev[0].session, None);
}

#[test]
fn the_human_scan_records_duplicates_and_changes_no_entry() {
    let Seed {
        mut ts, t, u, p, ..
    } = seed();
    let s = &mut ts.store;
    let a = s
        .kb_create(
            memory(
                Owner::Topic(t),
                "Keychain entry name",
                "The key is stored under acta.db",
                "The keychain entry is named acta.db.",
            ),
            C,
        )
        .unwrap();
    let b = s
        .kb_create(
            memory(
                Owner::Project(p),
                "Keychain entry name",
                "The key is stored under acta.db",
                "The keychain entry is named acta.db.",
            ),
            C,
        )
        .unwrap();
    mem(s, Owner::Topic(u), "Backups run nightly");
    let rev = s.kb_entry(a).unwrap().rev;
    assert_eq!(s.kb_scan_duplicates(p, H).unwrap(), 1);
    assert_eq!(s.kb_entry(a).unwrap().rev, rev);
    let pairs = s.kb_pairs_of(a).unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].other(a), b);
    assert_eq!(pairs[0].state, KbPairState::Open);
    // Kept apart, a second scan finds nothing new.
    s.kb_set_pair_state(a, b, KbPairKind::Duplicate, KbPairState::Distinct, H)
        .unwrap();
    assert_eq!(s.kb_scan_duplicates(p, H).unwrap(), 0);
}

#[test]
fn the_scan_runs_page_by_page() {
    let Seed { mut ts, t, p, .. } = seed();
    let s = &mut ts.store;
    let same = |s: &mut Store, owner| {
        s.kb_create(
            memory(
                owner,
                "Keychain entry name",
                "The key is stored under acta.db",
                "The keychain entry is named acta.db.",
            ),
            C,
        )
        .unwrap()
    };
    let a = same(s, Owner::Topic(t));
    let b = same(s, Owner::Project(p));
    mem(s, Owner::Project(p), "Backups run nightly");
    // One memory per page: three pages, the pair found once from each side.
    let mut offset = 0;
    let mut pages = 0;
    let mut found = std::collections::HashSet::new();
    loop {
        let (pairs, next) = s.kb_scan_duplicates_page(p, offset, 1, H).unwrap();
        found.extend(pairs);
        pages += 1;
        match next {
            Some(n) => offset = n,
            None => break,
        }
    }
    assert_eq!(pages, 3);
    let want: std::collections::HashSet<_> = [(a.min(b), a.max(b))].into();
    assert_eq!(found, want);
}
