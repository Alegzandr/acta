//! The agent browser's operations, written against a [`Cdp`] trait (ADR
//! 0010, decision 4). In the product this module lives in a GUI-free crate;
//! the app implements [`Cdp`] over its WebView2 and `acta mcp` only maps MCP
//! calls onto requests. The pipe carries these operations, never raw CDP.

use crate::policy::{self, Policy};
use crate::render::{self, Filter, budget};
use serde_json::{Value, json};
use std::time::Duration;

pub trait Cdp {
    /// One DevTools Protocol call on the agent web view's top-level page.
    fn call(&self, method: &str, params: Value) -> Result<Value, String>;
    /// Waits for the next `Page.loadEventFired` after `since`.
    fn wait_load(&self, since: u64, timeout: Duration) -> bool;
    fn loads(&self) -> u64;
    /// Takes the console lines and policy notices gathered since last time.
    fn drain_console(&self) -> Vec<String>;
    fn drain_notices(&self) -> Vec<String>;
    /// Spike only: `shown`, `hidden` (`set_visible(false)`) or `parked`
    /// (visible, but moved outside the window's client area).
    fn set_view(&self, mode: &str) -> Result<(), String>;
}

pub struct Reply {
    pub ok: bool,
    pub text: String,
    /// Base64 JPEG for `screenshot`.
    pub image: Option<String>,
}

fn ok(text: String) -> Reply {
    Reply {
        ok: true,
        text,
        image: None,
    }
}

fn err(text: String) -> Reply {
    Reply {
        ok: false,
        text,
        image: None,
    }
}

/// Notices (blocked navigation, download, popup) ride on the next reply.
fn with_notices(cdp: &dyn Cdp, mut r: Reply) -> Reply {
    let n = cdp.drain_notices();
    if !n.is_empty() {
        r.text.push_str(&format!("\nblocked: {}", n.join(" · ")));
    }
    r
}

fn backend(rf: &str) -> Result<i64, String> {
    rf.strip_prefix('e')
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("bad ref {rf}: use an e<n> ref from snapshot"))
}

/// The current page's URL and title, from the navigation history: no
/// script runs in the page for it.
fn location(cdp: &dyn Cdp) -> (String, String) {
    match cdp.call("Page.getNavigationHistory", json!({})) {
        Ok(h) => {
            let i = h["currentIndex"].as_u64().unwrap_or(0) as usize;
            let e = &h["entries"][i];
            (
                e["url"].as_str().unwrap_or("").to_string(),
                e["title"].as_str().unwrap_or("").to_string(),
            )
        }
        Err(_) => (String::new(), String::new()),
    }
}

fn page_line(cdp: &dyn Cdp) -> String {
    let (url, title) = location(cdp);
    let fence = render::Fence::new();
    // URL and title are page-controlled: fenced, one line each.
    format!(
        "{}\n{}\n{}\n{}",
        fence.open(),
        render::defuse(&policy::clip(&url, 160)),
        render::defuse(&policy::clip(&title, 120)),
        fence.close()
    )
}

pub fn run(cdp: &dyn Cdp, policy: Policy, req: &Value) -> Reply {
    let op = req["op"].as_str().unwrap_or("");
    let r = match op {
        "navigate" => navigate(cdp, policy, req["url"].as_str().unwrap_or("")),
        "snapshot" => snapshot(cdp, req),
        "click" => click(cdp, req["ref"].as_str().unwrap_or("")),
        "type" => type_text(
            cdp,
            req["ref"].as_str().unwrap_or(""),
            req["text"].as_str().unwrap_or(""),
        ),
        "screenshot" => screenshot(cdp, req["width"].as_u64().unwrap_or(1024)),
        "console" => console(cdp),
        // Spike-only probes, not tools.
        "view" => match cdp.set_view(req["mode"].as_str().unwrap_or("shown")) {
            Ok(()) => ok("ok".into()),
            Err(e) => err(e),
        },
        "probe_cookies" => probe_cookies(cdp),
        "probe_targets" => probe_targets(cdp),
        "probe_location" => ok(page_line(cdp)),
        _ => err(format!("unknown op {op}")),
    };
    with_notices(cdp, r)
}

fn navigate(cdp: &dyn Cdp, policy: Policy, raw: &str) -> Reply {
    let url = match policy.check(raw) {
        Ok(u) => u,
        Err(e) => return err(e),
    };
    let since = cdp.loads();
    match cdp.call("Page.navigate", json!({ "url": url.as_str() })) {
        Ok(v) => {
            if let Some(e) = v["errorText"].as_str() {
                return err(format!("navigation failed: {e}"));
            }
        }
        Err(e) => return err(format!("navigation failed: {e}")),
    }
    let loaded = cdp.wait_load(since, Duration::from_secs(15));
    let head = if loaded {
        "ok loaded"
    } else {
        "ok navigating (load not finished after 15 s)"
    };
    ok(format!("{head}\n{}", page_line(cdp)))
}

fn snapshot(cdp: &dyn Cdp, req: &Value) -> Reply {
    let filter = if req["filter"].as_str() == Some("interactive") {
        Filter::Interactive
    } else {
        Filter::All
    };
    let within = match req["within"].as_str() {
        Some(r) => match backend(r) {
            Ok(b) => Some(b),
            Err(e) => return err(e),
        },
        None => None,
    };
    let max = req["max"]
        .as_u64()
        .map(|m| (m as usize).min(budget::SNAPSHOT_MAX))
        .unwrap_or(budget::SNAPSHOT);
    let tree = match cdp.call("Accessibility.getFullAXTree", json!({})) {
        Ok(t) => t,
        Err(e) => return err(format!("snapshot failed: {e}")),
    };
    let lines = render::snapshot_lines(&tree, filter, within);
    let head = format!(
        "snapshot · {} lines · refs e<n> valid until the page changes",
        lines.len()
    );
    ok(render::fenced(&head, &lines, max))
}

/// The centre of the element's first content quad, after scrolling it
/// into view.
fn centre(cdp: &dyn Cdp, b: i64) -> Result<(f64, f64), String> {
    let _ = cdp.call("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": b }));
    let q = cdp.call("DOM.getContentQuads", json!({ "backendNodeId": b }))?;
    let quad = q["quads"][0]
        .as_array()
        .ok_or("element has no box (hidden?)")?
        .iter()
        .filter_map(Value::as_f64)
        .collect::<Vec<_>>();
    if quad.len() != 8 {
        return Err("element has no box (hidden?)".into());
    }
    let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
    let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
    Ok((x, y))
}

fn click(cdp: &dyn Cdp, rf: &str) -> Reply {
    let b = match backend(rf) {
        Ok(b) => b,
        Err(e) => return err(e),
    };
    let (x, y) = match centre(cdp, b) {
        Ok(p) => p,
        Err(e) => return err(format!("click {rf} failed: {e}")),
    };
    // What is really under the point: an overlay covering the target is
    // reported instead of silently clicked.
    let hit = cdp
        .call(
            "DOM.getNodeForLocation",
            json!({ "x": x as i64, "y": y as i64 }),
        )
        .ok()
        .and_then(|v| v["backendNodeId"].as_i64());
    // Highlight what the agent acts on, for the human watching the pane.
    let _ = cdp.call(
        "Overlay.highlightNode",
        json!({ "backendNodeId": b, "highlightConfig": { "contentColor": {"r": 255, "g": 170, "b": 0, "a": 0.35} } }),
    );
    for (kind, extra) in [
        ("mouseMoved", json!({})),
        ("mousePressed", json!({ "button": "left", "clickCount": 1 })),
        (
            "mouseReleased",
            json!({ "button": "left", "clickCount": 1 }),
        ),
    ] {
        let mut p = json!({ "type": kind, "x": x, "y": y });
        if let (Value::Object(p), Value::Object(e)) = (&mut p, extra) {
            p.extend(e);
        }
        if let Err(e) = cdp.call("Input.dispatchMouseEvent", p) {
            return err(format!("click {rf} failed: {e}"));
        }
    }
    std::thread::sleep(Duration::from_millis(250));
    let _ = cdp.call("Overlay.hideHighlight", json!({}));
    let note = match hit {
        Some(h) if h != b => format!(" (point was on e{h}, inside or over the target)"),
        _ => String::new(),
    };
    ok(format!("ok clicked {rf} at {x:.0},{y:.0}{note}"))
}

fn type_text(cdp: &dyn Cdp, rf: &str, text: &str) -> Reply {
    let b = match backend(rf) {
        Ok(b) => b,
        Err(e) => return err(e),
    };
    let d = match cdp.call("DOM.describeNode", json!({ "backendNodeId": b })) {
        Ok(d) => d,
        Err(e) => return err(format!("type {rf} failed: {e}")),
    };
    let name = d["node"]["nodeName"].as_str().unwrap_or("");
    let attrs: Vec<String> = d["node"]["attributes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a.as_str().map(str::to_string))
        .collect();
    if policy::sensitive_field(name, &attrs) {
        return err(format!(
            "refused: {rf} is a password, one-time code or card field. Ask the user to sign in by hand in Acta's agent browser; the session is kept."
        ));
    }
    if let Err(e) = cdp.call("DOM.focus", json!({ "backendNodeId": b })) {
        return err(format!("type {rf} failed: {e}"));
    }
    if let Err(e) = cdp.call("Input.insertText", json!({ "text": text })) {
        return err(format!("type {rf} failed: {e}"));
    }
    ok(format!("ok typed {} chars into {rf}", text.chars().count()))
}

fn screenshot(cdp: &dyn Cdp, width: u64) -> Reply {
    let width = width.clamp(320, 1568) as f64;
    let m = match cdp.call("Page.getLayoutMetrics", json!({})) {
        Ok(m) => m,
        Err(e) => return err(format!("screenshot failed: {e}")),
    };
    let vw = m["cssLayoutViewport"]["clientWidth"]
        .as_f64()
        .unwrap_or(1024.0);
    let vh = m["cssLayoutViewport"]["clientHeight"]
        .as_f64()
        .unwrap_or(768.0);
    let scale = (width / vw).min(1.0);
    let shot = cdp.call(
        "Page.captureScreenshot",
        json!({
            "format": "jpeg", "quality": 70,
            "clip": { "x": 0, "y": 0, "width": vw, "height": vh, "scale": scale },
        }),
    );
    match shot {
        Ok(v) => {
            let data = v["data"].as_str().unwrap_or("").to_string();
            let (w, h) = ((vw * scale).round(), (vh * scale).round());
            // Claude's image cost is about width * height / 750 tokens.
            let tokens = (w * h / 750.0).round();
            Reply {
                ok: true,
                text: format!(
                    "screenshot {w}x{h} jpeg · {} KB · ~{tokens} image tokens",
                    data.len() * 3 / 4 / 1024
                ),
                image: Some(data),
            }
        }
        Err(e) => err(format!("screenshot failed: {e}")),
    }
}

fn console(cdp: &dyn Cdp) -> Reply {
    let lines = cdp.drain_console();
    let n = lines.len();
    let last: Vec<String> = lines.into_iter().rev().take(20).rev().collect();
    let head = format!("console · {n} new messages · last {}", last.len());
    ok(render::fenced(&head, &last, budget::CONSOLE))
}

fn probe_cookies(cdp: &dyn Cdp) -> Reply {
    // Spike probe: Network.getAllCookies is deprecated but still answers
    // for every cookie of the browser process the view belongs to.
    match cdp.call("Network.getAllCookies", json!({})) {
        Ok(v) => {
            let names: Vec<String> = v["cookies"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    format!(
                        "{}@{}",
                        c["name"].as_str().unwrap_or(""),
                        c["domain"].as_str().unwrap_or("")
                    )
                })
                .collect();
            ok(format!(
                "cookies visible to the agent view: [{}]",
                names.join(", ")
            ))
        }
        Err(e) => err(format!("probe failed: {e}")),
    }
}

fn probe_targets(cdp: &dyn Cdp) -> Reply {
    match cdp.call("Target.getTargets", json!({})) {
        Ok(v) => {
            let t: Vec<String> = v["targetInfos"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| {
                    format!(
                        "{} {}",
                        t["type"].as_str().unwrap_or(""),
                        policy::clip(t["url"].as_str().unwrap_or(""), 60)
                    )
                })
                .collect();
            ok(format!(
                "targets visible to the agent view: [{}]",
                t.join(", ")
            ))
        }
        Err(e) => err(format!("Target.getTargets: {e}")),
    }
}
