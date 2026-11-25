//! The drawing side of section folds (components-records.md 5, Section
//! fold): where each header was last laid out, and the scroll anchoring of
//! a fold change ("folding and unfolding never move the reader"). The
//! rules are the pure ones of [`crate::fold`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::{AnyElement, ScrollHandle, point, px};

use crate::fold::{self, Section};

/// The headers of a page and a pending anchor; cheap to clone, shared with
/// the measures drawn in the page.
#[derive(Clone, Default)]
pub struct Anchor(Rc<RefCell<Inner>>);

#[derive(Default)]
struct Inner {
    /// Each header's y in the scrolled content (its window y less the
    /// scroll offset), as last laid out.
    ys: HashMap<Section, f32>,
    /// The header to keep in place, its y before the change and the
    /// scroll top then.
    pending: Option<(Section, f32, f32)>,
}

impl Anchor {
    /// An element over the header of `section` that records its place, and
    /// once a fold change has laid the page out again, scrolls so that the
    /// held header is back at its screen y (one frame later: GPUI lays out
    /// before it can be measured).
    pub fn measure(&self, section: Section, scroll: &ScrollHandle) -> AnyElement {
        let inner = Rc::clone(&self.0);
        let scroll = scroll.clone();
        crate::ui::measure(move |bounds, window| {
            let offset = f32::from(scroll.offset().y);
            let y = f32::from(bounds.origin.y) - offset;
            let mut inner = inner.borrow_mut();
            inner.ys.insert(section, y);
            let Some((held, before, top)) = inner.pending else {
                return;
            };
            if held != section {
                return;
            }
            inner.pending = None;
            let max = f32::from(scroll.max_offset().y);
            let new_top = fold::anchored_top(top, before, Some(y), max.max(top));
            if (new_top - -offset).abs() > 0.5 {
                // Measured sizes are already zoomed: GPUI's own `px`.
                scroll.set_offset(point(scroll.offset().x, -px(new_top)));
                window.on_next_frame(|window, _| window.refresh());
            }
        })
    }

    /// Before a fold change: hold `section`'s header where it is, or with
    /// `None` the header of the section that holds the top of the viewport
    /// (Collapse all, Expand all).
    pub fn hold(&self, section: Option<Section>, order: &[Section], scroll: &ScrollHandle) {
        let mut inner = self.0.borrow_mut();
        let top = -f32::from(scroll.offset().y);
        let view_top = f32::from(scroll.bounds().origin.y);
        let section = section.or_else(|| {
            let headers: Vec<(Section, f32)> = order
                .iter()
                .filter_map(|s| inner.ys.get(s).map(|y| (*s, *y - view_top)))
                .collect();
            fold::anchor(&headers, top)
        });
        inner.pending = section.and_then(|s| inner.ys.get(&s).map(|y| (s, *y, top)));
    }
}
