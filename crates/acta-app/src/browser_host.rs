//! The agent browser's WebView2 side (ADR 0010), Windows only: DevTools
//! Protocol calls on the agent view's page, in-process (never a port), the
//! events `acta-browser` reads, and the WebView2 events its policy needs
//! (dialogs dismissed, permissions denied, HTTP sign-in prompts cancelled).
//! Also the check that refuses to create the view when a debugging port is
//! configured for Acta.
//!
//! Everything here runs on the UI thread; the pipe threads reach it through
//! the panel's job channel (browser_panel.rs).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use acta_browser::events::{Events, Notice, SUBSCRIBED};
use acta_browser::policy::origin_of;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CAMERA,
    COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ, COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION,
    COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS,
    COREWEBVIEW2_PERMISSION_STATE_DENY, COREWEBVIEW2_SCRIPT_DIALOG_KIND,
    COREWEBVIEW2_SCRIPT_DIALOG_KIND_ALERT, COREWEBVIEW2_SCRIPT_DIALOG_KIND_BEFOREUNLOAD,
    COREWEBVIEW2_SCRIPT_DIALOG_KIND_CONFIRM, COREWEBVIEW2_SCRIPT_DIALOG_KIND_PROMPT, ICoreWebView2,
    ICoreWebView2_10, ICoreWebView2_18, ICoreWebView2DevToolsProtocolEventReceiver,
};
use webview2_com::{
    BasicAuthenticationRequestedEventHandler, CallDevToolsProtocolMethodCompletedHandler,
    DevToolsProtocolEventReceivedEventHandler, LaunchingExternalUriSchemeEventHandler,
    NavigationStartingEventHandler, PermissionRequestedEventHandler,
    ScriptDialogOpeningEventHandler, take_pwstr,
};
use windows_core::{HSTRING, Interface as _, PWSTR};

/// `<data dir>/agent-browser`: its own user data folder, hence its own
/// browser process, never the chat's (ADR 0010, decision 2).
pub fn profile_dir() -> Option<PathBuf> {
    use acta_core::paths::{BaseDirs, Os, data_dir};
    Some(data_dir(Os::CURRENT, &BaseDirs::from_env())?.join("agent-browser"))
}

/// One DevTools Protocol call; `reply` runs on the UI thread with the JSON
/// result or a one-line error.
pub fn call(
    webview: &ICoreWebView2,
    method: &str,
    params: &str,
    reply: impl Fn(Result<String, String>) + 'static,
) {
    let reply = std::rc::Rc::new(reply);
    let on_done = reply.clone();
    let name = method.to_string();
    let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |res, json| {
        on_done(match res {
            Ok(()) => Ok(json),
            Err(e) => Err(format!(
                "{name}: {}",
                acta_browser::policy::clip(&devtools_message(&json, e.code().0), 160)
            )),
        });
        Ok(())
    }));
    // SAFETY: a COM call on the live web view, from its own thread.
    if let Err(e) = unsafe {
        webview.CallDevToolsProtocolMethod(&HSTRING::from(method), &HSTRING::from(params), &handler)
    } {
        reply(Err(format!("{method}: {}", e.code().0)));
    }
}

/// The message of a DevTools error (`{"code":..,"message":".."}`).
fn devtools_message(json: &str, code: i32) -> String {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| format!("error {code:#x}"))
}

/// The registered handlers; dropping this removes them.
pub struct Hooks {
    webview: ICoreWebView2,
    receivers: Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>,
    dialog: i64,
    permission: i64,
    frame: i64,
    auth: Option<(ICoreWebView2_10, i64)>,
    external: Option<(ICoreWebView2_18, i64)>,
}

impl Drop for Hooks {
    fn drop(&mut self) {
        // SAFETY: each token came from the matching `add_` call on the object
        // kept beside it.
        unsafe {
            for (r, token) in &self.receivers {
                let _ = r.remove_DevToolsProtocolEventReceived(*token);
            }
            let _ = self.webview.remove_ScriptDialogOpening(self.dialog);
            let _ = self.webview.remove_PermissionRequested(self.permission);
            let _ = self.webview.remove_FrameNavigationStarting(self.frame);
            if let Some((w, token)) = &self.external {
                let _ = w.remove_LaunchingExternalUriScheme(*token);
            }
            if let Some((w, token)) = &self.auth {
                let _ = w.remove_BasicAuthenticationRequested(*token);
            }
        }
    }
}

fn dialog_kind(kind: COREWEBVIEW2_SCRIPT_DIALOG_KIND) -> &'static str {
    match kind {
        COREWEBVIEW2_SCRIPT_DIALOG_KIND_ALERT => "alert",
        COREWEBVIEW2_SCRIPT_DIALOG_KIND_CONFIRM => "confirm",
        COREWEBVIEW2_SCRIPT_DIALOG_KIND_PROMPT => "prompt",
        COREWEBVIEW2_SCRIPT_DIALOG_KIND_BEFOREUNLOAD => "leave-page",
        _ => "script",
    }
}

fn permission_kind(kind: COREWEBVIEW2_PERMISSION_KIND) -> &'static str {
    match kind {
        COREWEBVIEW2_PERMISSION_KIND_CAMERA => "camera",
        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => "microphone",
        COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION => "location",
        COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => "notifications",
        COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ => "clipboard",
        _ => "other",
    }
}

/// Subscribes to the DevTools events `acta-browser` reads and to the
/// WebView2 events of the policy, and turns off the page's own dialogs.
/// What a frame may load: `about:` and the page's own `blob:` documents,
/// and whatever the navigation policy allows.
fn frame_allowed(url: &str, allowed: &dyn Fn(&str) -> bool) -> bool {
    url.starts_with("about:") || url.starts_with("blob:") || allowed(url)
}

/// `allowed` is the navigation policy, applied to frames too.
pub fn install(
    webview: &ICoreWebView2,
    events: Arc<Events>,
    allowed: Arc<dyn Fn(&str) -> bool + Send + Sync>,
) -> windows_core::Result<Hooks> {
    let mut receivers = Vec::new();
    for name in SUBSCRIBED {
        let sink = events.clone();
        let event = name.to_string();
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut p = PWSTR::null();
                    // SAFETY: an out-parameter of the event's own arguments.
                    unsafe { args.ParameterObjectAsJson(&mut p)? };
                    let json = take_pwstr(p);
                    let value = serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
                    sink.on(&event, &value);
                }
                Ok(())
            }));
        let mut token = 0i64;
        // SAFETY: COM calls on the live web view; the receiver is kept to
        // remove the handler in Drop.
        let receiver = unsafe {
            let r = webview.GetDevToolsProtocolEventReceiver(&HSTRING::from(*name))?;
            r.add_DevToolsProtocolEventReceived(&handler, &mut token)?;
            r
        };
        receivers.push((receiver, token));
    }

    // SAFETY: settings of the live web view.
    unsafe {
        let settings = webview.Settings()?;
        settings.SetAreDefaultScriptDialogsEnabled(false)?;
        settings.SetAreDevToolsEnabled(false)?;
    }
    let sink = events.clone();
    let dialog_handler = ScriptDialogOpeningEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            let mut kind = COREWEBVIEW2_SCRIPT_DIALOG_KIND::default();
            // SAFETY: an out-parameter of the event's own arguments.
            unsafe { args.Kind(&mut kind)? };
            // Not accepted: the dialog is dismissed.
            sink.notice(Notice::Dialog(dialog_kind(kind).into()));
        }
        Ok(())
    }));
    let sink = events.clone();
    let permission_handler = PermissionRequestedEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
            // SAFETY: the event's own arguments.
            unsafe {
                args.PermissionKind(&mut kind)?;
                args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
            }
            sink.notice(Notice::Permission(permission_kind(kind).into()));
        }
        Ok(())
    }));
    let sink = events.clone();
    let frame_handler = NavigationStartingEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            let mut uri = PWSTR::null();
            // SAFETY: the event's own arguments.
            unsafe { args.Uri(&mut uri)? };
            let uri = take_pwstr(uri);
            if !frame_allowed(&uri, &*allowed) {
                // SAFETY: as above.
                unsafe { args.SetCancel(true)? };
                sink.notice(Notice::Navigation(origin_of(&uri)));
            }
        }
        Ok(())
    }));
    let (mut dialog, mut permission, mut frame) = (0i64, 0i64, 0i64);
    // SAFETY: COM calls on the live web view; tokens kept for Drop.
    unsafe {
        webview.add_ScriptDialogOpening(&dialog_handler, &mut dialog)?;
        webview.add_PermissionRequested(&permission_handler, &mut permission)?;
        webview.add_FrameNavigationStarting(&frame_handler, &mut frame)?;
    }
    let external = match webview.cast::<ICoreWebView2_18>() {
        Ok(w18) => {
            let sink = events.clone();
            let handler =
                LaunchingExternalUriSchemeEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut uri = PWSTR::null();
                        // SAFETY: the event's own arguments.
                        unsafe {
                            args.Uri(&mut uri)?;
                            args.SetCancel(true)?;
                        }
                        let uri = take_pwstr(uri);
                        let scheme = uri.split(':').next().unwrap_or("").to_string();
                        sink.notice(Notice::ExternalApp(scheme));
                    }
                    Ok(())
                }));
            let mut token = 0i64;
            // SAFETY: as above.
            unsafe { w18.add_LaunchingExternalUriScheme(&handler, &mut token)? };
            Some((w18, token))
        }
        Err(_) => None,
    };
    let auth = match webview.cast::<ICoreWebView2_10>() {
        Ok(w10) => {
            let sink = events.clone();
            let handler =
                BasicAuthenticationRequestedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut uri = PWSTR::null();
                        // SAFETY: the event's own arguments.
                        unsafe {
                            args.Uri(&mut uri)?;
                            args.SetCancel(true)?;
                        }
                        sink.notice(Notice::HttpAuth(origin_of(&take_pwstr(uri))));
                    }
                    Ok(())
                }));
            let mut token = 0i64;
            // SAFETY: as above.
            unsafe { w10.add_BasicAuthenticationRequested(&handler, &mut token)? };
            Some((w10, token))
        }
        Err(_) => None,
    };
    Ok(Hooks {
        webview: webview.clone(),
        receivers,
        dialog,
        permission,
        frame,
        auth,
        external,
    })
}

/// Holds `Local\acta-<SID>-app-<data dir tag>` for the life of the app, so
/// `acta mcp browser` knows it runs (or is starting) and never starts a
/// second one. `None` when the user or the data directory is unknown.
pub fn hold_app_mutex() -> Option<acta_browser::pipe::AppMutex> {
    let data_dir = crate::db::config()?.data_dir;
    let sid = acta_browser::pipe::user_sid().ok()?;
    acta_browser::pipe::AppMutex::hold(&acta_browser::pipe::mutex_name(&sid, &data_dir)).ok()
}

// ---- the debugging-port check (ADR 0010, decision 3) ----

/// The WebView2 policies whose values (named after an executable) change
/// what runs inside Acta's web views: extra browser arguments (a debugging
/// port), another runtime, another user data folder.
const POLICY_KEYS: &[&str] = &[
    r"Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments",
    r"Software\Policies\Microsoft\Edge\WebView2\BrowserExecutableFolder",
    r"Software\Policies\Microsoft\Edge\WebView2\UserDataFolder",
];
const ARGS_ENV: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
/// Set at all, these environment variables replace the runtime, the user
/// data folder, or open a script debugger pipe.
const OVERRIDE_ENVS: &[&str] = &[
    "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
    "WEBVIEW2_USER_DATA_FOLDER",
    "WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER",
];

/// Why a debugging port, or another WebView2 override, may be in effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugPort {
    /// A WebView2 policy value names Acta (or every application).
    Policy,
    /// `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` asks for a debugging port,
    /// or another `WEBVIEW2_*` override is set.
    Environment,
    /// A `DevToolsActivePort` file exists in a profile.
    PortFile,
}

/// Whether a policy value name applies to `exe`: its file name, its full
/// path, or `*`.
fn names_exe(name: &str, exe: &Path) -> bool {
    let name = name.trim();
    name == "*"
        || exe
            .file_name()
            .is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))
        || exe.to_string_lossy().eq_ignore_ascii_case(name)
}

/// The decision, from what was read.
fn debug_port_from(
    policy_names: &[String],
    exe: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    port_file: bool,
) -> Option<DebugPort> {
    let set = |name: &str| env(name).is_some_and(|v| !v.trim().is_empty());
    if policy_names.iter().any(|n| names_exe(n, exe)) {
        Some(DebugPort::Policy)
    } else if env(ARGS_ENV).is_some_and(|v| v.to_ascii_lowercase().contains("remote-debugging"))
        || OVERRIDE_ENVS.iter().any(|n| set(n))
    {
        Some(DebugPort::Environment)
    } else if port_file {
        Some(DebugPort::PortFile)
    } else {
        None
    }
}

/// Reads the policy keys (user and machine), the environment and both
/// profiles of `data_dir`.
pub fn debug_port(data_dir: &Path) -> Option<DebugPort> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let mut names = Vec::new();
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for path in POLICY_KEYS {
            if let Ok(key) = RegKey::predef(root).open_subkey(path) {
                names.extend(key.enum_values().flatten().map(|(name, _)| name));
            }
        }
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let env = |name: &str| std::env::var(name).ok();
    let port_file = ["agent-browser", "webview"].iter().any(|p| {
        let dir = data_dir.join(p);
        dir.join("DevToolsActivePort").exists()
            || dir.join("EBWebView").join("DevToolsActivePort").exists()
    });
    debug_port_from(&names, &exe, &env, port_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_policy_naming_acta_or_everyone_refuses() {
        let exe = Path::new(r"C:\Users\Alexandre\AppData\Local\Acta\current\Acta.exe");
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let none = |_: &str| None;
        assert_eq!(
            debug_port_from(&names(&["acta.exe"]), exe, &none, false),
            Some(DebugPort::Policy)
        );
        assert_eq!(
            debug_port_from(&names(&["*"]), exe, &none, false),
            Some(DebugPort::Policy)
        );
        assert_eq!(
            debug_port_from(&names(&[&exe.to_string_lossy()]), exe, &none, false),
            Some(DebugPort::Policy)
        );
        assert_eq!(
            debug_port_from(&names(&["other.exe"]), exe, &none, false),
            None
        );
        let only = |key: &'static str, value: &'static str| {
            move |name: &str| (name == key).then(|| value.to_string())
        };
        assert_eq!(
            debug_port_from(
                &[],
                exe,
                &only(ARGS_ENV, "--remote-debugging-port=0"),
                false
            ),
            Some(DebugPort::Environment)
        );
        assert_eq!(
            debug_port_from(&[], exe, &only(ARGS_ENV, "--remote-debugging-pipe"), false),
            Some(DebugPort::Environment)
        );
        assert_eq!(
            debug_port_from(&[], exe, &only(ARGS_ENV, "--lang=fr"), false),
            None
        );
        for name in OVERRIDE_ENVS {
            assert_eq!(
                debug_port_from(&[], exe, &only(name, r"C:\other"), false),
                Some(DebugPort::Environment),
                "{name}"
            );
            assert_eq!(debug_port_from(&[], exe, &only(name, " "), false), None);
        }
        assert_eq!(
            debug_port_from(&[], exe, &none, true),
            Some(DebugPort::PortFile)
        );
        assert_eq!(POLICY_KEYS.len(), 3);
    }

    #[test]
    fn a_port_file_in_either_profile_counts() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            debug_port(tmp.path()).filter(|d| *d == DebugPort::PortFile),
            None
        );
        let dir = tmp.path().join("webview").join("EBWebView");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("DevToolsActivePort"), "9222\n/devtools/browser/x").unwrap();
        assert_eq!(debug_port(tmp.path()), Some(DebugPort::PortFile));
    }

    #[test]
    fn frames_follow_the_policy() {
        let policy = acta_browser::policy::Policy::default();
        let allowed = |u: &str| policy.check(u).is_ok();
        assert!(frame_allowed("about:srcdoc", &allowed));
        assert!(frame_allowed("blob:http://127.0.0.1/x", &allowed));
        assert!(frame_allowed("https://example.com/embed", &allowed));
        for u in [
            "https://claude.ai/",
            "file:///C:/x",
            "data:text/html,x",
            "javascript:1",
        ] {
            assert!(!frame_allowed(u, &allowed), "{u}");
        }
    }

    #[test]
    fn dialogs_and_permissions_are_named() {
        assert_eq!(
            dialog_kind(COREWEBVIEW2_SCRIPT_DIALOG_KIND_CONFIRM),
            "confirm"
        );
        assert_eq!(
            permission_kind(COREWEBVIEW2_PERMISSION_KIND_CAMERA),
            "camera"
        );
        assert_eq!(
            devtools_message(
                r#"{"code":-32000,"message":"No node with given id found"}"#,
                1
            ),
            "No node with given id found"
        );
    }
}
