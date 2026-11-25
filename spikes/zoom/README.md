# Spike: how the pinned GPUI can zoom a whole window (layout.md 4.9)

Question (PLAN.md, Zoom and section folds): a multiplier on the window's scale
factor, or a factor applied by the theme's size helpers?

## What the pinned GPUI (gpui-pre 0.3.12, through gpui-kit 0.10.0) allows

- `Window::scale_factor` is read from the platform window in
  `Window::bounds_changed` on every resize and DPI change. Its only setter,
  `Window::set_scale_factor`, exists under `cfg(any(test, feature =
  "test-support"))` and would be overwritten by the next resize. The
  viewport size, mouse positions and hit testing are also in the platform's
  logical pixels. A multiplied scale factor is not reachable without forking
  GPUI, which ADR 0001's exact pins rule out.
- `Window::set_rem_size` is public and read during layout. Every rem-based
  length (GPUI's `p_2`, `gap_1`, `text_sm`..., gpui-kit's input heights and
  its default font size) follows it.
- Acta's views size almost everything with `px(...)` literals taken from the
  design (about 1,300 calls), plus `border_1()` and friends, which are fixed
  `px(1.)`.
- Strokes are snapped by `round_stroke_to_device_pixel`: a non-zero border
  is rounded to whole device pixels and never under one, so a 1px border
  at 110% on a 100% display stays 1 device pixel, at 175% becomes 2, and on
  a 150% display 1px x 1.25 = 1.875 device px rounds to 2. Text is shaped at
  the requested size (no bitmap scaling); SVG icons are rasterised at their
  drawn size.

## Answer

A factor applied by the size helpers, in two places:

1. `zoom::px` replaces GPUI's `px` in every view of `acta-app` (the import
   changes, not the call sites); it multiplies by the applied zoom, read
   from a UI-thread `thread_local` that the main window sets at the start
   of each frame, before any child view renders. `border_N()` became
   `border(px(N.))`.
2. The window's rem size is set to 16 x the zoom in the same place, which
   scales GPUI's rem helpers, gpui-kit's components and, through
   `window.rem_size()`, the terminal grid (acta-terminal reads it to size
   its cell).

Pure layout code keeps working in effective px: what GPUI measures (bounds,
pointer positions, viewport) is converted with `zoom::eff` where it enters
that code, and scroll offsets written back stay in GPUI's logical px.
Window sizes (`WindowOptions`, `resize`) stay logical, never zoomed.

Limits found: GPUI has no runtime minimum-size setter and no window move, so
the zoomed minimum of 4.9 cannot be enforced by the OS; a window made smaller
than a step's minimum lowers the applied zoom instead (the chosen zoom stays),
and zooming in grows the window from its top-left corner up to the work area
without moving it.

Captures of the borders at 110%, 125% and 175% on a 100% and a 150% display
were not taken (captures were paused on the lead's machine); the snapping
rule above is read from the source.
