//! Dev-only window with a terminal, for manual checks of `acta-terminal`.
//!
//! ```text
//! cargo run -p acta-terminal --example shell -- [--light] [--no-focus] [-- program args...]
//! ```
//!
//! Without a program it runs `%ComSpec%` on Windows and `$SHELL` (or
//! `/bin/sh`) elsewhere, in the current directory. Enter on an ended session
//! restarts the same program.

use std::borrow::Cow;

use acta_terminal::{LaunchSpec, TerminalEvent, TerminalTheme, TerminalView};
use gpui_kit::{
    App, AppContext, Bounds, Context, Entity, Focusable, IntoElement, ParentElement, Render,
    Styled, WindowBounds, WindowOptions, div, px, size,
};

struct Host {
    terminal: Entity<TerminalView>,
}

impl Render for Host {
    fn render(
        &mut self,
        _window: &mut gpui_kit::Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().size_full().child(self.terminal.clone())
    }
}

fn default_spec() -> LaunchSpec {
    let program = if cfg!(windows) {
        std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
    };
    LaunchSpec {
        program,
        args: Vec::new(),
        cwd: std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()),
        env: Vec::new(),
    }
}

fn register_fonts(cx: &App) {
    macro_rules! font {
        ($path:literal) => {
            Cow::Borrowed(include_bytes!(concat!("../../../assets/fonts/", $path)).as_slice())
        };
    }
    let fonts = vec![
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Regular.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Italic.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Bold.ttf"),
    ];
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("could not register fonts: {err}");
    }
}

fn main() {
    let mut light = false;
    let mut focus = true;
    let mut spec = default_spec();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--light" => light = true,
            "--no-focus" => focus = false,
            "--" => {
                if let Some(program) = args.next() {
                    spec.program = program;
                    spec.args = args.by_ref().collect();
                }
            }
            other => eprintln!("ignored argument: {other}"),
        }
    }

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        register_fonts(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(900.), px(560.)),
                cx,
            ))),
            focus,
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            let terminal = cx.new(|cx| {
                let mut view = TerminalView::spawn(spec, cx);
                if light {
                    view.set_theme(TerminalTheme::light(), cx);
                }
                view
            });
            // Focus inside our own window; it does not raise the window.
            let handle = terminal.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
            cx.new(|cx| {
                cx.subscribe_in(
                    &terminal,
                    window,
                    |_, terminal, event, window, cx| match event {
                        TerminalEvent::TitleChanged(title) => {
                            window.set_window_title(title.as_deref().unwrap_or("Terminal"));
                        }
                        TerminalEvent::RestartRequested => terminal.update(cx, |view, cx| {
                            let spec = view.spec().clone();
                            view.restart(spec, cx);
                        }),
                        TerminalEvent::Exited(code) => eprintln!("exited: {code:?}"),
                        TerminalEvent::Bell => eprintln!("bell"),
                        TerminalEvent::TextSizeChanged(size) => eprintln!("text size {size}"),
                    },
                )
                .detach();
                Host { terminal }
            })
        })
        .expect("open window");
        cx.activate(focus);
    });
}
