//! Markdown at rest (docs/design/typography.md 3.3), drawn by gpui-kit's
//! text view with Acta's tokens (mapped onto its theme in `theme.rs`) and
//! the two settings of 3.3: short surfaces (current state, description,
//! entries, notes) in `type.reading` with compact headings, and documents in
//! `type.document` with real heading sizes.

use std::sync::Arc;

use gpui_kit::component::text::{TextView, TextViewStyle};
use gpui_kit::{
    Div, ElementId, HighlightStyle, ParentElement, SharedString, StyleRefinement, Styled, px, rems,
};

use crate::theme::Tokens;
use crate::type_scale;

/// typography.md 3.3: a code block on `bg.sunken`, padding 12, radius 6,
/// no border.
fn code_block(tokens: &Tokens) -> StyleRefinement {
    StyleRefinement::default()
        .bg(tokens.bg_sunken)
        .border_0()
        .p(px(12.))
        .rounded(px(crate::theme::RADIUS_MD))
}

/// typography.md 3.3: inline code on `bg.sunken`.
fn inline_code(tokens: &Tokens) -> HighlightStyle {
    HighlightStyle {
        background_color: Some(tokens.bg_sunken),
        ..HighlightStyle::default()
    }
}

/// A short surface: `type.reading`, paragraph gap 12, h1/h2 as
/// `type.heading` (16) and h3 as `type.ui-strong` (13).
pub fn short(id: impl Into<ElementId>, text: impl Into<SharedString>, tokens: &Tokens) -> Div {
    let style = TextViewStyle {
        paragraph_gap: rems(0.75),
        heading_font_size: Some(Arc::new(|level, _| match level {
            1 | 2 => px(16.),
            _ => px(13.),
        })),
        code_block: code_block(tokens),
        inline_code: inline_code(tokens),
        ..TextViewStyle::default()
    };
    type_scale::READING
        .el()
        .w_full()
        .text_color(tokens.text_primary)
        .child(TextView::markdown(id, text).style(style))
}

/// A document body: `type.document`, paragraph gap 16, h1 24, h2 20, h3 16.
pub fn document(id: impl Into<ElementId>, text: impl Into<SharedString>, tokens: &Tokens) -> Div {
    let style = TextViewStyle {
        paragraph_gap: rems(1.),
        heading_font_size: Some(Arc::new(|level, _| match level {
            1 => px(24.),
            2 => px(20.),
            _ => px(16.),
        })),
        code_block: code_block(tokens),
        inline_code: inline_code(tokens),
        ..TextViewStyle::default()
    };
    type_scale::DOCUMENT
        .el()
        .w_full()
        .text_color(tokens.text_primary)
        .child(TextView::markdown(id, text).style(style))
}
