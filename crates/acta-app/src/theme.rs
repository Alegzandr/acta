//! DESIGN.md section 2.1, in one place. No other module contains a colour.
//!
//! [`Tokens`] is what Acta's own views read (`cx.global::<Tokens>()`).
//! [`install`] also writes the tokens into the gpui-kit `Theme`, so that any
//! gpui-component widget starts from Acta's palette instead of its default
//! (ADR 0001, decision 3).

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, BoxShadow, Global, Hsla, point, px, rgb};

pub const FONT_SANS: &str = "Source Sans 3";
pub const FONT_MONO: &str = "JetBrains Mono NL";

pub const RADIUS_SM: f32 = 4.;
pub const RADIUS_MD: f32 = 6.;
pub const RADIUS_LG: f32 = 8.;

/// Every colour token of DESIGN.md 2.1. Field names are the token names with
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
            bg: [0x141210, 0x1A1816, 0x211E1B, 0x292622],
            border: [0x2B2824, 0x38342F, 0x7A7468],
            text: [0xE9E3D8, 0xB5AD9F, 0x9A9285, 0x625C52],
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
            bg: [0xECE7DD, 0xF6F2EA, 0xFCFAF5, 0xFFFDF9],
            border: [0xE6E0D5, 0xD6CFC2, 0x8A8273],
            text: [0x26221D, 0x575045, 0x6A6357, 0xA69F92],
            accent: [0x2B6F61, 0x235E52, 0x1C4E44],
            text_on_accent: 0xFBF8F2,
            danger: 0xAD4337,
            warning: 0x85590A,
            success: 0x4C6A24,
            scrim: (0x26221D, 0.25),
            shadow: (0x26221D, 0.14),
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

    /// Every token with its DESIGN.md name, in table order (type specimen).
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

/// Make `tokens` the active theme: Acta's global, and the gpui-kit `Theme`.
/// Dark is the only mode shipped for now; light exists as tokens.
pub fn install(tokens: Tokens, cx: &mut App) {
    // Loading the mode replaces the colours, so it is a separate edit
    // (see `Theme::update`).
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = FONT_SANS.into();
        theme.font_size = px(14.);
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
    cx.set_global(tokens);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::Rgba;

    /// WCAG 2.x relative luminance of an opaque colour.
    fn luminance(color: Hsla) -> f32 {
        let Rgba { r, g, b, .. } = color.to_rgb();
        let lin = |c: f32| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let (hi, lo) = {
            let (la, lb) = (luminance(a), luminance(b));
            if la > lb { (la, lb) } else { (lb, la) }
        };
        (hi + 0.05) / (lo + 0.05)
    }

    fn assert_ratio(actual: f32, documented: f32) {
        assert!(
            (actual - documented).abs() < 0.06,
            "measured {actual:.2}, DESIGN.md says {documented:.2}"
        );
    }

    #[test]
    fn dark_values_reproduce_the_contrast_table_of_design_md() {
        let t = Tokens::dark();
        assert_ratio(contrast(t.text_primary, t.bg_sunken), 14.63);
        assert_ratio(contrast(t.text_primary, t.bg_base), 13.86);
        assert_ratio(contrast(t.text_secondary, t.bg_base), 7.96);
        assert_ratio(contrast(t.text_tertiary, t.bg_overlay), 4.89);
        assert_ratio(contrast(t.accent, t.bg_base), 7.46);
        assert_ratio(contrast(t.danger, t.bg_base), 6.05);
        assert_ratio(contrast(t.text_on_accent, t.accent), 7.10);
        assert_ratio(contrast(t.border_strong, t.bg_base), 3.82);
        assert_ratio(contrast(t.caption_close_glyph, t.caption_close_hover), 5.66);
    }

    #[test]
    fn light_values_reproduce_the_contrast_table_of_design_md() {
        let t = Tokens::light();
        assert_ratio(contrast(t.text_primary, t.bg_sunken), 12.82);
        assert_ratio(contrast(t.text_primary, t.bg_base), 14.15);
        assert_ratio(contrast(t.text_secondary, t.bg_base), 7.13);
        assert_ratio(contrast(t.text_tertiary, t.bg_sunken), 4.82);
        assert_ratio(contrast(t.accent, t.bg_base), 5.30);
        assert_ratio(contrast(t.danger, t.bg_sunken), 4.69);
        assert_ratio(contrast(t.text_on_accent, t.accent), 5.58);
        assert_ratio(contrast(t.border_strong, t.bg_sunken), 3.09);
    }

    #[test]
    fn alpha_tokens_follow_their_rules() {
        for t in [Tokens::dark(), Tokens::light()] {
            let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
            assert!(close(t.fill_hover.a, 0.06));
            assert!(close(t.fill_selected.a, 0.09));
            assert!(close(t.fill_pressed.a, 0.10));
            assert!(close(t.accent_subtle.a, 0.16));
            assert!(close(t.selection.a, 0.30));
            assert_eq!(t.focus_ring, t.accent);
            assert_eq!(t.fill_hover.opacity(0.), t.text_primary.opacity(0.));
        }
        assert!((Tokens::dark().scrim.a - 0.45).abs() < 1e-4);
        assert!((Tokens::light().scrim.a - 0.25).abs() < 1e-4);
    }

    #[test]
    fn elevation_gets_lighter_towards_the_overlay() {
        for t in [Tokens::dark(), Tokens::light()] {
            let steps = [t.bg_sunken, t.bg_base, t.bg_raised, t.bg_overlay].map(luminance);
            assert!(steps.windows(2).all(|pair| pair[0] < pair[1]), "{steps:?}");
        }
    }

    #[test]
    fn no_surface_or_text_is_pure_black_or_white() {
        for t in [Tokens::dark(), Tokens::light()] {
            for (name, color) in t.named() {
                let opaque_ui = !matches!(name, "scrim" | "shadow.overlay" | "caption.close glyph");
                if opaque_ui {
                    assert!(color.l > 0.0 && color.l < 1.0, "{name}");
                }
            }
        }
    }
}
