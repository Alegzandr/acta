//! Zoom (docs/design/layout.md 4.9): one factor that scales everything Acta
//! draws.
//!
//! The pinned GPUI takes the window's scale factor from the platform only
//! (its setter is test-only, and every resize reads the platform's again),
//! so zoom cannot ride on it. It is applied in two places instead, both
//! read during every frame (spikes/zoom/README.md):
//! - [`px`], which every view of the app uses in place of GPUI's own, so
//!   each length the design gives in px is multiplied by the factor;
//! - the window's rem size (16 x the factor), which scales GPUI's
//!   rem-based helpers, gpui-kit's components and the terminal grid.
//!
//! Layout rules read *effective* px (logical px divided by the factor):
//! [`eff`] converts what GPUI measures (bounds, mouse positions, scroll
//! offsets, viewport) back to them, so the pure layout code never sees
//! the zoom.

use std::cell::Cell;

use gpui_kit::Pixels;

/// The steps, in percent (4.9): the browser ladder.
pub const STEPS: [u16; 7] = [90, 100, 110, 125, 150, 175, 200];
pub const DEFAULT: u16 = 100;
/// What a zoomed window needs at least, in effective px (4.9).
pub const MIN_EFFECTIVE: (f32, f32) = (760., 480.);
/// The window minimum at 100%, in logical px (shell.rs `MIN_SIZE`).
pub const WINDOW_MIN: (f32, f32) = (880., 560.);

thread_local! {
    /// The applied zoom as a factor. Set by the main window's frame
    /// before any view renders; UI thread only.
    static FACTOR: Cell<f32> = const { Cell::new(1.) };
}

/// The applied factor (1.5 at 150%).
pub fn factor() -> f32 {
    FACTOR.with(Cell::get)
}

/// Sets the applied factor; returns whether it changed.
pub fn set_factor(factor: f32) -> bool {
    FACTOR.with(|f| {
        let changed = (f.get() - factor).abs() > f32::EPSILON;
        f.set(factor);
        changed
    })
}

/// A length of the design, in px at 100%, at the applied zoom.
pub fn px(value: f32) -> Pixels {
    gpui_kit::px(value * factor())
}

/// A length GPUI measured, in effective px (what the design's rules read).
pub fn eff(value: Pixels) -> f32 {
    f32::from(value) / factor()
}

/// The rem size of a window at the applied zoom.
pub fn rem_size() -> Pixels {
    gpui_kit::px(16. * factor())
}

/// A stored value read as the nearest step; a tie goes to the smaller.
pub fn nearest_step(value: i64) -> u16 {
    let mut best = DEFAULT;
    let mut distance = i64::MAX;
    for step in STEPS {
        let d = (i64::from(step) - value).abs();
        if d < distance {
            best = step;
            distance = d;
        }
    }
    best
}

pub fn as_factor(step: u16) -> f32 {
    f32::from(step) / 100.
}

/// The window's minimum size at `step`, in logical px: the usual minimum
/// up to 100%, max(880, 760 x zoom) by max(560, 480 x zoom) above (4.9).
pub fn window_min(step: u16) -> (f32, f32) {
    if step <= DEFAULT {
        return WINDOW_MIN;
    }
    let z = as_factor(step);
    (
        WINDOW_MIN.0.max(MIN_EFFECTIVE.0 * z),
        WINDOW_MIN.1.max(MIN_EFFECTIVE.1 * z),
    )
}

/// Whether a window (or a work area) of `w` x `h` logical px holds `step`.
/// Half a pixel of slack absorbs rounding of the platform's sizes.
pub fn fits(step: u16, w: f32, h: f32) -> bool {
    let (mw, mh) = window_min(step);
    step <= DEFAULT || (w + 0.5 >= mw && h + 0.5 >= mh)
}

/// The zoom the window shows: the chosen one, or the largest step under
/// it that the window holds (4.9, applied and chosen zoom). Steps at or
/// under 100% always apply.
pub fn applied(chosen: u16, w: f32, h: f32) -> u16 {
    STEPS
        .iter()
        .copied()
        .filter(|s| *s <= chosen && fits(*s, w, h))
        .max()
        .unwrap_or(chosen.min(DEFAULT))
}

/// The largest step a work area of `w` x `h` logical px can hold.
pub fn largest_fitting(w: f32, h: f32) -> u16 {
    applied(STEPS[STEPS.len() - 1], w, h)
}

/// The step after `from` going in, or `None` at the largest step the work
/// area holds (`Mod+=` then does nothing).
pub fn zoom_in(from: u16, work_w: f32, work_h: f32) -> Option<u16> {
    let limit = largest_fitting(work_w, work_h);
    STEPS.iter().copied().find(|s| *s > from && *s <= limit)
}

/// The step before `from`, or `None` at the floor.
pub fn zoom_out(from: u16) -> Option<u16> {
    STEPS.iter().rev().copied().find(|s| *s < from)
}

/// The window size that holds `step`, growing `size` only as needed and
/// never past the work area: zooming in on a small window grows it (4.9).
pub fn grown(step: u16, size: (f32, f32), work: (f32, f32)) -> Option<(f32, f32)> {
    let (mw, mh) = window_min(step);
    let want = (size.0.max(mw).min(work.0), size.1.max(mh).min(work.1));
    (want.0 > size.0 + 0.5 || want.1 > size.1 + 0.5).then_some(want)
}

/// What a zoom key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomKey {
    In,
    Out,
    Reset,
}

/// The zoom key bindings (4.9, Keys), as (keystroke, key, context).
/// `=`, `+` and `-` are the characters the layout types (GPUI reports
/// the shifted `+` with Shift consumed, and the numpad's `+`, `-` and `0`
/// as those characters); `0` is the physical key, which a French AZERTY
/// layout reports as `à`. In the terminal on Windows and Linux, `Ctrl+-`
/// is Claude Code's undo, so it is bound outside terminals only; the
/// terminal hands the others to the app (acta-terminal `kept_by_app`).
pub fn bindings(mac: bool) -> Vec<(&'static str, ZoomKey, Option<&'static str>)> {
    if mac {
        vec![
            ("cmd-=", ZoomKey::In, None),
            ("cmd-+", ZoomKey::In, None),
            ("cmd--", ZoomKey::Out, None),
            ("cmd-0", ZoomKey::Reset, None),
            ("cmd-à", ZoomKey::Reset, None),
        ]
    } else {
        vec![
            ("ctrl-=", ZoomKey::In, None),
            ("ctrl-+", ZoomKey::In, None),
            ("ctrl-shift-=", ZoomKey::In, None),
            ("ctrl--", ZoomKey::Out, Some("!Terminal")),
            ("ctrl-0", ZoomKey::Reset, None),
            ("ctrl-à", ZoomKey::Reset, None),
        ]
    }
}

/// What a keystroke as GPUI reports it does, per [`bindings`], with
/// `terminal` when a terminal has the keyboard (the tests' matcher).
#[cfg(test)]
pub fn key_action(keystroke: &str, mac: bool, terminal: bool) -> Option<ZoomKey> {
    bindings(mac)
        .into_iter()
        .find(|(k, _, context)| *k == keystroke && !(terminal && context.is_some()))
        .map(|(_, key, _)| key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bad_value_reads_as_the_nearest_step() {
        assert_eq!(nearest_step(100), 100);
        assert_eq!(nearest_step(150), 150);
        assert_eq!(nearest_step(0), 90);
        assert_eq!(nearest_step(-40), 90);
        assert_eq!(nearest_step(130), 125);
        assert_eq!(nearest_step(140), 150);
        assert_eq!(nearest_step(1000), 200);
        // A tie goes to the smaller step.
        assert_eq!(nearest_step(105), 100);
    }

    #[test]
    fn the_window_minimum_per_step() {
        assert_eq!(window_min(90), (880., 560.));
        assert_eq!(window_min(100), (880., 560.));
        assert_eq!(window_min(110), (880., 560.));
        assert_eq!(window_min(125), (950., 600.));
        assert_eq!(window_min(150), (1140., 720.));
        assert_eq!(window_min(175), (1330., 840.));
        assert_eq!(window_min(200), (1520., 960.));
    }

    #[test]
    fn applied_zoom_from_window_and_work_area_sizes() {
        // The examples of 4.9, maximised.
        assert_eq!(largest_fitting(1920., 1040.), 200);
        assert_eq!(largest_fitting(1536., 790.), 150);
        // Half height on a 1080p screen.
        assert_eq!(applied(150, 1920., 520.), 100);
        // A window made smaller drops the applied zoom, not the chosen.
        assert_eq!(applied(175, 1200., 760.), 150);
        assert_eq!(applied(175, 1400., 900.), 175);
        // 100% and under always apply.
        assert_eq!(applied(90, 880., 560.), 90);
        assert_eq!(applied(100, 880., 560.), 100);
        assert_eq!(applied(110, 880., 560.), 110);
    }

    #[test]
    fn keys_step_through_the_ladder_and_skip_what_does_not_fit() {
        let big = (2560., 1400.);
        assert_eq!(zoom_in(100, big.0, big.1), Some(110));
        assert_eq!(zoom_in(150, big.0, big.1), Some(175));
        assert_eq!(zoom_in(200, big.0, big.1), None);
        // 1536 x 790 holds 150 at most: Mod+= there does nothing.
        assert_eq!(zoom_in(150, 1536., 790.), None);
        assert_eq!(zoom_in(125, 1536., 790.), Some(150));
        assert_eq!(zoom_out(100), Some(90));
        assert_eq!(zoom_out(90), None);
        assert_eq!(zoom_out(175), Some(150));
    }

    #[test]
    fn zooming_in_grows_a_small_window_inside_its_work_area() {
        assert_eq!(
            grown(150, (1000., 700.), (1920., 1040.)),
            Some((1140., 720.))
        );
        assert_eq!(grown(150, (1400., 900.), (1920., 1040.)), None);
        assert_eq!(grown(110, (880., 560.), (1920., 1040.)), None);
        // Never past the work area.
        assert_eq!(
            grown(200, (1000., 700.), (1536., 1000.)),
            Some((1520., 960.))
        );
    }

    #[test]
    fn effective_sizes_feed_the_layout_clamps() {
        // A 1140 x 720 window at 150% is laid out as 760 x 480.
        set_factor(1.5);
        assert_eq!(eff(gpui_kit::px(1140.)), 760.);
        assert_eq!(eff(gpui_kit::px(720.)), 480.);
        assert_eq!(px(28.), gpui_kit::px(42.));
        assert_eq!(rem_size(), gpui_kit::px(24.));
        set_factor(1.);
        assert_eq!(px(28.), gpui_kit::px(28.));
    }

    /// What GPUI reports for the zoom keys on three layouts (Windows:
    /// the layout's unshifted character, Shift consumed when it changes
    /// the character; the numpad as its characters).
    #[test]
    fn the_key_matcher_on_us_azerty_and_qwertz() {
        let other = |k| key_action(k, false, false);
        // US: `=` key, Shift+`=` gives `+`, `-` key, `0`, numpad.
        assert_eq!(other("ctrl-="), Some(ZoomKey::In));
        assert_eq!(other("ctrl-+"), Some(ZoomKey::In));
        assert_eq!(other("ctrl--"), Some(ZoomKey::Out));
        assert_eq!(other("ctrl-0"), Some(ZoomKey::Reset));
        // AZERTY: `=` has its own key, `-` is on the 6 key, the 0 key
        // types `à` unshifted.
        assert_eq!(other("ctrl-="), Some(ZoomKey::In));
        assert_eq!(other("ctrl--"), Some(ZoomKey::Out));
        assert_eq!(other("ctrl-à"), Some(ZoomKey::Reset));
        // QWERTZ: `+` is unshifted, `-` has its key, `0` is `0`.
        assert_eq!(other("ctrl-+"), Some(ZoomKey::In));
        assert_eq!(other("ctrl--"), Some(ZoomKey::Out));
        assert_eq!(other("ctrl-0"), Some(ZoomKey::Reset));
        // Nothing else.
        assert_eq!(other("ctrl-9"), None);
        assert_eq!(other("ctrl-alt-="), None);
        // In a terminal Ctrl+- is Claude Code's.
        assert_eq!(key_action("ctrl--", false, true), None);
        assert_eq!(key_action("ctrl-=", false, true), Some(ZoomKey::In));
        assert_eq!(key_action("ctrl-0", false, true), Some(ZoomKey::Reset));
        // macOS.
        assert_eq!(key_action("cmd-=", true, false), Some(ZoomKey::In));
        assert_eq!(key_action("cmd--", true, true), Some(ZoomKey::Out));
        assert_eq!(key_action("cmd-0", true, false), Some(ZoomKey::Reset));
    }

    #[test]
    fn every_binding_parses() {
        for mac in [false, true] {
            for (k, _, _) in bindings(mac) {
                assert!(gpui_kit::Keystroke::parse(k).is_ok(), "{k}");
            }
        }
    }
}
