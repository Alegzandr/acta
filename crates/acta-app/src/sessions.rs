//! Live Claude Code sessions as the app shows them (docs/design/components-app.md
//! 5, Claude Code sessions): the rows of `presence.db` (acta_core::presence),
//! watched off the UI thread, and the pure view model the status bar, the
//! tree, the page line, the Sessions view and the terminal share.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use acta_core::model::Owner;
use acta_core::presence::{LiveSession, Presence, SessionState};
use gpui_kit::{App, Global, Hsla};

use crate::i18n::{Arg, Key, Locale, text, tr};
use crate::theme::Tokens;

/// How often the watcher looks for a commit to `presence.db`.
const POLL: Duration = Duration::from_millis(500);
/// Sessions expire without a write (a missed heartbeat): the list is read
/// again at least this often.
const EXPIRE_CHECK: Duration = Duration::from_secs(15);

/// The live sessions, newest activity first.
#[derive(Default)]
pub struct Live {
    pub list: Vec<LiveSession>,
    /// Bumped on every change, for views that cache.
    pub revision: u64,
}

impl Global for Live {}

/// The plan usage last reported through `acta statusline`
/// (acta_core::usage), read from its file every few seconds.
#[derive(Default)]
pub struct PlanUsage(pub Option<acta_core::usage::Usage>);

impl Global for PlanUsage {}

pub fn plan_usage(cx: &App) -> Option<acta_core::usage::Usage> {
    cx.try_global::<PlanUsage>().and_then(|u| u.0)
}

/// How often the usage file is read (a small file; Claude Code rewrites it
/// at most once per status line update).
const USAGE_POLL: Duration = Duration::from_secs(5);

/// Reads the usage file for as long as the app runs.
pub fn watch_usage(cx: &mut App) {
    cx.default_global::<PlanUsage>();
    let Some(dir) = crate::db::config().map(|c| c.data_dir) else {
        return;
    };
    cx.spawn(async move |cx| {
        loop {
            let dir = dir.clone();
            let usage = cx
                .background_executor()
                .spawn(async move { acta_core::usage::read(&dir) })
                .await;
            cx.update(|cx| {
                let state = cx.default_global::<PlanUsage>();
                if state.0 != usage {
                    state.0 = usage;
                    cx.refresh_windows();
                }
            });
            cx.background_executor().timer(USAGE_POLL).await;
        }
    })
    .detach();
}

/// One status bar meter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meter {
    pub percent: u8,
    /// Above 80 %: drawn in `warning`.
    pub high: bool,
    /// The window reset after the number was seen.
    pub stale: bool,
    /// Unix seconds.
    pub resets_at: i64,
}

pub const HIGH_PERCENT: u8 = 80;

pub fn meter(w: Option<acta_core::usage::Window>, now_ms: i64) -> Option<Meter> {
    let w = w?;
    let percent = w.percent();
    Some(Meter {
        percent,
        high: percent > HIGH_PERCENT,
        stale: w.is_stale(now_ms),
        resets_at: w.resets_at,
    })
}

/// "resets at 19:00" today, "resets Mon 09:00" later.
pub fn resets_text(locale: Locale, resets_at: i64, now_ms: i64) -> String {
    let at = crate::when::local(resets_at.saturating_mul(1000));
    let now = crate::when::local(now_ms);
    let time = crate::i18n::time::clock(at);
    if at.day_number() == now.day_number() {
        tr(locale, Key::UsageResetsAt, &[("time", Arg::Text(&time))])
    } else {
        let day = crate::i18n::time::weekday_short(locale, at);
        tr(
            locale,
            Key::UsageResetsOn,
            &[("day", Arg::Text(day)), ("time", Arg::Text(&time))],
        )
    }
}

pub fn live(cx: &App) -> &[LiveSession] {
    cx.try_global::<Live>()
        .map(|l| l.list.as_slice())
        .unwrap_or(&[])
}

/// Watches `presence.db` for as long as the app runs. Opening it needs the
/// store (the key); if that fails, nothing is shown, as before presence.
pub fn start(db: crate::db::Db, cx: &mut App) {
    cx.default_global::<Live>();
    cx.spawn(async move |cx| {
        let opened = cx
            .background_executor()
            .spawn(async move {
                let presence = db.lock().presence()?;
                let watcher = presence.watcher()?;
                Ok::<_, acta_core::Error>((presence, watcher))
            })
            .await;
        let (presence, watcher) = match opened {
            Ok(pair) => pair,
            Err(_) => {
                crate::log::line("sessions: presence database not available");
                return;
            }
        };
        let presence = Arc::new(Mutex::new(presence));
        let watcher = Arc::new(Mutex::new(watcher));
        let mut since_read = EXPIRE_CHECK;
        loop {
            let changed = {
                let watcher = Arc::clone(&watcher);
                cx.background_executor()
                    .spawn(async move {
                        let mut w = watcher.lock().unwrap_or_else(|p| p.into_inner());
                        w.changed().unwrap_or(false)
                    })
                    .await
            };
            if changed || since_read >= EXPIRE_CHECK {
                since_read = Duration::ZERO;
                let presence = Arc::clone(&presence);
                let list = cx
                    .background_executor()
                    .spawn(async move { read(&presence) })
                    .await;
                cx.update(|cx| {
                    let flash = crate::settings::current(cx).flash_waiting;
                    let state = cx.default_global::<Live>();
                    if state.list != list {
                        let waiting = newly_waiting(&state.list, &list);
                        state.list = list;
                        state.revision += 1;
                        if flash && !waiting.is_empty() {
                            flash_taskbar();
                        }
                        cx.refresh_windows();
                    }
                });
            }
            cx.background_executor().timer(POLL).await;
            since_read += POLL;
        }
    })
    .detach();
}

/// Flashes Acta's taskbar button until it is activated, unless an Acta
/// window is already in front (components-app.md 5, Claude Code sessions:
/// "Flash Acta in the taskbar"). Never activates a window or takes focus.
#[cfg(windows)]
fn flash_taskbar() {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx, GW_OWNER,
        GetForegroundWindow, GetWindow, GetWindowThreadProcessId, IsWindowVisible,
    };
    struct Found {
        pid: u32,
        hwnd: HWND,
    }
    unsafe extern "system" fn each(hwnd: HWND, lp: LPARAM) -> windows_sys::core::BOOL {
        // SAFETY: `lp` is the `Found` passed to EnumWindows below, alive
        // for the whole enumeration; the calls only read window state.
        unsafe {
            let found = &mut *(lp as *mut Found);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == found.pid && IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null()
            {
                found.hwnd = hwnd;
                return 0;
            }
        }
        1
    }
    // SAFETY: plain Win32 calls on window handles of this process; the
    // callback's pointer lives on this stack frame until EnumWindows returns.
    unsafe {
        let pid = GetCurrentProcessId();
        let front = GetForegroundWindow();
        let mut front_pid = 0u32;
        if !front.is_null() {
            GetWindowThreadProcessId(front, &mut front_pid);
        }
        if front_pid == pid {
            return;
        }
        let mut found = Found {
            pid,
            hwnd: std::ptr::null_mut(),
        };
        EnumWindows(Some(each), &mut found as *mut Found as LPARAM);
        if found.hwnd.is_null() {
            return;
        }
        let info = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd: found.hwnd,
            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        FlashWindowEx(&info);
    }
}

#[cfg(not(windows))]
fn flash_taskbar() {}

fn read(presence: &Mutex<Presence>) -> Vec<LiveSession> {
    let mut p = presence.lock().unwrap_or_else(|p| p.into_inner());
    p.live(acta_core::time::now_ms()).unwrap_or_default()
}

// ---- view model (pure) ----

/// The record a session shows on: its topic or epic, else its project.
pub fn place(s: &LiveSession) -> Owner {
    s.focus.unwrap_or(Owner::Project(s.project))
}

/// Waiting before working before idle.
pub fn rank(state: SessionState) -> u8 {
    match state {
        SessionState::Waiting => 3,
        SessionState::Working => 2,
        SessionState::Idle => 1,
        SessionState::Ended => 0,
    }
}

/// The strongest state among `sessions`.
pub fn strongest<'a>(sessions: impl IntoIterator<Item = &'a LiveSession>) -> Option<SessionState> {
    sessions
        .into_iter()
        .map(|s| s.state)
        .max_by_key(|s| rank(*s))
}

/// What shows on one record: the strongest state and how many sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Here {
    pub state: SessionState,
    pub count: usize,
}

pub fn here(list: &[LiveSession], owner: Owner) -> Option<Here> {
    let on: Vec<&LiveSession> = list.iter().filter(|s| place(s) == owner).collect();
    Some(Here {
        state: strongest(on.iter().copied())?,
        count: on.len(),
    })
}

/// Counts by state, for the status bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub waiting: u64,
    pub working: u64,
    pub idle: u64,
}

pub fn counts(list: &[LiveSession]) -> Counts {
    let mut c = Counts::default();
    for s in list {
        match s.state {
            SessionState::Waiting => c.waiting += 1,
            SessionState::Working => c.working += 1,
            SessionState::Idle => c.idle += 1,
            SessionState::Ended => {}
        }
    }
    c
}

/// "Claude Code: 1 waiting for you · 2 working"; `None` with no session.
pub fn status_text(locale: Locale, c: Counts) -> Option<String> {
    let parts: Vec<String> = [
        (c.waiting, Key::SessionsWaiting),
        (c.working, Key::SessionsWorking),
        (c.idle, Key::SessionsIdle),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, key)| tr(locale, key, &[("n", Arg::Count(n))]))
    .collect();
    if parts.is_empty() {
        return None;
    }
    let list = parts.join(" \u{b7} ");
    Some(tr(
        locale,
        Key::StatusbarSessions,
        &[("list", Arg::Text(&list))],
    ))
}

pub fn state_word(locale: Locale, state: SessionState) -> String {
    text(
        locale,
        match state {
            SessionState::Waiting => Key::SessionsStateWaiting,
            SessionState::Working => Key::SessionsStateWorking,
            _ => Key::SessionsStateIdle,
        },
    )
}

/// The sentence of the tree tooltip and the page line.
pub fn here_text(locale: Locale, h: Here) -> String {
    if h.count > 1 {
        return tr(
            locale,
            Key::SessionsHereMany,
            &[("n", Arg::Count(h.count as u64))],
        );
    }
    text(
        locale,
        match h.state {
            SessionState::Waiting => Key::SessionsHereWaiting,
            SessionState::Working => Key::SessionsHereWorking,
            _ => Key::SessionsHereIdle,
        },
    )
}

pub fn state_color(state: SessionState, t: &Tokens) -> Hsla {
    match state {
        SessionState::Waiting => t.warning,
        SessionState::Working => t.accent,
        _ => t.text_tertiary,
    }
}

/// "since 14:05" today, "since 18 Nov 2026, 09:12" before.
pub fn since(locale: Locale, started_at: i64) -> String {
    let at = crate::when::local(started_at);
    let now = crate::when::local(acta_core::time::now_ms());
    let time = if at.day_number() == now.day_number() {
        crate::i18n::time::clock(at)
    } else {
        crate::when::datetime(locale, started_at)
    };
    tr(locale, Key::SessionsSince, &[("time", Arg::Text(&time))])
}

/// "last: WAL on", else "last tool: Bash", else nothing.
pub fn last(locale: Locale, s: &LiveSession) -> Option<String> {
    if let Some(a) = &s.activity {
        return Some(tr(locale, Key::SessionsLast, &[("activity", Arg::Text(a))]));
    }
    s.tool
        .as_ref()
        .map(|t| tr(locale, Key::SessionsLastTool, &[("tool", Arg::Text(t))]))
}

/// The second line of a Sessions row and the tail of the page line:
/// "since 14:05 · last: WAL on", with where it runs when `with_place`.
pub fn detail(locale: Locale, s: &LiveSession, with_place: bool) -> String {
    let mut parts = vec![since(locale, s.started_at)];
    parts.extend(last(locale, s));
    if with_place {
        parts.push(text(
            locale,
            if s.launch.is_some() {
                Key::SessionsInActa
            } else {
                Key::SessionsOutside
            },
        ));
    }
    parts.join(" \u{b7} ")
}

/// Sessions started in Acta's terminal that started waiting since `before`
/// (by launch token): what the taskbar flash reacts to.
pub fn newly_waiting(before: &[LiveSession], after: &[LiveSession]) -> Vec<String> {
    after
        .iter()
        .filter(|s| s.state == SessionState::Waiting && s.launch.is_some())
        .filter(|s| {
            !before
                .iter()
                .any(|b| b.id == s.id && b.state == SessionState::Waiting)
        })
        .map(|s| s.id.clone())
        .collect()
}

/// The live session of an Acta terminal, by its launch token.
pub fn of_launch<'a>(list: &'a [LiveSession], token: &str) -> Option<&'a LiveSession> {
    list.iter().find(|s| s.launch.as_deref() == Some(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{EpicId, ProjectId, TopicId};

    fn s(id: &str, focus: Option<Owner>, state: SessionState, launch: Option<&str>) -> LiveSession {
        LiveSession {
            id: id.into(),
            project: ProjectId(1),
            focus,
            launch: launch.map(str::to_owned),
            started_at: 0,
            activity_at: 0,
            alive_at: 0,
            state,
            tool: Some("Bash".into()),
            activity: None,
        }
    }

    #[test]
    fn meters_warn_above_80_and_go_stale_after_their_reset() {
        use acta_core::usage::Window;
        let w = Window {
            used: 80.4,
            resets_at: 1_000,
        };
        let m = meter(Some(w), 999_000).unwrap();
        assert_eq!((m.percent, m.high, m.stale), (80, false, false));
        let m = meter(
            Some(Window {
                used: 81.0,
                resets_at: 1_000,
            }),
            1_000_000,
        )
        .unwrap();
        assert!(m.high && m.stale);
        assert_eq!(meter(None, 0), None);
    }

    #[test]
    fn sessions_land_on_their_focus_or_their_project() {
        let t = Owner::Topic(TopicId(12));
        let list = vec![
            s("a", Some(t), SessionState::Working, None),
            s("b", Some(t), SessionState::Waiting, Some("tok")),
            s("c", None, SessionState::Idle, None),
            s("d", Some(Owner::Epic(EpicId(3))), SessionState::Idle, None),
        ];
        assert_eq!(
            here(&list, t),
            Some(Here {
                state: SessionState::Waiting,
                count: 2
            })
        );
        assert_eq!(
            here(&list, Owner::Project(ProjectId(1))).map(|h| h.count),
            Some(1)
        );
        assert_eq!(here(&list, Owner::Topic(TopicId(99))), None);
        assert_eq!(
            counts(&list),
            Counts {
                waiting: 1,
                working: 1,
                idle: 2
            }
        );
        assert_eq!(of_launch(&list, "tok").map(|s| s.id.as_str()), Some("b"));
    }

    #[test]
    fn the_status_names_only_the_states_that_have_sessions() {
        let c = Counts {
            waiting: 1,
            working: 2,
            idle: 0,
        };
        assert_eq!(
            status_text(Locale::En, c).as_deref(),
            Some("Claude Code: 1 waiting for you \u{b7} 2 working")
        );
        assert_eq!(
            status_text(Locale::Fr, c).as_deref(),
            Some("Claude Code\u{202f}: 1 vous attend \u{b7} 2 au travail")
        );
        assert_eq!(status_text(Locale::En, Counts::default()), None);
    }

    #[test]
    fn only_acta_sessions_that_start_waiting_flash() {
        let t = Some(Owner::Topic(TopicId(1)));
        let before = vec![
            s("a", t, SessionState::Working, Some("tok-a")),
            s("b", t, SessionState::Waiting, Some("tok-b")),
        ];
        let after = vec![
            s("a", t, SessionState::Waiting, Some("tok-a")),
            s("b", t, SessionState::Waiting, Some("tok-b")),
            s("c", t, SessionState::Waiting, None),
        ];
        assert_eq!(newly_waiting(&before, &after), ["a"]);
    }

    #[test]
    fn the_detail_line_says_since_last_and_where() {
        let mut one = s("a", None, SessionState::Working, Some("tok"));
        one.started_at = acta_core::time::now_ms();
        let line = detail(Locale::En, &one, true);
        assert!(line.starts_with("since "), "{line}");
        assert!(
            line.ends_with("last tool: Bash \u{b7} in Acta\u{2019}s terminal"),
            "{line}"
        );
        one.activity = Some("WAL on".into());
        one.launch = None;
        let line = detail(Locale::En, &one, true);
        assert!(
            line.ends_with("last: WAL on \u{b7} in another terminal"),
            "{line}"
        );
    }
}
