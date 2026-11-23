//! What a project, epic or topic page shows, read from the store in one go
//! on a background thread (docs/design/layout.md 4.2: title, properties,
//! then the sections; data.md 11 for where each value comes from).

use std::collections::HashSet;

use acta_core::date::{Date, Zone};
use std::collections::HashMap;

use acta_core::model::{
    Author, Entry, EntryKind, EpicId, FolderId, Owner, PageReq, Place, ProjectId, QuestionId,
    QuestionStatus, Record, Report, Topic, TopicId, TopicSummary,
};
use acta_core::store::{EntryFilter, KbFilter, Overview, OverviewFilter, QuestionFilter, Scope};
use acta_core::{Result, Store};

use crate::folders_model::FolderItem;
use crate::kb_model::{self, Section};
use crate::questions_model;

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

/// The working folders of a page (docs/design/components-folders.md 5;
/// ADR 0011: always inherited, one active folder).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folders {
    /// Every folder the page sees, in list order.
    pub items: Vec<FolderItem>,
    /// The name of the page and of each level above it.
    pub levels: Vec<(Owner, String)>,
    /// Paths (as stored) that are not a folder on this computer.
    pub missing: HashSet<String>,
    /// The Settings default of "Commit in my name" (Settings > Claude Code).
    pub as_user_default: bool,
    /// The page's own folders that other topics (or epics) of the project
    /// have too (components-folders.md 5, Same folder on two topics).
    pub also: HashMap<FolderId, Also>,
}

/// The other owners of the same folder, with their names, and the level a
/// session started there outside Acta opens on (the nearest one they
/// share with the page).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Also {
    pub others: Vec<(Owner, String)>,
    pub level: (Owner, String),
}

/// The owners of the same kind as `owner` (topics for a topic, epics for
/// an epic), in the same project, other than `owner`, that have a folder
/// at `path` (the same folder: stored forms equal, ignoring case on
/// Windows and macOS). A project has no such line.
pub fn same_folder_owners(
    owner: Owner,
    project: ProjectId,
    path: &str,
    all: &[acta_core::model::Folder],
    os: acta_core::paths::Os,
) -> Vec<Owner> {
    let kind = |o: &Owner| match o {
        Owner::Topic(_) => 0,
        Owner::Epic(_) => 1,
        Owner::Project(_) => 2,
    };
    if matches!(owner, Owner::Project(_)) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for f in all {
        if f.project == project
            && f.owner != owner
            && kind(&f.owner) == kind(&owner)
            && acta_core::folder_path::same(os, &f.path, path)
            && !out.contains(&f.owner)
        {
            out.push(f.owner);
        }
    }
    out
}

impl Folders {
    pub fn active(&self) -> Option<&FolderItem> {
        self.items.iter().find(|f| f.active)
    }

    pub fn get(&self, id: FolderId) -> Option<&FolderItem> {
        self.items.iter().find(|f| f.id == id)
    }

    pub fn is_missing(&self, path: &str) -> bool {
        self.missing.contains(path)
    }

    /// Every path the page shows, for the existence check.
    pub fn paths(&self) -> Vec<String> {
        self.items.iter().map(|f| f.path.clone()).collect()
    }

    /// The name of a level (the page, its epic, its project).
    pub fn level_name(&self, owner: Owner) -> &str {
        self.levels
            .iter()
            .find(|(o, _)| *o == owner)
            .map_or("", |(_, n)| n.as_str())
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
    /// The store's flags of the section's entries (ADR 0015, 1); git's
    /// arrive later.
    pub kb_flags: Vec<(acta_core::model::KbId, Vec<acta_core::agent::kbq::Flag>)>,
    /// Epic and project pages: entries in the levels below.
    pub kb_below: i64,
    pub folders: Folders,
    /// The record's own due date (components-records.md 5, Due row).
    pub due_on: Option<Date>,
    /// Project and epic pages: the Overview's figures (request 17).
    pub overview: Option<Overview>,
    /// The topics behind the table's filter chip, in the list's order:
    /// the same store query as the figure that opened it.
    pub chip: Option<Chip>,
    /// The Questions section, its roll-up row and the questions Claude
    /// Code wrote that were not shown yet (components-questions.md 5).
    pub questions: QuestionsData,
    /// Topic and epic pages: who to report to, and whether it was.
    pub report: Report,
    /// Where the record sits, for the contacts it may name.
    pub place: Place,
    /// The names of the contacts the timeline's Report to events name, by
    /// UUID (drawn with the current name).
    pub event_names: HashMap<String, String>,
    /// The files of the description (topics and epics) and of the
    /// entries shown (components-attachments.md 5).
    pub files: HashMap<acta_core::model::AttachOwner, Vec<acta_core::model::Attachment>>,
}

/// What the Questions section and the topic table's two columns need.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuestionsData {
    pub section: questions_model::Section,
    pub rollup: questions_model::Rollup,
    /// Open questions per topic, as `(open, overdue)`.
    pub per_topic: HashMap<TopicId, (i64, i64)>,
    pub unread: HashSet<QuestionId>,
}

/// The page's questions, from the store in the same read.
fn questions(
    store: &Store,
    owner: Owner,
    project: ProjectId,
    today: Date,
) -> Result<QuestionsData> {
    let section = questions_model::Section::of(store.questions_of(owner)?);
    let (rollup, per_topic) = match owner {
        Owner::Topic(_) => (questions_model::Rollup::default(), HashMap::new()),
        Owner::Epic(_) | Owner::Project(_) => {
            let filter = QuestionFilter {
                statuses: vec![QuestionStatus::Open],
                epic: match owner {
                    Owner::Epic(e) => Some(e),
                    _ => None,
                },
                ..QuestionFilter::default()
            };
            let scope = store.find_questions(project, &filter)?;
            (
                questions_model::rollup(owner, &scope, today),
                store.open_questions_by_topic(project, today)?,
            )
        }
    };
    let unread = store
        .unread_questions()?
        .into_iter()
        .filter(|(_, o)| *o == owner)
        .map(|(q, _)| q)
        .collect();
    Ok(QuestionsData {
        section,
        rollup,
        per_topic,
        unread,
    })
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

    /// Open for due dates: a topic not closed; an epic or a project below
    /// 100% or with no topic (components-records.md 5).
    pub fn due_open(&self) -> bool {
        match &self.topic {
            Some(topic) => topic.status.is_open(),
            None => {
                let (done, total) = self.progress();
                total == 0 || done < total
            }
        }
    }

    /// Every topic the page lists (an epic's, or a project's in all its
    /// groups), in the tree's order.
    pub fn all_topics(&self) -> Vec<TopicSummary> {
        let mut all = self.topics.clone();
        all.extend(self.loose.iter().cloned());
        for (_, _, topics) in &self.groups {
            all.extend(topics.iter().cloned());
        }
        all
    }
}

/// A filter chip of the topic table and the topics it lists.
pub type Chip = (OverviewFilter, Vec<TopicId>);

/// The Overview of an epic or a project page for a reader on `today`, and
/// the list behind `chip` when the table shows one.
fn overview(
    store: &Store,
    scope: Scope,
    today: Date,
    chip: Option<OverviewFilter>,
) -> Result<(Overview, Option<Chip>)> {
    let zone = Zone::system();
    let figures = store.overview(scope, today, zone)?;
    let chip = match chip {
        Some(filter) => {
            let ids = store
                .overview_list(scope, filter, today, zone)?
                .into_iter()
                .filter_map(|item| match item.record {
                    Record::Topic(id) => Some(id),
                    _ => None,
                })
                .collect();
            Some((filter, ids))
        }
        None => None,
    };
    Ok((figures, chip))
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

/// The folders `owner` sees, as stored. `missing` is left empty: the
/// caller checks the disk with [`Folders::paths`] once the store is
/// released.
pub fn load_folders(store: &Store, owner: Owner) -> Result<Folders> {
    let visible = store.visible_folders(owner)?;
    let items = visible
        .folders
        .iter()
        .map(|v| FolderItem {
            id: v.folder.id,
            path: v.folder.path.clone(),
            name: v.folder.display_name().to_owned(),
            own_name: v.folder.name.clone(),
            icon: v.folder.icon.clone(),
            origin: v.origin,
            own: v.origin == owner,
            active: v.active,
            commit_mode: v.folder.commit_mode,
            commit_as_user: v.folder.commit_as_user,
            commit_on_default: v.folder.commit_on_default,
        })
        .collect();
    let mut levels = vec![(owner, owner_name(store, owner)?)];
    let mut level = owner;
    while let Some(parent) = parent_of(store, level)? {
        levels.push((parent, owner_name(store, parent)?));
        level = parent;
    }
    let mut also = HashMap::new();
    if !matches!(owner, Owner::Project(_)) {
        let all = store.all_folders()?;
        let os = acta_core::paths::Os::CURRENT;
        for v in visible.folders.iter().filter(|v| v.origin == owner) {
            let others = same_folder_owners(owner, v.folder.project, &v.folder.path, &all, os);
            if others.is_empty() {
                continue;
            }
            // The nearest level they share: the epic when every topic is
            // in the same one, else the project.
            let shared = match owner {
                Owner::Topic(_) => {
                    let mine = parent_of(store, owner)?;
                    let mut same = true;
                    for o in &others {
                        same &= parent_of(store, *o)? == mine;
                    }
                    match mine {
                        Some(Owner::Epic(e)) if same => Owner::Epic(e),
                        _ => Owner::Project(v.folder.project),
                    }
                }
                _ => Owner::Project(v.folder.project),
            };
            let mut named = Vec::new();
            for o in others {
                named.push((o, owner_name(store, o)?));
            }
            also.insert(
                v.folder.id,
                Also {
                    others: named,
                    level: (shared, owner_name(store, shared)?),
                },
            );
        }
    }
    Ok(Folders {
        items,
        levels,
        missing: HashSet::new(),
        as_user_default: acta_core::claude_settings::as_user_default(store)?,
        also,
    })
}

const KB_PAGE: PageReq = PageReq {
    offset: 0,
    limit: 200,
};

/// The page of `owner`. `today` is the reader's local date (the Overview
/// counts in local days); `chip` the table's filter chip, if any.
pub fn load(
    store: &Store,
    owner: Owner,
    filter: EntryFilterKind,
    today: Date,
    chip: Option<OverviewFilter>,
) -> Result<Loaded> {
    let kb_scope = store.kb_scope(owner, &KbFilter::default(), KB_PAGE)?.items;
    let kb = kb_model::section(owner, kb_scope);
    let kb_ids: Vec<acta_core::model::KbId> = kb
        .own
        .iter()
        .chain(kb.inherited.iter().flat_map(|(_, e)| e.iter()))
        .map(|e| e.id)
        .collect();
    let kb_flags = crate::kbq_ui::store_flags_of(store, &kb_ids, today)?;
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
            let uuids: Vec<String> = entries
                .items
                .iter()
                .filter_map(|e| e.event.as_ref())
                .filter(|ev| {
                    matches!(
                        ev.kind,
                        acta_core::model::EventKind::Report | acta_core::model::EventKind::Reported
                    )
                })
                .flat_map(|ev| [ev.old.clone(), ev.new.clone()])
                .flatten()
                .collect();
            let event_names = store.contact_names(&uuids)?;
            let owners: Vec<acta_core::model::AttachOwner> =
                std::iter::once(acta_core::model::AttachOwner::Topic(id))
                    .chain(
                        entries
                            .items
                            .iter()
                            .map(|e| acta_core::model::AttachOwner::Entry(e.id)),
                    )
                    .collect();
            Ok(Loaded {
                files: store.attachments_of(&owners)?,
                event_names,
                questions: questions(store, owner, topic.project, today)?,
                report: topic.report.clone(),
                place: Place {
                    project: topic.project,
                    epic: topic.epic,
                },
                owner,
                title: topic.title.clone(),
                description: topic.description.clone(),
                updated_at: topic.stamp.updated_at,
                updated_by: topic.stamp.updated_by,
                project: (project.id, project.title),
                epic,
                entries: entries.items,
                entries_total: entries.total,
                due_on: topic.due_on,
                overview: None,
                chip: None,
                topic: Some(topic),
                topics: Vec::new(),
                loose: Vec::new(),
                groups: Vec::new(),
                kb,
                kb_flags: kb_flags.clone(),
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
            let (figures, chip) = overview(store, Scope::Epic(id), today, chip)?;
            Ok(Loaded {
                files: store.attachments_of(&[acta_core::model::AttachOwner::Epic(id)])?,
                event_names: HashMap::new(),
                questions: questions(store, owner, epic.project, today)?,
                report: epic.report.clone(),
                place: Place {
                    project: epic.project,
                    epic: Some(epic.id),
                },
                owner,
                due_on: epic.due_on,
                overview: Some(figures),
                chip,
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
                kb_flags: kb_flags.clone(),
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
            let (figures, chip) = overview(store, Scope::Project(id), today, chip)?;
            Ok(Loaded {
                files: HashMap::new(),
                event_names: HashMap::new(),
                questions: questions(store, owner, id, today)?,
                report: Report::default(),
                place: Place {
                    project: id,
                    epic: None,
                },
                owner,
                due_on: project.due_on,
                overview: Some(figures),
                chip,
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
                kb_flags: kb_flags.clone(),
                kb_below: (below - own).max(0),
                folders,
            })
        }
    }
}

/// A block of a record page that `Up` / `Down` step through
/// (interaction.md 6.1): the title, the folder row, the current state, the
/// description, the Questions section and each entry of the timeline.
/// `Enter` edits the focused block (or goes inside the folder row and the
/// Questions section).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Block {
    Title,
    Folders,
    State,
    Description,
    Questions,
    Entry(acta_core::model::EntryId),
}

/// The blocks of a page, in reading order: a topic has a current state and
/// a timeline (`entries`: the entries shown, system lines excluded, newest
/// first); the folder row is a block while it is drawn.
pub fn blocks(topic: bool, folders: bool, entries: &[Entry]) -> Vec<Block> {
    let mut out = vec![Block::Title];
    if folders {
        out.push(Block::Folders);
    }
    if topic {
        out.push(Block::State);
    }
    out.push(Block::Description);
    out.push(Block::Questions);
    if topic {
        out.extend(
            entries
                .iter()
                .filter(|e| e.kind != EntryKind::System)
                .map(|e| Block::Entry(e.id)),
        );
    }
    out
}

/// `Up` / `Down` from `at` (`None`: the page itself has the keyboard, and
/// `Down` goes to the first block). The ends stay where they are.
pub fn step_block(order: &[Block], at: Option<Block>, up: bool) -> Option<Block> {
    let Some(at) = at else {
        return if up { None } else { order.first().copied() };
    };
    let i = order.iter().position(|b| *b == at)?;
    if up {
        order.get(i.checked_sub(1)?).copied()
    } else {
        order.get(i + 1).copied()
    }
}

/// The entries the timeline shows, newest first: every entry but the
/// system events that have no line in the catalogue yet (data.md 11,
/// request 10); status and due date changes are shown, collapsed by
/// [`collapse_status_events`].
pub fn timeline_entries(entries: &[Entry]) -> Vec<Entry> {
    use acta_core::model::EventKind;
    collapse_status_events(
        entries
            .iter()
            .filter(|e| {
                e.kind != EntryKind::System
                    || e.event.as_ref().is_some_and(|ev| {
                        matches!(
                            ev.kind,
                            EventKind::Status
                                | EventKind::Due
                                | EventKind::Report
                                | EventKind::Reported
                        )
                    })
            })
            .cloned()
            .collect(),
    )
}

/// components-records.md 5, System event: consecutive status changes, or
/// due date changes, within 5 minutes of each other read as one line, from
/// the first value to the last, by the same author. A run that ends where
/// it started (a change and its undo) leaves no line. `entries` is newest
/// first, as the timeline shows it.
pub fn collapse_status_events(entries: Vec<Entry>) -> Vec<Entry> {
    use acta_core::model::EventKind;
    const WINDOW_MS: i64 = 5 * 60 * 1000;
    let kind = |e: &Entry| {
        e.event
            .as_ref()
            .map(|ev| ev.kind)
            .filter(|k| matches!(k, EventKind::Status | EventKind::Due))
    };
    let mut out: Vec<Entry> = Vec::with_capacity(entries.len());
    // The time of the oldest change of the run the last line stands for,
    // while that line is a status or due date change.
    let mut run: Option<i64> = None;
    for entry in entries {
        if let Some(oldest) = run
            && let Some(line) = out.last_mut()
            && kind(&entry).is_some()
            && kind(line) == kind(&entry)
            && line.author == entry.author
            && oldest - entry.stamp.created_at <= WINDOW_MS
        {
            // `entry` is older: the line now starts at its old value.
            let old = entry.event.as_ref().and_then(|e| e.old.clone());
            if let Some(event) = line.event.as_mut() {
                event.old = old;
            }
            run = Some(entry.stamp.created_at);
            continue;
        }
        run = kind(&entry).is_some().then_some(entry.stamp.created_at);
        out.push(entry);
    }
    out.retain(|e| match (kind(e), e.event.as_ref()) {
        (Some(EventKind::Status), Some(ev)) => ev.old.is_none() || ev.old != ev.new,
        // A date set then cleared (old and new both none) is no change.
        (Some(EventKind::Due), Some(ev)) => ev.old != ev.new,
        _ => true,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_of(id: i64, kind: EntryKind) -> Entry {
        let mut e = status_event(id, 0, Status::Draft, Status::Done, Author::Human);
        e.kind = kind;
        e
    }

    #[test]
    fn a_topic_page_steps_title_folders_state_description_questions_entries() {
        let entries = [
            entry_of(3, EntryKind::Decision),
            entry_of(2, EntryKind::System),
            entry_of(1, EntryKind::Progress),
        ];
        let order = blocks(true, true, &entries);
        assert_eq!(
            order,
            vec![
                Block::Title,
                Block::Folders,
                Block::State,
                Block::Description,
                Block::Questions,
                Block::Entry(acta_core::model::EntryId(3)),
                Block::Entry(acta_core::model::EntryId(1)),
            ]
        );
        // An epic has no state and no timeline; no folder row, no stop.
        assert_eq!(
            blocks(false, false, &entries),
            vec![Block::Title, Block::Description, Block::Questions]
        );
        assert_eq!(step_block(&order, None, false), Some(Block::Title));
        assert_eq!(step_block(&order, None, true), None);
        assert_eq!(step_block(&order, Some(Block::Title), true), None);
        assert_eq!(
            step_block(&order, Some(Block::Questions), false),
            Some(Block::Entry(acta_core::model::EntryId(3)))
        );
        assert_eq!(
            step_block(
                &order,
                Some(Block::Entry(acta_core::model::EntryId(1))),
                false
            ),
            None
        );
        assert_eq!(
            step_block(&order, Some(Block::State), true),
            Some(Block::Folders)
        );
    }
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

    fn due_event(id: i64, at_min: i64, old: Option<&str>, new: Option<&str>) -> Entry {
        let mut e = status_event(id, at_min, Status::Draft, Status::Draft, Author::Human);
        e.event = Some(acta_core::model::SystemEvent {
            kind: acta_core::model::EventKind::Due,
            old: old.map(Into::into),
            new: new.map(Into::into),
        });
        e
    }

    #[test]
    fn due_date_events_reach_the_timeline() {
        let mut temp = temp_store();
        let s = &mut temp.store;
        let me = Author::Human;
        let project = s.create_project("Acta", "", me).unwrap();
        let topic = s
            .create_topic(
                NewTopic {
                    project,
                    epic: None,
                    title: "Embed the terminal".into(),
                    description: String::new(),
                    status: Status::Planned,
                },
                me,
            )
            .unwrap();
        let day = acta_core::date::Date::parse_iso("2026-12-15");
        s.set_due(Owner::Topic(topic), day, me).unwrap();
        let page = load(s, Owner::Topic(topic), EntryFilterKind::All, today(), None).unwrap();
        let shown = timeline_entries(&page.entries);
        let due: Vec<_> = shown
            .iter()
            .filter_map(|e| e.event.as_ref())
            .filter(|ev| ev.kind == acta_core::model::EventKind::Due)
            .collect();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].new.as_deref(), Some("2026-12-15"));
        // Other system events (creation) stay hidden.
        assert!(shown.iter().all(|e| {
            e.kind != EntryKind::System
                || e.event
                    .as_ref()
                    .is_some_and(|ev| ev.kind == acta_core::model::EventKind::Due)
        }));
    }

    #[test]
    fn due_date_changes_collapse_into_the_last_and_an_undo_leaves_nothing() {
        // Newest first: set at 10, changed at 12, changed at 13.
        let entries = vec![
            due_event(3, 13, Some("2026-12-02"), Some("2026-12-15")),
            due_event(2, 12, Some("2026-12-01"), Some("2026-12-02")),
            due_event(1, 10, None, Some("2026-12-01")),
        ];
        assert_eq!(
            lines(&collapse_status_events(entries)),
            [(None, Some("2026-12-15".into()), 13)]
        );
        // Set and cleared within 5 minutes: no line.
        let undone = vec![
            due_event(2, 11, Some("2026-12-01"), None),
            due_event(1, 10, None, Some("2026-12-01")),
        ];
        assert!(collapse_status_events(undone).is_empty());
        // A status change between them breaks the run.
        let mixed = vec![
            due_event(3, 12, Some("2026-12-01"), None),
            status_event(2, 11, Status::Planned, Status::Active, Author::Human),
            due_event(1, 10, None, Some("2026-12-01")),
        ];
        assert_eq!(collapse_status_events(mixed).len(), 3);
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
    fn a_folder_on_two_topics_names_the_other_and_the_level_they_share() {
        let mut temp = temp_store();
        let dir = tempfile::tempdir().unwrap();
        let s = &mut temp.store;
        let me = Author::Human;
        let project = s.create_project("Acta", "", me).unwrap();
        let epic = s.create_epic(project, "Terminal", "", me).unwrap();
        let topic = |s: &mut Store, title: &str, epic: Option<EpicId>| {
            s.create_topic(
                NewTopic {
                    project,
                    epic,
                    title: title.into(),
                    description: String::new(),
                    status: Status::Active,
                },
                me,
            )
            .unwrap()
        };
        let embed = topic(s, "Embed the terminal", Some(epic));
        let scroll = topic(s, "Scrollback", Some(epic));
        let loose = topic(s, "Links", None);
        s.add_folder(Owner::Topic(embed), dir.path(), me).unwrap();
        s.add_folder(Owner::Topic(scroll), dir.path(), me).unwrap();

        let page = load_folders(s, Owner::Topic(embed)).unwrap();
        let own = page.items.iter().find(|f| f.own).unwrap();
        let also = page.also.get(&own.id).unwrap();
        assert_eq!(
            also.others,
            vec![(Owner::Topic(scroll), "Scrollback".to_owned())]
        );
        assert_eq!(also.level, (Owner::Epic(epic), "Terminal".to_owned()));

        // A third topic outside the epic: they share only the project.
        s.add_folder(Owner::Topic(loose), dir.path(), me).unwrap();
        let page = load_folders(s, Owner::Topic(embed)).unwrap();
        let own = page.items.iter().find(|f| f.own).unwrap();
        let also = page.also.get(&own.id).unwrap();
        assert_eq!(also.others.len(), 2);
        assert_eq!(also.level, (Owner::Project(project), "Acta".to_owned()));
        // The epic page has no line: the topics are not of its kind.
        assert!(load_folders(s, Owner::Epic(epic)).unwrap().also.is_empty());
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

        let mut page = load(s, Owner::Topic(topic), EntryFilterKind::All, today(), None).unwrap();
        page.folders.missing = crate::workspace::missing_paths(page.folders.paths());
        assert_eq!(page.title, "Embed the terminal");
        assert_eq!(page.project.1, "Acta");
        assert_eq!(page.epic.as_ref().unwrap().1, "Terminal");
        assert!(page.entries.iter().any(|e| e.kind == EntryKind::Decision));
        assert!(page.kb.own.is_empty());
        assert_eq!(page.kb.inherited_count(), 1);
        assert_eq!(page.folders.items.len(), 1);
        let active = page.folders.active().unwrap();
        assert!(!active.own, "inherited");
        assert_eq!(
            page.folders.level_name(active.origin),
            "Terminal",
            "the folder row names where it comes from"
        );
        assert!(page.folders.missing.is_empty());

        let decisions = load(
            s,
            Owner::Topic(topic),
            EntryFilterKind::Decisions,
            today(),
            None,
        )
        .unwrap();
        assert_eq!(decisions.entries.len(), 1);
        let progress = load(
            s,
            Owner::Topic(topic),
            EntryFilterKind::Progress,
            today(),
            None,
        )
        .unwrap();
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

        let mut page = load(s, Owner::Epic(epic), EntryFilterKind::All, today(), None).unwrap();
        assert!(
            page.folders.missing.is_empty(),
            "nothing checked under the store"
        );
        page.folders.missing = crate::workspace::missing_paths(page.folders.paths());
        assert_eq!(page.topics.len(), 3);
        assert_eq!(page.progress(), (1, 2));
        let missing = page.folders.active().unwrap().path.clone();
        assert!(page.folders.is_missing(&missing));

        let page = load(
            s,
            Owner::Project(project),
            EntryFilterKind::All,
            today(),
            None,
        )
        .unwrap();
        assert_eq!(page.groups.len(), 1);
        assert_eq!(page.progress(), (1, 2));
        assert!(page.folders.active().unwrap().own);
    }

    fn today() -> Date {
        Zone::system().today()
    }

    #[test]
    fn pages_carry_their_date_the_overview_and_the_chip_list() {
        let mut temp = temp_store();
        let s = &mut temp.store;
        let me = Author::Human;
        let project = s.create_project("Acta", "", me).unwrap();
        let epic = s.create_epic(project, "Terminal", "", me).unwrap();
        let mut topics = Vec::new();
        for (title, status) in [
            ("Embed the terminal", Status::Blocked),
            ("Scrollback", Status::Active),
            ("Links", Status::Blocked),
        ] {
            topics.push(
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
                .unwrap(),
            );
        }
        let yesterday = today().add_days(-1);
        s.set_due(Owner::Topic(topics[1]), Some(yesterday), me)
            .unwrap();
        s.set_due(Owner::Epic(epic), Some(yesterday), me).unwrap();

        let page = load(s, Owner::Epic(epic), EntryFilterKind::All, today(), None).unwrap();
        assert_eq!(page.due_on, Some(yesterday));
        assert!(page.due_open(), "no topic done: open");
        let figures = page.overview.as_ref().unwrap();
        assert_eq!((figures.blocked, figures.overdue), (2, 1));
        assert!(page.chip.is_none());

        // The chip's list is the figure's: same count, tree order.
        let page = load(
            s,
            Owner::Project(project),
            EntryFilterKind::All,
            today(),
            Some(OverviewFilter::Blocked),
        )
        .unwrap();
        let (filter, ids) = page.chip.clone().unwrap();
        assert_eq!(filter, OverviewFilter::Blocked);
        assert_eq!(ids, [topics[0], topics[2]]);
        assert_eq!(ids.len(), page.overview.as_ref().unwrap().blocked);

        // A topic page has its date and no Overview.
        let page = load(
            s,
            Owner::Topic(topics[1]),
            EntryFilterKind::All,
            today(),
            None,
        )
        .unwrap();
        assert_eq!(page.due_on, Some(yesterday));
        assert!(page.overview.is_none());
        s.update_topic(
            topics[1],
            acta_core::store::TopicPatch {
                status: Some(Status::Done),
                ..Default::default()
            },
            me,
        )
        .unwrap();
        let page = load(
            s,
            Owner::Topic(topics[1]),
            EntryFilterKind::All,
            today(),
            None,
        )
        .unwrap();
        assert!(!page.due_open(), "a done topic is closed");
    }
}
