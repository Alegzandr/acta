//! The GPUI side of the terminal: [`TerminalView`] owns a [`Session`] and
//! draws it with [`TerminalElement`] (ADR 0002, rendering plan).
//!
//! Colours come from [`TerminalTheme`] (docs/design/color.md 2.4), the face is
//! JetBrains Mono NL at `type.terminal` 13 / 17 (docs/design/typography.md),
//! adjustable 11 to 18. The app registers the bundled fonts.

use std::ops::Range;
use std::time::Instant;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::Side;
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui_kit::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, Font, FontFallbacks,
    FontStyle, FontWeight, GlobalElementId, Hsla, InspectorElementId, InteractiveElement,
    IntoElement, KeyDownEvent, LayoutId, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, ParentElement, Pixels, Point, Render, ScrollEvent, ShapedLine,
    SharedString, StrikethroughStyle, Styled, Task, TextAlign, TextRun, UTF16Selection,
    UnderlineStyle, Window, div, fill, font, outline, point, px, relative, rgb, rgba, size,
};

use crate::boxes::{BoxGlyph, box_glyph};
use crate::keys::{self, Command, Key, Mods, Os, Route};
use crate::launch::LaunchSpec;
use crate::mouse::{self, Action, Button};
use crate::session::{Frame, GridSize, Link, Session, SessionEvent};
use crate::theme::{SELECTION_ALPHA, TerminalTheme};

/// The key context of a focused terminal. The app's bindings that must not
/// fire inside the terminal are scoped with `!Terminal`.
pub const KEY_CONTEXT: &str = "Terminal";
pub const FONT_FAMILY: &str = "JetBrains Mono NL";
/// The platform monospace faces (docs/design/typography.md, fallback order).
const FONT_FALLBACKS: [&str; 4] = ["Cascadia Mono", "Menlo", "DejaVu Sans Mono", "Consolas"];
pub const DEFAULT_TEXT_SIZE: f32 = 13.0;
pub const MIN_TEXT_SIZE: f32 = 11.0;
pub const MAX_TEXT_SIZE: f32 = 18.0;
/// `type.terminal` is 13 / 17; the line keeps that ratio at every size.
const LINE_RATIO: f32 = 17.0 / 13.0;
/// The line under the last screen (docs/design/screens.md, terminal).
pub const DEFAULT_ENDED_MESSAGE: &str = "Session ended. Enter to restart.";
const RESIZE_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(75);

gpui_kit::actions!(
    terminal,
    [
        /// Copy the selection.
        Copy,
        /// Paste the clipboard into the child (bracketed when asked).
        Paste,
        /// "Increase terminal text size".
        IncreaseTextSize,
        /// "Decrease terminal text size".
        DecreaseTextSize
    ]
);

/// What the view tells its owner.
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalEvent {
    TitleChanged(Option<String>),
    Bell,
    /// The child exited, with its code when it has one.
    Exited(Option<i32>),
    /// Enter was pressed on an ended session. The owner calls
    /// [`TerminalView::restart`] with the folders of the page open now.
    RestartRequested,
    /// The text size changed (keys or palette); the owner persists it.
    TextSizeChanged(f32),
}

/// Where the grid sits on screen, from the last prepaint.
#[derive(Clone, Copy, Debug, PartialEq)]
struct GridLayout {
    origin: Point<Pixels>,
    cell: gpui_kit::Size<Pixels>,
    cols: usize,
    rows: usize,
}

impl GridLayout {
    /// The viewport cell under a window position, and the half it is in.
    fn cell_at(&self, position: Point<Pixels>) -> (usize, usize, Side) {
        let x = ((position.x - self.origin.x) / self.cell.width).max(0.0);
        let y = ((position.y - self.origin.y) / self.cell.height).max(0.0);
        let col = (x as usize).min(self.cols.saturating_sub(1));
        let row = (y as usize).min(self.rows.saturating_sub(1));
        let side = if x.fract() < 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        (row, col, side)
    }
}

pub struct TerminalView {
    spec: LaunchSpec,
    session: Option<Session>,
    /// Bumped on every (re)start, so a late spawn result is dropped.
    generation: u64,
    exited: Option<Option<i32>>,
    spawn_error: Option<String>,
    theme: TerminalTheme,
    text_size: f32,
    ended_message: SharedString,
    title: Option<String>,
    focus: FocusHandle,
    was_focused: bool,
    layout: Option<GridLayout>,
    hovered_link: Option<Link>,
    /// The mouse button the child is being told about, while held.
    reported_button: Option<Button>,
    selecting: bool,
    /// IME composition in progress, drawn at the cursor.
    marked: Option<String>,
    scroll_remainder: f32,
    /// When the session last printed something (idle detection).
    last_output: Instant,
    /// Started before the first layout: spawned once the grid size is known.
    start_pending: bool,
    _start: Task<()>,
    _resize: Task<()>,
    _events: Task<()>,
}

impl EventEmitter<TerminalEvent> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TerminalView {
    /// Start `spec` in a new terminal. The process is spawned off the UI
    /// thread; output appears when it arrives.
    pub fn spawn(spec: LaunchSpec, cx: &mut Context<Self>) -> Self {
        let mut view = TerminalView {
            spec: spec.clone(),
            session: None,
            generation: 0,
            exited: None,
            spawn_error: None,
            theme: TerminalTheme::dark(),
            text_size: DEFAULT_TEXT_SIZE,
            ended_message: DEFAULT_ENDED_MESSAGE.into(),
            title: None,
            focus: cx.focus_handle(),
            was_focused: false,
            layout: None,
            hovered_link: None,
            reported_button: None,
            selecting: false,
            marked: None,
            scroll_remainder: 0.0,
            last_output: Instant::now(),
            start_pending: false,
            _start: Task::ready(()),
            _resize: Task::ready(()),
            _events: Task::ready(()),
        };
        view.start(spec, cx);
        view
    }

    /// End the current session (killing its child) and start `spec`.
    pub fn restart(&mut self, spec: LaunchSpec, cx: &mut Context<Self>) {
        self.start(spec, cx);
        cx.notify();
    }

    /// The spec of the current session.
    pub fn spec(&self) -> &LaunchSpec {
        &self.spec
    }

    /// Whether the child is still running ("Restart the terminal?" asks
    /// only then, interaction.md 6.4).
    pub fn is_running(&self) -> bool {
        self.session.is_some() && self.exited.is_none()
    }

    /// Whether Claude Code sits idle at an empty prompt (see
    /// [`crate::idle::claude_idle`]): only then may a command be typed in.
    pub fn idle_at_prompt(&self) -> bool {
        let Some(session) = self.session.as_ref().filter(|_| self.exited.is_none()) else {
            return false;
        };
        let Some(cursor) = session.cursor() else {
            return false;
        };
        let screen = session.text();
        let row = screen.lines().nth(cursor.row).unwrap_or_default();
        crate::idle::claude_idle(&screen, row, self.last_output.elapsed())
    }

    /// Type `line` and Enter into the session, as if from the keyboard.
    /// Callers check [`TerminalView::idle_at_prompt`] first.
    pub fn send_line(&mut self, line: &str) {
        if let Some(session) = &self.session {
            let mut bytes = line.as_bytes().to_vec();
            bytes.push(b'\r');
            session.input(bytes);
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_theme(&mut self, theme: TerminalTheme, cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            session.set_theme(theme.clone());
        }
        self.theme = theme;
        cx.notify();
    }

    pub fn text_size(&self) -> f32 {
        self.text_size
    }

    pub fn set_text_size(&mut self, size: f32, cx: &mut Context<Self>) {
        let size = size.round().clamp(MIN_TEXT_SIZE, MAX_TEXT_SIZE);
        if size != self.text_size {
            self.text_size = size;
            cx.emit(TerminalEvent::TextSizeChanged(size));
            cx.notify();
        }
    }

    /// The translated "Session ended. {keys} to restart." line.
    pub fn set_ended_message(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.ended_message = message;
        cx.notify();
    }

    fn start(&mut self, spec: LaunchSpec, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.session = None;
        self.exited = None;
        self.spawn_error = None;
        self.title = None;
        self.hovered_link = None;
        self.spec = spec.clone();
        self._events = Task::ready(());
        // Spawn at the real grid size: a resize right after start makes
        // ConPTY repaint, and the child would start at the wrong width.
        if self.layout.is_none() {
            self.start_pending = true;
            return;
        }
        self.start_pending = false;
        let size = self.grid_size();
        let theme = self.theme.clone();
        let spawn = cx
            .background_executor()
            .spawn(async move { Session::spawn(&spec, size, theme) });
        self._start = cx.spawn(async move |this, cx| {
            let result = spawn.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(session) => this.attach(session, cx),
                    Err(err) => this.spawn_error = Some(err.to_string()),
                }
                cx.notify();
            });
        });
    }

    fn attach(&mut self, session: Session, cx: &mut Context<Self>) {
        // The pane may have been laid out while the process started.
        session.resize(self.grid_size());
        session.set_focused(self.was_focused);
        let events = session.events();
        self.session = Some(session);
        self._events = cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |this, cx| this.on_session_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    fn on_session_event(&mut self, event: SessionEvent, cx: &mut Context<Self>) {
        match event {
            SessionEvent::Wakeup => {
                self.last_output = Instant::now();
                if let Some(session) = &self.session {
                    session.take_wakeup();
                }
                cx.notify();
            }
            SessionEvent::Title => {
                if let Some(title) = self.session.as_ref().and_then(Session::take_title) {
                    self.title = title.clone();
                    cx.emit(TerminalEvent::TitleChanged(title));
                }
            }
            SessionEvent::Bell => cx.emit(TerminalEvent::Bell),
            SessionEvent::ClipboardStore(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            SessionEvent::Exited(code) => {
                self.exited = Some(code);
                cx.emit(TerminalEvent::Exited(code));
                cx.notify();
            }
        }
    }

    fn grid_size(&self) -> GridSize {
        match self.layout {
            Some(layout) => GridSize {
                cols: layout.cols as u16,
                rows: layout.rows as u16,
                cell_width: f32::from(layout.cell.width).round() as u16,
                cell_height: f32::from(layout.cell.height).round() as u16,
            },
            None => GridSize::new(80, 24),
        }
    }

    fn set_layout(&mut self, layout: GridLayout, cx: &mut Context<Self>) {
        if self.layout == Some(layout) {
            return;
        }
        self.layout = Some(layout);
        // Debounced (ADR 0002): a dragged divider resizes the PTY once it
        // rests, not on every frame. The layout itself applies at once.
        if self.session.is_some() {
            let size = self.grid_size();
            let timer = cx.background_executor().timer(RESIZE_DEBOUNCE);
            self._resize = cx.spawn(async move |this, cx| {
                timer.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(session) = &this.session {
                        session.resize(size);
                        cx.notify();
                    }
                });
            });
        }
        if self.start_pending {
            self.start(self.spec.clone(), cx);
        }
    }

    fn mode(&self) -> TermMode {
        self.session
            .as_ref()
            .map(Session::mode)
            .unwrap_or(TermMode::NONE)
    }

    fn live_session(&self) -> Option<&Session> {
        self.session.as_ref().filter(|_| self.exited.is_none())
    }

    fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.session.as_ref().and_then(Session::selection_text) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        if let Some(session) = self.live_session() {
            session.input(keys::paste_bytes(&text, session.mode()));
        }
    }

    fn larger(&mut self, _: &IncreaseTextSize, _window: &mut Window, cx: &mut Context<Self>) {
        self.set_text_size(self.text_size + 1.0, cx);
    }

    fn smaller(&mut self, _: &DecreaseTextSize, _window: &mut Window, cx: &mut Context<Self>) {
        self.set_text_size(self.text_size - 1.0, cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let key = Key {
            key: &keystroke.key,
            key_char: keystroke.key_char.as_deref(),
            mods: mods(&keystroke.modifiers),
        };
        if self.exited.is_some() || self.spawn_error.is_some() {
            let route = keys::route(&key, TermMode::NONE, Os::CURRENT);
            if route == Route::App {
                return;
            }
            if let Route::Command(command) = route {
                self.run_command(command, window, cx);
            } else if key.key == "enter" && key.mods == Mods::default() {
                cx.emit(TerminalEvent::RestartRequested);
            }
            cx.stop_propagation();
            return;
        }
        let mode = self.mode();
        if !mode.contains(TermMode::ALT_SCREEN) && key.mods == shift_only() {
            let scroll = match key.key {
                "pageup" => Some(Scroll::PageUp),
                "pagedown" => Some(Scroll::PageDown),
                "home" => Some(Scroll::Top),
                "end" => Some(Scroll::Bottom),
                _ => None,
            };
            if let (Some(scroll), Some(session)) = (scroll, &self.session) {
                session.scroll(scroll);
                cx.notify();
                cx.stop_propagation();
                return;
            }
        }
        match keys::route(&key, mode, Os::CURRENT) {
            Route::App => {}
            Route::Command(command) => {
                self.run_command(command, window, cx);
                cx.stop_propagation();
            }
            Route::Send(bytes) => {
                if let Some(session) = &self.session {
                    session.input(bytes);
                }
                cx.stop_propagation();
            }
            // Delivered by the platform through the input handler.
            Route::Text => {}
            Route::Ignore => cx.stop_propagation(),
        }
    }

    fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::Copy => self.copy(&Copy, window, cx),
            Command::Paste => self.paste(&Paste, window, cx),
            Command::TextLarger => self.larger(&IncreaseTextSize, window, cx),
            Command::TextSmaller => self.smaller(&DecreaseTextSize, window, cx),
        }
    }

    fn report_mouse(
        &self,
        button: Button,
        action: Action,
        mods: Mods,
        row: usize,
        col: usize,
    ) -> bool {
        let Some(session) = self.live_session() else {
            return false;
        };
        let mode = session.mode();
        if !mouse::wants(mode, action, mods) {
            return false;
        }
        session.write(mouse::report(mode, button, action, mods, col, row));
        true
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let Some(layout) = self.layout else { return };
        let (row, col, side) = layout.cell_at(event.position);
        let mods = mods(&event.modifiers);
        let button = match event.button {
            MouseButton::Left => Button::Left,
            MouseButton::Middle => Button::Middle,
            MouseButton::Right => Button::Right,
            _ => return,
        };
        if button == Button::Left
            && link_modifier(&event.modifiers)
            && let Some(link) = self.session.as_ref().and_then(|s| s.link_at(row, col))
            && crate::url::is_openable(&link.uri)
        {
            cx.open_url(&link.uri);
            return;
        }
        if self.report_mouse(button, Action::Press, mods, row, col) {
            self.reported_button = Some(button);
            return;
        }
        if button != Button::Left {
            return;
        }
        let Some(session) = &self.session else { return };
        let ty = match event.click_count {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        session.start_selection(row, col, side, ty);
        self.selecting = true;
        cx.notify();
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(button), Some(layout)) = (self.reported_button.take(), self.layout) {
            let (row, col, _) = layout.cell_at(event.position);
            self.report_mouse(button, Action::Release, mods(&event.modifiers), row, col);
            return;
        }
        if self.selecting {
            self.selecting = false;
            // A click without a drag leaves nothing selected.
            if let Some(session) = &self.session
                && session.selection_text().is_none()
            {
                session.clear_selection();
                cx.notify();
            }
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(layout) = self.layout else { return };
        let (row, col, side) = layout.cell_at(event.position);
        let mods = mods(&event.modifiers);
        if let Some(button) = self.reported_button {
            self.report_mouse(button, Action::Motion(Some(button)), mods, row, col);
            return;
        }
        if self.selecting {
            if let Some(session) = &self.session {
                session.update_selection(row, col, side);
                cx.notify();
            }
            return;
        }
        if event.pressed_button.is_none() {
            self.report_mouse(Button::Left, Action::Motion(None), mods, row, col);
        }
        let link = if link_modifier(&event.modifiers) {
            self.session.as_ref().and_then(|s| s.link_at(row, col))
        } else {
            None
        };
        if link != self.hovered_link {
            self.hovered_link = link;
            cx.notify();
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.layout else { return };
        let line_height = layout.cell.height;
        self.scroll_remainder += f32::from(event.delta.pixel_delta(line_height).y);
        let lines = (self.scroll_remainder / f32::from(line_height)).trunc();
        if lines == 0.0 {
            return;
        }
        self.scroll_remainder -= lines * f32::from(line_height);
        let lines = lines as i32;
        let Some(session) = &self.session else { return };
        let mode = session.mode();
        let (row, col, _) = layout.cell_at(event.position);
        let mods = mods(&event.modifiers);
        let button = if lines > 0 {
            Button::WheelUp
        } else {
            Button::WheelDown
        };
        if self.exited.is_none() && mouse::wants(mode, Action::Press, mods) {
            for _ in 0..lines.unsigned_abs() {
                session.write(mouse::report(mode, button, Action::Press, mods, col, row));
            }
        } else if self.exited.is_none()
            && mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
            && !mods.shift
        {
            // A full-screen program without mouse tracking: arrow keys.
            let arrow: &[u8] = match (lines > 0, mode.contains(TermMode::APP_CURSOR)) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            session.write(arrow.repeat(lines.unsigned_abs() as usize));
        } else {
            session.scroll(Scroll::Delta(lines));
            cx.notify();
        }
    }

    fn sync_focus(&mut self, window: &Window) {
        let focused = self.focus.is_focused(window);
        if focused != self.was_focused {
            self.was_focused = focused;
            if let Some(session) = self.live_session() {
                session.set_focused(focused);
            }
        }
    }
}

fn mods(modifiers: &Modifiers) -> Mods {
    Mods {
        ctrl: modifiers.control,
        alt: modifiers.alt,
        shift: modifiers.shift,
        platform: modifiers.platform,
    }
}

fn shift_only() -> Mods {
    Mods {
        shift: true,
        ..Mods::default()
    }
}

/// Mod+click opens a link: Cmd on macOS, Ctrl elsewhere.
fn link_modifier(modifiers: &Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.platform
    } else {
        modifiers.control
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_focus(window);
        let cursor = if self.hovered_link.is_some() {
            CursorStyle::PointingHand
        } else {
            CursorStyle::IBeam
        };
        div()
            .id("terminal")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .cursor(cursor)
            .bg(color(self.theme.background))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::larger))
            .on_action(cx.listener(Self::smaller))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(TerminalElement { view: cx.entity() })
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|text| 0..text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = None;
        if let Some(session) = self.live_session() {
            session.input(text.as_bytes().to_vec());
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = (!new_text.is_empty()).then(|| new_text.to_owned());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // The IME window opens at the cursor cell.
        let layout = self.layout?;
        let cursor = self.session.as_ref()?.cursor()?;
        Some(Bounds::new(
            point(
                layout.origin.x + layout.cell.width * cursor.col as f32,
                layout.origin.y + layout.cell.height * cursor.row as f32,
            ),
            layout.cell,
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

fn color(value: u32) -> Hsla {
    rgb(value).into()
}

fn color_alpha(value: u32, alpha: f32) -> Hsla {
    rgba((value << 8) | (alpha * 255.0).round() as u32).into()
}

/// Draws the grid of a [`TerminalView`].
struct TerminalElement {
    view: Entity<TerminalView>,
}

impl IntoElement for TerminalElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

#[derive(Default)]
struct Painted {
    quads: Vec<PaintQuad>,
    lines: Vec<(Point<Pixels>, ShapedLine)>,
    /// Drawn after the text: link underline and cursor.
    overlay: Vec<PaintQuad>,
    overlay_lines: Vec<(Point<Pixels>, ShapedLine)>,
    line_height: Pixels,
}

#[derive(Clone, PartialEq)]
struct RunStyle {
    fg: u32,
    bold: bool,
    italic: bool,
    underline: bool,
    undercurl: bool,
    strike: bool,
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = Painted;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui_kit::Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Painted {
        let (text_size, theme) = {
            let view = self.view.read(cx);
            (view.text_size, view.theme.clone())
        };
        let font_size = px(text_size);
        let base_font = terminal_font();
        let text_system = window.text_system().clone();
        let font_id = text_system.resolve_font(&base_font);
        let cell_width = text_system
            .advance(font_id, font_size, 'm')
            .map(|s| s.width)
            .unwrap_or(px(text_size * 0.6));
        let line_height = px((text_size * LINE_RATIO).round());
        let cols = (f32::from(bounds.size.width) / f32::from(cell_width))
            .floor()
            .max(2.0) as usize;
        let rows = (f32::from(bounds.size.height) / f32::from(line_height))
            .floor()
            .max(1.0) as usize;
        let layout = GridLayout {
            origin: bounds.origin,
            cell: size(cell_width, line_height),
            cols,
            rows,
        };
        let focused = self.view.read(cx).focus.is_focused(window);
        let frame = self.view.update(cx, |view, cx| {
            view.set_layout(layout, cx);
            view.session.as_ref().map(|s| s.frame(&view.theme))
        });
        let view = self.view.read(cx);

        let mut painted = Painted {
            line_height,
            ..Painted::default()
        };
        let cell_origin = |row: usize, col: usize| {
            point(
                bounds.origin.x + cell_width * col as f32,
                bounds.origin.y + line_height * row as f32,
            )
        };
        let shape = |text: String, style: &RunStyle, color: u32| -> ShapedLine {
            let mut font = base_font.clone();
            if style.bold {
                font.weight = FontWeight::BOLD;
            }
            if style.italic {
                font.style = FontStyle::Italic;
            }
            let fg = self::color(color);
            let run = TextRun {
                len: text.len(),
                font,
                color: fg,
                background_color: None,
                underline: (style.underline || style.undercurl).then_some(UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(fg),
                    wavy: style.undercurl,
                }),
                strikethrough: style.strike.then_some(StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(fg),
                }),
            };
            text_system.shape_line(text.into(), font_size, &[run], None)
        };

        let mut last_row = 0;
        if let Some(frame) = &frame {
            paint_grid(
                frame,
                &theme,
                &mut painted,
                cell_origin,
                cell_width,
                line_height,
                &shape,
            );
            last_row = frame
                .cursor
                .map(|c| c.row)
                .unwrap_or(frame.rows.saturating_sub(1));

            if let Some(link) = &view.hovered_link {
                for (row, cols) in &link.cells {
                    let origin = cell_origin(*row, cols.start);
                    painted.overlay.push(fill(
                        Bounds::new(
                            point(origin.x, origin.y + line_height - px(2.0)),
                            size(cell_width * cols.len() as f32, px(1.0)),
                        ),
                        color(theme.accent),
                    ));
                }
            }

            if view.exited.is_none()
                && let Some(cursor) = frame.cursor
            {
                let origin = cell_origin(cursor.row, cursor.col);
                let width = cell_width * if cursor.wide { 2.0 } else { 1.0 };
                let accent = color(theme.accent);
                let block = Bounds::new(origin, size(width, line_height));
                match cursor.shape {
                    CursorShape::Block if focused => {
                        painted.overlay.push(fill(block, accent));
                        // The character under the cursor, in the background colour.
                        let cell = &frame.row(cursor.row)[cursor.col];
                        if cell.c != ' ' && box_glyph(cell.c).is_none() {
                            let style = run_style(cell.flags, theme.background);
                            let mut text = cell.c.to_string();
                            if let Some(extra) = &cell.extra {
                                text.push_str(extra);
                            }
                            painted
                                .overlay_lines
                                .push((origin, shape(text, &style, theme.background)));
                        }
                    }
                    CursorShape::Beam => painted.overlay.push(fill(
                        Bounds::new(origin, size(px(2.0), line_height)),
                        accent,
                    )),
                    CursorShape::Underline => painted.overlay.push(fill(
                        Bounds::new(
                            point(origin.x, origin.y + line_height - px(2.0)),
                            size(width, px(2.0)),
                        ),
                        accent,
                    )),
                    // Unfocused, or hollow by request: a 1px outline.
                    _ => painted
                        .overlay
                        .push(outline(block, accent, gpui_kit::BorderStyle::Solid)),
                }

                if let Some(marked) = &view.marked {
                    let style = RunStyle {
                        underline: true,
                        ..run_style(Flags::empty(), theme.foreground)
                    };
                    let line = shape(marked.clone(), &style, theme.foreground);
                    painted.overlay.push(fill(
                        Bounds::new(origin, size(line.width, line_height)),
                        color(theme.background),
                    ));
                    painted.overlay_lines.push((origin, line));
                }
            }
        }

        // The one line under the last screen, or the reason nothing started.
        let message = if let Some(err) = &view.spawn_error {
            Some(format!("Could not start {}: {err}", view.spec.program))
        } else {
            view.exited.map(|_| view.ended_message.to_string())
        };
        if let Some(message) = message {
            let row = if frame.is_some() {
                (last_row + 1).min(rows.saturating_sub(1))
            } else {
                0
            };
            let origin = cell_origin(row, 0);
            painted.overlay.push(fill(
                Bounds::new(origin, size(bounds.size.width, line_height)),
                color(theme.background),
            ));
            let style = run_style(Flags::empty(), theme.muted);
            painted
                .overlay_lines
                .push((origin, shape(message, &style, theme.muted)));
        }
        painted
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        painted: &mut Painted,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.view.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        window.with_content_mask(Some(gpui_kit::ContentMask { bounds }), |window| {
            for quad in painted.quads.drain(..) {
                window.paint_quad(quad);
            }
            for (origin, line) in &painted.lines {
                let _ = line.paint(
                    *origin,
                    painted.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for quad in painted.overlay.drain(..) {
                window.paint_quad(quad);
            }
            for (origin, line) in &painted.overlay_lines {
                let _ = line.paint(
                    *origin,
                    painted.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        });
    }
}

fn terminal_font() -> Font {
    let mut font = font(FONT_FAMILY);
    font.fallbacks = Some(FontFallbacks::from_fonts(
        FONT_FALLBACKS.iter().map(|f| (*f).to_owned()).collect(),
    ));
    font
}

fn run_style(flags: Flags, fg: u32) -> RunStyle {
    RunStyle {
        fg,
        bold: flags.contains(Flags::BOLD),
        italic: flags.contains(Flags::ITALIC),
        underline: flags.intersects(Flags::ALL_UNDERLINES) && !flags.contains(Flags::UNDERCURL),
        undercurl: flags.contains(Flags::UNDERCURL),
        strike: flags.contains(Flags::STRIKEOUT),
    }
}

/// Backgrounds, selection, box glyphs and text runs of every row.
fn paint_grid(
    frame: &Frame,
    theme: &TerminalTheme,
    painted: &mut Painted,
    cell_origin: impl Fn(usize, usize) -> Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    shape: &impl Fn(String, &RunStyle, u32) -> ShapedLine,
) {
    let span = |row: usize, cols: Range<usize>| {
        Bounds::new(
            cell_origin(row, cols.start),
            size(cell_width * cols.len() as f32, line_height),
        )
    };
    let selection = color_alpha(theme.accent, SELECTION_ALPHA);
    // Lines as quads: light 1px per 13px of text, heavy twice that.
    let light = (f32::from(line_height) / 17.0).round().max(1.0);

    for row in 0..frame.rows {
        let cells = frame.row(row);
        // Backgrounds that differ from the terminal's, merged per run.
        let mut col = 0;
        while col < frame.cols {
            let bg = cells[col].bg;
            let start = col;
            while col < frame.cols && cells[col].bg == bg {
                col += 1;
            }
            if bg != theme.background {
                painted.quads.push(fill(span(row, start..col), color(bg)));
            }
        }
        // Selection, text colours unchanged (color.md 2.4).
        let mut col = 0;
        while col < frame.cols {
            if !cells[col].selected {
                col += 1;
                continue;
            }
            let start = col;
            while col < frame.cols && cells[col].selected {
                col += 1;
            }
            painted.quads.push(fill(span(row, start..col), selection));
        }

        // Text: runs of narrow characters with one style, each placed at
        // its own column so the grid never drifts.
        let mut run: Option<(usize, String, RunStyle)> = None;
        let flush = |run: &mut Option<(usize, String, RunStyle)>, painted: &mut Painted| {
            if let Some((start, text, style)) = run.take() {
                let text = text.trim_end().to_owned();
                if !text.is_empty() {
                    let fg = style.fg;
                    painted
                        .lines
                        .push((cell_origin(row, start), shape(text, &style, fg)));
                }
            }
        };
        for (col, cell) in cells.iter().enumerate() {
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            if let Some(glyph) = box_glyph(cell.c) {
                flush(&mut run, painted);
                let origin = cell_origin(row, col);
                paint_box(
                    glyph,
                    origin,
                    cell_width,
                    line_height,
                    light,
                    cell.fg,
                    painted,
                );
                continue;
            }
            let style = run_style(cell.flags, cell.fg);
            let decorated = style.underline || style.undercurl || style.strike;
            if cell.c == ' ' && !decorated {
                if let Some((start, text, _)) = &mut run
                    && *start + text.chars().count() == col
                {
                    text.push(' ');
                }
                continue;
            }
            let narrow =
                cell.c.is_ascii() && cell.extra.is_none() && !cell.flags.contains(Flags::WIDE_CHAR);
            if narrow {
                if let Some((start, text, run_style)) = &mut run
                    && *run_style == style
                    && *start + text.chars().count() == col
                {
                    text.push(cell.c);
                    continue;
                }
                flush(&mut run, painted);
                run = Some((col, cell.c.to_string(), style));
            } else {
                flush(&mut run, painted);
                let mut text = cell.c.to_string();
                if let Some(extra) = &cell.extra {
                    text.push_str(extra);
                }
                let fg = style.fg;
                painted
                    .lines
                    .push((cell_origin(row, col), shape(text, &style, fg)));
            }
        }
        flush(&mut run, painted);
    }
}

fn paint_box(
    glyph: BoxGlyph,
    origin: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    light: f32,
    fg: u32,
    painted: &mut Painted,
) {
    let (w, h) = (f32::from(cell_width), f32::from(line_height));
    let at = |x0: f32, y0: f32, x1: f32, y1: f32| {
        Bounds::from_corners(
            point(origin.x + px(x0), origin.y + px(y0)),
            point(origin.x + px(x1), origin.y + px(y1)),
        )
    };
    match glyph {
        BoxGlyph::Rects(rects) => {
            for r in rects {
                // Snap to whole pixels so neighbouring cells meet exactly.
                let bounds = at(
                    (r.x0 * w).round(),
                    (r.y0 * h).round(),
                    (r.x1 * w).round(),
                    (r.y1 * h).round(),
                );
                painted.quads.push(fill(bounds, color_alpha(fg, r.alpha)));
            }
        }
        BoxGlyph::Arms([up, down, left, right]) => {
            let cx = (w / 2.0).floor();
            let cy = (h / 2.0).floor();
            let fg = color(fg);
            // Each arm runs from its edge through the centre band, so arms
            // of any weight meet without a gap.
            let band = |weight: u8, centre: f32| {
                let t = light * weight as f32;
                let start = centre - (t / 2.0).floor();
                (start, start + t)
            };
            if up > 0 {
                let (x0, x1) = band(up, cx);
                painted
                    .quads
                    .push(fill(at(x0, 0.0, x1, band(up, cy).1), fg));
            }
            if down > 0 {
                let (x0, x1) = band(down, cx);
                painted
                    .quads
                    .push(fill(at(x0, band(down, cy).0, x1, h), fg));
            }
            if left > 0 {
                let (y0, y1) = band(left, cy);
                painted
                    .quads
                    .push(fill(at(0.0, y0, band(left, cx).1, y1), fg));
            }
            if right > 0 {
                let (y0, y1) = band(right, cy);
                painted
                    .quads
                    .push(fill(at(band(right, cx).0, y0, w, y1), fg));
            }
        }
    }
}
