//! Keyboard: which keys Acta keeps while the terminal has focus
//! (docs/design/interaction.md 6.1), and the bytes every other key sends to the
//! child (xterm encoding, plus the kitty keyboard protocol's "disambiguate"
//! level that Claude Code turns on).
//!
//! GPUI-free: the view converts its `Keystroke` into a [`Key`].

use alacritty_terminal::term::TermMode;

/// Modifier state of one key press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Cmd on macOS, the Windows key, Super on Linux.
    pub platform: bool,
}

/// One key press, named as GPUI names it: `key` is "a", "enter", "f6",
/// "up", "-"; `key_char` is the character the layout would type, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key<'a> {
    pub key: &'a str,
    pub key_char: Option<&'a str>,
    pub mods: Mods,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Other,
}

impl Os {
    pub const CURRENT: Os = if cfg!(target_os = "macos") {
        Os::Mac
    } else {
        Os::Other
    };
}

/// Keys the terminal handles itself instead of sending them to the child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Copy,
    Paste,
    TextLarger,
    TextSmaller,
}

/// What a key press does in a focused terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// Acta's own shortcut: the terminal leaves it alone.
    App,
    /// Handled by the terminal view.
    Command(Command),
    /// Sent to the child as these bytes.
    Send(Vec<u8>),
    /// Plain text: delivered through the platform's text input (IME, dead
    /// keys), then written to the child.
    Text,
    /// Swallowed: no meaning in a terminal (a lone modifier, an unknown key).
    Ignore,
}

/// The routing table of interaction.md 6.1, then the encoder.
pub fn route(key: &Key, mode: TermMode, os: Os) -> Route {
    if let Some(command) = command(key, os) {
        return Route::Command(command);
    }
    if kept_by_app(key, os) {
        return Route::App;
    }
    match encode(key, mode, os) {
        Some(bytes) if bytes.is_empty() => Route::Ignore,
        Some(bytes) => Route::Send(bytes),
        None if key.key_char.is_some() => Route::Text,
        None => Route::Ignore,
    }
}

fn command(key: &Key, os: Os) -> Option<Command> {
    let m = key.mods;
    let k = key.key;
    let larger = matches!(k, "=" | "+");
    let smaller = matches!(k, "-" | "_");
    match os {
        Os::Mac => {
            if m.platform && !m.ctrl && !m.alt {
                if !m.shift && k == "c" {
                    return Some(Command::Copy);
                }
                if !m.shift && k == "v" {
                    return Some(Command::Paste);
                }
                // Cmd+Shift+= and Cmd+Shift+- (a layout may report the
                // shifted character with Shift already consumed).
                if larger && (m.shift || k == "+") {
                    return Some(Command::TextLarger);
                }
                if smaller && (m.shift || k == "_") {
                    return Some(Command::TextSmaller);
                }
            }
            None
        }
        Os::Other => {
            if m.ctrl && m.shift && !m.alt && !m.platform {
                if k == "c" {
                    return Some(Command::Copy);
                }
                if k == "v" {
                    return Some(Command::Paste);
                }
            }
            // Ctrl+Alt+= / Ctrl+Alt+-. Where AltGr with that key types a
            // character, the character wins (components-app.md).
            let typed_other = key.key_char.is_some_and(|c| c != k);
            if m.ctrl && m.alt && !m.shift && !m.platform && !typed_other {
                if k == "=" {
                    return Some(Command::TextLarger);
                }
                if k == "-" {
                    return Some(Command::TextSmaller);
                }
            }
            None
        }
    }
}

/// Acta's own shortcuts that stay active while the terminal has focus.
pub fn kept_by_app(key: &Key, os: Os) -> bool {
    let m = key.mods;
    let k = key.key;
    let digit = matches!(k, "1" | "2" | "3" | "4");
    // F6 and Shift+F6 cycle panes on every platform.
    if k == "f6" && !m.ctrl && !m.alt && !m.platform {
        return true;
    }
    // Ctrl+` (Ctrl+Shift+`: a new Claude Code session), Ctrl+Tab /
    // Ctrl+Shift+Tab and Ctrl+PageUp / Ctrl+PageDown (the previous and
    // next instance of the view) on every platform.
    if m.ctrl
        && !m.alt
        && !m.platform
        && (k == "tab" || k == "`" || (!m.shift && (k == "pageup" || k == "pagedown")))
    {
        return true;
    }
    // Claude Code (no project): Ctrl+Alt+J, Cmd+Option+J on macOS.
    let free = match os {
        Os::Other => m.ctrl && !m.platform,
        Os::Mac => m.platform && !m.ctrl,
    };
    // Where AltGr+J types a character, the character wins (6.1).
    let typed_other = key.key_char.is_some_and(|c| c != k);
    if free && m.alt && !m.shift && k == "j" && !typed_other {
        return true;
    }
    match os {
        Os::Other => {
            if !m.ctrl || m.alt || m.platform {
                return false;
            }
            if m.shift {
                matches!(k, "k" | "j" | "h" | "enter")
            } else {
                digit
            }
        }
        Os::Mac => {
            if !m.platform || m.ctrl || m.alt {
                return false;
            }
            // Every Cmd shortcut of the table except undo and redo, which
            // reach the child like Ctrl+Z does elsewhere.
            k != "z"
        }
    }
}

/// xterm modifier parameter: 1 + shift + 2 alt + 4 ctrl (+ 8 super, kitty).
fn modifier_param(m: Mods) -> u8 {
    1 + u8::from(m.shift) + 2 * u8::from(m.alt) + 4 * u8::from(m.ctrl) + 8 * u8::from(m.platform)
}

/// Bytes for a key that is not plain text, or `None` when the key types text.
/// `Some(vec![])` means the key has no encoding and is swallowed.
///
/// Kitty keyboard protocol: the disambiguate level is implemented. With
/// "report all keys as escape codes" set, modified keys are encoded the same
/// way but plain text still arrives as text, and no release or repeat events
/// are reported (Claude Code asks for the disambiguate level only).
pub fn encode(key: &Key, mode: TermMode, os: Os) -> Option<Vec<u8>> {
    let m = key.mods;
    let kitty =
        mode.intersects(TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_ALL_KEYS_AS_ESC);
    let has_mods = m.ctrl || m.alt || m.shift || m.platform;
    let param = modifier_param(m);
    let csi = |body: String| format!("\x1b[{body}").into_bytes();

    // Cursor keys and Home/End: SS3 in application cursor mode.
    let cursor = match key.key {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        _ => None,
    };
    if let Some(letter) = cursor {
        return Some(if has_mods {
            csi(format!("1;{param}{letter}"))
        } else if mode.contains(TermMode::APP_CURSOR) {
            format!("\x1bO{letter}").into_bytes()
        } else {
            csi(letter.to_string())
        });
    }

    let tilde = match key.key {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        _ => None,
    };
    if let Some(n) = tilde {
        return Some(if has_mods {
            csi(format!("{n};{param}~"))
        } else {
            csi(format!("{n}~"))
        });
    }

    let ss3 = match key.key {
        "f1" => Some('P'),
        "f2" => Some('Q'),
        "f3" => Some('R'),
        "f4" => Some('S'),
        _ => None,
    };
    if let Some(letter) = ss3 {
        return Some(if has_mods {
            csi(format!("1;{param}{letter}"))
        } else {
            format!("\x1bO{letter}").into_bytes()
        });
    }

    // Enter, Tab, Backspace, Escape, Space: legacy bytes unmodified; with
    // modifiers the kitty form when the child asked for it.
    let special = match key.key {
        "enter" => Some((13u32, b"\r".as_slice())),
        "tab" => Some((9, b"\t".as_slice())),
        "backspace" => Some((127, b"\x7f".as_slice())),
        "escape" => Some((27, b"\x1b".as_slice())),
        "space" => Some((32, b" ".as_slice())),
        _ => None,
    };
    if let Some((code, plain)) = special {
        if kitty && (has_mods || code == 27) {
            // Shift+Space is text in kitty's disambiguate level.
            if code == 32 && !m.ctrl && !m.alt && !m.platform {
                return Some(plain.to_vec());
            }
            return Some(if has_mods {
                csi(format!("{code};{param}u"))
            } else {
                csi(format!("{code}u"))
            });
        }
        if m.platform {
            return Some(Vec::new());
        }
        let mut bytes = match (key.key, m.ctrl, m.shift) {
            ("tab", _, true) => b"\x1b[Z".to_vec(),
            ("backspace", true, _) => b"\x08".to_vec(),
            ("space", true, _) => b"\x00".to_vec(),
            _ => plain.to_vec(),
        };
        if m.alt {
            bytes.insert(0, 0x1b);
        }
        return Some(bytes);
    }

    // The remaining keys are characters.
    let ch = single_char(key.key)?;
    if !m.ctrl && !m.alt && !m.platform {
        // Plain or shifted: text.
        return None;
    }
    // macOS: Option types the layout's character unless Ctrl or Cmd is down.
    if os == Os::Mac
        && m.alt
        && !m.ctrl
        && !m.platform
        && key.key_char.is_some_and(|c| c != key.key)
    {
        return None;
    }
    // AltGr (reported as Ctrl+Alt on Windows) that types a character: text.
    if m.ctrl
        && m.alt
        && key
            .key_char
            .is_some_and(|c| c != key.key && !c.chars().all(char::is_control))
    {
        return None;
    }
    if kitty {
        // Kitty reports the unshifted key: Windows names Ctrl+Shift+- as
        // "_" with Shift already consumed, so put it back (US layout).
        let (base, shifted) = unshift(ch);
        let param = modifier_param(Mods {
            shift: m.shift || shifted,
            ..m
        });
        return Some(csi(format!("{};{param}u", base as u32)));
    }
    if m.platform {
        return Some(Vec::new());
    }
    let mut bytes = Vec::new();
    if m.alt {
        bytes.push(0x1b);
    }
    if m.ctrl {
        // Keys with no control byte send themselves, as xterm does.
        match control_byte(ch) {
            Some(byte) => bytes.push(byte),
            None => bytes.extend(ch.to_string().as_bytes()),
        }
    } else {
        let text = key
            .key_char
            .map(str::to_owned)
            .unwrap_or_else(|| ch.to_string());
        bytes.extend(text.as_bytes());
    }
    Some(bytes)
}

/// The unshifted key of a shifted character on a US layout, and whether
/// it was shifted.
fn unshift(ch: char) -> (char, bool) {
    const PAIRS: [(char, char); 21] = [
        ('_', '-'),
        ('+', '='),
        ('!', '1'),
        ('@', '2'),
        ('#', '3'),
        ('$', '4'),
        ('%', '5'),
        ('^', '6'),
        ('&', '7'),
        ('*', '8'),
        ('(', '9'),
        (')', '0'),
        ('{', '['),
        ('}', ']'),
        ('|', '\\'),
        (':', ';'),
        ('"', '\''),
        ('<', ','),
        ('>', '.'),
        ('?', '/'),
        ('~', '`'),
    ];
    if ch.is_ascii_uppercase() {
        return (ch.to_ascii_lowercase(), true);
    }
    PAIRS
        .iter()
        .find(|(shifted, _)| *shifted == ch)
        .map_or((ch, false), |(_, base)| (*base, true))
}

fn single_char(key: &str) -> Option<char> {
    let mut chars = key.chars();
    let ch = chars.next()?;
    chars.next().is_none().then_some(ch)
}

/// Legacy Ctrl+key byte, as xterm sends it.
fn control_byte(ch: char) -> Option<u8> {
    let ch = ch.to_ascii_lowercase();
    Some(match ch {
        'a'..='z' => ch as u8 - b'a' + 1,
        '@' | '2' | ' ' => 0,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' | '~' => 0x1e,
        '_' | '-' | '/' | '7' => 0x1f,
        '?' | '8' => 0x7f,
        _ => return None,
    })
}

/// Text from the clipboard, as the child should receive it.
pub fn paste_bytes(text: &str, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::BRACKETED_PASTE) {
        // A pasted end marker would let the text escape the bracket.
        let clean = text.replace('\x1b', "");
        format!("\x1b[200~{clean}\x1b[201~").into_bytes()
    } else {
        // Without bracketed paste, newlines become carriage returns, as typed.
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: &str) -> Key<'_> {
        Key {
            key,
            key_char: None,
            mods: Mods::default(),
        }
    }

    fn with(key: &str, f: impl FnOnce(&mut Mods)) -> Key<'_> {
        let mut mods = Mods::default();
        f(&mut mods);
        Key {
            key,
            key_char: None,
            mods,
        }
    }

    fn ctrl(k: &str) -> Key<'_> {
        with(k, |m| m.ctrl = true)
    }

    fn send(k: &Key, mode: TermMode) -> Vec<u8> {
        match route(k, mode, Os::Other) {
            Route::Send(bytes) => bytes,
            other => panic!("{k:?} routed to {other:?}"),
        }
    }

    const NONE: TermMode = TermMode::NONE;
    const KITTY: TermMode = TermMode::DISAMBIGUATE_ESC_CODES;

    #[test]
    fn cursor_keys_follow_app_cursor_mode() {
        assert_eq!(send(&key("up"), NONE), b"\x1b[A");
        assert_eq!(send(&key("up"), TermMode::APP_CURSOR), b"\x1bOA");
        assert_eq!(send(&key("end"), TermMode::APP_CURSOR), b"\x1bOF");
        assert_eq!(send(&ctrl("left"), TermMode::APP_CURSOR), b"\x1b[1;5D");
        let shift_alt = with("right", |m| {
            m.shift = true;
            m.alt = true
        });
        assert_eq!(send(&shift_alt, NONE), b"\x1b[1;4C");
    }

    #[test]
    fn function_and_editing_keys() {
        assert_eq!(send(&key("f1"), NONE), b"\x1bOP");
        assert_eq!(send(&with("f1", |m| m.shift = true), NONE), b"\x1b[1;2P");
        assert_eq!(send(&key("f5"), NONE), b"\x1b[15~");
        assert_eq!(send(&key("f12"), NONE), b"\x1b[24~");
        assert_eq!(send(&ctrl("f12"), NONE), b"\x1b[24;5~");
        assert_eq!(send(&key("delete"), NONE), b"\x1b[3~");
        assert_eq!(send(&key("pageup"), NONE), b"\x1b[5~");
    }

    #[test]
    fn legacy_specials_and_control_bytes() {
        assert_eq!(send(&key("enter"), NONE), b"\r");
        assert_eq!(send(&with("enter", |m| m.alt = true), NONE), b"\x1b\r");
        assert_eq!(send(&with("tab", |m| m.shift = true), NONE), b"\x1b[Z");
        assert_eq!(send(&key("backspace"), NONE), b"\x7f");
        assert_eq!(send(&ctrl("backspace"), NONE), b"\x08");
        assert_eq!(send(&key("escape"), NONE), b"\x1b");
        assert_eq!(send(&ctrl("space"), NONE), b"\x00");
        assert_eq!(send(&ctrl("c"), NONE), b"\x03");
        assert_eq!(send(&ctrl("k"), NONE), b"\x0b");
        assert_eq!(send(&ctrl("["), NONE), b"\x1b");
        assert_eq!(send(&with("b", |m| m.alt = true), NONE), b"\x1bb");
    }

    #[test]
    fn claude_code_undo_keys_reach_the_child() {
        // Ctrl+- and Ctrl+Shift+- (Windows reports the latter as "_").
        assert_eq!(send(&ctrl("-"), NONE), b"\x1f");
        assert_eq!(send(&ctrl("_"), NONE), b"\x1f");
        assert_eq!(send(&ctrl("-"), KITTY), b"\x1b[45;5u");
        let ctrl_shift = with("-", |m| {
            m.ctrl = true;
            m.shift = true
        });
        assert_eq!(send(&ctrl_shift, KITTY), b"\x1b[45;6u");
        // As Windows reports it: "_" with Shift consumed.
        assert_eq!(send(&ctrl("_"), KITTY), b"\x1b[45;6u");
    }

    #[test]
    fn kitty_disambiguate_level() {
        assert_eq!(send(&key("escape"), KITTY), b"\x1b[27u");
        assert_eq!(
            send(&with("enter", |m| m.shift = true), KITTY),
            b"\x1b[13;2u"
        );
        assert_eq!(send(&ctrl("enter"), KITTY), b"\x1b[13;5u");
        assert_eq!(send(&key("enter"), KITTY), b"\r");
        assert_eq!(send(&ctrl("c"), KITTY), b"\x1b[99;5u");
        assert_eq!(send(&with("x", |m| m.alt = true), KITTY), b"\x1b[120;3u");
        assert_eq!(send(&key("up"), KITTY), b"\x1b[A");
        let shift_space = Key {
            key: "space",
            key_char: Some(" "),
            mods: Mods {
                shift: true,
                ..Mods::default()
            },
        };
        assert_eq!(send(&shift_space, KITTY), b" ");
    }

    #[test]
    fn text_goes_through_text_input() {
        let a = Key {
            key: "a",
            key_char: Some("a"),
            mods: Mods::default(),
        };
        assert_eq!(route(&a, NONE, Os::Other), Route::Text);
        let shifted = Key {
            key: "a",
            key_char: Some("A"),
            mods: Mods {
                shift: true,
                ..Mods::default()
            },
        };
        assert_eq!(route(&shifted, KITTY, Os::Other), Route::Text);
        // AltGr+e on a French layout types the euro sign.
        let altgr = Key {
            key: "e",
            key_char: Some("€"),
            mods: Mods {
                ctrl: true,
                alt: true,
                ..Mods::default()
            },
        };
        assert_eq!(route(&altgr, NONE, Os::Other), Route::Text);
        // Option+s types ß on macOS.
        let option = Key {
            key: "s",
            key_char: Some("ß"),
            mods: Mods {
                alt: true,
                ..Mods::default()
            },
        };
        assert_eq!(route(&option, NONE, Os::Mac), Route::Text);
    }

    #[test]
    fn routing_on_windows_and_linux() {
        let ctrl_shift = |k| {
            with(k, |m| {
                m.ctrl = true;
                m.shift = true
            })
        };
        assert_eq!(route(&ctrl_shift("k"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl_shift("j"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl_shift("h"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl_shift("enter"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl("`"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl_shift("`"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl("pagedown"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl("pageup"), NONE, Os::Other), Route::App);
        let ctrl_alt_j = with("j", |m| {
            m.ctrl = true;
            m.alt = true
        });
        assert_eq!(route(&ctrl_alt_j, NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl("3"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl("tab"), NONE, Os::Other), Route::App);
        assert_eq!(route(&ctrl_shift("tab"), NONE, Os::Other), Route::App);
        assert_eq!(route(&key("f6"), NONE, Os::Other), Route::App);
        assert_eq!(
            route(&with("f6", |m| m.shift = true), NONE, Os::Other),
            Route::App
        );
        assert_eq!(
            route(&ctrl_shift("c"), NONE, Os::Other),
            Route::Command(Command::Copy)
        );
        assert_eq!(
            route(&ctrl_shift("v"), NONE, Os::Other),
            Route::Command(Command::Paste)
        );
        let ctrl_alt = |k| {
            with(k, |m| {
                m.ctrl = true;
                m.alt = true
            })
        };
        assert_eq!(
            route(&ctrl_alt("="), NONE, Os::Other),
            Route::Command(Command::TextLarger)
        );
        assert_eq!(
            route(&ctrl_alt("-"), NONE, Os::Other),
            Route::Command(Command::TextSmaller)
        );
        // Passed through: readline keys, Ctrl+Enter, Ctrl+, and Ctrl+/, F5.
        for k in ["k", "p", "n", "e", "b", "w", "z", ",", "/"] {
            assert!(
                matches!(route(&ctrl(k), NONE, Os::Other), Route::Send(_)),
                "{k}"
            );
        }
        assert_eq!(send(&ctrl("enter"), NONE), b"\r");
        assert_eq!(send(&key("f5"), NONE), b"\x1b[15~");
    }

    #[test]
    fn routing_on_macos() {
        let cmd = |k| with(k, |m| m.platform = true);
        assert_eq!(
            route(&cmd("c"), NONE, Os::Mac),
            Route::Command(Command::Copy)
        );
        assert_eq!(
            route(&cmd("v"), NONE, Os::Mac),
            Route::Command(Command::Paste)
        );
        let cmd_shift = |k| {
            with(k, |m| {
                m.platform = true;
                m.shift = true
            })
        };
        assert_eq!(
            route(&cmd_shift("="), NONE, Os::Mac),
            Route::Command(Command::TextLarger)
        );
        assert_eq!(
            route(&cmd_shift("-"), NONE, Os::Mac),
            Route::Command(Command::TextSmaller)
        );
        assert_eq!(route(&cmd("k"), NONE, Os::Mac), Route::App);
        assert_eq!(route(&cmd_shift("k"), NONE, Os::Mac), Route::App);
        assert_eq!(route(&cmd("1"), NONE, Os::Mac), Route::App);
        // Cmd+Z and Cmd+Shift+Z reach the child; legacy mode has no bytes.
        assert_eq!(route(&cmd("z"), NONE, Os::Mac), Route::Ignore);
        assert_eq!(send_mac(&cmd("z"), KITTY), b"\x1b[122;9u");
        assert_eq!(route(&ctrl("`"), NONE, Os::Mac), Route::App);
        assert_eq!(route(&ctrl("tab"), NONE, Os::Mac), Route::App);
        // Every other Ctrl combination goes through, Ctrl+1 included.
        assert_eq!(send_mac(&ctrl("k"), NONE), b"\x0b");
        assert_eq!(send_mac(&ctrl("1"), NONE), b"1");
    }

    fn send_mac(k: &Key, mode: TermMode) -> Vec<u8> {
        match route(k, mode, Os::Mac) {
            Route::Send(bytes) => bytes,
            other => panic!("{k:?} routed to {other:?}"),
        }
    }

    #[test]
    fn paste_is_bracketed_when_asked() {
        assert_eq!(
            paste_bytes("a\nb", TermMode::BRACKETED_PASTE),
            b"\x1b[200~a\nb\x1b[201~"
        );
        assert_eq!(
            paste_bytes("x\x1b[201~y", TermMode::BRACKETED_PASTE),
            b"\x1b[200~x[201~y\x1b[201~"
        );
        assert_eq!(paste_bytes("a\r\nb\nc", NONE), b"a\rb\rc");
    }
}
