//! Type specimen, a development aid opened with `Acta --specimen` or
//! `ACTA_SPECIMEN=1`. Not part
//! of the product and not reachable from the UI.
//!
//! It shows how GPUI renders the bundled faces, dark and light side by
//! side: every sans role of docs/design/typography.md 3.2 on the four surfaces, the text
//! levels, the mono roles and the colour tokens.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, Div, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::theme::{FONT_MONO, FONT_SANS, RADIUS_MD, RADIUS_SM, Tokens};
use crate::type_scale::{self, MONO_ROLES, SANS_ROLES, TypeRole};

/// Taller than the main window so the whole specimen fits one screenshot.
pub const WINDOW_SIZE: (f32, f32) = (1440., 1380.);

pub struct Specimen;

fn sample(role: &TypeRole) -> &'static str {
    match role.name {
        "type.title" => "Embed the terminal",
        "type.subtitle" => "Appearance",
        "type.heading" => "Start with a project",
        "type.reading" => "Resize now reflows the grid.",
        "type.document" => "Read for minutes, not glanced at.",
        "type.ui" => "Sidebar tree, palette, 3 open",
        "type.ui-strong" => "Alexandre set status",
        "type.small" => "Updated 2 h ago, 12 Nov 2026",
        "type.label" => "Current state",
        _ => "acta setup --check",
    }
}

fn spec(role: &TypeRole) -> String {
    format!(
        "{} {}/{} {}",
        role.name.trim_start_matches("type."),
        role.size,
        role.line,
        role.weight.0
    )
}

fn caption(text: impl Into<String>, color: Hsla) -> Div {
    type_scale::SMALL.el().text_color(color).child(text.into())
}

/// One surface with every sans role.
/// `floating` dresses it as overlays are: `border.default` and `shadow.overlay`.
fn surface(name: &'static str, background: Hsla, floating: bool, tokens: &Tokens) -> Div {
    let header = div()
        .flex()
        .flex_row()
        .gap(px(12.))
        .child(
            caption(name, tokens.text_tertiary)
                .w(px(132.))
                .flex_shrink_0(),
        )
        .child(caption(FONT_SANS, tokens.text_tertiary).flex_1());

    let rows = SANS_ROLES.iter().map(|role| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(
                caption(spec(role), tokens.text_tertiary)
                    .w(px(132.))
                    .flex_shrink_0(),
            )
            .child(
                role.el()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(tokens.text_primary)
                    .child(sample(role)),
            )
    });

    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(16.))
        .bg(background)
        .border_1()
        .border_color(tokens.border_subtle)
        .rounded(px(RADIUS_MD))
        .when(floating, |this| {
            this.border_color(tokens.border_default)
                .shadow(vec![tokens.shadow_overlay()])
        })
        .child(header)
        .children(rows)
}

/// Body text in the three text levels, italics and tabular figures.
fn text_levels(tokens: &Tokens) -> Div {
    let reading = type_scale::READING;
    let tabular = type_scale::tabular();
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(caption(FONT_SANS, tokens.text_tertiary))
        .child(reading.el().text_color(tokens.text_primary).child(
            "text.primary: Rendering and input work on Windows; resize still drops the last row.",
        ))
        .child(
            reading
                .el()
                .text_color(tokens.text_secondary)
                .child("text.secondary: Run Claude Code inside Acta, in the project directory."),
        )
        .child(
            reading
                .el()
                .text_color(tokens.text_tertiary)
                .child("text.tertiary: Updated 2 h ago by Claude Code, Tue 14:05."),
        )
        .child(
            reading
                .el()
                .text_color(tokens.text_disabled)
                .child("text.disabled: a control that cannot be used."),
        )
        .child(
            reading
                .el()
                .text_color(tokens.accent)
                .child("accent: a link to the topic, the focus, the one primary action."),
        )
        .child(
            reading
                .el()
                .italic()
                .text_color(tokens.text_primary)
                .child("Italic 400, for Markdown emphasis only. Il1 O0 rn m, fi fl."),
        )
        .child(
            reading
                .el()
                .italic()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tokens.text_primary)
                .child("Semibold italic 600, strong emphasis inside emphasis."),
        )
        .child(
            reading
                .el()
                .text_color(tokens.text_primary)
                .child("Proportional figures 1111 0000 14:05 0.4.1"),
        )
        .child(
            reading
                .el()
                .font_features(tabular)
                .text_color(tokens.text_primary)
                .child("Tabular figures (tnum) 1111 0000 14:05 0.4.1"),
        )
}

fn mono_samples(tokens: &Tokens) -> Div {
    let rows = MONO_ROLES.iter().map(|role| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(
                caption(spec(role), tokens.text_tertiary)
                    .w(px(132.))
                    .flex_shrink_0(),
            )
            .child(
                role.el()
                    .text_color(tokens.text_primary)
                    .child("acta setup --check  0O 1lI |[]{} -> != ~/.local/bin"),
            )
    });
    let terminal = type_scale::TERMINAL;
    let boxes = "\u{250c}\u{2500}\u{2500}\u{252c}\u{2500}\u{2500}\u{2510} \u{256d}\u{2500}\u{2500}\u{256e}\n\
                 \u{2502}ab\u{2502}cd\u{2502} \u{2502}ef\u{2502}\n\
                 \u{251c}\u{2500}\u{2500}\u{253c}\u{2500}\u{2500}\u{2524} \u{2502}gh\u{2502}\n\
                 \u{2502}ij\u{2502}kl\u{2502} \u{2502}mn\u{2502}\n\
                 \u{2514}\u{2500}\u{2500}\u{2534}\u{2500}\u{2500}\u{2518} \u{2570}\u{2500}\u{2500}\u{256f}";
    let lines = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(caption(
            format!("{FONT_MONO} on bg.sunken"),
            tokens.text_tertiary,
        ))
        .children(rows)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child(
                    caption("italic 400", tokens.text_tertiary)
                        .w(px(132.))
                        .flex_shrink_0(),
                )
                .child(
                    type_scale::CODE
                        .el()
                        .italic()
                        .text_color(tokens.text_primary)
                        .child("Resize now reflows the grid."),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child(
                    caption("terminal bold 700", tokens.text_tertiary)
                        .w(px(132.))
                        .flex_shrink_0(),
                )
                .child(
                    terminal
                        .el()
                        .font_weight(FontWeight::BOLD)
                        .text_color(tokens.text_primary)
                        .child("error: could not save the entry  0123456789"),
                ),
        );
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(16.))
        .bg(tokens.bg_sunken)
        .border_1()
        .border_color(tokens.border_subtle)
        .rounded(px(RADIUS_MD))
        .child(lines)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(caption("box drawing, terminal 13/17", tokens.text_tertiary))
                .child(terminal.el().text_color(tokens.text_primary).child(boxes)),
        )
}

/// All colour tokens of one theme, each on that theme's own `bg.base`.
fn swatches(name: &'static str, theme: &Tokens, tokens: &Tokens) -> Div {
    let chips = theme.named().into_iter().map(|(token, color)| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .w(px(152.))
            .child(
                div()
                    .size(px(16.))
                    .flex_shrink_0()
                    .rounded(px(RADIUS_SM))
                    .bg(color)
                    .border_1()
                    .border_color(theme.border_default),
            )
            .child(caption(token, theme.text_secondary))
    });
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(caption(name, tokens.text_tertiary))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(4.))
                .p(px(12.))
                .bg(theme.bg_base)
                .border_1()
                .border_color(tokens.border_subtle)
                .rounded(px(RADIUS_MD))
                .children(chips),
        )
}

/// One theme's column: every role on its four surfaces, the text levels,
/// the mono roles and its colour tokens, all on that theme's `bg.base`.
fn theme_column(name: &'static str, t: &Tokens) -> Div {
    let pair = |a: Div, b: Div| div().flex().flex_row().gap(px(12.)).child(a).child(b);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(16.))
        .bg(t.bg_base)
        .text_color(t.text_primary)
        .child(
            type_scale::HEADING
                .el()
                .text_color(t.text_primary)
                .child(name),
        )
        .child(pair(
            surface("bg.sunken", t.bg_sunken, false, t),
            surface("bg.base", t.bg_base, false, t),
        ))
        .child(pair(
            surface("bg.raised", t.bg_raised, false, t),
            surface("bg.overlay", t.bg_overlay, true, t),
        ))
        .child(text_levels(t))
        .child(mono_samples(t))
        .child(swatches("Colour tokens", t, t))
}

impl Render for Specimen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Tokens>().clone();
        div()
            .id("specimen")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .child(
                caption(
                    format!(
                        "Acta {}. Every role of docs/design/typography.md 3.2 at its size / line height and \
                         weight, in {FONT_SANS} and {FONT_MONO}, in both themes whatever the \
                         Appearance setting.",
                        acta_core::VERSION
                    ),
                    t.text_tertiary,
                )
                .px(px(16.))
                .py(px(8.)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .child(theme_column("Dark", &Tokens::dark()))
                    .child(theme_column("Light", &Tokens::light())),
            )
    }
}
