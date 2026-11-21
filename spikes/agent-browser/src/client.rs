//! Stands in for `acta mcp`: connects to the app's pipe, authenticates and
//! drives the agent web view through the operations the MCP tools would
//! call. Prints each reply's size against its budget and saves screenshots.

use crate::pipe::{self, Conn};
use crate::render::budget;
use base64::Engine as _;
use serde_json::{Value, json};
use std::path::Path;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

struct Session {
    c: Conn,
    failures: Vec<String>,
}

impl Session {
    fn call(&mut self, req: Value) -> Value {
        self.c.send(&req).expect("send");
        self.c.recv().expect("recv").expect("reply")
    }

    /// Runs one step, prints it, and records a failure if `check` is false.
    fn step(
        &mut self,
        label: &str,
        req: Value,
        max: usize,
        check: impl Fn(&Value) -> bool,
    ) -> Value {
        let r = self.call(req);
        let text = r["text"].as_str().unwrap_or("");
        let within = text.len() <= max;
        let passed = check(&r) && within;
        println!(
            "\n== {label} · {} · {} chars (budget {max}) · {} ms",
            if passed { "PASS" } else { "FAIL" },
            text.len(),
            r["ms"]
        );
        let shown: Vec<&str> = text.lines().take(40).collect();
        println!("{}", shown.join("\n"));
        if text.lines().count() > 40 {
            println!("... ({} lines in all)", text.lines().count());
        }
        if !passed {
            self.failures.push(label.to_string());
        }
        r
    }
}

/// The ref (`e<n>`) of the first snapshot line with this role and name.
fn find_ref(snapshot: &str, role: &str, name: &str) -> String {
    let needle = format!("{role} \"{name}");
    snapshot
        .lines()
        .find(|l| l.contains(&needle))
        .and_then(|l| l.split_whitespace().next())
        .filter(|r| r.starts_with('e'))
        .unwrap_or("e0")
        .to_string()
}

fn save(r: &Value, out: &Path, name: &str) -> bool {
    let Some(b64) = r["image"].as_str() else {
        return false;
    };
    match base64::engine::general_purpose::STANDARD.decode(b64) {
        Ok(bytes) if bytes.len() > 1000 => {
            let p = out.join(name);
            let _ = std::fs::write(&p, &bytes);
            println!("saved {}", p.display());
            true
        }
        _ => false,
    }
}

pub fn run(key: [u8; 32], base: &str, out: &Path, host_pid: u32) -> Result<(), String> {
    let sid = pipe::user_sid().map_err(|e| e.to_string())?;
    let name = pipe::pipe_name(&sid);
    // SAFETY: plain query.
    let fg_before = unsafe { GetForegroundWindow() };

    // 1. A client without the key is refused and learns nothing.
    let mut bad =
        Conn::new(pipe::connect(&name).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let wrong = crate::auth::derive(b"not the database key");
    match pipe::client_handshake(&mut bad, &wrong) {
        Err(e) => println!("== wrong key · PASS · refused: {e}"),
        Ok(()) => return Err("a client with the wrong key was accepted".into()),
    }
    drop(bad);

    // 2. The real client.
    let mut c =
        Conn::new(pipe::connect(&name).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    pipe::client_handshake(&mut c, &key)?;
    println!("== handshake · PASS · both sides proved the key");
    let mut s = Session {
        c,
        failures: vec![],
    };
    let ok = |r: &Value| r["ok"] == true;
    let refused =
        |r: &Value| r["ok"] == false && r["text"].as_str().unwrap_or("").starts_with("refused");

    s.step(
        "navigate form",
        json!({"op": "navigate", "url": format!("{base}/form.html")}),
        budget::ACK,
        |r| ok(r) && r["text"].as_str().unwrap_or("").contains("Spike form"),
    );
    let snap = s.step(
        "snapshot",
        json!({"op": "snapshot"}),
        budget::SNAPSHOT,
        |r| {
            let t = r["text"].as_str().unwrap_or("");
            ok(r) && t.contains("button \"Greet\"") && t.contains("(end web page 00000000]")
        },
    );
    let snap = snap["text"].as_str().unwrap_or("").to_string();
    let name_ref = find_ref(&snap, "textbox", "Name");
    let pw_ref = find_ref(&snap, "textbox", "Password");
    let greet = find_ref(&snap, "button", "Greet");
    s.step(
        "type name",
        json!({"op": "type", "ref": name_ref, "text": "Alexandre"}),
        budget::ACK,
        ok,
    );
    s.step(
        "click Greet",
        json!({"op": "click", "ref": greet}),
        budget::ACK,
        ok,
    );
    s.step(
        "snapshot after click (interactive filter would hide text, so all)",
        json!({"op": "snapshot"}),
        budget::SNAPSHOT,
        |r| {
            r["text"]
                .as_str()
                .unwrap_or("")
                .contains("Hello, Alexandre!")
        },
    );
    s.step("console", json!({"op": "console"}), budget::CONSOLE, |r| {
        r["text"]
            .as_str()
            .unwrap_or("")
            .contains("greeted Alexandre trusted=true")
    });
    s.step(
        "type into password field",
        json!({"op": "type", "ref": pw_ref, "text": "x"}),
        budget::ACK,
        refused,
    );
    let shot = s.step(
        "screenshot (visible)",
        json!({"op": "screenshot", "width": 1024}),
        budget::ACK,
        ok,
    );
    if !save(&shot, out, "visible.jpg") {
        s.failures.push("screenshot (visible) empty".into());
    }

    // Chromium itself refuses an http page's link to file: before any
    // navigation event, so the page must simply still be there.
    let file_link = find_ref(&snap, "link", "Leave to a local file");
    s.step(
        "click file: link",
        json!({"op": "click", "ref": file_link}),
        budget::ACK,
        ok,
    );
    s.step(
        "still on the form",
        json!({"op": "probe_location"}),
        budget::ACK,
        |r| r["text"].as_str().unwrap_or("").contains("/form.html"),
    );
    for (label, link, expect) in [
        (
            "click external link",
            "External site",
            "navigation to https://example.com",
        ),
        ("click download link", "Download report", "download of"),
        ("click popup link", "Open popup", "new window for"),
    ] {
        let r = find_ref(&snap, "link", link);
        s.step(
            label,
            json!({"op": "click", "ref": r}),
            budget::ACK,
            move |v| v["text"].as_str().unwrap_or("").contains(expect),
        );
    }
    s.step(
        "navigate file:",
        json!({"op": "navigate", "url": "file:///C:/Windows/win.ini"}),
        budget::ACK,
        refused,
    );
    s.step(
        "navigate claude.ai",
        json!({"op": "navigate", "url": "https://claude.ai/"}),
        budget::ACK,
        refused,
    );

    s.step(
        "navigate big page",
        json!({"op": "navigate", "url": format!("{base}/big.html")}),
        budget::ACK,
        ok,
    );
    s.step(
        "snapshot big page (budget cut)",
        json!({"op": "snapshot"}),
        budget::SNAPSHOT,
        |r| {
            r["text"]
                .as_str()
                .unwrap_or("")
                .contains("more lines cut at the budget")
        },
    );
    s.step(
        "snapshot big page, interactive, max 2000",
        json!({"op": "snapshot", "filter": "interactive", "max": 2000}),
        2_000,
        ok,
    );

    s.step(
        "probe: cookies seen by the agent view",
        json!({"op": "probe_cookies"}),
        2_000,
        |r| {
            let t = r["text"].as_str().unwrap_or("");
            t.contains("agent_seen") && !t.contains("chat_session")
        },
    );
    s.step(
        "probe: targets seen by the agent view",
        json!({"op": "probe_targets"}),
        2_000,
        |r| !r["text"].as_str().unwrap_or("").contains("chat.html"),
    );

    for mode in ["parked", "hidden"] {
        s.step(
            &format!("view {mode}"),
            json!({"op": "view", "mode": mode}),
            budget::ACK,
            ok,
        );
        let shot = s.step(
            &format!("screenshot ({mode})"),
            json!({"op": "screenshot", "width": 800}),
            budget::ACK,
            |_| true,
        );
        let got = save(&shot, out, &format!("{mode}.jpg"));
        println!("{mode} screenshot produced an image: {got}");
        s.step(
            &format!("snapshot ({mode})"),
            json!({"op": "snapshot", "filter": "interactive", "max": 1000}),
            1_000,
            ok,
        );
        s.step(
            &format!("click ({mode})"),
            json!({"op": "click", "ref": "e99"}),
            budget::ACK,
            |_| true,
        );
        s.step(
            "view shown",
            json!({"op": "view", "mode": "shown"}),
            budget::ACK,
            ok,
        );
    }
    let shot = s.step(
        "screenshot (shown again)",
        json!({"op": "screenshot", "width": 800}),
        budget::ACK,
        ok,
    );
    save(&shot, out, "shown-again.jpg");

    let _ = s.call(json!({"op": "quit"}));
    // SAFETY: plain query.
    let fg_after = unsafe { GetForegroundWindow() };
    let mut pid = 0u32;
    // SAFETY: plain query.
    unsafe { GetWindowThreadProcessId(fg_after, Some(&mut pid)) };
    println!(
        "foreground window now belongs to pid {pid}; host pid {}",
        host_pid
    );
    println!(
        "\nforeground window before {:?}, after {:?} ({})",
        fg_before.0,
        fg_after.0,
        if fg_before == fg_after {
            "unchanged"
        } else {
            "changed"
        }
    );
    if s.failures.is_empty() {
        println!("ALL PASS");
        Ok(())
    } else {
        Err(format!("failed: {}", s.failures.join(", ")))
    }
}
