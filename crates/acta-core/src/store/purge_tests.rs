//! Purging the trash on demand (data.md 11, request 25).

use super::test_support;
use super::*;
use crate::model::{
    AttachOwner, Attachment, ContactLevel, EntryKind, KbId, KbKind, NoteId, PageReq, Record, Status,
};

const H: Author = Author::Human;

fn topic(s: &mut Store, project: ProjectId, epic: Option<EpicId>, title: &str) -> TopicId {
    s.create_topic(
        NewTopic {
            project,
            epic,
            title: title.into(),
            description: String::new(),
            status: Status::Active,
        },
        H,
    )
    .unwrap()
}

fn add(s: &mut Store, owner: AttachOwner, name: &str, data: Vec<u8>) -> Attachment {
    s.attach(owner, name, AttachSource::Bytes(data), H, &mut |_, _| {})
        .unwrap()
}

fn count(s: &Store, sql: &str) -> i64 {
    s.conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Blobs are stored now, inside the sweep's grace: age them.
fn age_blobs(s: &Store) {
    s.conn
        .execute("UPDATE attachment_blobs SET created_at = 0", [])
        .unwrap();
}

fn foreign_keys_hold(s: &Store) {
    let mut stmt = s.conn.prepare("PRAGMA foreign_key_check").unwrap();
    let broken = stmt.query_map([], |_| Ok(())).unwrap().count();
    assert_eq!(broken, 0, "a row names a purged one");
}

fn trash_len(s: &Store) -> i64 {
    s.trash(PageReq::first(PageReq::MAX_LIMIT)).unwrap().total
}

fn kind(report: &PurgeReport, kind: &str) -> u64 {
    report
        .counts
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(0, |(_, n)| *n)
}

fn kb(s: &mut Store, owner: Owner, title: &str) -> KbId {
    s.kb_create(
        NewKb {
            owner,
            kind: KbKind::Memory,
            title: title.into(),
            summary: "One line.".into(),
            body: "Body.".into(),
            from_file: false,
        },
        H,
    )
    .unwrap()
}

#[test]
fn a_project_goes_with_everything_under_it_and_the_counts_say_so() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let keep = s.create_project("Kept", "", H).unwrap();
    let p = s.create_project("Alexandre's site", "", H).unwrap();
    let e = s.create_epic(p, "Launch", "", H).unwrap();
    let t1 = topic(s, p, Some(e), "Hosting");
    let t2 = topic(s, p, None, "Domain");
    for t in [t1, t2] {
        s.log(
            t,
            NewEntry {
                kind: EntryKind::Progress,
                text: "Compared three hosts.".into(),
                state: None,
                status: None,
            },
            H,
        )
        .unwrap();
    }
    let n = s.add_note(p, "Ask Alexandre about DNS", H).unwrap();
    let k = kb(s, Owner::Topic(t1), "Host choice");
    let q = s
        .ask(
            NewQuestion {
                owner: Owner::Epic(e),
                text: "Which region?".into(),
                asked_to: None,
                expected_on: None,
            },
            H,
        )
        .unwrap();
    add(s, AttachOwner::Note(n), "plan.txt", b"plan".to_vec());
    // A topic trashed on its own before the project: it goes too.
    s.delete(Record::Topic(t2), H).unwrap();
    s.delete(Record::Project(p), H).unwrap();
    let live_note = s.add_note(keep, "Stays", H).unwrap();
    age_blobs(s);
    assert_eq!(s.trash_project_content(p).unwrap(), (2, 1));
    assert_eq!(s.trash_project_content(keep).unwrap(), (0, 0));

    let report = s.trash_purge(&[Record::Project(p)]).unwrap();
    assert!(report.skipped.is_empty());
    assert_eq!(kind(&report, "project"), 1);
    assert_eq!(kind(&report, "epic"), 1);
    assert_eq!(kind(&report, "topic"), 2);
    assert!(kind(&report, "entry") >= 2);
    assert_eq!(kind(&report, "note"), 1);
    assert_eq!(kind(&report, "kb"), 1);
    assert_eq!(kind(&report, "question"), 1);
    assert_eq!(kind(&report, "attachment"), 1);
    assert_eq!((report.blobs, report.bytes), (1, 4));
    assert!(report.total() >= 10);
    for sql in [
        "SELECT count(*) FROM projects WHERE title = 'Alexandre''s site'",
        "SELECT count(*) FROM topics",
        "SELECT count(*) FROM kb_revisions",
        "SELECT count(*) FROM attachment_chunks",
    ] {
        assert_eq!(count(s, sql), 0, "{sql}");
    }
    assert!(s.note(live_note).is_ok());
    assert_eq!(
        count(
            s,
            &format!("SELECT count(*) FROM kb_entries WHERE id = {}", k.0)
        ),
        0
    );
    assert!(s.question(q).is_err());
    assert_eq!(trash_len(s), 0);
    foreign_keys_hold(s);
}

#[test]
fn a_blob_shared_with_a_live_note_survives() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let gone = s.add_note(p, "Goes", H).unwrap();
    let stays = s.add_note(p, "Stays", H).unwrap();
    add(s, AttachOwner::Note(gone), "shared.txt", b"shared".to_vec());
    add(s, AttachOwner::Note(gone), "only.txt", b"only!!!".to_vec());
    let kept = add(s, AttachOwner::Note(stays), "copy.txt", b"shared".to_vec());
    s.delete(Record::Note(gone), H).unwrap();
    age_blobs(s);
    let report = s.trash_purge(&[Record::Note(gone)]).unwrap();
    assert_eq!(kind(&report, "note"), 1);
    assert_eq!(kind(&report, "attachment"), 2);
    assert_eq!((report.blobs, report.bytes), (1, 7));
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut s.read_attachment(kept.id).unwrap(), &mut out).unwrap();
    assert_eq!(out, b"shared");
    foreign_keys_hold(s);
}

#[test]
fn a_contact_still_named_is_skipped_and_one_under_a_purged_project_is_lifted() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let other = s.create_project("Other", "", H).unwrap();
    let t = topic(s, p, None, "Review");
    let alexandre = s
        .create_contact("Alexandre", ContactLevel::Project(p), H)
        .unwrap();
    let marie = s.create_contact("Marie", ContactLevel::All, H).unwrap();
    s.set_report_to(Record::Topic(t), Some(marie), H).unwrap();
    // Marie is trashed while the live topic still names her.
    s.delete(Record::Contact(marie), H).unwrap();
    let report = s.trash_purge(&[Record::Contact(marie)]).unwrap();
    assert_eq!(report.skipped, [Record::Contact(marie)]);
    assert_eq!(report.total(), 0);
    assert!(s.contact_any(marie).is_ok());

    // Alexandre lives in the project, and a note elsewhere names him
    // (made by hand: the level rules would refuse it).
    let n = s.add_note(other, "Call Alexandre", H).unwrap();
    s.conn
        .execute(
            "UPDATE notes SET report_contact_id = ?1 WHERE id = ?2",
            rusqlite::params![alexandre.0, n.0],
        )
        .unwrap();
    s.delete(Record::Project(p), H).unwrap();
    let report = s.trash_purge(&[Record::Project(p)]).unwrap();
    assert_eq!(kind(&report, "contact"), 0);
    let lifted = s.contact_any(alexandre).unwrap();
    assert_eq!(lifted.level, ContactLevel::All);
    foreign_keys_hold(s);

    // Unnamed, a contact goes.
    s.set_report_to(Record::Topic(t), None, H).ok();
    let lone = s.create_contact("Lone", ContactLevel::All, H).unwrap();
    s.delete(Record::Contact(lone), H).unwrap();
    let report = s.trash_purge(&[Record::Contact(lone)]).unwrap();
    assert_eq!(kind(&report, "contact"), 1);
    assert!(s.contact_any(lone).is_err());
    foreign_keys_hold(s);
}

#[test]
fn an_id_restored_meanwhile_is_skipped() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let t = topic(s, p, None, "Back again");
    let n = s.add_note(p, "Goes", H).unwrap();
    s.delete(Record::Topic(t), H).unwrap();
    s.delete(Record::Note(n), H).unwrap();
    s.restore(Record::Topic(t), H).unwrap();
    let report = s
        .trash_purge(&[Record::Topic(t), Record::Note(n), Record::Note(NoteId(999))])
        .unwrap();
    assert_eq!(
        report.skipped,
        [Record::Topic(t), Record::Note(NoteId(999))]
    );
    assert_eq!(kind(&report, "note"), 1);
    assert!(s.topic(t).is_ok());
    foreign_keys_hold(s);
}

#[test]
fn emptying_the_trash_leaves_it_empty_and_live_notes_filed_into_a_purged_topic_live_on() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Acta", "", H).unwrap();
    let t = topic(s, p, None, "Filed into");
    let n = s.add_note(p, "Filed note of Alexandre", H).unwrap();
    s.file_notes(&[n], t, H).unwrap();
    let folder = ts.dir.path().join("work");
    std::fs::create_dir(&folder).unwrap();
    let s = &mut ts.store;
    let f = s.add_folder(Owner::Topic(t), &folder, H).unwrap();
    let k = kb(s, Owner::Project(p), "Kept memory");
    s.conn
        .execute(
            "UPDATE kb_entries SET created_topic_id = ?1, context_folder_id = ?2 WHERE id = ?3",
            rusqlite::params![t.0, f.0, k.0],
        )
        .unwrap();
    s.delete(Record::Topic(t), H).unwrap();
    let gone = s.add_note(p, "Trashed", H).unwrap();
    s.delete(Record::Note(gone), H).unwrap();
    assert_eq!(trash_len(s), 2);

    let report = s.trash_purge_all().unwrap();
    assert_eq!(kind(&report, "topic"), 1);
    assert_eq!(kind(&report, "folder"), 1);
    assert_eq!(trash_len(s), 0);
    let filed: Option<i64> = s
        .conn
        .query_row(
            "SELECT filed_topic_id FROM notes WHERE id = ?1",
            [n.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(filed, None);
    assert!(s.note(n).is_ok());
    assert_eq!(
        count(
            s,
            &format!("SELECT count(*) FROM kb_entries WHERE id = {}", k.0)
        ),
        1
    );
    foreign_keys_hold(s);
    // An empty trash purges nothing.
    assert_eq!(s.trash_purge_all().unwrap(), PurgeReport::default());
}

/// Request 25: the purge is the human's. Neither the agent layer (the MCP
/// tools and the agent CLI) nor the `acta` command line calls it.
#[test]
fn nothing_the_agent_reaches_can_purge() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut stack = vec![
        crates.join("acta-core").join("src").join("agent"),
        crates.join("acta-cli").join("src"),
    ];
    let mut checked = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("a source dir") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("a source file");
                // Spelled in two parts so this file does not match itself.
                let call = ["trash", "_purge"].concat();
                assert!(!text.contains(&call), "{} purges", path.display());
                checked += 1;
            }
        }
    }
    assert!(checked > 5);
}

#[test]
fn a_purged_page_takes_its_drafts_and_addresses_and_other_pages_keep_theirs() {
    let mut ts = test_support::open();
    let s = &mut ts.store;
    let p = s.create_project("Alexandre's site", "", H).unwrap();
    let gone = topic(s, p, None, "Hosting");
    let kept = topic(s, p, None, "Domain");
    for t in [gone, kept] {
        let route = format!("T-{}", t.0);
        s.set_local_value(
            &format!("draft.{route}.description"),
            Some("Alexandre's draft"),
        )
        .unwrap();
        s.set_local_value(&format!("draft.{route}.state"), Some("Half a sentence"))
            .unwrap();
        s.set_local_value(
            &format!("browser.addresses.{route}"),
            Some("[\"https://example.com/\"]"),
        )
        .unwrap();
    }
    s.delete(Record::Topic(gone), H).unwrap();
    s.trash_purge(&[Record::Topic(gone)]).unwrap();
    let left = |s: &Store, t: TopicId| {
        let route = format!("T-{}", t.0);
        s.local_values(&format!("draft.{route}.")).unwrap().len()
            + usize::from(
                s.local_value(&format!("browser.addresses.{route}"))
                    .unwrap()
                    .is_some(),
            )
    };
    assert_eq!(left(s, gone), 0);
    assert_eq!(left(s, kept), 3);
}
