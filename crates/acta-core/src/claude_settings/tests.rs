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
    // The settings are still recorded: the guard applies them.
    let dir = r.to_string_lossy().into_owned();
    assert_eq!(
        commit_settings_for(s, &dir).unwrap().unwrap().1.mode,
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

#[test]
fn the_deepest_ledger_decides_the_commit_settings() {
    let mut t = open();
    let tmp = tempfile::tempdir().unwrap();
    let s = &mut t.store;
    let outer = tmp.path().join("acta");
    let inner = outer.join("crates");
    fs::create_dir_all(&inner).unwrap();
    apply(s, &outer, mode(CommitMode::Push, false)).unwrap();
    apply(
        s,
        &inner,
        Wanted {
            auto_memory_off: Some(true),
            commit: None,
        },
    )
    .unwrap();
    let at = |s: &Store, p: &Path| {
        commit_settings_for(s, &p.to_string_lossy())
            .unwrap()
            .map(|(_, c)| c.mode)
    };
    assert_eq!(at(s, &outer.join("docs")), Some(CommitMode::Push));
    assert_eq!(at(s, &inner.join("x")), Some(CommitMode::Manual));
    assert_eq!(at(s, tmp.path()), None);
}
