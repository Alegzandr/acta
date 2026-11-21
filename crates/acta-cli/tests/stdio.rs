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
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CONFIG_DIR");
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
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: i64,
    init: Value,
}

impl Mcp {
    fn start(data: &Path, project_dir: &Path, memory_key: bool) -> Mcp {
        let mut cmd = base_cmd(data);
        cmd.arg("mcp")
            .env("CLAUDE_PROJECT_DIR", project_dir)
            .env("CLAUDE_CODE_SESSION_ID", "e2e-session")
            .stdin(Stdio::piped())
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
            stdin,
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
        writeln!(self.stdin, "{v}").unwrap();
        self.stdin.flush().unwrap();
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
            "brief", "create", "entries", "find", "hook", "kb", "kb_read", "kb_write", "log",
            "update"
        ]
        .into_iter()
        .collect()
    );
    let list_size = init_again["result"].to_string().len();
    assert!(list_size < 7_000, "tools/list is {list_size} chars");
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
            Some("brief" | "entries" | "find" | "kb" | "kb_read" | "hook")
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
    assert!(
        brief.starts_with("T-1 Encrypted storage spike · active · E-1 Storage"),
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
    let start = Instant::now();
    let (ok, out, _) = cli(&data, &other, &["hook", "session-start"], &input(&other));
    let miss = start.elapsed();
    assert!(ok && out.is_empty(), "{out}");
    assert!(miss < Duration::from_secs(2), "{miss:?}");

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
    let (ok, out, err) = cli(&data, &repo, &["link"], "");
    assert!(ok, "{out}{err}");
    let exclude = std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap();
    assert!(
        exclude.contains("/.claude/settings.local.json"),
        "{exclude}"
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
    let file = tmp.path().join("out.acta.json");
    let (ok, out, err) = cli(&data, &repo, &["export", file.to_str().unwrap()], "");
    assert!(ok && out.contains("plaintext"), "{out}{err}");
    let (ok, out, _) = cli(
        &data,
        &repo,
        &["import", file.to_str().unwrap(), "--dry-run"],
        "",
    );
    assert!(ok && out.contains("0 created"), "{out}");
    let (ok, out, _) = cli(&data, &repo, &["snapshot", "list"], "");
    assert!(ok, "{out}");
}
