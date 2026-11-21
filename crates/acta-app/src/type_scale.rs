//! docs/design/typography.md 3.2: the type roles. Text is always set through a role.
//! No role asks for letter-spacing (the pinned GPUI has none).

use std::sync::Arc;

use gpui_kit::{Div, FontFeatures, FontWeight, Styled, div, px};

use crate::theme::{FONT_MONO, FONT_SANS};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeRole {
    /// The DESIGN.md token, e.g. `type.title`.
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
pub const READING: TypeRole = sans("type.reading", 15., 24., FontWeight::NORMAL);
pub const DOCUMENT: TypeRole = sans("type.document", 16., 26., FontWeight::NORMAL);
pub const UI: TypeRole = sans("type.ui", 13., 20., FontWeight::NORMAL);
pub const UI_STRONG: TypeRole = sans("type.ui-strong", 13., 20., FontWeight::SEMIBOLD);
pub const SMALL: TypeRole = sans("type.small", 12., 16., FontWeight::NORMAL);
pub const LABEL: TypeRole = sans("type.label", 12., 16., FontWeight::SEMIBOLD);
pub const CODE: TypeRole = mono("type.code", 13., 20.);
pub const TERMINAL: TypeRole = mono("type.terminal", 13., 17.);

/// The sans roles, in table order.
pub const SANS_ROLES: [TypeRole; 9] = [
    TITLE, SUBTITLE, HEADING, READING, DOCUMENT, UI, UI_STRONG, SMALL, LABEL,
];

/// Tabular figures (`tnum`), for digits that align or tick: timestamps,
/// counts, ids, versions, shortcut hints (docs/design/typography.md 3.1).
pub fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}
/// The mono roles, in table order.
pub const MONO_ROLES: [TypeRole; 2] = [CODE, TERMINAL];

impl TypeRole {
    /// The family DESIGN.md assigns to this role.
    pub fn family(&self) -> &'static str {
        if self.mono { FONT_MONO } else { FONT_SANS }
    }

    /// A block of text in this role. Colour is the caller's (a text token).
    pub fn el(&self) -> Div {
        div()
            .font_family(self.family())
            .text_size(px(self.size))
            .line_height(px(self.line))
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
            (UI, 13., 20.),
            (UI_STRONG, 13., 20.),
            (SMALL, 12., 16.),
            (LABEL, 12., 16.),
            (CODE, 13., 20.),
            (TERMINAL, 13., 17.),
        ];
        for (role, size, line) in table {
            assert_eq!((role.size, role.line), (size, line), "{}", role.name);
        }
    }
}
