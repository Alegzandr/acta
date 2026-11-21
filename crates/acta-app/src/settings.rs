//! The settings in force while the app runs: what `preferences.json` holds,
//! as a GPUI global. Every change applies at once, repaints every window and
//! is saved in the background (docs/design/interaction.md: no Save button anywhere).

use gpui_kit::{App, Global, actions};

use crate::i18n::Locale;
use crate::preferences::{self, Preferences, Writer};
use crate::theme::{self, ThemeSetting};

actions!(
    acta,
    [
        /// Palette: "Theme: System".
        ThemeSystem,
        /// Palette: "Theme: Light".
        ThemeLight,
        /// Palette: "Theme: Dark".
        ThemeDark,
        /// Palette: "Language: English".
        LanguageEnglish,
        /// Palette: "Language: Français".
        LanguageFrench,
    ]
);

pub struct Settings {
    current: Preferences,
    writer: Writer,
}

impl Global for Settings {}

/// Load the preferences and apply them. Call once, before any window opens.
pub fn init(cx: &mut App) {
    let current = preferences::load();
    theme::apply(current.theme, cx);
    cx.set_global(Settings {
        current,
        writer: Writer::new(preferences::path()),
    });
    cx.on_action(|_: &ThemeSystem, cx| set_theme(ThemeSetting::System, cx));
    cx.on_action(|_: &ThemeLight, cx| set_theme(ThemeSetting::Light, cx));
    cx.on_action(|_: &ThemeDark, cx| set_theme(ThemeSetting::Dark, cx));
    cx.on_action(|_: &LanguageEnglish, cx| set_language(Locale::En, cx));
    cx.on_action(|_: &LanguageFrench, cx| set_language(Locale::Fr, cx));
    // GPUI gives quit handlers 200 ms and then ends the process (on macOS
    // without dropping globals), so the last change is written here, on
    // this thread; it is a few bytes.
    cx.on_app_quit(|cx| {
        flush(cx);
        async {}
    })
    .detach();
}

/// Write any change not yet on disk, now, on the calling thread. Used at
/// quit and before an update restarts the app.
pub fn flush(cx: &mut App) {
    if let Some(settings) = cx.try_global::<Settings>() {
        let current = settings.current.clone();
        cx.global_mut::<Settings>().writer.flush(&current);
    }
}

pub fn current(cx: &App) -> &Preferences {
    &cx.global::<Settings>().current
}

/// Change something, apply nothing: the caller applies.
fn change(cx: &mut App, edit: impl FnOnce(&mut Preferences)) {
    let settings = cx.global_mut::<Settings>();
    edit(&mut settings.current);
    settings.writer.save(settings.current.clone());
}

pub fn set_theme(setting: ThemeSetting, cx: &mut App) {
    if current(cx).theme == setting {
        return;
    }
    change(cx, |prefs| prefs.theme = setting);
    theme::apply(setting, cx);
}

/// The interface language in force.
pub fn locale(cx: &App) -> Locale {
    current(cx).language
}

/// Change the language: every window repaints in the new language at once,
/// without a restart (docs/design/copy.md 10).
pub fn set_language(locale: Locale, cx: &mut App) {
    if current(cx).language == locale {
        return;
    }
    change(cx, |prefs| prefs.language = locale);
    cx.refresh_windows();
}
