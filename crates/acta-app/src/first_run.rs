//! First run, docs/design/screens.md 8.5, as far as M1 goes.
//!
//! Left out on purpose until they can work: the "Choose a folder" primary
//! button (no projects before M2) and the three "Then" steps (`acta setup`,
//! note capture and the terminal arrive in M3 to M5).

use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window, div, px,
};

use crate::i18n::{Key, text};
use crate::settings;
use crate::theme::Tokens;
use crate::type_scale;

/// docs/design/typography.md 3.3: the content column is at most 680 wide.
pub const COLUMN_MAX: f32 = 680.;

pub struct FirstRun;

impl Render for FirstRun {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = cx.global::<Tokens>();
        let locale = settings::locale(cx);
        div()
            .id("first-run")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .px(px(32.))
            .child(
                // The column is centred; everything inside it is left-aligned.
                div()
                    .w_full()
                    .max_w(px(COLUMN_MAX))
                    .pt(px(96.))
                    .pb(px(48.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        type_scale::TITLE
                            .el()
                            .text_color(tokens.text_primary)
                            .child(text(locale, Key::FirstrunTitle)),
                    )
                    .child(
                        type_scale::READING
                            .el()
                            .max_w(px(480.))
                            .text_color(tokens.text_secondary)
                            .child(text(locale, Key::FirstrunBody)),
                    ),
            )
    }
}
