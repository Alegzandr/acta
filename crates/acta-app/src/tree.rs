//! The sidebar tree as rows (docs/design/layout.md 4.2, components.md 5 list /
//! tree row): every project; under an expanded project its Inbox and
//! Knowledge rows, the topics outside any epic, then the epics with their
//! topics. Closed topics collapse under a "3 closed" row at the end of their
//! group (color.md 2.3). Pure, so the order and the expansion rules are
//! tested without a window.

use std::collections::HashSet;

use acta_core::model::{EpicId, ProjectId, ProjectTree, Status, TopicId, TopicSummary};

use crate::route::Route;

/// The group a "3 closed" row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// The topics of a project outside any epic.
    Loose(ProjectId),
    Epic(EpicId),
}

impl Group {
    pub fn key(self) -> String {
        match self {
            Group::Loose(id) => format!("closed:{id}"),
            Group::Epic(id) => format!("closed:{id}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Project {
        id: ProjectId,
        title: String,
        expanded: bool,
    },
    Inbox {
        project: ProjectId,
        count: i64,
    },
    Knowledge {
        project: ProjectId,
        count: i64,
    },
    Epic {
        id: EpicId,
        project: ProjectId,
        title: String,
        expanded: bool,
    },
    Topic {
        id: TopicId,
        title: String,
        status: Status,
        depth: u8,
    },
    Closed {
        group: Group,
        count: usize,
        shown: bool,
        depth: u8,
    },
}

impl Row {
    /// Indentation level: project 0, epic and loose topic 1, topic in an
    /// epic 2 (16 per level).
    pub fn depth(&self) -> u8 {
        match self {
            Row::Project { .. } => 0,
            Row::Inbox { .. } | Row::Knowledge { .. } | Row::Epic { .. } => 1,
            Row::Topic { depth, .. } | Row::Closed { depth, .. } => *depth,
        }
    }

    /// The page the row opens, if it opens one.
    pub fn route(&self) -> Option<Route> {
        match self {
            Row::Project { id, .. } => Some(Route::Project(*id)),
            Row::Inbox { project, .. } => Some(Route::Inbox(*project)),
            Row::Knowledge { project, .. } => Some(Route::Knowledge(*project)),
            Row::Epic { id, .. } => Some(Route::Epic(*id)),
            Row::Topic { id, .. } => Some(Route::Topic(*id)),
            Row::Closed { .. } => None,
        }
    }

    /// The key of the row's expansion state, if it folds.
    pub fn fold_key(&self) -> Option<String> {
        match self {
            Row::Project { id, .. } => Some(id.to_string()),
            Row::Epic { id, .. } => Some(id.to_string()),
            Row::Closed { group, .. } => Some(group.key()),
            _ => None,
        }
    }

    pub fn is_expanded(&self) -> Option<bool> {
        match self {
            Row::Project { expanded, .. } | Row::Epic { expanded, .. } => Some(*expanded),
            Row::Closed { shown, .. } => Some(*shown),
            _ => None,
        }
    }
}

/// What the user folded and unfolded. Projects and epics start expanded and
/// are listed when collapsed; closed groups start folded and are listed
/// when shown. Both hold record ids only (docs/design/layout.md 4.7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folds {
    pub collapsed: HashSet<String>,
    pub closed_shown: HashSet<String>,
}

impl Folds {
    /// Flip the fold of the row with `key` (a `closed:` key is a closed group).
    pub fn toggle(&mut self, key: String) {
        let set = if key.starts_with("closed:") {
            &mut self.closed_shown
        } else {
            &mut self.collapsed
        };
        if !set.remove(&key) {
            set.insert(key);
        }
    }

    /// Unfold a project or epic (the project of the page that opens).
    pub fn expand(&mut self, key: &str) {
        self.collapsed.remove(key);
    }
}

fn push_topics(
    rows: &mut Vec<Row>,
    topics: &[TopicSummary],
    group: Group,
    depth: u8,
    folds: &Folds,
) {
    let (open, closed): (Vec<_>, Vec<_>) = topics.iter().partition(|t| t.status.is_open());
    for topic in open {
        rows.push(topic_row(topic, depth));
    }
    if closed.is_empty() {
        return;
    }
    let shown = folds.closed_shown.contains(&group.key());
    rows.push(Row::Closed {
        group,
        count: closed.len(),
        shown,
        depth,
    });
    if shown {
        for topic in closed {
            rows.push(topic_row(topic, depth));
        }
    }
}

fn topic_row(topic: &TopicSummary, depth: u8) -> Row {
    Row::Topic {
        id: topic.id,
        title: topic.title.clone(),
        status: topic.status,
        depth,
    }
}

/// The visible rows, top to bottom.
pub fn rows(projects: &[ProjectTree], folds: &Folds) -> Vec<Row> {
    let mut rows = Vec::new();
    for tree in projects {
        let project = &tree.project;
        let expanded = !folds.collapsed.contains(&project.id.to_string());
        rows.push(Row::Project {
            id: project.id,
            title: project.title.clone(),
            expanded,
        });
        if !expanded {
            continue;
        }
        rows.push(Row::Inbox {
            project: project.id,
            count: project.inbox,
        });
        rows.push(Row::Knowledge {
            project: project.id,
            count: project.knowledge,
        });
        push_topics(
            &mut rows,
            &tree.loose_topics,
            Group::Loose(project.id),
            1,
            folds,
        );
        for (epic, topics) in &tree.epics {
            let expanded = !folds.collapsed.contains(&epic.id.to_string());
            rows.push(Row::Epic {
                id: epic.id,
                project: project.id,
                title: epic.title.clone(),
                expanded,
            });
            if expanded {
                push_topics(&mut rows, topics, Group::Epic(epic.id), 2, folds);
            }
        }
    }
    rows
}

/// Where the keyboard cursor goes on `Left`: the row's parent, or `None`
/// for a top-level row.
pub fn parent_index(rows: &[Row], at: usize) -> Option<usize> {
    let depth = rows.get(at)?.depth();
    if depth == 0 {
        return None;
    }
    (0..at).rev().find(|&i| rows[i].depth() < depth)
}

/// `Alt+Up` / `Alt+Down`: the index to give `Store::reorder` so that `id`
/// passes its visible neighbour. `stored` is every sibling in the stored
/// order (closed topics included), `visible` the siblings the tree shows,
/// in the same order. `None` at either end.
pub fn move_target<T: PartialEq>(stored: &[T], visible: &[T], id: &T, up: bool) -> Option<usize> {
    let at = visible.iter().position(|v| v == id)?;
    let neighbour = if up {
        visible.get(at.checked_sub(1)?)?
    } else {
        visible.get(at + 1)?
    };
    stored.iter().position(|s| s == neighbour)
}

/// The stored siblings of a tree record (topics of the same group, epics of
/// the same project, or every project), as records.
pub fn stored_siblings(projects: &[ProjectTree], row: &Row) -> Vec<acta_core::model::Record> {
    use acta_core::model::Record;
    match row {
        Row::Project { .. } => projects
            .iter()
            .map(|t| Record::Project(t.project.id))
            .collect(),
        Row::Epic { project, .. } => projects
            .iter()
            .find(|t| t.project.id == *project)
            .map(|t| t.epics.iter().map(|(e, _)| Record::Epic(e.id)).collect())
            .unwrap_or_default(),
        Row::Topic { id, .. } => projects
            .iter()
            .find_map(|t| {
                if t.loose_topics.iter().any(|x| x.id == *id) {
                    return Some(t.loose_topics.iter().map(|x| Record::Topic(x.id)).collect());
                }
                t.epics
                    .iter()
                    .find(|(_, topics)| topics.iter().any(|x| x.id == *id))
                    .map(|(_, topics)| topics.iter().map(|x| Record::Topic(x.id)).collect())
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The record of a row that can move or be deleted.
pub fn row_record(row: &Row) -> Option<acta_core::model::Record> {
    use acta_core::model::Record;
    match row {
        Row::Topic { id, .. } => Some(Record::Topic(*id)),
        Row::Epic { id, .. } => Some(Record::Epic(*id)),
        Row::Project { id, .. } => Some(Record::Project(*id)),
        _ => None,
    }
}

/// The project, epic and topic a page belongs to, as far as the tree knows:
/// the breadcrumb (docs/design/layout.md 4.3) and the active project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Location {
    pub project: Option<(ProjectId, String)>,
    pub epic: Option<(EpicId, String)>,
    pub topic: Option<(TopicId, String)>,
}

pub fn locate_topic(projects: &[ProjectTree], id: TopicId) -> Option<Location> {
    for tree in projects {
        let project = Some((tree.project.id, tree.project.title.clone()));
        if let Some(topic) = tree.loose_topics.iter().find(|t| t.id == id) {
            return Some(Location {
                project,
                epic: None,
                topic: Some((topic.id, topic.title.clone())),
            });
        }
        for (epic, topics) in &tree.epics {
            if let Some(topic) = topics.iter().find(|t| t.id == id) {
                return Some(Location {
                    project,
                    epic: Some((epic.id, epic.title.clone())),
                    topic: Some((topic.id, topic.title.clone())),
                });
            }
        }
    }
    None
}

pub fn locate_epic(projects: &[ProjectTree], id: EpicId) -> Option<Location> {
    projects.iter().find_map(|tree| {
        let (epic, _) = tree.epics.iter().find(|(e, _)| e.id == id)?;
        Some(Location {
            project: Some((tree.project.id, tree.project.title.clone())),
            epic: Some((epic.id, epic.title.clone())),
            topic: None,
        })
    })
}

pub fn locate_project(projects: &[ProjectTree], id: ProjectId) -> Option<Location> {
    projects
        .iter()
        .find(|t| t.project.id == id)
        .map(|tree| Location {
            project: Some((tree.project.id, tree.project.title.clone())),
            ..Location::default()
        })
}

#[cfg(test)]
pub mod sample {
    //! A tree like the wireframe of docs/design/screens.md 8.1.
    use super::*;
    use acta_core::model::{Author, EpicSummary, Progress, ProjectSummary};

    pub fn topic(id: i64, epic: Option<i64>, title: &str, status: Status) -> TopicSummary {
        TopicSummary {
            id: TopicId(id),
            epic: epic.map(EpicId),
            title: title.into(),
            status,
            position: id,
            entries: 0,
            updated_at: 0,
            updated_by: Author::Human,
        }
    }

    pub fn epic(id: i64, title: &str) -> EpicSummary {
        EpicSummary {
            id: EpicId(id),
            title: title.into(),
            position: id,
            progress: Progress::default(),
            updated_at: 0,
        }
    }

    pub fn project(id: i64, title: &str) -> ProjectSummary {
        ProjectSummary {
            id: ProjectId(id),
            title: title.into(),
            position: id,
            progress: Progress::default(),
            inbox: 4,
            knowledge: 55,
            updated_at: 0,
        }
    }

    pub fn acta() -> Vec<ProjectTree> {
        vec![
            ProjectTree {
                project: project(1, "Acta"),
                loose_topics: vec![topic(1, None, "Naming", Status::Draft)],
                epics: vec![(
                    epic(1, "Terminal"),
                    vec![
                        topic(2, Some(1), "Embed the terminal", Status::Active),
                        topic(3, Some(1), "Scrollback", Status::Blocked),
                        topic(4, Some(1), "Terminal spike", Status::Done),
                        topic(5, Some(1), "Old idea", Status::Dropped),
                    ],
                )],
            },
            ProjectTree {
                project: project(2, "Atlas"),
                loose_topics: vec![],
                epics: vec![],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::sample::acta;
    use super::*;

    fn labels(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                Row::Project { title, .. } => format!("P {title}"),
                Row::Inbox { count, .. } => format!("  inbox {count}"),
                Row::Knowledge { count, .. } => format!("  knowledge {count}"),
                Row::Epic { title, .. } => format!("  E {title}"),
                Row::Topic { title, depth, .. } => {
                    format!("{}{title}", " ".repeat(*depth as usize * 2))
                }
                Row::Closed { count, depth, .. } => {
                    format!("{}{count} closed", " ".repeat(*depth as usize * 2))
                }
            })
            .collect()
    }

    #[test]
    fn rows_follow_the_shell_order_and_fold_closed_topics() {
        let rows = rows(&acta(), &Folds::default());
        assert_eq!(
            labels(&rows),
            [
                "P Acta",
                "  inbox 4",
                "  knowledge 55",
                "  Naming",
                "  E Terminal",
                "    Embed the terminal",
                "    Scrollback",
                "    2 closed",
                "P Atlas",
                "  inbox 4",
                "  knowledge 55",
            ]
        );
    }

    #[test]
    fn folds_hide_and_show_and_round_trip() {
        let mut folds = Folds::default();
        folds.toggle("E-1".into());
        folds.toggle("P-2".into());
        let rows = rows(&acta(), &folds);
        assert!(!labels(&rows).iter().any(|l| l.contains("Embed")));
        assert_eq!(labels(&rows).last().unwrap(), "P Atlas");

        folds.toggle("E-1".into());
        folds.toggle(Group::Epic(EpicId(1)).key());
        let rows = super::rows(&acta(), &folds);
        let labels = labels(&rows);
        assert!(labels.contains(&"    Terminal spike".to_string()));
        assert!(labels.contains(&"    Old idea".to_string()));

        folds.expand("P-2");
        assert!(folds.collapsed.is_empty());
    }

    #[test]
    fn left_goes_to_the_parent_row() {
        let rows = rows(&acta(), &Folds::default());
        let embed = rows
            .iter()
            .position(|r| matches!(r, Row::Topic { id, .. } if *id == TopicId(2)))
            .unwrap();
        let epic = parent_index(&rows, embed).unwrap();
        assert!(matches!(rows[epic], Row::Epic { .. }));
        assert_eq!(parent_index(&rows, epic), Some(0));
        assert_eq!(parent_index(&rows, 0), None);
    }

    #[test]
    fn moving_passes_the_visible_neighbour_across_hidden_closed_topics() {
        // Stored: a, x (closed, folded away), b, c.
        let stored = ["a", "x", "b", "c"];
        let visible = ["a", "b", "c"];
        // b up passes a: index 0.
        assert_eq!(move_target(&stored, &visible, &"b", true), Some(0));
        // a down passes b: b's stored index (after removal, right after b).
        assert_eq!(move_target(&stored, &visible, &"a", false), Some(2));
        assert_eq!(move_target(&stored, &visible, &"a", true), None);
        assert_eq!(move_target(&stored, &visible, &"c", false), None);
        let projects = acta();
        let rows = rows(&projects, &Folds::default());
        let scroll = rows
            .iter()
            .find(|r| matches!(r, Row::Topic { id, .. } if *id == TopicId(3)))
            .unwrap();
        assert_eq!(
            stored_siblings(&projects, scroll).len(),
            4,
            "closed included"
        );
    }

    #[test]
    fn locations_name_project_and_epic() {
        let projects = acta();
        let at = locate_topic(&projects, TopicId(2)).unwrap();
        assert_eq!(at.project.unwrap().1, "Acta");
        assert_eq!(at.epic.unwrap().1, "Terminal");
        let loose = locate_topic(&projects, TopicId(1)).unwrap();
        assert!(loose.epic.is_none());
        assert!(locate_topic(&projects, TopicId(99)).is_none());
        assert_eq!(
            locate_epic(&projects, EpicId(1))
                .unwrap()
                .project
                .unwrap()
                .0,
            ProjectId(1)
        );
        assert!(locate_project(&projects, ProjectId(2)).is_some());
    }
}
