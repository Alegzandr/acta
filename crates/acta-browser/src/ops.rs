//! The nine operations (ADR 0010, Tools), written against a [`Host`]: the
//! app implements it over its WebView2 (DevTools Protocol calls in-process,
//! on the UI thread), tests over a fake. Everything a reply says is built
//! here, within the budgets of [`budget`]; whatever came from a page is
//! fenced.

use serde_json::{Value, json};
use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::args::*;
use crate::events::{Events, Notice, Severity};
use crate::policy::{Policy, clip, origin_of, secret_field};
use crate::render::{self, Fence, Filter, budget, one_line};

/// Room the list replies keep for a `blocked:` line (its newline included),
/// so a reply with notices still fits its budget. The short replies add it
/// to theirs: see [`budget_with_notices`].
pub const NOTICE_ROOM: usize = budget::NOTICES + 1;

/// The most a reply to `req` may be, a `blocked:` line included.
pub fn budget_with_notices(req: &Request) -> usize {
    match req {
        Request::Snapshot(a) => a
            .max
            .map(|m| (m as usize).clamp(500, budget::SNAPSHOT_MAX))
            .unwrap_or(budget::SNAPSHOT),
        Request::Read(_) => budget::READ,
        Request::Console(_) => budget::CONSOLE,
        Request::Network(_) => budget::NETWORK,
        Request::Open(_) => budget::OPEN + NOTICE_ROOM,
        Request::Click(_) | Request::Type(_) => budget::ACK + NOTICE_ROOM,
        Request::Screenshot(_) => budget::CAPTION + NOTICE_ROOM,
        Request::Handoff(_) => budget::HANDOFF + NOTICE_ROOM,
    }
}

/// The one line every call gets while the user has pressed Stop.
pub const STOPPED: &str = "stopped by the user; ask them before continuing";
/// How long `open` (and a click or key that navigates) waits for the load.
pub const LOAD_WAIT: Duration = Duration::from_secs(15);
/// `snapshot(wait_for)` and `handoff` limits.
pub const WAIT_FOR: Duration = Duration::from_secs(10);
pub const HANDOFF_WAIT: Duration = Duration::from_secs(600);
pub const TEXT_MAX: usize = 2_000;
pub const MESSAGE_MAX: usize = 200;

/// How a handoff ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handoff {
    Done,
    Stopped,
    Timeout,
}

/// What the agent just did, for the activity line the human sees. The
/// app words it in the interface language; the text is a ref with the
/// element's role and name (`e19 button "Greet"`) or an origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activity {
    Opened(String),
    Reloaded,
    Back,
    Forward,
    Clicked(String),
    Typed(String),
    Screenshot,
    Waiting,
}

/// What the operations need from the app.
pub trait Host: Send + Sync {
    /// Makes sure the agent web view exists, creating it on first use. True
    /// when the view is not set up yet, whoever created it (the agent, the
    /// human's address field, Retry) and whether an earlier setup failed:
    /// the operations then run [`setup`] and call [`Host::set_up`]. `Err`
    /// is one line saying why there is no view.
    fn ensure_view(&self) -> Result<bool, String>;
    /// [`setup`] succeeded on the current view. The app forgets it when the
    /// view is closed or created again.
    fn set_up(&self);
    /// One DevTools Protocol call on the agent view's top-level page.
    fn cdp(&self, method: &str, params: Value) -> Result<Value, String>;
    fn events(&self) -> &Events;
    /// The navigation policy now (the user's blocked sites included).
    fn policy(&self) -> Policy;
    /// The activity line the human sees.
    fn activity(&self, what: Activity);
    /// Highlights the element acted on for 300 ms.
    fn highlight(&self, backend: i64);
    /// Shows `message` with a Done button and waits for the human.
    fn handoff(&self, message: &str, timeout: Duration) -> Handoff;
    /// The human pressed Stop and has not resumed.
    fn stopped(&self) -> bool;
}

/// The DevTools domains enabled on a new view, and the file chooser
/// interception (decision 5).
pub fn setup(host: &dyn Host) -> Result<(), String> {
    for (m, p) in [
        ("Page.enable", json!({})),
        ("Runtime.enable", json!({})),
        ("Network.enable", json!({})),
        ("DOM.enable", json!({})),
        ("Overlay.enable", json!({})),
        (
            "Page.setInterceptFileChooserDialog",
            json!({"enabled": true}),
        ),
    ] {
        host.cdp(m, p)?;
    }
    let tree = host.cdp("Page.getFrameTree", json!({}))?;
    host.events().set_main_frame(
        tree["frameTree"]["frame"]["id"]
            .as_str()
            .map(str::to_string),
    );
    // A new view drops input until it has produced a frame (run on
    // WebView2 157: a click within a second of creation never reached the
    // page). A one-pixel capture makes it draw one now.
    host.cdp(
        "Page.captureScreenshot",
        json!({"format": "jpeg", "quality": 10,
               "clip": {"x": 0, "y": 0, "width": 1, "height": 1, "scale": 1}}),
    )?;
    Ok(())
}

/// Runs one request and returns its reply, with the `blocked:` line of
/// whatever the policy refused since the previous reply.
pub fn run(host: &dyn Host, req: &Request) -> Reply {
    if host.stopped() {
        return Reply::err(STOPPED);
    }
    match host.ensure_view() {
        Ok(true) => match setup(host) {
            Ok(()) => host.set_up(),
            // Not marked: the next call sets it up again.
            Err(e) => {
                return Reply::err(clip(
                    &format!("the agent browser did not start: {e}"),
                    budget::ERROR,
                ));
            }
        },
        Ok(false) => {}
        Err(why) => return Reply::err(clip(&why, budget::ERROR)),
    }
    let mut reply = match req {
        Request::Open(a) => open(host, a),
        Request::Snapshot(a) => snapshot(host, a),
        Request::Read(a) => read(host, a),
        Request::Click(a) => click(host, a),
        Request::Type(a) => type_text(host, a),
        Request::Screenshot(a) => screenshot(host, a),
        Request::Console(a) => console(host, a),
        Request::Network(a) => network(host, a),
        Request::Handoff(a) => handoff(host, a),
    };
    if !reply.ok && host.stopped() {
        reply = Reply::err(STOPPED);
    }
    if let Some(line) = notices_line(&host.events().drain_notices()) {
        reply.text.push('\n');
        reply.text.push_str(&line);
    }
    reply
}

/// `blocked: [web page n] download https://x.com; dialog alert [end web page n]`,
/// within [`budget::NOTICES`].
pub fn notices_line(notices: &[Notice]) -> Option<String> {
    if notices.is_empty() {
        return None;
    }
    let items: Vec<String> = notices
        .iter()
        .map(|n| match n {
            Notice::Navigation(o) => format!("navigation to {}", clip(o, 60)),
            Notice::Download(o) => format!("download from {}", clip(o, 60)),
            Notice::PopupHere(o) => format!("new window opened here instead: {}", clip(o, 60)),
            Notice::Popup(o) => format!("pop-up to {}", clip(o, 60)),
            Notice::Dialog(k) => format!("{} dialog dismissed", clip(k, 12)),
            Notice::Permission(k) => format!("{} permission denied", clip(k, 20)),
            Notice::HttpAuth(o) => format!("sign-in prompt of {}", clip(o, 60)),
            Notice::FileChooser => "file chooser (no uploads)".into(),
            Notice::ExternalApp(s) => format!("{}: link to another application", clip(s, 20)),
        })
        .collect();
    let fence = Fence::new();
    let fixed = "blocked: ".len() + fence.cost() + 16;
    let mut kept = Vec::new();
    let mut used = fixed;
    for item in &items {
        if used + item.len() + 2 > budget::NOTICES {
            break;
        }
        used += item.len() + 2;
        kept.push(item.clone());
    }
    let more = items.len() - kept.len();
    let mut body = kept.join("; ");
    if more > 0 {
        body.push_str(&format!("; +{more} more"));
    }
    Some(format!("blocked: {}", fence.inline(&one_line(&body, 200))))
}

fn backend(rf: &str) -> Result<i64, String> {
    rf.trim()
        .strip_prefix('e')
        .and_then(|n| n.parse().ok())
        .filter(|n: &i64| *n > 0)
        .ok_or_else(|| format!("bad ref {}: use an e<n> ref from snapshot", clip(rf, 20)))
}

/// A DevTools error, as the agent reads it: a node that went away means the
/// page changed.
fn failed(what: &str, rf: Option<&str>, e: &str) -> Reply {
    if let Some(rf) = rf
        && (e.contains("No node") || e.contains("Could not find node") || e.contains("not found"))
    {
        return Reply::err(format!(
            "{rf} is not on the page anymore; take a new snapshot"
        ));
    }
    Reply::err(clip(&format!("{what} failed: {e}"), budget::ERROR))
}

/// The current URL and title, fenced on two lines. From the navigation
/// history: no script runs in the page for it.
fn location(host: &dyn Host) -> String {
    let (url, title) = match host.cdp("Page.getNavigationHistory", json!({})) {
        Ok(h) => {
            let i = h["currentIndex"].as_u64().unwrap_or(0) as usize;
            let e = &h["entries"][i];
            (
                e["url"].as_str().unwrap_or("").to_string(),
                e["title"].as_str().unwrap_or("").to_string(),
            )
        }
        Err(_) => (String::new(), String::new()),
    };
    let fence = Fence::new();
    format!(
        "{}\n{}\n{}\n{}",
        fence.open(),
        one_line(&url, 160),
        one_line(&title, 100),
        fence.close()
    )
}

fn stop_fn(host: &dyn Host) -> impl Fn() -> bool + '_ {
    move || host.stopped()
}

fn open(host: &dyn Host, a: &OpenArgs) -> Reply {
    let events = host.events();
    let since = events.loads();
    match (&a.url, a.go) {
        (Some(raw), None) => {
            let url = match host.policy().check(raw) {
                Ok(u) => u,
                Err(r) => return Reply::err(r.line()),
            };
            let v = match host.cdp("Page.navigate", json!({ "url": url.as_str() })) {
                Ok(v) => v,
                Err(e) => return failed("open", None, &e),
            };
            if let Some(e) = v["errorText"].as_str() {
                return Reply::err(clip(&format!("open failed: {e}"), budget::ERROR));
            }
            host.activity(Activity::Opened(origin_of(url.as_str())));
            if v.get("loaderId").is_none() {
                // Same document (a fragment): no load comes.
                return Reply::ok(format!("ok same page\n{}", location(host)));
            }
        }
        (None, Some(Go::Reload)) => {
            if let Err(e) = host.cdp("Page.reload", json!({})) {
                return failed("reload", None, &e);
            }
            host.activity(Activity::Reloaded);
        }
        (None, Some(go)) => {
            let h = match host.cdp("Page.getNavigationHistory", json!({})) {
                Ok(h) => h,
                Err(e) => return failed("open", None, &e),
            };
            let i = h["currentIndex"].as_i64().unwrap_or(0);
            let j = if go == Go::Back { i - 1 } else { i + 1 };
            let entries = h["entries"].as_array().cloned().unwrap_or_default();
            let Some(entry) = usize::try_from(j).ok().and_then(|j| entries.get(j)) else {
                return Reply::err(format!(
                    "no page to go {}",
                    if go == Go::Back {
                        "back to"
                    } else {
                        "forward to"
                    }
                ));
            };
            if let Err(r) = host.policy().check(entry["url"].as_str().unwrap_or("")) {
                return Reply::err(r.line());
            }
            let id = entry["id"].as_i64().unwrap_or(0);
            if let Err(e) = host.cdp("Page.navigateToHistoryEntry", json!({ "entryId": id })) {
                return failed("open", None, &e);
            }
            host.activity(if go == Go::Back {
                Activity::Back
            } else {
                Activity::Forward
            });
        }
        _ => return Reply::err("give url, or go: back, forward or reload"),
    }
    let loaded = events.wait_load(since, LOAD_WAIT, &stop_fn(host));
    if host.stopped() {
        return Reply::err(STOPPED);
    }
    let head = if loaded {
        "ok loaded"
    } else {
        "ok still loading after 15 s"
    };
    Reply::ok(format!("{head}\n{}", location(host)))
}

fn tree(host: &dyn Host) -> Result<Value, String> {
    host.cdp("Accessibility.getFullAXTree", json!({}))
}

fn tree_has_text(tree: &Value, needle: &str) -> bool {
    let needle = needle.to_lowercase();
    tree["nodes"].as_array().into_iter().flatten().any(|n| {
        n["name"]["value"]
            .as_str()
            .is_some_and(|s| s.to_lowercase().contains(&needle))
    })
}

fn snapshot(host: &dyn Host, a: &SnapshotArgs) -> Reply {
    let filter = match a.filter.unwrap_or_default() {
        SnapshotFilter::All => Filter::All,
        SnapshotFilter::Interactive => Filter::Interactive,
    };
    let within = match a.within.as_deref().map(backend).transpose() {
        Ok(w) => w,
        Err(e) => return Reply::err(e),
    };
    let max = a
        .max
        .map(|m| (m as usize).clamp(500, budget::SNAPSHOT_MAX))
        .unwrap_or(budget::SNAPSHOT);
    let mut t = match tree(host) {
        Ok(t) => t,
        Err(e) => return failed("snapshot", None, &e),
    };
    let mut waited = None;
    if let Some(text) = a.wait_for.as_deref().filter(|t| !t.trim().is_empty()) {
        let end = Instant::now() + WAIT_FOR;
        let mut found = tree_has_text(&t, text);
        while !found && Instant::now() < end && !host.stopped() {
            std::thread::sleep(Duration::from_millis(250));
            t = match tree(host) {
                Ok(t) => t,
                Err(e) => return failed("snapshot", None, &e),
            };
            found = tree_has_text(&t, text);
        }
        if host.stopped() {
            return Reply::err(STOPPED);
        }
        waited = Some(found);
    }
    if let (Some(b), Some(rf)) = (within, a.within.as_deref())
        && !render::has_node(&t, b)
    {
        return Reply::err(format!(
            "{} is not on the page anymore; take a new snapshot",
            clip(rf, 20)
        ));
    }
    let lines = render::snapshot_lines(&t, filter, within);
    let mut head = format!(
        "snapshot · {} lines · refs valid until the page changes",
        lines.len()
    );
    if waited == Some(false) {
        head.push_str(" · wait_for text not found after 10 s");
    }
    let tail_room = 100;
    let (mut text, n) = render::fenced(&head, &lines, max, tail_room + NOTICE_ROOM);
    if n < lines.len() {
        text.push_str(&format!(
            "\n+{} more lines; narrow with within=<ref> or filter=interactive",
            lines.len() - n
        ));
    }
    Reply::ok(text)
}

fn read(host: &dyn Host, a: &ReadArgs) -> Reply {
    let within = match a.within.as_deref().map(backend).transpose() {
        Ok(w) => w,
        Err(e) => return Reply::err(e),
    };
    let t = match tree(host) {
        Ok(t) => t,
        Err(e) => return failed("read", None, &e),
    };
    if let (Some(b), Some(rf)) = (within, a.within.as_deref())
        && !render::has_node(&t, b)
    {
        return Reply::err(format!(
            "{} is not on the page anymore; take a new snapshot",
            clip(rf, 20)
        ));
    }
    let lines = render::text_lines(&t, within);
    let from = (a.cursor.unwrap_or(0) as usize).min(lines.len());
    if lines.is_empty() {
        return Reply::ok("read · the page has no text");
    }
    if from == lines.len() {
        return Reply::ok(format!("read · end of the text ({} lines)", lines.len()));
    }
    let fence = Fence::new();
    let head_max = "read · lines 999999-999999 of 999999".len();
    let tail_max = "\nmore: cursor=999999".len();
    let room = budget::READ - head_max - 1 - fence.cost() - tail_max - NOTICE_ROOM;
    let n = render::fitting(&lines[from..], room).max(1);
    let to = (from + n).min(lines.len());
    let mut text = format!(
        "read · lines {}-{} of {}\n{}\n",
        from + 1,
        to,
        lines.len(),
        fence.open()
    );
    for l in &lines[from..to] {
        text.push_str(l);
        text.push('\n');
    }
    text.push_str(&fence.close());
    if to < lines.len() {
        text.push_str(&format!("\nmore: cursor={to}"));
    }
    Reply::ok(text)
}

/// The element that has focus and its role, from the accessibility tree
/// (no script). `None` when it cannot be read or nothing in this document
/// has focus.
fn focused(host: &dyn Host) -> Option<(i64, String)> {
    let tree = host.cdp("Accessibility.getFullAXTree", json!({})).ok()?;
    tree["nodes"].as_array()?.iter().rev().find_map(|n| {
        let focused = n["properties"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p["name"] == "focused" && p["value"]["value"] == json!(true));
        let role = n["role"]["value"].as_str().unwrap_or("");
        (focused && role != "RootWebArea")
            .then(|| {
                n["backendDOMNodeId"]
                    .as_i64()
                    .map(|b| (b, role.to_string()))
            })
            .flatten()
    })
}

/// The centre of the element's first box, after scrolling it into view.
fn centre(host: &dyn Host, b: i64) -> Result<(f64, f64), String> {
    let _ = host.cdp("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": b }));
    let q = host.cdp("DOM.getContentQuads", json!({ "backendNodeId": b }))?;
    let quad: Vec<f64> = q["quads"][0]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    if quad.len() != 8 {
        return Err("no box".into());
    }
    let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
    let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
    Ok((x, y))
}

/// The element and its descendants a few levels down.
fn subtree_ids(host: &dyn Host, b: i64) -> HashSet<i64> {
    fn collect(n: &Value, out: &mut HashSet<i64>) {
        if let Some(id) = n["backendNodeId"].as_i64() {
            out.insert(id);
        }
        for c in n["children"].as_array().into_iter().flatten() {
            collect(c, out);
        }
        for c in n["shadowRoots"].as_array().into_iter().flatten() {
            collect(c, out);
        }
    }
    let mut out = HashSet::from([b]);
    if let Ok(d) = host.cdp(
        "DOM.describeNode",
        json!({ "backendNodeId": b, "depth": 8, "pierce": true }),
    ) {
        collect(&d["node"], &mut out);
    }
    out
}

/// Role and name of an element, for the activity line only.
fn label(host: &dyn Host, b: i64) -> String {
    host.cdp(
        "Accessibility.getPartialAXTree",
        json!({ "backendNodeId": b, "fetchRelatives": false }),
    )
    .ok()
    .and_then(|t| render::describe(&t, b))
    .unwrap_or_default()
}

/// `e19 button "Greet"`.
fn target(host: &dyn Host, rf: &str, b: i64) -> String {
    let what = label(host, b);
    if what.is_empty() {
        rf.to_string()
    } else {
        format!("{rf} {what}")
    }
}

/// After an action: whether it started a navigation, and if so how it
/// ended. `navs`, `loads` and `stops` were read before the action.
fn after_action(host: &dyn Host, navs: u64, loads: u64, stops: u64) -> &'static str {
    let events = host.events();
    let end = Instant::now() + Duration::from_millis(300);
    while Instant::now() < end && events.navigations() == navs && events.loads() == loads {
        std::thread::sleep(Duration::from_millis(25));
    }
    if events.navigations() == navs && events.loads() == loads {
        return "";
    }
    match events.wait_settled(loads, stops, LOAD_WAIT, &stop_fn(host)) {
        Some(true) => " · a new page loaded",
        // Refused or failed: the `blocked:` line says why when it was us.
        Some(false) => "",
        None => " · a new page is still loading",
    }
}

fn mouse(host: &dyn Host, kind: &str, x: f64, y: f64, count: u32) -> Result<(), String> {
    let mut p = json!({ "type": kind, "x": x, "y": y });
    if kind != "mouseMoved" {
        p["button"] = json!("left");
        p["clickCount"] = json!(count);
    }
    host.cdp("Input.dispatchMouseEvent", p).map(|_| ())
}

fn click(host: &dyn Host, a: &ClickArgs) -> Reply {
    let rf = a.target.trim();
    let b = match backend(rf) {
        Ok(b) => b,
        Err(e) => return Reply::err(e),
    };
    let (x, y) = match centre(host, b) {
        Ok(p) => p,
        Err(e) if e == "no box" => {
            return Reply::err(format!(
                "{rf} has no box on screen (hidden or removed); take a new snapshot"
            ));
        }
        Err(e) => return failed("click", Some(rf), &e),
    };
    // What is really under the point: a covering element is reported.
    let hit = host
        .cdp(
            "DOM.getNodeForLocation",
            json!({ "x": x.round() as i64, "y": y.round() as i64, "includeUserAgentShadowDOM": true }),
        )
        .ok()
        .and_then(|v| v["backendNodeId"].as_i64());
    let note = match hit {
        Some(h) if !subtree_ids(host, b).contains(&h) => {
            format!(" (the point was covered by e{h})")
        }
        _ => String::new(),
    };
    host.highlight(b);
    host.activity(Activity::Clicked(target(host, rf, b)));
    let (navs, loads, stops) = (
        host.events().navigations(),
        host.events().loads(),
        host.events().stops(),
    );
    let mut steps = vec![("mouseMoved", 0), ("mousePressed", 1), ("mouseReleased", 1)];
    if a.double == Some(true) {
        steps.extend([("mousePressed", 2), ("mouseReleased", 2)]);
    }
    for (kind, count) in steps {
        if let Err(e) = mouse(host, kind, x, y, count) {
            return failed("click", Some(rf), &e);
        }
    }
    let nav = after_action(host, navs, loads, stops);
    let verb = if a.double == Some(true) {
        "double-clicked"
    } else {
        "clicked"
    };
    Reply::ok(format!("ok {verb} {rf}{note}{nav}"))
}

/// `(key, code, windowsVirtualKeyCode, text)` for `Input.dispatchKeyEvent`.
fn key_info(k: Key) -> (&'static str, &'static str, i64, Option<&'static str>) {
    match k {
        Key::Enter => ("Enter", "Enter", 13, Some("\r")),
        Key::Tab => ("Tab", "Tab", 9, None),
        Key::Escape => ("Escape", "Escape", 27, None),
        Key::Backspace => ("Backspace", "Backspace", 8, None),
        Key::Delete => ("Delete", "Delete", 46, None),
        Key::ArrowUp => ("ArrowUp", "ArrowUp", 38, None),
        Key::ArrowDown => ("ArrowDown", "ArrowDown", 40, None),
        Key::ArrowLeft => ("ArrowLeft", "ArrowLeft", 37, None),
        Key::ArrowRight => ("ArrowRight", "ArrowRight", 39, None),
        Key::Home => ("Home", "Home", 36, None),
        Key::End => ("End", "End", 35, None),
        Key::PageUp => ("PageUp", "PageUp", 33, None),
        Key::PageDown => ("PageDown", "PageDown", 34, None),
    }
}

fn press(host: &dyn Host, k: Key) -> Result<(), String> {
    let (key, code, vk, text) = key_info(k);
    let mut down = json!({
        "type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
        "key": key, "code": code,
        "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk,
    });
    if let Some(t) = text {
        down["text"] = json!(t);
        down["unmodifiedText"] = json!(t);
    }
    host.cdp("Input.dispatchKeyEvent", down)?;
    host.cdp(
        "Input.dispatchKeyEvent",
        json!({"type": "keyUp", "key": key, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk}),
    )
    .map(|_| ())
}

fn type_text(host: &dyn Host, a: &TypeArgs) -> Reply {
    let rf = a.target.trim();
    let b = match backend(rf) {
        Ok(b) => b,
        Err(e) => return Reply::err(e),
    };
    let text = a.text.as_deref().unwrap_or("");
    let n = text.chars().count();
    if n > TEXT_MAX {
        return Reply::err(format!("refused: text is {n} chars, at most {TEXT_MAX}"));
    }
    if text.is_empty() && a.clear != Some(true) && a.key.is_none() {
        return Reply::err("nothing to do: give text, clear or key");
    }
    let d = match host.cdp("DOM.describeNode", json!({ "backendNodeId": b })) {
        Ok(d) => d,
        Err(e) => return failed("type", Some(rf), &e),
    };
    let name = d["node"]["nodeName"].as_str().unwrap_or("");
    let attrs: Vec<String> = d["node"]["attributes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a.as_str().map(str::to_string))
        .collect();
    if secret_field(name, &attrs) {
        return Reply::err(format!(
            "refused: {rf} is a password, one-time code or card field. Call handoff so the user signs in by hand in Acta's browser; the session is kept."
        ));
    }
    host.highlight(b);
    host.activity(Activity::Typed(target(host, rf, b)));
    let (navs, loads, stops) = (
        host.events().navigations(),
        host.events().loads(),
        host.events().stops(),
    );
    if let Err(e) = host.cdp("DOM.focus", json!({ "backendNodeId": b })) {
        return failed("type", Some(rf), &e);
    }
    // The page may move focus on focus (to a password field, say): what
    // receives the text is checked again, and nothing is typed when it
    // cannot be confirmed (the tree unreadable, focus in another frame).
    let unconfirmed = || {
        Reply::err(format!(
            "refused: cannot confirm that {rf} has the keyboard (focus may be in a frame); take a new snapshot or call handoff"
        ))
    };
    let f = match focused(host) {
        None => return unconfirmed(),
        Some((_, role)) if role.eq_ignore_ascii_case("Iframe") => return unconfirmed(),
        Some((f, _)) => f,
    };
    if f != b {
        let secret = host
            .cdp("DOM.describeNode", json!({ "backendNodeId": f }))
            .map(|d| {
                let name = d["node"]["nodeName"].as_str().unwrap_or("").to_string();
                let attrs: Vec<String> = d["node"]["attributes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a.as_str().map(str::to_string))
                    .collect();
                secret_field(&name, &attrs)
            })
            // Unknown: refused rather than typed into.
            .unwrap_or(true);
        if secret {
            return Reply::err(format!(
                "refused: focus moved from {rf} to a password, one-time code or card field. Call handoff so the user signs in by hand."
            ));
        }
    }
    if a.clear == Some(true) {
        let select = json!({
            "type": "rawKeyDown", "key": "a", "code": "KeyA", "modifiers": 2,
            "windowsVirtualKeyCode": 65, "commands": ["selectAll"],
        });
        let r = host
            .cdp("Input.dispatchKeyEvent", select)
            .and_then(|_| {
                host.cdp(
                    "Input.dispatchKeyEvent",
                    json!({"type": "keyUp", "key": "a", "code": "KeyA", "modifiers": 2, "windowsVirtualKeyCode": 65}),
                )
            })
            .and_then(|_| press(host, Key::Delete));
        if let Err(e) = r {
            return failed("type", Some(rf), &e);
        }
    }
    if !text.is_empty()
        && let Err(e) = host.cdp("Input.insertText", json!({ "text": text }))
    {
        return failed("type", Some(rf), &e);
    }
    if let Some(k) = a.key
        && let Err(e) = press(host, k)
    {
        return failed("type", Some(rf), &e);
    }
    let mut out = String::from("ok");
    if a.clear == Some(true) {
        out.push_str(" cleared");
    }
    if n > 0 {
        out.push_str(&format!(" typed {n} chars into"));
    } else {
        out.push_str(" in");
    }
    out.push(' ');
    out.push_str(rf);
    if let Some(k) = a.key {
        out.push_str(&format!(" · pressed {}", key_info(k).0));
        out.push_str(after_action(host, navs, loads, stops));
    }
    Reply::ok(out)
}

fn screenshot(host: &dyn Host, a: &ScreenshotArgs) -> Reply {
    let width = f64::from(a.width.unwrap_or(1024).clamp(320, 1568));
    let _ = host.cdp("Overlay.hideHighlight", json!({}));
    let target = match a.target.as_deref().map(backend).transpose() {
        Ok(t) => t,
        Err(e) => return Reply::err(e),
    };
    if let Some(b) = target {
        let _ = host.cdp("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": b }));
    }
    let m = match host.cdp("Page.getLayoutMetrics", json!({})) {
        Ok(m) => m,
        Err(e) => return failed("screenshot", None, &e),
    };
    let vv = &m["cssVisualViewport"];
    // Device pixels per CSS pixel: the deprecated `visualViewport` is in
    // device pixels, `cssVisualViewport` in CSS pixels.
    let dpr = match (
        m["visualViewport"]["clientWidth"].as_f64(),
        vv["clientWidth"].as_f64(),
    ) {
        (Some(device), Some(css)) if css > 0.0 && device > 0.0 => device / css,
        _ => 1.0,
    };
    let (px, py) = (
        vv["pageX"].as_f64().unwrap_or(0.0),
        vv["pageY"].as_f64().unwrap_or(0.0),
    );
    let (vw, vh) = (
        vv["clientWidth"].as_f64().unwrap_or(1024.0).max(1.0),
        vv["clientHeight"].as_f64().unwrap_or(768.0).max(1.0),
    );
    let (mut x, mut y, mut w, mut h) = (px, py, vw, vh);
    if let (Some(b), Some(rf)) = (target, a.target.as_deref()) {
        let q = match host.cdp("DOM.getContentQuads", json!({ "backendNodeId": b })) {
            Ok(q) => q,
            Err(e) => return failed("screenshot", Some(rf), &e),
        };
        let quad: Vec<f64> = q["quads"][0]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        if quad.len() != 8 {
            return Reply::err(format!("{} has no box on screen", clip(rf, 20)));
        }
        let xs = [quad[0], quad[2], quad[4], quad[6]];
        let ys = [quad[1], quad[3], quad[5], quad[7]];
        let (x0, x1) = (
            xs.iter().cloned().fold(f64::MAX, f64::min),
            xs.iter().cloned().fold(f64::MIN, f64::max),
        );
        let (y0, y1) = (
            ys.iter().cloned().fold(f64::MAX, f64::min),
            ys.iter().cloned().fold(f64::MIN, f64::max),
        );
        // Inside the viewport only: what is on screen.
        let (x0, y0) = (x0.max(0.0), y0.max(0.0));
        let (x1, y1) = (x1.min(vw), y1.min(vh));
        if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
            return Reply::err(format!("{} has no box on screen", clip(rf, 20)));
        }
        (x, y, w, h) = (px + x0, py + y0, x1 - x0, y1 - y0);
    }
    let geometry = shot_geometry(w, h, dpr, width);
    let scale = geometry.scale;
    let shot = host.cdp(
        "Page.captureScreenshot",
        json!({
            "format": "jpeg", "quality": 70,
            "clip": { "x": x, "y": y, "width": w, "height": h, "scale": scale },
        }),
    );
    match shot {
        Ok(v) => {
            let data = v["data"].as_str().unwrap_or("").to_string();
            if data.is_empty() {
                return Reply::err("screenshot failed: no image");
            }
            let (iw, ih, tokens) = (geometry.width, geometry.height, geometry.tokens);
            let of = a
                .target
                .as_deref()
                .map(|r| format!(" of {}", clip(r.trim(), 20)))
                .unwrap_or_default();
            host.activity(Activity::Screenshot);
            Reply {
                ok: true,
                text: format!(
                    "screenshot{of} {iw}x{ih} jpeg · {} KB · ~{tokens} image tokens",
                    data.len() * 3 / 4 / 1024
                ),
                image: Some(data),
            }
        }
        Err(e) => failed("screenshot", None, &e),
    }
}

/// The capture's scale and the image it gives.
#[derive(Debug, PartialEq)]
pub struct ShotGeometry {
    /// `clip.scale` of `Page.captureScreenshot`.
    pub scale: f64,
    pub width: f64,
    pub height: f64,
    /// Claude's image cost: about width * height / 750 tokens.
    pub tokens: f64,
}

/// A clip of `w` x `h` CSS pixels on a display of `dpr` device pixels per
/// CSS pixel gives `w * scale * dpr` image pixels: the scale keeps the image
/// at most `width` wide and never enlarges it.
pub fn shot_geometry(w: f64, h: f64, dpr: f64, width: f64) -> ShotGeometry {
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let scale = (width / (w * dpr)).min(1.0);
    let (iw, ih) = ((w * scale * dpr).round(), (h * scale * dpr).round());
    ShotGeometry {
        scale,
        width: iw,
        height: ih,
        tokens: (iw * ih / 750.0).round(),
    }
}

fn limit(l: Option<u32>) -> usize {
    l.unwrap_or(20).clamp(1, 50) as usize
}

fn console(host: &dyn Host, a: &ConsoleArgs) -> Reply {
    let (all, dropped) = host.events().drain_console();
    let level = a.level.unwrap_or_default();
    let pattern = a.pattern.as_deref().map(str::to_lowercase);
    let kept: Vec<_> = all
        .iter()
        .filter(|m| match level {
            Level::Error => m.severity == Severity::Error,
            Level::Warning => m.severity != Severity::Info,
            Level::All => true,
        })
        .filter(|m| {
            pattern
                .as_deref()
                .is_none_or(|p| m.text.to_lowercase().contains(p))
        })
        .collect();
    let lines: Vec<String> = kept
        .iter()
        .rev()
        .take(limit(a.limit))
        .rev()
        .map(|m| format!("{:<9} {}", clip(&m.kind, 9), m.text))
        .collect();
    let mut head = format!("console · {} new since the last call", all.len());
    if dropped > 0 {
        head.push_str(&format!(" (+{dropped} older dropped)"));
    }
    if lines.is_empty() {
        head.push_str(" · none match");
        return Reply::ok(head);
    }
    head.push_str(&format!(" · last {} shown", lines.len()));
    let (text, _) = render::fenced(&head, &lines, budget::CONSOLE, NOTICE_ROOM);
    Reply::ok(text)
}

fn size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1048576.0)
    }
}

fn network(host: &dyn Host, a: &NetworkArgs) -> Reply {
    let all = host.events().drain_network();
    let pattern = a.pattern.as_deref().map(str::to_lowercase);
    let failed_only = a.failed == Some(true);
    let lines: Vec<String> = all
        .iter()
        .filter(|r| !failed_only || r.failed.is_some() || r.status.is_some_and(|s| s >= 400))
        .filter(|r| {
            pattern
                .as_deref()
                .is_none_or(|p| r.url.to_lowercase().contains(p))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(limit(a.limit))
        .rev()
        .map(|r| {
            let status = match (&r.failed, r.status) {
                (Some(why), _) => why.clone(),
                (None, Some(s)) => s.to_string(),
                (None, None) => "-".into(),
            };
            let url = match r.url.split_once('?') {
                Some((base, _)) => format!("{base}?..."),
                None => r.url.clone(),
            };
            format!(
                "{} {} {} {} {}",
                one_line(&r.method, 8),
                one_line(&status, 40),
                one_line(&r.kind.to_lowercase(), 12),
                r.size.map(size).unwrap_or_else(|| "-".into()),
                one_line(&url, 140)
            )
        })
        .collect();
    let mut head = format!("network · {} requests since the last call", all.len());
    if lines.is_empty() {
        head.push_str(" · none match");
        return Reply::ok(head);
    }
    head.push_str(&format!(" · last {} shown", lines.len()));
    let (text, _) = render::fenced(&head, &lines, budget::NETWORK, NOTICE_ROOM);
    Reply::ok(text)
}

fn handoff(host: &dyn Host, a: &HandoffArgs) -> Reply {
    let message = a.message.trim();
    let n = message.chars().count();
    if n == 0 {
        return Reply::err("give a message saying what the user should do");
    }
    if n > MESSAGE_MAX {
        return Reply::err(format!(
            "refused: message is {n} chars, at most {MESSAGE_MAX}"
        ));
    }
    host.activity(Activity::Waiting);
    match host.handoff(message, HANDOFF_WAIT) {
        Handoff::Done => Reply::ok("done: the user finished; take a new snapshot"),
        Handoff::Stopped => Reply::err(STOPPED),
        Handoff::Timeout => Reply::err("timeout: no answer from the user in 10 min"),
    }
}

#[cfg(test)]
#[path = "ops_tests.rs"]
pub(crate) mod tests;
