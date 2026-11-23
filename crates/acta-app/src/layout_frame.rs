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
    /// The right sidebar is shown in the layout but its active view and
    /// the page's 420 do not fit side by side (step 3 of 4.8.10, only in a
    /// zoomed window): its toggle shows it over the page.
    pub right_auto_hidden: bool,
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
    let right_wanted = layout
        .right
        .shown
        .then(|| sidebar_width(layout, Side::Right, width));
    let right_fits = side_min(layout, Side::Right) + PAGE_MIN_W <= width;
    let right = right_wanted.filter(|_| right_fits);
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
        right_auto_hidden: right_wanted.is_some() && !right_fits,
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
    ctx: DropContext,
    x: f32,
    y: f32,
) -> Option<Zone> {
    let is_view = matches!(dragged.tab, Tab::View(_)) || dragged.as_view.is_some();
    if is_view {
        for strip in strips {
            if strip.hit.contains(x, y) {
                return finish(layout, dragged, Target::Area(strip.side), None, ctx);
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
        return finish(layout, dragged, target, line, ctx);
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
    let target = match best {
        Some((_, edge)) => Target::Split { place, group, edge },
        None => into(None),
    };
    finish(layout, dragged, target, None, ctx)
}

/// What a drop needs besides the layout: the body it lays out in, and the
/// page tab that closes when an Inbox or Knowledge view lands in the page
/// area (its page is open once, 4.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropContext {
    pub body: Rect,
    pub close: Option<TabId>,
}

/// The zone of `target`, its preview the bounds the tab takes after the
/// drop (4.8.11, rule 6): the drop is made on a copy and the copy laid
/// out. No zone when the drop fails, changes nothing, or lands somewhere
/// not drawn (an auto-hidden sidebar, a folded group).
fn finish(
    layout: &Layout,
    dragged: Dragged,
    target: Target,
    line: Option<Rect>,
    ctx: DropContext,
) -> Option<Zone> {
    let (after, spot) = simulate(
        layout,
        dragged,
        target,
        ctx.close,
        TabId(u64::MAX),
        ctx.body.w,
    )?;
    let f = frame(&after, ctx.body.w, ctx.body.h);
    let preview = group_rect(&after, &f, ctx.body, spot.place, spot.group)?;
    Some(Zone {
        target,
        preview,
        line,
    })
}

/// The layout after dropping `dragged` at `target` in a window `width`
/// wide, and where it landed. A drop or a move never lands out of sight:
/// when the left sidebar it lands in would auto-hide, the right sidebar
/// narrows to make room ([`fit_left`]); with no room even so, `None`.
pub fn simulate(
    layout: &Layout,
    dragged: Dragged,
    target: Target,
    close: Option<TabId>,
    new_page: TabId,
    width: f32,
) -> Option<(Layout, crate::layout::Spot)> {
    let (mut after, spot) = place(layout, dragged, target, close, new_page)?;
    if spot.place == Place::Side(Side::Left) && !fit_left(&mut after, width) {
        return None;
    }
    Some((after, spot))
}

/// Whether the left sidebar, when shown, has room beside the page in a
/// window `width` wide, the right sidebar narrowed (down to its minimum)
/// to give it that room. Nothing changes when there is no room at all.
pub fn fit_left(l: &mut Layout, width: f32) -> bool {
    if !l.left.shown {
        return true;
    }
    let left = sidebar_width(l, Side::Left, width);
    let right = l.right.shown.then(|| sidebar_width(l, Side::Right, width));
    if left + PAGE_MIN_W + right.unwrap_or(0.) <= width {
        return true;
    }
    if right.is_none() {
        return false;
    }
    let room = (width - PAGE_MIN_W - left).floor();
    if room < side_min(l, Side::Right) {
        return false;
    }
    l.right.size = room;
    true
}

/// The layout after dropping `dragged` at `target`, room or not.
pub fn place(
    layout: &Layout,
    dragged: Dragged,
    target: Target,
    close: Option<TabId>,
    new_page: TabId,
) -> Option<(Layout, crate::layout::Spot)> {
    let mut copy = layout.clone();
    let into_page = match target {
        Target::Into { place, .. } | Target::Split { place, .. } => place == Place::Page,
        Target::Area(_) => false,
    };
    // Only an Inbox or Knowledge view landing in the page area closes the
    // page it becomes, where that page was already open.
    let close =
        close.filter(|_| into_page && matches!(dragged.tab, Tab::View(v) if v.is_page_kind()));
    if let Some(close) = close {
        copy.close_page(close).ok()?;
    }
    // The closed page may shift the target's group or index: the target is
    // named against the layout the user sees, so it is read again in the
    // layout without that page.
    let target = if close.is_some() {
        retarget(layout, &copy, target)?
    } else {
        target
    };
    let spot = drop_into(&mut copy, dragged, target, new_page).ok()?;
    Some((copy, spot))
}

/// `target` named in `before`, read in `after` (a page tab closed between).
fn retarget(before: &Layout, after: &Layout, target: Target) -> Option<Target> {
    let map_group = |place: Place, group: usize| -> Option<usize> {
        if place != Place::Page {
            return Some(group);
        }
        // The group whose tabs survive at the same position.
        let tabs = &before.page.groups.get(group)?.tabs;
        let survivor = tabs.iter().find_map(|t| after.locate(*t))?;
        Some(survivor.group)
    };
    Some(match target {
        Target::Into {
            place,
            group,
            index,
        } => {
            let new_group = map_group(place, group)?;
            let len = after.area(place).groups[new_group].tabs.len();
            Target::Into {
                place,
                group: new_group,
                index: index.map(|i| i.min(len)),
            }
        }
        Target::Split { place, group, edge } => Target::Split {
            place,
            group: map_group(place, group)?,
            edge,
        },
        Target::Area(side) => Target::Area(side),
    })
}

/// The minimum of each page group along the page area's direction.
pub fn page_mins(l: &Layout) -> Vec<f32> {
    let min = match l.page.direction {
        Direction::Row => GROUP_MIN_W,
        Direction::Column => PAGE_MIN_H,
    };
    vec![min; l.page.groups.len()]
}

/// The minimum of each group of a side area along its direction.
pub fn side_group_mins(l: &Layout, side: Side) -> Vec<f32> {
    l.side(side)
        .groups
        .iter()
        .map(|g| match g.active_tab() {
            Some(Tab::View(v)) => match side.direction() {
                Direction::Column => v.min().1,
                Direction::Row => v.min().0,
            },
            _ => 0.,
        })
        .collect()
}

/// The bounds of an area as `f` lays it out in `body` (handles aside).
/// `None` when the area is not drawn beside the page.
pub fn area_rect(f: &Frame, body: Rect, place: Place) -> Option<Rect> {
    let left = f.left.unwrap_or(0.);
    let page_x = body.x + left;
    Some(match place {
        Place::Side(Side::Left) => Rect::new(body.x, body.y, f.left?, body.h),
        Place::Side(Side::Right) => {
            let w = f.right?;
            Rect::new(body.right() - w, body.y, w, body.h)
        }
        Place::Side(Side::Bottom) => Rect::new(page_x, body.y + f.page_h, f.page_w, f.bottom?),
        Place::Page => Rect::new(page_x, body.y, f.page_w, f.page_h),
    })
}

/// The bounds of `group` of `place` as `f` lays it out; `None` when it is
/// not drawn (its area hidden or auto-hidden, or folded away).
pub fn group_rect(l: &Layout, f: &Frame, body: Rect, place: Place, group: usize) -> Option<Rect> {
    let rect = area_rect(f, body, place)?;
    let area = l.area(place);
    let n = area.groups.len();
    if group >= n {
        return (n == 0).then_some(rect);
    }
    let folded = match place {
        Place::Page => f.page_folded,
        Place::Side(side) => f.side_folded(side),
    };
    if folded || n == 1 {
        let shown = match place {
            Place::Page => l.focused.min(n - 1),
            Place::Side(_) => 0,
        };
        return (group == shown).then_some(rect);
    }
    let mins = match place {
        Place::Page => page_mins(l),
        Place::Side(side) => side_group_mins(l, side),
    };
    let total = match area.direction {
        Direction::Row => rect.w,
        Direction::Column => rect.h,
    };
    let sizes = group_sizes(total, &area.shares, &mins);
    let start: f32 = sizes[..group].iter().sum();
    Some(match area.direction {
        Direction::Row => Rect::new(rect.x + start, rect.y, sizes[group], rect.h),
        Direction::Column => Rect::new(rect.x, rect.y + start, rect.w, sizes[group]),
    })
}

/// Where the handle after group `at` sits along an area's axis, for a
/// handle drag; `None` when there is no group after `at`.
pub fn handle_position(
    start: f32,
    total: f32,
    shares: &[f32],
    mins: &[f32],
    at: usize,
) -> Option<f32> {
    if at + 1 >= shares.len() || shares.len() != mins.len() {
        return None;
    }
    let sizes = group_sizes(total, shares, mins);
    Some(start + sizes[..=at].iter().sum::<f32>())
}

/// Whether "Split right" or "Split down" may run (4.8.8): the rules of
/// the layout, and room for every group at its minimum once split.
pub fn split_fits(l: &Layout, f: &Frame, direction: Direction) -> bool {
    let n = l.page.groups.len() as f32 + 1.;
    let room = match direction {
        Direction::Row => f.page_w >= GROUP_MIN_W * n,
        Direction::Column => f.page_h >= PAGE_MIN_H * n,
    };
    room && l.can_split_active(direction)
}

/// What the shell draws over or instead of the layout, which decides what
/// is on screen with it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    /// "Maximize focused area": only that group is drawn.
    pub zoomed: Option<(Place, usize)>,
    /// The auto-hidden left sidebar shown over the page.
    pub left_overlay: bool,
    /// The auto-hidden right sidebar shown over the page.
    pub right_overlay: bool,
}

/// Whether `view` is drawn: visible in the layout, its area beside the
/// page (or over it), its group not folded away, no other group maximized.
pub fn on_screen(l: &Layout, f: &Frame, shown: Shown, view: ViewId) -> bool {
    let Some(spot) = l.locate(Tab::View(view)) else {
        return false;
    };
    if !l.is_visible(view) {
        return false;
    }
    if let Some(zoom) = shown.zoomed {
        return zoom == (spot.place, spot.group);
    }
    match spot.place {
        Place::Page => !f.page_folded || spot.group == l.focused,
        Place::Side(side) => {
            let drawn = match side {
                Side::Left => f.left.is_some() || (f.left_auto_hidden && shown.left_overlay),
                Side::Right => f.right.is_some() || (f.right_auto_hidden && shown.right_overlay),
                Side::Bottom => f.bottom.is_some(),
            };
            drawn && (!f.side_folded(side) || spot.group == 0)
        }
    }
}

/// Bring a placed and visible view on screen (the toggle rule, 4.8.5): a
/// maximized other group gives way, a folded page group takes the place of
/// the focused one, a folded second sidebar group moves its view into the
/// first, the auto-hidden left sidebar shows over the page.
pub fn reveal(l: &mut Layout, f: &Frame, shown: &mut Shown, view: ViewId) {
    let Some(spot) = l.locate(Tab::View(view)) else {
        return;
    };
    if shown.zoomed.is_some_and(|z| z != (spot.place, spot.group)) {
        shown.zoomed = None;
    }
    match spot.place {
        Place::Page => l.focused = spot.group,
        Place::Side(Side::Left) if f.left_auto_hidden => shown.left_overlay = true,
        Place::Side(Side::Right) if f.right_auto_hidden => shown.right_overlay = true,
        Place::Side(side) if f.side_folded(side) && spot.group > 0 => {
            let _ = l.move_tab(
                Tab::View(view),
                Target::Into {
                    place: Place::Side(side),
                    group: 0,
                    index: None,
                },
            );
        }
        _ => {}
    }
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
    /// The body of the drawn regions below: 1440 x 840 at the origin.
    const CTX: DropContext = DropContext {
        body: Rect {
            x: 0.,
            y: 0.,
            w: 1440.,
            h: 840.,
        },
        close: None,
    };

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
        let zone = zone_at(&l, &regions, &strips, chat, CTX, 500., 830.).unwrap();
        assert_eq!(zone.target, Target::Area(Side::Bottom));
        assert_eq!(zone.preview, Rect::new(260., 520., 532., 320.));
        // The page area's centre: a view tab in that group.
        let zone = zone_at(&l, &regions, &strips, chat, CTX, 520., 400.).unwrap();
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
        let zone = zone_at(&l, &regions, &strips, chat, CTX, 270., 400.).unwrap();
        assert!(matches!(zone.target, Target::Into { .. }));
        // Its own group's centre is the last tab's own place: no zone.
        let last = Dragged {
            tab: crate::layout::Tab::View(ViewId::AgentBrowser),
            as_view: None,
        };
        assert_eq!(zone_at(&l, &regions, &strips, last, CTX, 1100., 400.), None);
        // Before the terminal in the header: an insertion line.
        let zone = zone_at(&l, &regions, &strips, chat, CTX, 805., 10.).unwrap();
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
        assert_eq!(zone_at(&l, &regions, &strips, chat, CTX, 940., 10.), None);
        // The bottom of the right sidebar: a second group, stacked.
        let zone = zone_at(&l, &regions, &strips, chat, CTX, 1100., 800.).unwrap();
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
        let zone = zone_at(&l, &regions, &strips, last, CTX, 1430., 400.);
        assert_eq!(zone, None, "the centre of its own group");
        // Outside every region: nothing.
        assert_eq!(zone_at(&l, &regions, &strips, chat, CTX, 2000., 400.), None);
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
        let zone = zone_at(&l, &[], &[strip], chat, CTX, 400., 700.).unwrap();
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
        assert_eq!(zone_at(&l, &regions, &strips, page, CTX, 100., 400.), None);
        assert_eq!(zone_at(&l, &regions, &strips, page, CTX, 1100., 400.), None);
        // No strip for a page: the page group's bottom edge splits it.
        let zone = zone_at(&l, &regions, &strips, page, CTX, 500., 830.).unwrap();
        assert!(matches!(
            zone.target,
            Target::Split {
                place: Place::Page,
                edge: Edge::Bottom,
                ..
            }
        ));
        // After the second tab in the tab bar.
        let zone = zone_at(&l, &regions, &strips, page, CTX, 700., 10.).unwrap();
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
        let zone = zone_at(&l, &regions, &strips, inbox, CTX, 100., 10.).unwrap();
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
        const CTX: DropContext = DropContext {
            body: Rect {
                x: 0.,
                y: 0.,
                w: 1200.,
                h: 800.,
            },
            close: None,
        };
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
        let zone = zone_at(&l, &regions, &[], p2, CTX, 1150., 400.).unwrap();
        assert_eq!(
            zone.target,
            Target::Split {
                place: Place::Page,
                group: 0,
                edge: Edge::Right
            }
        );
        assert_eq!(zone.preview, Rect::new(730., 0., 470., 800.));
        // Near the bottom right corner, nearer the bottom edge.
        let zone = zone_at(&l, &regions, &[], p2, CTX, 1100., 790.).unwrap();
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

    /// The bounds `spot` has after the drop, as the frame lays them out.
    fn landed(l: &Layout, body: Rect, tab: crate::layout::Tab) -> Rect {
        let spot = l.locate(tab).unwrap();
        let f = frame(l, body.w, body.h);
        group_rect(l, &f, body, spot.place, spot.group).unwrap()
    }

    #[test]
    fn every_preview_is_the_bounds_after_the_drop() {
        use crate::layout::Tab;
        // Wide enough that the left sidebar keeps the 400 of a terminal or
        // chat beside the right one (narrower, it would auto-hide and the
        // drop be refused).
        let body = Rect::new(0., 0., 1700., 840.);
        let ctx = DropContext { body, close: None };
        let l = shown();
        let regions = regions(&l);
        let strips = strips(&l, body, Rect::new(260., 0., 532., 840.));
        let chat = Dragged {
            tab: Tab::View(ViewId::Chat),
            as_view: None,
        };
        let page = Dragged {
            tab: Tab::Page(P1),
            as_view: None,
        };
        let inbox_page = Dragged {
            tab: Tab::Page(P2),
            as_view: Some(ViewId::Inbox),
        };
        // A hidden area's strip, a header, a centre, an edge, a tab row, a
        // page that becomes a view.
        let cases = [
            (chat, 500., 830.),
            (chat, 100., 10.),
            (chat, 520., 400.),
            (chat, 1100., 800.),
            (chat, 805., 10.),
            (page, 700., 10.),
            (page, 500., 830.),
            (inbox_page, 100., 400.),
        ];
        let mut kinds = 0;
        for (dragged, x, y) in cases {
            let zone = zone_at(&l, &regions, &strips, dragged, ctx, x, y)
                .unwrap_or_else(|| panic!("a zone at {x}, {y}"));
            let mut after = l.clone();
            drop_into(&mut after, dragged, zone.target, TabId(99)).unwrap();
            let tab = match dragged.as_view {
                Some(view) => Tab::View(view),
                None => dragged.tab,
            };
            assert_eq!(zone.preview, landed(&after, body, tab), "{x}, {y}");
            kinds += 1;
        }
        assert_eq!(kinds, cases.len());
    }

    #[test]
    fn a_centre_preview_grows_with_the_minimum_of_what_lands() {
        use crate::layout::Tab;
        // The terminal (400 wide) dropped into the 260 left sidebar: the
        // preview is the 400 the sidebar takes, not the 260 drawn now.
        let l = shown();
        let regions = regions(&l);
        let terminal = Dragged {
            tab: Tab::View(ViewId::Terminal),
            as_view: None,
        };
        let mut l = l;
        l.set_area_shown(Side::Right, false);
        let zone = zone_at(&l, &regions, &[], terminal, CTX, 130., 400.).unwrap();
        assert_eq!(zone.preview.w, 400.);
        let mut after = l.clone();
        drop_into(&mut after, terminal, zone.target, TabId(99)).unwrap();
        assert_eq!(zone.preview, landed(&after, CTX.body, terminal.tab));
    }

    #[test]
    fn a_view_that_becomes_an_open_page_closes_it_and_lands_where_previewed() {
        use crate::layout::Tab;
        // The Inbox view dropped into the page area where its page P2 is
        // open: P2 closes, the new page lands, exactly as previewed.
        let mut l = base();
        l.page_to_view(P2, ViewId::Inbox, Target::Area(Side::Left))
            .unwrap();
        l.add_page(TabId(7));
        let regions = vec![Region {
            place: Place::Page,
            group: 0,
            bounds: Rect::new(260., 0., 1180., 840.),
            header: Some(Rect::new(260., 0., 1180., 32.)),
            tabs: vec![
                Rect::new(260., 0., 150., 32.),
                Rect::new(410., 0., 150., 32.),
            ],
        }];
        let inbox = Dragged {
            tab: Tab::View(ViewId::Inbox),
            as_view: None,
        };
        let ctx = DropContext {
            body: CTX.body,
            close: Some(TabId(7)),
        };
        let zone = zone_at(&l, &regions, &[], inbox, ctx, 600., 10.).unwrap();
        let (after, spot) =
            simulate(&l, inbox, zone.target, ctx.close, TabId(9), CTX.body.w).unwrap();
        assert!(
            after.locate(Tab::Page(TabId(7))).is_none(),
            "the open page closed"
        );
        assert_eq!(after.locate(Tab::Page(TabId(9))), Some(spot));
        assert!(!after.is_placed(ViewId::Inbox));
        let f = frame(&after, CTX.body.w, CTX.body.h);
        assert_eq!(
            Some(zone.preview),
            group_rect(&after, &f, CTX.body, spot.place, spot.group)
        );
    }

    #[test]
    fn a_drop_into_the_auto_hidden_sidebar_is_no_zone() {
        use crate::layout::Tab;
        // At 880 with the right sidebar shown the left one is auto-hidden:
        // a view dropped into it would not be seen, so there is no zone.
        let mut l = shown();
        l.hide_view(ViewId::Projects);
        l.set_area_shown(Side::Left, true);
        let body = Rect::new(0., 0., 880., 500.);
        let strip = Strip {
            side: Side::Left,
            hit: Rect::new(0., 0., 24., 500.),
            preview: Rect::new(0., 0., 260., 500.),
        };
        let chat = Dragged {
            tab: Tab::View(ViewId::Chat),
            as_view: None,
        };
        let ctx = DropContext { body, close: None };
        assert_eq!(zone_at(&l, &[], &[strip], chat, ctx, 10., 100.), None);
    }

    /// Every view dropped on the left sidebar lands there, on screen,
    /// exactly where previewed: its header, its centre, its edge, the
    /// Projects group, its strip when hidden, with the right sidebar shown
    /// (it narrows to make room) or hidden.
    #[test]
    fn the_left_sidebar_takes_drops_everywhere() {
        use crate::layout::Tab;
        let body = Rect::new(0., 0., 1440., 840.);
        let ctx = DropContext { body, close: None };
        let views = [
            Dragged {
                tab: Tab::View(ViewId::Chat),
                as_view: None,
            },
            Dragged {
                tab: Tab::View(ViewId::Terminal),
                as_view: None,
            },
            Dragged {
                tab: Tab::View(ViewId::AgentBrowser),
                as_view: None,
            },
            Dragged {
                tab: Tab::Page(P2),
                as_view: Some(ViewId::Inbox),
            },
            Dragged {
                tab: Tab::Page(P1),
                as_view: Some(ViewId::Knowledge),
            },
        ];
        for right_shown in [true, false] {
            for left_shown in [true, false] {
                let mut l = shown();
                l.set_area_shown(Side::Right, right_shown);
                l.set_area_shown(Side::Left, left_shown);
                let f = frame(&l, body.w, body.h);
                let page_x = f.left.unwrap_or(0.);
                let page = Rect::new(page_x, 0., f.page_w, 840.);
                let strips = strips(&l, body, page);
                let mut regions = regions(&l);
                if !left_shown {
                    regions.retain(|r| r.place != Place::Side(Side::Left));
                }
                let points: &[(f32, f32)] = if left_shown {
                    // Header, centre, near the bottom and top edges.
                    &[(100., 10.), (130., 400.), (130., 820.), (130., 40.)]
                } else {
                    &[(10., 400.), (2., 10.)]
                };
                for dragged in views {
                    for &(x, y) in points {
                        let case = format!(
                            "{:?} at {x}, {y}, left {left_shown}, right {right_shown}",
                            dragged.as_view.map(Tab::View).unwrap_or(dragged.tab)
                        );
                        let zone = zone_at(&l, &regions, &strips, dragged, ctx, x, y)
                            .unwrap_or_else(|| panic!("no zone: {case}"));
                        let (after, spot) =
                            simulate(&l, dragged, zone.target, None, TabId(99), body.w)
                                .unwrap_or_else(|| panic!("no drop: {case}"));
                        assert_eq!(spot.place, Place::Side(Side::Left), "{case}");
                        let tab = dragged.as_view.map(Tab::View).unwrap_or(dragged.tab);
                        assert_eq!(zone.preview, landed(&after, body, tab), "{case}");
                        let f = frame(&after, body.w, body.h);
                        assert!(!f.left_auto_hidden, "{case}");
                        let Tab::View(view) = tab else { unreachable!() };
                        assert!(on_screen(&after, &f, Shown::default(), view), "{case}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_left_drop_with_no_room_even_beside_a_narrowed_right_sidebar_is_no_zone() {
        use crate::layout::Tab;
        // 1100: 400 for the chat + 420 for the page leaves 280 for the
        // right sidebar, under its 400.
        let body = Rect::new(0., 0., 1100., 840.);
        let l = shown();
        let chat = Dragged {
            tab: Tab::View(ViewId::Chat),
            as_view: None,
        };
        let target = Target::Area(Side::Left);
        assert!(place(&l, chat, target, None, TabId(99)).is_some());
        assert!(simulate(&l, chat, target, None, TabId(99), body.w).is_none());
        // Without the right sidebar it fits.
        let mut hidden = l.clone();
        hidden.set_area_shown(Side::Right, false);
        assert!(simulate(&hidden, chat, target, None, TabId(99), body.w).is_some());
    }

    #[test]
    fn fit_left_narrows_the_right_sidebar_to_its_minimum_at_most() {
        let mut l = shown();
        l.move_tab(
            crate::layout::Tab::View(ViewId::Chat),
            Target::Area(Side::Left),
        )
        .unwrap();
        assert!(frame(&l, 1440., 840.).left_auto_hidden);
        let mut fitted = l.clone();
        assert!(fit_left(&mut fitted, 1440.));
        assert_eq!(fitted.right.size, 620.);
        let f = frame(&fitted, 1440., 840.);
        assert_eq!((f.left, f.right, f.page_w), (Some(400.), Some(620.), 420.));
        // No room: nothing changes.
        let mut narrow = l.clone();
        assert!(!fit_left(&mut narrow, 1100.));
        assert_eq!(narrow, l);
    }

    #[test]
    fn a_handle_after_the_last_group_is_none() {
        assert_eq!(
            handle_position(0., 1000., &[0.5, 0.5], &[400., 400.], 0),
            Some(500.)
        );
        assert_eq!(
            handle_position(0., 1000., &[0.5, 0.5], &[400., 400.], 1),
            None
        );
        assert_eq!(handle_position(0., 1000., &[1.], &[400.], 0), None);
        assert_eq!(handle_position(0., 1000., &[0.5, 0.5], &[400.], 0), None);
    }

    #[test]
    fn split_needs_room_for_every_group() {
        let l = base();
        assert!(split_fits(&l, &frame(&l, 1440., 840.), Direction::Row));
        // 1100 - 260 = 840 < 2 x 420? 840 >= 800: fits; 1000 - 260 does not.
        assert!(split_fits(&l, &frame(&l, 1100., 840.), Direction::Row));
        assert!(!split_fits(&l, &frame(&l, 1000., 840.), Direction::Row));
        assert!(split_fits(&l, &frame(&l, 1440., 500.), Direction::Column));
        assert!(!split_fits(&l, &frame(&l, 1440., 380.), Direction::Column));
    }

    #[test]
    fn toggles_bring_folded_and_auto_hidden_views_on_screen() {
        use crate::layout::Tab;
        // A view in the second, folded group of the right sidebar.
        let mut l = shown();
        l.split_off(ViewId::Chat).unwrap();
        let f = frame(&l, 1440., 380.);
        assert!(f.side_folded(Side::Right));
        let mut shown_ = Shown::default();
        assert!(l.is_visible(ViewId::Chat));
        assert!(!on_screen(&l, &f, shown_, ViewId::Chat), "folded away");
        reveal(&mut l, &f, &mut shown_, ViewId::Chat);
        let f = frame(&l, 1440., 380.);
        assert!(on_screen(&l, &f, shown_, ViewId::Chat));
        assert!(l.locate(Tab::View(ViewId::Chat)).unwrap().group == 0);

        // Projects in the auto-hidden left sidebar at 880.
        let mut l = shown();
        let f = frame(&l, 880., 500.);
        let mut shown_ = Shown::default();
        assert!(!on_screen(&l, &f, shown_, ViewId::Projects));
        reveal(&mut l, &f, &mut shown_, ViewId::Projects);
        assert!(shown_.left_overlay);
        assert!(on_screen(&l, &f, shown_, ViewId::Projects));

        // A view tab in a folded page group: its group takes the focus.
        let mut l = base();
        l.split_active(Direction::Row).unwrap();
        l.move_tab(
            Tab::View(ViewId::Terminal),
            Target::Into {
                place: Place::Page,
                group: 1,
                index: None,
            },
        )
        .unwrap();
        l.focused = 0;
        let f = frame(&l, 1000., 840.);
        assert!(f.page_folded);
        let mut shown_ = Shown::default();
        assert!(!on_screen(&l, &f, shown_, ViewId::Terminal));
        reveal(&mut l, &f, &mut shown_, ViewId::Terminal);
        assert_eq!(l.focused, 1);
        assert!(on_screen(&l, &f, shown_, ViewId::Terminal));

        // A maximized other group gives way.
        let l = shown();
        let f = frame(&l, 1440., 840.);
        let mut zoomed = Shown {
            zoomed: Some((Place::Page, 0)),
            left_overlay: false,
            right_overlay: false,
        };
        assert!(!on_screen(&l, &f, zoomed, ViewId::Terminal));
        let mut l2 = l.clone();
        reveal(&mut l2, &f, &mut zoomed, ViewId::Terminal);
        assert_eq!(zoomed.zoomed, None);
        assert!(on_screen(&l2, &f, zoomed, ViewId::Terminal));
    }
}
