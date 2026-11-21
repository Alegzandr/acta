//! The workbench layout in pixels (docs/design/workbench.md 4.8.2, 4.8.6,
//! 4.8.7, 4.8.10): how big each area is in a window of a given size, which
//! areas give way when it narrows, how groups share an area, snap to hide,
//! and the drop zones of a drag. Pure arithmetic over [`Layout`], so it is
//! tested without a window; the shell measures the rectangles.

use crate::layout::{Direction, Edge, Layout, Place, Side, Spot, Tab, TabId, Target, ViewId};

/// 4.8.2: the page area keeps 420 with one group, 400 per group when
/// split, and 200 of height.
pub const PAGE_MIN_W: f32 = 420.;
pub const GROUP_MIN_W: f32 = 400.;
pub const PAGE_MIN_H: f32 = 200.;
/// 4.8.2: sidebars up to 60% of the window, the bottom panel up to 70% of
/// the body.
const SIDE_MAX_SHARE: f32 = 0.6;
const BOTTOM_MAX_SHARE: f32 = 0.7;
/// An empty area shown on purpose (4.8.2) keeps this much.
const EMPTY_MIN: f32 = 160.;
/// 4.8.7: the strip standing for a hidden area, and the edge zones.
pub const STRIP: f32 = 24.;
const EDGE_SHARE: f32 = 0.25;
/// 4.8.6: palette "Grow focused area" / "Shrink focused area".
pub const STEP: f32 = 40.;
/// components.md 5, Resize handle: the hit area is 8 wide.
pub const HANDLE_HIT: f32 = 8.;
/// 4.8.7: the insertion line between tabs is 20 high.
const LINE_HEIGHT: f32 = 20.;

/// A rectangle in window coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// The half of this rectangle on `edge`.
    pub fn half(&self, edge: Edge) -> Rect {
        match edge {
            Edge::Left => Rect::new(self.x, self.y, self.w / 2., self.h),
            Edge::Right => Rect::new(self.x + self.w / 2., self.y, self.w / 2., self.h),
            Edge::Top => Rect::new(self.x, self.y, self.w, self.h / 2.),
            Edge::Bottom => Rect::new(self.x, self.y + self.h / 2., self.w, self.h / 2.),
        }
    }
}

/// The minimum of an area along its own axis (width for a sidebar, height
/// for the bottom panel): the largest minimum of its groups' active views.
pub fn side_min(layout: &Layout, side: Side) -> f32 {
    let area = layout.side(side);
    area.active_views()
        .map(|v| match side {
            Side::Left | Side::Right => v.min().0,
            Side::Bottom => v.min().1,
        })
        .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))))
        .unwrap_or(EMPTY_MIN)
}

/// The areas as a window `width` wide with a body `body` high shows them
/// (4.8.2, 4.8.10). Nothing here is saved: the layout keeps what the user
/// chose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The left sidebar's width beside the page; `None` when hidden or
    /// auto-hidden.
    pub left: Option<f32>,
    /// The left sidebar is shown in the layout but there is no room for it
    /// (step 1 of 4.8.10): its toggle shows it over the page.
    pub left_auto_hidden: bool,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    /// The width and height of the page area.
    pub page_w: f32,
    pub page_h: f32,
    /// The page area shows only its focused group (step 2).
    pub page_folded: bool,
    /// A side area with two groups shows only its first (4.8.10).
    pub folded: [bool; 3],
}

impl Frame {
    pub fn side(&self, side: Side) -> Option<f32> {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
            Side::Bottom => self.bottom,
        }
    }

    pub fn side_folded(&self, side: Side) -> bool {
        self.folded[side as usize]
    }
}

/// The width a shown sidebar of `size` takes in a window `width` wide.
pub fn sidebar_width(layout: &Layout, side: Side, width: f32) -> f32 {
    let min = side_min(layout, side);
    let size = layout.side(side).size;
    let max = (width * SIDE_MAX_SHARE).max(min);
    if side == Side::Right {
        // The page keeps its 420 first; the view keeps its minimum.
        let max = max.min(width - PAGE_MIN_W).max(min);
        return size.clamp(min, max);
    }
    size.clamp(min, max)
}

/// The height a shown bottom panel takes in a body `body` high.
pub fn bottom_height(layout: &Layout, body: f32) -> f32 {
    let min = side_min(layout, Side::Bottom);
    let max = (body * BOTTOM_MAX_SHARE).min(body - PAGE_MIN_H).max(min);
    layout.bottom.size.clamp(min, max)
}

pub fn frame(layout: &Layout, width: f32, body: f32) -> Frame {
    let right = layout
        .right
        .shown
        .then(|| sidebar_width(layout, Side::Right, width));
    let left_wanted = layout
        .left
        .shown
        .then(|| sidebar_width(layout, Side::Left, width));
    let left_fits = left_wanted.is_some_and(|l| l + PAGE_MIN_W + right.unwrap_or(0.) <= width);
    let left = left_wanted.filter(|_| left_fits);
    let page_w = width - left.unwrap_or(0.) - right.unwrap_or(0.);
    let bottom = layout.bottom.shown.then(|| bottom_height(layout, body));
    let page_h = body - bottom.unwrap_or(0.);
    let n = layout.page.groups.len() as f32;
    let page_folded = layout.page.groups.len() > 1
        && match layout.page.direction {
            Direction::Row => page_w < GROUP_MIN_W * n,
            Direction::Column => page_h < PAGE_MIN_H * n,
        };
    let mut folded = [false; 3];
    for side in Side::ALL {
        let area = layout.side(side);
        if area.groups.len() < 2 {
            continue;
        }
        let needed: f32 = area
            .active_views()
            .map(|v| match side.direction() {
                Direction::Column => v.min().1,
                Direction::Row => v.min().0,
            })
            .sum();
        let room = match side {
            Side::Left | Side::Right => body,
            Side::Bottom => page_w,
        };
        folded[side as usize] = room < needed;
    }
    Frame {
        left,
        left_auto_hidden: left_wanted.is_some() && !left_fits,
        right,
        bottom,
        page_w,
        page_h,
        page_folded,
        folded,
    }
}

/// The sizes of groups sharing `total` by `shares`, each at least its
/// minimum while the total allows (4.8.2).
pub fn group_sizes(total: f32, shares: &[f32], mins: &[f32]) -> Vec<f32> {
    let mut sizes: Vec<f32> = shares.iter().map(|s| s * total).collect();
    let need: f32 = mins.iter().sum();
    if need > total {
        // Not enough room for every minimum: in proportion to them.
        return mins.iter().map(|m| total * m / need.max(1.)).collect();
    }
    for _ in 0..sizes.len() {
        let short: f32 = sizes.iter().zip(mins).map(|(s, m)| (m - s).max(0.)).sum();
        if short <= 0. {
            break;
        }
        let spare: f32 = sizes.iter().zip(mins).map(|(s, m)| (s - m).max(0.)).sum();
        for (s, m) in sizes.iter_mut().zip(mins) {
            if *s < *m {
                *s = *m;
            } else if spare > 0. {
                *s -= short * (*s - m) / spare;
            }
        }
    }
    sizes
}

/// The shares after the handle between group `at` and `at + 1` moved by
/// `delta` pixels, the two groups stopping at their minimums (groups never
/// snap, 4.8.6).
pub fn resize_shares(shares: &[f32], at: usize, delta: f32, total: f32, mins: &[f32]) -> Vec<f32> {
    let mut sizes = group_sizes(total, shares, mins);
    if at + 1 >= sizes.len() || total <= 0. {
        return shares.to_vec();
    }
    let pair = sizes[at] + sizes[at + 1];
    let first = (sizes[at] + delta).clamp(
        mins[at].min(pair),
        (pair - mins[at + 1]).max(mins[at].min(pair)),
    );
    sizes[at] = first;
    sizes[at + 1] = pair - first;
    let sum: f32 = sizes.iter().sum();
    sizes.iter().map(|s| s / sum.max(1.)).collect()
}

/// A side area's size while its handle is dragged to `wanted` (4.8.6):
/// under half its active view's minimum it snaps hidden (`None`), and back
/// at its minimum past that point.
pub fn drag_size(wanted: f32, min: f32) -> Option<f32> {
    (wanted >= min / 2.).then(|| wanted.max(min))
}

// ---- drop zones (4.8.7) ----

/// A group as the last frame drew it.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub place: Place,
    pub group: usize,
    /// The whole group: its header or tab bar and its content.
    pub bounds: Rect,
    /// The header (side areas) or tab bar (page groups), when shown.
    pub header: Option<Rect>,
    /// Each tab of the header or bar, in group order.
    pub tabs: Vec<Rect>,
}

/// A hidden area's strip: where the pointer stands for it, and where the
/// area would appear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strip {
    pub side: Side,
    pub hit: Rect,
    pub preview: Rect,
}

/// What is dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dragged {
    pub tab: Tab,
    /// For an Inbox or Knowledge page tab: the view it becomes outside the
    /// page area.
    pub as_view: Option<ViewId>,
}

/// Where a drop lands, and what shows it before release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub target: Target,
    /// The bounds the dragged tab will take (4.8.7, drop preview).
    pub preview: Rect,
    /// The insertion line between tabs, for a drop on a tab row.
    pub line: Option<Rect>,
}

/// The strips of the hidden areas: 24 along the body edge where each would
/// appear (above the status bar under the page area for the bottom panel),
/// with its preview at its remembered size. `page` is the page area.
pub fn strips(layout: &Layout, body: Rect, page: Rect) -> Vec<Strip> {
    let mut out = Vec::new();
    if !layout.left.shown {
        let w = layout.left.size.min(body.w * SIDE_MAX_SHARE);
        out.push(Strip {
            side: Side::Left,
            hit: Rect::new(body.x, body.y, STRIP, body.h),
            preview: Rect::new(body.x, body.y, w, body.h),
        });
    }
    if !layout.right.shown {
        let w = layout.right.size.min(body.w * SIDE_MAX_SHARE);
        out.push(Strip {
            side: Side::Right,
            hit: Rect::new(body.right() - STRIP, body.y, STRIP, body.h),
            preview: Rect::new(body.right() - w, body.y, w, body.h),
        });
    }
    if !layout.bottom.shown {
        let h = layout.bottom.size.min(page.h * BOTTOM_MAX_SHARE);
        out.push(Strip {
            side: Side::Bottom,
            hit: Rect::new(page.x, page.bottom() - STRIP, page.w, STRIP),
            preview: Rect::new(page.x, page.bottom() - h, page.w, h),
        });
    }
    out
}

/// The minimum width and height a tab needs in a group of `place`.
fn tab_min(tab: Tab, place: Place) -> (f32, f32) {
    match (tab, place) {
        (Tab::View(v), Place::Side(_)) => v.min(),
        (_, Place::Page) => (GROUP_MIN_W, PAGE_MIN_H),
        (Tab::Page(_), Place::Side(_)) => (GROUP_MIN_W, PAGE_MIN_H),
    }
}

/// The drop zone under the pointer at (`x`, `y`), if any. A zone exists
/// only where the drop would succeed and change something, and its
/// preview is exactly where the tab lands (4.8.11, rules 5 and 6).
pub fn zone_at(
    layout: &Layout,
    regions: &[Region],
    strips: &[Strip],
    dragged: Dragged,
    x: f32,
    y: f32,
) -> Option<Zone> {
    let is_view = matches!(dragged.tab, Tab::View(_)) || dragged.as_view.is_some();
    if is_view {
        for strip in strips {
            if strip.hit.contains(x, y) {
                let target = Target::Area(strip.side);
                return lands(layout, dragged, target).then_some(Zone {
                    target,
                    preview: strip.preview,
                    line: None,
                });
            }
        }
    }
    let region = regions.iter().find(|r| r.bounds.contains(x, y))?;
    let place = region.place;
    let accepts = match (dragged.tab, place) {
        (Tab::Page(_), Place::Side(_)) => dragged.as_view.is_some(),
        _ => true,
    };
    if !accepts {
        return None;
    }
    let group = region.group;
    let into = |index| Target::Into {
        place,
        group,
        index,
    };
    if let Some(header) = region.header
        && header.contains(x, y)
    {
        let single_view = matches!(place, Place::Side(_)) && region.tabs.len() <= 1;
        let (target, line) = if single_view || region.tabs.is_empty() {
            (into(None), None)
        } else {
            let index = region.tabs.iter().filter(|t| t.x + t.w / 2. <= x).count();
            let line_x = match index {
                0 => region.tabs[0].x,
                i => region.tabs[i - 1].right(),
            };
            let line_y = header.y + (header.h - LINE_HEIGHT) / 2.;
            (
                into(Some(index)),
                Some(Rect::new(line_x - 1., line_y, 2., LINE_HEIGHT)),
            )
        };
        return lands(layout, dragged, target).then_some(Zone {
            target,
            preview: region.bounds,
            line,
        });
    }
    // The content: an edge that may split, else the centre.
    let content = match region.header {
        Some(h) if h.bottom() <= region.bounds.bottom() => Rect::new(
            region.bounds.x,
            h.bottom(),
            region.bounds.w,
            region.bounds.bottom() - h.bottom(),
        ),
        _ => region.bounds,
    };
    let from = layout.locate(dragged.tab);
    let moving = match (dragged.tab, place) {
        (Tab::Page(_), Place::Side(_)) => dragged.as_view.map(Tab::View).unwrap_or(dragged.tab),
        (Tab::View(v), Place::Page) if v.is_page_kind() => Tab::Page(TabId(u64::MAX)),
        _ => dragged.tab,
    };
    let (mw, mh) = tab_min(moving, place);
    let area = layout.area(place);
    let (gw, gh) = area
        .groups
        .get(group)
        .and_then(|g| g.active_tab())
        .map_or((0., 0.), |t| tab_min(t, place));
    let mut best: Option<(f32, Edge)> = None;
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        let leaving = from.filter(|f| f.place == place);
        if !layout.may_split(place, group, edge, leaving) {
            continue;
        }
        let fits = match edge.direction() {
            Direction::Row => content.w / 2. >= mw.max(gw) && content.h >= mh.max(gh),
            Direction::Column => content.h / 2. >= mh.max(gh) && content.w >= mw.max(gw),
        };
        if !fits {
            continue;
        }
        let distance = match edge {
            Edge::Left => x - content.x,
            Edge::Right => content.right() - x,
            Edge::Top => y - content.y,
            Edge::Bottom => content.bottom() - y,
        };
        let depth = match edge.direction() {
            Direction::Row => content.w * EDGE_SHARE,
            Direction::Column => content.h * EDGE_SHARE,
        };
        if distance < depth && best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, edge));
        }
    }
    let (target, preview) = match best {
        Some((_, edge)) => (
            Target::Split { place, group, edge },
            region.bounds.half(edge),
        ),
        None => (into(None), region.bounds),
    };
    lands(layout, dragged, target).then_some(Zone {
        target,
        preview,
        line: None,
    })
}

/// Whether dropping `dragged` at `target` would succeed and change the
/// layout: the same call the drop makes, on a copy.
pub fn lands(layout: &Layout, dragged: Dragged, target: Target) -> bool {
    let mut copy = layout.clone();
    drop_into(&mut copy, dragged, target, TabId(u64::MAX)).is_ok()
}

/// Apply a drop: a plain move, or Inbox and Knowledge crossing between
/// page and view (`new_page` is the tab a view becomes in the page area).
pub fn drop_into(
    layout: &mut Layout,
    dragged: Dragged,
    target: Target,
    new_page: TabId,
) -> Result<Spot, crate::layout::Refused> {
    let place = match target {
        Target::Into { place, .. } | Target::Split { place, .. } => place,
        Target::Area(side) => Place::Side(side),
    };
    match (dragged.tab, place) {
        (Tab::Page(id), Place::Side(_)) => match dragged.as_view {
            Some(view) => layout.page_to_view(id, view, target),
            None => Err(crate::layout::Refused::Invalid),
        },
        (Tab::View(v), Place::Page) if v.is_page_kind() => layout.view_to_page(v, new_page, target),
        (tab, _) => layout.move_tab(tab, target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Layout, TabId};

    const P1: TabId = TabId(1);
    const P2: TabId = TabId(2);
    const RIGHT: Place = Place::Side(Side::Right);

    fn base() -> Layout {
        Layout::defaults(vec![P1, P2], true)
    }

    #[test]
    fn at_the_minimum_window_the_right_sidebar_gets_460_and_the_left_gives_way() {
        // 4.8.10: "at 880 it gets 460 against the page's 420".
        let mut l = base();
        l.show_view(ViewId::Terminal);
        let f = frame(&l, 880., 500.);
        assert_eq!(f.right, Some(460.));
        assert_eq!(f.left, None);
        assert!(f.left_auto_hidden);
        assert_eq!(f.page_w, 420.);
        let wide = frame(&l, 1440., 840.);
        assert_eq!((wide.left, wide.right), (Some(260.), Some(648.)));
        assert!(!wide.left_auto_hidden);
        // Without the right sidebar the left one fits at 880.
        l.set_area_shown(Side::Right, false);
        assert_eq!(frame(&l, 880., 500.).left, Some(260.));
    }

    #[test]
    fn an_area_renders_at_least_at_its_active_views_minimum() {
        let mut l = base();
        // The terminal moved into the 260 left sidebar: 400 while active,
        // the remembered 260 untouched.
        l.move_tab(
            crate::layout::Tab::View(ViewId::Terminal),
            Target::Into {
                place: Place::Side(Side::Left),
                group: 0,
                index: None,
            },
        )
        .unwrap();
        assert_eq!(frame(&l, 1440., 840.).left, Some(400.));
        assert_eq!(l.left.size, 260.);
        l.left.groups[0].active = 0;
        assert_eq!(frame(&l, 1440., 840.).left, Some(260.));
        // And at most 60% of the window.
        l.left.size = 3000.;
        assert_eq!(frame(&l, 2000., 840.).left, Some(1200.));
    }

    #[test]
    fn the_bottom_panel_leaves_the_page_200_and_keeps_160_at_the_minimum() {
        let mut l = base();
        l.move_tab(
            crate::layout::Tab::View(ViewId::Terminal),
            Target::Area(Side::Bottom),
        )
        .unwrap();
        l.bottom.size = 2000.;
        let f = frame(&l, 880., 500.);
        assert_eq!(f.bottom, Some(300.));
        assert_eq!(f.page_h, 200.);
        l.bottom.size = 10.;
        assert_eq!(frame(&l, 880., 500.).bottom, Some(160.));
    }

    #[test]
    fn a_split_folds_when_its_groups_lose_their_400() {
        let mut l = base();
        l.split_active(crate::layout::Direction::Row).unwrap();
        assert!(!frame(&l, 1440., 840.).page_folded);
        // 1100 - 260 = 840 >= 800.
        assert!(!frame(&l, 1100., 840.).page_folded);
        assert!(frame(&l, 1000., 840.).page_folded);
        // The left sidebar gives way first: 880 without it fits 2 x 400.
        let f = frame(&l, 880., 500.);
        assert!(f.left.is_none() || f.page_folded);
    }

    #[test]
    fn two_groups_in_a_sidebar_fold_when_the_height_is_short() {
        let mut l = base();
        l.show_view(ViewId::Terminal);
        l.split_off(ViewId::Chat).unwrap();
        // Terminal 160 + chat 240 = 400.
        assert!(!frame(&l, 1440., 500.).side_folded(Side::Right));
        assert!(frame(&l, 1440., 380.).side_folded(Side::Right));
    }

    #[test]
    fn group_sizes_keep_each_minimum() {
        assert_eq!(group_sizes(1000., &[0.5, 0.5], &[400., 400.]), [500., 500.]);
        let s = group_sizes(1000., &[0.9, 0.1], &[400., 400.]);
        assert!(
            (s[0] - 600.).abs() < 0.01 && (s[1] - 400.).abs() < 0.01,
            "{s:?}"
        );
        let tight = group_sizes(600., &[0.5, 0.5], &[400., 400.]);
        assert_eq!(tight, [300., 300.]);
        let shares = resize_shares(&[0.5, 0.5], 0, 300., 1000., &[400., 400.]);
        assert!(
            (shares[0] - 0.6).abs() < 1e-4,
            "stops at the second group's minimum"
        );
        let shares = resize_shares(&[0.5, 0.5], 0, -50., 1000., &[400., 400.]);
        assert!((shares[0] - 0.45).abs() < 1e-4);
    }

    #[test]
    fn snap_to_hide_under_half_the_minimum() {
        assert_eq!(drag_size(300., 200.), Some(300.));
        assert_eq!(drag_size(150., 200.), Some(200.), "past half: the minimum");
        assert_eq!(drag_size(99., 200.), None);
    }

    /// The drawn regions of the default layout with the right sidebar shown
    /// in a 1440 x 840 body at the origin: left 0..260, page 260..792,
    /// right 792..1440.
    fn regions(l: &Layout) -> Vec<Region> {
        let mut out = vec![Region {
            place: Place::Side(Side::Left),
            group: 0,
            bounds: Rect::new(0., 0., 260., 840.),
            header: Some(Rect::new(0., 0., 260., 28.)),
            tabs: vec![Rect::new(8., 2., 70., 24.)],
        }];
        let n = l.page.groups.len();
        let w = 532. / n as f32;
        for (g, group) in l.page.groups.iter().enumerate() {
            let tabs = group.tabs.len();
            out.push(Region {
                place: Place::Page,
                group: g,
                bounds: Rect::new(260. + w * g as f32, 0., w, 840.),
                header: Some(Rect::new(260. + w * g as f32, 0., w, 32.)),
                tabs: (0..tabs)
                    .map(|i| Rect::new(260. + w * g as f32 + 150. * i as f32, 0., 150., 32.))
                    .collect(),
            });
        }
        out.push(Region {
            place: RIGHT,
            group: 0,
            bounds: Rect::new(792., 0., 648., 840.),
            header: Some(Rect::new(792., 0., 648., 28.)),
            tabs: vec![Rect::new(800., 2., 80., 24.), Rect::new(884., 2., 60., 24.)],
        });
        out
    }

    fn shown() -> Layout {
        let mut l = base();
        l.show_view(ViewId::Terminal);
        l
    }

    #[test]
    fn dragging_the_chat_previews_exactly_where_it_lands() {
        let l = shown();
        let regions = regions(&l);
        let body = Rect::new(0., 0., 1440., 840.);
        let page = Rect::new(260., 0., 532., 840.);
        let strips = strips(&l, body, page);
        let chat = Dragged {
            tab: crate::layout::Tab::View(ViewId::Chat),
            as_view: None,
        };
        // 8.18: the 24 strip along the bottom of the page area.
        let zone = zone_at(&l, &regions, &strips, chat, 500., 830.).unwrap();
        assert_eq!(zone.target, Target::Area(Side::Bottom));
        assert_eq!(zone.preview, Rect::new(260., 520., 532., 320.));
        // The page area's centre: a view tab in that group.
        let zone = zone_at(&l, &regions, &strips, chat, 520., 400.).unwrap();
        assert_eq!(
            zone.target,
            Target::Into {
                place: Place::Page,
                group: 0,
                index: None
            }
        );
        assert_eq!(zone.preview, regions[1].bounds);
        // The page area's left edge: a new group there, half the old one.
        // (532 / 2 = 266 < 400: too narrow to split, so it is the centre.)
        let zone = zone_at(&l, &regions, &strips, chat, 270., 400.).unwrap();
        assert!(matches!(zone.target, Target::Into { .. }));
        // Its own group's centre is its own place: no zone.
        assert_eq!(zone_at(&l, &regions, &strips, chat, 1100., 400.), None);
        // Before the terminal in the header: an insertion line.
        let zone = zone_at(&l, &regions, &strips, chat, 805., 10.).unwrap();
        assert_eq!(
            zone.target,
            Target::Into {
                place: RIGHT,
                group: 0,
                index: Some(0)
            }
        );
        assert_eq!(zone.line.map(|r| r.h), Some(20.));
        // Just after itself: no change, no zone.
        assert_eq!(zone_at(&l, &regions, &strips, chat, 940., 10.), None);
        // The bottom of the right sidebar: a second group, stacked.
        let zone = zone_at(&l, &regions, &strips, chat, 1100., 800.).unwrap();
        assert_eq!(
            zone.target,
            Target::Split {
                place: RIGHT,
                group: 0,
                edge: Edge::Bottom
            }
        );
        assert_eq!(zone.preview, Rect::new(792., 420., 648., 420.));
        // The right edge of a sidebar is not a split: centre.
        let zone = zone_at(&l, &regions, &strips, chat, 1430., 400.);
        assert_eq!(zone, None, "the centre of its own group");
        // Outside every region: nothing.
        assert_eq!(zone_at(&l, &regions, &strips, chat, 2000., 400.), None);
    }

    #[test]
    fn an_empty_area_shown_on_purpose_takes_a_view() {
        let mut l = shown();
        l.set_area_shown(Side::Bottom, true);
        let area = Rect::new(260., 680., 532., 160.);
        let strip = Strip {
            side: Side::Bottom,
            hit: area,
            preview: area,
        };
        let chat = Dragged {
            tab: crate::layout::Tab::View(ViewId::Chat),
            as_view: None,
        };
        let zone = zone_at(&l, &[], &[strip], chat, 400., 700.).unwrap();
        assert_eq!(zone.target, Target::Area(Side::Bottom));
        let mut after = l.clone();
        drop_into(&mut after, chat, zone.target, TabId(99)).unwrap();
        assert!(after.is_visible(ViewId::Chat));
        assert_eq!(
            after
                .locate(crate::layout::Tab::View(ViewId::Chat))
                .unwrap()
                .place,
            Place::Side(Side::Bottom)
        );
    }

    #[test]
    fn a_page_shows_drop_zones_only_in_the_page_area() {
        let l = shown();
        let regions = regions(&l);
        let body = Rect::new(0., 0., 1440., 840.);
        let strips = strips(&l, body, Rect::new(260., 0., 532., 840.));
        let page = Dragged {
            tab: crate::layout::Tab::Page(P1),
            as_view: None,
        };
        assert_eq!(zone_at(&l, &regions, &strips, page, 100., 400.), None);
        assert_eq!(zone_at(&l, &regions, &strips, page, 1100., 400.), None);
        // No strip for a page: the page group's bottom edge splits it.
        let zone = zone_at(&l, &regions, &strips, page, 500., 830.).unwrap();
        assert!(matches!(
            zone.target,
            Target::Split {
                place: Place::Page,
                edge: Edge::Bottom,
                ..
            }
        ));
        // After the second tab in the tab bar.
        let zone = zone_at(&l, &regions, &strips, page, 700., 10.).unwrap();
        assert_eq!(
            zone.target,
            Target::Into {
                place: Place::Page,
                group: 0,
                index: Some(2)
            }
        );
        // An inbox page goes to a side area as the Inbox view.
        let inbox = Dragged {
            tab: crate::layout::Tab::Page(P2),
            as_view: Some(ViewId::Inbox),
        };
        let zone = zone_at(&l, &regions, &strips, inbox, 100., 10.).unwrap();
        assert_eq!(
            zone.target,
            Target::Into {
                place: Place::Side(Side::Left),
                group: 0,
                index: None
            }
        );
    }

    #[test]
    fn a_wide_page_area_splits_at_its_edges_and_corners_go_to_the_nearer_edge() {
        let l = base();
        let regions = vec![Region {
            place: Place::Page,
            group: 0,
            bounds: Rect::new(0., 0., 1200., 800.),
            header: Some(Rect::new(0., 0., 1200., 32.)),
            tabs: vec![Rect::new(0., 0., 150., 32.), Rect::new(150., 0., 150., 32.)],
        }];
        let p2 = Dragged {
            tab: crate::layout::Tab::Page(P2),
            as_view: None,
        };
        let zone = zone_at(&l, &regions, &[], p2, 1150., 400.).unwrap();
        assert_eq!(
            zone.target,
            Target::Split {
                place: Place::Page,
                group: 0,
                edge: Edge::Right
            }
        );
        assert_eq!(zone.preview, Rect::new(600., 0., 600., 800.));
        // Near the bottom right corner, nearer the bottom edge.
        let zone = zone_at(&l, &regions, &[], p2, 1100., 790.).unwrap();
        assert!(matches!(
            zone.target,
            Target::Split {
                edge: Edge::Bottom,
                ..
            }
        ));
        // A drop on the computed zone lands exactly there.
        let mut after = l.clone();
        drop_into(&mut after, p2, zone.target, TabId(99)).unwrap();
        assert_eq!(after.page.direction, crate::layout::Direction::Column);
        assert_eq!(after.page.groups.len(), 2);
    }
}
