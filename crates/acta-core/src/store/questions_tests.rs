//! Questions and people in the store (data.md requests 18 and 19).

use super::test_support::{TestStore, open};
use super::*;
use crate::date::Date;
use crate::model::*;

const H: Author = Author::Human;
const C: Author = Author::Claude;

struct Fixture {
    t: TestStore,
    project: ProjectId,
    epic: EpicId,
    topic: TopicId,
    other: TopicId,
}

fn fixture() -> Fixture {
    let mut t = open();
    let s = &mut t.store;
    let project = s.create_project("Acta", "", H).unwrap();
    let epic = s.create_epic(project, "Storage", "", H).unwrap();
    let mk = |s: &mut Store, title: &str| {
        s.create_topic(
            NewTopic {
                project,
                epic: Some(epic),
                title: title.into(),
                description: String::new(),
                status: Status::Active,
            },
            H,
        )
        .unwrap()
    };
    let topic = mk(s, "Export format");
    let other = mk(s, "Backups");
    Fixture {
        t,
        project,
        epic,
        topic,
        other,
    }
}

fn ask(s: &mut Store, owner: Owner, text: &str, to: Option<&str>, by: Option<&str>) -> QuestionId {
    s.ask(
        NewQuestion {
            owner,
            text: text.into(),
            asked_to: to.map(str::to_owned),
            expected_on: by.map(|d| Date::parse_iso(d).unwrap()),
        },
        C,
    )
    .unwrap()
}

fn d(s: &str) -> Date {
    Date::parse_iso(s).unwrap()
}

#[test]
fn a_question_is_asked_edited_answered_dropped_and_reopened() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let q = ask(
        s,
        Owner::Topic(f.topic),
        "  Can we keep the v1 export\nuntil March? ",
        Some("  Marie   Dubois "),
        Some("2026-12-04"),
    );
    let x = s.question(q).unwrap();
    assert_eq!(x.text, "Can we keep the v1 export until March?");
    assert_eq!(x.asked_to.as_deref(), Some("Marie Dubois"));
    assert_eq!(x.expected_on, Some(d("2026-12-04")));
    assert_eq!((x.status, x.created_by), (QuestionStatus::Open, C));
    // A question written by Claude Code is a write to its owner.
    assert_eq!(s.topic(f.topic).unwrap().stamp.updated_by, C);
    assert!(s.unread().unwrap().topics.contains(&f.topic));
    assert_eq!(s.unread_questions().unwrap(), [(q, Owner::Topic(f.topic))]);
    assert!(s.mark_seen(Record::Question(q)).unwrap());
    assert!(s.unread_questions().unwrap().is_empty());

    // Limits.
    assert!(
        s.ask(
            NewQuestion {
                owner: Owner::Topic(f.topic),
                text: "x".repeat(301),
                asked_to: None,
                expected_on: None,
            },
            H,
        )
        .is_err()
    );
    assert!(
        s.edit_question(q, None, Some(Some(&"a".repeat(81))), H)
            .is_err()
    );
    assert!(s.answer(q, Some(&"a".repeat(1_501)), false, H).is_err());

    s.edit_question(q, Some("Can we keep v1 until March?"), Some(None), H)
        .unwrap();
    assert_eq!(s.question(q).unwrap().asked_to, None);
    s.answer(q, Some("Yes, until 31 March."), false, H).unwrap();
    let x = s.question(q).unwrap();
    assert_eq!(x.status, QuestionStatus::Answered);
    assert_eq!(x.resolved_by, Some(H));
    assert!(x.decision_entry.is_none());
    // A closed question is never overdue.
    assert!(
        s.late_answers(Scope::Project(f.project), d("2027-01-01"))
            .unwrap()
            .is_empty()
    );
    s.reopen_question(q, C).unwrap();
    let x = s.question(q).unwrap();
    assert_eq!(x.status, QuestionStatus::Open);
    assert_eq!((x.resolved_at, x.resolved_by), (None, None));
    assert_eq!(x.answer.as_deref(), Some("Yes, until 31 March."), "kept");
    s.drop_question(q, C).unwrap();
    assert_eq!(s.question(q).unwrap().status, QuestionStatus::Dropped);
}

#[test]
fn answer_and_decision_are_one_step_and_refused_off_a_topic() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let q = ask(
        s,
        Owner::Topic(f.topic),
        "Can the export leave the EU?",
        Some("Legal team"),
        None,
    );
    let before = s
        .entries(f.topic, &EntryFilter::default(), PageReq::first(50))
        .unwrap()
        .total;
    let entry = s
        .answer(q, Some("Yes, inside the EEA only."), true, H)
        .unwrap()
        .expect("logged");
    assert_eq!(entry.kind, EntryKind::Decision);
    assert_eq!(entry.author, H);
    assert_eq!(
        entry.text,
        "> Can the export leave the EU?\n\nYes, inside the EEA only."
    );
    let x = s.question(q).unwrap();
    assert_eq!(x.decision_entry, Some(entry.id));
    assert_eq!(
        s.entries(f.topic, &EntryFilter::default(), PageReq::first(50))
            .unwrap()
            .total,
        before + 1
    );
    // Logged once only.
    assert!(s.answer(q, Some("Corrected."), true, H).is_err());
    // A correction keeps when and by whom it was answered.
    s.answer(q, Some("Yes, EEA only."), false, C).unwrap();
    let y = s.question(q).unwrap();
    assert_eq!(y.answer.as_deref(), Some("Yes, EEA only."));
    assert_eq!((y.resolved_at, y.resolved_by), (x.resolved_at, Some(H)));

    // Off a topic: refused, nothing written.
    let on_epic = ask(
        s,
        Owner::Epic(f.epic),
        "Budget for the backup disks?",
        None,
        None,
    );
    let e = s.answer(on_epic, Some("200 EUR"), true, H).unwrap_err();
    assert_eq!(
        e.to_string(),
        format!(
            "log_decision needs a question on a topic; {on_epic} is on {}",
            f.epic
        )
    );
    assert_eq!(s.question(on_epic).unwrap().status, QuestionStatus::Open);

    // One transaction: the entry fails (its topic went to the trash behind
    // the store's back), so the answer is not kept either.
    let q2 = ask(s, Owner::Topic(f.other), "Which disk?", None, None);
    let entries_before: i64 = s
        .conn
        .query_row("SELECT count(*) FROM entries", [], |r| r.get(0))
        .unwrap();
    s.conn
        .execute(
            "UPDATE topics SET deleted_at = 1 WHERE id = ?1",
            [f.other.0],
        )
        .unwrap();
    assert!(s.answer(q2, Some("The big one"), true, H).is_err());
    s.conn
        .execute(
            "UPDATE topics SET deleted_at = NULL WHERE id = ?1",
            [f.other.0],
        )
        .unwrap();
    assert_eq!(s.question(q2).unwrap().status, QuestionStatus::Open);
    assert_eq!(s.question(q2).unwrap().answer, None);
    let entries_after: i64 = s
        .conn
        .query_row("SELECT count(*) FROM entries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(entries_before, entries_after);
}

#[test]
fn late_answers_follow_due_state_at_every_level() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let today = d("2026-11-22");
    let on_project = ask(
        s,
        Owner::Project(f.project),
        "Who pays?",
        None,
        Some("2026-11-12"),
    );
    let on_epic = ask(
        s,
        Owner::Epic(f.epic),
        "Which cloud?",
        None,
        Some("2026-11-20"),
    );
    let on_topic = ask(
        s,
        Owner::Topic(f.topic),
        "Format?",
        None,
        Some("2026-11-21"),
    );
    ask(
        s,
        Owner::Topic(f.topic),
        "Due today",
        None,
        Some("2026-11-22"),
    );
    ask(s, Owner::Topic(f.other), "Undated", None, None);
    let answered = ask(s, Owner::Topic(f.other), "Done", None, Some("2026-11-01"));
    s.answer(answered, None, false, H).unwrap();

    let late = s.late_answers(Scope::Project(f.project), today).unwrap();
    assert_eq!(late, [(on_project, 10), (on_epic, 2), (on_topic, 1)]);
    let epic = s.late_answers(Scope::Epic(f.epic), today).unwrap();
    assert_eq!(epic, [(on_epic, 2), (on_topic, 1)]);
    let o = s
        .overview(Scope::Project(f.project), today, crate::date::Zone::UTC)
        .unwrap();
    assert_eq!(o.answers_late, 3);
    // Per topic: open and overdue, for the epic brief and the table.
    let per = s.open_questions_by_topic(f.project, today).unwrap();
    assert_eq!(per[&f.topic], (2, 1));
    assert_eq!(per[&f.other], (1, 0));
}

#[test]
fn questions_are_ordered_moved_and_follow_their_owner_into_the_trash() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let a = ask(s, Owner::Topic(f.topic), "A?", None, None);
    let b = ask(s, Owner::Topic(f.topic), "B?", None, None);
    let c = ask(s, Owner::Topic(f.topic), "C?", None, None);
    s.reorder_question(c, 0, H).unwrap();
    let order: Vec<QuestionId> = s
        .questions_of(Owner::Topic(f.topic))
        .unwrap()
        .iter()
        .map(|q| q.id)
        .collect();
    assert_eq!(order, [c, a, b]);
    s.answer(a, None, false, H).unwrap();
    // Open first in stored order, then the closed ones.
    let order: Vec<QuestionId> = s
        .questions_of(Owner::Topic(f.topic))
        .unwrap()
        .iter()
        .map(|q| q.id)
        .collect();
    assert_eq!(order, [c, b, a]);
    s.move_question(b, Owner::Epic(f.epic), H).unwrap();
    assert_eq!(s.question(b).unwrap().owner, Owner::Epic(f.epic));
    let other = s.create_project("Other", "", H).unwrap();
    assert!(s.move_question(b, Owner::Project(other), H).is_err());

    // The topic goes to the trash with its questions, and comes back.
    s.delete(Record::Topic(f.topic), H).unwrap();
    assert!(s.question(c).is_err());
    let trash = s.trash(PageReq::first(20)).unwrap();
    assert!(
        !trash.items.iter().any(|i| i.record == Record::Question(c)),
        "listed with its topic, not on its own"
    );
    s.restore(Record::Topic(f.topic), H).unwrap();
    assert_eq!(s.question(c).unwrap().text, "C?");
    // The epic takes its own and its topics' questions.
    s.delete(Record::Epic(f.epic), H).unwrap();
    assert!(s.question(b).is_err() && s.question(c).is_err());
    s.restore(Record::Epic(f.epic), H).unwrap();
    assert!(s.question(b).is_ok() && s.question(c).is_ok());
    // A question deleted on its own is listed and restored.
    s.delete(Record::Question(c), H).unwrap();
    let trash = s.trash(PageReq::first(20)).unwrap();
    let item = trash
        .items
        .iter()
        .find(|i| i.record == Record::Question(c))
        .unwrap();
    assert_eq!(item.label, "C?");
    s.restore(Record::Question(c), H).unwrap();
    assert!(s.question(c).is_ok());
}

#[test]
fn report_to_is_set_reset_marked_and_written_to_the_timeline() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let t = Record::Topic(f.topic);
    assert!(s.set_reported(t, true, H).is_err(), "no person yet");
    s.set_report_to(t, Some("Marie"), H).unwrap();
    s.set_reported(t, true, C).unwrap();
    let r = s.topic(f.topic).unwrap().report;
    assert_eq!(r.to.as_deref(), Some("Marie"));
    assert!(r.is_reported() && r.by == Some(C));
    // The same person in another spelling keeps "reported".
    s.set_report_to(t, Some("MARIE"), H).unwrap();
    assert!(s.topic(f.topic).unwrap().report.is_reported());
    // Another person resets it.
    s.set_report_to(t, Some("Paul"), H).unwrap();
    let r = s.topic(f.topic).unwrap().report;
    assert_eq!((r.to.as_deref(), r.at), (Some("Paul"), None));
    s.set_report_to(t, Some(" "), H).unwrap();
    assert_eq!(s.topic(f.topic).unwrap().report.to, None);
    // Projects have none; notes have no "reported".
    assert!(
        s.set_report_to(Record::Project(f.project), Some("Marie"), H)
            .is_err()
    );
    let n = s.add_note(f.project, "Tell Marie", H).unwrap();
    s.set_report_to(Record::Note(n), Some("Marie"), H).unwrap();
    assert_eq!(s.note(n).unwrap().report_to.as_deref(), Some("Marie"));
    assert!(s.set_reported(Record::Note(n), true, H).is_err());
    // Epics too, without a timeline.
    s.set_report_to(Record::Epic(f.epic), Some("Legal team"), H)
        .unwrap();
    s.set_reported(Record::Epic(f.epic), true, H).unwrap();
    assert!(s.epic(f.epic).unwrap().report.is_reported());

    let events: Vec<(EventKind, Option<String>, Option<String>)> = s
        .entries(
            f.topic,
            &EntryFilter {
                include_system: true,
                ..Default::default()
            },
            PageReq::first(50),
        )
        .unwrap()
        .items
        .into_iter()
        .rev()
        .filter_map(|e| e.event)
        .filter(|e| matches!(e.kind, EventKind::Report | EventKind::Reported))
        .map(|e| (e.kind, e.old, e.new))
        .collect();
    let s_ = |v: &str| Some(v.to_owned());
    assert_eq!(
        events,
        [
            (EventKind::Report, None, s_("Marie")),
            (EventKind::Reported, None, s_("Marie")),
            (EventKind::Report, s_("Marie"), s_("MARIE")),
            (EventKind::Report, s_("MARIE"), s_("Paul")),
            (EventKind::Report, s_("Paul"), None),
        ]
    );
}

#[test]
fn filing_carries_the_first_persons_and_keeps_an_existing_one() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let note = |s: &mut Store, text: &str, to: Option<&str>| {
        let n = s.add_note(p, text, H).unwrap();
        if to.is_some() {
            s.set_report_to(Record::Note(n), to, H).unwrap();
        }
        n
    };
    let new_topic = |title: &str| NewTopic {
        project: p,
        epic: None,
        title: title.into(),
        description: String::new(),
        status: Status::Draft,
    };
    // One note: its person goes to the new topic ("Make topic").
    let one = note(s, "Ask the landlord", Some("Marie"));
    let t1 = s.note_to_topic(one, new_topic("Landlord"), H).unwrap();
    assert_eq!(s.topic(t1).unwrap().report.to.as_deref(), Some("Marie"));
    // Several notes: the first one naming someone, in the order given.
    let a = note(s, "No one", None);
    let b = note(s, "For Paul", Some("Paul"));
    let c = note(s, "For Marie", Some("Marie"));
    let t2 = s.create_topic(new_topic("Mixed"), H).unwrap();
    s.file_notes(&[a, b, c], t2, H).unwrap();
    assert_eq!(s.topic(t2).unwrap().report.to.as_deref(), Some("Paul"));
    // An epic made from notes.
    let e = s.create_epic(p, "Sync", "", H).unwrap();
    let g = note(s, "Sync", Some("Legal team"));
    s.file_notes_in_epic(&[g], e, H).unwrap();
    assert_eq!(s.epic(e).unwrap().report.to.as_deref(), Some("Legal team"));
    // An existing topic with its own person keeps it.
    s.set_report_to(Record::Topic(f.topic), Some("Alexandre"), H)
        .unwrap();
    let h = note(s, "Entry for Marie", Some("Marie"));
    s.file_notes(&[h], f.topic, H).unwrap();
    assert_eq!(
        s.topic(f.topic).unwrap().report.to.as_deref(),
        Some("Alexandre")
    );
    // A dismissed note's person goes nowhere; the note keeps it.
    let i = note(s, "Noise", Some("Zoé"));
    s.dismiss_notes(&[i], H).unwrap();
    assert_eq!(s.note(i).unwrap().report_to.as_deref(), Some("Zoé"));

    // People: grouped by key, open items first, spelled as written last.
    s.set_report_to(Record::Topic(f.other), Some("marie"), H)
        .unwrap();
    // The newest write, beyond the same millisecond.
    s.conn
        .execute(
            "UPDATE topics SET updated_at = updated_at + 60000 WHERE id = ?1",
            [f.other.0],
        )
        .unwrap();
    let people = s.people(p).unwrap();
    let marie = people.iter().find(|x| x.key == "marie").unwrap();
    assert_eq!(marie.display, "marie");
    // Topics T1 and f.other, the open note "For Marie" was filed (not open).
    assert_eq!(marie.open, 2);
    assert!(people.iter().any(|x| x.key == "zoe" && x.open == 0));
    let targets = s.report_targets(p, "marie").unwrap();
    let ids: Vec<TopicId> = targets.topics.iter().map(|t| t.id).collect();
    assert!(ids.contains(&t1) && ids.contains(&f.other));
    assert!(targets.notes.is_empty());
}

#[test]
fn questions_and_people_survive_an_export_and_an_import() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let q = ask(
        s,
        Owner::Topic(f.topic),
        "Can we keep v1?",
        Some("Hélène"),
        Some("2026-12-04"),
    );
    s.answer(q, Some("Yes."), true, H).unwrap();
    ask(s, Owner::Project(f.project), "Who pays?", None, None);
    s.set_report_to(Record::Topic(f.topic), Some("Marie"), H)
        .unwrap();
    s.set_reported(Record::Topic(f.topic), true, H).unwrap();
    s.set_report_to(Record::Epic(f.epic), Some("Legal team"), H)
        .unwrap();
    let n = s.add_note(f.project, "Tell Paul", H).unwrap();
    s.set_report_to(Record::Note(n), Some("Paul"), H).unwrap();
    let file = s.export(None).unwrap();
    assert_eq!(file.version, 3);
    assert_eq!(file.questions.len(), 2);
    let json = file.to_json().unwrap();
    // Keys are never exported.
    assert!(!json.contains("asked_key") && !json.contains("report_key"));

    let mut b = open();
    let report = b.store.import(&file, false).unwrap();
    assert_eq!(report.questions.created, 2);
    let again = b.store.export(None).unwrap();
    assert_eq!(again.questions, file.questions);
    assert_eq!(again.topics, file.topics);
    assert_eq!(again.epics, file.epics);
    assert_eq!(again.notes, file.notes);
    // Keys recomputed here: the person filter finds them.
    let found = b
        .store
        .find_questions(
            b.store.projects().unwrap()[0].id,
            &QuestionFilter {
                person_key: Some("helene".into()),
                statuses: Vec::new(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].decision_entry.is_some(), "the link to the entry");
    // Imported questions count as seen.
    assert!(b.store.unread_questions().unwrap().is_empty());
    // Twice: nothing changes.
    let report = b.store.import(&file, false).unwrap();
    assert_eq!(report.questions.created + report.questions.updated, 0);
}

#[test]
fn a_version_2_export_imports_and_its_updates_keep_the_people() {
    let mut f = fixture();
    let s = &mut f.t.store;
    s.set_report_to(Record::Topic(f.topic), Some("Marie"), H)
        .unwrap();
    s.set_reported(Record::Topic(f.topic), true, H).unwrap();
    s.set_report_to(Record::Epic(f.epic), Some("Legal team"), H)
        .unwrap();
    let n = s.add_note(f.project, "Tell Paul", H).unwrap();
    s.set_report_to(Record::Note(n), Some("Paul"), H).unwrap();
    ask(s, Owner::Topic(f.topic), "Can we keep v1?", None, None);
    let file = s.export(None).unwrap();

    // The same records, newer, as a version 2 file carries them.
    let mut v2: serde_json::Value = serde_json::from_str(&file.to_json().unwrap()).unwrap();
    v2["version"] = 2.into();
    v2.as_object_mut().unwrap().remove("questions");
    for kind in ["epics", "topics", "notes"] {
        for x in v2[kind].as_array_mut().unwrap() {
            let o = x.as_object_mut().unwrap();
            for k in ["report_to", "reported_at", "reported_by"] {
                o.remove(k);
            }
            let at = o["updated_at"].as_i64().unwrap();
            o.insert("updated_at".into(), (at + 1_000).into());
        }
    }
    let old = ExportFile::from_json(&v2.to_string()).unwrap();
    assert!(old.questions.is_empty());
    // Into this database: updated, the people kept.
    let report = s.import(&old, false).unwrap();
    assert!(report.topics.updated > 0 && report.notes.updated > 0);
    let r = s.topic(f.topic).unwrap().report;
    assert_eq!(r.to.as_deref(), Some("Marie"));
    assert!(r.is_reported());
    assert_eq!(
        s.epic(f.epic).unwrap().report.to.as_deref(),
        Some("Legal team")
    );
    assert_eq!(s.note(n).unwrap().report_to.as_deref(), Some("Paul"));
    // Into an empty one: no questions, no people.
    let mut b = open();
    let report = b.store.import(&old, false).unwrap();
    assert!(report.topics.created > 0);
    let again = b.store.export(None).unwrap();
    assert!(again.questions.is_empty());
    assert!(again.topics.iter().all(|t| t.report.report_to.is_none()));
    assert!(again.notes.iter().all(|n| n.report_to.is_none()));
}
