//! The settings in force while the app runs: what `preferences.json` holds,
//! as a GPUI global. Every change applies at once, repaints every window and
//! is saved in the background (DESIGN.md: no Save button anywhere).

use gpui_kit::{App, Global, actions};

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
