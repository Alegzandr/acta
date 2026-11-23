//! Tools belong to the page they were started from (docs/design/layout.md
//! 4.5.1): which record owns a page's Claude Code sessions, chats and agent
//! browsers, how instances are numbered and capped, when an off-screen web
//! view is closed, and when a session offers "Restart in {folder}". No GPUI.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use acta_core::model::Owner;
use acta_core::paths::Os;

use crate::route::Route;

/// Claude Code sessions on one page.
pub const CLAUDE_MAX: usize = 4;
/// Chats on one page.
pub const CHAT_MAX: usize = 3;
/// Agent browsers on one page.
pub const BROWSER_MAX: usize = 3;
/// Claude Code (no project) sessions.
pub const FREE_MAX: usize = 4;
/// A web view off screen and unused this long is closed, its address kept.
pub const WEB_IDLE: Duration = Duration::from_secs(10 * 60);
/// Web views (chats and browsers) open at once.
pub const WEB_OPEN_MAX: usize = 6;

/// The record that owns the tools of the page `route` shows: a project,
/// an epic or a topic, and the pages that stand for one (the inbox and the
/// knowledge base for their project, the questions page for its scope, a
/// document or a memory for its owner, given as `kb_owner`). Settings,
/// Trash, Keyboard shortcuts and the contact pages own nothing.
pub fn owner_of(route: Route, kb_owner: Option<Owner>) -> Option<Owner> {
    match route {
        Route::Project(id) | Route::Inbox(id) | Route::Knowledge(id) => Some(Owner::Project(id)),
        Route::Epic(id) => Some(Owner::Epic(id)),
        Route::Topic(id) => Some(Owner::Topic(id)),
        Route::Questions(owner) => Some(owner),
        Route::Kb(_) => kb_owner,
        Route::Trash | Route::Settings | Route::Shortcuts | Route::Contacts | Route::Contact(_) => {
            None
        }
    }
}

/// The number a new instance gets: the lowest free one from 1, `None` at
/// `max`.
pub fn next_number(used: &[u8], max: usize) -> Option<u8> {
    if used.len() >= max {
        return None;
    }
    (1..=u8::MAX).find(|n| !used.contains(n))
}

/// Who holds an instance: a page's record, or nobody (Claude Code (no
/// project)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Holder {
    Page(Owner),
    Free,
}

/// One instance: its holder and its number there ("Claude 2").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub holder: Holder,
    pub n: u8,
}

/// The instances of one tool, by holder, numbered in the order they were
/// opened (the lowest free number reused), one of each holder's selected.
pub struct Instances<T> {
    items: Vec<(InstanceKey, T)>,
    selected: HashMap<Holder, u8>,
    max: usize,
}

impl<T> Instances<T> {
    pub fn new(max: usize) -> Self {
        Instances {
            items: Vec::new(),
            selected: HashMap::new(),
            max,
        }
    }

    pub fn max(&self) -> usize {
        self.max
    }

    /// The numbers `holder` has, in order.
    pub fn numbers(&self, holder: Holder) -> Vec<u8> {
        let mut out: Vec<u8> = self
            .items
            .iter()
            .filter(|(k, _)| k.holder == holder)
            .map(|(k, _)| k.n)
            .collect();
        out.sort_unstable();
        out
    }

    /// The keys of `holder`'s instances, in number order.
    pub fn keys_of(&self, holder: Holder) -> Vec<InstanceKey> {
        self.numbers(holder)
            .into_iter()
            .map(|n| InstanceKey { holder, n })
            .collect()
    }

    pub fn at_limit(&self, holder: Holder) -> bool {
        self.numbers(holder).len() >= self.max
    }

    pub fn get(&self, key: InstanceKey) -> Option<&T> {
        self.items.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: InstanceKey) -> Option<&mut T> {
        self.items
            .iter_mut()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    pub fn contains(&self, key: InstanceKey) -> bool {
        self.get(key).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (InstanceKey, &T)> {
        self.items.iter().map(|(k, v)| (*k, v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (InstanceKey, &mut T)> {
        self.items.iter_mut().map(|(k, v)| (*k, v))
    }

    /// Adds `value` under the lowest free number of `holder` and selects
    /// it; gives it back at the limit.
    pub fn open(&mut self, holder: Holder, value: T) -> Result<InstanceKey, T> {
        let Some(n) = next_number(&self.numbers(holder), self.max) else {
            return Err(value);
        };
        let key = InstanceKey { holder, n };
        self.items.push((key, value));
        self.selected.insert(holder, n);
        Ok(key)
    }

    /// Adds `value` under number `n` (a stored instance coming back);
    /// gives it back when `n` is taken or `holder` is at its limit.
    pub fn open_at(&mut self, holder: Holder, n: u8, value: T) -> Result<InstanceKey, T> {
        let numbers = self.numbers(holder);
        if n == 0 || numbers.contains(&n) || numbers.len() >= self.max {
            return Err(value);
        }
        let key = InstanceKey { holder, n };
        self.items.push((key, value));
        self.selected.entry(holder).or_insert(n);
        Ok(key)
    }

    /// Replaces the value of an existing instance (a restart keeps its
    /// number); `false` when there is none.
    pub fn replace(&mut self, key: InstanceKey, value: T) -> bool {
        match self.get_mut(key) {
            Some(slot) => {
                *slot = value;
                true
            }
            None => false,
        }
    }

    /// Removes an instance. When it was selected, the next one (else the
    /// one before) is.
    pub fn close(&mut self, key: InstanceKey) -> Option<T> {
        let i = self.items.iter().position(|(k, _)| *k == key)?;
        let (_, value) = self.items.remove(i);
        if self.selected.get(&key.holder) == Some(&key.n) {
            let rest = self.numbers(key.holder);
            match rest.iter().find(|n| **n > key.n).or_else(|| rest.last()) {
                Some(n) => {
                    self.selected.insert(key.holder, *n);
                }
                None => {
                    self.selected.remove(&key.holder);
                }
            }
        }
        Some(value)
    }

    /// The instance `holder` shows: the one selected last, else its first.
    pub fn selected(&self, holder: Holder) -> Option<InstanceKey> {
        let numbers = self.numbers(holder);
        let n = self
            .selected
            .get(&holder)
            .filter(|n| numbers.contains(n))
            .copied()
            .or_else(|| numbers.first().copied())?;
        Some(InstanceKey { holder, n })
    }

    pub fn select(&mut self, key: InstanceKey) -> bool {
        if !self.contains(key) {
            return false;
        }
        self.selected.insert(key.holder, key.n);
        true
    }

    /// `Ctrl+PageDown` / `Ctrl+PageUp`: the next or previous instance of
    /// `holder`, wrapping; selected and returned.
    pub fn step(&mut self, holder: Holder, forward: bool) -> Option<InstanceKey> {
        let numbers = self.numbers(holder);
        let current = self.selected(holder)?;
        let i = numbers.iter().position(|n| *n == current.n)?;
        let len = numbers.len();
        let j = if forward {
            (i + 1) % len
        } else {
            (i + len - 1) % len
        };
        let key = InstanceKey {
            holder,
            n: numbers[j],
        };
        self.select(key);
        Some(key)
    }
}

/// A web view as the resource rule sees it (layout.md 4.5.1, Resources).
#[derive(Clone, Copy, Debug)]
pub struct WebUse<K> {
    pub key: K,
    /// Its native view exists.
    pub open: bool,
    /// It is the instance in view, in a view on screen.
    pub on_screen: bool,
    /// When it was last on screen or used (a call, a load).
    pub last_used: Instant,
    /// A browser with a call in flight or a handoff waiting: never closed.
    pub busy: bool,
}

/// The web views to close now: every one off screen and unused for
/// [`WEB_IDLE`], then, while more than [`WEB_OPEN_MAX`] stay open, the one
/// off screen the longest. One on screen or busy is never closed.
pub fn web_to_close<K: Copy>(views: &[WebUse<K>], now: Instant) -> Vec<K> {
    let closable = |v: &&WebUse<K>| v.open && !v.on_screen && !v.busy;
    let mut close: Vec<usize> = Vec::new();
    for (i, v) in views.iter().enumerate() {
        if closable(&v) && now.saturating_duration_since(v.last_used) >= WEB_IDLE {
            close.push(i);
        }
    }
    let mut open = views.iter().filter(|v| v.open).count() - close.len();
    if open > WEB_OPEN_MAX {
        let mut rest: Vec<usize> = views
            .iter()
            .enumerate()
            .filter(|(i, v)| closable(v) && !close.contains(i))
            .map(|(i, _)| i)
            .collect();
        rest.sort_by_key(|i| views[*i].last_used);
        for i in rest {
            if open <= WEB_OPEN_MAX {
                break;
            }
            close.push(i);
            open -= 1;
        }
    }
    close.into_iter().map(|i| views[i].key).collect()
}

/// The one mismatch a session can have on its own page (layout.md 4.5.1,
/// Restart): the name of the page's active folder when it is not the
/// folder the session runs in, or when the commit settings of that folder
/// changed since it started. `None`: Restart restarts in the same folder.
pub fn restart_in(
    running_in: &str,
    page_active: Option<(&str, &str)>,
    mode_changed: bool,
) -> Option<String> {
    let (path, name) = page_active?;
    let moved = !acta_core::folder_path::same(Os::CURRENT, path, running_in);
    (moved || mode_changed).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use acta_core::model::{ContactId, EpicId, KbId, ProjectId, TopicId};

    fn topic(n: i64) -> Owner {
        Owner::Topic(TopicId(n))
    }

    #[test]
    fn each_page_kind_names_its_owner() {
        let p = ProjectId(1);
        let e = EpicId(3);
        let t = TopicId(42);
        assert_eq!(owner_of(Route::Project(p), None), Some(Owner::Project(p)));
        assert_eq!(owner_of(Route::Epic(e), None), Some(Owner::Epic(e)));
        assert_eq!(owner_of(Route::Topic(t), None), Some(Owner::Topic(t)));
        // Pages that stand for a scope.
        assert_eq!(owner_of(Route::Inbox(p), None), Some(Owner::Project(p)));
        assert_eq!(owner_of(Route::Knowledge(p), None), Some(Owner::Project(p)));
        assert_eq!(
            owner_of(Route::Questions(Owner::Epic(e)), None),
            Some(Owner::Epic(e))
        );
        // A document or a memory: its owner.
        assert_eq!(
            owner_of(Route::Kb(KbId(7)), Some(Owner::Topic(t))),
            Some(Owner::Topic(t))
        );
        // Pages that own nothing, whatever is passed.
        for route in [
            Route::Settings,
            Route::Trash,
            Route::Shortcuts,
            Route::Contacts,
            Route::Contact(ContactId(2)),
        ] {
            assert_eq!(owner_of(route, Some(Owner::Project(p))), None, "{route:?}");
        }
    }

    #[test]
    fn numbering_reuses_the_lowest_free_number_up_to_the_limit() {
        assert_eq!(next_number(&[], 4), Some(1));
        assert_eq!(next_number(&[1, 2], 4), Some(3));
        assert_eq!(next_number(&[1, 3], 4), Some(2));
        assert_eq!(next_number(&[2, 3, 4], 4), Some(1));
        assert_eq!(next_number(&[1, 2, 3, 4], 4), None);

        let page = Holder::Page(topic(42));
        let mut sessions = Instances::new(CLAUDE_MAX);
        for name in ["a", "b", "c", "d"] {
            sessions.open(page, name).unwrap();
        }
        assert!(sessions.at_limit(page));
        assert_eq!(sessions.open(page, "e"), Err("e"), "4 is the most");
        // Another page has its own numbers and its own limit.
        let other = Holder::Page(topic(51));
        assert_eq!(sessions.open(other, "x").unwrap().n, 1);
        sessions.close(InstanceKey { holder: page, n: 2 });
        let key = sessions.open(page, "f").unwrap();
        assert_eq!(key.n, 2, "the lowest free number");
        assert_eq!(sessions.get(key), Some(&"f"));
        // A stored instance comes back under its own number.
        let mut chats = Instances::new(CHAT_MAX);
        assert_eq!(chats.open_at(other, 3, "c3").unwrap().n, 3);
        assert_eq!(chats.open_at(other, 3, "again"), Err("again"));
        assert_eq!(chats.open(other, "c1").unwrap().n, 1);
    }

    #[test]
    fn navigation_only_changes_what_is_looked_up() {
        // The views ask for the current page's instances; nothing is
        // removed or moved when the page changes.
        let t42 = Holder::Page(topic(42));
        let t51 = Holder::Page(topic(51));
        let mut sessions = Instances::new(CLAUDE_MAX);
        sessions.open(t42, "claude 1").unwrap();
        sessions.open(t42, "claude 2").unwrap();
        assert_eq!(sessions.keys_of(t51), []);
        assert_eq!(sessions.selected(t51), None);
        assert_eq!(sessions.numbers(t42), [1, 2]);
        assert_eq!(sessions.iter().count(), 2);
        assert_eq!(sessions.selected(t42).map(|k| k.n), Some(2));
    }

    #[test]
    fn closing_and_stepping_keep_a_sensible_selection() {
        let page = Holder::Page(topic(1));
        let mut chats = Instances::new(CHAT_MAX);
        for v in [10, 20, 30] {
            chats.open(page, v).unwrap();
        }
        let key = |n| InstanceKey { holder: page, n };
        assert!(chats.select(key(2)));
        assert_eq!(chats.step(page, true), Some(key(3)));
        assert_eq!(chats.step(page, true), Some(key(1)), "wraps");
        assert_eq!(chats.step(page, false), Some(key(3)));
        chats.close(key(3));
        assert_eq!(chats.selected(page), Some(key(2)), "the one before");
        chats.select(key(1));
        chats.close(key(1));
        assert_eq!(chats.selected(page), Some(key(2)), "the next one");
        chats.close(key(2));
        assert_eq!(chats.selected(page), None);
        assert!(!chats.select(key(1)));
    }

    #[test]
    fn a_restart_keeps_the_number() {
        let page = Holder::Page(topic(1));
        let mut s = Instances::new(CLAUDE_MAX);
        let k = s.open(page, 1).unwrap();
        assert!(s.replace(k, 2));
        assert_eq!(s.get(k), Some(&2));
        assert!(!s.replace(InstanceKey { holder: page, n: 3 }, 9));
    }

    fn web(key: u8, open: bool, on_screen: bool, ago: u64, busy: bool, now: Instant) -> WebUse<u8> {
        WebUse {
            key,
            open,
            on_screen,
            last_used: now - Duration::from_secs(ago),
            busy,
        }
    }

    #[test]
    fn an_off_screen_web_view_closes_after_ten_idle_minutes() {
        let now = Instant::now() + Duration::from_secs(3600);
        let views = [
            web(1, true, false, 601, false, now),
            web(2, true, false, 599, false, now),
            web(3, true, true, 3000, false, now),
            web(4, true, false, 3000, true, now),
            web(5, false, false, 3000, false, now),
        ];
        assert_eq!(web_to_close(&views, now), [1]);
    }

    #[test]
    fn beyond_six_open_the_one_off_screen_longest_closes_first() {
        let now = Instant::now() + Duration::from_secs(3600);
        let mut views: Vec<WebUse<u8>> = (1..=6)
            .map(|k| web(k, true, false, 60 * u64::from(k), false, now))
            .collect();
        assert!(web_to_close(&views, now).is_empty(), "six may stay open");
        // A seventh opens on screen: the oldest off-screen one closes.
        views.push(web(7, true, true, 0, false, now));
        assert_eq!(web_to_close(&views, now), [6]);
        // A busy browser is skipped.
        views[5].busy = true;
        assert_eq!(web_to_close(&views, now), [5]);
    }

    #[test]
    fn restart_in_is_offered_only_for_a_folder_change() {
        let here = if cfg!(windows) {
            "C:\\dev\\acta"
        } else {
            "/dev/acta"
        };
        let docs = if cfg!(windows) {
            "C:\\dev\\acta-docs"
        } else {
            "/dev/acta-docs"
        };
        assert_eq!(restart_in(here, Some((here, "acta")), false), None);
        assert_eq!(
            restart_in(here, Some((docs, "acta-docs")), false).as_deref(),
            Some("acta-docs")
        );
        assert_eq!(
            restart_in(here, Some((here, "acta")), true).as_deref(),
            Some("acta"),
            "a commit-mode change restarts in the same folder"
        );
        assert_eq!(restart_in(here, None, true), None);
    }
}
