// Spike for ADR 0001: GPUI (gpui-pre snapshot via gpui-kit) + gpui-component input,
// custom theme tokens, bundled font, GUI subsystem and embedded icon on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::borrow::Cow;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

const BG: u32 = 0x1c1a17;
const SURFACE: u32 = 0x26231f;
const BORDER: u32 = 0x3a352f;
const TEXT: u32 = 0xe8e2d9;
const MUTED: u32 = 0x9a9186;
const ACCENT: u32 = 0xd98f4a;

struct Shell {
    input: Entity<InputState>,
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .font_family("IBM Plex Sans")
            .child("Acta - GPUI spike, bundled IBM Plex Sans")
            .child(div().w(px(360.)).child(Input::new(&self.input)))
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);

        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "../assets/IBMPlexSans-Regular.ttf"
            ))])
            .expect("failed to register bundled font");

        // Own the tokens: start from the dark theme, then overwrite what the input uses.
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::update(cx, |theme| {
            theme.font_family = "IBM Plex Sans".into();
            theme.radius = px(4.);
            theme.colors.background = rgb(BG).into();
            theme.colors.foreground = rgb(TEXT).into();
            theme.colors.muted_foreground = rgb(MUTED).into();
            theme.colors.input = rgb(BORDER).into();
            theme.colors.border = rgb(BORDER).into();
            theme.colors.ring = rgb(ACCENT).into();
            theme.colors.caret = rgb(ACCENT).into();
            theme.colors.secondary = rgb(SURFACE).into();
        });

        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("Acta".into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(720.), px(440.)),
                cx,
            ))),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type here"));
            cx.new(|_| Shell { input })
        })
        .expect("failed to open window");

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
