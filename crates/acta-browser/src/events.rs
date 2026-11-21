//! What the agent web view reports between calls: loads, console messages,
//! network requests and what the policy blocked. The app forwards the
//! DevTools events of [`SUBSCRIBED`] and its own WebView2 events here from
//! the UI thread; operations read and drain them from the pipe thread.

use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::policy::clip;

/// The DevTools events the host subscribes to.
pub const SUBSCRIBED: &[&str] = &[
    "Page.loadEventFired",
    "Page.frameStartedLoading",
    "Page.frameStoppedLoading",
    "Page.fileChooserOpened",
    "Runtime.consoleAPICalled",
    "Runtime.exceptionThrown",
    "Network.requestWillBeSent",
    "Network.responseReceived",
    "Network.loadingFinished",
    "Network.loadingFailed",
];

/// Kept at most, oldest dropped first.
const KEEP: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleMessage {
    pub severity: Severity,
    pub kind: String,
    /// Page-controlled; one line, 160 chars.
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// Page-controlled.
    pub url: String,
    pub kind: String,
    pub status: Option<u16>,
    pub size: Option<u64>,
    pub failed: Option<String>,
    pub done: bool,
}

/// What the policy refused or dismissed, reported once in the next reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A navigation refused (origin of the URL, page-controlled).
    Navigation(String),
    Download(String),
    /// A pop-up or `target=_blank` link opened in the same view.
    PopupHere(String),
    Popup(String),
    Dialog(String),
    Permission(String),
    HttpAuth(String),
    FileChooser,
    /// A link to another application (`mailto:`, `ms-settings:`): never
    /// launched.
    ExternalApp(String),
}

#[derive(Default)]
struct State {
    loads: u64,
    navigations: u64,
    stops: u64,
    main_frame: Option<String>,
    console: VecDeque<ConsoleMessage>,
    console_dropped: u64,
    requests: HashMap<String, Request>,
    order: VecDeque<String>,
    notices: Vec<Notice>,
}

#[derive(Default)]
pub struct Events {
    state: Mutex<State>,
    changed: Condvar,
}

fn text_of(arg: &Value) -> String {
    match &arg["value"] {
        Value::String(s) => s.clone(),
        Value::Null => arg["description"]
            .as_str()
            .or(arg["unserializableValue"].as_str())
            .unwrap_or("")
            .to_string(),
        v => v.to_string(),
    }
}

impl Events {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Forget everything (the web view was closed or recreated).
    pub fn reset(&self) {
        *self.lock() = State::default();
        self.changed.notify_all();
    }

    pub fn set_main_frame(&self, id: Option<String>) {
        self.lock().main_frame = id;
    }

    /// A DevTools event of [`SUBSCRIBED`].
    pub fn on(&self, name: &str, p: &Value) {
        let mut s = self.lock();
        match name {
            "Page.loadEventFired" => s.loads += 1,
            "Page.frameStartedLoading" => {
                let main = s.main_frame.as_deref();
                if main.is_none() || p["frameId"].as_str() == main {
                    s.navigations += 1;
                }
            }
            "Page.frameStoppedLoading" => {
                let main = s.main_frame.as_deref();
                if main.is_none() || p["frameId"].as_str() == main {
                    s.stops += 1;
                }
            }
            "Page.fileChooserOpened" => s.notices.push(Notice::FileChooser),
            "Runtime.consoleAPICalled" => {
                let kind = p["type"].as_str().unwrap_or("log").to_string();
                let severity = match kind.as_str() {
                    "error" | "assert" => Severity::Error,
                    "warning" => Severity::Warning,
                    _ => Severity::Info,
                };
                let args: Vec<String> = p["args"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(text_of)
                    .collect();
                push_console(&mut s, severity, kind, &args.join(" "));
            }
            "Runtime.exceptionThrown" => {
                let d = &p["exceptionDetails"];
                let text = d["exception"]["description"]
                    .as_str()
                    .or(d["text"].as_str())
                    .unwrap_or("exception");
                push_console(&mut s, Severity::Error, "exception".into(), text);
            }
            "Network.requestWillBeSent" => {
                let Some(id) = p["requestId"].as_str() else {
                    return;
                };
                let r = Request {
                    method: p["request"]["method"].as_str().unwrap_or("GET").into(),
                    url: p["request"]["url"].as_str().unwrap_or("").into(),
                    kind: p["type"].as_str().unwrap_or("Other").into(),
                    ..Request::default()
                };
                if s.requests.insert(id.to_string(), r).is_none() {
                    s.order.push_back(id.to_string());
                }
                while s.order.len() > KEEP {
                    if let Some(old) = s.order.pop_front() {
                        s.requests.remove(&old);
                    }
                }
            }
            "Network.responseReceived" => {
                if let Some(r) = p["requestId"]
                    .as_str()
                    .and_then(|id| s.requests.get_mut(id))
                {
                    r.status = p["response"]["status"].as_u64().map(|v| v as u16);
                    if let Some(k) = p["type"].as_str() {
                        r.kind = k.into();
                    }
                }
            }
            "Network.loadingFinished" => {
                if let Some(r) = p["requestId"]
                    .as_str()
                    .and_then(|id| s.requests.get_mut(id))
                {
                    r.size = p["encodedDataLength"].as_f64().map(|v| v as u64);
                    r.done = true;
                }
            }
            "Network.loadingFailed" => {
                if let Some(r) = p["requestId"]
                    .as_str()
                    .and_then(|id| s.requests.get_mut(id))
                {
                    let why = if p["canceled"].as_bool() == Some(true) {
                        "canceled"
                    } else {
                        p["errorText"].as_str().unwrap_or("failed")
                    };
                    r.failed = Some(clip(why, 40));
                    r.done = true;
                }
            }
            _ => {}
        }
        drop(s);
        self.changed.notify_all();
    }

    pub fn notice(&self, n: Notice) {
        let mut s = self.lock();
        if s.notices.len() < 50 {
            s.notices.push(n);
        }
    }

    pub fn loads(&self) -> u64 {
        self.lock().loads
    }

    pub fn navigations(&self) -> u64 {
        self.lock().navigations
    }

    pub fn stops(&self) -> u64 {
        self.lock().stops
    }

    /// After an action that started a navigation: waits until the page
    /// loaded (`Some(true)`), the main frame stopped loading without a load
    /// (`Some(false)`: the navigation was refused or failed), or `timeout`.
    pub fn wait_settled(
        &self,
        loads: u64,
        stops: u64,
        timeout: Duration,
        stop: &dyn Fn() -> bool,
    ) -> Option<bool> {
        let end = Instant::now() + timeout;
        let mut s = self.lock();
        loop {
            if s.loads > loads {
                return Some(true);
            }
            if s.stops > stops {
                return Some(false);
            }
            let now = Instant::now();
            if now >= end || stop() {
                return None;
            }
            let slice = (end - now).min(Duration::from_millis(100));
            s = self
                .changed
                .wait_timeout(s, slice)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }

    /// Waits until more than `since` loads happened, `stop` says to give up,
    /// or `timeout` passes. True when a load came.
    pub fn wait_load(&self, since: u64, timeout: Duration, stop: &dyn Fn() -> bool) -> bool {
        let end = Instant::now() + timeout;
        let mut s = self.lock();
        loop {
            if s.loads > since {
                return true;
            }
            let now = Instant::now();
            if now >= end || stop() {
                return false;
            }
            let slice = (end - now).min(Duration::from_millis(100));
            s = self
                .changed
                .wait_timeout(s, slice)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }

    /// Console messages since the last call, and how many older ones were
    /// dropped unread.
    pub fn drain_console(&self) -> (Vec<ConsoleMessage>, u64) {
        let mut s = self.lock();
        let dropped = std::mem::take(&mut s.console_dropped);
        (s.console.drain(..).collect(), dropped)
    }

    /// Finished requests since the last call, oldest first; requests still
    /// in flight stay for the next call.
    pub fn drain_network(&self) -> Vec<Request> {
        let mut s = self.lock();
        let mut out = Vec::new();
        let order: Vec<String> = s.order.iter().cloned().collect();
        for id in order {
            if s.requests.get(&id).is_some_and(|r| r.done) {
                if let Some(r) = s.requests.remove(&id) {
                    out.push(r);
                }
                s.order.retain(|x| *x != id);
            }
        }
        out
    }

    pub fn drain_notices(&self) -> Vec<Notice> {
        std::mem::take(&mut self.lock().notices)
    }
}

fn push_console(s: &mut State, severity: Severity, kind: String, text: &str) {
    s.console.push_back(ConsoleMessage {
        severity,
        kind,
        text: crate::render::one_line(text, 160),
    });
    while s.console.len() > KEEP {
        s.console.pop_front();
        s.console_dropped += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn console_and_network_are_drained_once() {
        let e = Events::default();
        e.on(
            "Runtime.consoleAPICalled",
            &json!({"type": "error", "args": [{"type": "string", "value": "boom\nline"}]}),
        );
        e.on(
            "Network.requestWillBeSent",
            &json!({"requestId": "1", "type": "Fetch", "request": {"method": "POST", "url": "http://127.0.0.1/api?q=1"}}),
        );
        e.on(
            "Network.requestWillBeSent",
            &json!({"requestId": "2", "request": {"url": "http://127.0.0.1/slow"}}),
        );
        e.on(
            "Network.responseReceived",
            &json!({"requestId": "1", "response": {"status": 500}}),
        );
        e.on(
            "Network.loadingFinished",
            &json!({"requestId": "1", "encodedDataLength": 2048.0}),
        );
        let (c, dropped) = e.drain_console();
        assert_eq!(c[0].text, "boom line");
        assert_eq!((c.len(), dropped), (1, 0));
        assert!(e.drain_console().0.is_empty());
        let n = e.drain_network();
        assert_eq!(n.len(), 1);
        assert_eq!((n[0].status, n[0].size), (Some(500), Some(2048)));
        e.on(
            "Network.loadingFailed",
            &json!({"requestId": "2", "errorText": "net::ERR_FAILED"}),
        );
        assert_eq!(
            e.drain_network()[0].failed.as_deref(),
            Some("net::ERR_FAILED")
        );
    }

    #[test]
    fn loads_wake_a_waiter_and_stop_ends_the_wait() {
        let e = std::sync::Arc::new(Events::default());
        let e2 = e.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            e2.on("Page.loadEventFired", &json!({}));
        });
        assert!(e.wait_load(0, Duration::from_secs(5), &|| false));
        t.join().unwrap();
        let started = Instant::now();
        assert!(!e.wait_load(1, Duration::from_secs(5), &|| true));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn only_main_frame_loads_count_as_navigations() {
        let e = Events::default();
        e.set_main_frame(Some("main".into()));
        e.on("Page.frameStartedLoading", &json!({"frameId": "child"}));
        assert_eq!(e.navigations(), 0);
        e.on("Page.frameStartedLoading", &json!({"frameId": "main"}));
        assert_eq!(e.navigations(), 1);
    }
}
