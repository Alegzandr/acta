//! Search everywhere over titles and content (the user's report of
//! 2026-11-22: "searches don't search in descriptions and contents").

use super::test_support::{TestStore, open};
use super::*;
use crate::model::*;

const H: Author = Author::Human;

fn find(s: &Store, text: &str) -> Vec<Record> {
    s.search(
        &SearchQuery {
            text: text.into(),
            ..SearchQuery::default()
        },
        PageReq::first(50),
    )
    .unwrap()
    .items
    .into_iter()
    .map(|h| h.record)
    .collect()
}

struct Seed {
    ts: TestStore,
    p: ProjectId,
    e: EpicId,
    t: TopicId,
}

fn seed() -> Seed {
    let mut ts = open();
    let s = &mut ts.store;
    let p = s.create_project("Alexandre's app", "", H).unwrap();
    let e = s.create_epic(p, "Storage", "", H).unwrap();
    let t = s
        .create_topic(
            NewTopic {
                project: p,
                epic: Some(e),
                title: "Encryption".into(),
                description: String::new(),
                status: Status::Active,
            },
            H,
        )
        .unwrap();
    Seed { ts, p, e, t }
}

#[test]
fn every_kind_is_found_by_a_word_only_in_its_content() {
    let Seed { mut ts, p, e, t } = seed();
    let s = &mut ts.store;
    s.update_project(
        p,
        ProjectPatch {
            description: Some("Local-first tracker with a lighthouse logo".into()),
            ..ProjectPatch::default()
        },
        H,
    )
    .unwrap();
    s.update_epic(
        e,
        EpicPatch {
            description: Some("Everything about the vault format".into()),
            state: Some("Waiting on the walrus review".into()),
            ..EpicPatch::default()
        },
        H,
    )
    .unwrap();
    s.update_topic(
        t,
        TopicPatch {
            description: Some("Keys live in the keychain; mention the gondola".into()),
            state: Some("Next: the narwhal benchmark".into()),
            ..TopicPatch::default()
        },
        H,
    )
    .unwrap();
    let entry = s
        .log(
            t,
            NewEntry {
                kind: EntryKind::Decision,
                text: "Chose the zeppelin approach".into(),
                state: None,
                status: None,
            },
            Author::Claude,
        )
        .unwrap();
    let note = s.add_note(p, "Remember the quokka idea", H).unwrap();
    let q = s
        .ask(
            NewQuestion {
                owner: Owner::Topic(t),
                text: "Which cipher?".into(),
                asked_to: None,
                expected_on: None,
            },
            H,
        )
        .unwrap();
    s.answer(q, Some("Use the axolotl suite"), false, H)
        .unwrap();
    let c = s
        .create_contact("Marie", ContactLevel::Project(p), H)
        .unwrap();
    s.update_contact(
        c,
        ContactPatch {
            job_title: Some(Some("Head of pelican relations".into())),
            company: Some(Some("Toucan Industries".into())),
            ..ContactPatch::default()
        },
        H,
    )
    .unwrap();
    let k = s
        .kb_create(
            NewKb {
                owner: Owner::Project(p),
                kind: KbKind::Memory,
                title: "Key name".into(),
                summary: "Where the key is".into(),
                body: "The entry is named after the platypus".into(),
                from_file: false,
            },
            H,
        )
        .unwrap();
    let file = s
        .attach(
            AttachOwner::Topic(t),
            "meerkat-plan.txt",
            AttachSource::Bytes(b"The wombat protocol, step by step.".to_vec()),
            H,
            &mut |_, _| {},
        )
        .unwrap();
    for (word, record) in [
        ("lighthouse", Record::Project(p)),
        ("vault", Record::Epic(e)),
        ("walrus", Record::Epic(e)),
        ("gondola", Record::Topic(t)),
        ("narwhal", Record::Topic(t)),
        ("zeppelin", Record::Entry(entry.id)),
        ("quokka", Record::Note(note)),
        ("axolotl", Record::Question(q)),
        ("pelican", Record::Contact(c)),
        ("toucan", Record::Contact(c)),
        ("platypus", Record::Kb(k)),
        ("meerkat", Record::Attachment(file.id)),
        ("wombat", Record::Attachment(file.id)),
    ] {
        assert_eq!(find(s, word), [record], "{word}");
    }
    // Trashed and retired records are not found.
    s.kb_retire(k, None, "", &KbWriteCtx::default(), H).unwrap();
    assert!(find(s, "platypus").is_empty());
    s.delete(Record::Attachment(file.id), H).unwrap();
    assert!(find(s, "wombat").is_empty());
    // A project filter keeps a contact of every project.
    let all = s.create_contact("Paul", ContactLevel::All, H).unwrap();
    s.update_contact(
        all,
        ContactPatch {
            company: Some(Some("Toucan Partners".into())),
            ..ContactPatch::default()
        },
        H,
    )
    .unwrap();
    let in_p = s
        .search(
            &SearchQuery {
                text: "toucan".into(),
                project: Some(p),
                ..SearchQuery::default()
            },
            PageReq::first(10),
        )
        .unwrap();
    assert_eq!(in_p.total, 2);
}

#[test]
fn accents_and_endings_meet_in_english_and_french() {
    let Seed { mut ts, p, .. } = seed();
    let s = &mut ts.store;
    let n1 = s
        .add_note(p, "Le fichier de l'élève est configuré", H)
        .unwrap();
    let n2 = s
        .add_note(p, "The configuration files are ready", H)
        .unwrap();
    assert_eq!(find(s, "eleve"), [Record::Note(n1)]);
    assert_eq!(find(s, "Élève"), [Record::Note(n1)]);
    assert_eq!(find(s, "fichiers"), [Record::Note(n1)]);
    let mut configured = find(s, "configured");
    configured.sort_by_key(|r| format!("{r}"));
    assert_eq!(configured, [Record::Note(n1), Record::Note(n2)]);
    assert_eq!(find(s, "file"), [Record::Note(n2)]);
}

#[test]
fn a_title_match_ranks_before_a_content_match() {
    let Seed { mut ts, p, e, t } = seed();
    let s = &mut ts.store;
    // The word is in t's description many times, and in the other's title
    // once: the title wins anyway.
    s.update_topic(
        t,
        TopicPatch {
            description: Some("heron heron heron heron heron".into()),
            ..TopicPatch::default()
        },
        H,
    )
    .unwrap();
    let titled = s
        .create_topic(
            NewTopic {
                project: p,
                epic: Some(e),
                title: "Heron migration with many other words in the title".into(),
                description: String::new(),
                status: Status::Draft,
            },
            H,
        )
        .unwrap();
    let page = s
        .search(
            &SearchQuery {
                text: "heron".into(),
                ..SearchQuery::default()
            },
            PageReq::first(10),
        )
        .unwrap();
    assert_eq!(page.items[0].record, Record::Topic(titled));
    assert!(page.items[0].in_title && !page.items[1].in_title);
    assert_eq!(page.items[1].record, Record::Topic(t));
    // The excerpt marks the matched word.
    assert!(
        page.items[1]
            .snippet
            .contains(&format!("{SNIPPET_OPEN}heron{SNIPPET_CLOSE}"))
    );
}

#[test]
fn the_migration_indexes_what_is_already_there() {
    let dir = tempfile::tempdir().unwrap();
    let keys = crate::keys::MemoryKeyStore::new();
    let config = StoreConfig {
        data_dir: dir.path().to_path_buf(),
        keys: keys.stores(),
    };
    let (p, file) = {
        let mut old = Store::open_with(&config, &crate::schema::MIGRATIONS[..7]).unwrap();
        let p = old
            .create_project("Alexandre's app", "With a lighthouse logo", H)
            .unwrap();
        let e = old
            .create_epic(p, "Storage", "The vault format", H)
            .unwrap();
        let c = old.create_contact("Marie", ContactLevel::All, H).unwrap();
        old.update_contact(
            c,
            ContactPatch {
                company: Some(Some("Toucan Industries".into())),
                ..ContactPatch::default()
            },
            H,
        )
        .unwrap();
        let q = old
            .ask(
                NewQuestion {
                    owner: Owner::Epic(e),
                    text: "Which axolotl?".into(),
                    asked_to: None,
                    expected_on: None,
                },
                H,
            )
            .unwrap();
        let _ = q;
        let file = old
            .attach(
                AttachOwner::Epic(e),
                "plan.md",
                AttachSource::Bytes(b"# The wombat protocol".to_vec()),
                H,
                &mut |_, _| {},
            )
            .unwrap();
        (p, file)
    };
    let s = Store::open(&config).unwrap();
    assert_eq!(find(&s, "lighthouse"), [Record::Project(p)]);
    assert_eq!(find(&s, "vault").len(), 1);
    assert_eq!(find(&s, "toucan").len(), 1);
    assert_eq!(find(&s, "axolotl").len(), 1);
    assert_eq!(find(&s, "wombat"), [Record::Attachment(file.id)]);
}

/// Search over the Overview's seeded database (10,000 topics, 200,000
/// entries, 2,000 notes, 1,000 memories): reported, not asserted. Run
/// with `cargo test -p acta-core --release -- --ignored --nocapture
/// search_timing`.
#[test]
#[ignore]
fn search_timing_on_ten_thousand_topics() {
    let mut ts = open();
    let s = &mut ts.store;
    let p = s.create_project("Alexandre's app", "", H).unwrap();
    s.conn.execute_batch("BEGIN").unwrap();
    let words = [
        "keychain",
        "snapshot",
        "migration",
        "terminal",
        "export",
        "folder",
        "branch",
        "commit",
        "release",
        "theme",
        "session",
        "budget",
        "parallel",
        "timeout",
        "server",
        "cache",
    ];
    {
        let mut topic = s
            .conn
            .prepare(
                "INSERT INTO topics (uuid, project_id, title, description, status, position, created_at, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, 'active', ?5, 0, 0, 'human')",
            )
            .unwrap();
        let mut entry = s
            .conn
            .prepare(
                "INSERT INTO entries (uuid, topic_id, seq, kind, author, text, created_at, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, 'progress', 'claude', ?4, 0, 0, 'claude')",
            )
            .unwrap();
        for i in 0..10_000i64 {
            let w = |k: i64| words[((i * 7 + k * 3) % words.len() as i64) as usize];
            topic
                .execute(rusqlite::params![
                    format!("t{i}"),
                    p.0,
                    format!("{} {} {i}", w(0), w(1)),
                    format!("About the {} and the {} of Alexandre", w(2), w(3)),
                    i
                ])
                .unwrap();
            let id = s.conn.last_insert_rowid();
            for j in 0..20i64 {
                entry
                    .execute(rusqlite::params![
                        format!("e{i}-{j}"),
                        id,
                        j + 1,
                        format!("Worked on the {} with the {} today", w(j), w(j + 1))
                    ])
                    .unwrap();
            }
        }
    }
    s.conn.execute_batch("COMMIT; ANALYZE;").unwrap();
    for text in [
        "keychain",
        "migr",
        "keychain snapshot",
        "parallel timeout server",
    ] {
        let runs = 20;
        let start = std::time::Instant::now();
        let mut total = 0;
        for _ in 0..runs {
            total = s
                .search(
                    &SearchQuery {
                        text: text.into(),
                        ..SearchQuery::default()
                    },
                    PageReq::first(20),
                )
                .unwrap()
                .total;
        }
        println!(
            "search \"{text}\" over 10,000 topics / 200,000 entries: {:?} per query ({total} hits, capped at 500)",
            start.elapsed() / runs
        );
    }
}
