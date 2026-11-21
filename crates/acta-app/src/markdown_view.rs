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

/// typography.md 3.3: a code block on `bg.sunken`, padding 12 x 16,
/// radius 8, no border.
fn code_block(tokens: &Tokens) -> StyleRefinement {
    StyleRefinement::default()
        .bg(tokens.bg_sunken)
        .border_0()
        .px(px(16.))
        .py(px(12.))
        .rounded(px(crate::theme::RADIUS_MD))
}

/// typography.md 3.3: inline code on `bg.sunken`.
fn inline_code(tokens: &Tokens) -> HighlightStyle {
    HighlightStyle {
        background_color: Some(tokens.bg_sunken),
        ..HighlightStyle::default()
    }
}

/// A short surface: `type.reading` at the Text size setting, paragraph gap
/// 0.75 em; headings stay close to the body (h1/h2 two px larger, h3 the
/// body size), so an entry never out-shouts the page title.
pub fn short(id: impl Into<ElementId>, text: impl Into<SharedString>, tokens: &Tokens) -> Div {
    let body = type_scale::READING.size();
    let style = TextViewStyle {
        paragraph_gap: rems(0.75),
        heading_font_size: Some(Arc::new(move |level, _| match level {
            1 | 2 => px(body + 2.),
            _ => px(body),
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

/// A document body: `type.document` at the Text size setting, paragraph gap
/// 16; h1 the page title's 24, h2 three px over the body (at most 22), h3
/// one px over it, so the steps hold at every Text size.
pub fn document(id: impl Into<ElementId>, text: impl Into<SharedString>, tokens: &Tokens) -> Div {
    let body = type_scale::DOCUMENT.size();
    let style = TextViewStyle {
        paragraph_gap: rems(1.),
        heading_font_size: Some(Arc::new(move |level, _| match level {
            1 => px(24.),
            2 => px((body + 3.).min(22.)),
            _ => px(body + 1.),
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
