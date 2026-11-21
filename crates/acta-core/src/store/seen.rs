//! Read markers (docs/design/data.md, request 1): when this installation
//! last showed a topic or a knowledge entry, so the unread dot can mark
//! what Claude Code changed since. Local only: the markers are never
//! exported or imported and never touch a record's `updated_at`.

use std::collections::HashSet;

use rusqlite::params;

use super::Store;
use crate::Result;
use crate::model::{KbId, ProjectId, Record, TopicId};
use crate::time::now_ms;

/// What Claude Code wrote last and this installation has not shown since.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unread {
    pub topics: HashSet<TopicId>,
    /// Each entry with its project (the Knowledge row of the tree).
    pub kb: Vec<(KbId, ProjectId)>,
}

impl Unread {
    pub fn has_kb(&self, id: KbId) -> bool {
        self.kb.iter().any(|(k, _)| *k == id)
    }

    pub fn project_has_kb(&self, project: ProjectId) -> bool {
        self.kb.iter().any(|(_, p)| *p == project)
    }
}

/// The marker kind and table of a record, for the two kinds that carry one.
fn marker(record: Record) -> Option<(&'static str, &'static str, i64)> {
    match record {
        Record::Topic(id) => Some(("topic", "topics", id.0)),
        Record::Kb(id) => Some(("kb", "kb_entries", id.0)),
        _ => None,
    }
}

impl Store {
    /// Mark `record` as shown now. Writes nothing when the marker is
    /// already as new as the record's last write (so showing a page that is
    /// up to date costs no commit), or for kinds without markers. Returns
    /// whether a marker was written.
    pub fn mark_seen(&mut self, record: Record) -> Result<bool> {
        let Some((kind, table, id)) = marker(record) else {
            return Ok(false);
        };
        let tx = self.write()?;
        let n = tx.execute(
            &format!(
                "INSERT INTO read_markers (kind, record_id, last_seen_at)
                 SELECT ?1, id, MAX(updated_at, ?3) FROM {table} WHERE id = ?2
                 ON CONFLICT (kind, record_id) DO UPDATE SET last_seen_at = excluded.last_seen_at
                 WHERE read_markers.last_seen_at < (SELECT updated_at FROM {table} WHERE id = ?2)"
            ),
            params![kind, id, now_ms()],
        )?;
        tx.commit()?;
        Ok(n > 0)
    }

    /// Live topics and knowledge entries whose last write is Claude Code's
    /// and came after this installation last showed them (or that it never
    /// showed).
    pub fn unread(&self) -> Result<Unread> {
        let topics = self
            .conn
            .prepare(
                "SELECT t.id FROM topics t
                 LEFT JOIN read_markers m ON m.kind = 'topic' AND m.record_id = t.id
                 WHERE t.deleted_at IS NULL AND t.updated_by = 'claude'
                   AND (m.last_seen_at IS NULL OR t.updated_at > m.last_seen_at)",
            )?
            .query_map([], |r| Ok(TopicId(r.get(0)?)))?
            .collect::<rusqlite::Result<HashSet<_>>>()?;
        let kb = self
            .conn
            .prepare(
                "SELECT k.id, k.project_id FROM kb_entries k
                 LEFT JOIN read_markers m ON m.kind = 'kb' AND m.record_id = k.id
                 WHERE k.deleted_at IS NULL AND k.updated_by = 'claude'
                   AND (m.last_seen_at IS NULL OR k.updated_at > m.last_seen_at)
                 ORDER BY k.id",
            )?
            .query_map([], |r| Ok((KbId(r.get(0)?), ProjectId(r.get(1)?))))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Unread { topics, kb })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Author, KbKind, Owner, Status};
    use crate::store::{NewKb, NewTopic, StoreConfig, TopicPatch};

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let keys = crate::keys::MemoryKeyStore::new();
        let config = StoreConfig {
            data_dir: dir.path().to_path_buf(),
            keys: keys.stores(),
        };
        let store = Store::open(&config).unwrap();
        (dir, store)
    }

    fn topic(store: &mut Store, by: Author) -> TopicId {
        let project = store.create_project("Acta", "", Author::Human).unwrap();
        store
            .create_topic(
                NewTopic {
                    project,
                    epic: None,
                    title: "Alexandre's topic".into(),
                    description: String::new(),
                    status: Status::Draft,
                },
                by,
            )
            .unwrap()
    }

    #[test]
    fn a_topic_claude_code_wrote_is_unread_until_shown() {
        let (_dir, mut s) = store();
        let mine = topic(&mut s, Author::Human);
        let theirs = topic(&mut s, Author::Claude);
        let unread = s.unread().unwrap();
        assert!(unread.topics.contains(&theirs));
        assert!(!unread.topics.contains(&mine), "the human's own write");
        assert!(s.mark_seen(Record::Topic(theirs)).unwrap());
        assert!(s.unread().unwrap().topics.is_empty());
        // Shown again with nothing new: no write.
        assert!(!s.mark_seen(Record::Topic(theirs)).unwrap());
        // Claude Code writes again, later: unread again.
        s.update_topic(
            theirs,
            TopicPatch {
                status: Some(Status::Active),
                ..TopicPatch::default()
            },
            Author::Claude,
        )
        .unwrap();
        s.conn
            .execute(
                "UPDATE topics SET updated_at = updated_at + 60000 WHERE id = ?1",
                [theirs.0],
            )
            .unwrap();
        assert!(s.unread().unwrap().topics.contains(&theirs));
        assert!(s.mark_seen(Record::Topic(theirs)).unwrap());
        assert!(s.unread().unwrap().topics.is_empty());
    }

    #[test]
    fn a_memory_claude_code_wrote_is_unread_with_its_project() {
        let (_dir, mut s) = store();
        let t = topic(&mut s, Author::Human);
        let project = s.topic(t).unwrap().project;
        let kb = s
            .kb_create(
                NewKb {
                    owner: Owner::Topic(t),
                    kind: KbKind::Memory,
                    title: "Alexandre uses AZERTY".into(),
                    summary: "French keyboard layout".into(),
                    body: "AZERTY.".into(),
                    from_file: false,
                },
                Author::Claude,
            )
            .unwrap();
        let unread = s.unread().unwrap();
        assert!(unread.has_kb(kb) && unread.project_has_kb(project));
        s.mark_seen(Record::Kb(kb)).unwrap();
        assert!(!s.unread().unwrap().has_kb(kb));
        // Other kinds carry no marker.
        assert!(!s.mark_seen(Record::Project(project)).unwrap());
        // Deleted: not unread.
        let other = topic(&mut s, Author::Claude);
        s.delete(Record::Topic(other), Author::Human).unwrap();
        assert!(!s.unread().unwrap().topics.contains(&other));
    }

    #[test]
    fn markers_are_not_exported() {
        let (_dir, mut s) = store();
        let t = topic(&mut s, Author::Claude);
        s.mark_seen(Record::Topic(t)).unwrap();
        let json = serde_json::to_string(&s.export(None).unwrap()).unwrap();
        assert!(!json.contains("read_markers") && !json.contains("seen"));
    }

    #[test]
    fn imported_records_count_as_seen() {
        let (_a, mut source) = store();
        let t = topic(&mut source, Author::Claude);
        let project = source.topic(t).unwrap().project;
        source
            .kb_create(
                NewKb {
                    owner: Owner::Project(project),
                    kind: KbKind::Memory,
                    title: "Alexandre's memory".into(),
                    summary: "Imported".into(),
                    body: "Imported.".into(),
                    from_file: false,
                },
                Author::Claude,
            )
            .unwrap();
        let file = source.export(None).unwrap();
        let (_b, mut target) = store();
        target.import(&file, false).unwrap();
        assert_eq!(target.unread().unwrap(), Unread::default());
    }

    #[test]
    fn records_present_at_the_migration_count_as_seen() {
        let dir = tempfile::tempdir().unwrap();
        let keys = crate::keys::MemoryKeyStore::new();
        let config = StoreConfig {
            data_dir: dir.path().to_path_buf(),
            keys: keys.stores(),
        };
        // A database written by 0.4.0: schema 2, a topic by Claude Code.
        let mut v2 = Store::open_with(&config, &crate::schema::MIGRATIONS[..2]).unwrap();
        let t = topic(&mut v2, Author::Claude);
        drop(v2);
        let s = Store::open(&config).unwrap();
        assert!(!s.unread().unwrap().topics.contains(&t));
    }
}
