//! The settings in force while the app runs: what `preferences.json` holds,
//! as a GPUI global. Every change applies at once, repaints every window and
//! is saved in the background (docs/design/interaction.md: no Save button anywhere).

use gpui_kit::{App, Global, actions};

use crate::i18n::Locale;
use crate::preferences::{self, Preferences, Writer};
use crate::theme::{self, ThemeSetting};
use crate::type_scale::{self, TextSize};
use crate::ui::{self, ReadingWidth};

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
    /// This launch runs a newer version than the last one (the status
    /// bar's "Updated to ..." for this session, components.md 5).
    just_updated: bool,
}

impl Global for Settings {}

/// Load the preferences and apply them. Call once, before any window opens.
pub fn init(cx: &mut App) {
    let current = preferences::load();
    theme::apply(current.theme, cx);
    type_scale::set_text_size(current.text_size);
    ui::set_reading_width(current.reading_width);
    let updated = just_updated(current.last_version.as_deref(), acta_core::VERSION);
    let first_record = current.last_version.as_deref() != Some(acta_core::VERSION);
    cx.set_global(Settings {
        current,
        writer: Writer::new(preferences::path()),
        just_updated: updated,
    });
    if first_record {
        change(cx, |prefs| {
            prefs.last_version = Some(acta_core::VERSION.into())
        });
    }
    cx.on_action(|_: &ThemeSystem, cx| set_theme(ThemeSetting::System, cx));
    cx.on_action(|_: &ThemeLight, cx| set_theme(ThemeSetting::Light, cx));
    cx.on_action(|_: &ThemeDark, cx| set_theme(ThemeSetting::Dark, cx));
    cx.on_action(|_: &LanguageEnglish, cx| set_language(Locale::En, cx));
    cx.on_action(|_: &LanguageFrench, cx| set_language(Locale::Fr, cx));
    // GPUI gives quit handlers 200 ms and then ends the process (on macOS
    // without dropping globals), so the last change is written here, on
    // this thread; it is a few bytes.
    cx.on_app_quit(|cx| {
        crate::resume::set_quitting();
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

/// Whether a launch on `now` follows one on another version. No record
/// (a first launch, or a build from before the record) is not an update.
pub fn just_updated(last: Option<&str>, now: &str) -> bool {
    last.is_some_and(|v| v != now)
}

/// This session runs a version that was just installed by an update.
pub fn updated_this_session(cx: &App) -> bool {
    cx.try_global::<Settings>().is_some_and(|s| s.just_updated)
}

pub fn current(cx: &App) -> &Preferences {
    &cx.global::<Settings>().current
}

/// Change something, apply nothing: the caller applies.
pub fn change(cx: &mut App, edit: impl FnOnce(&mut Preferences)) {
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

/// Change the size of reading text: every window relays out at once.
pub fn set_text_size(size: TextSize, cx: &mut App) {
    if current(cx).text_size == size {
        return;
    }
    change(cx, |prefs| prefs.text_size = size);
    type_scale::set_text_size(size);
    cx.refresh_windows();
}

/// Change the widest the content column gets, at once.
pub fn set_reading_width(width: ReadingWidth, cx: &mut App) {
    if current(cx).reading_width == width {
        return;
    }
    change(cx, |prefs| prefs.reading_width = width);
    ui::set_reading_width(width);
    cx.refresh_windows();
}

/// The chosen zoom, a step in percent (layout.md 4.9).
pub fn zoom(cx: &App) -> u16 {
    current(cx).zoom.unwrap_or(crate::zoom::DEFAULT)
}

/// Choose a zoom: every window lays out again at once.
pub fn set_zoom(step: u16, cx: &mut App) {
    let step = crate::zoom::nearest_step(i64::from(step));
    if zoom(cx) == step {
        return;
    }
    change(cx, |prefs| {
        prefs.zoom = (step != crate::zoom::DEFAULT).then_some(step)
    });
    cx.refresh_windows();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_change_of_version_counts_as_an_update() {
        assert!(just_updated(Some("0.3.0"), "0.4.1"));
        assert!(!just_updated(Some("0.4.1"), "0.4.1"));
        assert!(!just_updated(None, "0.4.1"));
    }
}
