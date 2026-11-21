//! Pane sizes of the workbench (docs/design/layout.md 4.2 and 4.5): the
//! sidebar, the page and the docks, and the order in which they give way
//! when the window narrows. Pure arithmetic, so it is tested without a
//! window.

/// layout.md 4.2: the sidebar is 260 wide, resizable 200 to 400.
pub const SIDEBAR_DEFAULT: f32 = 260.;
pub const SIDEBAR_MIN: f32 = 200.;
pub const SIDEBAR_MAX: f32 = 400.;
/// layout.md 4.2: the page is never narrower than 420.
pub const PAGE_MIN: f32 = 420.;
/// layout.md 4.5: the right dock is 648 (80 columns), 400 to 60% of the
/// window.
pub const DOCK_DEFAULT: f32 = 648.;
pub const DOCK_MIN: f32 = 400.;
const DOCK_MAX_SHARE: f32 = 0.6;
/// layout.md 4.5: the bottom dock is 320 high, 160 to 70% of the window.
pub const BOTTOM_DEFAULT: f32 = 320.;
pub const BOTTOM_MIN: f32 = 160.;
const BOTTOM_MAX_SHARE: f32 = 0.7;
/// docs/design/components.md 5, Resize handle: the hit area is 8 wide.
pub const HANDLE_HIT: f32 = 8.;
/// Palette "Widen ..." / "Narrow ...": steps of 40.
pub const STEP: f32 = 40.;

/// The sidebar's width for a stored or dragged value.
pub fn sidebar_width(wanted: f32) -> f32 {
    wanted.clamp(SIDEBAR_MIN, SIDEBAR_MAX)
}

/// The right dock's width in a window `window` wide: the wanted width
/// clamped to 400..60% of the window, and never so wide that the page
/// drops under its 420 (a tool window keeps its 400 minimum first).
pub fn dock_width(wanted: f32, window: f32) -> f32 {
    let max = (window * DOCK_MAX_SHARE)
        .min(window - PAGE_MIN)
        .max(DOCK_MIN);
    wanted.clamp(DOCK_MIN, max)
}

/// The bottom dock's height in a body `body` high (the window between the
/// titlebar and the status bar).
pub fn bottom_height(wanted: f32, body: f32) -> f32 {
    let max = (body * BOTTOM_MAX_SHARE).max(BOTTOM_MIN);
    wanted.clamp(BOTTOM_MIN, max)
}

/// Whether the sidebar fits beside the page and the right dock (`dock` is
/// 0 when no tool window is in it). Panes give way sidebar first: when it
/// does not fit, it auto-hides and comes back when space returns.
pub fn sidebar_fits(sidebar: f32, dock: f32, window: f32) -> bool {
    sidebar + PAGE_MIN + dock <= window
}

/// The sizes the user chose, kept in `preferences.json` (layout.md 4.7).
/// The right dock's width is per tool window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    pub sidebar: f32,
    pub terminal: f32,
    pub chat: f32,
    pub bottom: f32,
}

impl Default for Sizes {
    fn default() -> Self {
        Self {
            sidebar: SIDEBAR_DEFAULT,
            terminal: DOCK_DEFAULT,
            chat: DOCK_DEFAULT,
            bottom: BOTTOM_DEFAULT,
        }
    }
}

impl Sizes {
    pub fn from_prefs(prefs: &crate::preferences::Preferences) -> Self {
        let d = Self::default();
        let or = |v: Option<u16>, default: f32| v.map_or(default, f32::from);
        Self {
            sidebar: sidebar_width(or(prefs.sidebar_width, d.sidebar)),
            terminal: or(prefs.terminal_width, d.terminal).max(DOCK_MIN),
            chat: or(prefs.chat_width, d.chat).max(DOCK_MIN),
            bottom: or(prefs.chat_height, d.bottom).max(BOTTOM_MIN),
        }
    }

    /// Write the sizes into `prefs`; a default size is stored as none.
    pub fn to_prefs(self, prefs: &mut crate::preferences::Preferences) {
        let d = Self::default();
        let keep = |v: f32, default: f32| ((v - default).abs() >= 1.).then(|| v.round() as u16);
        prefs.sidebar_width = keep(self.sidebar, d.sidebar);
        prefs.terminal_width = keep(self.terminal, d.terminal);
        prefs.chat_width = keep(self.chat, d.chat);
        prefs.chat_height = keep(self.bottom, d.bottom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_the_minimum_window_the_dock_gets_460_and_the_page_420() {
        // layout.md 4.2: "At 880 with the right dock open the split is
        // page 420 / dock 460."
        let dock = dock_width(DOCK_DEFAULT, 880.);
        assert_eq!(dock, 460.);
        assert!(!sidebar_fits(SIDEBAR_DEFAULT, dock, 880.));
        assert!(sidebar_fits(SIDEBAR_DEFAULT, 0., 880.));
    }

    #[test]
    fn the_dock_keeps_its_default_in_a_wide_window_and_its_limits_when_dragged() {
        assert_eq!(dock_width(DOCK_DEFAULT, 1440.), DOCK_DEFAULT);
        assert!(sidebar_fits(SIDEBAR_DEFAULT, DOCK_DEFAULT, 1440.));
        assert_eq!(dock_width(100., 1440.), DOCK_MIN);
        assert_eq!(dock_width(5000., 1440.), 1440. * 0.6);
        // Narrower than 400 + 420: the tool window keeps its 400.
        assert_eq!(dock_width(DOCK_DEFAULT, 700.), DOCK_MIN);
    }

    #[test]
    fn sizes_round_trip_through_the_preferences_and_defaults_stay_unset() {
        let mut prefs = crate::preferences::Preferences::default();
        assert_eq!(Sizes::from_prefs(&prefs), Sizes::default());
        Sizes::default().to_prefs(&mut prefs);
        assert_eq!(prefs, crate::preferences::Preferences::default());
        let sizes = Sizes {
            sidebar: 320.,
            terminal: 500.,
            chat: 700.,
            bottom: 250.,
        };
        sizes.to_prefs(&mut prefs);
        assert_eq!(prefs.sidebar_width, Some(320));
        assert_eq!(Sizes::from_prefs(&prefs), sizes);
    }

    #[test]
    fn the_sidebar_and_the_bottom_dock_stay_in_their_ranges() {
        assert_eq!(sidebar_width(120.), SIDEBAR_MIN);
        assert_eq!(sidebar_width(300.), 300.);
        assert_eq!(sidebar_width(900.), SIDEBAR_MAX);
        assert_eq!(bottom_height(BOTTOM_DEFAULT, 800.), BOTTOM_DEFAULT);
        assert_eq!(bottom_height(10., 800.), BOTTOM_MIN);
        assert_eq!(bottom_height(900., 500.), 350.);
    }
}
