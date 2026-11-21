//! Mouse reports to the child, for programs that asked for them (xterm
//! mouse tracking, SGR or the legacy encoding). GPUI-free.

use alacritty_terminal::term::TermMode;

use crate::keys::Mods;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Press,
    Release,
    /// Movement, with the button held (or none).
    Motion(Option<Button>),
}

/// Whether the child wants this event. Shift always keeps the mouse local,
/// so the user can select text inside a program that tracks the mouse.
pub fn wants(mode: TermMode, action: Action, mods: Mods) -> bool {
    if mods.shift || !mode.intersects(TermMode::MOUSE_MODE) {
        return false;
    }
    match action {
        Action::Press | Action::Release => true,
        Action::Motion(Some(_)) => mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION),
        Action::Motion(None) => mode.contains(TermMode::MOUSE_MOTION),
    }
}

/// The report for an event at a 0-based viewport cell.
pub fn report(
    mode: TermMode,
    button: Button,
    action: Action,
    mods: Mods,
    col: usize,
    row: usize,
) -> Vec<u8> {
    let mut code: u32 = match button {
        Button::Left => 0,
        Button::Middle => 1,
        Button::Right => 2,
        Button::WheelUp => 64,
        Button::WheelDown => 65,
    };
    if let Action::Motion(held) = action {
        code = match held {
            Some(_) => code + 32,
            None => 35,
        };
    }
    code += 4 * u32::from(mods.shift) + 8 * u32::from(mods.alt) + 16 * u32::from(mods.ctrl);
    let (x, y) = (col + 1, row + 1);
    if mode.contains(TermMode::SGR_MOUSE) {
        let end = if action == Action::Release { 'm' } else { 'M' };
        return format!("\x1b[<{code};{x};{y}{end}").into_bytes();
    }
    // Legacy: release has no button, and coordinates stop at 223.
    if action == Action::Release {
        code = (code & !3) | 3;
    }
    if x > 223 || y > 223 {
        return Vec::new();
    }
    vec![
        0x1b,
        b'[',
        b'M',
        32 + code as u8,
        32 + x as u8,
        32 + y as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SGR: TermMode = TermMode::SGR_MOUSE.union(TermMode::MOUSE_REPORT_CLICK);

    #[test]
    fn sgr_reports() {
        let none = Mods::default();
        assert_eq!(
            report(SGR, Button::Left, Action::Press, none, 0, 0),
            b"\x1b[<0;1;1M"
        );
        assert_eq!(
            report(SGR, Button::Left, Action::Release, none, 4, 2),
            b"\x1b[<0;5;3m"
        );
        assert_eq!(
            report(SGR, Button::WheelDown, Action::Press, none, 9, 9),
            b"\x1b[<65;10;10M"
        );
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        assert_eq!(
            report(
                SGR,
                Button::Left,
                Action::Motion(Some(Button::Left)),
                ctrl,
                1,
                1
            ),
            b"\x1b[<48;2;2M"
        );
    }

    #[test]
    fn legacy_reports() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        let none = Mods::default();
        assert_eq!(
            report(mode, Button::Left, Action::Press, none, 0, 0),
            b"\x1b[M !!"
        );
        assert_eq!(
            report(mode, Button::Left, Action::Release, none, 0, 0),
            b"\x1b[M#!!"
        );
        assert!(report(mode, Button::Left, Action::Press, none, 300, 0).is_empty());
    }

    #[test]
    fn shift_keeps_the_mouse_local() {
        let shift = Mods {
            shift: true,
            ..Mods::default()
        };
        assert!(wants(SGR, Action::Press, Mods::default()));
        assert!(!wants(SGR, Action::Press, shift));
        assert!(!wants(TermMode::NONE, Action::Press, Mods::default()));
        assert!(!wants(SGR, Action::Motion(None), Mods::default()));
        assert!(wants(
            SGR | TermMode::MOUSE_DRAG,
            Action::Motion(Some(Button::Left)),
            Mods::default()
        ));
    }
}
