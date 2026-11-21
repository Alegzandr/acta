//! docs/design/color.md 2.1, in one place. No other module contains a colour.
//!
//! [`Tokens`] is what Acta's own views read (`cx.global::<Tokens>()`).
//! [`apply`] also writes the tokens into the gpui-kit `Theme`, so that any
//! gpui-component widget starts from Acta's palette instead of its default
//! (ADR 0001, decision 3).
//!
//! The Appearance setting (docs/design/color.md 2) is System, Light or Dark. System
//! follows the OS live: every window reports appearance changes to
//! [`os_appearance_changed`]. Switching replaces the tokens and refreshes
//! every window in the same frame; nothing animates.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, BoxShadow, Global, Hsla, WindowAppearance, point, px, rgb};

pub const FONT_SANS: &str = "Inter";
pub const FONT_MONO: &str = "JetBrains Mono NL";

pub const RADIUS_SM: f32 = 4.;
pub const RADIUS_MD: f32 = 6.;
pub const RADIUS_LG: f32 = 8.;

/// Every colour token of docs/design/color.md 2.1. Field names are the token names with
/// `.` and `-` replaced by `_`.
#[derive(Clone, Debug)]
pub struct Tokens {
    pub bg_sunken: Hsla,
    pub bg_base: Hsla,
    pub bg_raised: Hsla,
    pub bg_overlay: Hsla,
    pub fill_hover: Hsla,
    pub fill_selected: Hsla,
    pub fill_pressed: Hsla,
    pub border_subtle: Hsla,
    pub border_default: Hsla,
    pub border_strong: Hsla,
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_disabled: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_pressed: Hsla,
    pub accent_subtle: Hsla,
    pub text_on_accent: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,
    pub success: Hsla,
    pub selection: Hsla,
    pub focus_ring: Hsla,
    pub scrim: Hsla,
    /// Colour of `shadow.overlay`; its geometry is [`Tokens::shadow_overlay`].
    pub shadow_overlay_color: Hsla,
    pub caption_close_hover: Hsla,
    /// Glyph on `caption.close.hover` (the platform's white, per 2.1).
    pub caption_close_glyph: Hsla,
}

impl Global for Tokens {}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

/// The opaque values of one theme column; alpha tokens are derived from them.
struct Palette {
    bg: [u32; 4],
    border: [u32; 3],
    text: [u32; 4],
    accent: [u32; 3],
    text_on_accent: u32,
    danger: u32,
    warning: u32,
    success: u32,
    scrim: (u32, f32),
    shadow: (u32, f32),
}

impl Tokens {
    pub fn dark() -> Self {
        Self::from_palette(&Palette {
            bg: [0x1B1B1A, 0x212120, 0x292928, 0x30302E],
            border: [0x353533, 0x424240, 0x85847E],
            text: [0xE4E3DF, 0xB2B1AB, 0x9C9B94, 0x62615C],
            accent: [0x72B5A2, 0x86C4B2, 0x5E9F8D],
            text_on_accent: 0x10201C,
            danger: 0xDE7B70,
            warning: 0xD4A24C,
            success: 0x9BB26B,
            scrim: (0x000000, 0.45),
            shadow: (0x000000, 0.40),
        })
    }

    pub fn light() -> Self {
        Self::from_palette(&Palette {
            bg: [0xF4F3F0, 0xFBFAF8, 0xFFFFFF, 0xFFFFFF],
            border: [0xE8E6E1, 0xD9D7D1, 0x8A8881],
            text: [0x24231F, 0x56544E, 0x66645E, 0xA7A59F],
            accent: [0x2B6F61, 0x235E52, 0x1C4E44],
            text_on_accent: 0xFBFAF8,
            danger: 0xAD4337,
            warning: 0x85590A,
            success: 0x4C6A24,
            scrim: (0x24231F, 0.25),
            shadow: (0x24231F, 0.14),
        })
    }

    fn from_palette(p: &Palette) -> Self {
        let text_primary = hex(p.text[0]);
        let accent = hex(p.accent[0]);
        Self {
            bg_sunken: hex(p.bg[0]),
            bg_base: hex(p.bg[1]),
            bg_raised: hex(p.bg[2]),
            bg_overlay: hex(p.bg[3]),
            fill_hover: text_primary.opacity(0.06),
            fill_selected: text_primary.opacity(0.09),
            fill_pressed: text_primary.opacity(0.10),
            border_subtle: hex(p.border[0]),
            border_default: hex(p.border[1]),
            border_strong: hex(p.border[2]),
            text_primary,
            text_secondary: hex(p.text[1]),
            text_tertiary: hex(p.text[2]),
            text_disabled: hex(p.text[3]),
            accent,
            accent_hover: hex(p.accent[1]),
            accent_pressed: hex(p.accent[2]),
            accent_subtle: accent.opacity(0.16),
            text_on_accent: hex(p.text_on_accent),
            danger: hex(p.danger),
            warning: hex(p.warning),
            success: hex(p.success),
            selection: accent.opacity(0.30),
            focus_ring: accent,
            scrim: hex(p.scrim.0).opacity(p.scrim.1),
            shadow_overlay_color: hex(p.shadow.0).opacity(p.shadow.1),
            caption_close_hover: hex(0xC42B1C),
            caption_close_glyph: hex(0xFFFFFF),
        }
    }

    /// `shadow.overlay`: 0 8px 24px. Only for layers that float.
    pub fn shadow_overlay(&self) -> BoxShadow {
        BoxShadow {
            color: self.shadow_overlay_color,
            offset: point(px(0.), px(8.)),
            blur_radius: px(24.),
            spread_radius: px(0.),
            inset: false,
        }
    }

    /// Every token with its docs/design/color.md name, in table order (type specimen).
    pub fn named(&self) -> [(&'static str, Hsla); 28] {
        [
            ("bg.sunken", self.bg_sunken),
            ("bg.base", self.bg_base),
            ("bg.raised", self.bg_raised),
            ("bg.overlay", self.bg_overlay),
            ("fill.hover", self.fill_hover),
            ("fill.selected", self.fill_selected),
            ("fill.pressed", self.fill_pressed),
            ("border.subtle", self.border_subtle),
            ("border.default", self.border_default),
            ("border.strong", self.border_strong),
            ("text.primary", self.text_primary),
            ("text.secondary", self.text_secondary),
            ("text.tertiary", self.text_tertiary),
            ("text.disabled", self.text_disabled),
            ("accent", self.accent),
            ("accent.hover", self.accent_hover),
            ("accent.pressed", self.accent_pressed),
            ("accent.subtle", self.accent_subtle),
            ("text.on-accent", self.text_on_accent),
            ("danger", self.danger),
            ("warning", self.warning),
            ("success", self.success),
            ("selection", self.selection),
            ("focus.ring", self.focus_ring),
            ("scrim", self.scrim),
            ("shadow.overlay", self.shadow_overlay_color),
            ("caption.close.hover", self.caption_close_hover),
            ("caption.close glyph", self.caption_close_glyph),
        ]
    }
}

/// The Appearance setting of docs/design/color.md 2. System is the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeSetting {
    #[default]
    System,
    Light,
    Dark,
}

/// The theme actually shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    pub fn tokens(self) -> Tokens {
        match self {
            Mode::Light => Tokens::light(),
            Mode::Dark => Tokens::dark(),
        }
    }

    /// What the OS reports, as a mode.
    pub fn of(appearance: WindowAppearance) -> Mode {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Mode::Light,
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Mode::Dark,
        }
    }
}

/// The mode a setting resolves to. `os` is `None` when the OS reports no
/// preference; docs/design/color.md 2 then asks for dark.
pub fn resolve(setting: ThemeSetting, os: Option<Mode>) -> Mode {
    match setting {
        ThemeSetting::Light => Mode::Light,
        ThemeSetting::Dark => Mode::Dark,
        ThemeSetting::System => os.unwrap_or(Mode::Dark),
    }
}

/// The current setting and the mode it resolved to (a GPUI global).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub setting: ThemeSetting,
    pub mode: Mode,
}

impl Global for Appearance {}

/// The OS appearance as GPUI reports it. The pinned GPUI always reports one:
/// on Linux a desktop without a colour-scheme preference reads as light (its
/// portal maps "no preference" to light), so the dark fallback of
/// docs/design/color.md 2 can only apply where the platform says nothing at all.
fn os_mode(cx: &App) -> Option<Mode> {
    Some(Mode::of(cx.window_appearance()))
}

/// Apply `setting`: resolve it, install the tokens, repaint every window.
pub fn apply(setting: ThemeSetting, cx: &mut App) {
    // A forced appearance on macOS also themes the native window chrome;
    // System clears it so the OS value can be read and followed.
    cx.set_window_appearance(match setting {
        ThemeSetting::System => None,
        ThemeSetting::Light => Some(WindowAppearance::Light),
        ThemeSetting::Dark => Some(WindowAppearance::Dark),
    });
    let mode = resolve(setting, os_mode(cx));
    cx.set_global(Appearance { setting, mode });
    install(mode, cx);
}

/// A window saw the OS appearance change. Only System follows it.
pub fn os_appearance_changed(appearance: WindowAppearance, cx: &mut App) {
    let Some(current) = cx.try_global::<Appearance>().copied() else {
        return;
    };
    if current.setting != ThemeSetting::System {
        return;
    }
    let mode = resolve(ThemeSetting::System, Some(Mode::of(appearance)));
    if mode != current.mode {
        cx.set_global(Appearance { mode, ..current });
        install(mode, cx);
    }
}

/// Make `mode` the active theme: Acta's tokens, and the gpui-kit `Theme` in
/// the same mode with its colours taken from the tokens. `Theme::update`
/// refreshes every window.
fn install(mode: Mode, cx: &mut App) {
    let tokens = mode.tokens();
    // Loading the mode replaces the colours, so it is a separate edit
    // (see `Theme::update`).
    let theme_mode = match mode {
        Mode::Light => ThemeMode::Light,
        Mode::Dark => ThemeMode::Dark,
    };
    // Acta's global first: the refresh below repaints with it.
    cx.set_global(tokens.clone());
    Theme::change(theme_mode, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = FONT_SANS.into();
        theme.font_size = px(13.);
        theme.mono_font_family = FONT_MONO.into();
        theme.mono_font_size = px(13.);
        theme.radius = px(RADIUS_SM);
        theme.radius_lg = px(RADIUS_LG);
        theme.shadow = false;

        let t = &tokens;
        let c = &mut theme.colors;
        c.background = t.bg_base;
        c.foreground = t.text_primary;
        c.muted = t.bg_raised;
        c.muted_foreground = t.text_tertiary;
        c.border = t.border_subtle;
        c.input = t.border_strong;
        c.ring = t.focus_ring;
        c.caret = t.accent;
        c.selection = t.selection;
        c.overlay = t.scrim;

        c.primary = t.accent;
        c.primary_hover = t.accent_hover;
        c.primary_active = t.accent_pressed;
        c.primary_foreground = t.text_on_accent;
        c.secondary = t.bg_base;
        c.secondary_hover = t.fill_hover;
        c.secondary_active = t.fill_pressed;
        c.secondary_foreground = t.text_primary;
        c.accent = t.fill_selected;
        c.accent_foreground = t.text_primary;

        c.danger = t.danger;
        c.danger_hover = t.danger.opacity(0.9);
        c.danger_active = t.danger.opacity(0.8);
        c.danger_foreground = t.text_on_accent;
        c.warning = t.warning;
        c.success = t.success;

        c.link = t.accent;
        c.link_hover = t.accent_hover;
        c.link_active = t.accent_pressed;

        c.popover = t.bg_overlay;
        c.popover_foreground = t.text_primary;
        c.list = t.bg_base;
        c.list_hover = t.fill_hover;
        c.list_active = t.fill_selected;
        c.sidebar = t.bg_sunken;
        c.sidebar_foreground = t.text_secondary;
        c.sidebar_border = t.border_subtle;
        c.title_bar = t.bg_sunken;
        c.title_bar_border = t.border_subtle;
        c.window_border = t.border_subtle;
        c.scrollbar_thumb = t.border_strong.opacity(0.5);
        c.scrollbar_thumb_hover = t.border_strong;
        c.drag_border = t.accent;
        c.drop_target = t.accent_subtle;
    });
}

#[cfg(test)]
mod tests {
    //! The values are pinned to docs/design/color.md: the token table of 2.1 is
    //! read from the file and every row must match [`Tokens`], and the
    //! contrast table of 2.2 is reproduced from the values. Changing a colour
    //! in either place without the other fails here.

    use super::*;
    use gpui_kit::Rgba;

    const DESIGN_MD: &str = include_str!("../../../docs/design/color.md");

    /// 8-bit sRGB channels of a colour, alpha ignored, as the renderer stores them.
    fn rgb8(color: Hsla) -> [u8; 3] {
        let Rgba { r, g, b, .. } = color.to_rgb();
        [r, g, b].map(|c| (c * 255.).round() as u8)
    }

    /// `over` with its alpha composited on the opaque `under`. docs/design/color.md 2.2
    /// measures its table on the exact blend; `rounded` gives the 8-bit
    /// value the renderer stores, which its rule about `text.tertiary` uses.
    fn blend(over: Hsla, under: Hsla) -> [f64; 3] {
        let a = over.a as f64;
        let (o, u) = (rgb8(over), rgb8(under));
        [0, 1, 2].map(|i| o[i] as f64 * a + u[i] as f64 * (1. - a))
    }

    fn rounded(c: [f64; 3]) -> [f64; 3] {
        c.map(f64::round)
    }

    fn opaque(c: [u8; 3]) -> [f64; 3] {
        c.map(f64::from)
    }

    fn luminance(c: [f64; 3]) -> f64 {
        let lin = |c: f64| {
            let c = c / 255.;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    }

    fn ratio(a: [u8; 3], b: [f64; 3]) -> f64 {
        let a = opaque(a);
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    fn check(what: &str, actual: f64, documented: f64) {
        assert!(
            (actual - documented).abs() <= 0.006,
            "{what}: measured {actual:.3}, color.md says {documented:.2}"
        );
    }

    fn hex_of(text: &str) -> Option<u32> {
        let at = text.find('#')?;
        u32::from_str_radix(text.get(at + 1..at + 7)?, 16).ok()
    }

    fn percent_of(text: &str) -> Option<f32> {
        let at = text.find("@ ")?;
        let digits: String = text[at + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        Some(digits.parse::<f32>().ok()? / 100.)
    }

    /// The rows of the table under "### 2.1 Tokens": (token, dark, light).
    fn design_token_rows() -> Vec<(String, String, String)> {
        let section = DESIGN_MD
            .split("### 2.1 Tokens")
            .nth(1)
            .and_then(|rest| rest.split("###").next())
            .expect("docs/design/color.md has section 2.1");
        section
            .lines()
            .filter(|line| line.starts_with("| `"))
            .map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                (
                    cells[1].trim_matches('`').to_string(),
                    cells[2].to_string(),
                    cells[3].to_string(),
                )
            })
            .collect()
    }

    fn lookup(theme: &Tokens, name: &str) -> Hsla {
        theme
            .named()
            .into_iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("color.md token {name} is not in Tokens"))
            .1
    }

    /// Check one cell of the 2.1 table against the token value.
    fn check_cell(theme: &Tokens, token: &str, cell: &str, dark_cell: &str) {
        let value = lookup(theme, token);
        // "same rule" repeats the dark column's rule.
        let cell = if cell == "same rule" { dark_cell } else { cell };
        if let Some(rest) = cell.strip_prefix("= `") {
            let reference = rest.trim_end_matches('`');
            assert_eq!(value, lookup(theme, reference), "{token}");
        } else if let Some(value_hex) = hex_of(cell) {
            assert_eq!(rgb8(value), rgb8(hex(value_hex)), "{token}");
            let alpha = percent_of(cell).unwrap_or(1.);
            assert!((value.a - alpha).abs() < 1e-4, "{token} alpha");
        } else if let Some(rest) = cell.strip_prefix('`') {
            let reference = &rest[..rest.find('`').expect("closing backtick")];
            let alpha = percent_of(cell).expect("an alpha rule");
            assert_eq!(rgb8(value), rgb8(lookup(theme, reference)), "{token}");
            assert!((value.a - alpha).abs() < 1e-4, "{token} alpha");
        } else {
            panic!("{token}: cannot read {cell:?}");
        }
    }

    #[test]
    fn every_token_matches_the_table_in_design_md() {
        let rows = design_token_rows();
        assert!(rows.len() >= 27, "found {} rows", rows.len());
        let (dark, light) = (Tokens::dark(), Tokens::light());
        for (token, dark_cell, light_cell) in &rows {
            check_cell(&dark, token, dark_cell, dark_cell);
            check_cell(&light, token, light_cell, dark_cell);
        }
        // Every token Acta has is documented (the glyph is a note of 2.1).
        for (name, _) in dark.named() {
            assert!(
                name == "caption.close glyph" || rows.iter().any(|(t, _, _)| t == name),
                "{name} is not in docs/design/color.md 2.1"
            );
        }
    }

    type Four = [f64; 4];
    type Three = [f64; 3];

    /// docs/design/color.md 2.2, every row, dark then light.
    #[test]
    fn contrast_reproduces_the_table_in_design_md() {
        for (t, dark) in [(Tokens::dark(), true), (Tokens::light(), false)] {
            let pick = |d: &[f64], l: &[f64], i: usize| if dark { d[i] } else { l[i] };
            let four = [t.bg_sunken, t.bg_base, t.bg_raised, t.bg_overlay].map(rgb8);
            let rows: [(&str, Hsla, Four, Four); 7] = [
                (
                    "text.primary",
                    t.text_primary,
                    [13.42, 12.55, 11.34, 10.30],
                    [14.17, 15.08, 15.73, 15.73],
                ),
                (
                    "text.secondary",
                    t.text_secondary,
                    [8.02, 7.50, 6.77, 6.15],
                    [6.82, 7.26, 7.57, 7.57],
                ),
                (
                    "text.tertiary",
                    t.text_tertiary,
                    [6.18, 5.78, 5.22, 4.74],
                    [5.33, 5.67, 5.92, 5.92],
                ),
                (
                    "accent",
                    t.accent,
                    [7.26, 6.79, 6.13, 5.57],
                    [5.33, 5.67, 5.92, 5.92],
                ),
                (
                    "danger",
                    t.danger,
                    [5.89, 5.50, 4.97, 4.52],
                    [5.21, 5.54, 5.78, 5.78],
                ),
                (
                    "warning",
                    t.warning,
                    [7.45, 6.96, 6.29, 5.71],
                    [5.52, 5.87, 6.12, 6.12],
                ),
                (
                    "success",
                    t.success,
                    [7.37, 6.89, 6.22, 5.65],
                    [5.58, 5.93, 6.19, 6.19],
                ),
            ];
            for (name, fg, d, l) in rows {
                for (i, surface) in four.iter().enumerate() {
                    check(name, ratio(rgb8(fg), opaque(*surface)), pick(&d, &l, i));
                }
            }

            let on = rgb8(t.text_on_accent);
            let fills = [t.accent, t.accent_hover, t.accent_pressed];
            let (d, l) = ([7.10, 8.47, 5.48], [5.67, 7.20, 9.06]);
            for (i, fill) in fills.iter().enumerate() {
                check(
                    "text.on-accent",
                    ratio(on, opaque(rgb8(*fill))),
                    pick(&d, &l, i),
                );
            }
            let documented = if dark { 5.76 } else { 5.54 };
            check("on danger", ratio(on, opaque(rgb8(t.danger))), documented);

            // Text on a translucent fill over sunken / base / overlay.
            let three = [t.bg_sunken, t.bg_base, t.bg_overlay];
            let on_fill: [(&str, Hsla, Hsla, Three, Three); 4] = [
                (
                    "primary on selected",
                    t.text_primary,
                    t.fill_selected,
                    [10.73, 9.93, 8.08],
                    [11.94, 12.68, 13.22],
                ),
                (
                    "secondary on selected",
                    t.text_secondary,
                    t.fill_selected,
                    [6.41, 5.93, 4.83],
                    [5.75, 6.11, 6.36],
                ),
                (
                    "secondary on hover",
                    t.text_secondary,
                    t.fill_hover,
                    [6.96, 6.45, 5.24],
                    [6.09, 6.48, 6.75],
                ),
                (
                    "primary on accent.subtle",
                    t.text_primary,
                    t.accent_subtle,
                    [10.09, 9.37, 7.73],
                    [11.41, 12.07, 12.55],
                ),
            ];
            for (name, fg, fill, d, l) in on_fill {
                for (i, surface) in three.iter().enumerate() {
                    let measured = ratio(rgb8(fg), blend(fill, *surface));
                    check(name, measured, pick(&d, &l, i));
                }
            }
            let (d, l) = ([5.36, 4.97], [4.76, 5.06]);
            for (i, surface) in [t.bg_sunken, t.bg_base].iter().enumerate() {
                let measured = ratio(rgb8(t.text_tertiary), blend(t.fill_hover, *surface));
                check("tertiary on hover", measured, pick(&d, &l, i));
            }
            let measured = ratio(rgb8(t.warning), blend(t.fill_hover, t.bg_base));
            check("warning on hover", measured, if dark { 5.99 } else { 5.24 });
            let measured = ratio(rgb8(t.accent), blend(t.fill_selected, t.bg_sunken));
            check(
                "accent on selected",
                measured,
                if dark { 5.81 } else { 4.49 },
            );
            let measured = ratio(rgb8(t.text_primary), blend(t.selection, t.bg_base));
            check(
                "primary on selection",
                measured,
                if dark { 6.99 } else { 9.78 },
            );

            let (d, l) = ([4.60, 4.30, 3.88, 3.53], [3.20, 3.40, 3.55, 3.55]);
            for (i, surface) in four.iter().enumerate() {
                let measured = ratio(rgb8(t.border_strong), opaque(*surface));
                check("border.strong", measured, pick(&d, &l, i));
            }
            let (d, l) = ([7.26, 6.79, 5.57], [5.33, 5.67, 5.92]);
            for (i, surface) in three.iter().enumerate() {
                let measured = ratio(rgb8(t.focus_ring), opaque(rgb8(*surface)));
                check("focus.ring", measured, pick(&d, &l, i));
            }
            let measured = ratio(
                rgb8(t.caption_close_glyph),
                opaque(rgb8(t.caption_close_hover)),
            );
            check("caption close", measured, 5.66);
        }
    }

    #[test]
    fn tertiary_text_fails_on_a_selected_row_as_documented() {
        // docs/design/color.md 2.2: 3.73 over overlay in dark, 4.48 over sunken in light.
        let t = Tokens::dark();
        let measured = ratio(
            rgb8(t.text_tertiary),
            rounded(blend(t.fill_selected, t.bg_overlay)),
        );
        check("dark", measured, 3.73);
        let t = Tokens::light();
        let measured = ratio(
            rgb8(t.text_tertiary),
            rounded(blend(t.fill_selected, t.bg_sunken)),
        );
        check("light", measured, 4.48);
    }

    #[test]
    fn system_follows_the_os_and_falls_back_to_dark() {
        assert_eq!(
            resolve(ThemeSetting::System, Some(Mode::Light)),
            Mode::Light
        );
        assert_eq!(resolve(ThemeSetting::System, Some(Mode::Dark)), Mode::Dark);
        assert_eq!(resolve(ThemeSetting::System, None), Mode::Dark);
        assert_eq!(resolve(ThemeSetting::Light, Some(Mode::Dark)), Mode::Light);
        assert_eq!(resolve(ThemeSetting::Dark, Some(Mode::Light)), Mode::Dark);
        assert_eq!(ThemeSetting::default(), ThemeSetting::System);
        assert_eq!(Mode::of(WindowAppearance::VibrantDark), Mode::Dark);
        assert_eq!(Mode::of(WindowAppearance::VibrantLight), Mode::Light);
    }

    #[test]
    fn elevation_gets_lighter_towards_the_overlay() {
        for t in [Tokens::dark(), Tokens::light()] {
            let steps = [t.bg_sunken, t.bg_base, t.bg_raised, t.bg_overlay]
                .map(|c| luminance(opaque(rgb8(c))));
            assert!(
                steps[0] < steps[1] && steps[1] < steps[2] && steps[2] <= steps[3],
                "{steps:?}"
            );
        }
    }

    #[test]
    fn no_pure_black_anywhere_and_no_white_in_dark_or_as_text() {
        for (dark, t) in [(true, Tokens::dark()), (false, Tokens::light())] {
            for (name, color) in t.named() {
                if matches!(name, "scrim" | "shadow.overlay" | "caption.close glyph") {
                    continue;
                }
                let c = rgb8(color);
                assert_ne!(c, [0, 0, 0], "{name}");
                if dark || name.starts_with("text.") {
                    assert_ne!(c, [255, 255, 255], "{name}");
                }
            }
        }
    }
}
