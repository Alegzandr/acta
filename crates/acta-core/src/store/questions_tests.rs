//! Questions, contacts and the profile in the store (data.md requests 18,
//! 19, 22 and 23).

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

fn ask(
    s: &mut Store,
    owner: Owner,
    text: &str,
    to: Option<ContactId>,
    by: Option<&str>,
) -> QuestionId {
    s.ask(
        NewQuestion {
            owner,
            text: text.into(),
            asked_to: to,
            expected_on: by.map(|d| Date::parse_iso(d).unwrap()),
        },
        C,
    )
    .unwrap()
}

fn d(s: &str) -> Date {
    Date::parse_iso(s).unwrap()
}

fn contact(s: &mut Store, name: &str, level: ContactLevel) -> ContactId {
    s.create_contact(name, level, H).unwrap()
}

#[test]
fn a_question_is_asked_edited_answered_dropped_and_reopened() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let marie = contact(s, "  Marie   Dubois ", ContactLevel::Project(f.project));
    assert_eq!(s.contact(marie).unwrap().name, "Marie Dubois");
    let q = ask(
        s,
        Owner::Topic(f.topic),
        "  Can we keep the v1 export\nuntil March? ",
        Some(marie),
        Some("2026-12-04"),
    );
    let x = s.question(q).unwrap();
    assert_eq!(x.text, "Can we keep the v1 export until March?");
    let asked = x.asked_to.unwrap();
    assert_eq!((asked.id, asked.name.as_str()), (marie, "Marie Dubois"));
    assert_eq!(x.expected_on, Some(d("2026-12-04")));
    assert_eq!((x.status, x.created_by), (QuestionStatus::Open, C));
    // A question written by Claude Code is a write to its owner.
    assert_eq!(s.topic(f.topic).unwrap().stamp.updated_by, C);
    assert!(s.unread().unwrap().topics.contains(&f.topic));
    assert_eq!(s.unread_questions().unwrap(), [(q, Owner::Topic(f.topic))]);
    assert!(s.mark_seen(Record::Question(q)).unwrap());
    assert!(s.unread_questions().unwrap().is_empty());

    // Limits.
    let long = NewQuestion {
        owner: Owner::Topic(f.topic),
        text: "x".repeat(301),
        asked_to: None,
        expected_on: None,
    };
    assert!(s.ask(long, H).is_err());
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
    let legal = contact(s, "Legal team", ContactLevel::All);
    let q = ask(
        s,
        Owner::Topic(f.topic),
        "Can the export leave the EU?",
        Some(legal),
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
    let on_epic = ask(s, Owner::Epic(f.epic), "Budget for the disks?", None, None);
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
    let count = |s: &Store| -> i64 {
        s.conn
            .query_row("SELECT count(*) FROM entries", [], |r| r.get(0))
            .unwrap()
    };
    let entries_before = count(s);
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
    assert_eq!(entries_before, count(s));
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
    let order = |s: &Store| -> Vec<QuestionId> {
        s.questions_of(Owner::Topic(f.topic))
            .unwrap()
            .iter()
            .map(|q| q.id)
            .collect()
    };
    assert_eq!(order(s), [c, a, b]);
    s.answer(a, None, false, H).unwrap();
    assert_eq!(order(s), [c, b, a], "open first, then closed");
    s.move_question(b, Owner::Epic(f.epic), H).unwrap();
    assert_eq!(s.question(b).unwrap().owner, Owner::Epic(f.epic));
    let other = s.create_project("Other", "", H).unwrap();
    assert!(s.move_question(b, Owner::Project(other), H).is_err());

    s.delete(Record::Topic(f.topic), H).unwrap();
    assert!(s.question(c).is_err());
    let trash = s.trash(PageReq::first(20)).unwrap();
    assert!(
        !trash.items.iter().any(|i| i.record == Record::Question(c)),
        "listed with its topic, not on its own"
    );
    s.restore(Record::Topic(f.topic), H).unwrap();
    assert_eq!(s.question(c).unwrap().text, "C?");
    s.delete(Record::Epic(f.epic), H).unwrap();
    assert!(s.question(b).is_err() && s.question(c).is_err());
    s.restore(Record::Epic(f.epic), H).unwrap();
    assert!(s.question(b).is_ok() && s.question(c).is_ok());
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
    let marie = contact(s, "Marie", ContactLevel::Project(f.project));
    let paul = contact(s, "Paul", ContactLevel::Epic(f.project, f.epic));
    let t = Record::Topic(f.topic);
    assert!(s.set_reported(t, true, H).is_err(), "no contact yet");
    s.set_report_to(t, Some(marie), H).unwrap();
    s.set_reported(t, true, C).unwrap();
    let r = s.topic(f.topic).unwrap().report;
    assert_eq!(r.to.as_ref().map(|c| c.name.as_str()), Some("Marie"));
    assert!(r.is_reported() && r.by == Some(C));
    // Another contact resets it.
    s.set_report_to(t, Some(paul), H).unwrap();
    let r = s.topic(f.topic).unwrap().report;
    assert_eq!((r.to.map(|c| c.id), r.at), (Some(paul), None));
    s.set_report_to(t, None, H).unwrap();
    assert_eq!(s.topic(f.topic).unwrap().report.to, None);
    // Projects have none; notes have no "reported"; a note sees no epic
    // contact.
    assert!(
        s.set_report_to(Record::Project(f.project), Some(marie), H)
            .is_err()
    );
    let n = s.add_note(f.project, "Tell Marie", H).unwrap();
    let e = s.set_report_to(Record::Note(n), Some(paul), H).unwrap_err();
    assert!(matches!(e, Error::OutOfScope { .. }), "{e}");
    assert_eq!(
        e.to_string(),
        format!("{paul} is not visible from {n} (it belongs to {})", f.epic)
    );
    s.set_report_to(Record::Note(n), Some(marie), H).unwrap();
    assert!(s.set_reported(Record::Note(n), true, H).is_err());
    s.set_report_to(Record::Epic(f.epic), Some(paul), H)
        .unwrap();
    s.set_reported(Record::Epic(f.epic), true, H).unwrap();
    assert!(s.epic(f.epic).unwrap().report.is_reported());

    // Events hold the contacts' UUIDs.
    let uuid = |c: ContactId| Some(s.contact(c).unwrap().stamp.uuid);
    let (um, up) = (uuid(marie), uuid(paul));
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
    assert_eq!(
        events,
        [
            (EventKind::Report, None, um.clone()),
            (EventKind::Reported, None, um.clone()),
            (EventKind::Report, um, up.clone()),
            (EventKind::Report, up, None),
        ]
    );
}

#[test]
fn filing_carries_the_first_contact_and_keeps_an_existing_one() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let marie = contact(s, "Marie", ContactLevel::Project(p));
    let paul = contact(s, "Paul", ContactLevel::All);
    let note = |s: &mut Store, text: &str, to: Option<ContactId>| {
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
    let rep = |s: &Store, t: TopicId| s.topic(t).unwrap().report.to.map(|c| c.id);
    let one = note(s, "Ask the landlord", Some(marie));
    let t1 = s.note_to_topic(one, new_topic("Landlord"), H).unwrap();
    assert_eq!(rep(s, t1), Some(marie));
    let a = note(s, "No one", None);
    let b = note(s, "For Paul", Some(paul));
    let c = note(s, "For Marie", Some(marie));
    let t2 = s.create_topic(new_topic("Mixed"), H).unwrap();
    s.file_notes(&[a, b, c], t2, H).unwrap();
    assert_eq!(rep(s, t2), Some(paul), "the first naming someone");
    let e = s.create_epic(p, "Sync", "", H).unwrap();
    let g = note(s, "Sync", Some(paul));
    s.file_notes_in_epic(&[g], e, H).unwrap();
    assert_eq!(s.epic(e).unwrap().report.to.map(|c| c.id), Some(paul));
    s.set_report_to(Record::Topic(f.topic), Some(paul), H)
        .unwrap();
    let h = note(s, "Entry for Marie", Some(marie));
    s.file_notes(&[h], f.topic, H).unwrap();
    assert_eq!(rep(s, f.topic), Some(paul), "keeps its own");

    let targets = s.report_targets(p, marie).unwrap();
    let ids: Vec<TopicId> = targets.topics.iter().map(|t| t.id).collect();
    assert_eq!(ids, [t1]);
    assert!(targets.notes.is_empty());
}

#[test]
fn contacts_move_up_freely_and_down_only_while_every_use_stays_in_scope() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let marie = contact(s, "Marie", ContactLevel::Project(p));
    let loose = s
        .create_topic(
            NewTopic {
                project: p,
                epic: None,
                title: "Loose".into(),
                description: String::new(),
                status: Status::Draft,
            },
            H,
        )
        .unwrap();
    s.set_report_to(Record::Topic(loose), Some(marie), H)
        .unwrap();
    let q = ask(s, Owner::Project(p), "Who signs?", Some(marie), None);
    ask(s, Owner::Topic(f.topic), "In the epic", Some(marie), None);
    let e = s
        .move_contact(marie, ContactLevel::Epic(p, f.epic), H)
        .unwrap_err();
    match &e {
        Error::ContactInUse { places, .. } => {
            let ids: Vec<&str> = places.iter().map(|(id, _)| id.as_str()).collect();
            assert!(ids.contains(&loose.to_string().as_str()), "{e}");
            assert!(ids.contains(&q.to_string().as_str()), "{e}");
            assert_eq!(places.len(), 2, "the epic's own use is fine: {e}");
        }
        other => panic!("{other}"),
    }
    assert!(
        e.to_string()
            .starts_with(&format!("{marie} is named outside {}: ", f.epic))
    );
    assert!(e.to_string().ends_with("; change those first"));
    assert_eq!(
        s.contact(marie).unwrap().level,
        ContactLevel::Project(p),
        "nothing moved"
    );
    s.move_contact(marie, ContactLevel::All, H).unwrap();
    assert_eq!(s.contact(marie).unwrap().level, ContactLevel::All);
    // An epic not in the level's project is refused.
    let other = s.create_project("Other", "", H).unwrap();
    assert!(
        s.move_contact(marie, ContactLevel::Epic(other, f.epic), H)
            .is_err()
    );
}

#[test]
fn moving_a_record_lifts_the_contacts_it_names() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let e2 = s.create_epic(p, "Sync", "", H).unwrap();
    let paul = contact(s, "Paul", ContactLevel::Epic(p, f.epic));
    let zoe = contact(s, "Zoé", ContactLevel::Epic(p, f.epic));
    s.set_report_to(Record::Topic(f.topic), Some(paul), H)
        .unwrap();
    ask(s, Owner::Topic(f.topic), "Which format?", Some(zoe), None);
    assert!(s.take_lifts().is_empty());
    s.update_topic(
        f.topic,
        TopicPatch {
            epic: Some(Some(e2)),
            ..Default::default()
        },
        H,
    )
    .unwrap();
    // Both widened to the project, the narrowest level seeing both epics.
    assert_eq!(s.contact(paul).unwrap().level, ContactLevel::Project(p));
    assert_eq!(s.contact(zoe).unwrap().level, ContactLevel::Project(p));
    let lifts = s.take_lifts();
    assert_eq!(lifts.len(), 2);
    assert!(
        lifts
            .iter()
            .all(|l| l.from == ContactLevel::Epic(p, f.epic) && l.to == ContactLevel::Project(p))
    );
    assert!(s.take_lifts().is_empty(), "taken once");
    // A question moved up from an epic topic to the project.
    let ann = contact(s, "Ann", ContactLevel::Epic(p, e2));
    let q = ask(s, Owner::Topic(f.topic), "Who signs?", Some(ann), None);
    s.move_question(q, Owner::Project(p), H).unwrap();
    assert_eq!(s.contact(ann).unwrap().level, ContactLevel::Project(p));
    assert_eq!(s.take_lifts().len(), 1);
    // Nothing to lift: nothing changes.
    s.update_topic(
        f.topic,
        TopicPatch {
            epic: Some(None),
            ..Default::default()
        },
        H,
    )
    .unwrap();
    assert!(s.take_lifts().is_empty());
}

#[test]
fn merging_keeps_references_and_lands_at_the_wider_level() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let a = contact(s, "Marie", ContactLevel::Epic(p, f.epic));
    let b = contact(s, "Marie Dubois", ContactLevel::Project(p));
    s.update_contact(
        a,
        ContactPatch {
            job_title: Some(Some("Head of legal".into())),
            address_note: Some(Some("Formal, in French.".into())),
            ..Default::default()
        },
        H,
    )
    .unwrap();
    s.update_contact(
        b,
        ContactPatch {
            company: Some(Some("Acme".into())),
            address_note: Some(Some("Prefers e-mail.".into())),
            ..Default::default()
        },
        H,
    )
    .unwrap();
    s.set_report_to(Record::Topic(f.topic), Some(a), H).unwrap();
    let q = ask(s, Owner::Topic(f.other), "Sign?", Some(a), None);
    s.mark_distinct(a, b).unwrap();
    assert!(s.is_distinct(b, a).unwrap());
    s.merge_contacts(a, b, H).unwrap();
    let m = s.contact(b).unwrap();
    assert_eq!(m.name, "Marie Dubois");
    assert_eq!(m.level, ContactLevel::Project(p));
    assert_eq!(m.job_title.as_deref(), Some("Head of legal"));
    assert_eq!(m.company.as_deref(), Some("Acme"));
    assert_eq!(
        m.address_note.as_deref(),
        Some("Prefers e-mail.\n\nFormal, in French.")
    );
    assert_eq!(s.topic(f.topic).unwrap().report.to.map(|c| c.id), Some(b));
    assert_eq!(s.question(q).unwrap().asked_to.map(|c| c.id), Some(b));
    // The event of the old reference now names the merged contact.
    let uuid_b = s.contact(b).unwrap().stamp.uuid;
    let report_events: Vec<Option<String>> = s
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
        .filter_map(|e| e.event)
        .filter(|e| e.kind == EventKind::Report)
        .map(|e| e.new)
        .collect();
    assert_eq!(report_events, [Some(uuid_b)]);
    // `from` is in the trash, merged into `b`.
    assert!(s.contact(a).is_err());
    assert_eq!(s.contact_any(a).unwrap().merged_into, Some(b));
    assert!(
        s.trash(PageReq::first(20))
            .unwrap()
            .items
            .iter()
            .any(|i| i.record == Record::Contact(a))
    );
    // Two epics' contacts merge at their project; two projects' at all.
    let e2 = s.create_epic(p, "Sync", "", H).unwrap();
    let x = contact(s, "X", ContactLevel::Epic(p, f.epic));
    let y = contact(s, "Y", ContactLevel::Epic(p, e2));
    s.merge_contacts(x, y, H).unwrap();
    assert_eq!(s.contact(y).unwrap().level, ContactLevel::Project(p));
}

#[test]
fn a_deleted_contact_stays_named_and_levels_carry_their_contacts() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let marie = contact(s, "Marie", ContactLevel::Project(p));
    s.set_report_to(Record::Topic(f.topic), Some(marie), H)
        .unwrap();
    s.delete(Record::Contact(marie), H).unwrap();
    let r = s.topic(f.topic).unwrap().report.to.unwrap();
    assert_eq!((r.id, r.name.as_str(), r.in_trash), (marie, "Marie", true));
    assert!(
        s.contacts_visible(Place {
            project: p,
            epic: None
        })
        .unwrap()
        .is_empty()
    );
    // A trashed contact cannot be newly named.
    assert!(
        s.set_report_to(Record::Topic(f.other), Some(marie), H)
            .is_err()
    );
    s.restore(Record::Contact(marie), H).unwrap();
    assert!(!s.topic(f.topic).unwrap().report.to.unwrap().in_trash);
    // An epic's contacts go to the trash with it, and come back.
    let paul = contact(s, "Paul", ContactLevel::Epic(p, f.epic));
    s.delete(Record::Epic(f.epic), H).unwrap();
    assert!(s.contact(paul).is_err());
    let trash = s.trash(PageReq::first(20)).unwrap();
    assert!(
        !trash
            .items
            .iter()
            .any(|i| i.record == Record::Contact(paul))
    );
    s.restore(Record::Epic(f.epic), H).unwrap();
    assert!(s.contact(paul).is_ok());
    s.delete(Record::Project(p), H).unwrap();
    assert!(s.contact(marie).is_err() && s.contact(paul).is_err());
    s.restore(Record::Project(p), H).unwrap();
    assert!(s.contact(marie).is_ok() && s.contact(paul).is_ok());
}

#[test]
fn visible_contacts_are_listed_nearest_level_first() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let other = s.create_project("Other", "", H).unwrap();
    let e2 = s.create_epic(p, "Sync", "", H).unwrap();
    let all = contact(s, "Zoe", ContactLevel::All);
    let proj = contact(s, "Marie", ContactLevel::Project(p));
    let epic = contact(s, "Paul", ContactLevel::Epic(p, f.epic));
    contact(s, "Sibling", ContactLevel::Epic(p, e2));
    contact(s, "Elsewhere", ContactLevel::Project(other));
    let ids = |place| -> Vec<ContactId> {
        s.contacts_visible(place)
            .unwrap()
            .iter()
            .map(|c| c.contact.id)
            .collect()
    };
    assert_eq!(
        ids(Place {
            project: p,
            epic: Some(f.epic)
        }),
        [epic, proj, all]
    );
    assert_eq!(
        ids(Place {
            project: p,
            epic: None
        }),
        [proj, all]
    );
    assert_eq!(
        ids(Place {
            project: other,
            epic: None
        })
        .len(),
        2
    );
}

#[test]
fn the_profile_is_set_and_cleared() {
    let mut t = open();
    let s = &mut t.store;
    assert_eq!(s.profile().unwrap(), Profile::default());
    s.set_profile(Some(" Alexandre "), Some("Product designer"))
        .unwrap();
    let p = s.profile().unwrap();
    assert_eq!(p.name.as_deref(), Some("Alexandre"));
    assert_eq!(p.job_title.as_deref(), Some("Product designer"));
    s.set_profile(Some("Alexandre"), Some("")).unwrap();
    assert_eq!(s.profile().unwrap().job_title, None);
    assert!(s.set_profile(Some(&"a".repeat(81)), None).is_err());
}

#[test]
fn questions_contacts_and_the_profile_survive_an_export_and_an_import() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let helene = contact(s, "Hélène", ContactLevel::Epic(f.project, f.epic));
    let marie = contact(s, "Marie", ContactLevel::All);
    let paul = contact(s, "Paul", ContactLevel::Project(f.project));
    s.update_contact(
        marie,
        ContactPatch {
            job_title: Some(Some("Head of legal".into())),
            company: Some(Some("Acme".into())),
            address_note: Some(Some("Formal.".into())),
            ..Default::default()
        },
        H,
    )
    .unwrap();
    let q = ask(
        s,
        Owner::Topic(f.topic),
        "Can we keep v1?",
        Some(helene),
        Some("2026-12-04"),
    );
    s.answer(q, Some("Yes."), true, H).unwrap();
    ask(s, Owner::Project(f.project), "Who pays?", None, None);
    s.set_report_to(Record::Topic(f.topic), Some(marie), H)
        .unwrap();
    s.set_reported(Record::Topic(f.topic), true, H).unwrap();
    s.set_report_to(Record::Epic(f.epic), Some(helene), H)
        .unwrap();
    let n = s.add_note(f.project, "Tell Paul", H).unwrap();
    s.set_report_to(Record::Note(n), Some(paul), H).unwrap();
    s.set_profile(Some("Alexandre"), Some("Product designer"))
        .unwrap();
    let file = s.export(None).unwrap();
    assert!(file.version >= 3);
    assert_eq!((file.questions.len(), file.contacts.len()), (2, 3));
    assert!(file.profile.is_some());
    let json = file.to_json().unwrap();
    assert!(!json.contains("name_key") && !json.contains("report_contact_id"));

    let mut b = open();
    let report = b.store.import(&file, false).unwrap();
    assert_eq!(report.questions.created, 2);
    assert_eq!(report.contacts.created, 3);
    let again = b.store.export(None).unwrap();
    assert_eq!(again.questions, file.questions);
    assert_eq!(again.contacts, file.contacts);
    assert_eq!(again.topics, file.topics);
    assert_eq!(again.epics, file.epics);
    assert_eq!(again.notes, file.notes);
    assert_eq!(again.profile, file.profile);
    assert!(b.store.unread_questions().unwrap().is_empty());
    assert!(b.store.unread_contacts().unwrap().is_empty());
    let report = b.store.import(&file, false).unwrap();
    assert_eq!(report.questions.created + report.questions.updated, 0);
    assert_eq!(report.contacts.created + report.contacts.updated, 0);
    // The profile goes only into an empty one.
    b.store.set_profile(Some("Someone else"), None).unwrap();
    b.store.import(&file, false).unwrap();
    assert_eq!(
        b.store.profile().unwrap().name.as_deref(),
        Some("Someone else")
    );
    // A project's export: its contacts, its epics', and the global ones it
    // names; never the profile.
    let one = s.export(Some(f.project)).unwrap();
    assert_eq!(one.contacts.len(), 3);
    assert!(one.profile.is_none());
}

#[test]
fn a_contact_named_by_text_rather_than_uuid_is_refused() {
    let mut f = fixture();
    let s = &mut f.t.store;
    ask(s, Owner::Topic(f.topic), "Can we keep v1?", None, None);
    let file = s.export(None).unwrap();
    // Names where UUIDs belong (the free-text build of 0.4) were only ever
    // in plaintext exports: a backup never holds them.
    let mut v: serde_json::Value = serde_json::from_str(&file.to_json().unwrap()).unwrap();
    v["topics"][0]["report_to"] = "Marie Dubois".into();
    let named = ExportFile::from_json(&v.to_string()).unwrap();
    let mut b = open();
    match b.store.import(&named, false) {
        Err(Error::Invalid(m)) => assert!(m.contains("missing record"), "{m}"),
        other => panic!("{other:?}"),
    }
    // Nothing was written.
    assert_eq!(b.store.export(None).unwrap().topics.len(), 0);
}

#[test]
fn a_merge_stamps_what_it_rewrites_and_travels_to_another_database() {
    let mut f = fixture();
    let p = f.project;
    let s = &mut f.t.store;
    let x = contact(s, "Marie", ContactLevel::Project(p));
    let y = contact(s, "Marie Dubois", ContactLevel::Project(p));
    s.set_report_to(Record::Topic(f.topic), Some(x), H).unwrap();
    // Machine B has the same data, and a note of its own naming X.
    let mut b = open();
    b.store.import(&s.export(None).unwrap(), false).unwrap();
    let x_uuid = s.contact(x).unwrap().stamp.uuid;
    let bx = b
        .store
        .contacts_visible(Place {
            project: b.store.projects().unwrap()[0].id,
            epic: None,
        })
        .unwrap()
        .into_iter()
        .find(|c| c.contact.stamp.uuid == x_uuid)
        .unwrap()
        .contact
        .id;
    let bp = b.store.projects().unwrap()[0].id;
    let note = b.store.add_note(bp, "Tell Marie", H).unwrap();
    b.store
        .set_report_to(Record::Note(note), Some(bx), H)
        .unwrap();

    // Merged on A by Claude Code: the topic is stamped with the merge.
    let before = s.topic(f.topic).unwrap().stamp.updated_at;
    s.merge_contacts(x, y, C).unwrap();
    let t = s.topic(f.topic).unwrap();
    assert_eq!(t.stamp.updated_by, C);
    assert!(t.stamp.updated_at >= before);

    // B imports A's export: the topic follows, and B's own note too.
    b.store.import(&s.export(None).unwrap(), false).unwrap();
    let y_uuid = s.contact(y).unwrap().stamp.uuid;
    fn named(s: &Store, r: Option<ContactRef>) -> String {
        s.contact_any(r.unwrap().id).unwrap().stamp.uuid
    }
    let bt = b
        .store
        .topics(&TopicFilter::default(), PageReq::first(10))
        .unwrap()
        .items
        .into_iter()
        .find(|t| t.title == "Export format")
        .unwrap()
        .id;
    assert_eq!(
        named(&b.store, b.store.topic(bt).unwrap().report.to),
        y_uuid
    );
    assert_eq!(
        named(&b.store, b.store.note(note).unwrap().report_to),
        y_uuid
    );
    // A reference the file still makes to X resolves to Y.
    let mut again = s.export(None).unwrap();
    let topic_uuid = s.topic(f.topic).unwrap().stamp.uuid;
    for t in &mut again.topics {
        if t.stamp.uuid == topic_uuid {
            t.report.report_to = Some(x_uuid.clone());
            t.stamp.updated_at += 60_000;
        }
    }
    b.store.import(&again, false).unwrap();
    assert_eq!(
        named(&b.store, b.store.topic(bt).unwrap().report.to),
        y_uuid
    );
    // Restoring the merged contact is refused.
    let e = s.restore(Record::Contact(x), H).unwrap_err();
    assert_eq!(
        e.to_string(),
        format!("{x} was merged into {y}; its uses are there")
    );
}

#[test]
fn a_restored_record_lifts_a_contact_moved_while_it_was_in_the_trash() {
    let mut f = fixture();
    let p = f.project;
    let s = &mut f.t.store;
    let e2 = s.create_epic(p, "Sync", "", H).unwrap();
    let paul = contact(s, "Paul", ContactLevel::Epic(p, f.epic));
    s.set_report_to(Record::Topic(f.topic), Some(paul), H)
        .unwrap();
    s.delete(Record::Topic(f.topic), H).unwrap();
    // Its only live use is gone: the contact may move to another epic.
    s.move_contact(paul, ContactLevel::Epic(p, e2), H).unwrap();
    assert!(s.take_lifts().is_empty());
    s.restore(Record::Topic(f.topic), H).unwrap();
    assert_eq!(s.contact(paul).unwrap().level, ContactLevel::Project(p));
    let lifts = s.take_lifts();
    assert_eq!(lifts.len(), 1);
    assert_eq!(lifts[0].record, Record::Topic(f.topic));
}

#[test]
fn an_import_lifts_references_out_of_scope_and_counts_them() {
    let mut f = fixture();
    let p = f.project;
    let s = &mut f.t.store;
    let e2 = s.create_epic(p, "Sync", "", H).unwrap();
    let paul = contact(s, "Paul", ContactLevel::Epic(p, f.epic));
    s.set_report_to(Record::Topic(f.topic), Some(paul), H)
        .unwrap();
    let mut file = s.export(None).unwrap();
    // The file puts Paul in the other epic: the topic would lose sight.
    let e2_uuid = s.epic(e2).unwrap().stamp.uuid;
    for c in &mut file.contacts {
        c.epic = Some(e2_uuid.clone());
    }
    let mut b = open();
    let report = b.store.import(&file, false).unwrap();
    assert_eq!(report.lifted, 1);
    let c = b
        .store
        .contacts_visible(Place {
            project: b.store.projects().unwrap()[0].id,
            epic: None,
        })
        .unwrap();
    assert_eq!(c.len(), 1);
    assert!(matches!(c[0].contact.level, ContactLevel::Project(_)));
}

#[test]
fn a_claude_import_keeps_local_details_and_stamps_what_it_creates() {
    let mut f = fixture();
    let p = f.project;
    let s = &mut f.t.store;
    let marie = contact(s, "Marie", ContactLevel::Project(p));
    s.update_contact(
        marie,
        ContactPatch {
            job_title: Some(Some("Head of legal".into())),
            ..Default::default()
        },
        H,
    )
    .unwrap();
    let mut file = s.export(None).unwrap();
    for c in &mut file.contacts {
        c.name = "Somebody else".into();
        c.job_title = Some("Intern".into());
        c.stamp.updated_at += 60_000;
    }
    // A new contact in the file, claiming to be the human's.
    let mut extra = file.contacts[0].clone();
    extra.stamp.uuid = "0190c0de-0000-7000-8000-000000000001".into();
    extra.name = "Paul".into();
    extra.created_by = H;
    extra.stamp.updated_by = H;
    file.contacts.push(extra);
    s.import_by(&file, false, C).unwrap();
    let m = s.contact(marie).unwrap();
    assert_eq!(m.name, "Marie");
    assert_eq!(m.job_title.as_deref(), Some("Head of legal"));
    let paul = s
        .contacts_visible(Place {
            project: p,
            epic: None,
        })
        .unwrap()
        .into_iter()
        .find(|c| c.contact.name == "Paul")
        .unwrap()
        .contact;
    assert_eq!(paul.created_by, C);
    assert_eq!(paul.stamp.updated_by, C);
}

#[test]
fn an_unmerge_puts_back_the_contact_its_uses_and_the_other_contact() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let p = f.project;
    let a = contact(s, "Marie", ContactLevel::Epic(p, f.epic));
    let b = contact(s, "Marie Dubois", ContactLevel::Project(p));
    s.set_report_to(Record::Topic(f.topic), Some(a), H).unwrap();
    let q = ask(s, Owner::Topic(f.other), "Sign?", Some(a), None);
    let kept = ask(s, Owner::Topic(f.other), "Other?", Some(b), None);
    let third = contact(s, "Marie", ContactLevel::All);
    s.mark_distinct(a, third).unwrap();
    let uuid_a = s.contact(a).unwrap().stamp.uuid;
    let before = s.contact(b).unwrap();
    let uses: Vec<Record> = s
        .contact_uses(a)
        .unwrap()
        .into_iter()
        .map(|u| u.record)
        .collect();
    let trace = s.merge_contacts(a, b, H).unwrap();
    assert_eq!(trace.events.len(), 1, "the report event named a");
    assert_eq!(trace.distinct.len(), 1);
    s.unmerge_contacts(a, &before, &uses, &trace, H).unwrap();
    // The event names `a` again, and "Keep both" holds again.
    let events: Vec<Option<String>> = s
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
        .filter_map(|e| e.event)
        .filter(|e| e.kind == EventKind::Report)
        .map(|e| e.new)
        .collect();
    assert_eq!(events, [Some(uuid_a)]);
    assert!(s.is_distinct(a, third).unwrap());
    let back = s.contact(a).unwrap();
    assert_eq!(back.merged_into, None);
    assert_eq!(back.level, ContactLevel::Epic(p, f.epic));
    assert_eq!(s.contact(b).unwrap().level, ContactLevel::Project(p));
    assert_eq!(s.topic(f.topic).unwrap().report.to.map(|c| c.id), Some(a));
    assert_eq!(s.question(q).unwrap().asked_to.map(|c| c.id), Some(a));
    // What named `b` before the merge still names it.
    assert_eq!(s.question(kept).unwrap().asked_to.map(|c| c.id), Some(b));
}

#[test]
fn a_question_is_put_back_exactly_as_it_was() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let marie = contact(s, "Marie", ContactLevel::Project(f.project));
    let q = ask(
        s,
        Owner::Topic(f.topic),
        "Keep v1?",
        Some(marie),
        Some("2026-12-04"),
    );
    ask(s, Owner::Topic(f.topic), "Second?", None, None);
    let before = s.question(q).unwrap();
    let entry = s
        .answer(q, Some("Yes, until March."), true, H)
        .unwrap()
        .expect("a decision entry");
    s.put_question(&before, H).unwrap();
    let now = s.question(q).unwrap();
    assert_eq!(now.status, QuestionStatus::Open);
    assert_eq!(now.answer, None);
    assert_eq!(now.decision_entry, None);
    assert_eq!(now.position, before.position);
    assert_eq!(now.asked_to.map(|c| c.id), Some(marie));
    // The entry is the caller's to delete in the same step.
    assert!(s.entry(entry.id).is_ok());
    // A move is put back too.
    s.move_question(q, Owner::Epic(f.epic), H).unwrap();
    s.put_question(&before, H).unwrap();
    assert_eq!(s.question(q).unwrap().owner, Owner::Topic(f.topic));
}

#[test]
fn every_contact_is_listed_with_its_open_items_across_projects() {
    let mut f = fixture();
    let s = &mut f.t.store;
    let atlas = s.create_project("Atlas", "", H).unwrap();
    let all = contact(s, "Alexandre's accountant", ContactLevel::All);
    let here = contact(s, "Legal team", ContactLevel::Project(f.project));
    ask(s, Owner::Topic(f.topic), "Fees?", Some(all), None);
    ask(s, Owner::Project(atlas), "Tax?", Some(all), None);
    s.set_report_to(Record::Topic(f.other), Some(here), H)
        .unwrap();
    let list = s.contacts_all().unwrap();
    let open = |id| list.iter().find(|c| c.contact.id == id).unwrap().open;
    assert_eq!(open(all), 2);
    assert_eq!(open(here), 1);
    s.delete(Record::Contact(here), H).unwrap();
    assert_eq!(s.contacts_all().unwrap().len(), 1);
    assert_eq!(s.contact_count().unwrap(), 1);
    // A contact in the trash is still named by its UUID.
    let uuid = s.contact_any(here).unwrap().stamp.uuid;
    let names = s.contact_names(&[uuid.clone(), "nope".into()]).unwrap();
    assert_eq!(names.get(&uuid).map(String::as_str), Some("Legal team"));
    assert_eq!(names.len(), 1);
}

#[test]
fn late_answers_count_as_an_attention_line() {
    let mut f = fixture();
    let s = &mut f.t.store;
    ask(s, Owner::Topic(f.topic), "Late?", None, Some("2026-11-01"));
    let o = s
        .overview(
            Scope::Project(f.project),
            d("2026-11-22"),
            crate::date::Zone::UTC,
        )
        .unwrap();
    assert_eq!(o.answers_late, 1);
    assert_eq!(o.attention_lines(), 1);
}
