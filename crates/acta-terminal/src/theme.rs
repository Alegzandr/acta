//! The terminal palette of docs/design/color.md 2.4, and how cell colours resolve
//! against it. GPUI-free: colours are `0xRRGGBB`.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

/// Colours the terminal paints with. Switch at runtime with
/// `TerminalView::set_theme`; nothing is cached across a switch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalTheme {
    /// `bg.sunken` in dark, `bg.raised` in light.
    pub background: u32,
    pub foreground: u32,
    /// The 16 ANSI slots, 0 black to 15 bright white.
    pub ansi: [u32; 16],
    /// `accent`: cursor, and selection at [`SELECTION_ALPHA`].
    pub accent: u32,
    /// `text.tertiary`: the "Session ended" line.
    pub muted: u32,
}

/// `selection` is `accent` at 30% (color.md 2.1).
pub const SELECTION_ALPHA: f32 = 0.30;

impl TerminalTheme {
    pub fn dark() -> Self {
        Self {
            background: 0x1B1B1A,
            foreground: 0xD9D8D3,
            ansi: [
                0x353533, 0xDE7B70, 0x9BB26B, 0xD4A24C, 0x7FA6CC, 0xBC96BC, 0x72B5A2, 0xB2B1AB,
                0x9C9B94, 0xEC9A90, 0xB4C888, 0xE4BB70, 0x9DBDDD, 0xD0B0D2, 0x93CCBB, 0xF1F0EC,
            ],
            accent: 0x72B5A2,
            muted: 0x9C9B94,
        }
    }

    pub fn light() -> Self {
        Self {
            background: 0xFFFFFF,
            foreground: 0x24231F,
            ansi: [
                0x24231F, 0xAD4337, 0x4C6A24, 0x85590A, 0x2F5F8F, 0x844C86, 0x2B6F61, 0x66645E,
                0x66645E, 0x963529, 0x3E591B, 0x6F4A06, 0x244D76, 0x6E3D70, 0x235E52, 0x56544E,
            ],
            accent: 0x2B6F61,
            muted: 0x66645E,
        }
    }

    /// Resolve a cell colour. `overrides` are the colours the child set
    /// with OSC 4/10/11; they win over the palette, as in any terminal.
    pub fn resolve(&self, color: Color, overrides: &Colors) -> u32 {
        match color {
            Color::Spec(rgb) => pack(rgb),
            Color::Indexed(index) => overrides[index as usize]
                .map(pack)
                .unwrap_or_else(|| self.indexed(index)),
            Color::Named(named) => overrides[named]
                .map(pack)
                .unwrap_or_else(|| self.named(named)),
        }
    }

    fn named(&self, named: NamedColor) -> u32 {
        let index = named as usize;
        if index < 16 {
            return self.ansi[index];
        }
        match named {
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.accent,
            NamedColor::BrightForeground => self.foreground,
            // Dim colours: the base slot at two thirds towards the background.
            NamedColor::DimForeground => mix(self.foreground, self.background, 0.66),
            NamedColor::DimBlack
            | NamedColor::DimRed
            | NamedColor::DimGreen
            | NamedColor::DimYellow
            | NamedColor::DimBlue
            | NamedColor::DimMagenta
            | NamedColor::DimCyan
            | NamedColor::DimWhite => {
                let base = index - NamedColor::DimBlack as usize;
                mix(self.ansi[base], self.background, 0.66)
            }
            _ => self.foreground,
        }
    }

    /// xterm's 256-colour table: 16 palette slots, a 6x6x6 cube, 24 greys.
    pub fn indexed(&self, index: u8) -> u32 {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let i = index - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v as u32 };
                (level(i / 36) << 16) | (level((i / 6) % 6) << 8) | level(i % 6)
            }
            _ => {
                let v = 8 + 10 * (index as u32 - 232);
                (v << 16) | (v << 8) | v
            }
        }
    }

    /// The reply to an OSC 10/11/12 colour query (`index` 256, 257, 258).
    pub fn query(&self, index: usize, overrides: &Colors) -> Rgb {
        let value = if let Some(rgb) = overrides[index] {
            pack(rgb)
        } else if index < 256 {
            self.indexed(index as u8)
        } else if index == NamedColor::Background as usize {
            self.background
        } else if index == NamedColor::Cursor as usize {
            self.accent
        } else {
            self.foreground
        };
        unpack(value)
    }
}

pub fn pack(rgb: Rgb) -> u32 {
    ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | rgb.b as u32
}

pub fn unpack(value: u32) -> Rgb {
    Rgb {
        r: (value >> 16) as u8,
        g: (value >> 8) as u8,
        b: value as u8,
    }
}

/// `a` weighted `weight`, `b` the rest.
pub fn mix(a: u32, b: u32, weight: f32) -> u32 {
    let channel = |shift: u32| {
        let x = ((a >> shift) & 0xFF) as f32;
        let y = ((b >> shift) & 0xFF) as f32;
        ((x * weight + y * (1.0 - weight)).round() as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_matches_color_md() {
        let dark = TerminalTheme::dark();
        assert_eq!(dark.ansi[6], dark.accent);
        assert_eq!(dark.ansi[8], dark.muted);
        let light = TerminalTheme::light();
        assert_eq!(light.ansi[6], light.accent);
        assert_eq!(light.background, 0xFFFFFF);
    }

    #[test]
    fn resolves_named_indexed_spec_and_overrides() {
        let theme = TerminalTheme::dark();
        let mut overrides = Colors::default();
        assert_eq!(
            theme.resolve(Color::Named(NamedColor::Red), &overrides),
            0xDE7B70
        );
        assert_eq!(theme.resolve(Color::Indexed(9), &overrides), 0xEC9A90);
        assert_eq!(theme.resolve(Color::Indexed(16), &overrides), 0x000000);
        assert_eq!(theme.resolve(Color::Indexed(231), &overrides), 0xFFFFFF);
        assert_eq!(theme.resolve(Color::Indexed(196), &overrides), 0xFF0000);
        assert_eq!(theme.resolve(Color::Indexed(232), &overrides), 0x080808);
        let spec = Rgb { r: 1, g: 2, b: 3 };
        assert_eq!(theme.resolve(Color::Spec(spec), &overrides), 0x010203);
        overrides[NamedColor::Red] = Some(spec);
        assert_eq!(
            theme.resolve(Color::Named(NamedColor::Red), &overrides),
            0x010203
        );
        let bg = theme.query(NamedColor::Background as usize, &Colors::default());
        assert_eq!(pack(bg), 0x1B1B1A);
    }
}
