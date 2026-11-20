//! Bundled fonts (DESIGN.md 3.1), embedded in the executable. Static files
//! only; licences are next to the files in `assets/fonts`.

use std::borrow::Cow;

use gpui_kit::App;

macro_rules! font {
    ($path:literal) => {
        Cow::Borrowed(include_bytes!(concat!("../../../assets/fonts/", $path)).as_slice())
    };
}

/// Family of the comparison face in the type specimen.
pub const FONT_PLEX: &str = "IBM Plex Sans";

/// Register the bundled families. A failure is logged and the platform UI
/// font takes over: the window must open regardless.
pub fn register(cx: &App) {
    let fonts = vec![
        font!("source-sans-3/SourceSans3-Regular.ttf"),
        font!("source-sans-3/SourceSans3-It.ttf"),
        font!("source-sans-3/SourceSans3-Semibold.ttf"),
        font!("source-sans-3/SourceSans3-SemiboldIt.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Regular.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Italic.ttf"),
        font!("jetbrains-mono-nl/JetBrainsMonoNL-Bold.ttf"),
        // IBM Plex Sans is the named fallback of DESIGN.md 3.1. It is bundled
        // only so the type specimen can show both; drop the family that loses
        // once the Windows rendering check is ruled on.
        font!("ibm-plex-sans/IBMPlexSans-Regular.ttf"),
        font!("ibm-plex-sans/IBMPlexSans-Italic.ttf"),
        font!("ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
        font!("ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf"),
    ];
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        crate::log::line(&format!("fonts: could not register bundled fonts: {err}"));
    }
}
