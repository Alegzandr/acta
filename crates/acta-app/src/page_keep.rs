//! A page's scroll and focused element across launches, the drawing side
//! of interaction.md 6.6 for the pages that are not record pages (the
//! inbox, knowledge, questions, contacts, contact and document pages).
//! The record page keeps its own, with its blocks.

use gpui_kit::{Context, ScrollHandle, Task, point};

use crate::resume::{self, PageState};
use crate::settings;
use crate::zoom;

/// What one page keeps: its route key, the scroll it tracks, and the
/// state of the last launch until it is put back.
pub struct Keep {
    route: String,
    scroll: ScrollHandle,
    restore: Option<PageState>,
    write: Task<()>,
}

impl Keep {
    /// For the page at `route` (its route key); the kept state is read now.
    pub fn new(route: String, cx: &gpui_kit::App) -> Self {
        let restore = settings::current(cx).pages.get(&route).cloned();
        Self {
            route,
            scroll: ScrollHandle::new(),
            restore,
            write: Task::ready(()),
        }
    }

    /// The same, on a scroll the page already tracks.
    pub fn with_scroll(route: String, scroll: ScrollHandle, cx: &gpui_kit::App) -> Self {
        Self {
            scroll,
            ..Self::new(route, cx)
        }
    }

    pub fn route(&self) -> &str {
        &self.route
    }

    /// The handle the page's scroll tracks.
    pub fn scroll(&self) -> &ScrollHandle {
        &self.scroll
    }

    /// The page scrolled: the position is kept 500 ms after it stops.
    pub fn scrolled<T: 'static>(&mut self, cx: &mut Context<T>) {
        let scroll = self.scroll.clone();
        let route = self.route.clone();
        let timer = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(500));
        self.write = cx.spawn(async move |_, cx| {
            timer.await;
            let top = zoom::eff(-scroll.offset().y);
            cx.update(|cx| {
                let now = top.max(0.).round() as u32;
                if settings::current(cx).pages.get(&route).map(|s| s.scroll) != Some(now) {
                    settings::change(cx, |p| {
                        resume::set_scroll(&mut p.pages, &route, top);
                    });
                }
            });
        });
    }

    /// Once the page has its content: the kept state, taken (put back
    /// once). The caller scrolls with [`Keep::apply_scroll`] on the next
    /// frame and gives the focus back.
    pub fn take(&mut self) -> Option<PageState> {
        self.restore.take()
    }

    /// Scroll to the kept position (GPUI clamps it to the content).
    pub fn apply_scroll(&self, kept: &PageState) {
        if kept.scroll > 0 {
            let x = self.scroll.offset().x;
            self.scroll
                .set_offset(point(x, -zoom::px(kept.scroll as f32)));
        }
    }
}
