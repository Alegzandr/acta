//! Headless spike for ADR 0002: PTY + terminal emulation without any UI.
//!
//! Default backend: `alacritty_terminal`'s own tty module and event loop.
//! `cargo run -- portable` repeats the interactive scenario over `portable-pty`
//! and taps the raw byte stream to list the modes the child asks for.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color, Processor, Rgb};

const CELL_W: u16 = 9;
const CELL_H: u16 = 18;

#[derive(Clone, Copy)]
struct Size {
    cols: u16,
    rows: u16,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

impl From<Size> for WindowSize {
    fn from(s: Size) -> Self {
        WindowSize {
            num_lines: s.rows,
            num_cols: s.cols,
            cell_width: CELL_W,
            cell_height: CELL_H,
        }
    }
}

type Reply = Box<dyn Fn(String) + Send + Sync>;

/// Receives terminal events. Replies the emulator wants to send to the child
/// (cursor position reports, colour queries, size queries) go straight back to the PTY.
#[derive(Clone)]
struct Listener {
    events: Sender<Event>,
    reply: Arc<OnceLock<Reply>>,
    size: Arc<Mutex<Size>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let reply = |text: String| {
            if let Some(f) = self.reply.get() {
                f(text)
            }
        };
        match &event {
            Event::PtyWrite(text) => reply(text.clone()),
            // Pretend to be a dark theme: fg #d0d0d0 on bg #101010.
            Event::ColorRequest(index, format) => {
                let rgb = if *index == 11 {
                    Rgb {
                        r: 0x10,
                        g: 0x10,
                        b: 0x10,
                    }
                } else {
                    Rgb {
                        r: 0xd0,
                        g: 0xd0,
                        b: 0xd0,
                    }
                };
                reply(format(rgb));
            }
            Event::TextAreaSizeRequest(format) => {
                reply(format((*self.size.lock().unwrap()).into()));
            }
            _ => {}
        }
        let _ = self.events.send(event);
    }
}

fn new_term(size: Size) -> (Arc<FairMutex<Term<Listener>>>, Listener, Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let listener = Listener {
        events: tx,
        reply: Arc::default(),
        size: Arc::new(Mutex::new(size)),
    };
    let config = Config {
        kitty_keyboard: true,
        ..Config::default()
    };
    let term = Arc::new(FairMutex::new(Term::new(config, &size, listener.clone())));
    (term, listener, rx)
}

fn child_env() -> HashMap<String, String> {
    HashMap::from([
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM_PROGRAM".into(), "Acta".into()),
        ("TERM_PROGRAM_VERSION".into(), "0.0.0".into()),
    ])
}

/// A child running in a PTY, driven by alacritty_terminal's event loop.
struct Session {
    term: Arc<FairMutex<Term<Listener>>>,
    listener: Listener,
    events: Receiver<Event>,
    sender: EventLoopSender,
    pid: Option<u32>,
    seen: Vec<String>,
    exit: Option<String>,
}

impl Session {
    fn spawn(program: &str, args: &[&str], cwd: &Path, size: Size) -> std::io::Result<Self> {
        let (term, listener, events) = new_term(size);
        let options = tty::Options {
            shell: Some(tty::Shell::new(
                program.to_owned(),
                args.iter().map(|a| (*a).to_owned()).collect(),
            )),
            working_directory: Some(cwd.to_owned()),
            drain_on_exit: true,
            env: child_env(),
            #[cfg(target_os = "windows")]
            escape_args: true,
        };
        let pty = tty::new(&options, size.into(), 0)?;
        #[cfg(windows)]
        let pid = pty.child_watcher().pid().map(|p| p.get());
        #[cfg(unix)]
        let pid = Some(pty.child().id());

        let event_loop = EventLoop::new(term.clone(), listener.clone(), pty, true, false)?;
        let sender = event_loop.channel();
        let reply_sender = sender.clone();
        let _ = listener.reply.set(Box::new(move |text| {
            let _ = reply_sender.send(Msg::Input(Cow::Owned(text.into_bytes())));
        }));
        event_loop.spawn();
        Ok(Self {
            term,
            listener,
            events,
            sender,
            pid,
            seen: Vec::new(),
            exit: None,
        })
    }

    fn input(&self, text: &str) {
        let _ = self
            .sender
            .send(Msg::Input(Cow::Owned(text.as_bytes().to_vec())));
    }

    /// The order the app must use: the emulator first, then the PTY.
    fn resize(&self, size: Size) {
        *self.listener.size.lock().unwrap() = size;
        self.term.lock().resize(size);
        let _ = self.sender.send(Msg::Resize(size.into()));
    }

    /// Pump events until `done` holds, the child exits, or the timeout passes.
    fn pump(&mut self, timeout: Duration, done: impl Fn(&str) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            while let Ok(event) = self.events.try_recv() {
                match event {
                    Event::ChildExit(status) => self.exit = Some(format!("{status:?}")),
                    Event::Wakeup | Event::MouseCursorDirty => {}
                    other => self.seen.push(format!("{other:?}")),
                }
            }
            if done(&grid_text(&self.term.lock())) {
                return true;
            }
            if self.exit.is_some() || Instant::now() >= deadline {
                // One last look: output may have landed together with the exit.
                std::thread::sleep(Duration::from_millis(150));
                return done(&grid_text(&self.term.lock()));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn dump(&self, label: &str) {
        dump(label, &self.term.lock());
    }

    fn shutdown(self) -> Option<u32> {
        let _ = self.sender.send(Msg::Shutdown);
        self.pid
    }
}

/// Visible screen as plain text, one line per row, trailing blanks trimmed.
fn grid_text<T>(term: &Term<T>) -> String {
    let grid = term.grid();
    let mut out = String::new();
    for row in 0..grid.screen_lines() {
        let mut line = String::new();
        for col in 0..grid.columns() {
            let cell = &grid[Line(row as i32)][Column(col)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            line.push(cell.c);
            if let Some(zerowidth) = cell.zerowidth() {
                line.extend(zerowidth);
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn dump<T>(label: &str, term: &Term<T>) {
    let grid = term.grid();
    let (cols, rows) = (grid.columns(), grid.screen_lines());
    let (mut truecolor, mut indexed, mut styled, mut wide) = (0, 0, 0, 0);
    for row in 0..rows {
        for col in 0..cols {
            let cell = &grid[Line(row as i32)][Column(col)];
            match (cell.fg, cell.bg) {
                (Color::Spec(_), _) | (_, Color::Spec(_)) => truecolor += 1,
                (Color::Indexed(_), _) | (_, Color::Indexed(_)) => indexed += 1,
                _ => {}
            }
            if cell
                .flags
                .intersects(Flags::BOLD | Flags::DIM | Flags::ITALIC | Flags::INVERSE)
            {
                styled += 1;
            }
            if cell.flags.contains(Flags::WIDE_CHAR) {
                wide += 1;
            }
        }
    }
    let mode = *term.mode();
    let cursor = grid.cursor.point;
    println!("\n===== {label} =====");
    println!(
        "grid {cols}x{rows}, history {} lines, cursor line {} col {}, cursor visible: {}",
        grid.history_size(),
        cursor.line.0,
        cursor.column.0,
        mode.contains(TermMode::SHOW_CURSOR)
    );
    println!(
        "cells: {truecolor} truecolor, {indexed} indexed-colour, {styled} bold/dim/italic/inverse, {wide} wide"
    );
    println!(
        "modes: alt_screen={} bracketed_paste={} focus_events={} mouse={} sgr_mouse={} kitty_keyboard={}",
        mode.contains(TermMode::ALT_SCREEN),
        mode.contains(TermMode::BRACKETED_PASTE),
        mode.contains(TermMode::FOCUS_IN_OUT),
        mode.intersects(TermMode::MOUSE_MODE),
        mode.contains(TermMode::SGR_MOUSE),
        mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL),
    );
    let border = format!("+{}+", "-".repeat(cols));
    println!("{border}");
    for line in grid_text(term).lines() {
        println!("|{line:<cols$}|");
    }
    println!("{border}");
}

fn verdict(name: &str, ok: bool, detail: &str) -> bool {
    println!(
        "\n>>> {} {name}: {detail}",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// What a real user would type. On Windows `claude` may be a native exe or an
/// npm `.cmd` shim, and CreateProcess cannot start a `.cmd` directly.
fn resolve_claude() -> Option<(String, Vec<String>)> {
    let path = std::env::var_os("PATH")?;
    let names: &[&str] = if cfg!(windows) {
        &["claude.exe", "claude.cmd", "claude.bat"]
    } else {
        &["claude"]
    };
    for dir in std::env::split_paths(&path) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                let text = candidate.to_string_lossy().into_owned();
                return Some(if text.ends_with(".exe") || !cfg!(windows) {
                    (text, vec![])
                } else {
                    let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into());
                    (comspec, vec!["/d".into(), "/s".into(), "/c".into(), text])
                });
            }
        }
    }
    None
}

fn shell() -> (&'static str, Vec<&'static str>) {
    if cfg!(windows) {
        ("cmd.exe", vec!["/d", "/k"])
    } else {
        ("/bin/sh", vec![])
    }
}

fn process_alive(pid: u32) -> bool {
    let output = if cfg!(windows) {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
    } else {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
    };
    match output {
        Ok(out) if cfg!(windows) => {
            String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
        }
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

fn wait_dead(pid: Option<u32>) -> String {
    let Some(pid) = pid else {
        return "pid unknown".into();
    };
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if !process_alive(pid) {
            return format!("pid {pid} gone after {} ms", start.elapsed().as_millis());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    format!("pid {pid} STILL ALIVE after 5 s")
}

fn scenario_shell(cwd: &Path, size: Size) -> bool {
    let (program, args) = shell();
    let mut s = Session::spawn(program, &args, cwd, size).expect("spawn shell");
    // SGR truecolor + a wide CJK char + an emoji, written by the shell itself.
    let command = if cfg!(windows) {
        "echo hello Alexandre && cd && echo \x1b[38;2;255;100;0morange\x1b[0m wide:\u{6f22}\u{5b57} ok\r"
    } else {
        "echo hello Alexandre && pwd && printf '\\033[38;2;255;100;0morange\\033[0m wide:\u{6f22}\u{5b57} ok\\n'\r"
    };
    s.input(command);
    let leaf = cwd.file_name().unwrap().to_string_lossy().into_owned();
    let ok = s.pump(Duration::from_secs(10), |g| {
        g.lines().any(|l| l.trim() == "hello Alexandre")
            && g.lines()
                .any(|l| l.starts_with("orange wide:\u{6f22}\u{5b57} ok"))
            && g.lines()
                .any(|l| l.trim_end().ends_with(&leaf) && !l.contains('>'))
    });
    s.dump("(a) shell command");
    let pid = s.shutdown();
    verdict(
        "(a) shell command",
        ok,
        &format!("echo, cwd and wide (CJK) text; {}", wait_dead(pid)),
    )
}

fn scenario_version(cwd: &Path, size: Size, claude: &(String, Vec<String>)) -> bool {
    let mut args: Vec<&str> = claude.1.iter().map(String::as_str).collect();
    args.push("--version");
    let mut s = Session::spawn(&claude.0, &args, cwd, size).expect("spawn claude --version");
    let ok = s.pump(Duration::from_secs(30), |g| g.contains("(Claude Code)"));
    s.dump("(b) claude --version");
    let exit = s.exit.clone().unwrap_or_else(|| "no exit event yet".into());
    s.shutdown();
    verdict(
        "(b) claude --version",
        ok,
        &format!("direct spawn of {}; exit: {exit}", claude.0),
    )
}

fn scenario_interactive(
    cwd: &Path,
    size: Size,
    small: Size,
    claude: &(String, Vec<String>),
) -> bool {
    let args: Vec<&str> = claude.1.iter().map(String::as_str).collect();
    let started = Instant::now();
    let mut s = Session::spawn(&claude.0, &args, cwd, size).expect("spawn claude");
    // No input is ever sent. Wait for something recognisable, then let it settle.
    let recognisable = |g: &str| {
        let g = g.to_lowercase();
        g.contains("claude") || g.contains("trust") || g.contains("log in") || g.contains("login")
    };
    let seen = s.pump(Duration::from_secs(25), recognisable);
    let first_paint = started.elapsed();
    s.pump(Duration::from_secs(4), |_| false);
    s.dump("(c) interactive claude, first screen at 120x40");
    println!(
        "first recognisable content after {} ms",
        first_paint.as_millis()
    );
    println!("emulator events: {:?}", s.seen);
    let before = grid_text(&s.term.lock());
    let ok_c = verdict(
        "(c) interactive claude",
        seen && s.exit.is_none(),
        &format!(
            "recognisable first screen: {seen}, child exit: {:?}",
            s.exit
        ),
    );

    s.resize(small);
    s.pump(Duration::from_secs(4), |_| false);
    s.dump("(d2) interactive claude after resize to 100x30");
    let after = grid_text(&s.term.lock());
    let widest = after.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let ok_d = verdict(
        "(d2) claude repaints after resize",
        s.term.lock().grid().columns() == small.cols as usize
            && recognisable(&after)
            && after != before
            && s.exit.is_none(),
        &format!(
            "grid is {} cols, widest painted line {widest} chars",
            small.cols
        ),
    );
    let pid = s.shutdown();
    println!("kill: {}", wait_dead(pid));
    ok_c && ok_d
}

fn scenario_resize(cwd: &Path, size: Size, small: Size) -> bool {
    let (program, args) = shell();
    let mut s = Session::spawn(program, &args, cwd, size).expect("spawn shell");
    let long: String = (0..11)
        .map(|i| format!("{i:02}34567890"))
        .collect::<String>()
        + "END";
    s.input(&format!("echo {long}\r"));
    let printed = s.pump(Duration::from_secs(10), |g| g.lines().any(|l| l == long));
    s.dump("(d1) before resize: one 113-char line at 120 cols");

    s.resize(small);
    let size_query = if cfg!(windows) {
        "powershell -NoProfile -Command \"'child sees ' + $Host.UI.RawUI.WindowSize.Width + 'x' + $Host.UI.RawUI.WindowSize.Height\"\r"
    } else {
        "echo \"child sees $(stty size)\"\r"
    };
    s.input(size_query);
    let expected = if cfg!(windows) {
        format!("child sees {}x{}", small.cols, small.rows)
    } else {
        format!("child sees {} {}", small.rows, small.cols)
    };
    let propagated = s.pump(Duration::from_secs(20), |g| {
        g.lines().any(|l| l == expected)
    });
    s.dump("(d1) after resize to 100x30");
    let text = grid_text(&s.term.lock());
    let head = &long[..small.cols as usize];
    let tail = &long[small.cols as usize..];
    let lines: Vec<&str> = text.lines().collect();
    let reflowed = lines.windows(2).any(|w| w[0] == head && w[1] == tail);
    let pid = s.shutdown();
    wait_dead(pid);
    verdict(
        "(d1) resize propagation and reflow",
        printed && propagated && reflowed,
        &format!(
            "printed={printed} child_saw_new_size={propagated} long_line_rewrapped={reflowed}"
        ),
    )
}

/// Same interactive launch over portable-pty, with a raw tap on the byte stream.
fn scenario_portable(
    label: &str,
    expect: &str,
    wait: Duration,
    cwd: &Path,
    size: Size,
    claude: &(String, Vec<String>),
) -> bool {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut cmd = CommandBuilder::new(&claude.0);
    cmd.args(&claude.1);
    cmd.cwd(cwd);
    for (key, value) in child_env() {
        cmd.env(key, value);
    }
    let mut child = pair.slave.spawn_command(cmd).expect("spawn claude");
    drop(pair.slave);

    let (term, listener, _events) = new_term(size);
    let writer = Mutex::new(pair.master.take_writer().expect("writer"));
    let _ = listener.reply.set(Box::new(move |text| {
        let mut writer = writer.lock().unwrap();
        let _ = writer.write_all(text.as_bytes());
        let _ = writer.flush();
    }));

    let raw = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut reader = pair.master.try_clone_reader().expect("reader");
    let (reader_term, reader_raw) = (term.clone(), raw.clone());
    std::thread::spawn(move || {
        let mut parser: Processor = Processor::new();
        let mut buf = [0u8; 65536];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            reader_raw.lock().unwrap().extend_from_slice(&buf[..n]);
            parser.advance(&mut *reader_term.lock(), &buf[..n]);
        }
    });

    std::thread::sleep(wait);
    dump(label, &term.lock());
    let text = grid_text(&term.lock()).to_lowercase();
    let raw = raw.lock().unwrap().clone();
    let count = |needle: &[u8]| raw.windows(needle.len()).filter(|w| *w == needle).count();
    println!("raw bytes received: {}", raw.len());
    for (name, needle) in [
        ("synchronized output begin  CSI ?2026h", &b"\x1b[?2026h"[..]),
        ("synchronized output end    CSI ?2026l", b"\x1b[?2026l"),
        ("bracketed paste on         CSI ?2004h", b"\x1b[?2004h"),
        ("focus reporting on         CSI ?1004h", b"\x1b[?1004h"),
        ("alt screen on              CSI ?1049h", b"\x1b[?1049h"),
        ("mouse (any-event) on       CSI ?1003h", b"\x1b[?1003h"),
        ("mouse (SGR) on             CSI ?1006h", b"\x1b[?1006h"),
        ("win32-input-mode on        CSI ?9001h", b"\x1b[?9001h"),
        ("kitty keyboard push        CSI >", b"\x1b[>"),
        ("kitty keyboard query       CSI ?u", b"\x1b[?u"),
        ("cursor position request    CSI 6n", b"\x1b[6n"),
        ("device attributes request  CSI c", b"\x1b[c"),
        ("truecolor SGR              38;2;", b"38;2;"),
        ("OSC 8 hyperlink", b"\x1b]8;"),
        ("OSC 0/2 title", b"\x1b]0;"),
    ] {
        println!("  {:>4} x {name}", count(needle));
    }
    let _ = child.kill();
    let _ = child.wait();
    verdict(
        label,
        text.contains(expect),
        &format!("grid contains {expect:?}"),
    )
}

fn main() {
    // This spike is usually started from inside a Claude Code session; do not
    // let that session's private variables leak into the child we launch.
    let inherited: Vec<String> = std::env::vars()
        .map(|(key, _)| key)
        .filter(|key| key.starts_with("CLAUDE") || key.starts_with("TERM_PROGRAM"))
        .collect();
    for key in inherited {
        // SAFETY: single-threaded, before anything else runs.
        unsafe { std::env::remove_var(key) };
    }

    let size = Size {
        cols: 120,
        rows: 40,
    };
    let small = Size {
        cols: 100,
        rows: 30,
    };
    let cwd: PathBuf =
        std::env::temp_dir().join(format!("acta-terminal-spike-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).expect("create temp working directory");
    let cwd = cwd.canonicalize().map(strip_verbatim).unwrap_or(cwd);
    println!(
        "os: {} / working directory: {}",
        std::env::consts::OS,
        cwd.display()
    );

    let claude = resolve_claude();
    println!("claude resolved to: {claude:?}");

    let mut results = Vec::new();
    let mode = std::env::args().nth(1);
    if mode.as_deref() == Some("portable") {
        let claude = claude.expect("claude on PATH");
        let label = "(portable-pty) interactive claude, first screen at 120x40";
        results.push(scenario_portable(
            label,
            "claude",
            Duration::from_secs(10),
            &cwd,
            size,
            &claude,
        ));
    } else if mode.as_deref() == Some("probe") && cfg!(windows) {
        // Which sequences survive the trip through ConPTY? Emit known ones from a child.
        let script = r#"$e=[char]27; [Console]::Write("$e[?2026hsync-probe$e[?2026l $e[?2004h$e[?1004h$e[>1u$e[38;2;1;2;3mrgb$e[0m $e]8;;https://example.com$e\link$e]8;;$e\ done")"#;
        let probe = (
            "powershell.exe".to_owned(),
            vec!["-NoProfile".into(), "-Command".into(), script.into()],
        );
        let label = "(probe) known sequences written by a child through ConPTY";
        results.push(scenario_portable(
            label,
            "sync-probe",
            Duration::from_secs(6),
            &cwd,
            size,
            &probe,
        ));
    } else {
        results.push(scenario_shell(&cwd, size));
        results.push(scenario_resize(&cwd, size, small));
        match &claude {
            Some(claude) => {
                results.push(scenario_version(&cwd, size, claude));
                results.push(scenario_interactive(&cwd, size, small, claude));
            }
            None => results.push(verdict("(b)(c) claude", false, "claude not found on PATH")),
        }
    }

    std::thread::sleep(Duration::from_millis(500));
    let cleanup = std::fs::remove_dir_all(&cwd);
    println!("\ntemp dir removed: {}", cleanup.is_ok());
    let failed = results.iter().filter(|ok| !**ok).count();
    println!(
        "SUMMARY: {} passed, {failed} failed",
        results.len() - failed
    );
    std::process::exit(if failed == 0 { 0 } else { 1 });
}

/// `canonicalize` yields `\\?\C:\...` on Windows, which cmd.exe refuses as a cwd.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(stripped) => PathBuf::from(stripped),
        None => path,
    }
}
