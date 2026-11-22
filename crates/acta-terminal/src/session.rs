//! A child process in a PTY, emulated by `alacritty_terminal` (ADR 0002).
//! GPUI-free: the view reads [`Frame`]s and sends bytes, sizes and selections.
//!
//! Locking rule: the PTY reader thread holds the `Term` lock while it parses,
//! so every method here takes it briefly and never across a paint.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode, viewport_to_point};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::CursorShape;

use crate::launch::LaunchSpec;
use crate::theme::TerminalTheme;
use crate::url;

/// Lines of scrollback (ADR 0002).
pub const SCROLLBACK: usize = 10_000;

/// Grid size in cells, plus the cell size in pixels for size reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl GridSize {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(2),
            rows: rows.max(1),
            cell_width: 8,
            cell_height: 17,
        }
    }
}

impl Dimensions for GridSize {
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

impl From<GridSize> for WindowSize {
    fn from(s: GridSize) -> Self {
        WindowSize {
            num_lines: s.rows,
            num_cols: s.cols,
            cell_width: s.cell_width,
            cell_height: s.cell_height,
        }
    }
}

/// What the view hears from a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    /// New content; coalesced, so at most one is queued at a time.
    Wakeup,
    /// The title changed; read it with [`Session::take_title`]. Coalesced
    /// like `Wakeup`.
    Title,
    Bell,
    /// OSC 52: the child asks to put text on the clipboard. The view
    /// applies [`clipboard_store_allowed`] before writing it.
    ClipboardStore(String),
    /// The child exited; `None` when there is no code (killed by a signal).
    Exited(Option<i32>),
}

/// The largest OSC 52 store accepted, in bytes.
pub const CLIPBOARD_STORE_MAX: usize = 64 * 1024;
/// How recent the user's last key in the terminal must be for an OSC 52
/// store to be accepted.
pub const CLIPBOARD_STORE_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// Whether an OSC 52 store from the child reaches the system clipboard:
/// only while the terminal is focused, within [`CLIPBOARD_STORE_WINDOW`] of
/// a key pressed in it (Claude Code copies on the user's command), and up
/// to [`CLIPBOARD_STORE_MAX`] bytes. Otherwise any program printing to the
/// terminal (a `cat` of a hostile file, a dependency's install script)
/// could replace what the user is about to paste.
pub fn clipboard_store_allowed(
    focused: bool,
    last_key: Option<std::time::Instant>,
    now: std::time::Instant,
    len: usize,
) -> bool {
    focused
        && len <= CLIPBOARD_STORE_MAX
        && last_key.is_some_and(|at| now.saturating_duration_since(at) <= CLIPBOARD_STORE_WINDOW)
}

struct Shared {
    pty: OnceLock<EventLoopSender>,
    size: Mutex<GridSize>,
    theme: Mutex<TerminalTheme>,
    wake_queued: AtomicBool,
    /// The latest title not yet taken: `Some(None)` is a reset.
    title: Mutex<Option<Option<String>>>,
    exit_seen: AtomicBool,
}

#[derive(Clone)]
struct Listener {
    events: async_channel::Sender<SessionEvent>,
    shared: Arc<Shared>,
}

impl Listener {
    fn reply(&self, text: String) {
        if let Some(pty) = self.shared.pty.get() {
            let _ = pty.send(Msg::Input(Cow::Owned(text.into_bytes())));
        }
    }

    fn emit(&self, event: SessionEvent) {
        let _ = self.events.try_send(event);
    }

    fn title(&self, title: Option<String>) {
        if self.shared.title.lock().unwrap().replace(title).is_none() {
            self.emit(SessionEvent::Title);
        }
    }

    fn exited(&self, code: Option<i32>) {
        if !self.shared.exit_seen.swap(true, Ordering::AcqRel) {
            self.emit(SessionEvent::Exited(code));
        }
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            // Replies to the child's queries: without them Claude Code's
            // start-up queries go unanswered (ADR 0002, consequences).
            Event::PtyWrite(text) => self.reply(text),
            Event::ColorRequest(index, format) => {
                // Called with the Term lock held: read only our own theme.
                let theme = self.shared.theme.lock().unwrap().clone();
                let rgb = theme.query(index, &Default::default());
                self.reply(format(rgb));
            }
            Event::TextAreaSizeRequest(format) => {
                let size = *self.shared.size.lock().unwrap();
                self.reply(format(size.into()));
            }
            Event::Wakeup | Event::MouseCursorDirty | Event::CursorBlinkingChange => {
                if !self.shared.wake_queued.swap(true, Ordering::AcqRel) {
                    self.emit(SessionEvent::Wakeup);
                }
            }
            Event::Title(title) => self.title(Some(title)),
            Event::ResetTitle => self.title(None),
            Event::Bell => self.emit(SessionEvent::Bell),
            Event::ClipboardStore(_, text) => self.emit(SessionEvent::ClipboardStore(text)),
            Event::ChildExit(status) => self.exited(status.code()),
            // The event loop stopped (the PTY closed) without a child exit:
            // the session is over all the same.
            Event::Exit => self.exited(None),
            // Reading the clipboard from the child (OSC 52 query) is refused:
            // it would hand any program the user's clipboard.
            Event::ClipboardLoad(..) => {}
        }
    }
}

/// One cell of a [`Frame`], colours resolved, inverse and hidden applied.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameCell {
    pub c: char,
    /// Combining characters drawn on top of `c`.
    pub extra: Option<String>,
    pub fg: u32,
    pub bg: u32,
    pub flags: Flags,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCursor {
    pub row: usize,
    pub col: usize,
    pub shape: CursorShape,
    /// The cursor sits on a wide character.
    pub wide: bool,
}

/// The visible screen, copied out under one short lock.
#[derive(Clone, Debug)]
pub struct Frame {
    pub cols: usize,
    pub rows: usize,
    /// `rows * cols` cells, row-major.
    pub cells: Vec<FrameCell>,
    pub cursor: Option<FrameCursor>,
    pub display_offset: usize,
    pub mode: TermMode,
}

impl Frame {
    pub fn row(&self, row: usize) -> &[FrameCell] {
        &self.cells[row * self.cols..(row + 1) * self.cols]
    }
}

/// A link under the pointer: its target and the cells it covers, as
/// `(viewport row, column range)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub uri: String,
    pub cells: Vec<(usize, Range<usize>)>,
}

pub struct Session {
    term: Arc<FairMutex<Term<Listener>>>,
    pty: EventLoopSender,
    shared: Arc<Shared>,
    events: async_channel::Receiver<SessionEvent>,
}

impl Session {
    pub fn spawn(spec: &LaunchSpec, size: GridSize, theme: TerminalTheme) -> io::Result<Session> {
        let (tx, events) = async_channel::unbounded();
        let shared = Arc::new(Shared {
            pty: OnceLock::new(),
            size: Mutex::new(size),
            theme: Mutex::new(theme),
            wake_queued: AtomicBool::new(false),
            title: Mutex::new(None),
            exit_seen: AtomicBool::new(false),
        });
        let listener = Listener {
            events: tx,
            shared: shared.clone(),
        };
        let config = Config {
            scrolling_history: SCROLLBACK,
            kitty_keyboard: true,
            ..Config::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, listener.clone())));

        let mut env = HashMap::from([
            ("TERM".to_owned(), "xterm-256color".to_owned()),
            ("COLORTERM".to_owned(), "truecolor".to_owned()),
            ("TERM_PROGRAM".to_owned(), "Acta".to_owned()),
            (
                "TERM_PROGRAM_VERSION".to_owned(),
                env!("CARGO_PKG_VERSION").to_owned(),
            ),
        ]);
        env.extend(spec.env.iter().cloned());
        let options = tty::Options {
            shell: Some(tty::Shell::new(
                program_word(&spec.program),
                spec.args.clone(),
            )),
            working_directory: Some(spec.cwd.clone()),
            drain_on_exit: true,
            env,
            #[cfg(windows)]
            escape_args: !spec.verbatim_args,
        };
        let pty = tty::new(&options, size.into(), 0)?;
        let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)?;
        let sender = event_loop.channel();
        let _ = shared.pty.set(event_loop.channel());
        event_loop.spawn();
        Ok(Session {
            term,
            pty: sender,
            shared,
            events,
        })
    }

    /// The event stream. Every clone sees each event once.
    pub fn events(&self) -> async_channel::Receiver<SessionEvent> {
        self.events.clone()
    }

    /// The title set since the last call: `Some(None)` is a reset.
    pub fn take_title(&self) -> Option<Option<String>> {
        self.shared.title.lock().unwrap().take()
    }

    /// The cursor cell, without copying the screen.
    pub fn cursor(&self) -> Option<FrameCursor> {
        cursor_of(&self.term.lock())
    }

    /// Mark the queued wake-up as consumed, so the next output sends another.
    pub fn take_wakeup(&self) {
        self.shared.wake_queued.store(false, Ordering::Release);
    }

    pub fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.pty.send(Msg::Input(Cow::Owned(bytes)));
        }
    }

    /// Typing jumps back to the live screen, as in every terminal.
    pub fn input(&self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        {
            let mut term = self.term.lock();
            term.scroll_display(Scroll::Bottom);
            term.selection = None;
        }
        self.write(bytes);
    }

    pub fn size(&self) -> GridSize {
        *self.shared.size.lock().unwrap()
    }

    /// Resize the emulator first (it reflows), then the PTY.
    pub fn resize(&self, size: GridSize) {
        {
            let mut current = self.shared.size.lock().unwrap();
            if *current == size {
                return;
            }
            *current = size;
        }
        self.term.lock().resize(size);
        let _ = self.pty.send(Msg::Resize(size.into()));
    }

    pub fn set_theme(&self, theme: TerminalTheme) {
        *self.shared.theme.lock().unwrap() = theme;
    }

    pub fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    pub fn scroll(&self, scroll: Scroll) {
        self.term.lock().scroll_display(scroll);
    }

    /// Tell the child the focus changed, if it asked to know.
    pub fn set_focused(&self, focused: bool) {
        let report = {
            let mut term = self.term.lock();
            term.is_focused = focused;
            term.mode().contains(TermMode::FOCUS_IN_OUT)
        };
        if report {
            self.write(if focused { b"\x1b[I" } else { b"\x1b[O" }.to_vec());
        }
    }

    /// Start a selection at a viewport cell.
    pub fn start_selection(&self, row: usize, col: usize, side: Side, ty: SelectionType) {
        start_selection_of(&mut self.term.lock(), row, col, side, ty);
    }

    pub fn update_selection(&self, row: usize, col: usize, side: Side) {
        update_selection_of(&mut self.term.lock(), row, col, side);
    }

    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term
            .lock()
            .selection_to_string()
            .filter(|s| !s.is_empty())
    }

    /// The visible screen, resolved against `theme`.
    pub fn frame(&self, theme: &TerminalTheme) -> Frame {
        frame_of(&self.term.lock(), theme)
    }

    /// The visible screen as text, trailing blanks trimmed (tests, debugging).
    pub fn text(&self) -> String {
        text_of(&self.term.lock())
    }

    /// The link at a viewport cell: an OSC 8 hyperlink, or a URL in the text
    /// of the logical line (wrapped rows joined).
    pub fn link_at(&self, row: usize, col: usize) -> Option<Link> {
        link_of(&self.term.lock(), row, col)
    }

    /// The columns of the word around a viewport cell (double-click).
    pub fn word_bounds(&self, row: usize, col: usize) -> (usize, usize) {
        word_of(&self.term.lock(), row, col)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // The event loop closes the PTY, which ends the child.
        let _ = self.pty.send(Msg::Shutdown);
    }
}

fn frame_of<T: EventListener>(term: &Term<T>, theme: &TerminalTheme) -> Frame {
    let content = term.renderable_content();
    let cols = term.columns();
    let rows = term.screen_lines();
    let offset = content.display_offset as i32;
    let overrides = content.colors;
    let default_bg = theme.resolve(
        alacritty_terminal::vte::ansi::Color::Named(
            alacritty_terminal::vte::ansi::NamedColor::Background,
        ),
        overrides,
    );
    let blank = FrameCell {
        c: ' ',
        extra: None,
        fg: theme.foreground,
        bg: default_bg,
        flags: Flags::empty(),
        selected: false,
    };
    let mut cells = vec![blank; rows * cols];
    let selection = content.selection;
    let cursor_point = content.cursor.point;
    let cursor_shape = content.cursor.shape;
    let mut cursor_wide = false;
    for indexed in content.display_iter {
        let row = indexed.point.line.0 + offset;
        let col = indexed.point.column.0;
        if row < 0 || row as usize >= rows || col >= cols {
            continue;
        }
        let cell = indexed.cell;
        let mut fg = theme.resolve(cell.fg, overrides);
        let mut bg = theme.resolve(cell.bg, overrides);
        if cell.flags.contains(Flags::DIM) {
            fg = crate::theme::mix(fg, bg, 0.66);
        }
        if cell.flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if cell.flags.contains(Flags::HIDDEN) {
            fg = bg;
        }
        if indexed.point == cursor_point {
            cursor_wide = cell.flags.contains(Flags::WIDE_CHAR);
        }
        let selected =
            selection.is_some_and(|s| s.contains_cell(&indexed, cursor_point, cursor_shape));
        cells[row as usize * cols + col] = FrameCell {
            c: cell.c,
            extra: cell.zerowidth().map(|z| z.iter().collect()),
            fg,
            bg,
            flags: cell.flags,
            selected,
        };
    }
    let cursor_row = cursor_point.line.0 + offset;
    let cursor =
        (cursor_shape != CursorShape::Hidden && cursor_row >= 0 && (cursor_row as usize) < rows)
            .then_some(FrameCursor {
                row: cursor_row as usize,
                col: cursor_point.column.0.min(cols.saturating_sub(1)),
                shape: cursor_shape,
                wide: cursor_wide,
            });
    Frame {
        cols,
        rows,
        cells,
        cursor,
        display_offset: content.display_offset,
        mode: content.mode,
    }
}

fn cursor_of<T>(term: &Term<T>) -> Option<FrameCursor> {
    if !term.mode().contains(TermMode::SHOW_CURSOR) {
        return None;
    }
    let point = term.grid().cursor.point;
    let row = point.line.0 + term.grid().display_offset() as i32;
    if row < 0 || row as usize >= term.screen_lines() {
        return None;
    }
    let wide = term.grid()[point].flags.contains(Flags::WIDE_CHAR);
    Some(FrameCursor {
        row: row as usize,
        col: point.column.0,
        shape: term.cursor_style().shape,
        wide,
    })
}

/// Windows builds the command line by plain concatenation, so a program
/// path with a space or tab must arrive quoted. Unix spawns it as is.
fn program_word(program: &str) -> String {
    if cfg!(windows) && program.contains([' ', '\t']) && !program.starts_with('"') {
        format!("\"{program}\"")
    } else {
        program.to_owned()
    }
}

fn text_of<T: EventListener>(term: &Term<T>) -> String {
    let frame = frame_of(term, &TerminalTheme::dark());
    let mut out = String::new();
    for row in 0..frame.rows {
        let line: String = frame
            .row(row)
            .iter()
            .filter(|c| {
                !c.flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            })
            .map(|c| c.c)
            .collect();
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn link_of<T>(term: &Term<T>, row: usize, col: usize) -> Option<Link> {
    let rows = term.screen_lines();
    let cols = term.columns();
    if row >= rows || col >= cols {
        return None;
    }
    let offset = term.grid().display_offset() as i32;
    let line_of = |r: usize| Line(r as i32 - offset);
    let grid = term.grid();

    if let Some(link) = grid[line_of(row)][Column(col)].hyperlink() {
        // An OSC 8 target the gate refuses is no link at all.
        if !url::is_openable(link.uri()) {
            return None;
        }
        // The run of cells around `col` carrying the same link.
        let same = |c: usize| grid[line_of(row)][Column(c)].hyperlink().as_ref() == Some(&link);
        let start = (0..=col)
            .rev()
            .take_while(|&c| same(c))
            .last()
            .unwrap_or(col);
        let end = (col..cols).take_while(|&c| same(c)).last().unwrap_or(col) + 1;
        return Some(Link {
            uri: link.uri().to_owned(),
            cells: vec![(row, start..end)],
        });
    }

    // Join wrapped rows that are on screen.
    let wraps = |r: usize| {
        grid[line_of(r)][Column(cols - 1)]
            .flags
            .contains(Flags::WRAPLINE)
    };
    let mut first = row;
    while first > 0 && wraps(first - 1) {
        first -= 1;
    }
    let mut last = row;
    while last + 1 < rows && wraps(last) {
        last += 1;
    }
    let mut chars = Vec::new();
    let mut cells = Vec::new();
    for r in first..=last {
        for c in 0..cols {
            let cell = &grid[line_of(r)][Column(c)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            chars.push(cell.c);
            cells.push((r, c));
        }
    }
    let index = cells.iter().position(|&(r, c)| r == row && c >= col)?;
    let (range, uri) = url::url_at(&chars, index).filter(|(_, uri)| url::is_openable(uri))?;
    let mut spans: Vec<(usize, Range<usize>)> = Vec::new();
    for &(r, c) in &cells[range] {
        match spans.last_mut() {
            Some((last_row, span)) if *last_row == r => span.end = c + 1,
            _ => spans.push((r, c..c + 1)),
        }
    }
    Some(Link { uri, cells: spans })
}

fn word_of<T>(term: &Term<T>, row: usize, col: usize) -> (usize, usize) {
    let point = grid_point(term, row, col);
    let start = term.semantic_search_left(point);
    let end = term.semantic_search_right(point);
    let start = term.expand_wide(start, Direction::Left);
    let offset = term.grid().display_offset() as i32;
    let to_col = |p: Point| {
        if p.line.0 + offset == row as i32 {
            Some(p.column.0)
        } else {
            None
        }
    };
    (
        to_col(start).unwrap_or(0),
        to_col(end).unwrap_or(term.columns() - 1),
    )
}

fn start_selection_of<T>(
    term: &mut Term<T>,
    row: usize,
    col: usize,
    side: Side,
    ty: SelectionType,
) {
    let point = grid_point(term, row, col);
    term.selection = Some(Selection::new(ty, point, side));
}

fn update_selection_of<T>(term: &mut Term<T>, row: usize, col: usize, side: Side) {
    let point = grid_point(term, row, col);
    if let Some(selection) = term.selection.as_mut() {
        selection.update(point, side);
    }
}

fn grid_point<T>(term: &Term<T>, row: usize, col: usize) -> Point {
    let col = col.min(term.columns().saturating_sub(1));
    let row = row.min(term.screen_lines().saturating_sub(1));
    viewport_to_point(term.grid().display_offset(), Point::new(row, Column(col)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_osc_52_store_needs_focus_a_recent_key_and_a_small_payload() {
        use std::time::{Duration, Instant};
        let now = Instant::now();
        let key = Some(now - Duration::from_secs(1));
        assert!(clipboard_store_allowed(true, key, now, 10));
        assert!(clipboard_store_allowed(true, key, now, CLIPBOARD_STORE_MAX));
        // Not focused, no key yet, a key too long ago, too large.
        assert!(!clipboard_store_allowed(false, key, now, 10));
        assert!(!clipboard_store_allowed(true, None, now, 10));
        let old = Some(now - Duration::from_secs(6));
        assert!(!clipboard_store_allowed(true, old, now, 10));
        assert!(!clipboard_store_allowed(
            true,
            key,
            now,
            CLIPBOARD_STORE_MAX + 1
        ));
    }
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
    use std::time::{Duration, Instant};

    fn wait_for(session: &Session, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if session.text().contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    fn wait_exit(session: &Session) -> Option<Option<i32>> {
        let events = session.events();
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            while let Ok(event) = events.try_recv() {
                if let SessionEvent::Exited(code) = event {
                    return Some(code);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    fn spec(program: &str, args: &[&str]) -> LaunchSpec {
        LaunchSpec {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            cwd: std::env::temp_dir(),
            env: vec![("ACTA_TEST_NAME".to_owned(), "Alexandre".to_owned())],
            verbatim_args: false,
        }
    }

    #[cfg(windows)]
    #[test]
    fn pty_round_trip_with_cmd() {
        let session = Session::spawn(
            &spec("cmd.exe", &["/d", "/q", "/k"]),
            GridSize::new(100, 30),
            TerminalTheme::dark(),
        )
        .expect("spawn cmd.exe");
        session.input(b"echo hello %ACTA_TEST_NAME%\r".to_vec());
        assert!(wait_for(&session, "hello Alexandre"), "{}", session.text());

        // Resize reaches the child. `mode con` is localised, so ask
        // PowerShell; the typed command line itself cannot match `W=80`.
        session.resize(GridSize::new(80, 20));
        session.input(
            b"powershell -NoProfile -Command \"'W=' + $Host.UI.RawUI.WindowSize.Width\"\r".to_vec(),
        );
        assert!(wait_for(&session, "W=80"), "{}", session.text());

        session.input(b"exit 3\r".to_vec());
        assert_eq!(wait_exit(&session), Some(Some(3)));
    }

    #[cfg(unix)]
    #[test]
    fn pty_round_trip_with_sh() {
        let session = Session::spawn(
            &spec("/bin/sh", &[]),
            GridSize::new(100, 30),
            TerminalTheme::dark(),
        )
        .expect("spawn /bin/sh");
        session.input(b"echo hello $ACTA_TEST_NAME\n".to_vec());
        assert!(wait_for(&session, "hello Alexandre"), "{}", session.text());

        session.resize(GridSize::new(80, 20));
        session.input(b"stty size\n".to_vec());
        assert!(wait_for(&session, "20 80"), "{}", session.text());

        session.input(b"exit 3\n".to_vec());
        assert_eq!(wait_exit(&session), Some(Some(3)));
    }

    /// A program path with a space, and a `.cmd` shim given quoted words.
    #[cfg(windows)]
    #[test]
    fn windows_command_lines_survive_spaces() {
        use crate::launch::{ClaudeProgram, Focus, build_claude_launch};
        use std::path::PathBuf;

        let dir = std::env::temp_dir().join(format!("acta term {}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let system = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let cmd = dir.join("my cmd.exe");
        std::fs::copy(PathBuf::from(system).join("System32").join("cmd.exe"), &cmd).unwrap();
        let session = Session::spawn(
            &spec(
                &cmd.to_string_lossy(),
                &["/d", "/c", "echo spaced Alexandre & exit 4"],
            ),
            GridSize::new(100, 30),
            TerminalTheme::dark(),
        )
        .expect("spawn through a path with a space");
        assert_eq!(wait_exit(&session), Some(Some(4)));
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            session.text().contains("spaced Alexandre"),
            "{}",
            session.text()
        );
        drop(session);

        let shim = dir.join("claude shim.cmd");
        std::fs::write(&shim, "@echo off\r\necho args:[%*]\r\n").unwrap();
        let program = ClaudeProgram::CmdShim {
            comspec: std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into()),
            shim,
        };
        let folders = [dir.clone(), PathBuf::from("D:\\my docs")];
        // Acta's options go through the shim as separate quoted words too.
        let options = crate::launch::ClaudeOptions {
            plugin_dir: None,
            conversation: Some(crate::launch::Conversation::New(
                "019a7a6e-1c2d-7f00-8a00-0123456789ab".into(),
            )),
            token: None,
            prompt: None,
        };
        let launch =
            build_claude_launch(&program, &folders, &Focus::default(), &options, |_| true).unwrap();
        let session = Session::spawn(&launch.spec, GridSize::new(120, 30), TerminalTheme::dark())
            .expect("spawn the shim");
        assert_eq!(wait_exit(&session), Some(Some(0)));
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            session
                .text()
                .contains(r#"args:["--add-dir" "D:\my docs" "--session-id" "019a7a6e-1c2d-7f00-8a00-0123456789ab"]"#),
            "{}",
            session.text()
        );
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Arguments reach the program as separate words, quotes included.
    #[cfg(unix)]
    #[test]
    fn unix_arguments_stay_words() {
        let session = Session::spawn(
            &spec(
                "/bin/sh",
                &[
                    "-c",
                    "exec printf '<%s>\\n' \"$@\"",
                    "claude",
                    "a b",
                    "it's",
                ],
            ),
            GridSize::new(100, 30),
            TerminalTheme::dark(),
        )
        .expect("spawn /bin/sh");
        assert_eq!(wait_exit(&session), Some(Some(0)));
        std::thread::sleep(Duration::from_millis(300));
        let text = session.text();
        assert!(text.contains("<a b>") && text.contains("<it's>"), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn output_survives_a_quick_exit_unix() {
        let session = Session::spawn(
            &spec("/bin/sh", &["-c", "echo bye Alexandre; exit 2"]),
            GridSize::new(100, 30),
            TerminalTheme::dark(),
        )
        .expect("spawn /bin/sh");
        assert_eq!(wait_exit(&session), Some(Some(2)));
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            session.text().contains("bye Alexandre"),
            "{}",
            session.text()
        );
    }

    /// The last screen stays after the child exits (ADR 0002 point 3).
    #[cfg(windows)]
    #[test]
    fn output_survives_a_quick_exit() {
        let session = Session::spawn(
            &spec("cmd.exe", &["/d", "/c", "echo bye Alexandre & exit 2"]),
            GridSize::new(100, 30),
            TerminalTheme::dark(),
        )
        .expect("spawn cmd.exe");
        assert_eq!(wait_exit(&session), Some(Some(2)));
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            session.text().contains("bye Alexandre"),
            "{}",
            session.text()
        );
    }

    /// An emulator with no child, fed `text`.
    fn term_with(text: &str) -> Term<VoidListener> {
        let mut term = Term::new(Config::default(), &GridSize::new(40, 5), VoidListener);
        let mut parser = Processor::<StdSyncHandler>::new();
        parser.advance(&mut term, text.as_bytes());
        term
    }

    #[test]
    fn selection_copies_the_selected_text() {
        let mut term = term_with("one two three\r\nfour");
        start_selection_of(&mut term, 0, 4, Side::Left, SelectionType::Simple);
        update_selection_of(&mut term, 0, 6, Side::Right);
        assert_eq!(term.selection_to_string().as_deref(), Some("two"));
        let frame = frame_of(&term, &TerminalTheme::dark());
        assert!(frame.row(0)[5].selected);
        assert!(!frame.row(0)[8].selected);
        assert_eq!(word_of(&term, 0, 9), (8, 12));

        start_selection_of(&mut term, 0, 8, Side::Left, SelectionType::Lines);
        update_selection_of(&mut term, 1, 0, Side::Left);
        assert_eq!(
            term.selection_to_string().as_deref(),
            Some("one two three\nfour\n")
        );
    }

    #[test]
    fn links_are_found_across_a_wrapped_row() {
        // 40 columns: the URL wraps onto the second row.
        let url = "https://example.com/alexandre/a-very-long-path";
        let term = term_with(&format!("see {url} ok"));
        let link = link_of(&term, 1, 2).expect("link on the wrapped part");
        assert_eq!(link.uri, url);
        assert_eq!(link.cells, vec![(0, 4..40), (1, 0..10)]);
        assert_eq!(link_of(&term, 0, 1), None);
        assert_eq!(link_of(&term, 0, 5).map(|l| l.uri), Some(url.to_owned()));
    }

    #[test]
    fn osc8_hyperlinks_win() {
        let term = term_with("\x1b]8;;https://acta.example/\x1b\\docs\x1b]8;;\x1b\\ end");
        let link = link_of(&term, 0, 2).expect("hyperlink");
        assert_eq!(link.uri, "https://acta.example/");
        assert_eq!(link.cells, vec![(0, 0..4)]);
        assert_eq!(link_of(&term, 0, 6), None);

        // Only web links: anything else the system would open is no link.
        for target in [
            "file:///C:/Windows/System32/calc.exe",
            "javascript:alert(1)",
            "ms-msdt:/id PCWDiagnostic",
            "C:/Users/alexandre/run.bat",
            "//server/share/run.exe",
        ] {
            let term = term_with(&format!("\x1b]8;;{target}\x1b\\docs\x1b]8;;\x1b\\"));
            assert_eq!(link_of(&term, 0, 1), None, "{target}");
        }
        let term = term_with("see file:///etc/passwd and \\\\server\\share\\x.exe");
        assert_eq!(link_of(&term, 0, 8), None);
        assert_eq!(link_of(&term, 0, 30), None);
    }

    #[test]
    fn frame_resolves_colours_and_attributes() {
        let term = term_with("\x1b[31;1mR\x1b[0m\x1b[7mI\x1b[0m");
        let theme = TerminalTheme::dark();
        let frame = frame_of(&term, &theme);
        let row = frame.row(0);
        assert_eq!(row[0].c, 'R');
        assert_eq!(row[0].fg, theme.ansi[1]);
        assert!(row[0].flags.contains(Flags::BOLD));
        assert_eq!(row[1].fg, theme.background);
        assert_eq!(row[1].bg, theme.foreground);
    }
}
