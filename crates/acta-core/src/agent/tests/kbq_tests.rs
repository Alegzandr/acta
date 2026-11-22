//! Memory quality for the agent (ADR 0015, phase 3): the checker's
//! refusals and "keep both", supersede, verify and retire, the stale
//! revision reply and the section merge, markers, the promote hint, and
//! the budgets of each on a pathological seed.

use super::*;
use crate::model::{KbId, KbState};

fn bound(store: &mut Store) -> (ProjectId, TopicId, Session) {
    let p = store.create_project("Acta", "", Author::Human).unwrap();
    let t = store
        .create_topic(
            NewTopic {
                project: p,
                epic: None,
                title: "Encrypted storage".into(),
                description: String::new(),
                status: Status::Active,
            },
            Author::Human,
        )
        .unwrap();
    let mut s = Session::unbound(Author::Claude);
    s.project = Some(p);
    s.focus = Some(Owner::Topic(t));
    s.session_id = Some("session-a".into());
    (p, t, s)
}

fn mem(a: &mut Agent<'_>, level: &str, title: &str, summary: &str, text: &str) -> Reply {
    a.kb_write(KbWriteArgs {
        kind: Some("memory".into()),
        level: Some(level.into()),
        title: Some(title.into()),
        summary: Some(summary.into()),
        text: text.into(),
        ..Default::default()
    })
}

#[test]
fn a_conflict_is_refused_once_with_three_ways_out() {
    let mut ts = test_support::open();
    let (_, _, mut sess) = bound(&mut ts.store);
    let mut a = agent(&mut ts.store, &mut sess);
    let first = mem(
        &mut a,
        "project",
        "Tests run in parallel",
        "cargo test runs the store tests concurrently",
        "The store tests are safe to run in parallel.",
    )
    .unwrap();
    assert!(first.starts_with("ok K-1"), "{first}");
    let call = |a: &mut Agent<'_>, supersedes: Option<&str>| {
        a.kb_write(KbWriteArgs {
            kind: Some("memory".into()),
            title: Some("Store tests run serially".into()),
            summary: Some("Never run the store tests in parallel: they share a key store".into()),
            text: "Do not run the store tests in parallel.".into(),
            supersedes: supersedes.map(str::to_owned),
            ..Default::default()
        })
    };
    let refused = ok_err(call(&mut a, None), 600);
    assert!(
        refused.starts_with("not saved: possible conflict with K-1 rev 1 (project, verified "),
        "{refused}"
    );
    assert!(refused.contains("supersedes=\"K-1\""), "{refused}");
    assert!(refused.contains("kb_write id=\"K-1\" rev=1"), "{refused}");
    assert!(refused.contains("[acta data "), "{refused}");
    // Replace it: the old one is superseded in the same write.
    let ok = call(&mut a, Some("K-1")).unwrap();
    assert!(
        ok.starts_with("ok K-2 memory · topic T-1 · rev 1 · supersedes K-1"),
        "{ok}"
    );
    assert!(chars(&ok) <= budget::ACK);
    let old = a.store.kb_entry(KbId(1)).unwrap();
    assert_eq!(
        (old.state, old.superseded_by),
        (KbState::Superseded, Some(KbId(2)))
    );
    // Superseded: out of the list, the header counts it, read by id.
    let list = a.kb(KbArgs::default()).unwrap();
    assert!(!list.contains("K-1 "), "{list}");
    assert!(list.contains("· 1 superseded"), "{list}");
    let read = a
        .kb_read(KbReadArgs {
            id: "K-1".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(read.starts_with("K-1 superseded by K-2 on "), "{read}");
    // Provenance: the session and topic were recorded.
    let k2 = a.store.kb_entry(KbId(2)).unwrap();
    assert_eq!(k2.created_session.as_deref(), Some("session-a"));
}

fn ok_err(r: Reply, max: usize) -> String {
    let e = r.expect_err("refused");
    assert!(chars(&e) <= max, "{} > {max}: {e}", chars(&e));
    e
}

#[test]
fn repeating_the_identical_call_keeps_both_and_never_warns_again() {
    let mut ts = test_support::open();
    let (_, _, mut sess) = bound(&mut ts.store);
    let mut a = agent(&mut ts.store, &mut sess);
    let write = |a: &mut Agent<'_>, title: &str| {
        mem(
            a,
            "topic",
            title,
            "The SQLCipher key is stored in the OS keychain",
            "On first run the key goes to the keychain, entry acta.db.",
        )
    };
    write(&mut a, "Database key in the keychain").unwrap();
    let refused = write(&mut a, "Keychain holds the database key").unwrap_err();
    assert!(refused.contains("possible duplicate of K-1"), "{refused}");
    let kept = write(&mut a, "Keychain holds the database key").unwrap();
    assert!(kept.ends_with("· kept both"), "{kept}");
    let pairs = a.store.kb_pairs_of(KbId(2)).unwrap();
    assert_eq!(pairs[0].state, crate::model::KbPairState::Distinct);
    // An edit of the kept one is not refused for that pair again.
    let edit = a
        .kb_write(KbWriteArgs {
            id: Some("K-2".into()),
            rev: Some(1),
            op: Some("replace_body".into()),
            text: "On first run the key goes to the keychain under acta.db.".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(edit.starts_with("ok K-2 rev 2"), "{edit}");
    // The exact same text as another live memory is refused every time.
    let same = |a: &mut Agent<'_>| {
        mem(
            a,
            "topic",
            "Database key in the keychain",
            "The SQLCipher key is stored in the OS keychain",
            "On first run the key goes to the keychain, entry acta.db.",
        )
    };
    let s1 = same(&mut a).unwrap_err();
    assert_eq!(s1, "not saved: same as K-1 (rev 1).");
    assert_eq!(same(&mut a).unwrap_err(), s1);
}

#[test]
fn verify_and_retire_need_the_revision_read() {
    let mut ts = test_support::open();
    let (_, t, mut sess) = bound(&mut ts.store);
    let mut a = agent(&mut ts.store, &mut sess);
    mem(
        &mut a,
        "topic",
        "Credential Manager entry",
        "Where the key lives on Windows",
        "acta.db",
    )
    .unwrap();
    let op = |a: &mut Agent<'_>, op: &str, rev: Option<i64>, text: &str| {
        a.kb_write(KbWriteArgs {
            id: Some("K-1".into()),
            rev,
            op: Some(op.into()),
            text: text.into(),
            ..Default::default()
        })
    };
    assert!(
        op(&mut a, "verify", None, "")
            .unwrap_err()
            .contains("pass rev")
    );
    let stale = op(&mut a, "verify", Some(9), "").unwrap_err();
    assert!(
        stale.starts_with("not saved: K-1 changed since rev 9: rev 1 by this session"),
        "{stale}"
    );
    assert!(
        stale.contains("Current text:") && stale.contains("acta.db"),
        "{stale}"
    );
    assert!(chars(&stale) <= 1_600);
    assert_eq!(
        op(&mut a, "verify", Some(1), "").unwrap(),
        "ok K-1 verified (rev 2)"
    );
    let retire = op(&mut a, "retire", Some(2), &"x".repeat(201)).unwrap_err();
    assert!(retire.contains("reason"), "{retire}");
    assert_eq!(
        op(&mut a, "retire", Some(2), "The entry moved to the vault").unwrap(),
        "ok K-1 retired (rev 3)"
    );
    let list = a.kb(KbArgs::default()).unwrap();
    assert!(list.contains("· 1 retired"), "{list}");
    // Closing a topic that owns a live memory hints at promoting it.
    mem(
        &mut a,
        "topic",
        "Key rotation",
        "How the key is rotated",
        "Never.",
    )
    .unwrap();
    let closed = a
        .log(LogArgs {
            kind: "progress".into(),
            text: "Storage done.".into(),
            status: Some("done".into()),
            topic: Some(t.to_string()),
            ..Default::default()
        })
        .unwrap();
    assert!(
        closed.contains("T-1 owns 1 memory: promote lasting ones with kb_write(id, rev, level=\"epic\"|\"project\")"),
        "{closed}"
    );
    assert!(chars(&closed) <= budget::ACK + super::super::kbq::PROMOTE_HINT);
    // Once: not again when the topic is already done.
    let again = a
        .log(LogArgs {
            kind: "progress".into(),
            text: "Still done.".into(),
            status: Some("done".into()),
            topic: Some(t.to_string()),
            ..Default::default()
        })
        .unwrap();
    assert!(!again.contains("owns"), "{again}");
}

#[test]
fn a_stale_section_edit_merges_when_its_section_is_untouched() {
    let mut ts = test_support::open();
    let (_, _, mut sess) = bound(&mut ts.store);
    let mut a = agent(&mut ts.store, &mut sess);
    a.kb_write(KbWriteArgs {
        kind: Some("document".into()),
        level: Some("project".into()),
        title: Some("Architecture".into()),
        summary: Some("Crates and processes; read before changing structure.".into()),
        text: "# Overview\nOne\n# Crates\nTwo\n# Storage\nThree\n".into(),
        ..Default::default()
    })
    .unwrap();
    // Someone else changes section 1 (rev 2).
    a.store
        .kb_update(
            KbId(1),
            crate::store::KbUpdate {
                expected_rev: Some(1),
                title: None,
                summary: None,
                edit: crate::store::KbEdit::ReplaceSection {
                    number: "1".into(),
                    heading: None,
                    text: "One, by Alexandre".into(),
                },
            },
            Author::Human,
        )
        .unwrap();
    // This session still holds rev 1 and replaces section 3: merged.
    let merged = a
        .kb_write(KbWriteArgs {
            id: Some("K-1".into()),
            rev: Some(1),
            op: Some("replace_section".into()),
            section: Some("3".into()),
            text: "Three, by Claude".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        merged,
        "ok K-1 rev 3 · section replaced · merged over rev 2"
    );
    let body = a.store.kb_entry(KbId(1)).unwrap().body;
    assert!(
        body.contains("One, by Alexandre") && body.contains("Three, by Claude"),
        "{body}"
    );
    // The same section changed on both sides: refused with who and what.
    let refused = a
        .kb_write(KbWriteArgs {
            id: Some("K-1".into()),
            rev: Some(2),
            op: Some("replace_section".into()),
            section: Some("3".into()),
            text: "Three again".into(),
            ..Default::default()
        })
        .unwrap_err();
    assert!(
        refused.starts_with("not saved: K-1 changed since rev 2: rev 3 by this session"),
        "{refused}"
    );
    assert!(refused.contains("Section 3 changed (+"), "{refused}");
}

#[test]
fn markers_and_budgets_hold_on_a_pathological_seed() {
    let mut ts = test_support::open();
    let (p, t, mut sess) = bound(&mut ts.store);
    // Ten old memories in a closed topic, each in an open conflict.
    let mut ids = Vec::new();
    for i in 0..12 {
        let id = ts
            .store
            .kb_create(
                NewKb {
                    owner: Owner::Topic(t),
                    kind: KbKind::Memory,
                    title: format!("{} {i}", "Long title about keychain storage ".repeat(4)),
                    summary: "S".repeat(120),
                    body: "B".repeat(800),
                    from_file: false,
                },
                Author::Claude,
            )
            .unwrap();
        ts.store
            .conn
            .execute(
                "UPDATE kb_entries SET verified_at = 0, context_branch = ?2 WHERE id = ?1 AND ?1 % 2 = 0",
                rusqlite::params![id.0, "feature/".to_owned() + &"x".repeat(32)],
            )
            .unwrap();
        ids.push(id);
    }
    for w in ids.windows(2) {
        ts.store
            .kb_record_pair(
                w[0],
                w[1],
                crate::model::KbPairKind::Conflict,
                0.5,
                Author::Claude,
            )
            .unwrap();
    }
    ts.store
        .update_topic(
            t,
            crate::store::TopicPatch {
                status: Some(Status::Done),
                ..Default::default()
            },
            Author::Human,
        )
        .unwrap();
    let mut a = agent(&mut ts.store, &mut sess);
    let list = ok(a.kb(KbArgs::default()), budget::KB);
    assert!(list.contains("?conflict K-"), "{list}");
    assert!(list.contains("on other branches"), "{list}");
    for line in list.lines().filter(|l| l.starts_with("K-")) {
        let marker = line
            .split(" ?")
            .nth(1)
            .map_or(0, |m| m.split(" - ").next().unwrap().len() + 1);
        assert!(marker <= 30, "{line}");
    }
    let found = ok(
        a.kb(KbArgs {
            query: Some("keychain".into()),
            ..Default::default()
        }),
        budget::KB,
    );
    assert!(found.contains("?branch feature/"), "{found}");
    let read = ok(
        a.kb_read(KbReadArgs {
            id: "K-1".into(),
            ..Default::default()
        }),
        budget::KB_READ,
    );
    assert!(read.contains("?conflict K-2"), "{read}");
    assert!(read.contains("?topic done"), "{read}");
    let briefing = a.session_briefing(false).unwrap();
    assert!(
        chars(&briefing) <= budget::SESSION_BRIEFING,
        "{}",
        chars(&briefing)
    );
    assert!(briefing.contains("? "), "{briefing}");
    assert!(
        briefing.contains("memories to check (kb shows why)."),
        "{briefing}"
    );
    let _ = p;
}

#[test]
fn level_branch_needs_a_feature_branch() {
    let mut ts = test_support::open();
    let (_, _, mut sess) = bound(&mut ts.store);
    let mut a = agent(&mut ts.store, &mut sess);
    let r = mem(
        &mut a,
        "branch",
        "Schema at V7 here",
        "On this branch only",
        "x",
    )
    .unwrap_err();
    assert_eq!(r, "level=branch needs a feature branch; use topic");
    let r = a
        .kb_write(KbWriteArgs {
            kind: Some("memory".into()),
            title: Some("Bad refs".into()),
            summary: Some("x".into()),
            refs: Some("T-1, Q-1, N-1, K-1".into()),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(r, "refs takes at most 3 items");
    let r = a
        .kb_write(KbWriteArgs {
            kind: Some("memory".into()),
            title: Some("A file".into()),
            summary: Some("x".into()),
            refs: Some("src/kb.rs".into()),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(r, "a file ref needs a session in an attached folder");
    let ok = a
        .kb_write(KbWriteArgs {
            kind: Some("memory".into()),
            title: Some("From the topic".into()),
            summary: Some("Sources by id".into()),
            refs: Some("T-1".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(ok.starts_with("ok K-1"), "{ok}");
    let read = a
        .kb_read(KbReadArgs {
            id: "K-1".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(read.contains("from T-1 · claude · "), "{read}");
}
