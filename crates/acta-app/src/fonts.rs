//! Bundled fonts (DESIGN.md 3.1), embedded in the executable. Static files
//! only; licences are next to the files in `assets/fonts`.

use std::borrow::Cow;

use gpui_kit::App;

macro_rules! font {
    ($path:literal) => {
        Cow::Borrowed(include_bytes!(concat!("../../../assets/fonts/", $path)).as_slice())
    };
}

/// Register the bundled families. A failure is logged and the platform UI
/// font takes over: the window must open regardless.
pub fn register(cx: &App) {
    let fonts = vec![
        font!("inter/Inter-Regular.ttf"),
        font!("inter/Inter-Italic.ttf"),
        font!("inter/Inter-SemiBold.ttf"),
        font!("inter/Inter-SemiBoldItalic.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Regular.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Italic.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Bold.ttf"),
    ];
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        crate::log::line(&format!("fonts: could not register bundled fonts: {err}"));
    }
}
