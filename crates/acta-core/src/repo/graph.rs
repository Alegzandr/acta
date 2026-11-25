//! The commit graph's lanes (5.R.6, 5.R.10 Lanes), built row by row from
//! commits in topological order, so each page continues the last: the
//! builder's state is the lanes still waiting for a commit.
//!
//! For each commit: its lane is the first that waits for it, else the
//! first free slot, else a new one; every other lane waiting for it ends at
//! its node and is freed. Its first parent takes over its lane, unless
//! another lane already waits for that parent (the line then bends out to
//! that lane and this one is freed). Each other parent takes the lane
//! already waiting for it, else the first free slot right of the node, else
//! a new lane. Free slots are reused; lanes never move.

/// A line of one row. Lanes are counted from 0 at the left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// A lane passing the row from its top edge to its bottom edge.
    Through(u16),
    /// From the top edge at this lane to the node: straight when it is the
    /// node's lane, else a bend (a branch's first commit, a merge parent).
    In(u16),
    /// From the node to the bottom edge at this lane: straight when it is
    /// the node's lane, else a bend (a merge's other parent, a first
    /// parent another lane already waits for).
    Out(u16),
}

/// A line and whether it is on HEAD's first-parent chain (accent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub line: Line,
    pub accent: bool,
}

/// What a row draws.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// The node's lane.
    pub lane: u16,
    pub lines: Vec<Segment>,
    /// The node is HEAD or a first parent down from it.
    pub head_chain: bool,
    /// The node is HEAD's commit.
    pub head: bool,
    /// A merge (ring node).
    pub merge: bool,
    /// A shallow clone's boundary: its line ends below the node in a stub.
    pub stub: bool,
}

impl Row {
    /// Lanes this row needs (the graph column's width).
    pub fn width(&self) -> u16 {
        self.lines
            .iter()
            .map(|s| match s.line {
                Line::Through(l) | Line::In(l) | Line::Out(l) => l,
            })
            .chain([self.lane])
            .max()
            .unwrap_or(0)
            + 1
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Waiting {
    oid: String,
    accent: bool,
}

/// The streaming builder. Feed every page in order; after a refs change
/// the pages are read again from the top with a new builder.
#[derive(Clone, Debug, Default)]
pub struct LaneBuilder {
    lanes: Vec<Option<Waiting>>,
    /// The next commit of HEAD's first-parent chain.
    chain: Option<String>,
    head: Option<String>,
    shallow: Vec<String>,
}

impl LaneBuilder {
    /// `head`: HEAD's commit, for the accent chain. `shallow`: the
    /// boundary commits of a shallow clone ([`super::Repo::shallow_boundary`]).
    pub fn new(head: Option<String>, shallow: Vec<String>) -> LaneBuilder {
        LaneBuilder {
            lanes: Vec::new(),
            chain: head.clone(),
            head,
            shallow,
        }
    }

    /// Lanes currently open (waiting for a commit not yet seen).
    pub fn open_lanes(&self) -> usize {
        self.lanes.iter().filter(|l| l.is_some()).count()
    }

    /// The row of the next commit.
    pub fn push(&mut self, id: &str, parents: &[String]) -> Row {
        let waiting: Vec<usize> = (0..self.lanes.len())
            .filter(|&i| self.lanes[i].as_ref().is_some_and(|w| w.oid == id))
            .collect();
        let lane = match waiting.first() {
            Some(&i) => i,
            None => self.free_slot(0),
        };
        let on_chain = self.chain.as_deref() == Some(id);
        let mut row = Row {
            lane: lane as u16,
            head_chain: on_chain,
            head: self.head.as_deref() == Some(id),
            merge: parents.len() > 1,
            stub: self.shallow.iter().any(|s| s == id),
            lines: Vec::new(),
        };
        for &i in &waiting {
            let accent = self.lanes[i].take().is_some_and(|w| w.accent);
            row.lines.push(Segment {
                line: Line::In(i as u16),
                accent,
            });
        }
        for (i, w) in self.lanes.iter().enumerate() {
            if let Some(w) = w {
                row.lines.push(Segment {
                    line: Line::Through(i as u16),
                    accent: w.accent,
                });
            }
        }
        if on_chain {
            self.chain = parents.first().cloned();
        }
        for (n, p) in parents.iter().enumerate() {
            let accent = on_chain && n == 0;
            let target = match self.waiting_for(p) {
                Some(k) => {
                    if accent && let Some(w) = self.lanes[k].as_mut() {
                        w.accent = true;
                    }
                    k
                }
                None => {
                    let k = if n == 0 {
                        lane
                    } else {
                        self.free_slot(lane + 1)
                    };
                    self.lanes[k] = Some(Waiting {
                        oid: p.clone(),
                        accent,
                    });
                    k
                }
            };
            row.lines.push(Segment {
                line: Line::Out(target as u16),
                accent,
            });
        }
        while self.lanes.last().is_some_and(Option::is_none) {
            self.lanes.pop();
        }
        row
    }

    fn waiting_for(&self, oid: &str) -> Option<usize> {
        self.lanes
            .iter()
            .position(|l| l.as_ref().is_some_and(|w| w.oid == oid))
    }

    /// The first free slot at or right of `from`, made if needed.
    fn free_slot(&mut self, from: usize) -> usize {
        if let Some(i) = (from..self.lanes.len()).find(|&i| self.lanes[i].is_none()) {
            return i;
        }
        while self.lanes.len() < from {
            self.lanes.push(None);
        }
        self.lanes.push(None);
        self.lanes.len() - 1
    }
}
