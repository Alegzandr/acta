//! Full-text search over topics, timeline entries, notes and knowledge
//! (ADR 0003): FTS5, joined to the base tables to drop trashed rows, short
//! excerpts with `snippet()`, ordered by rank.

use super::Store;
use crate::error::Result;
use crate::model::{
    EntryId, KbId, KbKind, NoteId, Owner, Page, PageReq, ProjectId, Record, TopicId,
};

/// Marks the start and end of a matched term in [`SearchHit::snippet`].
pub const SNIPPET_OPEN: char = '\u{2}';
pub const SNIPPET_CLOSE: char = '\u{3}';
/// Upper bound of hits ranked per query; pages are cut from these.
const MAX_HITS: i64 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchKind {
    Topic,
    Entry,
    Note,
    Kb,
}

#[derive(Clone, Debug, Default)]
pub struct SearchQuery {
    /// Words as typed; each is matched as a prefix-free term, the last one as
    /// a prefix. No FTS syntax is interpreted.
    pub text: String,
    pub project: Option<ProjectId>,
    /// Empty means every kind.
    pub kinds: Vec<SearchKind>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub record: Record,
    pub project: ProjectId,
    /// The topic of an entry, for `T-42#14`.
    pub topic: Option<TopicId>,
    /// Title of the topic or knowledge entry; empty for entries and notes.
    pub title: String,
    /// About a dozen words around the match, terms wrapped in
    /// [`SNIPPET_OPEN`] and [`SNIPPET_CLOSE`].
    pub snippet: String,
    pub rank: f64,
}

/// Turns typed text into a safe FTS5 query: every word quoted, the last one
/// a prefix. `None` when there is nothing to search for.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .collect();
    let (last, rest) = words.split_last()?;
    let mut q: Vec<String> = rest.iter().map(|w| format!("\"{w}\"")).collect();
    q.push(format!("\"{last}\"*"));
    Some(q.join(" "))
}

impl Store {
    pub fn search(&self, query: &SearchQuery, page: PageReq) -> Result<Page<SearchHit>> {
        let (offset, limit) = page.clamped();
        let Some(fts) = fts_query(&query.text) else {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                offset,
            });
        };
        let wants = |k: SearchKind| query.kinds.is_empty() || query.kinds.contains(&k);
        let project = query
            .project
            .map_or(String::new(), |p| format!(" AND x.project_id = {}", p.0));
        let (o, c) = (SNIPPET_OPEN, SNIPPET_CLOSE);
        let mut parts = Vec::new();
        if wants(SearchKind::Topic) {
            parts.push(format!(
                "SELECT 'topic', x.id, x.project_id, NULL, x.title,
                        snippet(topics_fts, -1, '{o}', '{c}', '…', 12), bm25(topics_fts)
                 FROM topics_fts JOIN topics x ON x.id = topics_fts.rowid
                 WHERE topics_fts MATCH ?1 AND x.deleted_at IS NULL{project}"
            ));
        }
        if wants(SearchKind::Entry) {
            parts.push(format!(
                "SELECT 'entry', x.id, t.project_id AS project_id, x.topic_id, '',
                        snippet(entries_fts, -1, '{o}', '{c}', '…', 12), bm25(entries_fts)
                 FROM entries_fts JOIN entries x ON x.id = entries_fts.rowid
                 JOIN topics t ON t.id = x.topic_id
                 WHERE entries_fts MATCH ?1 AND x.deleted_at IS NULL AND t.deleted_at IS NULL{}",
                project.replace("x.project_id", "t.project_id")
            ));
        }
        if wants(SearchKind::Note) {
            parts.push(format!(
                "SELECT 'note', x.id, x.project_id, x.filed_topic_id, '',
                        snippet(notes_fts, -1, '{o}', '{c}', '…', 12), bm25(notes_fts)
                 FROM notes_fts JOIN notes x ON x.id = notes_fts.rowid
                 WHERE notes_fts MATCH ?1 AND x.deleted_at IS NULL{project}"
            ));
        }
        if wants(SearchKind::Kb) {
            parts.push(format!(
                "SELECT 'kb', x.id, x.project_id, x.topic_id, x.title,
                        snippet(kb_fts, -1, '{o}', '{c}', '…', 12), bm25(kb_fts)
                 FROM kb_fts JOIN kb_entries x ON x.id = kb_fts.rowid
                 WHERE kb_fts MATCH ?1 AND x.deleted_at IS NULL{project}"
            ));
        }
        if parts.is_empty() {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                offset,
            });
        }
        let sql = format!(
            "SELECT * FROM ({}) ORDER BY 7, 2 LIMIT {MAX_HITS}",
            parts.join(" UNION ALL ")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let all: Vec<SearchHit> = stmt
            .query_map([&fts], |r| {
                let kind: String = r.get(0)?;
                let id: i64 = r.get(1)?;
                let record = match kind.as_str() {
                    "topic" => Record::Topic(TopicId(id)),
                    "entry" => Record::Entry(EntryId(id)),
                    "note" => Record::Note(NoteId(id)),
                    _ => Record::Kb(KbId(id)),
                };
                let topic = match record {
                    Record::Topic(t) => Some(t),
                    _ => r.get::<_, Option<i64>>(3)?.map(TopicId),
                };
                Ok(SearchHit {
                    record,
                    project: ProjectId(r.get(2)?),
                    topic,
                    title: r.get(4)?,
                    snippet: r.get(5)?,
                    rank: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let total = all.len() as i64;
        let items = all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(Page {
            items,
            total,
            offset,
        })
    }
}

impl Store {
    /// Full-text search over the knowledge an agent at `owner` sees (its own,
    /// its epic's, its project's), best match first. Hits carry the entry
    /// title and a short excerpt.
    pub fn kb_search(
        &self,
        owner: Owner,
        text: &str,
        kind: Option<KbKind>,
        page: PageReq,
    ) -> Result<Page<SearchHit>> {
        let (offset, limit) = page.clamped();
        let scope = self.scope_condition(owner)?;
        let Some(fts) = fts_query(text) else {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                offset,
            });
        };
        let kind = kind.map_or(String::new(), |k| format!(" AND k.kind = '{}'", k.as_str()));
        let (o, c) = (SNIPPET_OPEN, SNIPPET_CLOSE);
        let filter = format!("kb_fts MATCH ?1 AND k.deleted_at IS NULL AND {scope}{kind}");
        let total: i64 = self.conn.query_row(
            &format!(
                "SELECT count(*) FROM kb_fts JOIN kb_entries k ON k.id = kb_fts.rowid WHERE {filter}"
            ),
            [&fts],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT k.id, k.project_id, k.topic_id, k.title,
                    snippet(kb_fts, -1, '{o}', '{c}', '…', 16), bm25(kb_fts)
             FROM kb_fts JOIN kb_entries k ON k.id = kb_fts.rowid
             WHERE {filter}
             ORDER BY 6, 1 LIMIT {limit} OFFSET {offset}"
        ))?;
        let items = stmt
            .query_map([&fts], |r| {
                Ok(SearchHit {
                    record: Record::Kb(KbId(r.get(0)?)),
                    project: ProjectId(r.get(1)?),
                    topic: r.get::<_, Option<i64>>(2)?.map(TopicId),
                    title: r.get(3)?,
                    snippet: r.get(4)?,
                    rank: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Page {
            items,
            total,
            offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::fts_query;

    #[test]
    fn typed_text_cannot_inject_fts_syntax() {
        assert_eq!(fts_query("key chain"), Some("\"key\" \"chain\"*".into()));
        assert_eq!(fts_query("a\" OR b"), Some("\"a\" \"OR\" \"b\"*".into()));
        assert_eq!(fts_query("  - * "), None);
        assert_eq!(fts_query(""), None);
    }
}
