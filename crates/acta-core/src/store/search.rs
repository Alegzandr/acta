//! Full-text search (ADR 0003) over titles and content: projects, epics
//! and topics (title, description, state), timeline entries, notes,
//! questions and their answers, contacts (name, job title, company),
//! live knowledge (title, summary, body) and attached files (name, and
//! the text of a small text file). FTS5 with accents folded, joined to the
//! base tables to drop trashed rows; short excerpts with `snippet()`;
//! a match in the title ranks first, then bm25.
//!
//! Typed words are matched as words and, from four letters, by the stem
//! of the memory checker ([`crate::kb_quality::stem`], English and French
//! plurals and endings), so "fichiers" finds "fichier" and "configured"
//! finds "configuration".

use super::Store;
use crate::error::Result;
use crate::model::{
    AttachmentId, ContactId, EntryId, EpicId, KbId, KbKind, NoteId, Owner, Page, PageReq,
    ProjectId, QuestionId, Record, Status, TopicId,
};

/// Marks the start and end of a matched term in [`SearchHit::snippet`].
pub const SNIPPET_OPEN: char = '\u{2}';
pub const SNIPPET_CLOSE: char = '\u{3}';
/// Upper bound of hits ranked per query; pages are cut from these.
const MAX_HITS: i64 = 500;
/// Matches ranked per kind: the newest ones (palette budget, interaction.md
/// 7: 30 ms over 10,000 topics).
const PER_KIND: i64 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchKind {
    Topic,
    Entry,
    Note,
    Kb,
    Project,
    Epic,
    Question,
    Contact,
    Attachment,
}

#[derive(Clone, Debug, Default)]
pub struct SearchQuery {
    /// Words as typed; each is matched as a prefix-free term, the last one as
    /// a prefix. No FTS syntax is interpreted.
    pub text: String,
    pub project: Option<ProjectId>,
    /// Empty means every kind.
    pub kinds: Vec<SearchKind>,
    /// Topic statuses kept (empty: all). Applies to topics and to entries
    /// through their topic; notes and knowledge are not filtered by it.
    pub statuses: Vec<Status>,
    /// Only topics of this epic and their entries; the other kinds are
    /// left out when set.
    pub epic: Option<EpicId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub record: Record,
    pub project: ProjectId,
    /// The topic of an entry, for `T-42#14`.
    pub topic: Option<TopicId>,
    /// Title of the record (a question's text, a contact's name, a file's
    /// name); empty for entries and notes.
    pub title: String,
    /// The words matched the title (these rank first).
    pub in_title: bool,
    /// About a dozen words around the match, terms wrapped in
    /// [`SNIPPET_OPEN`] and [`SNIPPET_CLOSE`].
    pub snippet: String,
    pub rank: f64,
}

/// Turns typed text into a safe FTS5 query: every word quoted, the last
/// one a prefix, and a word of four letters or more also matched by its
/// stem as a prefix. `None` when there is nothing to search for.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .take(16)
        .collect();
    let n = words.len();
    if n == 0 {
        return None;
    }
    let terms: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let star = if i + 1 == n { "*" } else { "" };
            let folded = crate::kb_quality::fold(w);
            let stem = crate::kb_quality::stem(&folded);
            let alnum = stem.chars().all(char::is_alphanumeric);
            if alnum && stem.chars().count() >= 4 && stem != folded {
                format!("(\"{w}\"{star} OR \"{stem}\"*)")
            } else {
                format!("\"{w}\"{star}")
            }
        })
        .collect();
    Some(terms.join(" AND "))
}

/// The same query restricted to one column (a title), as an SQL string
/// literal.
fn in_column(col: &str, fts: &str) -> String {
    format!("'{col} : ({})'", fts.replace('\'', "''"))
}

impl Store {
    pub fn search(&self, query: &SearchQuery, page: PageReq) -> Result<Page<SearchHit>> {
        let (offset, limit) = page.clamped();
        let Some(fts) = fts_query(&query.text) else {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                offset,
                capped: false,
            });
        };
        let wants = |k: SearchKind| query.kinds.is_empty() || query.kinds.contains(&k);
        let project = query
            .project
            .map_or(String::new(), |p| format!(" AND x.project_id = {}", p.0));
        let mut topic_filter = String::new();
        if !query.statuses.is_empty() {
            let list: Vec<String> = query
                .statuses
                .iter()
                .map(|s| format!("'{}'", s.as_str()))
                .collect();
            topic_filter.push_str(&format!(" AND t.status IN ({})", list.join(", ")));
        }
        if let Some(e) = query.epic {
            topic_filter.push_str(&format!(" AND t.epic_id = {}", e.0));
        }
        let unfiled = query.epic.is_none();
        let mut parts = Vec::new();
        let mut tables: Vec<&str> = Vec::new();
        // One SELECT per kind: kind, id, project, topic, title, an empty
        // excerpt (made below for the page only: `snippet()` on every
        // match cost 140 ms for a common word over 200,000 entries), bm25,
        // whether the title matched.
        let titled = |table: &str, col: &str| {
            format!(
                "(x.id IN (SELECT rowid FROM {table} WHERE {table} MATCH {}))",
                in_column(col, &fts)
            )
        };
        if wants(SearchKind::Topic) {
            tables.push("topics_fts");
            parts.push(format!(
                "SELECT 'topic', x.id, x.project_id, NULL, x.title,
                        '', m.score, {}
                 FROM (SELECT rowid AS rid, bm25(topics_fts) AS score FROM topics_fts WHERE topics_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN topics x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL{project}{}",
                titled("topics_fts", "title"),
                topic_filter.replace("t.", "x.")
            ));
        }
        if wants(SearchKind::Entry) {
            tables.push("entries_fts");
            parts.push(format!(
                "SELECT 'entry', x.id, t.project_id AS project_id, x.topic_id, '',
                        '', m.score, 0
                 FROM (SELECT rowid AS rid, bm25(entries_fts) AS score FROM entries_fts WHERE entries_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN entries x ON x.id = m.rid
                 JOIN topics t ON t.id = x.topic_id
                 WHERE x.deleted_at IS NULL AND t.deleted_at IS NULL{}{topic_filter}",
                project.replace("x.project_id", "t.project_id")
            ));
        }
        if wants(SearchKind::Note) && unfiled {
            tables.push("notes_fts");
            parts.push(format!(
                "SELECT 'note', x.id, x.project_id, x.filed_topic_id, '',
                        '', m.score, 0
                 FROM (SELECT rowid AS rid, bm25(notes_fts) AS score FROM notes_fts WHERE notes_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN notes x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL{project}"
            ));
        }
        if wants(SearchKind::Kb) && unfiled {
            tables.push("kb_fts");
            parts.push(format!(
                "SELECT 'kb', x.id, x.project_id, x.topic_id, x.title,
                        '', m.score, {}
                 FROM (SELECT rowid AS rid, bm25(kb_fts) AS score FROM kb_fts WHERE kb_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN kb_entries x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL AND x.state = 'live'{project}",
                titled("kb_fts", "title")
            ));
        }
        if wants(SearchKind::Project) && unfiled {
            tables.push("projects_fts");
            parts.push(format!(
                "SELECT 'project', x.id, x.id, NULL, x.title,
                        '', m.score, {}
                 FROM (SELECT rowid AS rid, bm25(projects_fts) AS score FROM projects_fts WHERE projects_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN projects x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL{}",
                titled("projects_fts", "title"),
                project.replace("x.project_id", "x.id")
            ));
        }
        if wants(SearchKind::Epic) && unfiled {
            tables.push("epics_fts");
            parts.push(format!(
                "SELECT 'epic', x.id, x.project_id, NULL, x.title,
                        '', m.score, {}
                 FROM (SELECT rowid AS rid, bm25(epics_fts) AS score FROM epics_fts WHERE epics_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN epics x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL{project}",
                titled("epics_fts", "title")
            ));
        }
        if wants(SearchKind::Question) && unfiled {
            tables.push("questions_fts");
            parts.push(format!(
                "SELECT 'question', x.id, x.project_id, x.topic_id, x.text,
                        '', m.score, {}
                 FROM (SELECT rowid AS rid, bm25(questions_fts) AS score FROM questions_fts WHERE questions_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN questions x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL{project}",
                titled("questions_fts", "text")
            ));
        }
        if wants(SearchKind::Contact) && unfiled {
            tables.push("contacts_fts");
            // A contact at "all projects" is seen from every project.
            let contact_project = query.project.map_or(String::new(), |p| {
                format!(" AND (x.project_id IS NULL OR x.project_id = {})", p.0)
            });
            parts.push(format!(
                "SELECT 'contact', x.id, coalesce(x.project_id, 0), NULL, x.name,
                        '', m.score, {}
                 FROM (SELECT rowid AS rid, bm25(contacts_fts) AS score FROM contacts_fts WHERE contacts_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN contacts x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL{contact_project}",
                titled("contacts_fts", "name")
            ));
        }
        if wants(SearchKind::Attachment) && unfiled {
            tables.push("attachments_fts");
            parts.push(format!(
                "SELECT 'attachment', x.id, x.project_id,
                        coalesce(x.topic_id, (SELECT e.topic_id FROM entries e WHERE e.id = x.entry_id)),
                        x.name,
                        '', m.score, {}
                 FROM (SELECT rowid AS rid, bm25(attachments_fts) AS score FROM attachments_fts WHERE attachments_fts MATCH ?1
                       ORDER BY rowid DESC LIMIT {PER_KIND}) m JOIN attachments x ON x.id = m.rid
                 WHERE x.deleted_at IS NULL{project}",
                titled("attachments_fts", "name")
            ));
        }
        if parts.is_empty() {
            return Ok(Page {
                items: Vec::new(),
                total: 0,
                offset,
                capped: false,
            });
        }
        let sql = format!(
            "SELECT * FROM ({}) ORDER BY 8 DESC, 7, 1, 2 LIMIT {}",
            parts.join(" UNION ALL "),
            MAX_HITS + 1
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
                    "project" => Record::Project(ProjectId(id)),
                    "epic" => Record::Epic(EpicId(id)),
                    "question" => Record::Question(QuestionId(id)),
                    "contact" => Record::Contact(ContactId(id)),
                    "attachment" => Record::Attachment(AttachmentId(id)),
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
                    in_title: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        // Each kind ranks its newest PER_KIND matches only (bm25 on every
        // match of a common word cost about 120 ms over 200,000 entries):
        // a kind that has more says the total is capped.
        let mut capped = all.len() as i64 > MAX_HITS;
        for table in tables {
            if capped {
                break;
            }
            let n: i64 = self.conn.query_row(
                &format!(
                    "SELECT count(*) FROM (SELECT rowid FROM {table} WHERE {table} MATCH ?1 LIMIT {})",
                    PER_KIND + 1
                ),
                [&fts],
                |r| r.get(0),
            )?;
            capped = n > PER_KIND;
        }
        let total = (all.len() as i64).min(MAX_HITS);
        let mut items: Vec<SearchHit> = all
            .into_iter()
            .take(MAX_HITS as usize)
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        let (o, c) = (SNIPPET_OPEN, SNIPPET_CLOSE);
        for hit in &mut items {
            let table = match hit.record {
                Record::Topic(_) => "topics_fts",
                Record::Entry(_) => "entries_fts",
                Record::Note(_) => "notes_fts",
                Record::Project(_) => "projects_fts",
                Record::Epic(_) => "epics_fts",
                Record::Question(_) => "questions_fts",
                Record::Contact(_) => "contacts_fts",
                Record::Attachment(_) => "attachments_fts",
                _ => "kb_fts",
            };
            let id = match hit.record {
                Record::Topic(TopicId(x))
                | Record::Entry(EntryId(x))
                | Record::Note(NoteId(x))
                | Record::Project(ProjectId(x))
                | Record::Epic(EpicId(x))
                | Record::Question(QuestionId(x))
                | Record::Contact(ContactId(x))
                | Record::Attachment(AttachmentId(x))
                | Record::Kb(KbId(x)) => x,
                _ => continue,
            };
            hit.snippet = self.conn.query_row(
                &format!(
                    "SELECT snippet({table}, -1, '{o}', '{c}', '…', 12) FROM {table}
                     WHERE {table} MATCH ?1 AND rowid = ?2"
                ),
                rusqlite::params![fts, id],
                |r| r.get(0),
            )?;
        }
        Ok(Page {
            items,
            total,
            offset,
            capped,
        })
    }
}

impl Store {
    /// Full-text search over the knowledge an agent at `owner` sees (its own,
    /// its epic's, its project's), best match first. Hits carry the entry
    /// title and a short excerpt. Live entries only: superseded and retired
    /// ones are read by id (ADR 0015, 2).
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
                capped: false,
            });
        };
        let kind = kind.map_or(String::new(), |k| format!(" AND k.kind = '{}'", k.as_str()));
        let (o, c) = (SNIPPET_OPEN, SNIPPET_CLOSE);
        let filter = format!(
            "kb_fts MATCH ?1 AND k.deleted_at IS NULL AND k.state = 'live' AND {scope}{kind}"
        );
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
                    in_title: false,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Page {
            items,
            total,
            offset,
            capped: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::fts_query;

    #[test]
    fn typed_text_cannot_inject_fts_syntax() {
        assert_eq!(
            fts_query("key chain"),
            Some("\"key\" AND \"chain\"*".into())
        );
        assert_eq!(
            fts_query("a\" OR b"),
            Some("\"a\" AND \"OR\" AND \"b\"*".into())
        );
        // From four letters, the stem as a prefix too.
        assert_eq!(
            fts_query("fichiers configured"),
            Some("(\"fichiers\" OR \"fichi\"*) AND (\"configured\"* OR \"configur\"*)".into())
        );
        assert_eq!(fts_query("  - * "), None);
        assert_eq!(fts_query(""), None);
    }
}
