//! docs/design/typography.md 3.2: the type roles. Text is always set through a role.
//! No role asks for letter-spacing (the pinned GPUI has none).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::zoom::px;
use gpui_kit::{Div, FontFeatures, FontWeight, Styled, div};

use crate::theme::{FONT_MONO, FONT_SANS};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeRole {
    /// The docs/design/typography.md token, e.g. `type.title`.
    pub name: &'static str,
    pub mono: bool,
    pub size: f32,
    pub line: f32,
    pub weight: FontWeight,
}

const fn sans(name: &'static str, size: f32, line: f32, weight: FontWeight) -> TypeRole {
    TypeRole {
        name,
        mono: false,
        size,
        line,
        weight,
    }
}

const fn mono(name: &'static str, size: f32, line: f32) -> TypeRole {
    TypeRole {
        name,
        mono: true,
        size,
        line,
        weight: FontWeight::NORMAL,
    }
}

pub const TITLE: TypeRole = sans("type.title", 24., 30., FontWeight::SEMIBOLD);
pub const SUBTITLE: TypeRole = sans("type.subtitle", 20., 28., FontWeight::SEMIBOLD);
pub const HEADING: TypeRole = sans("type.heading", 16., 24., FontWeight::SEMIBOLD);
/// The two reading roles at the Default text size; [`TypeRole::size`] and
/// [`TypeRole::line`] follow the Text size setting (typography.md 3.2).
pub const READING: TypeRole = sans("type.reading", 15., 24., FontWeight::NORMAL);
pub const DOCUMENT: TypeRole = sans("type.document", 16., 26., FontWeight::NORMAL);
pub const UI: TypeRole = sans("type.ui", 14., 20., FontWeight::NORMAL);
pub const UI_STRONG: TypeRole = sans("type.ui-strong", 14., 20., FontWeight::SEMIBOLD);
/// Items of context and dropdown menus (typography.md 3.2): one step under
/// `type.ui`, which read too big in a floating layer. Not the palette.
pub const MENU: TypeRole = sans("type.menu", 13., 18., FontWeight::NORMAL);
pub const SMALL: TypeRole = sans("type.small", 12., 16., FontWeight::NORMAL);
pub const LABEL: TypeRole = sans("type.label", 12., 16., FontWeight::SEMIBOLD);
pub const CODE: TypeRole = mono("type.code", 13., 20.);
pub const TERMINAL: TypeRole = mono("type.terminal", 13., 17.);
/// `type.code` among `type.small` metadata (the meta line's id): mono at
/// the small size, the one place mono takes it (typography.md 3.2).
pub const CODE_SMALL: TypeRole = mono("type.code", 12., 16.);

/// The sans roles, in table order.
pub const SANS_ROLES: [TypeRole; 10] = [
    TITLE, SUBTITLE, HEADING, READING, DOCUMENT, UI, UI_STRONG, MENU, SMALL, LABEL,
];

/// Tabular figures (`tnum`), for digits that align or tick: timestamps,
/// counts, ids, versions, shortcut hints (docs/design/typography.md 3.1).
pub fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}
/// The Text size setting (Settings > Appearance): it scales the reading
/// roles only, never the interface chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextSize {
    Small,
    #[default]
    Default,
    Large,
}

impl TextSize {
    pub const ALL: [TextSize; 3] = [TextSize::Small, TextSize::Default, TextSize::Large];

    /// `type.reading` and `type.document` at this setting, as (size, line).
    pub fn reading(self) -> ((f32, f32), (f32, f32)) {
        match self {
            TextSize::Small => ((14., 22.), (15., 24.)),
            TextSize::Default => ((15., 24.), (16., 26.)),
            TextSize::Large => ((17., 28.), (18., 28.)),
        }
    }
}

/// The setting in force. Text is laid out without an `App` at hand, so it
/// lives here; the settings module writes it at launch and on change.
static TEXT_SIZE: AtomicU8 = AtomicU8::new(1);

pub fn set_text_size(size: TextSize) {
    let n = TextSize::ALL.iter().position(|s| *s == size).unwrap_or(1);
    TEXT_SIZE.store(n as u8, Ordering::Relaxed);
}

pub fn text_size() -> TextSize {
    TextSize::ALL[(TEXT_SIZE.load(Ordering::Relaxed) as usize).min(2)]
}

/// The mono roles, in table order.
pub const MONO_ROLES: [TypeRole; 2] = [CODE, TERMINAL];

impl TypeRole {
    /// The family docs/design/typography.md assigns to this role.
    pub fn family(&self) -> &'static str {
        if self.mono { FONT_MONO } else { FONT_SANS }
    }

    /// The size in force: the reading roles follow the Text size setting.
    pub fn size(&self) -> f32 {
        self.scaled().0
    }

    /// The line height in force, as [`Self::size`].
    pub fn line(&self) -> f32 {
        self.scaled().1
    }

    fn scaled(&self) -> (f32, f32) {
        let (reading, document) = text_size().reading();
        match self.name {
            "type.reading" => reading,
            "type.document" => document,
            _ => (self.size, self.line),
        }
    }

    /// A block of text in this role. Colour is the caller's (a text token).
    pub fn el(&self) -> Div {
        div()
            .font_family(self.family())
            .text_size(px(self.size()))
            .line_height(px(self.line()))
            .font_weight(self.weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_ui_weights_and_nothing_under_12px() {
        for role in SANS_ROLES.iter().chain(MONO_ROLES.iter()) {
            assert!(
                role.weight == FontWeight::NORMAL || role.weight == FontWeight::SEMIBOLD,
                "{}",
                role.name
            );
            assert!(role.size >= 12., "{}", role.name);
            assert!(role.line > role.size, "{}", role.name);
        }
    }

    #[test]
    fn families_follow_the_role() {
        assert_eq!(TITLE.family(), FONT_SANS);
        assert_eq!(CODE.family(), FONT_MONO);
        assert!(SANS_ROLES.iter().all(|r| !r.mono));
        assert!(MONO_ROLES.iter().all(|r| r.mono));
    }

    #[test]
    fn scale_matches_design_md() {
        let table = [
            (TITLE, 24., 30.),
            (SUBTITLE, 20., 28.),
            (HEADING, 16., 24.),
            (READING, 15., 24.),
            (DOCUMENT, 16., 26.),
            (UI, 14., 20.),
            (UI_STRONG, 14., 20.),
            (MENU, 13., 18.),
            (SMALL, 12., 16.),
            (LABEL, 12., 16.),
            (CODE, 13., 20.),
            (TERMINAL, 13., 17.),
        ];
        for (role, size, line) in table {
            assert_eq!((role.size, role.line), (size, line), "{}", role.name);
        }
    }

    #[test]
    fn text_size_scales_reading_only_and_keeps_a_reading_line_height() {
        for size in TextSize::ALL {
            let ((r, rl), (d, dl)) = size.reading();
            assert!(d > r, "a document reads one step larger");
            for (s, l) in [(r, rl), (d, dl)] {
                let ratio = l / s;
                assert!((1.5..=1.7).contains(&ratio), "{size:?} {s}/{l}");
                assert_eq!(
                    l % 2.,
                    0.,
                    "even line heights keep baselines on whole pixels"
                );
            }
        }
        assert_eq!(
            TextSize::Default.reading(),
            ((READING.size, READING.line), (DOCUMENT.size, DOCUMENT.line))
        );
    }
}
