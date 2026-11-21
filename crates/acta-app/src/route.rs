//! Where the page area is (docs/design/layout.md 4.2): a typed reference to a
//! page, and the back / forward history of the window.
//!
//! A route's key is what `preferences.json` may hold (docs/design/layout.md
//! 4.7): a record id or a fixed page key with its project id, never a title.

use acta_core::model::{EpicId, KbId, ProjectId, TopicId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Route {
    Project(ProjectId),
    Epic(EpicId),
    Topic(TopicId),
    Inbox(ProjectId),
    Knowledge(ProjectId),
    /// A document or a memory in the reading view.
    Kb(KbId),
    Trash,
    Settings,
}

impl Route {
    /// The stable key: `P-1`, `E-3`, `T-42`, `K-7`, `inbox:P-1`,
    /// `knowledge:P-1`, `trash`, `settings`.
    pub fn key(self) -> String {
        match self {
            Route::Project(id) => id.to_string(),
            Route::Epic(id) => id.to_string(),
            Route::Topic(id) => id.to_string(),
            Route::Kb(id) => id.to_string(),
            Route::Inbox(id) => format!("inbox:{id}"),
            Route::Knowledge(id) => format!("knowledge:{id}"),
            Route::Trash => "trash".into(),
            Route::Settings => "settings".into(),
        }
    }

    pub fn parse(key: &str) -> Option<Route> {
        if let Some(rest) = key.strip_prefix("inbox:") {
            return project_key(rest).map(Route::Inbox);
        }
        if let Some(rest) = key.strip_prefix("knowledge:") {
            return project_key(rest).map(Route::Knowledge);
        }
        match key {
            "trash" => return Some(Route::Trash),
            "settings" => return Some(Route::Settings),
            _ => {}
        }
        let (prefix, _) = key.split_once('-')?;
        match prefix {
            "P" => ProjectId::parse(key).map(Route::Project),
            "E" => EpicId::parse(key).map(Route::Epic),
            "T" => TopicId::parse(key).map(Route::Topic),
            "K" => KbId::parse(key).map(Route::Kb),
            _ => None,
        }
    }
}

/// Short ids parse leniently ("42" is P-42); a stored key must carry its
/// prefix so that a page of one kind never opens as another.
fn project_key(text: &str) -> Option<ProjectId> {
    text.strip_prefix("P-").and_then(|_| ProjectId::parse(text))
}

/// The history of the window's single page (docs/design/layout.md 4.4: with
/// one tab, navigating replaces the page, and back and forward walk it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    back: Vec<Route>,
    current: Option<Route>,
    forward: Vec<Route>,
}

/// Enough to go back a long way without growing without bound.
const MAX_HISTORY: usize = 100;

impl History {
    pub fn current(&self) -> Option<Route> {
        self.current
    }

    /// Open `route`: the current page goes on the back stack, forward is
    /// cleared. Opening the page already shown changes nothing.
    pub fn open(&mut self, route: Route) {
        if self.current == Some(route) {
            return;
        }
        if let Some(current) = self.current.replace(route) {
            self.back.push(current);
            if self.back.len() > MAX_HISTORY {
                self.back.remove(0);
            }
        }
        self.forward.clear();
    }

    /// Replace the current page without a history step (a page that no
    /// longer exists, or the first page of a run).
    pub fn replace(&mut self, route: Route) {
        self.current = Some(route);
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn back(&mut self) -> Option<Route> {
        let previous = self.back.pop()?;
        if let Some(current) = self.current.replace(previous) {
            self.forward.push(current);
        }
        Some(previous)
    }

    pub fn forward(&mut self) -> Option<Route> {
        let next = self.forward.pop()?;
        if let Some(current) = self.current.replace(next) {
            self.back.push(current);
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_and_carry_no_title() {
        let routes = [
            Route::Project(ProjectId(1)),
            Route::Epic(EpicId(3)),
            Route::Topic(TopicId(42)),
            Route::Kb(KbId(7)),
            Route::Inbox(ProjectId(2)),
            Route::Knowledge(ProjectId(2)),
            Route::Trash,
            Route::Settings,
        ];
        for route in routes {
            assert_eq!(Route::parse(&route.key()), Some(route), "{route:?}");
        }
        assert_eq!(Route::Topic(TopicId(42)).key(), "T-42");
        assert_eq!(Route::Inbox(ProjectId(2)).key(), "inbox:P-2");
    }

    #[test]
    fn malformed_keys_are_refused() {
        for key in [
            "",
            "42",
            "X-1",
            "T-",
            "T-0",
            "inbox:",
            "inbox:2",
            "inbox:T-2",
            "trash2",
        ] {
            assert_eq!(Route::parse(key), None, "{key}");
        }
    }

    #[test]
    fn history_walks_back_and_forward_and_opening_clears_forward() {
        let a = Route::Topic(TopicId(1));
        let b = Route::Topic(TopicId(2));
        let c = Route::Epic(EpicId(1));
        let mut h = History::default();
        h.open(a);
        assert!(!h.can_go_back());
        h.open(b);
        h.open(b);
        assert_eq!(h.back(), Some(a));
        assert_eq!(h.current(), Some(a));
        assert!(h.can_go_forward());
        assert_eq!(h.forward(), Some(b));
        assert_eq!(h.back(), Some(a));
        h.open(c);
        assert!(!h.can_go_forward());
        assert_eq!(h.back(), Some(a));
        assert_eq!(h.back(), None);
    }

    #[test]
    fn history_is_bounded() {
        let mut h = History::default();
        for n in 1..=(MAX_HISTORY as i64 + 10) {
            h.open(Route::Topic(TopicId(n)));
        }
        let mut steps = 0;
        while h.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, MAX_HISTORY);
    }
}
