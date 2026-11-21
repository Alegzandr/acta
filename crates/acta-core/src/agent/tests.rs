//! Budgets and behaviour of the agent tools on a seeded database with
//! pathological sizes (ADR 0005, "Budget enforcement").

use super::args::*;
use super::render::{budget, chars, limit};
use super::*;
use crate::model::{Author, EntryKind, KbKind, Owner, ProjectId, Status, TopicId};
use crate::store::test_support::{self, TestStore};
use crate::store::{NewEntry, NewKb, NewTopic, Store};

struct Seed {
    ts: TestStore,
    project: ProjectId,
    focus: TopicId,
    other_project_topic: TopicId,
    big_doc: crate::model::KbId,
    flat_doc: crate::model::KbId,
}

fn long(word: &str, n: usize) -> String {
    let mut s = String::new();
    while s.chars().count() < n {
        s.push_str(word);
        s.push(' ');
    }
    s.chars().take(n).collect()
}

fn seed() -> Seed {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let h = Author::Human;
    let project = s.create_project("Acta", "", h).unwrap();
    let mut first_topic = None;
    for e in 0..12 {
        let epic = s
            .create_epic(project, &long(&format!("Epic{e} storage"), 120), "", h)
            .unwrap();
        for t in 0..8 {
            let id = s
                .create_topic(
                    NewTopic {
                        project,
                        epic: Some(epic),
                        title: long(&format!("Topic{e}x{t} keychain"), 120),
                        description: String::new(),
                        status: Status::Active,
                    },
                    h,
                )
                .unwrap();
            first_topic.get_or_insert(id);
        }
    }
    for t in 0..30 {
        s.create_topic(
            NewTopic {
                project,
                epic: None,
                title: long(&format!("Loose{t} keychain"), 120),
                description: String::new(),
                status: Status::Planned,
            },
            h,
        )
        .unwrap();
    }
    let focus = first_topic.unwrap();
    for i in 0..40 {
        s.log(
            focus,
            NewEntry {
                kind: EntryKind::Decision,
                text: long(&format!("keychain decision {i}"), 1_500),
                state: Some(long("state keychain", 600)),
                status: None,
            },
            Author::Claude,
        )
        .unwrap();
    }
    for i in 0..30 {
        s.add_note(project, &long(&format!("note{i} keychain"), 1_500), h)
            .unwrap();
    }
    for i in 0..30 {
        s.kb_create(
            NewKb {
                owner: Owner::Topic(focus),
                kind: KbKind::Memory,
                title: long(&format!("Memory{i} keychain fact"), 120),
                summary: long(&format!("Summary{i} keychain"), 120),
                body: long("body keychain", 800),
                from_file: false,
            },
            Author::Claude,
        )
        .unwrap();
    }
    let mut body = String::from("Intro\n\n");
    for i in 1..=60 {
        body.push_str(&format!(
            "# Section {i} keychain\n\n{}\n\n",
            long("text", 1_000)
        ));
    }
    let big_doc = s
        .kb_create(
            NewKb {
                owner: Owner::Project(project),
                kind: KbKind::Document,
                title: long("Architecture", 120),
                summary: long("What the crates do", 120),
                body,
                from_file: false,
            },
            h,
        )
        .unwrap();
    let flat_doc = s
        .kb_create(
            NewKb {
                owner: Owner::Project(project),
                kind: KbKind::Document,
                title: "Flat".into(),
                summary: "No headings".into(),
                body: (0..2_000).map(|i| format!("line {i} keychain\n")).collect(),
                from_file: false,
            },
            h,
        )
        .unwrap();
    let other = s.create_project("Other", "", h).unwrap();
    let other_project_topic = s
        .create_topic(
            NewTopic {
                project: other,
                epic: None,
                title: "Elsewhere".into(),
                description: String::new(),
                status: Status::Active,
            },
            h,
        )
        .unwrap();
    Seed {
        ts,
        project,
        focus,
        other_project_topic,
        big_doc,
        flat_doc,
    }
}

fn session(seed: &Seed) -> Session {
    let mut s = Session::unbound(Author::Claude);
    s.project = Some(seed.project);
    s.focus = Some(Owner::Topic(seed.focus));
    s
}

fn agent<'a>(store: &'a mut Store, session: &'a mut Session) -> Agent<'a> {
    Agent { store, session }
}

fn ok(r: Reply, max: usize) -> String {
    let text = r.unwrap_or_else(|e| panic!("refused: {e}"));
    assert!(
        chars(&text) <= max,
        "{} chars > {max}:\n{text}",
        chars(&text)
    );
    assert!(chars(&text) <= budget::HARD);
    text
}

#[test]
fn every_tool_stays_within_its_budget_on_pathological_data() {
    let mut seed = seed();
    let mut sess = session(&seed);
    let big = seed.big_doc;
    let flat = seed.flat_doc;
    let focus = seed.focus;
    let epic = seed.ts.store.topic(focus).unwrap().epic.unwrap();
    let project = seed.project;
    let mut a = agent(&mut seed.ts.store, &mut sess);

    let brief = ok(a.brief(BriefArgs::default()), budget::BRIEF);
    assert!(brief.contains("· focus"), "{brief}");
    assert!(brief.contains("knowledge in scope"), "{brief}");
    ok(
        a.brief(BriefArgs {
            id: Some(epic.to_string()),
            focus: false,
        }),
        budget::BRIEF,
    );
    let overview = ok(
        a.brief(BriefArgs {
            id: Some(project.to_string()),
            focus: false,
        }),
        budget::BRIEF,
    );
    assert!(overview.contains("more"), "{overview}");

    let entries = ok(a.entries(EntriesArgs::default()), budget::ENTRIES);
    assert!(entries.contains("older: before="), "{entries}");
    assert!(entries.contains("chars, id=\""), "{entries}");
    ok(
        a.entries(EntriesArgs {
            limit: Some(20),
            ..Default::default()
        }),
        budget::ENTRY_FULL,
    );
    let full = ok(
        a.entries(EntriesArgs {
            id: Some(format!("{focus}#3")),
            ..Default::default()
        }),
        budget::ENTRY_FULL,
    );
    assert!(chars(&full) > 1_500);

    ok(a.find(FindArgs::default()), budget::FIND);
    for kind in ["note", "epic"] {
        ok(
            a.find(FindArgs {
                kind: Some(kind.into()),
                ..Default::default()
            }),
            budget::FIND,
        );
    }
    let hits = ok(
        a.find(FindArgs {
            query: Some("keychain".into()),
            ..Default::default()
        }),
        budget::FIND,
    );
    assert!(hits.contains("cursor="), "{hits}");
    ok(
        a.find(FindArgs {
            query: Some("keychain".into()),
            limit: Some(20),
            ..Default::default()
        }),
        budget::FIND,
    );

    let kb = ok(a.kb(KbArgs::default()), budget::KB);
    assert!(kb.contains("more: cursor="), "{kb}");
    ok(
        a.kb(KbArgs {
            query: Some("keychain".into()),
            ..Default::default()
        }),
        budget::KB,
    );
    ok(
        a.kb(KbArgs {
            cursor: Some(10),
            ..Default::default()
        }),
        budget::KB,
    );

    let outline = ok(
        a.kb_read(KbReadArgs {
            id: big.to_string(),
            ..Default::default()
        }),
        budget::KB_READ,
    );
    assert!(outline.contains("outline"), "{outline}");
    ok(
        a.kb_read(KbReadArgs {
            id: big.to_string(),
            section: Some("7".into()),
            ..Default::default()
        }),
        budget::KB_READ,
    );
    let flat_read = ok(
        a.kb_read(KbReadArgs {
            id: flat.to_string(),
            ..Default::default()
        }),
        budget::KB_READ,
    );
    assert!(flat_read.contains("lines="), "{flat_read}");
    let part = ok(
        a.kb_read(KbReadArgs {
            id: flat.to_string(),
            lines: Some("1-2000".into()),
            ..Default::default()
        }),
        budget::KB_READ,
    );
    assert!(part.contains("read the rest with lines="), "{part}");

    let log = ok(
        a.log(LogArgs {
            kind: "progress".into(),
            text: long("did", 1_500),
            state: Some(long("now", 600)),
            status: Some("blocked".into()),
            topic: None,
        }),
        budget::ACK,
    );
    assert!(
        log.starts_with("ok T-") && log.contains("state updated") && log.contains("blocked"),
        "{log}"
    );
    ok(
        a.update(UpdateArgs {
            id: focus.to_string(),
            title: Some(long("t", 120)),
            ..Default::default()
        }),
        budget::ACK,
    );
    let created = ok(
        a.create(CreateArgs {
            items: (0..10)
                .map(|i| CreateItem {
                    kind: "topic".into(),
                    title: long(&format!("New{i}"), 120),
                    from_notes: Some(vec!["N-1".into(), "N-2".into()]),
                    ..Default::default()
                })
                .collect(),
        }),
        budget::CREATE,
    );
    assert!(created.starts_with("ok "), "{created}");
    let w = ok(
        a.kb_write(KbWriteArgs {
            kind: Some("document".into()),
            title: Some(long("Doc", 120)),
            summary: Some(long("What it says", 120)),
            text: long("x", limit::DOCUMENT_WRITE),
            level: Some("project".into()),
            ..Default::default()
        }),
        budget::ACK,
    );
    assert!(w.contains("rev 1"), "{w}");

    let briefing = a.session_briefing(true).unwrap();
    assert!(chars(&briefing) <= budget::SESSION_BRIEFING, "{briefing}");
    assert!(briefing.contains("compacted"), "{briefing}");
    let sub = a.subagent_note().unwrap();
    assert!(chars(&sub) <= budget::SUBAGENT, "{sub}");
    let hook = ok(
        a.hook(HookArgs {
            event: "compact".into(),
            ..Default::default()
        }),
        budget::HARD,
    );
    let v: serde_json::Value = serde_json::from_str(&hook).unwrap();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
}

#[test]
fn instructions_are_under_their_budget() {
    assert!(
        INSTRUCTIONS.len() < budget::INSTRUCTIONS,
        "{}",
        INSTRUCTIONS.len()
    );
}

#[test]
fn briefing_lists_recent_memory_titles_and_counts() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Acta", "", Author::Human).unwrap();
    let t = s
        .create_topic(
            NewTopic {
                project: p,
                epic: None,
                title: "Encrypted storage spike".into(),
                description: String::new(),
                status: Status::Active,
            },
            Author::Human,
        )
        .unwrap();
    s.kb_create(
        NewKb {
            owner: Owner::Project(p),
            kind: KbKind::Memory,
            title: "Clippy runs with -D warnings".into(),
            summary: "CI fails on any clippy warning.".into(),
            body: "Run clippy before committing.".into(),
            from_file: false,
        },
        Author::Claude,
    )
    .unwrap();
    let mut sess = Session::unbound(Author::Claude);
    sess.project = Some(p);
    sess.focus = Some(Owner::Topic(t));
    let a = agent(s, &mut sess);
    let b = a.session_briefing(false).unwrap();
    assert!(
        b.contains("T-1 Encrypted storage spike · active · focus"),
        "{b}"
    );
    assert!(b.contains("0 documents, 1 memory"), "{b}");
    assert!(
        b.contains("Recent memories: K-1 Clippy runs with -D warnings"),
        "{b}"
    );
    assert!(
        !b.contains("Run clippy before"),
        "bodies are never injected: {b}"
    );
}

#[test]
fn limits_are_refused_with_limit_and_size() {
    let mut seed = seed();
    let mut sess = session(&seed);
    let mut a = agent(&mut seed.ts.store, &mut sess);
    let e = a
        .log(LogArgs {
            kind: "progress".into(),
            text: "x".repeat(1_501),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(e, "text is 1501 chars; the limit is 1500. Shorten it.");
    let e = a
        .kb_write(KbWriteArgs {
            kind: Some("memory".into()),
            title: Some("A".into()),
            summary: Some("s".repeat(121)),
            text: "x".into(),
            ..Default::default()
        })
        .unwrap_err();
    assert!(e.contains("summary is 121 chars; the limit is 120"), "{e}");
    let e = a
        .kb_write(KbWriteArgs {
            kind: Some("memory".into()),
            title: Some("A".into()),
            summary: Some("s".into()),
            text: "x".repeat(801),
            ..Default::default()
        })
        .unwrap_err();
    assert!(
        e.contains("memory body is 801 chars; the limit is 800"),
        "{e}"
    );
    assert!(
        a.log(LogArgs {
            kind: "system".into(),
            text: "x".into(),
            ..Default::default()
        })
        .is_err()
    );
}

fn memory(a: &mut Agent<'_>, title: &str, summary: &str) -> Reply {
    a.kb_write(KbWriteArgs {
        kind: Some("memory".into()),
        level: Some("project".into()),
        title: Some(title.into()),
        summary: Some(summary.into()),
        text: "Use --test-threads=1.".into(),
        ..Default::default()
    })
}

#[test]
fn near_duplicate_memory_is_refused_once() {
    let mut ts = test_support::open();
    let p = ts.store.create_project("Acta", "", Author::Human).unwrap();
    let mut sess = Session::unbound(Author::Claude);
    sess.project = Some(p);
    let mut a = agent(&mut ts.store, &mut sess);
    let summary = "Tests deadlock when cargo runs them in parallel.";
    assert!(memory(&mut a, "Contention test needs --test-threads=1", summary).is_ok());
    let refused = memory(&mut a, "The contention test needs test-threads=1", summary).unwrap_err();
    assert!(refused.starts_with("not saved: K-1"), "{refused}");
    assert!(refused.contains("rev=1"), "{refused}");
    // Repeating the same call saves it.
    let again = memory(&mut a, "The contention test needs test-threads=1", summary).unwrap();
    assert!(again.starts_with("ok K-2"), "{again}");
    // A different memory is not a duplicate.
    let other = memory(
        &mut a,
        "Credential Manager entry name",
        "Where the database key lives on Windows.",
    )
    .unwrap();
    assert!(other.starts_with("ok K-3"), "{other}");
}

#[test]
fn section_edits_need_the_current_revision() {
    let mut ts = test_support::open();
    let p = ts.store.create_project("Acta", "", Author::Human).unwrap();
    let mut sess = Session::unbound(Author::Claude);
    sess.project = Some(p);
    let mut a = agent(&mut ts.store, &mut sess);
    a.kb_write(KbWriteArgs {
        kind: Some("document".into()),
        level: Some("project".into()),
        title: Some("Architecture".into()),
        summary: Some("Crates and processes; read before changing structure.".into()),
        text: "# Overview\nOne\n# Crates\nTwo\n".into(),
        ..Default::default()
    })
    .unwrap();
    let edit = |a: &mut Agent<'_>, rev: i64| {
        a.kb_write(KbWriteArgs {
            id: Some("K-1".into()),
            rev: Some(rev),
            op: Some("replace_section".into()),
            section: Some("2".into()),
            text: "Three".into(),
            ..Default::default()
        })
    };
    let r = edit(&mut a, 1).unwrap();
    assert!(r.starts_with("ok K-1 rev 2 · section 2 replaced"), "{r}");
    let stale = edit(&mut a, 1).unwrap_err();
    assert!(
        stale.contains("it is rev 2 now") && stale.contains("1 Overview"),
        "{stale}"
    );
    let missing_rev = a
        .kb_write(KbWriteArgs {
            id: Some("K-1".into()),
            op: Some("replace_body".into()),
            text: "x".into(),
            ..Default::default()
        })
        .unwrap_err();
    assert!(missing_rev.contains("pass rev"), "{missing_rev}");
    let read = a
        .kb_read(KbReadArgs {
            id: "K-1".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(read.contains("rev 2") && read.contains("Three"), "{read}");
    let found = a
        .kb(KbArgs {
            query: Some("three".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(found.contains("K-1 §2 Architecture"), "{found}");
}

#[test]
fn other_projects_are_out_of_reach_and_unlinked_sessions_say_so() {
    let mut seed = seed();
    let other = seed.other_project_topic;
    let mut sess = session(&seed);
    let mut a = agent(&mut seed.ts.store, &mut sess);
    let e = a
        .brief(BriefArgs {
            id: Some(other.to_string()),
            focus: false,
        })
        .unwrap_err();
    assert!(e.contains("another project"), "{e}");
    let e = a
        .log(LogArgs {
            kind: "progress".into(),
            text: "x".into(),
            topic: Some(other.to_string()),
            ..Default::default()
        })
        .unwrap_err();
    assert!(e.contains("another project"), "{e}");

    let mut unbound = Session::unbound(Author::Claude);
    let mut a = agent(&mut seed.ts.store, &mut unbound);
    assert_eq!(a.brief(BriefArgs::default()).unwrap_err(), NOT_LINKED);
    assert_eq!(a.find(FindArgs::default()).unwrap_err(), NOT_LINKED);
    // An explicit id still works.
    assert!(
        a.brief(BriefArgs {
            id: Some(other.to_string()),
            focus: false,
        })
        .is_ok()
    );
    assert!(a.session_briefing(false).is_none());
}

#[test]
fn focus_is_saved_per_session_and_worktree_and_explicit_ids_do_not_move_it() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Acta", "", Author::Human).unwrap();
    let mk = |s: &mut Store, title: &str| {
        s.create_topic(
            NewTopic {
                project: p,
                epic: None,
                title: title.into(),
                description: String::new(),
                status: Status::Active,
            },
            Author::Human,
        )
        .unwrap()
    };
    let t1 = mk(s, "One");
    let t2 = mk(s, "Two");
    let dir = tempfile::tempdir().unwrap();
    s.add_folder(Owner::Topic(t1), dir.path(), Author::Human)
        .unwrap();
    let launch = Launch {
        dir: Some(dir.path().to_path_buf()),
        session_id: Some("abc".into()),
        ..Launch::default()
    };
    let mut sess = Session::resolve(s, &launch, Author::Claude);
    assert_eq!(sess.project, Some(p));
    assert_eq!(
        sess.focus,
        Some(Owner::Topic(t1)),
        "the attached folder focuses its topic"
    );
    {
        let mut a = agent(s, &mut sess);
        a.log(LogArgs {
            kind: "progress".into(),
            text: "x".into(),
            topic: Some(t2.to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            a.session.focus,
            Some(Owner::Topic(t1)),
            "explicit ids never move the focus"
        );
        a.brief(BriefArgs {
            id: Some(t2.to_string()),
            focus: true,
        })
        .unwrap();
    }
    // A new process for the same session gets the saved focus.
    let again = Session::resolve(s, &launch, Author::Claude);
    assert_eq!(again.focus, Some(Owner::Topic(t2)));
    // A new session in the same folder gets the worktree's last focus.
    let other = Launch {
        session_id: Some("def".into()),
        ..launch.clone()
    };
    assert_eq!(
        Session::resolve(s, &other, Author::Claude).focus,
        Some(Owner::Topic(t2))
    );
    // The environment wins.
    let pinned = Launch {
        topic: Some(t1.to_string()),
        ..launch.clone()
    };
    assert_eq!(
        Session::resolve(s, &pinned, Author::Claude).focus,
        Some(Owner::Topic(t1))
    );
    // After /clear the server adopts the new id and carries the focus.
    let mut sess = Session::resolve(s, &launch, Author::Claude);
    let mut a = agent(s, &mut sess);
    a.hook(HookArgs {
        event: "clear".into(),
        session: Some("ghi".into()),
        agent: None,
    })
    .unwrap();
    let cleared = Launch {
        session_id: Some("ghi".into()),
        ..Launch::default()
    };
    assert_eq!(
        Session::resolve(s, &cleared, Author::Claude).focus,
        Some(Owner::Topic(t2))
    );
}

#[test]
fn create_validates_every_item_before_writing() {
    let mut seed = seed();
    let mut sess = session(&seed);
    let mut a = agent(&mut seed.ts.store, &mut sess);
    let epics = FindArgs {
        kind: Some("epic".into()),
        ..Default::default()
    };
    let before = a.find(epics.clone()).unwrap();
    let e = a
        .create(CreateArgs {
            items: vec![
                CreateItem {
                    kind: "epic".into(),
                    title: "Fine".into(),
                    ..Default::default()
                },
                CreateItem {
                    kind: "topic".into(),
                    title: "x".repeat(121),
                    ..Default::default()
                },
            ],
        })
        .unwrap_err();
    assert!(e.contains("item 2 title is 121 chars"), "{e}");
    assert_eq!(a.find(epics).unwrap(), before);
    let e = a
        .create(CreateArgs {
            items: vec![CreateItem::default(); 11],
        })
        .unwrap_err();
    assert!(e.contains("limit is 10"), "{e}");
}

#[test]
fn link_index_follows_folder_changes() {
    let mut ts = test_support::open();
    let p = ts.store.create_project("Acta", "", Author::Human).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let os = crate::paths::Os::CURRENT;
    let stored = crate::folder_path::canonical(dir.path()).unwrap();
    let f = ts
        .store
        .add_folder(Owner::Project(p), dir.path(), Author::Human)
        .unwrap();
    let index = crate::link_index::LinkIndex::read(ts.dir.path()).unwrap();
    assert!(index.matches(os, std::slice::from_ref(&stored)));
    ts.store
        .delete(crate::model::Record::Folder(f), Author::Human)
        .unwrap();
    let index = crate::link_index::LinkIndex::read(ts.dir.path()).unwrap();
    assert!(!index.matches(os, &[stored]));
}
