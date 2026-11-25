//! End to end: the real `acta` binary, driven as Claude Code drives it.
//! `acta mcp` over stdio (JSON-RPC lines), the hooks, `acta link` and the
//! CLI, all against a temporary data directory. The key is in a key file in
//! that directory (or in memory); the real keychain, data directory and
//! `~/.claude` are never touched.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const EXE: &str = env!("CARGO_BIN_EXE_acta-cli");

fn base_cmd(data: &Path) -> Command {
    let mut c = Command::new(EXE);
    c.env("ACTA_DEV_DATA_DIR", data)
        .env_remove("ACTA_DEV_KEYSTORE")
        .env_remove("ACTA_PROJECT")
        .env_remove("ACTA_EPIC")
        .env_remove("ACTA_TOPIC")
        .env_remove("ACTA_LAUNCH")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CLAUDE_PROJECT_DIR")
        // Ready for Claude Code searches only here: no test finds, or
        // asks, the real `claude`, `git` or `gh` (ADR 0013).
        .env("ACTA_DEV_PREREQS_PATH", data.join("no-tools"));
    c
}

/// Runs a CLI command in `dir` with `stdin`; returns (status ok, stdout, stderr).
fn cli(data: &Path, dir: &Path, args: &[&str], stdin: &str) -> (bool, String, String) {
    let mut child = base_cmd(data)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: i64,
    init: Value,
}

impl Mcp {
    fn start(data: &Path, project_dir: &Path, memory_key: bool) -> Mcp {
        Self::start_with(data, project_dir, memory_key, &[])
    }

    fn start_with(data: &Path, project_dir: &Path, memory_key: bool, env: &[(&str, &str)]) -> Mcp {
        let mut cmd = base_cmd(data);
        cmd.env_remove("ACTA_LAUNCH");
        cmd.arg("mcp")
            .env("CLAUDE_PROJECT_DIR", project_dir)
            .env("CLAUDE_CODE_SESSION_ID", "e2e-session");
        // Given variables win over the defaults (a second session's id).
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if memory_key {
            cmd.env("ACTA_DEV_KEYSTORE", "memory");
        }
        let mut child = cmd.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut m = Mcp {
            child,
            stdin: Some(stdin),
            stdout,
            next: 1,
            init: Value::Null,
        };
        let init = m.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "e2e", "version": "0"}
            }),
        );
        assert!(init["result"]["serverInfo"]["name"] == "acta", "{init}");
        m.init = init;
        m.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        m
    }

    fn send(&mut self, v: Value) {
        let stdin = self.stdin.as_mut().expect("open");
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "server closed"
            );
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == json!(id) {
                return v;
            }
        }
    }

    /// Calls a tool; returns (is_error, text, raw result).
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.request("tools/call", json!({"name": name, "arguments": args}));
        let result = &r["result"];
        assert!(
            result.get("structuredContent").is_none(),
            "plain text only: {r}"
        );
        let content = result["content"].as_array().expect("content");
        assert_eq!(content.len(), 1, "{r}");
        let text = content[0]["text"].as_str().unwrap().to_owned();
        (result["isError"] == json!(true), text)
    }
}

impl Mcp {
    /// Closes the connection as Claude Code does when a session ends, and
    /// waits for the server to exit.
    fn close(&mut self) {
        self.stdin.take();
        let _ = self.child.wait();
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

#[test]
fn tools_list_and_instructions_fit_their_budgets() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let unlinked = tmp.path().join("elsewhere");
    std::fs::create_dir_all(&unlinked).unwrap();
    let mut m = Mcp::start(&data, &unlinked, true);
    let init_again = m.request("tools/list", json!({}));
    let tools = init_again["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "attachment",
            "brief",
            "create",
            "entries",
            "find",
            "kb",
            "kb_read",
            "kb_write",
            "log",
            "update"
        ]
        .into_iter()
        .collect()
    );
    let list_size = init_again["result"].to_string().len();
    println!("tools/list: {list_size} chars");
    // ADR 0015, 7: the memory quality fields were paid for by trims; the
    // target after that change is 6,500.
    assert!(list_size <= 6_500, "tools/list is {list_size} chars");
    for t in tools {
        let always = t["_meta"]["anthropic/alwaysLoad"] == json!(true);
        assert_eq!(
            always,
            matches!(t["name"].as_str(), Some("brief" | "log")),
            "{t}"
        );
        assert!(t.get("outputSchema").is_none(), "{t}");
        let read_only = t["annotations"]["readOnlyHint"] == json!(true);
        let expect_ro = matches!(
            t["name"].as_str(),
            Some("brief" | "entries" | "find" | "kb" | "kb_read" | "attachment")
        );
        assert_eq!(read_only, expect_ro, "{t}");
    }
    // Unlinked folder: one line saying so.
    let (err, text) = m.call("brief", json!({}));
    assert!(err);
    assert!(text.contains("not linked"), "{text}");
    assert_eq!(text.lines().count(), 1);
    // The hook tool answers {} when there is nothing to say.
    let (err, text) = m.call("hook", json!({"event": "subagent"}));
    assert!(!err);
    assert_eq!(text, "{}");
}

#[test]
fn instructions_reach_the_client_under_800_chars() {
    let tmp = tempfile::tempdir().unwrap();
    let m = Mcp::start(&tmp.path().join("data"), tmp.path(), true);
    let instructions = m.init["result"]["instructions"].as_str().unwrap();
    assert!(chars(instructions) < 800, "{}", chars(instructions));
    assert!(instructions.contains("kb_write"));
}

/// Seeds through the CLI, then drives every tool over MCP and checks each
/// real `tools/call` result against its budget.
#[test]
fn every_tool_over_stdio_within_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    assert!(out.contains("to P-1 (primary)"), "{out}");
    let settings = std::fs::read_to_string(repo.join(".claude/settings.local.json")).unwrap();
    assert!(
        settings.contains("\"autoMemoryEnabled\": false"),
        "{settings}"
    );
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["create", "epic"],
        "Storage\nEverything about the encrypted store",
    );
    assert!(ok && out.starts_with("ok E-1"), "{out}{err}");
    let (ok, out, _) = cli(
        &data,
        &repo,
        &["create", "topic", "--epic", "E-1"],
        "Encrypted storage spike",
    );
    assert!(ok && out.starts_with("ok T-1"), "{out}");
    for i in 0..12 {
        let (ok, _, err) = cli(
            &data,
            &repo,
            &["log", "decision", "--topic", "T-1"],
            &"Raw 32-byte key, no KDF, open in 2 ms. ".repeat(20 + i),
        );
        assert!(ok, "{err}");
    }

    let mut m = Mcp::start(&data, &repo, false);
    let budgets: Vec<(&str, Value, usize)> = vec![
        ("brief", json!({"id": "T-1", "focus": true}), 1_200),
        ("brief", json!({}), 1_200),
        ("brief", json!({"id": "E-1"}), 1_200),
        ("entries", json!({}), 2_000),
        ("entries", json!({"limit": 20}), 6_000),
        ("entries", json!({"id": "T-1#3"}), 6_000),
        ("find", json!({"query": "key"}), 2_000),
        ("find", json!({"kind": "note"}), 2_000),
        (
            "log",
            json!({"type": "progress", "text": "WAL on; contention test passes.", "state": "WAL on. Next: snapshots."}),
            160,
        ),
        ("update", json!({"id": "T-1", "status": "active"}), 160),
        (
            "create",
            json!({"items": [{"kind": "note", "title": "Ask Alexandre about the macOS keychain prompt"}]}),
            320,
        ),
        (
            "kb_write",
            json!({"kind": "memory", "level": "project", "title": "Clippy runs with -D warnings", "summary": "CI fails on any clippy warning; run it before a commit.", "text": "cargo clippy --workspace --all-targets -- -D warnings"}),
            160,
        ),
        ("kb", json!({}), 1_800),
        ("kb", json!({"query": "clippy"}), 1_800),
        ("kb_read", json!({"id": "K-1"}), 6_000),
        (
            "hook",
            json!({"event": "compact", "session": "e2e-2"}),
            8_000,
        ),
        ("hook", json!({"event": "subagent"}), 8_000),
    ];
    for (tool, args, budget) in budgets {
        let (err, text) = m.call(tool, args.clone());
        assert!(!err, "{tool} {args} refused: {text}");
        assert!(
            chars(&text) <= budget,
            "{tool} {args}: {} > {budget}\n{text}",
            chars(&text)
        );
    }
    let (_, brief) = m.call("brief", json!({}));
    // One data fence with a random 8-hex nonce around the whole brief.
    let lines: Vec<&str> = brief.lines().collect();
    let nonce = lines[0]
        .strip_prefix("[acta data ")
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or_else(|| panic!("{brief}"));
    assert!(nonce.len() == 8 && nonce.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(*lines.last().unwrap(), format!("[end acta data {nonce}]"));
    assert!(
        lines[1].starts_with("T-1 Encrypted storage spike · active · E-1 Storage"),
        "{brief}"
    );
    assert!(brief.contains("State ("), "{brief}");
    let (_, sub) = m.call("hook", json!({"event": "subagent"}));
    let v: Value = serde_json::from_str(&sub).unwrap();
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        chars(ctx) <= 300 && ctx.contains("parent focus T-1"),
        "{ctx}"
    );
    let (err, text) = m.call("log", json!({"type": "nonsense", "text": "x"}));
    assert!(
        err && text.contains("progress, decision or discussion"),
        "{text}"
    );
    drop(m);

    // The CLI reads what the server wrote (the author is Claude over MCP).
    let (ok, out, _) = cli(&data, &repo, &["entries", "--limit", "1"], "");
    assert!(ok && out.contains("progress claude"), "{out}");
}

/// The `/acta:triage` flow over stdio: notes captured by the human (CLI),
/// read and filed by the agent (MCP) in one call, checked and partly
/// undone from the CLI.
#[test]
fn a_full_triage_over_stdio() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(cli(&data, &repo, &["link"], "").0);
    let (ok, out, err) = cli(&data, &repo, &["create", "epic"], "Storage");
    assert!(ok && out.starts_with("ok E-1"), "{out}{err}");
    let (ok, out, _) = cli(
        &data,
        &repo,
        &["create", "topic", "--epic", "E-1"],
        "Encrypted storage",
    );
    assert!(ok && out.starts_with("ok T-1"), "{out}");
    for text in [
        "Sync between two laptops",
        "Conflict handling when both edit the same topic",
        "Quick capture from the tray",
        "Capture should keep the last project",
        "WAL checkpoint runs too often on battery",
        "Quick capture from the tray (again)",
    ] {
        let (ok, out, err) = cli(&data, &repo, &["create", "note"], text);
        assert!(ok && out.starts_with("ok N-"), "{out}{err}");
    }

    let mut m = Mcp::start(&data, &repo, false);
    let (err, inbox) = m.call("find", json!({"kind": "note"}));
    assert!(!err && inbox.contains("N-6 note  Quick capture from the tray (again)"));
    assert!(
        inbox.ends_with("6 hits") && chars(&inbox) <= 2_000,
        "{inbox}"
    );
    let plan = json!({
        "items": [
            {"kind": "epic", "title": "Sync", "from_notes": ["N-1"]},
            {"kind": "topic", "title": "Conflict handling", "epic": "@1", "from_notes": ["N-2"]},
            {"kind": "topic", "title": "Quick capture", "epic": "E-1", "from_notes": ["N-3", "N-4"]},
            {"kind": "entry", "topic": "T-1", "from_notes": ["N-5"]}
        ],
        "dismiss": ["N-6"]
    });
    // One bad note id refuses the whole plan.
    let mut bad = plan.clone();
    bad["dismiss"] = json!(["N-6", "N-1"]);
    let (err, text) = m.call("create", bad);
    assert!(err && text.contains("N-1 is used twice"), "{text}");
    let (err, ack) = m.call("create", plan);
    assert!(!err, "{ack}");
    assert_eq!(
        ack,
        "ok E-2 (N-1) · T-2 draft in E-2 (N-2) · T-3 draft in E-1 (N-3 N-4) · T-1 +1 entry (N-5) · dismissed N-6"
    );
    assert!(chars(&ack) <= 320);
    let (_, inbox) = m.call("find", json!({"kind": "note"}));
    assert_eq!(inbox, "0 hits");
    let (_, brief) = m.call("brief", json!({"id": "E-2"}));
    assert!(brief.contains("T-2 draft  Conflict handling"), "{brief}");
    let (_, entries) = m.call("entries", json!({"topic": "T-1"}));
    assert!(
        entries.contains("#1 ") && entries.contains("discussion"),
        "{entries}"
    );
    let (err, back) = m.call("update", json!({"id": "N-3", "status": "open"}));
    assert!(
        !err && back == "ok N-3 updated: back in the inbox",
        "{back}"
    );
    drop(m);

    // The CLI equivalents: the inbox, a plan as JSON, dismiss.
    let (ok, out, _) = cli(&data, &repo, &["find", "--kind", "note"], "");
    assert!(ok && out.contains("N-3 note  Quick capture"), "{out}");
    let (ok, out, err) = cli(&data, &repo, &["create", "note"], "Tray icon in dark mode");
    assert!(ok && out.starts_with("ok N-7"), "{out}{err}");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["create", "plan"],
        r#"{"items":[{"kind":"entry","topic":"T-3","from_notes":["N-3"]}]}"#,
    );
    assert!(ok && out.trim() == "ok T-3 +1 entry (N-3)", "{out}{err}");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["create", "dismiss", "--from-notes", "N-7"],
        "",
    );
    assert!(ok && out.trim() == "ok dismissed N-7", "{out}{err}");
    let (ok, out, _) = cli(&data, &repo, &["find", "--kind", "note"], "");
    assert!(ok && out.trim() == "0 hits", "{out}");
}

#[test]
fn hooks_exit_fast_and_silently_outside_linked_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    let other = tmp.path().join("other");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    // Nothing linked yet: no index, no database created by the hook.
    let input = |dir: &Path| {
        json!({"session_id": "s1", "cwd": dir, "hook_event_name": "SessionStart", "source": "startup"}).to_string()
    };
    let (ok, out, _) = cli(&data, &other, &["hook", "session-start"], &input(&other));
    assert!(ok && out.is_empty());
    assert!(
        !data.join("acta.db").exists(),
        "the hook must not create the database"
    );

    let (ok, out, err) = cli(&data, &repo, &["link", "--keep-auto-memory"], "");
    assert!(ok, "{out}{err}");
    assert!(
        !repo.join(".claude").exists(),
        "--keep-auto-memory writes nothing"
    );
    let (ok, _, _) = cli(&data, &repo, &["create", "topic"], "Hook topic");
    assert!(ok);
    let (ok, _, _) = cli(&data, &repo, &["brief", "T-1", "--focus"], "");
    assert!(ok);
    let index = std::fs::read_to_string(data.join("linked-folders")).unwrap();
    assert!(
        !index.contains("repo"),
        "the pre-check file holds hashes only"
    );

    // Unrelated folder: silent, and fast because the database is not opened.
    // Generous bounds for CI runners; one warm-up run each.
    let timed = |dir: &Path| {
        let _ = cli(&data, dir, &["hook", "session-start"], &input(dir));
        let start = Instant::now();
        let r = cli(&data, dir, &["hook", "session-start"], &input(dir));
        (r, start.elapsed())
    };
    let ((ok, out, _), miss) = timed(&other);
    assert!(ok && out.is_empty(), "{out}");
    assert!(miss < Duration::from_millis(500), "miss path took {miss:?}");
    let ((ok, out, _), hit) = timed(&repo);
    assert!(ok && out.contains("Hook topic"), "{out}");
    assert!(hit < Duration::from_millis(500), "hit path took {hit:?}");

    // A hook never fails the session, whatever it is given.
    for args in [
        vec!["hook", "no-such-event"],
        vec!["hook"],
        vec!["hook", "session-start", "--bogus"],
    ] {
        let mut c = base_cmd(&data);
        let out = c
            .args(&args)
            .current_dir(&other)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}");
        assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{args:?}");
    }
    let (ok, out, _) = cli(&data, &repo, &["hook", "session-start"], "not json");
    assert!(
        ok && out.contains("Hook topic"),
        "bad stdin falls back to the cwd: {out}"
    );

    // A subfolder of the linked repository gets the briefing.
    let sub = repo.join("src");
    let (ok, out, _) = cli(&data, &sub, &["hook", "session-start"], &input(&sub));
    assert!(ok);
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.contains("T-1 Hook topic"), "{ctx}");
    assert!(chars(ctx) <= 1_200);

    let (ok, out, _) = cli(&data, &sub, &["hook", "pre-compact"], &input(&sub));
    assert!(ok && out.contains("focus id"), "{out}");
    let (ok, out, _) = cli(&data, &other, &["hook", "pre-compact"], &input(&other));
    assert!(ok && out.is_empty());

    // Unlink: the index no longer matches.
    let (ok, out, err) = cli(&data, &repo, &["link", "--remove"], "");
    assert!(ok, "{out}{err}");
    let (ok, out, _) = cli(&data, &sub, &["hook", "session-start"], &input(&sub));
    assert!(ok && out.is_empty(), "{out}");
}

#[test]
fn link_remove_restores_auto_memory_and_export_import_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".git").join("info")).unwrap();
    std::fs::write(repo.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
    // An existing exclude file, not UTF-8 and without a final newline, is
    // appended to, never rewritten.
    let before: &[u8] = b"# user rules\n/build\n\xff\xfe-latin1";
    std::fs::write(repo.join(".git/info/exclude"), before).unwrap();
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    let exclude = std::fs::read(repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.starts_with(before));
    assert!(
        exclude.ends_with(b"\n/.claude/settings.local.json\n"),
        "{}",
        String::from_utf8_lossy(&exclude)
    );
    let (ok, out, _) = cli(&data, &repo, &["link"], "");
    assert!(ok && out.contains("already linked"), "{out}");
    let (ok, _, _) = cli(&data, &repo, &["link", "--remove"], "");
    assert!(ok);
    assert!(!repo.join(".claude/settings.local.json").exists());

    let (ok, _, _) = cli(&data, &repo, &["link"], "");
    assert!(ok);
    let (ok, _, _) = cli(&data, &repo, &["create", "topic"], "Exported topic");
    assert!(ok);
    let f = |p: &Path| p.to_str().unwrap().to_owned();
    // Without a terminal the passphrase must come with --passphrase-stdin:
    // it is never an argument.
    let file = tmp.path().join("out.acta-backup");
    let (ok, _, err) = cli(&data, &repo, &["export", &f(&file)], "");
    assert!(!ok && err.contains("--passphrase-stdin"), "{err}");
    assert!(!file.exists());
    let (ok, _, err) = cli(
        &data,
        &repo,
        &["export", &f(&file), "--passphrase-stdin"],
        "short\n",
    );
    assert!(!ok && err.contains("at least 8"), "{err}");
    assert!(!file.exists());
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["export", &f(&tmp.path().join("out")), "--passphrase-stdin"],
        "Alexandre's long passphrase\r\n",
    );
    assert!(
        ok && out.contains("to out.acta-backup") && out.contains("cannot recover"),
        "{out}{err}"
    );
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.starts_with(b"age-encryption.org/v1\n"));
    assert!(!bytes.windows(14).any(|w| w == b"Exported topic"));
    let (ok, _, err) = cli(
        &data,
        &repo,
        &["export", &f(&file), "--passphrase-stdin"],
        "Alexandre's long passphrase\n",
    );
    assert!(
        !ok && err.contains("exists"),
        "a backup never overwrites: {err}"
    );
    let (ok, _, err) = cli(
        &data,
        &repo,
        &["import", &f(&file), "--dry-run", "--passphrase-stdin"],
        "Alexandre's wrong passphrase\n",
    );
    assert!(!ok && err.contains("wrong passphrase"), "{err}");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["import", &f(&file), "--dry-run", "--passphrase-stdin"],
        "Alexandre's long passphrase\n",
    );
    assert!(ok && out.contains("0 created"), "{out}{err}");
    // A truncated backup is refused whole.
    let cut = tmp.path().join("cut.acta-backup");
    std::fs::write(&cut, &bytes[..bytes.len() - 40]).unwrap();
    let (ok, _, err) = cli(
        &data,
        &repo,
        &["import", &f(&cut), "--passphrase-stdin"],
        "Alexandre's long passphrase\n",
    );
    assert!(!ok && err.contains("damaged"), "{err}");
    // A plaintext export of an earlier Acta is not a backup.
    let old = tmp.path().join("old.acta.json");
    std::fs::write(&old, "{\"format\":\"acta\",\"version\":4}").unwrap();
    let (ok, _, err) = cli(&data, &repo, &["import", &f(&old)], "");
    assert!(!ok && err.contains("not an Acta backup"), "{err}");
    // Under Claude Code, import is refused: it could set commit modes.
    let out = base_cmd(&data)
        .env("CLAUDECODE", "1")
        .args(["import", file.to_str().unwrap()])
        .current_dir(&repo)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && err.contains("Import is for you to run, not Claude Code"),
        "{err}"
    );
    // Export and restore are the user's too, whichever Claude Code
    // variable gives the agent away; the snapshot list stays open.
    let other = tmp.path().join("other.acta-backup");
    for (var, value) in [
        ("CLAUDECODE", "1"),
        ("CLAUDE_CODE_ENTRYPOINT", "cli"),
        ("CLAUDE_PROJECT_DIR", repo.to_str().unwrap()),
    ] {
        let run = |args: &[&str]| {
            let out = base_cmd(&data)
                .env(var, value)
                .args(args)
                .current_dir(&repo)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            (
                out.status.success(),
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        };
        let (ok, _, err) = run(&["export", other.to_str().unwrap()]);
        assert!(
            !ok && err.contains("Export is for you to run"),
            "{var}: {err}"
        );
        assert!(!other.exists(), "{var}");
        let (ok, _, err) = run(&["snapshot", "restore", "latest"]);
        assert!(!ok && err.contains("is for you to run"), "{var}: {err}");
        let (ok, _, err) = run(&["import", file.to_str().unwrap()]);
        assert!(!ok && err.contains("Import is for you"), "{var}: {err}");
        let (ok, out, err) = run(&["snapshot", "list"]);
        assert!(ok, "{var}: {out}{err}");
    }
    let (ok, out, _) = cli(&data, &repo, &["snapshot", "list"], "");
    assert!(ok, "{out}");
}

#[test]
fn setup_with_a_dev_data_dir_needs_a_claude_config_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let (ok, _, err) = cli(&data, tmp.path(), &["setup"], "");
    assert!(!ok && err.contains("CLAUDE_CONFIG_DIR"), "{err}");
    assert!(!data.join("claude").exists(), "nothing written");
}

/// A stub executable for `acta setup --check`: `cases` maps the first
/// argument to the line printed (exit 0); anything else prints `[]`.
fn tool_stub(dir: &Path, name: &str, cases: &[(&str, &str)]) {
    if cfg!(windows) {
        let mut body = String::from("@echo off\r\n");
        for (i, (arg, _)) in cases.iter().enumerate() {
            body.push_str(&format!("if \"%~1\"==\"{arg}\" goto c{i}\r\n"));
        }
        body.push_str("echo []\r\nexit /b 0\r\n");
        for (i, (_, line)) in cases.iter().enumerate() {
            body.push_str(&format!(":c{i}\r\necho {line}\r\nexit /b 0\r\n"));
        }
        std::fs::write(dir.join(format!("{name}.cmd")), body).unwrap();
    } else {
        let mut body = String::from("#!/bin/sh\ncase \"$1\" in\n");
        for (arg, line) in cases {
            body.push_str(&format!("  {arg}) echo '{line}' ;;\n"));
        }
        body.push_str("  *) echo '[]' ;;\nesac\n");
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}

/// `acta setup --check` against a fake PATH of stub tools: the four rows
/// of Ready for Claude Code come first, in order, with their states and
/// versions, the sign-in keeps only the method, and both forms fit their
/// budgets. Nothing real is found, run or installed.
#[test]
fn setup_check_lists_ready_for_claude_code_from_a_fake_path() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let tools = tmp.path().join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    let auth = r#"{"loggedIn":true,"authMethod":"claude.ai","email":"alexandre@example.com"}"#;
    tool_stub(
        &tools,
        "claude",
        &[("--version", "2.1.330 (Claude Code)"), ("auth", auth)],
    );
    tool_stub(
        &tools,
        "git",
        &[("--version", "git version 2.56.1.windows.1")],
    );
    tool_stub(
        &tools,
        "gh",
        &[("--version", "gh version 2.105.0 (2026-11-18)")],
    );
    let run_with = |extra: &[&str]| {
        let mut args = vec!["setup", "--check"];
        args.extend_from_slice(extra);
        let out = base_cmd(&data)
            .args(&args)
            .env("ACTA_DEV_PREREQS_PATH", &tools)
            .env("CLAUDE_CONFIG_DIR", tmp.path().join("claude-config"))
            .current_dir(tmp.path())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let run = |json: bool| run_with(if json { &["--json"] } else { &[] });
    let out = run(true);
    let v: Value = serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("{e}: {out}"));
    let steps = v["steps"].as_array().unwrap();
    let names: Vec<&str> = steps.iter().map(|s| s["step"].as_str().unwrap()).collect();
    assert_eq!(
        names[..5],
        ["claude", "signed in", "git", "gh", "executable"],
        "{out}"
    );
    let row = |i: usize| &steps[i];
    assert_eq!(row(0)["state"], "ok", "{out}");
    assert_eq!(row(0)["version"], "2.1.330");
    assert_eq!(row(0)["ok"], true);
    assert_eq!(row(1)["state"], "ok");
    assert_eq!(row(1)["detail"], "claude.ai");
    assert_eq!(row(2)["state"], "ok");
    assert_eq!(row(2)["version"], "2.56.1");
    assert_eq!(row(3)["state"], "ok");
    assert_eq!(row(3)["version"], "2.105.0");
    for i in 0..4 {
        assert!(row(i).get("command").is_some() && row(i).get("kind").is_some());
    }
    assert!(!out.contains("alexandre@"), "{out}");
    assert!(chars(&out) < 2000, "{} chars: {out}", chars(&out));
    assert_eq!(row(0)["found"], true);
    assert!(row(1).get("found").is_none());
    let text = run(false);
    assert!(text.starts_with("claude       ok        2.1.330"), "{text}");
    assert!(chars(&text) < 1200, "{} chars: {text}", chars(&text));
    // The cheap form for the app's focus refresh: plugin steps only.
    let cheap: Value = serde_json::from_str(run_with(&["--no-prereqs", "--json"]).trim()).unwrap();
    assert_eq!(cheap["steps"][0]["step"], "executable", "{cheap}");

    // Without gh, its row is optional and does not change the outcome;
    // without claude, the required rows fail and say what to run.
    std::fs::remove_file(tools.join(if cfg!(windows) { "gh.cmd" } else { "gh" })).unwrap();
    let v: Value = serde_json::from_str(run(true).trim()).unwrap();
    assert_eq!(v["steps"][3]["state"], "optional");
    assert_eq!(v["steps"][3]["ok"], true);
    std::fs::remove_file(tools.join(if cfg!(windows) {
        "claude.cmd"
    } else {
        "claude"
    }))
    .unwrap();
    let v: Value = serde_json::from_str(run(true).trim()).unwrap();
    assert_eq!(v["steps"][0]["state"], "missing");
    assert_eq!(v["steps"][0]["found"], false);
    assert_eq!(v["steps"][0]["ok"], false);
    assert!(
        v["steps"][0]["command"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "{v}"
    );
    assert_eq!(v["steps"][1]["ok"], false);
    assert_eq!(v["ok"], false);
}

/// Session presence over stdio: the server registers its session in a
/// linked folder, Acta's presence hooks move it, answering nothing the
/// model would read, `log` sets the activity line, and closing the
/// connection ends it. A session in an unrelated folder leaves no trace.
#[test]
fn presence_follows_the_session_over_stdio() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    let other = tmp.path().join("other");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    // Unrelated folder, nothing linked: hooks answer empty, no file.
    let mut stray = Mcp::start(&data, &other, false);
    let (err, text) = stray.call("hook", json!({"event": "prompt", "session": "stray"}));
    assert!(!err && text.is_empty(), "{text}");
    drop(stray);
    assert!(!data.join("presence.db").exists());

    let (ok, out, err) = cli(&data, &repo, &["link", "--keep-auto-memory"], "");
    assert!(ok, "{out}{err}");
    let (ok, _, _) = cli(&data, &repo, &["create", "topic"], "Embed the terminal");
    assert!(ok);

    let sessions = || cli(&data, &repo, &["sessions"], "").1;
    let mut m = Mcp::start_with(
        &data,
        &repo,
        false,
        &[("ACTA_TOPIC", "T-1"), ("ACTA_LAUNCH", "tok-1")],
    );
    // The heartbeat registers the session right after start.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !sessions().contains("T-1 idle") {
        assert!(Instant::now() < deadline, "not registered: {}", sessions());
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(sessions().contains("in Acta"), "{}", sessions());

    for (event, detail) in [
        ("prompt", None),
        ("tool", Some("mcp__plugin_acta_acta__brief")),
    ] {
        let mut args = json!({"event": event, "session": "e2e-session"});
        if let Some(d) = detail {
            args["detail"] = json!(d);
        }
        let (err, text) = m.call("hook", args);
        assert!(!err && text.is_empty(), "{event}: {text}");
    }
    assert!(sessions().contains("T-1 working"), "{}", sessions());
    let (err, _) = m.call(
        "log",
        json!({"type": "progress", "text": "Terminal tab shows the focus
second line"}),
    );
    assert!(!err);
    let (err, text) = m.call(
        "hook",
        json!({"event": "waiting", "session": "e2e-session"}),
    );
    assert!(!err && text.is_empty());
    let out = sessions();
    assert!(out.contains("T-1 waiting"), "{out}");
    assert!(out.contains("last: Terminal tab shows the focus"), "{out}");
    assert!(!out.contains("second line"), "{out}");

    // SessionEnd for /clear keeps it; the briefing hook adopts the new id.
    let (_, text) = m.call(
        "hook",
        json!({"event": "end", "detail": "clear", "session": "e2e-session"}),
    );
    assert!(text.is_empty());
    let (err, _) = m.call("hook", json!({"event": "clear", "session": "e2e-cleared"}));
    assert!(!err);
    assert!(sessions().contains("T-1 idle"), "{}", sessions());

    // The client closes the connection: the session ends.
    m.close();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !sessions().contains("no Claude Code session") {
        assert!(Instant::now() < deadline, "not ended: {}", sessions());
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// `acta statusline` as Claude Code runs it: the plan's numbers kept in
/// `usage.json` (and nothing else), a short line printed, the user's own
/// status line run first with the same stdin when setup recorded one. No
/// database is created.
#[test]
fn statusline_keeps_usage_and_wraps_the_users_line() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let dir = tmp.path();
    let payload = json!({
        "session_id": "s-alexandre",
        "model": {"display_name": "Opus"},
        "workspace": {"current_dir": "C:/Users/alexandre/acta"},
        "rate_limits": {
            "five_hour": {"used_percentage": 42.4, "resets_at": 4_102_444_800i64},
            "seven_day": {"used_percentage": 18, "resets_at": 4_102_444_800i64}
        }
    })
    .to_string();
    let (ok, out, err) = cli(&data, dir, &["statusline"], &payload);
    assert!(ok, "{err}");
    assert_eq!(out.trim(), "5h 42% \u{b7} 7d 18%");
    let usage = std::fs::read_to_string(data.join("usage.json")).unwrap();
    assert!(
        usage.contains("42.4") && !usage.contains("alexandre"),
        "{usage}"
    );
    assert!(
        !data.join("acta.db").exists(),
        "no database for the status line"
    );

    // No rate limits in this payload (an API-key session): the last
    // numbers stay and still show.
    let (ok, out, _) = cli(&data, dir, &["statusline"], r#"{"session_id":"x"}"#);
    assert!(ok);
    assert_eq!(out.trim(), "5h 42% \u{b7} 7d 18%");

    // The user's own status line, kept by setup, comes first.
    std::fs::create_dir_all(data.join("claude")).unwrap();
    std::fs::write(
        data.join("claude").join("setup-record.json"),
        json!({"statusline_original": {"type": "command", "command": "echo mine"}}).to_string(),
    )
    .unwrap();
    let (ok, out, _) = cli(&data, dir, &["statusline"], &payload);
    assert!(ok);
    assert_eq!(out.trim(), "mine  5h 42% \u{b7} 7d 18%");

    // Called from inside the user's own command (a script that calls
    // `acta statusline`): the original is not run again.
    let nested = base_cmd(&data)
        .arg("statusline")
        .env("ACTA_STATUSLINE_NESTED", "1")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            c.stdin.take().unwrap().write_all(payload.as_bytes())?;
            c.wait_with_output()
        })
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&nested.stdout).trim(),
        "5h 42% \u{b7} 7d 18%"
    );
    // An Acta command recorded as the original (an older install) is never
    // run: no recursion.
    let acta = format!("{} statusline", EXE.replace('\\', "/"));
    std::fs::write(
        data.join("claude").join("setup-record.json"),
        json!({"statusline_original": {"command": acta}}).to_string(),
    )
    .unwrap();
    let (ok, out, _) = cli(&data, dir, &["statusline"], &payload);
    assert!(ok);
    assert_eq!(out.trim(), "5h 42% \u{b7} 7d 18%");
}

/// `acta link --attached`: the folders already attached, read from Acta (no
/// path on the command line), get Claude Code's auto memory off; `acta
/// setup --remove` turns it back on where Acta turned it off. `claude` runs
/// against a temporary configuration directory only.
#[test]
fn link_attached_and_setup_remove_round_trip_auto_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    let broken = tmp.path().join("broken");
    let other = tmp.path().join("other");
    for d in [&repo, &broken, &other] {
        std::fs::create_dir_all(d).unwrap();
    }
    let (ok, out, err) = cli(&data, &repo, &["link", "--keep-auto-memory"], "");
    assert!(ok, "{out}{err}");
    let (ok, out, err) = cli(&data, &broken, &["link", "--keep-auto-memory", "--add"], "");
    assert!(ok, "{out}{err}");
    let local = repo.join(".claude").join("settings.local.json");
    let broken_local = broken.join(".claude").join("settings.local.json");
    assert!(!local.exists());

    let (ok, out, err) = cli(
        &data,
        &other,
        &["link", "--attached", "--project", "P-1", "--add"],
        "",
    );
    assert!(ok, "{out}{err}");
    assert!(
        out.contains("2 attached folder") && out.contains("turned off in 2"),
        "{out}"
    );
    let text = std::fs::read_to_string(&local).unwrap();
    assert!(text.contains("\"autoMemoryEnabled\": false"), "{text}");
    // Idempotent; a path is refused with --attached.
    let (ok, out, _) = cli(&data, &other, &["link", "--attached"], "");
    assert!(ok && !out.contains("turned off"), "{out}");
    let (ok, _, _) = cli(&data, &other, &["link", "--attached", "x"], "");
    assert!(!ok);

    // One settings file can no longer be read: that folder keeps its record.
    std::fs::write(&broken_local, "{ not json").unwrap();
    let config = tmp.path().join("claude-config");
    std::fs::create_dir_all(&config).unwrap();
    let remove = || {
        let out = base_cmd(&data)
            .args(["setup", "--remove", "--json"])
            .env("CLAUDE_CONFIG_DIR", &config)
            .current_dir(&other)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let stdout = remove();
    assert!(
        stdout.contains("back on in 1 folder") && stdout.contains("1 left as they were"),
        "{stdout}"
    );
    assert!(!local.exists(), "the settings file Acta created is gone");
    assert_eq!(
        std::fs::read_to_string(&broken_local).unwrap(),
        "{ not json"
    );
    // Repaired, the next removal restores it: the record was kept.
    std::fs::write(&broken_local, "{\"autoMemoryEnabled\": false}").unwrap();
    let stdout = remove();
    assert!(stdout.contains("back on in 1 folder"), "{stdout}");
    assert!(!broken_local.exists());
}

/// Due dates and the Attention line over stdio (data.md requests 16 and
/// 17): the agent sets, finds and clears a date; the CLI does the same with
/// `--due` and reads it back with `--json`. Dates far in the past or the
/// future keep the test independent of today.
#[test]
fn due_dates_set_found_and_cleared_over_stdio() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(cli(&data, &repo, &["link"], "").0);
    let (ok, out, err) = cli(&data, &repo, &["create", "epic"], "Storage");
    assert!(ok && out.starts_with("ok E-1"), "{out}{err}");
    for title in ["Encrypted storage", "Snapshots"] {
        let (ok, out, err) = cli(&data, &repo, &["create", "topic", "--epic", "E-1"], title);
        assert!(ok && out.starts_with("ok T-"), "{out}{err}");
    }

    let mut m = Mcp::start(&data, &repo, false);
    let (err, ack) = m.call("update", json!({"id": "T-1", "due": "2000-01-03"}));
    assert!(
        !err && ack.starts_with("ok T-1 · due 2000-01-03 ("),
        "{ack}"
    );
    assert!(chars(&ack) <= 160);
    let (err, text) = m.call("update", json!({"id": "T-2", "due": "next friday"}));
    assert!(err, "{text}");
    assert_eq!(
        text,
        "due must be YYYY-MM-DD (2000-2099) or \"\" to clear, got \"next friday\""
    );
    let (err, ack) = m.call("update", json!({"id": "E-1", "due": "2000-01-05"}));
    assert!(!err && ack.starts_with("ok E-1 · due 2000-01-05"), "{ack}");
    let (err, ack) = m.call("update", json!({"id": "P-1", "due": "2099-12-31"}));
    assert!(!err && ack.starts_with("ok P-1 · due 2099-12-31"), "{ack}");
    let (err, ack) = m.call(
        "create",
        json!({"items": [{"kind": "topic", "title": "Backups", "epic": "E-1", "due": "2099-06-01"}]}),
    );
    assert!(!err && ack.starts_with("ok T-3"), "{ack}");

    let (err, late) = m.call("find", json!({"due": "overdue"}));
    assert!(!err && chars(&late) <= 2_000, "{late}");
    let rows: Vec<&str> = late
        .lines()
        .filter(|l| l.starts_with("T-") || l.starts_with("E-"))
        .collect();
    assert_eq!(rows.len(), 2, "{late}");
    assert!(
        rows[0].starts_with("T-1 planned  Encrypted storage · E-1 · overdue ")
            && rows[0].ends_with("(due 2000-01-03)"),
        "{late}"
    );
    assert!(rows[1].starts_with("E-1 epic  Storage"), "{late}");
    assert!(late.ends_with("2 hits"), "{late}");

    let (_, project) = m.call("brief", json!({"id": "P-1"}));
    assert!(chars(&project) <= 1_200, "{project}");
    let attention = project
        .lines()
        .find(|l| l.starts_with("Attention: "))
        .unwrap_or_else(|| panic!("{project}"));
    assert!(
        attention.starts_with("Attention: overdue T-1 "),
        "{project}"
    );
    assert!(attention.contains(", E-1 "), "{project}");
    assert!(chars(attention) <= 160);
    assert!(project.contains("· due 2099-12-31"), "{project}");
    let (_, topic) = m.call("brief", json!({"id": "T-1"}));
    assert!(
        topic.contains("T-1 Encrypted storage · planned · overdue "),
        "{topic}"
    );
    assert!(topic.contains("(due 2000-01-03)"), "{topic}");
    // The session-start briefing is the brief: it carries the segments.
    let (_, hook) = m.call("hook", json!({"event": "compact", "session": "e2e-due"}));
    let v: Value = serde_json::from_str(&hook).unwrap();
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        chars(ctx) <= 1_200 && ctx.contains("Attention: overdue"),
        "{ctx}"
    );

    let (err, ack) = m.call("update", json!({"id": "T-1", "due": ""}));
    assert!(!err && ack == "ok T-1 · due cleared", "{ack}");
    let (_, late) = m.call("find", json!({"due": "overdue", "kind": "topic"}));
    assert_eq!(late, "0 hits");
    drop(m);

    // The CLI: --due sets and clears, --json reads back.
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["update", "T-2", "--due", "2000-02-01", "--json"],
        "",
    );
    assert!(ok, "{out}{err}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(
        v,
        json!({"id": "T-2", "due": "2000-02-01", "overdue": true, "reported_at": null, "report_to": null, "report_contact": null})
    );
    let (ok, out, err) = cli(&data, &repo, &["find", "--due", "overdue", "--json"], "");
    assert!(ok, "{out}{err}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    let ids: Vec<&str> = v["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["E-1", "T-2"], "{out}");
    assert_eq!(v["hits"][1]["due"], "2000-02-01");
    assert_eq!(v["hits"][1]["overdue"], true);
    let (ok, out, err) = cli(&data, &repo, &["update", "T-2", "--due", "friday"], "");
    assert!(!ok && err.contains("due must be YYYY-MM-DD"), "{out}{err}");
    let (ok, out, _) = cli(&data, &repo, &["update", "T-2", "--due", ""], "");
    assert!(ok && out.trim() == "ok T-2 · due cleared", "{out}");
    let (ok, out, _) = cli(&data, &repo, &["find", "--due", "overdue"], "");
    assert!(
        ok && out.contains("E-1 epic  Storage") && out.trim_end().ends_with("1 hit"),
        "{out}"
    );
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// ADR 0011: `acta folders commit` writes the folder's Claude Code
/// settings (refused under Claude Code), and `acta hook pre-git` denies
/// what an automatic mode forbids, silently otherwise, within 200 chars.
#[test]
fn commit_modes_and_the_pre_git_guard() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    let other = tmp.path().join("other");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&other).unwrap();
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
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");

    // Claude Code cannot set a commit mode.
    let out = base_cmd(&data)
        .args(["folders", "commit", "push"])
        .env("CLAUDECODE", "1")
        .current_dir(&repo)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("set by the user in Acta"));

    let (ok, out, err) = cli(&data, &repo, &["folders", "commit", "push", "--as-me"], "");
    assert!(ok, "{out}{err}");
    assert!(
        out.contains("commit and push automatically, in your name, not on main"),
        "{out}"
    );
    let local: Value = serde_json::from_str(
        &std::fs::read_to_string(repo.join(".claude/settings.local.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(local["autoMemoryEnabled"], json!(false));
    assert_eq!(local["permissions"]["allow"][2], "Bash(git push)");
    assert_eq!(local["env"]["GIT_TERMINAL_PROMPT"], "0");
    assert_eq!(local["attribution"]["sessionUrl"], json!(false));

    let hook = |dir: &Path, command: &str| {
        let input = json!({
            "session_id": "s1", "cwd": dir, "hook_event_name": "PreToolUse",
            "tool_name": "Bash", "tool_input": {"command": command}
        })
        .to_string();
        let (ok, out, err) = cli(&data, dir, &["hook", "pre-git"], &input);
        assert!(ok && err.is_empty(), "{err}");
        assert!(chars(out.trim()) <= 200, "{out}");
        out.trim().to_owned()
    };
    let reason = |out: &str| {
        let v: Value = serde_json::from_str(out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
        v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert!(reason(&hook(&repo, "git commit --no-verify -m x")).contains("--no-verify"));
    assert!(reason(&hook(&repo, "git push origin feature")).contains("exactly `git push`"));
    assert!(reason(&hook(&repo, "git push --force-with-lease")).contains("forced"));
    let heredoc = "git add a && git commit -m \"$(cat <<'EOF'\nNo --no-verify here\nEOF\n)\"";
    assert_eq!(hook(&repo, heredoc), "");
    assert_eq!(hook(&repo, "git push"), "");
    // Forms that hide the call are read through; an alias is resolved.
    assert!(reason(&hook(&repo, "{ git commit -n -m x; }")).contains("--no-verify"));
    assert!(reason(&hook(&repo, "bash -c 'git push --force'")).contains("forced"));
    assert!(reason(&hook(&repo, "git -c core.hooksPath=/x commit -m x")).contains("GIT_CONFIG"));
    git(&repo, &["config", "alias.ci", "commit --no-verify"]);
    assert!(reason(&hook(&repo, "git ci -m x")).contains("--no-verify"));
    assert_eq!(hook(&repo, "git status && git log -1"), "");
    // Judged where it acts: a manual folder next door, variables.
    let manual = tmp.path().join("manual");
    std::fs::create_dir_all(&manual).unwrap();
    git(&manual, &["init", "-q", "-b", "main"]);
    let (ok, out, err) = cli(&data, &manual, &["link", "--add", "--keep-auto-memory"], "");
    assert!(ok, "{out}{err}");
    assert!(
        reason(&hook(&repo, "git -C ../manual commit -m x"))
            .contains("commits in manual are manual")
    );
    assert!(reason(&hook(&repo, "cd ../manual && git commit -m x")).contains("are manual"));
    assert!(
        reason(&hook(&repo, "cd \"$HOME\" && git commit -m x")).contains("run git in this folder")
    );
    assert!(reason(&hook(&repo, "HUSKY=0 git commit -m x")).contains("environment variables"));
    git(&repo, &["switch", "-q", "main"]);
    assert!(reason(&hook(&repo, "git commit -m x")).contains("on main"));
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["folders", "commit", "push", "--on-default"],
        "",
    );
    assert!(ok && out.contains("also on main"), "{out}{err}");
    assert_eq!(hook(&repo, "git commit -m x"), "");
    // An unlinked folder: silent.
    assert_eq!(hook(&other, "git commit --no-verify -m x"), "");

    // Manual: nothing denied, Acta's commit entries out, auto memory kept.
    let (ok, out, err) = cli(&data, &repo, &["folders", "commit", "manual"], "");
    assert!(ok, "{out}{err}");
    assert_eq!(hook(&repo, "git commit --no-verify -m x"), "");
    let text = std::fs::read_to_string(repo.join(".claude/settings.local.json")).unwrap();
    assert_eq!(text, "{\n  \"autoMemoryEnabled\": false\n}\n");

    // Unlink takes out everything Acta wrote.
    let (ok, _, _) = cli(&data, &repo, &["folders", "commit", "commit"], "");
    assert!(ok);
    let (ok, out, err) = cli(&data, &repo, &["link", "--remove"], "");
    assert!(ok, "{out}{err}");
    assert!(!repo.join(".claude").exists());
}

/// ADR 0011 addendum 2026-11-23: one folder, Manual from one topic and
/// commit and push from another. Sessions Acta starts carry their page in
/// their launch token; `acta hook pre-git` and the MCP briefing follow that
/// page, a session started outside Acta keeps the folder's setting, and a
/// session cannot claim another page's mode.
#[test]
fn commit_mode_per_page_through_the_hook_and_mcp() {
    use acta_core::claude_settings::page::{self, PageCommit};
    use acta_core::keys::{KEY_FILE, KeyFile, KeyStores};
    use acta_core::model::{Author, CommitMode, Owner, Status};
    const T_A: &str = "0f2e4a10-1c2d-4f00-8a00-00000000000a";
    const T_B: &str = "0f2e4a10-1c2d-4f00-8a00-00000000000b";
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
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
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    let (ok, out, err) = cli(&data, &repo, &["folders", "commit", "commit"], "");
    assert!(ok, "{out}{err}");

    // Two topics on the project's folder, and two sessions as Acta's
    // terminal starts them (the app's part, done here on the store).
    let (a, b, file_a, file_b) = {
        let keys = KeyStores(vec![std::sync::Arc::new(KeyFile::new(data.join(KEY_FILE)))]);
        let mut store = acta_core::Store::open(&acta_core::StoreConfig {
            data_dir: data.clone(),
            keys,
        })
        .unwrap();
        let project = store.resolve_folder(&repo).unwrap().unwrap().project;
        let folder = store.folders(Owner::Project(project)).unwrap()[0].id;
        let mut topic = |title: &str| {
            store
                .create_topic(
                    acta_core::store::NewTopic {
                        project,
                        epic: None,
                        title: title.into(),
                        description: String::new(),
                        status: Status::Planned,
                    },
                    Author::Human,
                )
                .unwrap()
        };
        let (a, b) = (topic("Embed the terminal"), topic("Release notes"));
        let mode = |mode| {
            Some(PageCommit {
                mode,
                as_user: None,
                on_default: false,
            })
        };
        let h = Author::Human;
        let s = &mut store;
        page::set_page_commit(s, Owner::Topic(a), folder, mode(CommitMode::Manual), h).unwrap();
        page::set_page_commit(s, Owner::Topic(b), folder, mode(CommitMode::Push), h).unwrap();
        let fa = page::begin_session(s, T_A, Owner::Topic(a), &repo, None).unwrap();
        let fb = page::begin_session(s, T_B, Owner::Topic(b), &repo, None).unwrap();
        (a, b, fa.unwrap(), fb.unwrap())
    };
    let doc =
        |p: &Path| -> Value { serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap() };
    assert_eq!(doc(&file_b)["permissions"]["allow"][2], "Bash(git push)");
    assert!(
        doc(&file_a)["permissions"]["ask"]
            .to_string()
            .contains("git commit")
    );
    // The folder's own file never gets B's push rule.
    let local = std::fs::read_to_string(repo.join(".claude/settings.local.json")).unwrap();
    assert!(!local.contains("\"Bash(git push)\""), "{local}");

    // `token`: the session's ACTA_LAUNCH; `topic`: what it says it works on.
    let hook = |token: Option<&str>, topic: Option<&str>, command: &str| {
        let input = json!({
            "session_id": "s1", "cwd": repo, "hook_event_name": "PreToolUse",
            "tool_name": "Bash", "tool_input": {"command": command}
        })
        .to_string();
        let mut cmd = base_cmd(&data);
        cmd.args(["hook", "pre-git"])
            .current_dir(&repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(t) = token {
            cmd.env("ACTA_LAUNCH", t);
        }
        if let Some(t) = topic {
            cmd.env("ACTA_TOPIC", t);
        }
        let mut child = cmd.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        assert!(out.status.success() && chars(&text) <= 200, "{text}");
        if text.is_empty() {
            return "none".to_owned();
        }
        let v: Value = serde_json::from_str(&text).unwrap();
        v["hookSpecificOutput"]["permissionDecision"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (ta, tb) = (a.to_string(), b.to_string());
    let (ta, tb) = (Some(ta.as_str()), Some(tb.as_str()));
    // B pushes; only a plain push.
    assert_eq!(hook(Some(T_B), tb, "git commit -m x && git push"), "none");
    assert_eq!(hook(Some(T_B), tb, "git push origin feature"), "deny");
    // A: the folder file allows commits, the page does not: the user
    // confirms.
    assert_eq!(hook(Some(T_A), ta, "git commit -m x"), "ask");
    // A claiming B (its variables say B): still A's page.
    assert_eq!(hook(Some(T_A), tb, "git commit -m x && git push"), "ask");
    // A forged token: the strictest.
    let forged = "0f2e4a10-1c2d-4f00-8a00-0000000000ff";
    assert_eq!(hook(Some(forged), tb, "git push"), "ask");
    assert_eq!(hook(Some(forged), tb, "git commit -n -m x"), "deny");
    // Outside Acta: the folder's setting (commit), even with B's topic set.
    assert_eq!(hook(None, None, "git commit -m x"), "none");
    assert_eq!(hook(None, tb, "git commit -m x"), "none");
    assert_eq!(hook(None, None, "git commit -n -m x"), "deny");

    // The MCP briefing of B's session says commit and push; A's says only
    // when the user asks.
    for (token, topic, want) in [
        (T_B, b.to_string(), "commit and push"),
        (T_A, a.to_string(), "only when the user asks"),
    ] {
        let mut m = Mcp::start_with(
            &data,
            &repo,
            false,
            &[("ACTA_LAUNCH", token), ("ACTA_TOPIC", &topic)],
        );
        let (_, hook) = m.call("hook", json!({"event": "compact", "session": "e2e-pc"}));
        let v: Value = serde_json::from_str(&hook).unwrap();
        let ctx = v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(ctx.contains(want), "{ctx}");
        drop(m);
    }
    // `acta folders list` names the page's choice.
    let (ok, out, err) = cli(&data, &repo, &["folders", "list", &b.to_string()], "");
    assert!(
        ok && out.contains("commit and push automatically"),
        "{out}{err}"
    );
}

/// ADR 0011: `acta folders list|active`, and the session-start briefing's
/// folders and commit lines, driven through the binary.
#[test]
fn folders_list_active_and_the_briefing_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    let docs = tmp.path().join("docs");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&docs).unwrap();
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
    for (dir, args) in [(&repo, vec!["link"]), (&docs, vec!["link", "--add"])] {
        let (ok, out, err) = cli(&data, dir, &args, "");
        assert!(ok, "{out}{err}");
    }
    let (ok, out, err) = cli(&data, &repo, &["folders", "commit", "commit"], "");
    assert!(ok, "{out}{err}");

    let briefing = || {
        let input = json!({
            "session_id": "s1", "cwd": repo, "hook_event_name": "SessionStart",
            "source": "startup"
        })
        .to_string();
        let (ok, out, err) = cli(&data, &repo, &["hook", "session-start"], &input);
        assert!(ok && err.is_empty(), "{err}");
        let v: Value = serde_json::from_str(out.trim()).unwrap();
        let text = v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(chars(&text) <= 1_200, "{text}");
        text
    };
    let b = briefing();
    let commit = b.lines().nth(1).unwrap();
    assert_eq!(
        commit,
        "Commits here: commit each finished change without being asked (the user's standing request); not on main."
    );
    assert!(
        b.contains("\nFolders: work in repo (active). Also attached: docs.\n"),
        "{b}"
    );

    let (ok, out, err) = cli(&data, &repo, &["folders", "list"], "");
    assert!(ok, "{err}");
    assert!(out.starts_with("P-1 sees 2 folders (* active)."), "{out}");
    assert!(
        out.contains("* 1 repo · git feature · commit automatically · "),
        "{out}"
    );
    assert!(out.contains("  2 docs · "), "{out}");
    assert!(chars(&out) <= 4_000);

    // Claude Code cannot switch the active folder, nor read titles as JSON.
    for args in [vec!["folders", "active", "2"], vec!["find", "--json"]] {
        let out = base_cmd(&data)
            .args(&args)
            .env("CLAUDECODE", "1")
            .current_dir(&repo)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!out.status.success(), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("set by the user in Acta") || err.contains("find --json is for scripts"),
            "{err}"
        );
    }
    let (ok, out, err) = cli(&data, &repo, &["folders", "active", "docs"], "");
    assert!(ok, "{err}");
    assert!(out.starts_with("ok P-1 active folder: docs."), "{out}");
    let (ok, out, _) = cli(&data, &repo, &["folders", "active"], "");
    assert!(
        ok && out.starts_with("P-1 active folder:\n") && out.contains("2 docs · "),
        "{out}"
    );
    let (ok, out, _) = cli(&data, &repo, &["folders", "active", "nope"], "");
    assert!(!ok, "{out}");
    let b = briefing();
    assert!(
        b.contains(
            "Folders: work in repo (the page's active folder is docs). Also attached: docs."
        ),
        "{b}"
    );
    let (ok, out, err) = cli(&data, &repo, &["folders", "active", "default"], "");
    assert!(
        ok && out.contains("active folder: repo (default)"),
        "{out}{err}"
    );
}

/// Questions, contacts and Report to (data.md requests 18, 19 and 22) as
/// Claude Code drives them: a name that is not a contact is refused and
/// creates nothing, a contact is created by name, asked, found by person,
/// the answer logged as a decision and read back, a Report to set and
/// marked; the profile in the briefing; then the CLI, with every name and
/// text on stdin only.
#[test]
fn questions_contacts_and_report_to_over_stdio() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(cli(&data, &repo, &["link"], "").0);
    let (ok, out, err) = cli(&data, &repo, &["create", "epic"], "Export");
    assert!(ok && out.starts_with("ok E-1"), "{out}{err}");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["create", "topic", "--epic", "E-1"],
        "Export format",
    );
    assert!(ok && out.starts_with("ok T-1"), "{out}{err}");
    // A contact from the CLI, its name on stdin; a note naming it.
    let (ok, out, err) = cli(&data, &repo, &["create", "contact"], "Hélène");
    assert!(
        ok && out.trim() == "ok created C-1 Hélène (P-1)",
        "{out}{err}"
    );
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["create", "note", "--json"],
        r#"{"title": "Tell Hélène when the format is settled", "report_to": "helene"}"#,
    );
    assert!(ok && out.trim() == "ok N-1", "{out}{err}");

    let mut m = Mcp::start(&data, &repo, false);
    let question = json!({"kind": "question", "title": "Can we keep the v1 export format until March?",
                          "asked_to": "Marie", "owner": "T-1", "due": "2000-01-03"});
    // An unknown name is refused and creates nothing.
    let (err, text) = m.call("create", json!({"items": [question.clone()]}));
    assert!(err, "{text}");
    assert!(
        text.starts_with("item 1: no contact \"Marie\" here"),
        "{text}"
    );
    let (_, contacts) = m.call("find", json!({"kind": "contact"}));
    assert!(!contacts.contains("Marie"), "{contacts}");
    // The user confirms someone new: the contact, then the question.
    let mut with_new = question.clone();
    with_new["asked_to"] = json!("@1");
    let (err, ack) = m.call(
        "create",
        json!({"items": [{"kind": "contact", "title": "Marie"}, with_new]}),
    );
    assert!(
        !err && ack.starts_with(
            "ok created C-2 Marie (P-1) · Q-1 (T-1) · asked C-2 (created in this call) · by 2000-01-03 ("
        ),
        "{ack}"
    );
    assert!(chars(&ack) <= 320);
    let (err, ack) = m.call(
        "create",
        json!({"items": [{"kind": "question", "title": "Budget for the disks?", "owner": "E-1"}]}),
    );
    assert!(!err && ack == "ok Q-2 (E-1)", "{ack}");

    // Found by person, by key: case and accents do not matter.
    let (err, found) = m.call("find", json!({"person": "MARIE"}));
    assert!(!err && chars(&found) <= 2_000, "{found}");
    assert!(found.contains("\nC-2 Marie · P-1\n"), "{found}");
    assert!(
        found
            .lines()
            .any(|l| l.starts_with("Q-1 open T-1 · Marie · overdue ")),
        "{found}"
    );
    let (_, near) = m.call("find", json!({"person": "mari"}));
    assert!(
        near.contains("no contact \"mari\" here · near: C-2 Marie"),
        "{near}"
    );
    let (_, found) = m.call("find", json!({"person": "C-1", "kind": "note"}));
    assert!(found.contains("N-1 note  Tell Hélène"), "{found}");
    let (_, topic) = m.call("brief", json!({"id": "T-1"}));
    assert!(topic.contains("Questions 1 open, 1 overdue:"), "{topic}");
    assert!(topic.contains("\nQ-1 Marie · overdue "), "{topic}");
    let (_, project) = m.call("brief", json!({"id": "P-1"}));
    assert!(project.contains("answers late Q-1 "), "{project}");

    // Answered with a logged decision, read back.
    let (err, text) = m.call(
        "update",
        json!({"id": "Q-2", "status": "answered", "log_decision": true}),
    );
    assert!(err, "{text}");
    assert_eq!(
        text,
        "log_decision needs a question on a topic; Q-2 is on E-1"
    );
    let (err, ack) = m.call(
        "update",
        json!({"id": "Q-1", "status": "answered", "answer": "Yes, until 31 March.", "log_decision": true}),
    );
    assert!(
        !err && ack == "ok Q-1 answered · logged T-1#1 decision",
        "{ack}"
    );
    let (_, entry) = m.call("entries", json!({"id": "T-1#1"}));
    assert!(
        entry.contains("T-1#1 ") && entry.contains(" decision "),
        "{entry}"
    );
    assert!(
        entry.contains("> Can we keep the v1 export format until March?")
            && entry.contains("Yes, until 31 March."),
        "{entry}"
    );

    // Report to: set, done not reported, marked; a contact moved.
    let (err, text) = m.call("update", json!({"id": "P-1", "report_to": "Marie"}));
    assert!(err && text == "projects have no report_to", "{text}");
    let (err, ack) = m.call(
        "update",
        json!({"id": "T-1", "report_to": "Marie", "status": "done"}),
    );
    assert!(
        !err && ack == "ok T-1 updated: status · report to C-2 (existing contact)",
        "{ack}"
    );
    let (_, found) = m.call("find", json!({"person": "C-2"}));
    assert!(found.contains("· report Marie (not yet)"), "{found}");
    let (err, ack) = m.call("update", json!({"id": "T-1", "reported": true}));
    assert!(!err && ack == "ok T-1 · reported to C-2", "{ack}");
    let (err, text) = m.call("update", json!({"id": "C-1", "level": "E-1"}));
    assert!(err, "{text}");
    assert_eq!(
        text,
        "C-1 is named outside E-1: N-1 (report_to); change those first"
    );
    let (err, ack) = m.call("update", json!({"id": "C-2", "level": "all"}));
    assert!(!err && ack == "ok C-2 · level all", "{ack}");
    let (_, topic) = m.call("brief", json!({"id": "T-1"}));
    assert!(topic.contains("· reported to Marie"), "{topic}");
    drop(m);

    // The profile reaches the briefing (set here as the app would).
    {
        // The key file the debug build keeps in the dev data directory:
        // never the keychain.
        use acta_core::keys::{KEY_FILE, KeyFile, KeyStores};
        let keys = KeyStores(vec![std::sync::Arc::new(KeyFile::new(data.join(KEY_FILE)))]);
        let mut store = acta_core::Store::open(&acta_core::StoreConfig {
            data_dir: data.clone(),
            keys,
        })
        .unwrap();
        store
            .set_profile(Some("Alexandre"), Some("Product designer"))
            .unwrap();
    }
    let mut m = Mcp::start(&data, &repo, false);
    let (_, hook) = m.call("hook", json!({"event": "compact", "session": "e2e-q"}));
    let v: Value = serde_json::from_str(&hook).unwrap();
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(chars(ctx) <= 1_200, "{ctx}");
    assert!(
        ctx.contains("\nUser: Alexandre, Product designer\n"),
        "{ctx}"
    );
    drop(m);

    // The CLI: names and text on stdin only.
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["create", "contact", "--level", "E-1"],
        "Legal team",
    );
    assert!(
        ok && out.trim() == "ok created C-3 Legal team (E-1)",
        "{out}{err}"
    );
    let (ok, out, err) = cli(
        &data,
        &repo,
        &[
            "create",
            "question",
            "--owner",
            "T-1",
            "--due",
            "2099-12-04",
            "--json",
        ],
        r#"{"title": "Which disk vendor?", "asked_to": "legal TEAM"}"#,
    );
    assert!(
        ok && out.starts_with("ok Q-3 (T-1) · asked C-3 (existing contact) · by 2099-12-04"),
        "{out}{err}"
    );
    let (ok, out, err) = cli(&data, &repo, &["create", "question"], "Who pays?");
    assert!(!ok && err.contains("pass owner (no focus)"), "{out}{err}");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["create", "question", "--owner", "P-1"],
        "Who pays?",
    );
    assert!(ok && out.trim() == "ok Q-4 (P-1)", "{out}{err}");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &[
            "update",
            "Q-3",
            "--status",
            "answered",
            "--log-decision",
            "--field",
            "answer",
        ],
        "The cheaper one.",
    );
    assert!(
        ok && out.trim() == "ok Q-3 answered · logged T-1#2 decision",
        "{out}{err}"
    );
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["update", "E-1", "--field", "report_to"],
        "Legal team",
    );
    assert!(
        ok && out.trim() == "ok E-1 · report to C-3 (existing contact)",
        "{out}{err}"
    );
    let (ok, out, err) = cli(&data, &repo, &["update", "T-1", "--reported", "false"], "");
    assert!(ok && out.trim() == "ok T-1 · not reported", "{out}{err}");
    let (ok, out, err) = cli(&data, &repo, &["update", "C-3", "--level", "P-1"], "");
    assert!(ok && out.trim() == "ok C-3 · level P-1", "{out}{err}");
    let (ok, out, err) = cli(&data, &repo, &["find", "--person", "-"], "legal TEAM");
    assert!(ok && out.contains("E-1 epic  Export"), "{out}{err}");
    let (ok, out, err) = cli(&data, &repo, &["find", "--person", "Marie"], "");
    assert!(!ok && err.contains("--person -"), "{out}{err}");
    let (ok, out, err) = cli(&data, &repo, &["find", "--kind", "contact"], "");
    assert!(ok && out.contains("C-3 Legal team · P-1"), "{out}{err}");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["find", "--kind", "question", "--status", "all", "--json"],
        "",
    );
    assert!(ok, "{out}{err}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    let hits = v["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 4, "{out}");
    let q1 = hits.iter().find(|h| h["id"] == "Q-1").unwrap();
    assert_eq!(q1["status"], "answered");
    assert_eq!(q1["asked_to"], "Marie");
    assert_eq!(q1["asked_contact"], "C-2");
    assert_eq!(q1["answer"], "Yes, until 31 March.");
    assert_eq!(q1["owner"], "T-1");
    let (ok, out, err) = cli(
        &data,
        &repo,
        &["update", "Q-4", "--json", "--status", "dropped"],
        "",
    );
    assert!(ok, "{out}{err}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["status"], "dropped");
    assert_eq!(v["text"], "Who pays?");
    // Under Claude Code, --json carries no stored text.
    let mut c = base_cmd(&data);
    let out = c
        .args(["update", "Q-4", "--json", "--status", "open"])
        .env("CLAUDECODE", "1")
        .current_dir(&repo)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "open");
    assert!(v.get("text").is_none(), "{v}");
}

/// Attachments over stdio (ADR 0012, 10): names and sizes in list lines,
/// the `attachment` tool returning text, or one image block after its
/// text line, and `acta attachment` on the CLI. Files reach Acta from the
/// human (the app); here the test adds them through the store, with the
/// key file the dev data directory uses.
#[test]
fn attachments_over_stdio() {
    use acta_core::keys::{KEY_FILE, KeyFile, KeyStores};
    use acta_core::model::{AttachOwner, Author, NoteId};
    use acta_core::store::{AttachSource, Store, StoreConfig};
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    let (ok, out, _) = cli(
        &data,
        &repo,
        &["create", "note"],
        "Screenshot from Alexandre",
    );
    assert!(ok && out.starts_with("ok N-1"), "{out}");
    {
        let config = StoreConfig {
            data_dir: data.clone(),
            keys: KeyStores(vec![std::sync::Arc::new(KeyFile::new(data.join(KEY_FILE)))]),
        };
        let mut store = Store::open(&config).unwrap();
        let img = image::RgbImage::from_fn(2000, 1000, |x, y| image::Rgb([x as u8, y as u8, 40]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        for (name, bytes) in [
            ("screenshot.png", png),
            ("notes.md", b"# Plan\nShip attachments.\n".to_vec()),
            ("spec.pdf", one_page_pdf("Storage spec for Alexandre")),
            ("broken.pdf", b"%PDF-1.4 not really a PDF".to_vec()),
        ] {
            store
                .attach(
                    AttachOwner::Note(NoteId(1)),
                    name,
                    AttachSource::Bytes(bytes),
                    Author::Human,
                    &mut |_, _| {},
                )
                .unwrap();
        }
    }
    let mut m = Mcp::start(&data, &repo, false);
    let (err, text) = m.call("find", json!({"kind": "note"}));
    assert!(!err, "{text}");
    assert!(
        text.contains(" · files: A-1 screenshot.png ") && text.contains(", A-2 notes.md 25B"),
        "{text}"
    );
    let (err, text) = m.call("attachment", json!({"id": "A-2"}));
    assert!(!err && text.contains("Ship attachments."), "{text}");
    // The image: a text line, then one image block, nothing else.
    let r = m.request(
        "tools/call",
        json!({"name": "attachment", "arguments": {"id": "A-1"}}),
    );
    let content = r["result"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2, "{r}");
    let line = content[0]["text"].as_str().unwrap();
    assert!(line.starts_with("A-1 · image · 2000x1000 · "), "{line}");
    assert!(line.contains("sent 1092x546"), "{line}");
    assert_eq!(content[1]["type"], json!("image"));
    assert_eq!(content[1]["mimeType"], json!("image/png"));
    let b64 = content[1]["data"].as_str().unwrap();
    assert!(b64.len() < 2_000_000, "{}", b64.len());
    // PDFs are parsed in an `acta pdf-text` child.
    let (err, text) = m.call("attachment", json!({"id": "A-3"}));
    assert!(
        !err && text.contains("Storage spec for Alexandre"),
        "{text}"
    );
    let (err, text) = m.call("attachment", json!({"id": "A-4"}));
    assert!(!err && text.contains("could not read this PDF"), "{text}");
    let (err, text) = m.call("attachment", json!({"id": "A-1", "lines": "1-2"}));
    assert!(
        err && text.contains("lines applies to text files"),
        "{text}"
    );
    m.close();
    let (ok, out, _) = cli(&data, &repo, &["attachment", "A-2", "--lines", "2-2"], "");
    assert!(
        ok && out.contains("Ship attachments.") && !out.contains("# Plan"),
        "{out}"
    );
    // The parser command is internal: refused without the parent's marker,
    // and for a file of another project.
    let (ok, out, err) = cli(&data, &repo, &["pdf-text", "A-3", "--project", "P-1"], "");
    assert!(
        !ok && out.is_empty() && err.contains("internal"),
        "{out}{err}"
    );
    let marker = "0123456789abcdef0123456789abcdef";
    let child = |project: &str| {
        let mut c = base_cmd(&data);
        let mut child = c
            .args(["pdf-text", "A-3", "--project", project])
            .env("ACTA_PDF_CHILD", marker)
            .current_dir(&repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{marker}\n").as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let out = child("P-1");
    let json: Value = serde_json::from_slice(&out.stdout).expect("JSON only");
    assert_eq!(json["count"], json!(1));
    let out = child("P-2");
    assert!(!out.status.success() && out.stdout.is_empty());
    let (ok, out, _) = cli(&data, &repo, &["attachment", "A-3", "--pages", "1"], "");
    assert!(ok && out.contains("Storage spec for Alexandre"), "{out}");
    let (ok, out, _) = cli(&data, &repo, &["attachment", "A-1"], "");
    assert!(
        ok && out.contains("sent 1092x546") && out.contains("MCP"),
        "{out}"
    );
}

/// A one-page PDF with one line of Helvetica text and a correct xref.
fn one_page_pdf(text: &str) -> Vec<u8> {
    let stream = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [4 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents 5 0 R >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
    }
    let xref = out.len();
    out.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objs.len() + 1
    ));
    for off in offsets {
        out.push_str(&format!("{off:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objs.len() + 1
    ));
    out.into_bytes()
}

/// A git repository on `main` with one commit holding `src/kb.rs`.
fn repo_with_file(repo: &Path) {
    std::fs::create_dir_all(repo.join("src")).unwrap();
    git(repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("src/kb.rs"), "fn one() {}\n").unwrap();
    git(repo, &["add", "src/kb.rs"]);
    commit(repo, "First");
}

fn commit(repo: &Path, msg: &str) {
    git(
        repo,
        &[
            "-c",
            "user.name=Alexandre",
            "-c",
            "user.email=alexandre@example.com",
            "commit",
            "-q",
            "-m",
            msg,
        ],
    );
}

/// ADR 0015: create, conflict, supersede, keep both, verify and retire
/// over stdio, each answer within its budget; a file source flagged after
/// a commit changes it; the CLI's verify, check and quality.
#[test]
fn memory_quality_over_stdio() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    repo_with_file(&repo);
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    let (ok, _, _) = cli(&data, &repo, &["create", "topic"], "Encrypted storage");
    assert!(ok);
    let mut m = Mcp::start_with(&data, &repo, false, &[("ACTA_TOPIC", "T-1")]);

    let write = |m: &mut Mcp, args: Value| m.call("kb_write", args);
    let (err, text) = write(
        &mut m,
        json!({"kind": "memory", "level": "project", "title": "Tests run in parallel",
               "summary": "cargo test runs the store tests concurrently",
               "text": "The store tests are safe to run in parallel.",
               "refs": "src/kb.rs, T-1"}),
    );
    assert!(!err && text.starts_with("ok K-1"), "{text}");
    let conflicting = json!({"kind": "memory", "title": "Store tests run serially",
        "summary": "Never run the store tests in parallel: they share a key store",
        "text": "Do not run the store tests in parallel."});
    let (err, text) = write(&mut m, conflicting.clone());
    assert!(
        err && text.starts_with("not saved: possible conflict with K-1"),
        "{text}"
    );
    assert!(chars(&text) <= 600, "{}", chars(&text));
    let mut replacing = conflicting.clone();
    replacing["supersedes"] = json!("K-1");
    let (err, text) = write(&mut m, replacing);
    assert!(!err && text.contains("supersedes K-1"), "{text}");
    assert!(chars(&text) <= 160);

    // Keep both: the identical call, repeated.
    let dup = json!({"kind": "memory", "title": "Key in the OS keychain",
        "summary": "The SQLCipher key is stored in the OS keychain",
        "text": "On first run the key goes to the keychain, entry acta.db."});
    let (err, _) = write(&mut m, dup.clone());
    assert!(!err);
    let mut twin = dup.clone();
    twin["title"] = json!("The OS keychain holds the key");
    let (err, text) = write(&mut m, twin.clone());
    assert!(err && text.contains("possible duplicate of K-3"), "{text}");
    let (err, text) = write(&mut m, twin);
    assert!(!err && text.ends_with("kept both"), "{text}");

    // Verify and retire.
    let (err, text) = write(&mut m, json!({"id": "K-3", "rev": 1, "op": "verify"}));
    assert_eq!((err, text.as_str()), (false, "ok K-3 verified (rev 2)"));
    let (err, text) = write(
        &mut m,
        json!({"id": "K-4", "rev": 1, "op": "retire", "text": "Merged into K-3 by hand"}),
    );
    assert_eq!((err, text.as_str()), (false, "ok K-4 retired (rev 2)"));

    // A file source: a commit that changes it flags the memory.
    let (_, text) = write(
        &mut m,
        json!({"kind": "memory", "title": "Where kb lives",
               "summary": "The knowledge store is one module", "text": "See the module.",
               "refs": "src/kb.rs"}),
    );
    assert!(text.starts_with("ok K-5"), "{text}");
    let (_, list) = m.call("kb", json!({}));
    assert!(!list.contains("?file changed"), "{list}");
    std::fs::write(repo.join("src/kb.rs"), "fn two() {}\n").unwrap();
    git(&repo, &["add", "src/kb.rs"]);
    commit(&repo, "Second");
    let (_, list) = m.call("kb", json!({}));
    assert!(chars(&list) <= 1_800);
    let line = list.lines().find(|l| l.starts_with("K-5")).unwrap();
    assert!(line.contains("?file changed"), "{list}");
    let (_, read) = m.call("kb_read", json!({"id": "K-5"}));
    assert!(read.contains("?file changed src/kb.rs"), "{read}");
    // Verified again: the new blob is recorded, the flag goes.
    let (err, text) = write(&mut m, json!({"id": "K-5", "rev": 1, "op": "verify"}));
    assert!(!err, "{text}");
    let (_, list) = m.call("kb", json!({}));
    let line = list.lines().find(|l| l.starts_with("K-5")).unwrap();
    assert!(!line.contains('?'), "{line}");
    // Deleted: gone.
    std::fs::remove_file(repo.join("src/kb.rs")).unwrap();
    git(&repo, &["add", "-A"]);
    commit(&repo, "Third");
    let (_, list) = m.call("kb", json!({}));
    let line = list.lines().find(|l| l.starts_with("K-5")).unwrap();
    assert!(line.contains("?file gone"), "{list}");
    // Fixed, naming its sources again: they replace the old ones.
    let (err, text) = write(
        &mut m,
        json!({"id": "K-5", "rev": 2, "op": "replace_body", "text": "The module was removed.", "refs": "T-1"}),
    );
    assert!(!err, "{text}");
    let (_, list) = m.call("kb", json!({}));
    let line = list.lines().find(|l| l.starts_with("K-5")).unwrap();
    assert!(!line.contains('?'), "{line}");
    m.close();

    // The CLI: verify (the revision read), a stale retire, check, quality.
    let (ok, out, err) = cli(&data, &repo, &["kb", "verify", "K-5", "--rev", "3"], "");
    assert!(
        ok && out.starts_with("ok K-5 verified (rev 4)"),
        "{out}{err}"
    );
    let (ok, out, err) = cli(&data, &repo, &["kb", "retire", "K-5", "--rev", "1"], "gone");
    assert!(
        !ok && format!("{out}{err}").contains("changed since rev 1"),
        "{out}{err}"
    );
    let (ok, out, err) = cli(&data, &repo, &["kb", "check"], "");
    assert!(ok && out.starts_with("Checked "), "{out}{err}");
    let (ok, out, err) = cli(&data, &repo, &["kb", "quality", "--days", "30"], "");
    assert!(ok, "{out}{err}");
    assert_eq!(out.trim().lines().count(), 7, "{out}");
    assert!(out.contains("4 Warning noise: 1 of 2"), "{out}");
    assert!(chars(&out) <= 2_000);
}

/// ADR 0015, 6: two server processes edit one memory and one document.
/// The second writer is told who changed what, with the current text; a
/// section edit whose section is untouched is merged; "also open" names
/// the other session.
#[test]
fn two_servers_editing_one_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    for t in ["Storage", "Sync"] {
        let (ok, _, _) = cli(&data, &repo, &["create", "topic"], t);
        assert!(ok);
    }
    let mut a = Mcp::start_with(
        &data,
        &repo,
        false,
        &[
            ("ACTA_TOPIC", "T-1"),
            ("CLAUDE_CODE_SESSION_ID", "session-a"),
        ],
    );
    let mut b = Mcp::start_with(
        &data,
        &repo,
        false,
        &[
            ("ACTA_TOPIC", "T-2"),
            ("CLAUDE_CODE_SESSION_ID", "session-b"),
        ],
    );
    let (err, text) = a.call(
        "kb_write",
        json!({"kind": "memory", "level": "project", "title": "Release script",
               "summary": "How a release is cut", "text": "Run scripts/release.ps1."}),
    );
    assert!(!err, "{text}");
    let (err, text) = a.call(
        "kb_write",
        json!({"kind": "document", "level": "project", "title": "Architecture",
               "summary": "Crates and processes", "text": "# One\na\n# Two\nb\n# Three\nc\n"}),
    );
    assert!(!err, "{text}");
    // Both read rev 1.
    for m in [&mut a, &mut b] {
        for id in ["K-1", "K-2"] {
            let (err, _) = m.call("kb_read", json!({"id": id}));
            assert!(!err);
        }
    }
    // A writes first.
    let (err, _) = a.call(
        "kb_write",
        json!({"id": "K-1", "rev": 1, "op": "replace_body", "text": "Run scripts/release.ps1 -Sign:$false."}),
    );
    assert!(!err);
    let (err, _) = a.call(
        "kb_write",
        json!({"id": "K-2", "rev": 1, "op": "replace_section", "section": "1", "text": "a2"}),
    );
    assert!(!err);
    // B's memory edit is refused with the current text and who.
    let (err, text) = b.call(
        "kb_write",
        json!({"id": "K-1", "rev": 1, "op": "replace_body", "text": "Run the release job."}),
    );
    assert!(err, "{text}");
    assert!(
        text.starts_with(
            "not saved: K-1 changed since rev 1: rev 2 by another Claude Code session on T-1, "
        ),
        "{text}"
    );
    assert!(text.contains("-Sign:$false"), "{text}");
    assert!(chars(&text) <= 1_600);
    // A's touches reach its presence row: B is told A has it open.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let (_, read) = b.call("kb_read", json!({"id": "K-1"}));
        if read.contains("K-1 is also open in a session on T-1") {
            break;
        }
        assert!(Instant::now() < deadline, "no also-open line: {read}");
        std::thread::sleep(Duration::from_millis(300));
    }
    // B's section edit of an untouched section is merged.
    let (err, text) = b.call(
        "kb_write",
        json!({"id": "K-2", "rev": 1, "op": "replace_section", "section": "3", "text": "c2"}),
    );
    assert!(!err, "{text}");
    assert_eq!(text, "ok K-2 rev 3 · section replaced · merged over rev 2");
    let (_, read) = a.call("kb_read", json!({"id": "K-2"}));
    assert!(read.contains("a2") && read.contains("c2"), "{read}");
}

/// ADR 0015, 4: a branch memory shows on its branch only; on another
/// branch the list counts it and search marks it; merged, it is flagged.
#[test]
fn a_branch_memory_is_hidden_on_another_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let repo = tmp.path().join("repo");
    repo_with_file(&repo);
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    let (ok, _, _) = cli(&data, &repo, &["create", "topic"], "Schema work");
    assert!(ok);
    let mut m = Mcp::start_with(&data, &repo, false, &[("ACTA_TOPIC", "T-1")]);
    let branch_memory = json!({"kind": "memory", "level": "branch", "title": "Schema is at V7 here",
        "summary": "On this branch the schema is at V7", "text": "V7 adds kb_links."});
    let (err, text) = m.call("kb_write", branch_memory.clone());
    assert!(
        err && text == "level=branch needs a feature branch; use topic",
        "{text}"
    );
    git(&repo, &["switch", "-q", "-c", "feature/kb"]);
    std::fs::write(
        repo.join("src/schema.rs"),
        "// V7
",
    )
    .unwrap();
    git(&repo, &["add", "src/schema.rs"]);
    commit(&repo, "Schema 7");
    let (err, text) = m.call("kb_write", branch_memory);
    assert!(
        !err && text.starts_with("ok K-1 memory · branch on T-1"),
        "{text}"
    );
    let (_, list) = m.call("kb", json!({}));
    assert!(list.contains("K-1 "), "{list}");
    git(&repo, &["switch", "-q", "main"]);
    let (_, list) = m.call("kb", json!({}));
    assert!(
        !list.contains("K-1 ") && list.contains("· 1 on other branches"),
        "{list}"
    );
    let (_, found) = m.call("kb", json!({"query": "schema"}));
    assert!(found.contains("K-1 ?branch feature/kb"), "{found}");
    // Merged into main: flagged on its own branch.
    git(&repo, &["switch", "-q", "feature/kb"]);
    let (_, list) = m.call("kb", json!({}));
    assert!(!list.contains("?branch merged"), "{list}");
    git(&repo, &["switch", "-q", "main"]);
    git(&repo, &["merge", "-q", "--ff-only", "feature/kb"]);
    git(&repo, &["switch", "-q", "feature/kb"]);
    let (_, list) = m.call("kb", json!({}));
    assert!(list.contains("?branch merged"), "{list}");
}
