//! What a project, epic or topic page shows, read from the store in one go
//! on a background thread (docs/design/layout.md 4.2: title, properties,
//! then the sections; data.md 11 for where each value comes from).

use std::collections::HashSet;

use acta_core::model::{
    Author, Entry, EntryKind, EpicId, Folder, Owner, PageReq, ProjectId, Topic, TopicSummary,
};
use acta_core::store::{EntryFilter, KbFilter};
use acta_core::{Result, Store};

use crate::kb_model::{self, Section};

/// How many entries the timeline loads at first; "older" pages follow.
pub const TIMELINE_PAGE: i64 = 100;

/// The timeline filter (docs/design/components.md 5, segmented control).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EntryFilterKind {
    #[default]
    All,
    Progress,
    Decisions,
    Discussion,
}

impl EntryFilterKind {
    pub const ALL: [EntryFilterKind; 4] = [
        EntryFilterKind::All,
        EntryFilterKind::Progress,
        EntryFilterKind::Decisions,
        EntryFilterKind::Discussion,
    ];

    fn filter(self) -> EntryFilter {
        let kinds = match self {
            // System events (status changes, creation, moves) show with All.
            EntryFilterKind::All => {
                return EntryFilter {
                    include_system: true,
                    ..EntryFilter::default()
                };
            }
            EntryFilterKind::Progress => vec![EntryKind::Progress],
            EntryFilterKind::Decisions => vec![EntryKind::Decision],
            EntryFilterKind::Discussion => vec![EntryKind::Discussion],
        };
        EntryFilter {
            kinds,
            ..EntryFilter::default()
        }
    }
}

/// One folder a launch from a page uses, and the level it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveFolder {
    pub path: String,
    pub from: Owner,
}

/// The working folders of a page (docs/design/components-folders.md 5;
/// ADR 0011: always inherited, one active folder).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folders {
    /// The page's own list, in order.
    pub own: Vec<Folder>,
    /// Every folder the page sees, the active one first.
    pub effective: Vec<EffectiveFolder>,
    /// Where the active folder comes from when it is not the page's own.
    pub inherited_from: Option<(Owner, String)>,
    /// The folders the page inherits (its epic's, then its project's), in
    /// list order.
    pub above: Vec<EffectiveFolder>,
    /// The level directly above.
    pub above_from: Option<(Owner, String)>,
    /// Paths (as stored) that are not a folder on this computer.
    pub missing: HashSet<String>,
}

impl Folders {
    pub fn primary(&self) -> Option<&EffectiveFolder> {
        self.effective.first()
    }

    pub fn is_missing(&self, path: &str) -> bool {
        self.missing.contains(path)
    }

    /// Every path the page shows, for the existence check.
    pub fn paths(&self) -> Vec<String> {
        self.effective
            .iter()
            .chain(self.above.iter())
            .map(|f| f.path.clone())
            .chain(self.own.iter().map(|f| f.path.clone()))
            .collect()
    }

    /// Missing folders among the additional ones (not the primary).
    pub fn missing_additional(&self) -> usize {
        self.effective
            .iter()
            .skip(1)
            .filter(|f| self.is_missing(&f.path))
            .count()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub owner: Owner,
    pub title: String,
    pub description: String,
    pub updated_at: i64,
    pub updated_by: Author,
    pub project: (ProjectId, String),
    pub epic: Option<(EpicId, String)>,
    /// Topic pages: the topic itself (status, current state).
    pub topic: Option<Topic>,
    pub entries: Vec<Entry>,
    pub entries_total: i64,
    /// Epic pages: the epic's topics in the tree's order.
    pub topics: Vec<TopicSummary>,
    /// Project pages: topics outside any epic, then each epic's.
    pub loose: Vec<TopicSummary>,
    pub groups: Vec<(EpicId, String, Vec<TopicSummary>)>,
    pub kb: Section,
    /// Epic and project pages: entries in the levels below.
    pub kb_below: i64,
    pub folders: Folders,
}

impl Loaded {
    /// Done over not Dropped, over the page's topics.
    pub fn progress(&self) -> (i64, i64) {
        let mut all: Vec<TopicSummary> = self.topics.clone();
        all.extend(self.loose.iter().cloned());
        for (_, _, topics) in &self.groups {
            all.extend(topics.iter().cloned());
        }
        crate::table::progress(&all)
    }
}

fn owner_name(store: &Store, owner: Owner) -> Result<String> {
    Ok(match owner {
        Owner::Project(id) => store.project(id)?.title,
        Owner::Epic(id) => store.epic(id)?.title,
        Owner::Topic(id) => store.topic(id)?.title,
    })
}

fn parent_of(store: &Store, owner: Owner) -> Result<Option<Owner>> {
    Ok(match owner {
        Owner::Project(_) => None,
        Owner::Epic(id) => Some(Owner::Project(store.epic(id)?.project)),
        Owner::Topic(id) => {
            let t = store.topic(id)?;
            Some(t.epic.map_or(Owner::Project(t.project), Owner::Epic))
        }
    })
}

/// The folders of `owner`, as stored. `missing` is left empty: the caller
/// checks the disk with [`Folders::paths`] once the store is released.
pub fn load_folders(store: &Store, owner: Owner) -> Result<Folders> {
    let own = store.folders(owner)?;
    let visible = store.visible_folders(owner)?;
    let as_effective = |f: &acta_core::store::VisibleFolder| EffectiveFolder {
        path: f.folder.path.clone(),
        from: f.origin,
    };
    let effective: Vec<EffectiveFolder> = visible
        .active()
        .into_iter()
        .chain(visible.additional())
        .map(as_effective)
        .collect();
    let inherited_from = match visible.active().map(|f| f.origin) {
        Some(from) if from != owner => Some((from, owner_name(store, from)?)),
        _ => None,
    };
    let above = visible
        .folders
        .iter()
        .filter(|f| f.origin != owner)
        .map(as_effective)
        .collect();
    let above_from = match parent_of(store, owner)? {
        Some(parent) => Some((parent, owner_name(store, parent)?)),
        None => None,
    };
    // Existence is checked later, off the store (`Folders::paths`).
    let missing = HashSet::new();
    Ok(Folders {
        own,
        effective,
        inherited_from,
        above,
        above_from,
        missing,
    })
}

const KB_PAGE: PageReq = PageReq {
    offset: 0,
    limit: 200,
};

pub fn load(store: &Store, owner: Owner, filter: EntryFilterKind) -> Result<Loaded> {
    let kb_scope = store.kb_scope(owner, &KbFilter::default(), KB_PAGE)?.items;
    let kb = kb_model::section(owner, kb_scope);
    let folders = load_folders(store, owner)?;
    match owner {
        Owner::Topic(id) => {
            let topic = store.topic(id)?;
            let project = store.project(topic.project)?;
            let epic = match topic.epic {
                Some(e) => Some((e, store.epic(e)?.title)),
                None => None,
            };
            let entries = store.entries(id, &filter.filter(), PageReq::first(TIMELINE_PAGE))?;
            Ok(Loaded {
                owner,
                title: topic.title.clone(),
                description: topic.description.clone(),
                updated_at: topic.stamp.updated_at,
                updated_by: topic.stamp.updated_by,
                project: (project.id, project.title),
                epic,
                entries: entries.items,
                entries_total: entries.total,
                topic: Some(topic),
                topics: Vec::new(),
                loose: Vec::new(),
                groups: Vec::new(),
                kb,
                kb_below: 0,
                folders,
            })
        }
        Owner::Epic(id) => {
            let epic = store.epic(id)?;
            let project = store.project(epic.project)?;
            let tree = store.tree(epic.project)?;
            let topics = tree
                .epics
                .into_iter()
                .find(|(e, _)| e.id == id)
                .map(|(_, t)| t)
                .unwrap_or_default();
            let below = store
                .kb_rollup(owner, &KbFilter::default(), PageReq::first(1))?
                .total;
            let own = kb.own.len() as i64;
            Ok(Loaded {
                owner,
                title: epic.title.clone(),
                description: epic.description.clone(),
                updated_at: epic.stamp.updated_at,
                updated_by: epic.stamp.updated_by,
                project: (project.id, project.title),
                epic: Some((epic.id, epic.title)),
                topic: None,
                entries: Vec::new(),
                entries_total: 0,
                topics,
                loose: Vec::new(),
                groups: Vec::new(),
                kb,
                kb_below: (below - own).max(0),
                folders,
            })
        }
        Owner::Project(id) => {
            let project = store.project(id)?;
            let tree = store.tree(id)?;
            let below = store
                .kb_rollup(owner, &KbFilter::default(), PageReq::first(1))?
                .total;
            let own = kb.own.len() as i64;
            Ok(Loaded {
                owner,
                title: project.title.clone(),
                description: project.description.clone(),
                updated_at: project.stamp.updated_at,
                updated_by: project.stamp.updated_by,
                project: (project.id, project.title),
                epic: None,
                topic: None,
                entries: Vec::new(),
                entries_total: 0,
                topics: Vec::new(),
                loose: tree.loose_topics,
                groups: tree
                    .epics
                    .into_iter()
                    .map(|(e, t)| (e.id, e.title, t))
                    .collect(),
                kb,
                kb_below: (below - own).max(0),
                folders,
            })
        }
    }
}

/// components-records.md 5, System event: consecutive status changes within
/// 5 minutes of each other read as one line, from the first status to the
/// last, by the same author. A run that ends where it started (a change and
/// its undo) leaves no line. `entries` is newest first, as the timeline
/// shows it.
pub fn collapse_status_events(entries: Vec<Entry>) -> Vec<Entry> {
    use acta_core::model::EventKind;
    const WINDOW_MS: i64 = 5 * 60 * 1000;
    let is_status = |e: &Entry| {
        e.event
            .as_ref()
            .is_some_and(|ev| ev.kind == EventKind::Status)
    };
    let mut out: Vec<Entry> = Vec::with_capacity(entries.len());
    // The time of the oldest change of the run the last line stands for,
    // while that line is a status change.
    let mut run: Option<i64> = None;
    for entry in entries {
        if let Some(oldest) = run
            && is_status(&entry)
            && let Some(line) = out.last_mut()
            && line.author == entry.author
            && oldest - entry.stamp.created_at <= WINDOW_MS
        {
            // `entry` is older: the line now starts at its old status.
            let old = entry.event.as_ref().and_then(|e| e.old.clone());
            if let Some(event) = line.event.as_mut() {
                event.old = old;
            }
            run = Some(entry.stamp.created_at);
            continue;
        }
        run = is_status(&entry).then_some(entry.stamp.created_at);
        out.push(entry);
    }
    out.retain(|e| {
        !is_status(e)
            || e.event
                .as_ref()
                .is_none_or(|ev| ev.old.is_none() || ev.old != ev.new)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_support::temp_store;
    use acta_core::model::{KbKind, Status};
    use acta_core::store::{NewEntry, NewKb, NewTopic};

    fn status_event(id: i64, at_min: i64, old: Status, new: Status, by: Author) -> Entry {
        use acta_core::model::{EntryId, EventKind, Stamp, SystemEvent, TopicId};
        Entry {
            id: EntryId(id),
            topic: TopicId(1),
            seq: None,
            kind: EntryKind::System,
            author: by,
            text: String::new(),
            event: Some(SystemEvent {
                kind: EventKind::Status,
                old: Some(old.as_str().into()),
                new: Some(new.as_str().into()),
            }),
            stamp: Stamp {
                uuid: String::new(),
                created_at: at_min * 60_000,
                updated_at: at_min * 60_000,
                updated_by: by,
                deleted_at: None,
                deleted_by: None,
            },
        }
    }

    fn lines(entries: &[Entry]) -> Vec<(Option<String>, Option<String>, i64)> {
        entries
            .iter()
            .map(|e| {
                let ev = e.event.clone().unwrap();
                (ev.old, ev.new, e.stamp.created_at / 60_000)
            })
            .collect()
    }

    #[test]
    fn a_status_change_and_its_undo_leave_no_line() {
        // Newest first: Active -> Blocked at 10, undone (Blocked -> Active)
        // a minute later.
        let me = Author::Human;
        let entries = vec![
            status_event(2, 11, Status::Blocked, Status::Active, me),
            status_event(1, 10, Status::Active, Status::Blocked, me),
        ];
        assert!(collapse_status_events(entries).is_empty());
    }

    #[test]
    fn consecutive_changes_within_five_minutes_read_as_one_line() {
        let me = Author::Human;
        let entries = vec![
            status_event(4, 30, Status::Blocked, Status::Done, me),
            status_event(3, 14, Status::Active, Status::Blocked, me),
            status_event(2, 11, Status::Planned, Status::Active, me),
            // More than 5 minutes before the next: its own line.
            status_event(1, 2, Status::Draft, Status::Planned, me),
        ];
        let p = |s: Status| Some(s.as_str().to_owned());
        assert_eq!(
            lines(&collapse_status_events(entries)),
            [
                (p(Status::Blocked), p(Status::Done), 30),
                (p(Status::Planned), p(Status::Blocked), 14),
                (p(Status::Draft), p(Status::Planned), 2),
            ]
        );
        // Another author breaks the run.
        let mixed = vec![
            status_event(2, 11, Status::Blocked, Status::Active, Author::Claude),
            status_event(1, 10, Status::Active, Status::Blocked, me),
        ];
        assert_eq!(collapse_status_events(mixed).len(), 2);
    }

    #[test]
    fn a_topic_page_loads_its_location_timeline_knowledge_and_inherited_folders() {
        let mut temp = temp_store();
        let dir = tempfile::tempdir().unwrap();
        let s = &mut temp.store;
        let me = Author::Human;
        let project = s.create_project("Acta", "", me).unwrap();
        let epic = s.create_epic(project, "Terminal", "", me).unwrap();
        let topic = s
            .create_topic(
                NewTopic {
                    project,
                    epic: Some(epic),
                    title: "Embed the terminal".into(),
                    description: String::new(),
                    status: Status::Active,
                },
                me,
            )
            .unwrap();
        s.log(
            topic,
            NewEntry {
                kind: EntryKind::Decision,
                text: "One session per project.".into(),
                state: None,
                status: None,
            },
            Author::Claude,
        )
        .unwrap();
        s.kb_create(
            NewKb {
                owner: Owner::Project(project),
                kind: KbKind::Memory,
                title: "Sample data".into(),
                summary: "Sample and test data uses the name Alexandre".into(),
                body: "Alexandre.".into(),
                from_file: false,
            },
            Author::Claude,
        )
        .unwrap();
        s.add_folder(Owner::Epic(epic), dir.path(), me).unwrap();

        let mut page = load(s, Owner::Topic(topic), EntryFilterKind::All).unwrap();
        page.folders.missing = crate::workspace::missing_paths(page.folders.paths());
        assert_eq!(page.title, "Embed the terminal");
        assert_eq!(page.project.1, "Acta");
        assert_eq!(page.epic.as_ref().unwrap().1, "Terminal");
        assert!(page.entries.iter().any(|e| e.kind == EntryKind::Decision));
        assert!(page.kb.own.is_empty());
        assert_eq!(page.kb.inherited_count(), 1);
        assert_eq!(page.folders.effective.len(), 1);
        assert_eq!(
            page.folders.inherited_from.as_ref().unwrap().1,
            "Terminal",
            "the folder row names where it comes from"
        );
        assert!(page.folders.missing.is_empty());

        let decisions = load(s, Owner::Topic(topic), EntryFilterKind::Decisions).unwrap();
        assert_eq!(decisions.entries.len(), 1);
        let progress = load(s, Owner::Topic(topic), EntryFilterKind::Progress).unwrap();
        assert!(progress.entries.is_empty());
    }

    #[test]
    fn epic_and_project_pages_count_progress_and_folders_not_found() {
        let mut temp = temp_store();
        let dir = tempfile::tempdir().unwrap();
        let s = &mut temp.store;
        let me = Author::Human;
        let project = s.create_project("Acta", "", me).unwrap();
        let epic = s.create_epic(project, "Terminal", "", me).unwrap();
        for (title, status) in [
            ("A", Status::Done),
            ("B", Status::Active),
            ("C", Status::Dropped),
        ] {
            s.create_topic(
                NewTopic {
                    project,
                    epic: Some(epic),
                    title: title.into(),
                    description: String::new(),
                    status,
                },
                me,
            )
            .unwrap();
        }
        let gone = dir.path().join("gone");
        std::fs::create_dir(&gone).unwrap();
        s.add_folder(Owner::Project(project), &gone, me).unwrap();
        std::fs::remove_dir(&gone).unwrap();

        let mut page = load(s, Owner::Epic(epic), EntryFilterKind::All).unwrap();
        assert!(
            page.folders.missing.is_empty(),
            "nothing checked under the store"
        );
        page.folders.missing = crate::workspace::missing_paths(page.folders.paths());
        assert_eq!(page.topics.len(), 3);
        assert_eq!(page.progress(), (1, 2));
        let missing = page.folders.primary().unwrap().path.clone();
        assert!(page.folders.is_missing(&missing));

        let page = load(s, Owner::Project(project), EntryFilterKind::All).unwrap();
        assert_eq!(page.groups.len(), 1);
        assert_eq!(page.progress(), (1, 2));
        assert!(page.folders.inherited_from.is_none());
    }
}
