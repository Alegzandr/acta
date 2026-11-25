//! Commit mode per page: two pages on one folder, inheritance, sessions
//! started outside Acta, and a session claiming another page's mode.

use super::page::*;
use super::{CommitMode, CommitSettings, LOCAL_SETTINGS, set_commit, stored_form};
use crate::git_guard::{self, Alias, Context, Place, Verdict};
use crate::model::{Author, EpicId, FolderId, Owner, ProjectId, Status, TopicId};
use crate::store::NewTopic;
use crate::store::test_support::{TestStore, open};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const H: Author = Author::Human;
const T1: &str = "0f2e4a10-1c2d-4f00-8a00-0123456789ab";
const T2: &str = "0f2e4a10-1c2d-4f00-8a00-0123456789ac";

fn git(dir: &Path, args: &[&str]) {
    let out = crate::git_cmd::command(dir).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A project with a git folder (on branch `feature`), an epic, and three
/// topics: two in the epic, one directly in the project.
struct World {
    t: TestStore,
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    folder: FolderId,
    project: ProjectId,
    epic: EpicId,
    a: TopicId,
    b: TopicId,
    c: TopicId,
}

fn world() -> World {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("acta");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Alexandre",
            "-c",
            "user.email=alexandre@example.com",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "First",
        ],
    );
    git(&repo, &["switch", "-q", "-c", "feature"]);
    let s = &mut t.store;
    let project = s.create_project("Acta", "", H).unwrap();
    let folder = s.add_folder(Owner::Project(project), &repo, H).unwrap();
    let epic = s.create_epic(project, "Terminal", "", H).unwrap();
    let topic = |s: &mut crate::Store, epic: Option<EpicId>, title: &str| {
        s.create_topic(
            NewTopic {
                project,
                epic,
                title: title.into(),
                description: String::new(),
                status: Status::Planned,
            },
            H,
        )
        .unwrap()
    };
    let a = topic(s, Some(epic), "Embed the terminal");
    let b = topic(s, Some(epic), "Release notes");
    let c = topic(s, None, "Website");
    World {
        t,
        _tmp: tmp,
        repo,
        folder,
        project,
        epic,
        a,
        b,
        c,
    }
}

fn pc(mode: CommitMode) -> Option<PageCommit> {
    Some(PageCommit {
        mode,
        as_user: None,
        on_default: false,
    })
}

fn mode_of(w: &World, owner: Owner) -> (CommitMode, Source) {
    let e = effective(&w.t.store, owner, &stored_form(&w.repo)).unwrap();
    (e.settings.mode, e.source)
}

fn view(w: &World, session: &SessionPage) -> GuardView {
    guard_view(&w.t.store, &stored_form(&w.repo), session)
        .unwrap()
        .unwrap()
}

/// The guard's answer for `command` run in the repository, for a session.
fn judge(w: &World, session: &SessionPage, command: &str) -> Option<Verdict> {
    let place = |dir: &Path| {
        let stored = stored_form(dir);
        match guard_view(&w.t.store, &stored, session) {
            Err(_) => Place::Unreadable,
            Ok(None) => Place::Unlinked,
            Ok(Some(v)) => Place::Folder {
                name: v.folder.name,
                settings: v.folder.settings,
                git: crate::agent::git::inspect(dir).map(Box::new),
                permitted: v.permitted,
                granted: v.granted,
            },
        }
    };
    let alias = |_: &Path, _: &str| Alias::None;
    git_guard::judge(
        command,
        &Context {
            cwd: &w.repo,
            session: &w.repo,
            place: &place,
            alias: &alias,
        },
    )
}

fn settings_file(path: &Option<PathBuf>) -> Value {
    serde_json::from_str(&fs::read_to_string(path.as_ref().unwrap()).unwrap()).unwrap()
}

/// The user's request: one folder, Manual from one topic, commit and push
/// from another. Each session Acta starts gets its page's rules; the
/// folder's own file keeps only the folder's setting.
#[test]
fn two_pages_on_one_folder_with_different_modes() {
    let mut w = world();
    let (a, b) = (Owner::Topic(w.a), Owner::Topic(w.b));
    // The folder commits automatically; topic A says Manual, B push.
    set_commit(&mut w.t.store, w.folder, CommitMode::Commit, None, false, H).unwrap();
    set_page_commit(&mut w.t.store, a, w.folder, pc(CommitMode::Manual), H).unwrap();
    set_page_commit(&mut w.t.store, b, w.folder, pc(CommitMode::Push), H).unwrap();
    assert_eq!(mode_of(&w, a), (CommitMode::Manual, Source::Page(a)));
    assert_eq!(mode_of(&w, b), (CommitMode::Push, Source::Page(b)));

    // The folder file holds the folder's setting only: no push.
    let local: Value =
        serde_json::from_str(&fs::read_to_string(w.repo.join(LOCAL_SETTINGS)).unwrap()).unwrap();
    let allow = local["permissions"]["allow"].to_string();
    assert!(allow.contains("git commit") && !allow.contains("git push"));

    // A session for B is allowed to push; for A, every commit is asked.
    let fb = begin_session(&mut w.t.store, T2, b, &w.repo, None).unwrap();
    let doc = settings_file(&fb);
    assert!(
        doc["permissions"]["allow"]
            .as_array()
            .unwrap()
            .contains(&"Bash(git push)".into())
    );
    assert_eq!(doc["env"]["GIT_TERMINAL_PROMPT"], "0");
    assert!(doc["permissions"].get("ask").is_none());
    let fa = begin_session(&mut w.t.store, T1, a, &w.repo, None).unwrap();
    let doc = settings_file(&fa);
    assert!(doc["permissions"].get("allow").is_none());
    let ask = doc["permissions"]["ask"].as_array().unwrap();
    assert!(ask.contains(&"Bash(git commit *)".into()) && ask.contains(&"Bash(git add *)".into()));
    // Only rules, env and attribution: nothing of the user's.
    for d in [settings_file(&fa), settings_file(&fb)] {
        for k in d.as_object().unwrap().keys() {
            assert!(
                ["permissions", "env", "attribution"].contains(&k.as_str()),
                "{k}"
            );
        }
    }

    // The guard: A's commit goes to the user, B's push goes through, a
    // forced push of B's is denied.
    let sa = session_page(&w.t.store, Some(T1)).unwrap();
    let sb = session_page(&w.t.store, Some(T2)).unwrap();
    assert_eq!(view(&w, &sa).permitted, CommitMode::Manual);
    assert_eq!(view(&w, &sa).granted, CommitMode::Commit);
    assert!(matches!(
        judge(&w, &sa, "git commit -m x"),
        Some(Verdict::Ask(_))
    ));
    assert_eq!(judge(&w, &sb, "git commit -m x && git push"), None);
    assert!(matches!(
        judge(&w, &sb, "git push --force"),
        Some(Verdict::Deny(_))
    ));
    // Ask texts stay short.
    if let Some(Verdict::Ask(r)) = judge(&w, &sa, "git commit -m x") {
        assert!(r.chars().count() <= 200);
    }

    // The session ends: its record and file go.
    end_session(&mut w.t.store, T2).unwrap();
    assert!(!fb.unwrap().exists());
    assert_eq!(
        session_page(&w.t.store, Some(T2)).unwrap(),
        SessionPage::Unknown
    );
    assert_eq!(
        end_sessions_of_gone_apps(&mut w.t.store, &|_| false).unwrap(),
        1
    );
    assert!(!fa.unwrap().exists());
}

/// A session started outside Acta has no launch token: the folder's own
/// setting applies, whatever the pages chose.
#[test]
fn a_session_outside_acta_uses_the_folder_setting() {
    let mut w = world();
    let a = Owner::Topic(w.a);
    set_commit(&mut w.t.store, w.folder, CommitMode::Commit, None, false, H).unwrap();
    set_page_commit(&mut w.t.store, a, w.folder, pc(CommitMode::Push), H).unwrap();
    let outside = session_page(&w.t.store, None).unwrap();
    assert_eq!(outside, SessionPage::Outside);
    let v = view(&w, &outside);
    assert_eq!(
        (v.folder.settings.mode, v.permitted, v.granted),
        (CommitMode::Commit, CommitMode::Commit, CommitMode::Commit)
    );
    // Push is not the folder's: left to Claude Code, which asks.
    assert_eq!(judge(&w, &outside, "git push"), None);
    assert_eq!(judge(&w, &outside, "git commit -m x"), None);
    assert!(matches!(
        judge(&w, &outside, "git commit -n -m x"),
        Some(Verdict::Deny(_))
    ));
    // Manual folder, page Push: still nothing from the guard outside.
    set_commit(&mut w.t.store, w.folder, CommitMode::Manual, None, false, H).unwrap();
    assert_eq!(view(&w, &outside).permitted, CommitMode::Manual);
    assert_eq!(judge(&w, &outside, "git commit -n -m x"), None);
    // The briefing of an outside session says the folder's setting.
    let b = chosen_for_session(&w.t.store, &stored_form(&w.repo), &outside)
        .unwrap()
        .unwrap();
    assert_eq!(b.settings.mode, CommitMode::Manual);
}

/// A choice on the project or an epic reaches every page below it that
/// has none of its own; the nearest wins; "Use folder setting" inherits.
#[test]
fn page_choices_are_inherited_downward() {
    let mut w = world();
    let (p, e) = (Owner::Project(w.project), Owner::Epic(w.epic));
    let (a, b, c) = (Owner::Topic(w.a), Owner::Topic(w.b), Owner::Topic(w.c));
    for o in [p, e, a, c] {
        assert_eq!(mode_of(&w, o), (CommitMode::Manual, Source::Folder));
    }
    set_page_commit(&mut w.t.store, e, w.folder, pc(CommitMode::Push), H).unwrap();
    assert_eq!(mode_of(&w, a), (CommitMode::Push, Source::Page(e)));
    assert_eq!(mode_of(&w, b), (CommitMode::Push, Source::Page(e)));
    assert_eq!(mode_of(&w, c), (CommitMode::Manual, Source::Folder));
    assert_eq!(mode_of(&w, p), (CommitMode::Manual, Source::Folder));
    // The project owns the folder record: its setting is the folder's,
    // so it makes no page choice there.
    assert!(set_page_commit(&mut w.t.store, p, w.folder, pc(CommitMode::Commit), H).is_err());
    assert_eq!(mode_of(&w, c), (CommitMode::Manual, Source::Folder));
    assert_eq!(
        mode_of(&w, a),
        (CommitMode::Push, Source::Page(e)),
        "nearest"
    );
    set_page_commit(&mut w.t.store, a, w.folder, pc(CommitMode::Manual), H).unwrap();
    assert_eq!(mode_of(&w, a), (CommitMode::Manual, Source::Page(a)));
    assert_eq!(mode_of(&w, b), (CommitMode::Push, Source::Page(e)));
    // What A would use without its own choice.
    let i = inherited(&w.t.store, a, &stored_form(&w.repo)).unwrap();
    assert_eq!(
        (i.settings.mode, i.source),
        (CommitMode::Push, Source::Page(e))
    );
    set_page_commit(&mut w.t.store, a, w.folder, None, H).unwrap();
    assert_eq!(mode_of(&w, a), (CommitMode::Push, Source::Page(e)));
    // The two options follow the choice; "in my name" defaults as on a
    // folder.
    set_page_commit(
        &mut w.t.store,
        e,
        w.folder,
        Some(PageCommit {
            mode: CommitMode::Commit,
            as_user: Some(true),
            on_default: true,
        }),
        H,
    )
    .unwrap();
    let s = effective(&w.t.store, a, &stored_form(&w.repo))
        .unwrap()
        .settings;
    assert_eq!(
        s,
        CommitSettings {
            mode: CommitMode::Commit,
            as_user: true,
            on_default: true
        }
    );
    // Claude Code cannot choose; a folder the page does not see is refused.
    let err = set_page_commit(
        &mut w.t.store,
        a,
        w.folder,
        pc(CommitMode::Push),
        Author::Claude,
    );
    assert!(err.is_err());
    let other = w.t.store.create_project("Other", "", H).unwrap();
    assert!(
        set_page_commit(
            &mut w.t.store,
            Owner::Project(other),
            w.folder,
            pc(CommitMode::Push),
            H
        )
        .is_err()
    );
}

/// A session cannot claim another page's laxer mode: the page comes from
/// the launch record of its own token, never from what it says it works
/// on; a token Acta does not know gets the strictest answer.
#[test]
fn a_session_cannot_claim_another_pages_mode() {
    let mut w = world();
    let (a, b) = (Owner::Topic(w.a), Owner::Topic(w.b));
    set_page_commit(&mut w.t.store, b, w.folder, pc(CommitMode::Push), H).unwrap();
    begin_session(&mut w.t.store, T1, a, &w.repo, None).unwrap();
    begin_session(&mut w.t.store, T2, b, &w.repo, None).unwrap();

    // A's token is A's page, whatever else the session says (ACTA_TOPIC
    // and presence are not read): Manual, nothing granted.
    let sa = session_page(&w.t.store, Some(T1)).unwrap();
    match &sa {
        SessionPage::Launched(l) => assert_eq!(l.page, a),
        other => panic!("{other:?}"),
    }
    assert_eq!(view(&w, &sa).permitted, CommitMode::Manual);
    // The user switched the folder file to push behind A's back? A's
    // session still has the user confirm.
    set_commit(&mut w.t.store, w.folder, CommitMode::Push, None, false, H).unwrap();
    set_page_commit(&mut w.t.store, a, w.folder, pc(CommitMode::Manual), H).unwrap();
    assert!(matches!(judge(&w, &sa, "git push"), Some(Verdict::Ask(_))));
    assert!(matches!(
        judge(&w, &sa, "git commit -m x"),
        Some(Verdict::Ask(_))
    ));

    // A forged or stale token: not B's mode, but the strictest.
    for forged in ["0f2e4a10-1c2d-4f00-8a00-0000000000ff", "ACTA_TOPIC=T-2"] {
        let s = session_page(&w.t.store, Some(forged)).unwrap();
        assert_eq!(s, SessionPage::Unknown);
        let v = view(&w, &s);
        assert_eq!(v.permitted, CommitMode::Manual);
        assert!(!v.folder.settings.on_default);
        assert!(matches!(judge(&w, &s, "git push"), Some(Verdict::Ask(_))));
        assert!(matches!(
            judge(&w, &s, "git commit -n -m x"),
            Some(Verdict::Deny(_))
        ));
        let told = chosen_for_session(&w.t.store, &stored_form(&w.repo), &s)
            .unwrap()
            .unwrap();
        assert_eq!(told.settings.mode, CommitMode::Manual);
    }
    // A bad token is never recorded.
    assert!(begin_session(&mut w.t.store, "x; rm", b, &w.repo, None).is_err());
    // A page in the trash gives no mode either.
    w.t.store
        .delete(crate::model::Record::Topic(w.b), H)
        .unwrap();
    let sb = session_page(&w.t.store, Some(T2)).unwrap();
    assert_eq!(view(&w, &sb).permitted, CommitMode::Manual);
}

/// The per-session settings: rules of the wanted mode, and ask rules only
/// for what the folder's file allows beyond it.
#[test]
fn session_settings_ask_only_beyond_the_folder_file() {
    let s = |mode| CommitSettings {
        mode,
        as_user: false,
        on_default: false,
    };
    use CommitMode::*;
    assert_eq!(session_settings(s(Manual), None), serde_json::json!({}));
    assert_eq!(
        session_settings(s(Manual), Some(s(Manual))),
        serde_json::json!({})
    );
    let doc = session_settings(s(Commit), Some(s(Push)));
    assert_eq!(
        doc["permissions"]["ask"],
        serde_json::json!(["Bash(git push)"])
    );
    let doc = session_settings(
        CommitSettings {
            as_user: true,
            ..s(Push)
        },
        None,
    );
    assert!(doc["permissions"].get("ask").is_none());
    assert_eq!(doc["attribution"]["commit"], "");
}

/// A Push session whose folder is removed from its page keeps no right of
/// its own: each push is put to the user, the default branch is refused,
/// and its settings file is made strict.
#[test]
fn a_detached_folder_keeps_no_rights() {
    let mut w = world();
    let b = Owner::Topic(w.b);
    set_page_commit(&mut w.t.store, b, w.folder, pc(CommitMode::Push), H).unwrap();
    let file = begin_session(&mut w.t.store, T2, b, &w.repo, None)
        .unwrap()
        .unwrap();
    let sb = session_page(&w.t.store, Some(T2)).unwrap();
    assert_eq!(judge(&w, &sb, "git push"), None);
    w.t.store
        .delete(crate::model::Record::Folder(w.folder), H)
        .unwrap();
    let v = view(&w, &sb);
    assert_eq!(
        (v.permitted, v.granted),
        (CommitMode::Manual, CommitMode::Push)
    );
    assert!(matches!(judge(&w, &sb, "git push"), Some(Verdict::Ask(_))));
    assert!(matches!(
        judge(&w, &sb, "git push --force"),
        Some(Verdict::Deny(_))
    ));
    git(&w.repo, &["switch", "-q", "main"]);
    assert!(matches!(
        judge(&w, &sb, "git commit -m x"),
        Some(Verdict::Deny(_))
    ));
    // The file follows.
    assert!(session_rules_applied(&w.t.store, T2).unwrap());
    assert_eq!(restrict_detached_sessions(&mut w.t.store).unwrap(), 1);
    assert_eq!(restrict_detached_sessions(&mut w.t.store).unwrap(), 0);
    assert!(!session_rules_applied(&w.t.store, T2).unwrap());
    let doc: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert!(doc["permissions"].get("allow").is_none());
    assert!(
        doc["permissions"]["ask"]
            .to_string()
            .contains("Bash(git push)")
    );
    // A session that kept its folder is left alone.
    begin_session(&mut w.t.store, T1, Owner::Topic(w.c), &w.repo, None).unwrap();
    assert_eq!(
        restrict_detached_sessions(&mut w.t.store).unwrap(),
        1,
        "c no longer sees it either"
    );
}

/// The path attached at the epic (through the project) and at the topic:
/// a choice above never overrides the topic's own record.
#[test]
fn a_pages_own_record_ends_the_walk() {
    let mut w = world();
    let (e, a, b) = (Owner::Epic(w.epic), Owner::Topic(w.a), Owner::Topic(w.b));
    let own = w.t.store.add_folder(a, &w.repo, H).unwrap();
    set_commit(&mut w.t.store, own, CommitMode::Manual, None, false, H).unwrap();
    set_page_commit(&mut w.t.store, e, w.folder, pc(CommitMode::Push), H).unwrap();
    assert_eq!(mode_of(&w, a), (CommitMode::Manual, Source::Folder));
    assert_eq!(mode_of(&w, b), (CommitMode::Push, Source::Page(e)));
    // The topic's own record set to commit: that is what applies.
    set_commit(&mut w.t.store, own, CommitMode::Commit, None, false, H).unwrap();
    assert_eq!(mode_of(&w, a), (CommitMode::Commit, Source::Folder));
    let i = inherited(&w.t.store, a, &stored_form(&w.repo)).unwrap();
    assert_eq!(i.source, Source::Folder);
}

/// Only the sessions of an app that is gone are forgotten: another running
/// Acta keeps its own.
#[test]
fn sessions_of_a_running_app_are_kept() {
    let mut w = world();
    let a = Owner::Topic(w.a);
    let me = AppInstance {
        pid: 7,
        started: 1_700_000_000,
    };
    let gone = AppInstance {
        pid: 7,
        started: 1_600_000_000,
    };
    let mine = begin_session(&mut w.t.store, T1, a, &w.repo, Some(me)).unwrap();
    set_page_commit(&mut w.t.store, a, w.folder, pc(CommitMode::Push), H).unwrap();
    let theirs = begin_session(&mut w.t.store, T2, a, &w.repo, Some(gone))
        .unwrap()
        .unwrap();
    let alive = |x: AppInstance| x == me;
    assert_eq!(
        end_sessions_of_gone_apps(&mut w.t.store, &alive).unwrap(),
        1
    );
    assert!(matches!(
        session_page(&w.t.store, Some(T1)).unwrap(),
        SessionPage::Launched(_)
    ));
    assert_eq!(
        session_page(&w.t.store, Some(T2)).unwrap(),
        SessionPage::Unknown
    );
    assert!(!theirs.exists());
    assert!(
        mine.is_none(),
        "a Manual page on a Manual folder needs no file"
    );
}

/// Choices go with a folder or page purged for good, not with the trash.
#[test]
fn choices_go_with_a_purge_not_the_trash() {
    use crate::model::Record;
    let mut w = world();
    let (a, c) = (Owner::Topic(w.a), Owner::Topic(w.c));
    set_page_commit(&mut w.t.store, a, w.folder, pc(CommitMode::Push), H).unwrap();
    set_page_commit(&mut w.t.store, c, w.folder, pc(CommitMode::Commit), H).unwrap();
    let count = |s: &crate::Store| s.local_values(PAGE_PREFIX).unwrap().len();
    w.t.store.delete(Record::Topic(w.a), H).unwrap();
    assert_eq!(count(&w.t.store), 2, "trashed: kept for a restore");
    w.t.store.trash_purge(&[Record::Topic(w.a)]).unwrap();
    assert_eq!(count(&w.t.store), 1);
    w.t.store.delete(Record::Folder(w.folder), H).unwrap();
    assert_eq!(count(&w.t.store), 1);
    w.t.store.trash_purge(&[Record::Folder(w.folder)]).unwrap();
    assert_eq!(count(&w.t.store), 0);
}
