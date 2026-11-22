//! The settings writer on temporary folders and repositories, with an
//! in-memory key store. `~/.claude` is never touched.

use super::*;
use crate::store::test_support::open;

fn git(dir: &Path, args: &[&str]) {
    let out = git_cmd::command(dir).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn repo(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
}

fn read(folder: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(folder.join(LOCAL_SETTINGS)).unwrap()).unwrap()
}

fn mode(mode: CommitMode, as_user: bool) -> Wanted {
    Wanted {
        auto_memory_off: None,
        commit: Some(CommitSettings {
            mode,
            as_user,
            on_default: false,
        }),
    }
}

fn commit_json() -> Value {
    json!({"permissions": {
        "allow": ["Bash(git add *)", "Bash(git commit *)"],
        "deny": [
            "Bash(git * --no-verify*)",
            "Bash(git push *--force*)",
            "Bash(git push -f*)",
            "Bash(git push * -f*)"
        ]
    }})
}

#[test]
fn each_mode_writes_exactly_the_adr_json() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path().join("acta");
    repo(&r);
    let s = &mut t.store;

    let o = apply(s, &r, mode(CommitMode::Manual, true)).unwrap();
    assert!(!o.changed());
    assert!(!r.join(".claude").exists(), "Manual writes nothing");
    assert!(s.local_values(LEDGER_PREFIX).unwrap().is_empty());

    apply(s, &r, mode(CommitMode::Commit, false)).unwrap();
    assert_eq!(read(&r), commit_json());

    apply(s, &r, mode(CommitMode::Push, false)).unwrap();
    let mut push = commit_json();
    push["permissions"]["allow"]
        .as_array_mut()
        .unwrap()
        .push(json!("Bash(git push)"));
    push["env"] = json!({"GIT_TERMINAL_PROMPT": "0", "GCM_INTERACTIVE": "never"});
    assert_eq!(read(&r), push);

    apply(s, &r, mode(CommitMode::Push, true)).unwrap();
    push["attribution"] = json!({"commit": "", "pr": "", "sessionUrl": false});
    assert_eq!(read(&r), push);
    // Key order as written: permissions, env, attribution.
    let text = fs::read_to_string(r.join(LOCAL_SETTINGS)).unwrap();
    assert!(
        text.find("permissions") < text.find("\"env\"")
            && text.find("\"env\"") < text.find("attribution")
    );

    let mut commit_as_user = commit_json();
    commit_as_user["attribution"] = push["attribution"].clone();
    apply(s, &r, mode(CommitMode::Commit, true)).unwrap();
    assert_eq!(read(&r), commit_as_user);

    // The file Acta created is excluded locally, once.
    apply(s, &r, mode(CommitMode::Manual, false)).unwrap();
    assert!(
        !r.join(".claude").exists(),
        "file and directory Acta made are gone"
    );
    apply(s, &r, mode(CommitMode::Commit, false)).unwrap();
    let exclude = fs::read_to_string(r.join(".git").join("info").join("exclude")).unwrap();
    assert_eq!(exclude.matches("/.claude/settings.local.json").count(), 1);
    assert!(!git_cmd::is_tracked(&r, LOCAL_SETTINGS).unwrap());
    let status = git_cmd::status_summary(&r).unwrap();
    assert_eq!(
        status.changed, 0,
        "the created file does not show as a change"
    );
}

#[test]
fn user_keys_and_rules_survive_every_transition() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path().join("r");
    repo(&r);
    fs::create_dir_all(r.join(".claude")).unwrap();
    let user = json!({
        "model": "haiku",
        "attribution": {"commit": "Reviewed by Alexandre"},
        "env": {"GIT_TERMINAL_PROMPT": "1", "FOO": "bar"},
        "permissions": {"allow": ["Bash(npm test)", "Bash(git add *)"], "ask": ["Bash(rm *)"]}
    });
    let original = serde_json::to_string_pretty(&user).unwrap() + "\n";
    fs::write(r.join(LOCAL_SETTINGS), &original).unwrap();
    let s = &mut t.store;

    let o = apply(s, &r, mode(CommitMode::Commit, true)).unwrap();
    assert_eq!(o.user_kept, vec!["attribution"]);
    assert!(
        !o.added.contains(&"allow:Bash(git add *)".to_owned()),
        "the user's rule"
    );
    let v = read(&r);
    assert_eq!(v["attribution"], user["attribution"]);
    assert_eq!(
        v["permissions"]["allow"],
        json!(["Bash(npm test)", "Bash(git add *)", "Bash(git commit *)"])
    );
    let o = apply(s, &r, mode(CommitMode::Push, true)).unwrap();
    assert!(o.user_kept.contains(&"env.GIT_TERMINAL_PROMPT".to_owned()));
    let v = read(&r);
    assert_eq!(
        v["env"],
        json!({"GIT_TERMINAL_PROMPT": "1", "FOO": "bar", "GCM_INTERACTIVE": "never"})
    );
    apply(s, &r, mode(CommitMode::Commit, false)).unwrap();
    assert_eq!(read(&r)["env"], user["env"]);
    let o = apply(s, &r, mode(CommitMode::Manual, false)).unwrap();
    assert!(o.removed.contains(&"allow:Bash(git commit *)".to_owned()));
    // Back to exactly the user's file, in the user's key order.
    assert_eq!(
        fs::read_to_string(r.join(LOCAL_SETTINGS)).unwrap(),
        original
    );
    assert!(s.local_values(LEDGER_PREFIX).unwrap().is_empty());
}

#[test]
fn a_value_the_user_changed_is_theirs() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path();
    apply(s, f, mode(CommitMode::Push, true)).unwrap();
    let mut v = read(f);
    v["attribution"] = json!({"commit": "mine"});
    v["env"]["GCM_INTERACTIVE"] = json!("always");
    v["theme"] = json!("dark");
    fs::write(f.join(LOCAL_SETTINGS), v.to_string()).unwrap();
    apply(s, f, mode(CommitMode::Manual, false)).unwrap();
    assert_eq!(
        read(f),
        json!({"attribution": {"commit": "mine"}, "env": {"GCM_INTERACTIVE": "always"}, "theme": "dark"})
    );
}

#[test]
fn removal_deletes_only_a_file_acta_created_and_left_empty() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let (a, b, c) = (
        tmp.path().join("a"),
        tmp.path().join("b"),
        tmp.path().join("c"),
    );
    for d in [&a, &b, &c] {
        fs::create_dir_all(d).unwrap();
    }
    let both = Wanted {
        auto_memory_off: Some(true),
        commit: Some(CommitSettings {
            mode: CommitMode::Commit,
            as_user: true,
            on_default: false,
        }),
    };
    // a: created by Acta, left alone.
    apply(s, &a, both).unwrap();
    // b: created by Acta, the user adds a key.
    apply(s, &b, both).unwrap();
    let mut v = read(&b);
    v["model"] = json!("haiku");
    fs::write(b.join(LOCAL_SETTINGS), v.to_string()).unwrap();
    // c: the user's empty file.
    fs::create_dir_all(c.join(".claude")).unwrap();
    fs::write(c.join(LOCAL_SETTINGS), "{}").unwrap();
    apply(s, &c, both).unwrap();

    assert_eq!(remove_all(s).unwrap(), (3, 0));
    assert!(!a.join(".claude").exists());
    assert_eq!(read(&b), json!({"model": "haiku"}));
    assert_eq!(read(&c), json!({}));
    assert!(s.local_values(LEDGER_PREFIX).unwrap().is_empty());
    assert_eq!(remove(s, &a).unwrap(), Restore::NotNeeded);
}

#[test]
fn a_tracked_file_is_never_written() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path().join("r");
    repo(&r);
    fs::create_dir_all(r.join(".claude")).unwrap();
    fs::write(r.join(LOCAL_SETTINGS), "{\"model\": \"haiku\"}\n").unwrap();
    git(&r, &["add", "-f", LOCAL_SETTINGS]);
    let s = &mut t.store;
    let o = apply(s, &r, mode(CommitMode::Push, true)).unwrap();
    assert!(o.tracked && !o.changed());
    assert_eq!(
        fs::read_to_string(r.join(LOCAL_SETTINGS)).unwrap(),
        "{\"model\": \"haiku\"}\n"
    );
    // What was applied is still recorded (nothing was written).
    assert_eq!(
        read_ledger(s, &r).unwrap().unwrap().commit.unwrap().mode,
        CommitMode::Push
    );
    apply(s, &r, mode(CommitMode::Manual, false)).unwrap();
    assert!(s.local_values(LEDGER_PREFIX).unwrap().is_empty());
}

#[test]
fn writing_twice_changes_nothing() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path();
    let w = Wanted {
        auto_memory_off: Some(true),
        commit: Some(CommitSettings {
            mode: CommitMode::Push,
            as_user: true,
            on_default: true,
        }),
    };
    assert!(apply(s, f, w).unwrap().changed());
    let text = fs::read_to_string(f.join(LOCAL_SETTINGS)).unwrap();
    let ledger = s.local_values(LEDGER_PREFIX).unwrap();
    let o = apply(s, f, w).unwrap();
    assert_eq!(o, Outcome::default());
    assert_eq!(fs::read_to_string(f.join(LOCAL_SETTINGS)).unwrap(), text);
    assert_eq!(s.local_values(LEDGER_PREFIX).unwrap(), ledger);
    // Auto memory alone leaves the commit entries; commit alone leaves it.
    apply(
        s,
        f,
        Wanted {
            auto_memory_off: Some(false),
            commit: None,
        },
    )
    .unwrap();
    assert!(read(f).get("autoMemoryEnabled").is_none());
    assert!(read(f).get("attribution").is_some());
}

#[test]
fn malformed_files_are_refused_untouched() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path();
    fs::create_dir_all(f.join(".claude")).unwrap();
    for text in [
        "[1]",
        "{ not json",
        "{\"permissions\": []}",
        "{\"permissions\": {\"allow\": \"x\"}}",
        "{\"env\": 1}",
    ] {
        fs::write(f.join(LOCAL_SETTINGS), text).unwrap();
        assert!(
            apply(s, f, mode(CommitMode::Push, false)).is_err(),
            "{text}"
        );
        assert_eq!(fs::read_to_string(f.join(LOCAL_SETTINGS)).unwrap(), text);
    }
    assert!(s.local_values(LEDGER_PREFIX).unwrap().is_empty());
}

#[test]
fn auto_memory_markers_move_into_the_ledger() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path().join("old");
    fs::create_dir_all(f.join(".claude")).unwrap();
    fs::write(
        f.join(LOCAL_SETTINGS),
        "{\n  \"autoMemoryEnabled\": false\n}\n",
    )
    .unwrap();
    s.set_local_value(
        &format!("{AUTO_MEMORY_PREFIX}{}", f.to_string_lossy()),
        Some("created"),
    )
    .unwrap();
    assert_eq!(migrate_auto_memory_markers(s).unwrap(), 1);
    assert!(s.local_values(AUTO_MEMORY_PREFIX).unwrap().is_empty());
    assert_eq!(migrate_auto_memory_markers(s).unwrap(), 0);
    apply(s, &f, mode(CommitMode::Commit, false)).unwrap();
    assert_eq!(read(&f)["autoMemoryEnabled"], json!(false));
    assert_eq!(remove(s, &f).unwrap(), Restore::Restored);
    assert!(!f.join(LOCAL_SETTINGS).exists(), "Acta created it: gone");
}

/// A project with one folder attached at `path`, returned with its id.
fn attach(s: &mut Store, path: &Path) -> crate::model::FolderId {
    let p = match s.projects().unwrap().first() {
        Some(p) => p.id,
        None => s.create_project("Acta", "", Author::Human).unwrap(),
    };
    s.add_folder(crate::model::Owner::Project(p), path, Author::Human)
        .unwrap()
}

fn stored(p: &Path) -> String {
    folder_path::canonical(p).unwrap()
}

#[test]
fn the_columns_decide_and_the_deepest_folder_wins() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let outer = tmp.path().join("acta");
    let inner = outer.join("crates");
    fs::create_dir_all(&inner).unwrap();
    let o = attach(s, &outer);
    attach(s, &inner);
    set_commit(s, o, CommitMode::Push, None, false, Author::Human).unwrap();
    let at = |s: &Store, p: &Path| {
        commit_settings_for(s, &stored(p))
            .unwrap()
            .map(|f| (f.settings.mode, f.name))
    };
    assert_eq!(at(s, &outer), Some((CommitMode::Push, "acta".into())));
    assert_eq!(at(s, &inner), Some((CommitMode::Manual, "crates".into())));
    assert_eq!(at(s, tmp.path()), None);
    // A name given in Acta is the display name.
    s.set_folder_name(o, "Acta app", Author::Human).unwrap();
    assert_eq!(at(s, &outer).unwrap().1, "Acta app");
    // The ledger alone decides nothing.
    apply(s, &inner, mode(CommitMode::Push, false)).unwrap();
    assert_eq!(at(s, &inner).unwrap().0, CommitMode::Manual);
}

#[test]
fn set_commit_writes_the_file_and_is_refused_for_claude() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path().join("r");
    fs::create_dir_all(&f).unwrap();
    let id = attach(s, &f);
    let f = PathBuf::from(stored(&f));
    assert!(set_commit(s, id, CommitMode::Push, None, false, Author::Claude).is_err());
    assert!(!f.join(LOCAL_SETTINGS).exists());
    set_commit(s, id, CommitMode::Commit, Some(true), false, Author::Human).unwrap();
    let mut want = commit_json();
    want["attribution"] = json!({"commit": "", "pr": "", "sessionUrl": false});
    assert_eq!(read(&f), want);
    // In line already: nothing to do.
    assert_eq!(sync_path(s, &f).unwrap(), Outcome::default());
    assert_eq!(sync_all(s).unwrap(), (0, 0));
    // "Commit in my name" left to the Settings default.
    set_commit(s, id, CommitMode::Commit, None, false, Author::Human).unwrap();
    assert!(read(&f).get("attribution").is_none());
    assert_eq!(set_as_user_default(s, true).unwrap(), (1, 0));
    assert!(read(&f).get("attribution").is_some());
    set_commit(s, id, CommitMode::Manual, None, false, Author::Human).unwrap();
    assert!(!f.join(".claude").exists());
}

#[test]
fn settings_left_in_an_offline_folder_are_still_guarded() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path().join("r");
    fs::create_dir_all(&f).unwrap();
    let id = attach(s, &f);
    let f = PathBuf::from(stored(&f));
    set_commit(s, id, CommitMode::Push, Some(false), true, Author::Human).unwrap();
    // Offline: the switch to Manual cannot reach the file.
    let away = tmp.path().join("away");
    fs::rename(&f, &away).unwrap();
    set_commit(s, id, CommitMode::Manual, None, false, Author::Human).unwrap();
    let got = commit_settings_for(s, &f.to_string_lossy())
        .unwrap()
        .unwrap();
    assert_eq!(
        got.settings.mode,
        CommitMode::Push,
        "the file's rules still hold"
    );
    assert!(!got.settings.on_default, "the stricter option");
    // Back: the next sync writes Manual and the column decides again.
    fs::rename(&away, &f).unwrap();
    assert_eq!(sync_all(s).unwrap(), (1, 0));
    assert!(!f.join(LOCAL_SETTINGS).exists());
    let got = commit_settings_for(s, &f.to_string_lossy())
        .unwrap()
        .unwrap();
    assert_eq!(got.settings.mode, CommitMode::Manual);
}

#[test]
fn one_path_at_two_levels_takes_the_strictest() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path().join("r");
    fs::create_dir_all(&f).unwrap();
    let p = attach(s, &f);
    let project = s.projects().unwrap()[0].id;
    let topic = s
        .create_topic(
            crate::store::NewTopic {
                project,
                epic: None,
                title: "Alexandre's topic".into(),
                description: String::new(),
                status: crate::model::Status::Draft,
            },
            Author::Human,
        )
        .unwrap();
    let t2 = s
        .add_folder(crate::model::Owner::Topic(topic), &f, Author::Human)
        .unwrap();
    set_commit(s, p, CommitMode::Push, Some(true), true, Author::Human).unwrap();
    set_commit(s, t2, CommitMode::Commit, Some(false), true, Author::Human).unwrap();
    let got = commit_settings_for(s, &stored(&f))
        .unwrap()
        .unwrap()
        .settings;
    assert_eq!(
        got,
        CommitSettings {
            mode: CommitMode::Commit,
            as_user: false,
            on_default: true
        }
    );
    assert_eq!(read(&PathBuf::from(stored(&f))), commit_json());
}

#[test]
fn an_import_and_a_locate_bring_the_files_in_line() {
    let mut a = open();
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("r");
    fs::create_dir_all(&f).unwrap();
    let id = attach(&mut a.store, &f);
    a.store
        .set_commit_settings(id, CommitMode::Commit, Some(false), false, Author::Human)
        .unwrap();
    assert!(
        !f.join(LOCAL_SETTINGS).exists(),
        "the columns alone write nothing"
    );
    let file = a.store.export(None).unwrap();

    // Another database imports it: the folder exists here, so it is written.
    let mut b = open();
    b.store.import(&file, false).unwrap();
    assert_eq!(read(&PathBuf::from(stored(&f))), commit_json());
    // A Claude Code import changes no commit settings and writes nothing new.
    let mut c = open();
    c.store.import_by(&file, false, Author::Claude).unwrap();
    let cid = c.store.all_folders().unwrap()[0].id;
    assert_eq!(c.store.folder(cid).unwrap().commit_mode, CommitMode::Manual);
    // A Claude Code import keeps an existing folder's local path and
    // writes nothing.
    let moved = tmp.path().join("moved");
    fs::create_dir_all(&moved).unwrap();
    c.store.set_folder_path(cid, &moved, Author::Human).unwrap();
    let before = c.store.folder(cid).unwrap().path;
    let mut newer = file.clone();
    newer.folders[0].stamp.updated_at += 1_000;
    newer.folders[0].commit_mode = CommitMode::Push;
    c.store.import_by(&newer, false, Author::Claude).unwrap();
    assert_eq!(c.store.folder(cid).unwrap().path, before);
    assert_eq!(c.store.folder(cid).unwrap().commit_mode, CommitMode::Manual);
    assert!(!moved.join(LOCAL_SETTINGS).exists());

    // Locate: a folder set to commit while not found is written once found.
    let mut d = open();
    let gone = tmp.path().join("gone");
    fs::create_dir_all(&gone).unwrap();
    let did = attach(&mut d.store, &gone);
    fs::remove_dir(&gone).unwrap();
    d.store
        .set_commit_settings(did, CommitMode::Push, Some(false), false, Author::Human)
        .unwrap();
    assert!(sync_path(&mut d.store, &gone).unwrap() == Outcome::default());
    let found = tmp.path().join("found");
    fs::create_dir_all(&found).unwrap();
    d.store.set_folder_path(did, &found, Author::Human).unwrap();
    let v = read(&PathBuf::from(stored(&found)));
    assert_eq!(v["permissions"]["allow"][2], "Bash(git push)");
}

#[test]
fn an_entry_the_user_removed_is_not_added_again() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let f = tmp.path();
    apply(s, f, mode(CommitMode::Push, true)).unwrap();
    let mut v = read(f);
    v.as_object_mut().unwrap().remove("attribution");
    v["permissions"]["allow"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r != "Bash(git push)");
    fs::write(f.join(LOCAL_SETTINGS), v.to_string()).unwrap();
    let o = apply(s, f, mode(CommitMode::Push, true)).unwrap();
    assert_eq!(o, Outcome::default(), "nothing added back");
    assert_eq!(read(f), v);
    // Not wanted any more, then wanted again: added again.
    apply(s, f, mode(CommitMode::Commit, false)).unwrap();
    let o = apply(s, f, mode(CommitMode::Push, true)).unwrap();
    assert!(o.added.contains(&"attribution".to_owned()), "{o:?}");
    assert!(
        o.added.contains(&"allow:Bash(git push)".to_owned()),
        "{o:?}"
    );
    // A file the user deleted is not made again for the same wishes.
    fs::remove_file(f.join(LOCAL_SETTINGS)).unwrap();
    let o = apply(s, f, mode(CommitMode::Push, true)).unwrap();
    assert_eq!(o, Outcome::default());
    assert!(!f.join(LOCAL_SETTINGS).exists());
    apply(s, f, mode(CommitMode::Manual, false)).unwrap();
    assert!(s.local_values(LEDGER_PREFIX).unwrap().is_empty());
}

#[test]
fn the_exclude_line_uses_canonical_paths() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path().join("Repo");
    repo(&r);
    let sub = r.join("crates").join("core");
    fs::create_dir_all(&sub).unwrap();
    // A non-canonical spelling of the same folder.
    let spelled = PathBuf::from(format!(
        "{}{}crates{}.{}core",
        r.display(),
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    ));
    let o = apply(&mut t.store, &spelled, mode(CommitMode::Commit, false)).unwrap();
    assert!(!o.exclude_failed);
    let exclude = fs::read_to_string(r.join(".git").join("info").join("exclude")).unwrap();
    assert!(
        exclude.contains("/crates/core/.claude/settings.local.json"),
        "{exclude}"
    );
}

#[test]
fn the_form_tells_the_users_own_entries_and_a_tracked_file() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path().join("acta");
    repo(&r);
    let s = &mut t.store;
    let state = |s: &Store| file_state(&r, &recorded(s, &r).unwrap());
    assert_eq!(state(s), FileState::default());
    // Acta's own attribution and variables are not the user's.
    apply(s, &r, mode(CommitMode::Push, true)).unwrap();
    assert_eq!(state(s), FileState::default());
    // The user's own, in a file Acta did not write.
    let user = tmp.path().join("user");
    repo(&user);
    fs::create_dir_all(user.join(".claude")).unwrap();
    fs::write(
        user.join(LOCAL_SETTINGS),
        r#"{"attribution": {"commit": "Alexandre"}, "env": {"GIT_TERMINAL_PROMPT": "1"}}"#,
    )
    .unwrap();
    let st = file_state(&user, &recorded(s, &user).unwrap());
    assert!(st.user_attribution && st.user_env && !st.tracked);
    git(&user, &["add", "-f", LOCAL_SETTINGS]);
    assert!(file_state(&user, &recorded(s, &user).unwrap()).tracked);
}
