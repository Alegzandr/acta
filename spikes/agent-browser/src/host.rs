//! Stands in for the running app: a window that never activates, the agent
//! web view (its own profile) and a chat web view (another profile, standing
//! for claude.ai's), and the pipe server. WebView2 lives on this UI thread;
//! the pipe thread hands it one CDP call at a time through a thread message.

use crate::ops::{self, Cdp};
use crate::pipe::{self, Conn};
use crate::policy::{self, Policy};
use serde_json::{Value, json};
use std::num::NonZeroIsize;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, DevToolsProtocolEventReceivedEventHandler,
    take_pwstr,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PWSTR, w};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use wry::{NewWindowResponse, Rect, WebContext, WebViewBuilder, WebViewExtWindows as _};

pub const WIDTH: u32 = 1024;
pub const AGENT_HEIGHT: u32 = 700;

#[derive(Default)]
struct Shared {
    console: Vec<String>,
    notices: Vec<String>,
    loads: u64,
}

type SharedRef = Arc<(Mutex<Shared>, Condvar)>;

fn lock(s: &SharedRef) -> std::sync::MutexGuard<'_, Shared> {
    s.0.lock().unwrap_or_else(|p| p.into_inner())
}

enum Job {
    Cdp(String, String, Sender<Result<String, String>>),
    View(String, Sender<Result<(), String>>),
    Quit,
}

/// The pipe thread's handle on the UI thread.
struct Bridge {
    jobs: Sender<Job>,
    ui: u32,
    shared: SharedRef,
}

impl Bridge {
    fn post(&self, job: Job) -> Result<(), String> {
        self.jobs.send(job).map_err(|_| "app closed".to_string())?;
        // SAFETY: a thread message to the UI thread, drained in its loop.
        unsafe { PostThreadMessageW(self.ui, WM_APP, WPARAM(0), LPARAM(0)) }
            .map_err(|e| e.to_string())
    }
}

impl Cdp for Bridge {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let (tx, rx) = mpsc::channel();
        self.post(Job::Cdp(method.into(), params.to_string(), tx))?;
        let raw = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| format!("{method}: no answer in 10 s"))??;
        serde_json::from_str(&raw).map_err(|e| format!("{method}: {e}"))
    }

    fn wait_load(&self, since: u64, timeout: Duration) -> bool {
        let (m, cv) = &*self.shared;
        let g = m.lock().unwrap_or_else(|p| p.into_inner());
        let (g, _) = cv
            .wait_timeout_while(g, timeout, |s| s.loads <= since)
            .unwrap_or_else(|p| p.into_inner());
        g.loads > since
    }

    fn loads(&self) -> u64 {
        lock(&self.shared).loads
    }

    fn drain_console(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.shared).console)
    }

    fn drain_notices(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.shared).notices)
    }

    fn set_view(&self, mode: &str) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.post(Job::View(mode.into(), tx))?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "no answer".to_string())?
    }
}

struct Parent(HWND);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = NonZeroIsize::new(self.0.0 as isize).ok_or(HandleError::Unavailable)?;
        // SAFETY: the window outlives every web view built on it.
        Ok(
            unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(raw)))
            },
        )
    }
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if m == WM_MOUSEACTIVATE {
        // Clicks never activate this window (the machine is in use).
        return LRESULT(MA_NOACTIVATE as isize);
    }
    // SAFETY: default processing.
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// A top-level window to the right of every monitor, never activated, not
/// in the taskbar. WebView2 keeps rendering it (it is shown, not minimised).
fn make_window() -> windows::core::Result<HWND> {
    // SAFETY: standard window class registration and creation.
    unsafe {
        let inst = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: inst.into(),
            lpszClassName: w!("ActaSpikeAgentBrowser"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN) + GetSystemMetrics(SM_CXVIRTUALSCREEN) + 200;
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("ActaSpikeAgentBrowser"),
            w!("Acta spike: agent browser"),
            WS_POPUP | WS_CLIPCHILDREN,
            x,
            0,
            WIDTH as i32,
            AGENT_HEIGHT as i32 + 120,
            None,
            None,
            Some(inst.into()),
            None,
        )?;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Ok(hwnd)
    }
}

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w, h).into(),
    }
}

fn notice(shared: &SharedRef, s: String) {
    eprintln!("host: policy: {s}");
    lock(shared).notices.push(s);
}

fn on_event(
    wv: &ICoreWebView2,
    name: &str,
    mut f: impl FnMut(Value) + 'static,
) -> windows::core::Result<()> {
    // SAFETY: COM calls on the live web view; the handler is kept by it.
    unsafe {
        let recv = wv.GetDevToolsProtocolEventReceiver(&HSTRING::from(name))?;
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut p = PWSTR::null();
                    args.ParameterObjectAsJson(&mut p)?;
                    f(serde_json::from_str(&take_pwstr(p)).unwrap_or(Value::Null));
                }
                Ok(())
            }));
        let mut token = 0i64;
        recv.add_DevToolsProtocolEventReceived(&handler, &mut token)?;
    }
    Ok(())
}

fn cdp_call(wv: &ICoreWebView2, method: &str, params: &str, reply: Sender<Result<String, String>>) {
    let r2 = reply.clone();
    let m = method.to_string();
    let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |res, json| {
        let _ = reply.send(match res {
            Ok(()) => Ok(json),
            Err(e) => Err(format!("{m}: {} {}", e.code(), policy::clip(&json, 200))),
        });
        Ok(())
    }));
    // SAFETY: COM call on the live web view from its own thread.
    if let Err(e) = unsafe {
        wv.CallDevToolsProtocolMethod(&HSTRING::from(method), &HSTRING::from(params), &handler)
    } {
        let _ = r2.send(Err(format!("{method}: {e}")));
    }
}

fn console_line(p: &Value) -> String {
    let kind = p["type"].as_str().unwrap_or("log");
    let args: Vec<String> = p["args"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| match &a["value"] {
            Value::String(s) => s.clone(),
            Value::Null => a["description"].as_str().unwrap_or("").to_string(),
            v => v.to_string(),
        })
        .collect();
    format!("{kind} {}", policy::clip(&args.join(" "), 160))
}

pub fn run(key: [u8; 32], base: String, dir: PathBuf) -> Result<(), String> {
    let policy = Policy { local_only: true };
    // SAFETY: once, on the UI thread, before any COM use.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let hwnd = make_window().map_err(|e| e.to_string())?;
    let parent = Parent(hwnd);
    let shared: SharedRef = Arc::default();

    // The agent profile: its own user data folder, so its own browser
    // process. Never the chat profile.
    let mut agent_ctx = WebContext::new(Some(dir.join("agent-profile")));
    let (s1, s2, s3) = (shared.clone(), shared.clone(), shared.clone());
    let agent = WebViewBuilder::new_with_web_context(&mut agent_ctx)
        .with_url("about:blank")
        .with_bounds(rect(0, 0, WIDTH, AGENT_HEIGHT))
        .with_navigation_handler(move |url| match policy.check(&url) {
            Ok(_) => true,
            Err(e) => {
                notice(
                    &s1,
                    format!("navigation to {} ({e})", policy::clip(&url, 60)),
                );
                false
            }
        })
        .with_download_started_handler(move |url, _| {
            notice(&s2, format!("download of {}", policy::clip(&url, 60)));
            false
        })
        .with_new_window_req_handler(move |url, _| {
            notice(&s3, format!("new window for {}", policy::clip(&url, 60)));
            NewWindowResponse::Deny
        })
        .build_as_child(&parent)
        .map_err(|e| format!("agent web view: {e}"))?;

    // Stands for the claude.ai panel: another user data folder.
    let mut chat_ctx = WebContext::new(Some(dir.join("chat-profile")));
    let _chat = WebViewBuilder::new_with_web_context(&mut chat_ctx)
        .with_url(format!("{base}/chat.html"))
        .with_bounds(rect(0, AGENT_HEIGHT, WIDTH, 120))
        .build_as_child(&parent)
        .map_err(|e| format!("chat web view: {e}"))?;

    let wv = agent.webview();
    let sl = shared.clone();
    on_event(&wv, "Page.loadEventFired", move |_| {
        lock(&sl).loads += 1;
        sl.1.notify_all();
    })
    .map_err(|e| e.to_string())?;
    let sc = shared.clone();
    on_event(&wv, "Runtime.consoleAPICalled", move |p| {
        let mut g = lock(&sc);
        g.console.push(console_line(&p));
        let n = g.console.len();
        if n > 200 {
            g.console.drain(..n - 200);
        }
    })
    .map_err(|e| e.to_string())?;
    for m in ["Page.enable", "Runtime.enable"] {
        let (tx, _rx) = mpsc::channel();
        cdp_call(&wv, m, "{}", tx);
    }

    let (jobs_tx, jobs_rx): (Sender<Job>, Receiver<Job>) = mpsc::channel();
    // SAFETY: plain query.
    let ui = unsafe { GetCurrentThreadId() };
    let bridge = Bridge {
        jobs: jobs_tx,
        ui,
        shared: shared.clone(),
    };
    std::thread::spawn(move || {
        if let Err(e) = serve(bridge, key, policy) {
            eprintln!("host: pipe: {e}");
        }
    });

    let t0 = Instant::now();
    let mut foreground_hits = 0u32;
    let mut jobs_done = 0u32;
    let mut msg = MSG::default();
    // SAFETY: the standard message loop of this thread.
    unsafe {
        'outer: while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.hwnd.is_invalid() && msg.message == WM_APP {
                while let Ok(job) = jobs_rx.try_recv() {
                    jobs_done += 1;
                    if GetForegroundWindow() == hwnd {
                        foreground_hits += 1;
                    }
                    match job {
                        Job::Cdp(m, p, reply) => cdp_call(&wv, &m, &p, reply),
                        Job::View(mode, reply) => {
                            let r = match mode.as_str() {
                                "hidden" => agent.set_visible(false),
                                "parked" => agent.set_visible(true).and_then(|_| {
                                    agent.set_bounds(rect(WIDTH + 400, 0, WIDTH, AGENT_HEIGHT))
                                }),
                                _ => agent.set_visible(true).and_then(|_| {
                                    agent.set_bounds(rect(0, 0, WIDTH, AGENT_HEIGHT))
                                }),
                            };
                            let _ = reply.send(r.map_err(|e| e.to_string()));
                        }
                        Job::Quit => break 'outer,
                    }
                }
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    eprintln!(
        "host: quit after {:.1} s, {jobs_done} jobs; host window was foreground during {foreground_hits} of them",
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

fn serve(bridge: Bridge, key: [u8; 32], policy: Policy) -> Result<(), String> {
    let sid = pipe::user_sid().map_err(|e| e.to_string())?;
    let mut server = pipe::Server::new(&sid).map_err(|e| e.to_string())?;
    loop {
        let (file, pid, dacl) = server.accept().map_err(|e| format!("accept: {e}"))?;
        eprintln!("host: client pid {pid} connected; pipe DACL {dacl}");
        let mut c = Conn::new(file).map_err(|e| e.to_string())?;
        match pipe::server_handshake(&mut c, &key) {
            Ok(true) => eprintln!("host: client pid {pid} authenticated"),
            _ => {
                eprintln!("host: client pid {pid} refused (bad proof)");
                continue;
            }
        }
        while let Ok(Some(req)) = c.recv() {
            if req["op"] == "quit" {
                let _ = c.send(&json!({ "ok": true, "text": "bye" }));
                let _ = bridge.post(Job::Quit);
                return Ok(());
            }
            let t = Instant::now();
            let r = ops::run(&bridge, policy, &req);
            let _ = c.send(&json!({
                "ok": r.ok, "text": r.text, "image": r.image,
                "ms": t.elapsed().as_millis() as u64,
            }));
        }
        eprintln!("host: client pid {pid} disconnected");
    }
}
